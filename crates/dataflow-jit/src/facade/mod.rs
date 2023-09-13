use crate::{
    codegen::{
        json::{DeserializeJsonFn, JsonDeserConfig},
        CodegenConfig, NativeLayout, NativeLayoutCache, VTable,
    },
    dataflow::{CompiledDataflow, JitHandle, RowInput, RowOutput},
    ir::{
        literal::{NullableConstant, RowLiteral, StreamCollection},
        nodes::StreamLayout,
        pretty::{Arena, Pretty, DEFAULT_WIDTH},
        ColumnType, Constant, Graph, GraphExt, LayoutId, NodeId, RowLayout, Validator,
    },
    row::{row_from_literal, Row, UninitRow},
    thin_str::ThinStrRef,
};
use chrono::{TimeZone, Utc};
use cranelift_module::FuncId;
use csv::StringRecord;
use dbsp::{
    trace::{BatchReader, Cursor},
    CollectionHandle, DBSPHandle, Error, Runtime,
};
use rust_decimal::Decimal;
use serde_json::{Deserializer, Value};
use std::{
    collections::BTreeMap, io::Read, mem::transmute, ops::Not, path::Path, thread, time::Instant,
};

// TODO: A lot of this still needs fleshing out, mainly the little tweaks that
// users may want to add to parsing and how to do that ergonomically.
// We also need checks to make sure that the type is being fully initialized, as
// well as support for parsing maps from csv

pub struct Demands {
    #[allow(clippy::type_complexity)]
    csv: BTreeMap<LayoutId, Vec<(usize, usize, Option<String>)>>,
    json_deser: BTreeMap<LayoutId, JsonDeserConfig>,
}

impl Demands {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            csv: BTreeMap::new(),
            json_deser: BTreeMap::new(),
        }
    }

    pub fn add_csv_deserialize(
        &mut self,
        layout: LayoutId,
        column_mappings: Vec<(usize, usize, Option<String>)>,
    ) {
        let displaced = self.csv.insert(layout, column_mappings);
        assert_eq!(displaced, None);
    }

    pub fn add_json_deserialize(&mut self, layout: LayoutId, mappings: JsonDeserConfig) {
        let displaced = self.json_deser.insert(layout, mappings);
        assert_eq!(displaced, None);
    }
}

#[derive(Clone)]
pub struct JsonSetHandle {
    handle: CollectionHandle<Row, i32>,
    deserialize_fn: DeserializeJsonFn,
    vtable: &'static VTable,
}

impl JsonSetHandle {
    pub fn push(&self, key: &[u8], weight: i32) -> Result<(), serde_json::Error> {
        let value: Value = serde_json::from_slice(key)?;
        let key = unsafe {
            let mut uninit = UninitRow::new(self.vtable);
            (self.deserialize_fn)(uninit.as_mut_ptr(), &value);
            uninit.assume_init()
        };

        self.handle.push(key, weight);

        Ok(())
    }

    pub fn clear_input(&self) {
        self.handle.clear_input();
    }
}

pub struct DbspCircuit {
    jit: JitHandle,
    runtime: DBSPHandle,
    /// The input handles of all source nodes, will be `None` if the source is
    /// unused
    inputs: BTreeMap<NodeId, (Option<RowInput>, StreamLayout)>,
    /// The output handles of all sink nodes, will be `None` if the sink is
    /// unreachable
    outputs: BTreeMap<NodeId, (Option<RowOutput>, StreamLayout)>,
    csv_demands: BTreeMap<LayoutId, FuncId>,
    json_deser_demands: BTreeMap<LayoutId, FuncId>,
    layout_cache: NativeLayoutCache,
}

impl DbspCircuit {
    pub fn new(
        mut graph: Graph,
        optimize: bool,
        workers: usize,
        config: CodegenConfig,
        demands: Demands,
    ) -> Self {
        let arena = Arena::<()>::new();
        tracing::trace!(
            "created circuit from graph:\n{}",
            Pretty::pretty(&graph, &arena, graph.layout_cache()).pretty(DEFAULT_WIDTH),
        );

        let sources = graph.source_nodes();
        let sinks = graph.sink_nodes();

        {
            let mut validator = Validator::new(graph.layout_cache().clone());
            validator
                .validate_graph(&graph)
                .expect("failed to validate graph before optimization");

            if optimize {
                graph.optimize();
                tracing::trace!("optimized graph for dbsp circuit: {graph:#?}");

                validator
                    .validate_graph(&graph)
                    .expect("failed to validate graph after optimization");

                tracing::trace!(
                    "optimized graph:\n{}",
                    Pretty::pretty(&graph, &arena, graph.layout_cache()).pretty(DEFAULT_WIDTH),
                );
            }
        }

        let (mut json_deser_demands, mut csv_demands) = (BTreeMap::new(), BTreeMap::new());
        let (dataflow, jit, layout_cache) = CompiledDataflow::new(&graph, config, |codegen| {
            json_deser_demands = demands
                .json_deser
                .into_iter()
                .map(|(layout, mappings)| {
                    debug_assert_eq!(layout, mappings.layout);
                    let from_json = codegen.deserialize_json(&mappings);
                    (layout, from_json)
                })
                .collect();

            csv_demands = demands
                .csv
                .into_iter()
                .map(|(layout, mappings)| {
                    let from_csv = codegen.codegen_layout_from_csv(layout, &mappings);
                    (layout, from_csv)
                })
                .collect();
        });

        let (runtime, (inputs, outputs)) =
            Runtime::init_circuit(workers, move |circuit| dataflow.construct(circuit))
                .expect("failed to construct runtime");

        // Account for unused sources
        let mut inputs: BTreeMap<_, _> = inputs
            .into_iter()
            .map(|(id, (input, layout))| (id, (Some(input), layout)))
            .collect();
        for (source, layout) in sources {
            inputs.entry(source).or_insert((None, layout));
        }

        // Account for unreachable sinks
        let mut outputs: BTreeMap<_, _> = outputs
            .into_iter()
            .map(|(id, (output, layout))| (id, (Some(output), layout)))
            .collect();
        for (sink, layout) in sinks {
            outputs.entry(sink).or_insert((None, layout));
        }

        Self {
            jit,
            runtime,
            inputs,
            outputs,
            csv_demands,
            json_deser_demands,
            layout_cache,
        }
    }

    pub fn step(&mut self) -> Result<(), Error> {
        tracing::info!("stepping circuit");
        let start = Instant::now();

        let result = self.runtime.step();

        let elapsed = start.elapsed();
        tracing::info!(
            "step took {elapsed:#?} and finished {}successfully",
            if result.is_err() { "un" } else { "" },
        );

        result
    }

    pub fn kill(self) -> thread::Result<()> {
        tracing::trace!("killing circuit");
        let result = self.runtime.kill();

        drop(self.inputs);
        drop(self.outputs);
        unsafe { self.jit.free_memory() };

        result
    }

    /// Creates a new [`JsonSetHandle`] for ingesting json
    ///
    /// Returns [`None`] if the target source node is unreachable
    ///
    /// # Safety
    ///
    /// The produced `JsonSetHandle` must be dropped before the [`DbspCircuit`]
    /// that created it, using the handle after the parent circuit has shut down
    /// is undefined behavior
    // TODO: We should probably wrap the innards of `DbspCircuit` in a struct
    // and arc and handles should hold a reference to that (maybe even a weak ref).
    // Alternatively we could use lifetimes, but I'm not 100% sure how that would
    // interact with consumers
    pub unsafe fn json_input_set(&mut self, target: NodeId) -> Option<JsonSetHandle> {
        let (input, layout) = self.inputs.get(&target).unwrap_or_else(|| {
            panic!("attempted to append to {target}, but {target} is not a source node or doesn't exist");
        });
        let layout = layout.as_set().unwrap_or_else(|| {
            panic!(
                "called `DbspCircuit::json_input_set()` on node {target} which is a map, not a set",
            )
        });

        let handle = input.as_ref()?.as_set().unwrap().clone();
        let vtable = unsafe { &*self.jit.vtables()[&layout] };
        let deserialize_fn = unsafe {
            transmute::<_, DeserializeJsonFn>(
                self.jit
                    .jit
                    .get_finalized_function(self.json_deser_demands[&layout]),
            )
        };

        Some(JsonSetHandle {
            handle,
            deserialize_fn,
            vtable,
        })
    }

    pub fn append_input(&mut self, target: NodeId, data: &StreamCollection) {
        let (input, layout) = self.inputs.get_mut(&target).unwrap_or_else(|| {
            panic!("attempted to append to {target}, but {target} is not a source node or doesn't exist");
        });

        if let Some(input) = input {
            match data {
                StreamCollection::Set(set) => {
                    tracing::trace!("appending a set with {} values to {target}", set.len());

                    let key_layout = layout.unwrap_set();
                    let key_vtable = unsafe { &*self.jit.vtables()[&key_layout] };
                    let key_layout = self.layout_cache.layout_of(key_layout);

                    let mut batch = Vec::with_capacity(set.len());
                    for (literal, diff) in set {
                        let key = unsafe { row_from_literal(literal, key_vtable, &key_layout) };
                        batch.push((key, *diff));
                    }

                    input.as_set_mut().unwrap().append(&mut batch);
                }

                StreamCollection::Map(map) => {
                    tracing::trace!("appending a map with {} values to {target}", map.len());

                    let (key_layout, value_layout) = layout.unwrap_map();
                    let (key_vtable, value_vtable) = unsafe {
                        (
                            &*self.jit.vtables()[&key_layout],
                            &*self.jit.vtables()[&value_layout],
                        )
                    };
                    let (key_layout, value_layout) = (
                        self.layout_cache.layout_of(key_layout),
                        self.layout_cache.layout_of(value_layout),
                    );

                    let mut batch = Vec::with_capacity(map.len());
                    for (key_literal, value_literal, diff) in map {
                        let key = unsafe { row_from_literal(key_literal, key_vtable, &key_layout) };
                        let value =
                            unsafe { row_from_literal(value_literal, value_vtable, &value_layout) };
                        batch.push((key, (value, *diff)));
                    }

                    input.as_map_mut().unwrap().append(&mut batch);
                }
            }

        // If the source is unused, do nothing
        } else {
            tracing::info!("appended csv file to source {target} which is unused, doing nothing");
        }
    }

    // TODO: We probably want other ways to ingest json, e.g. `&[u8]`, `R: Read`,
    // etc.
    pub fn append_json_input<R>(&mut self, target: NodeId, json: R)
    where
        R: Read,
    {
        let (input, layout) = self.inputs.get_mut(&target).unwrap_or_else(|| {
            panic!("attempted to append to {target}, but {target} is not a source node or doesn't exist");
        });

        if let Some(input) = input {
            let start = Instant::now();

            let records = match *layout {
                StreamLayout::Set(key_layout) => {
                    let key_vtable = unsafe { &*self.jit.vtables()[&key_layout] };
                    let deserialize_json = unsafe {
                        transmute::<_, DeserializeJsonFn>(
                            self.jit
                                .jit
                                .get_finalized_function(self.json_deser_demands[&key_layout]),
                        )
                    };

                    let mut batch = Vec::new();
                    let stream = Deserializer::from_reader(json).into_iter::<Value>();
                    for value in stream {
                        // FIXME: Error handling
                        let value = value.unwrap();

                        let mut row = UninitRow::new(key_vtable);
                        unsafe { deserialize_json(row.as_mut_ptr(), &value) }
                        batch.push((unsafe { row.assume_init() }, 1));
                    }

                    let records = batch.len();
                    input.as_set_mut().unwrap().append(&mut batch);
                    records
                }

                StreamLayout::Map(..) => todo!(),
            };

            let elapsed = start.elapsed();
            tracing::info!("ingested {records} records for {target} in {elapsed:#?}");

        // If the source is unused, do nothing
        } else {
            tracing::info!("appended json to source {target} which is unused, doing nothing");
        }
    }

    pub fn append_json_record(&mut self, target: NodeId, record: &[u8]) {
        let (input, layout) = self.inputs.get_mut(&target).unwrap_or_else(|| {
            panic!("attempted to append to {target}, but {target} is not a source node or doesn't exist");
        });

        if let Some(input) = input {
            let start = Instant::now();

            match *layout {
                StreamLayout::Set(key_layout) => {
                    let key_vtable = unsafe { &*self.jit.vtables()[&key_layout] };
                    let deserialize_json = unsafe {
                        transmute::<_, DeserializeJsonFn>(
                            self.jit
                                .jit
                                .get_finalized_function(self.json_deser_demands[&key_layout]),
                        )
                    };

                    let value = serde_json::from_slice::<Value>(record).unwrap();
                    let mut row = UninitRow::new(key_vtable);
                    unsafe { deserialize_json(row.as_mut_ptr(), &value) }
                    input
                        .as_set_mut()
                        .unwrap()
                        .push(unsafe { row.assume_init() }, 1);
                }

                StreamLayout::Map(..) => todo!(),
            }

            let elapsed = start.elapsed();
            tracing::info!("ingested 1 record for {target} in {elapsed:#?}");

        // If the source is unused, do nothing
        } else {
            tracing::info!("appended json to source {target} which is unused, doing nothing");
        }
    }

    pub fn append_csv_input(&mut self, target: NodeId, path: &Path) {
        let (input, layout) = self.inputs.get_mut(&target).unwrap_or_else(|| {
            panic!("attempted to append to {target}, but {target} is not a source node or doesn't exist");
        });

        if let Some(input) = input {
            let mut csv = csv::ReaderBuilder::new()
                .has_headers(false)
                .from_path(path)
                .unwrap();

            let start = Instant::now();

            let records = match *layout {
                StreamLayout::Set(key_layout) => {
                    let key_vtable = unsafe { &*self.jit.vtables()[&key_layout] };
                    let marshall_csv = unsafe {
                        transmute::<_, unsafe extern "C" fn(*mut u8, *const StringRecord)>(
                            self.jit
                                .jit
                                .get_finalized_function(self.csv_demands[&key_layout]),
                        )
                    };

                    let (mut batch, mut buf) = (Vec::new(), StringRecord::new());
                    while csv.read_record(&mut buf).unwrap() {
                        let mut row = UninitRow::new(key_vtable);
                        unsafe { marshall_csv(row.as_mut_ptr(), &buf as *const StringRecord) };
                        batch.push((unsafe { row.assume_init() }, 1));
                    }

                    let records = batch.len();
                    input.as_set_mut().unwrap().append(&mut batch);
                    records
                }

                StreamLayout::Map(..) => todo!(),
            };

            let elapsed = start.elapsed();
            tracing::info!("ingested {records} records for {target} in {elapsed:#?}");

        // If the source is unused, do nothing
        } else {
            tracing::info!("appended csv file to source {target} which is unused, doing nothing");
        }
    }

    pub fn consolidate_output(&mut self, output: NodeId) -> StreamCollection {
        let (output, layout) = self.outputs.get(&output).unwrap_or_else(|| {
            panic!("attempted to consolidate data from {output}, but {output} is not a sink node or doesn't exist");
        });

        if let Some(output) = output {
            match output {
                RowOutput::Set(output) => {
                    let key_layout = layout.unwrap_set();
                    let (native_key_layout, key_layout) = self.layout_cache.get_layouts(key_layout);

                    let set = output.consolidate();
                    // println!("{set}");
                    let mut contents = Vec::with_capacity(set.len());

                    let mut cursor = set.cursor();
                    while cursor.key_valid() {
                        let diff = cursor.weight();
                        let key = cursor.key();

                        let key =
                            unsafe { row_literal_from_row(key, &native_key_layout, &key_layout) };
                        contents.push((key, diff));

                        cursor.step_key();
                    }

                    StreamCollection::Set(contents)
                }

                RowOutput::Map(output) => {
                    let (key_layout, value_layout) = layout.unwrap_map();
                    let (native_key_layout, key_layout) = self.layout_cache.get_layouts(key_layout);
                    let (native_value_layout, value_layout) =
                        self.layout_cache.get_layouts(value_layout);

                    let map = output.consolidate();
                    let mut contents = Vec::with_capacity(map.len());

                    let mut cursor = map.cursor();
                    while cursor.key_valid() {
                        let diff = cursor.weight();
                        let key = cursor.key();

                        let key_literal =
                            unsafe { row_literal_from_row(key, &native_key_layout, &key_layout) };

                        while cursor.val_valid() {
                            let value = cursor.val();
                            let value_literal = unsafe {
                                row_literal_from_row(value, &native_value_layout, &value_layout)
                            };

                            cursor.step_val();

                            if cursor.val_valid() {
                                contents.push((key_literal.clone(), value_literal, diff));

                            // Don't clone the key value if this is the last
                            // value
                            } else {
                                contents.push((key_literal, value_literal, diff));
                                break;
                            }
                        }

                        cursor.step_key();
                    }

                    StreamCollection::Map(contents)
                }
            }

        // The output is unreachable so we always return an empty stream
        } else {
            tracing::info!(
                "consolidating output from an unreachable sink, returning an empty stream",
            );
            StreamCollection::empty(*layout)
        }
    }
}

unsafe fn row_literal_from_row(row: &Row, native: &NativeLayout, layout: &RowLayout) -> RowLiteral {
    let mut literal = Vec::with_capacity(layout.len());
    for column in 0..layout.len() {
        let value = if layout.column_nullable(column) {
            NullableConstant::Nullable(
                row.column_is_null(column, native)
                    .not()
                    .then(|| unsafe { constant_from_column(column, row, native, layout) }),
            )
        } else {
            NullableConstant::NonNull(unsafe { constant_from_column(column, row, native, layout) })
        };

        literal.push(value);
    }

    RowLiteral::new(literal)
}

unsafe fn constant_from_column(
    column: usize,
    row: &Row,
    native: &NativeLayout,
    layout: &RowLayout,
) -> Constant {
    let ptr = unsafe { row.as_ptr().add(native.offset_of(column) as usize) };

    match layout.column_type(column) {
        ColumnType::Unit => Constant::Unit,
        ColumnType::U8 => Constant::U8(ptr.cast::<u8>().read()),
        ColumnType::I8 => Constant::I8(ptr.cast::<i8>().read()),
        ColumnType::U16 => Constant::U16(ptr.cast::<u16>().read()),
        ColumnType::I16 => Constant::I16(ptr.cast::<i16>().read()),
        ColumnType::U32 => Constant::U32(ptr.cast::<u32>().read()),
        ColumnType::I32 => Constant::I32(ptr.cast::<i32>().read()),
        ColumnType::U64 => Constant::U64(ptr.cast::<u64>().read()),
        ColumnType::I64 => Constant::I64(ptr.cast::<i64>().read()),
        ColumnType::Usize => Constant::Usize(ptr.cast::<usize>().read()),
        ColumnType::Isize => Constant::Isize(ptr.cast::<isize>().read()),
        ColumnType::F32 => Constant::F32(ptr.cast::<f32>().read()),
        ColumnType::F64 => Constant::F64(ptr.cast::<f64>().read()),
        ColumnType::Bool => Constant::Bool(ptr.cast::<bool>().read()),

        ColumnType::Date => Constant::Date(
            Utc.timestamp_opt(ptr.cast::<i32>().read() as i64 * 86400, 0)
                .unwrap()
                .date_naive(),
        ),
        ColumnType::Timestamp => Constant::Timestamp(
            Utc.timestamp_millis_opt(ptr.cast::<i64>().read())
                .unwrap()
                .naive_utc(),
        ),

        ColumnType::String => Constant::String(ptr.cast::<ThinStrRef>().read().to_string()),

        ColumnType::Decimal => Constant::Decimal(Decimal::deserialize(
            ptr.cast::<u128>().read().to_le_bytes(),
        )),

        ColumnType::Ptr => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        codegen::CodegenConfig,
        facade::Demands,
        ir::{
            literal::{NullableConstant, RowLiteral, StreamCollection},
            nodes::{IndexByColumn, StreamKind, StreamLayout},
            ColumnType, Constant, Graph, GraphExt, NodeId, RowLayoutBuilder,
        },
        sql_graph::SqlGraph,
        utils, DbspCircuit,
    };
    use std::path::Path;

    #[test]
    fn time_series_enrich_e2e() {
        utils::test_logger();

        // Deserialize the graph from json
        let graph = serde_json::from_str::<SqlGraph>(TIME_SERIES_ENRICH_SRC)
            .unwrap()
            .rematerialize();

        let transactions_layout = graph.nodes()[&TRANSACTIONS_ID]
            .clone()
            .unwrap_source()
            .layout();
        let demographics_layout = graph.nodes()[&DEMOGRAPHICS_ID]
            .clone()
            .unwrap_source()
            .layout();

        let mut demands = Demands::new();
        demands.add_csv_deserialize(transactions_layout, transaction_mappings());
        demands.add_csv_deserialize(demographics_layout, demographic_mappings());

        // Create the circuit
        let mut circuit = DbspCircuit::new(graph, true, 1, CodegenConfig::debug(), demands);

        // Ingest data
        circuit.append_csv_input(
            TRANSACTIONS_ID,
            &Path::new(PATH).join("transactions_20K.csv"),
        );
        circuit.append_csv_input(DEMOGRAPHICS_ID, &Path::new(PATH).join("demographics.csv"));

        // Step the circuit
        circuit.step().unwrap();

        // TODO: Inspect outputs
        let _output = circuit.consolidate_output(SINK_ID);

        // Shut down the circuit
        circuit.kill().unwrap();
    }

    #[test]
    fn time_series_enrich_e2e_2() {
        utils::test_logger();

        // Deserialize the graph from json
        let mut graph = Graph::new();

        let unit_layout = graph.layout_cache().unit();
        let demographics_layout = graph.layout_cache().add(
            RowLayoutBuilder::new()
                .with_column(ColumnType::F64, false)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::I32, true)
                .with_column(ColumnType::F64, true)
                .with_column(ColumnType::F64, true)
                .with_column(ColumnType::I32, true)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::Date, true)
                .build(),
        );
        let transactions_layout = graph.layout_cache().add(
            RowLayoutBuilder::new()
                .with_column(ColumnType::Timestamp, false)
                .with_column(ColumnType::F64, false)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::F64, true)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::I32, true)
                .with_column(ColumnType::F64, true)
                .with_column(ColumnType::F64, true)
                .with_column(ColumnType::I32, true)
                .build(),
        );
        let key_layout = graph.layout_cache().add(
            RowLayoutBuilder::new()
                .with_column(ColumnType::F64, false)
                .build(),
        );
        let culled_demographics_layout = graph.layout_cache().add(
            RowLayoutBuilder::new()
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::String, true)
                .build(),
        );
        let culled_transactions_layout = graph.layout_cache().add(
            RowLayoutBuilder::new()
                .with_column(ColumnType::Timestamp, false)
                .build(),
        );
        let output_layout = graph.layout_cache().add(
            RowLayoutBuilder::new()
                .with_column(ColumnType::Timestamp, false)
                .with_column(ColumnType::F64, false)
                .with_column(ColumnType::String, true)
                .with_column(ColumnType::String, true)
                .build(),
        );

        let demographics_src = graph.source(demographics_layout);
        let transactions_src = graph.source(transactions_layout);

        let indexed_demographics = graph.add_node(IndexByColumn::new(
            demographics_src,
            demographics_layout,
            0,
            vec![2, 3, 5, 6, 7, 8, 9, 10, 11],
            key_layout,
            culled_demographics_layout,
        ));
        let indexed_transactions = graph.add_node(IndexByColumn::new(
            transactions_src,
            transactions_layout,
            1,
            vec![2, 3, 4, 5, 6, 7, 8, 9],
            key_layout,
            culled_transactions_layout,
        ));

        let transactions_join_demographics = graph.join_core(
            indexed_transactions,
            indexed_demographics,
            {
                let mut builder = graph.function_builder();

                let key = builder.add_input(key_layout);
                let transaction = builder.add_input(culled_transactions_layout);
                let demographic = builder.add_input(culled_demographics_layout);
                let output = builder.add_output(output_layout);
                let _unit_output = builder.add_output(unit_layout);

                let trans_date_trans_time = builder.load(transaction, 0);
                let cc_num = builder.load(key, 0);
                builder.store(output, 0, trans_date_trans_time);
                builder.store(output, 1, cc_num);

                {
                    let first_not_null = builder.create_block();
                    let after = builder.create_block();

                    let first_null = builder.is_null(demographic, 0);
                    builder.set_null(output, 2, first_null);
                    builder.branch(first_null, after, [], first_not_null, []);

                    builder.move_to(first_not_null);
                    let first = builder.load(demographic, 0);
                    let first = builder.copy(first);
                    builder.store(output, 2, first);
                    builder.jump(after, []);

                    builder.move_to(after);
                }

                {
                    let city_not_null = builder.create_block();
                    let after = builder.create_block();

                    let city_null = builder.is_null(demographic, 1);
                    builder.set_null(output, 3, city_null);
                    builder.branch(city_null, after, [], city_not_null, []);

                    builder.move_to(city_not_null);
                    let city = builder.load(demographic, 1);
                    let city = builder.copy(city);
                    builder.store(output, 3, city);
                    builder.jump(after, []);

                    builder.move_to(after);
                }

                builder.ret_unit();
                builder.build()
            },
            output_layout,
            unit_layout,
            StreamKind::Set,
        );

        let sink = graph.sink(
            transactions_join_demographics,
            StreamLayout::Set(output_layout),
        );

        let mut demands = Demands::new();
        demands.add_csv_deserialize(transactions_layout, transaction_mappings());
        demands.add_csv_deserialize(demographics_layout, demographic_mappings());

        // Create the circuit
        let mut circuit = DbspCircuit::new(graph, true, 1, CodegenConfig::debug(), demands);

        // Ingest data
        circuit.append_csv_input(
            transactions_src,
            &Path::new(PATH).join("transactions_20K.csv"),
        );
        circuit.append_csv_input(demographics_src, &Path::new(PATH).join("demographics.csv"));

        // Step the circuit
        circuit.step().unwrap();

        // TODO: Inspect outputs
        let _output = circuit.consolidate_output(sink);

        // Shut down the circuit
        circuit.kill().unwrap();
    }

    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../demo/project_demo01-TimeSeriesEnrich",
    );

    fn transaction_mappings() -> Vec<(usize, usize, Option<String>)> {
        vec![
            (0, 0, Some("%F %T".into())),
            (1, 1, None),
            (2, 2, None),
            (3, 3, None),
            (4, 4, None),
            (5, 5, None),
            (6, 6, None),
            (7, 7, None),
            (8, 8, None),
            (9, 9, None),
        ]
    }

    fn demographic_mappings() -> Vec<(usize, usize, Option<String>)> {
        vec![
            (0, 0, None),
            (1, 1, None),
            (2, 2, None),
            (3, 3, None),
            (4, 4, None),
            (5, 5, None),
            (6, 6, None),
            (7, 7, None),
            (8, 8, None),
            (9, 9, None),
            (10, 10, None),
            (11, 11, Some("%F".into())),
        ]
    }

    const TRANSACTIONS_ID: NodeId = NodeId::new(54);
    const DEMOGRAPHICS_ID: NodeId = NodeId::new(68);
    const SINK_ID: NodeId = NodeId::new(273);

    static TIME_SERIES_ENRICH_SRC: &str = include_str!("time_series_enrich.json");
    static CONSTANT_STREAM_TEST: &str = include_str!("constant_stream.json");
    static UNUSED_SOURCE: &str = include_str!("unused_source.json");

    #[test]
    fn constant_stream() {
        utils::test_logger();

        // Deserialize the graph from json
        let graph = serde_json::from_str::<SqlGraph>(CONSTANT_STREAM_TEST)
            .unwrap()
            .rematerialize();

        // Create the circuit
        let mut circuit = DbspCircuit::new(graph, true, 1, CodegenConfig::debug(), Demands::new());

        // Step the circuit
        circuit.step().unwrap();

        // Inspect outputs
        let output = circuit.consolidate_output(NodeId::new(2));

        // Shut down the circuit
        circuit.kill().unwrap();

        // Ensure the output is correct
        let expected = StreamCollection::Set(vec![(
            RowLiteral::new(vec![NullableConstant::NonNull(Constant::U32(1))]),
            1,
        )]);
        assert_eq!(output, expected);
    }

    #[test]
    fn append_unused_source() {
        utils::test_logger();

        // Deserialize the graph from json
        let graph = serde_json::from_str::<SqlGraph>(UNUSED_SOURCE)
            .unwrap()
            .rematerialize();

        // Create the circuit
        let mut circuit = DbspCircuit::new(graph, true, 1, CodegenConfig::debug(), Demands::new());

        // Feed data to our unused input
        circuit.append_input(
            NodeId::new(1),
            &StreamCollection::Set(vec![(
                RowLiteral::new(vec![
                    NullableConstant::NonNull(Constant::I32(1)),
                    NullableConstant::NonNull(Constant::F64(1.0)),
                    NullableConstant::NonNull(Constant::Bool(true)),
                    NullableConstant::NonNull(Constant::String("foobar".into())),
                    NullableConstant::Nullable(Some(Constant::I32(1))),
                    NullableConstant::Nullable(Some(Constant::F64(1.0))),
                ]),
                1,
            )]),
        );

        // Step the circuit
        circuit.step().unwrap();

        let output = circuit.consolidate_output(NodeId::new(3));

        // Kill the circuit
        circuit.kill().unwrap();

        // Ensure the output is correct
        let expected = StreamCollection::Set(vec![(
            RowLiteral::new(vec![NullableConstant::NonNull(Constant::I32(0))]),
            1,
        )]);
        assert_eq!(output, expected);
    }
}
