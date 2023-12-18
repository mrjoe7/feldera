// Table showing the list of pipelines for a user.
//
// The rows of the table can be expanded for even more details.
'use client'

import useStatusNotification from '$lib/components/common/errors/useStatusNotification'
import { DataGridFooter } from '$lib/components/common/table/DataGridFooter'
import { DataGridPro } from '$lib/components/common/table/DataGridProDeclarative'
import DataGridSearch from '$lib/components/common/table/DataGridSearch'
import DataGridToolbar from '$lib/components/common/table/DataGridToolbar'
import { ResetColumnViewButton } from '$lib/components/common/table/ResetColumnViewButton'
import { TextIcon } from '$lib/components/common/TextIcon'
import AnalyticsPipelineTput from '$lib/components/streaming/management/AnalyticsPipelineTput'
import PipelineMemoryGraph from '$lib/components/streaming/management/PipelineMemoryGraph'
import { PipelineRevisionStatusChip } from '$lib/components/streaming/management/RevisionStatus'
import { useDataGridPresentationLocalStorage } from '$lib/compositions/persistence/dataGrid'
import { usePipelineMetrics } from '$lib/compositions/streaming/management/usePipelineMetrics'
import { usePipelineMutation } from '$lib/compositions/streaming/management/usePipelineMutation'
import { useDeleteDialog } from '$lib/compositions/useDialog'
import { useHashPart } from '$lib/compositions/useHashPart'
import { humanSize } from '$lib/functions/common/string'
import { invalidateQuery } from '$lib/functions/common/tanstack'
import { tuple } from '$lib/functions/common/tuple'
import {
  ApiError,
  AttachedConnector,
  ConnectorDescr,
  PipelineId,
  PipelineRevision,
  PipelinesService,
  Relation,
  UpdatePipelineRequest,
  UpdatePipelineResponse
} from '$lib/services/manager'
import {
  mutationDeletePipeline,
  mutationPausePipeline,
  mutationShutdownPipeline,
  mutationStartPipeline,
  PipelineManagerQuery
} from '$lib/services/pipelineManagerQuery'
import { LS_PREFIX } from '$lib/types/localStorage'
import { Pipeline, PipelineStatus } from '$lib/types/pipeline'
import { format } from 'd3-format'
import dayjs from 'dayjs'
import Link from 'next/link'
import React, { useCallback, useEffect, useState } from 'react'
import CustomChip from 'src/@core/components/mui/chip'
import invariant from 'tiny-invariant'
import { match, P } from 'ts-pattern'
import IconCalendar from '~icons/bx/calendar'
import IconLogInCircle from '~icons/bx/log-in-circle'
import IconPauseCircle from '~icons/bx/pause-circle'
import IconPencil from '~icons/bx/pencil'
import IconPlayCircle from '~icons/bx/play-circle'
import IconShow from '~icons/bx/show'
import IconStopCircle from '~icons/bx/stop-circle'
import IconTrashAlt from '~icons/bx/trash-alt'
import IconUpload from '~icons/bx/upload'
import Icon270RingWithBg from '~icons/svg-spinners/270-ring-with-bg'

import { useLocalStorage } from '@mantine/hooks'
import ExpandMoreIcon from '@mui/icons-material/ExpandMore'
import { alpha, Button, Typography, useTheme } from '@mui/material'
import Badge from '@mui/material/Badge'
import Box from '@mui/material/Box'
import Card from '@mui/material/Card'
import Grid from '@mui/material/Grid'
import IconButton from '@mui/material/IconButton'
import List from '@mui/material/List'
import ListItem from '@mui/material/ListItem'
import ListItemIcon from '@mui/material/ListItemIcon'
import ListItemText from '@mui/material/ListItemText'
import ListSubheader from '@mui/material/ListSubheader'
import Paper from '@mui/material/Paper'
import Tooltip from '@mui/material/Tooltip'
import {
  DataGridProProps,
  GRID_DETAIL_PANEL_TOGGLE_COL_DEF,
  GridColDef,
  GridRenderCellParams,
  GridRowId,
  GridToolbarFilterButton,
  GridValueSetterParams,
  useGridApiRef
} from '@mui/x-data-grid-pro'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

interface ConnectorData {
  relation: Relation
  connections: [AttachedConnector, ConnectorDescr][]
}
type InputOrOutput = 'input' | 'output'

// Joins the relation with attached connectors and connectors and returns it as
// a list of ConnectorData that has a list of `connections` for each `relation`.
function getConnectorData(revision: PipelineRevision, direction: InputOrOutput): ConnectorData[] {
  const schema = revision.program.schema
  if (!schema) {
    // This means the backend sent invalid data,
    // revisions should always have a schema
    throw Error('Pipeline revision has no schema.')
  }

  const relations = direction === ('input' as const) ? schema.inputs : schema.outputs
  const attachedConnectors = revision.pipeline.attached_connectors
  const connectors = revision.connectors

  return relations.map(relation => {
    const connections = attachedConnectors
      .filter(ac => ac.relation_name === relation.name)
      .map(ac => {
        const connector = connectors.find(c => c.name === ac?.connector_name)
        invariant(connector, 'Attached connector has no connector.') // This can't happen in a revision
        return tuple(ac, connector)
      })

    return { relation, connections }
  })
}

const DetailPanelContent = (props: { row: Pipeline }) => {
  const theme = useTheme()
  const [inputs, setInputs] = useState<ConnectorData[]>([])
  const [outputs, setOutputs] = useState<ConnectorData[]>([])
  const { descriptor, state } = props.row

  const pipelineRevisionQuery = useQuery(PipelineManagerQuery.pipelineLastRevision(descriptor.pipeline_id))
  useEffect(() => {
    if (!pipelineRevisionQuery.isPending && !pipelineRevisionQuery.isError && pipelineRevisionQuery.data) {
      setInputs(getConnectorData(pipelineRevisionQuery.data, 'input'))
      setOutputs(getConnectorData(pipelineRevisionQuery.data, 'output'))
    }
  }, [
    pipelineRevisionQuery.isPending,
    pipelineRevisionQuery.isError,
    pipelineRevisionQuery.data,
    setInputs,
    setOutputs
  ])

  const metrics = usePipelineMetrics({
    pipelineId: descriptor.pipeline_id,
    status: state.current_status,
    refetchMs: 3000
  })

  function getRelationColumns(direction: InputOrOutput): GridColDef<ConnectorData>[] {
    return [
      {
        field: 'name',
        headerName: direction === 'input' ? 'Input' : 'Output',
        flex: 0.4,
        renderCell: params => {
          if (params.row.connections.length > 0) {
            return params.row.connections.map(c => c[1].name).join(', ')
          } else {
            return <Box sx={{ fontStyle: 'italic' }}>No connection.</Box>
          }
        }
      },
      {
        field: 'config',
        valueGetter: params => params.row.relation.name,
        headerName: direction === 'input' ? 'Table' : 'View',
        flex: 0.6
      },
      {
        field: 'records',
        headerName: 'Records',
        flex: 0.3,
        renderCell: params => {
          if (params.row.connections.length > 0) {
            const records =
              (direction === 'input'
                ? metrics.input.get(params.row.relation.name)?.total_records
                : metrics.output.get(params.row.relation.name)?.transmitted_records) || 0
            return format(records > 1000 ? '.3s' : '~s')(records)
          } else {
            // TODO: we need to count records also when relation doesn't have
            // connections in the backend.
            return '-'
          }
        }
      },
      {
        field: 'traffic',
        headerName: 'Traffic',
        flex: 0.15,
        renderCell: params => {
          const bytes =
            direction === 'input'
              ? metrics.input.get(params.row.relation.name)?.total_bytes
              : metrics.output.get(params.row.relation.name)?.transmitted_bytes
          return humanSize(bytes || 0)
        }
      },
      {
        field: 'errors',
        headerName: 'Errors',
        flex: 0.15,
        renderCell: params => {
          const errors =
            direction === 'input'
              ? (m => (m ? m.num_parse_errors + m.num_transport_errors : 0))(
                  metrics.input.get(params.row.relation.name)
                )
              : (m => (m ? m.num_encode_errors + m.num_transport_errors : 0))(
                  metrics.output.get(params.row.relation.name)
                )
          return (
            <Box
              sx={{
                width: '100%',
                height: '200%',
                display: 'flex',
                px: 2,
                alignItems: 'center',
                backgroundColor: errors > 0 ? alpha(theme.palette.warning.main, 0.5) : undefined
              }}
            >
              {format(',')(errors)}
            </Box>
          )
        }
      },
      {
        field: 'action',
        headerName: 'Action',
        flex: 0.15,
        renderCell: params => (
          <>
            <Tooltip title={direction === 'input' ? 'Inspect Table' : 'Inspect View'}>
              <IconButton
                size='small'
                href={`/streaming/inspection/?pipeline_id=${descriptor.pipeline_id}&relation=${params.row.relation.name}`}
              >
                <IconShow fontSize={20} />
              </IconButton>
            </Tooltip>
            {direction === 'input' && state.current_status == PipelineStatus.RUNNING && (
              <Tooltip title='Import Data'>
                <IconButton
                  size='small'
                  href={`/streaming/inspection/?pipeline_id=${descriptor.pipeline_id}&relation=${params.row.relation.name}#insert`}
                >
                  <IconUpload fontSize={20} />
                </IconButton>
              </Tooltip>
            )}
          </>
        )
      }
    ]
  }

  const gridPersistence = useDataGridPresentationLocalStorage({
    key: LS_PREFIX + 'settings/streaming/management/details/grid'
  })

  return !pipelineRevisionQuery.isPending && !pipelineRevisionQuery.isError ? (
    <Box display='flex' sx={{ m: 2 }} justifyContent='center'>
      <Grid container spacing={3} sx={{ height: 1, width: '95%' }} alignItems='stretch'>
        <Grid item xs={4}>
          <Card>
            <List subheader={<ListSubheader>Configuration</ListSubheader>} dense>
              <ListItem>
                <ListItemIcon>
                  <TextIcon size={24} fontSize={10} text='SQL' />
                </ListItemIcon>
                <ListItemText
                  primary={pipelineRevisionQuery.data?.program.name || 'not set'}
                  secondary={
                    <Typography variant='caption'>
                      {pipelineRevisionQuery.data?.pipeline.pipeline_id || 'not set'}
                    </Typography>
                  }
                />
              </ListItem>
              {state.current_status == PipelineStatus.RUNNING && (
                <>
                  <ListItem>
                    <Tooltip title='Pipeline Running Since'>
                      <ListItemIcon>
                        <IconCalendar fontSize={20} />
                      </ListItemIcon>
                    </Tooltip>
                    <ListItemText
                      primary={state.created ? dayjs(state.created).format('MM/DD/YYYY HH:MM') : 'Not running'}
                    />
                  </ListItem>
                  <ListItem>
                    <Tooltip title='Pipeline Port'>
                      <ListItemIcon>
                        <IconLogInCircle fontSize={20} />
                      </ListItemIcon>
                    </Tooltip>
                    <ListItemText className='pipelinePort' primary={state.location || '0000'} />
                  </ListItem>
                </>
              )}
            </List>
          </Card>
        </Grid>
        <Grid item xs={5}>
          <AnalyticsPipelineTput metrics={metrics.global} />
        </Grid>
        <Grid item xs={3}>
          <PipelineMemoryGraph metrics={metrics.global} />
        </Grid>
        <Grid item xs={12}>
          {/* className referenced by webui-tester */}
          <Paper className='inputStats'>
            <DataGridPro
              autoHeight
              getRowId={(row: ConnectorData) => row.relation.name}
              columns={getRelationColumns('input')}
              rows={inputs}
              sx={{ flex: 1 }}
              hideFooter
              {...gridPersistence}
            />
          </Paper>
        </Grid>

        <Grid item xs={12}>
          {/* className referenced by webui-tester */}
          <Paper className='outputStats'>
            <DataGridPro
              autoHeight
              getRowId={(row: ConnectorData) => row.relation.name}
              columns={getRelationColumns('output')}
              rows={outputs}
              sx={{ flex: 1 }}
              hideFooter
              {...gridPersistence}
            />
          </Paper>
        </Grid>
      </Grid>
    </Box>
  ) : (
    <Box>Loading...</Box>
  )
}

// Only show the details tab button if this pipeline has a revision
function CustomDetailPanelToggle(props: Pick<GridRenderCellParams, 'id' | 'value' | 'row'>) {
  const { value: isExpanded, row: row } = props
  const [hasRevision, setHasRevision] = useState<boolean>(false)

  const pipelineRevisionQuery = useQuery(PipelineManagerQuery.pipelineLastRevision(props.row.descriptor.pipeline_id))
  useEffect(() => {
    if (!pipelineRevisionQuery.isPending && !pipelineRevisionQuery.isError && pipelineRevisionQuery.data != null) {
      setHasRevision(true)
    }
  }, [pipelineRevisionQuery.isPending, pipelineRevisionQuery.isError, pipelineRevisionQuery.data])

  return (isExpanded ||
    row.state.current_status === PipelineStatus.RUNNING ||
    row.state.current_status === PipelineStatus.PAUSED) &&
    hasRevision ? (
    <IconButton size='small' tabIndex={-1} aria-label={isExpanded ? 'Close' : 'Open'}>
      <ExpandMoreIcon
        sx={{
          transform: `rotateZ(${isExpanded ? 180 : 0}deg)`,
          transition: theme =>
            theme.transitions.create('transform', {
              duration: theme.transitions.duration.shortest
            })
        }}
        fontSize='inherit'
      />
    </IconButton>
  ) : (
    <></>
  )
}

export default function PipelineTable() {
  const [rows, setRows] = useState<Pipeline[]>([])
  const [filteredData, setFilteredData] = useState<Pipeline[]>([])
  const [paginationModel, setPaginationModel] = useState({
    pageSize: 7,
    page: 0
  })

  const pipelineQuery = useQuery({
    ...PipelineManagerQuery.pipelines(),
    refetchInterval: 2000
  })
  const { isPending, isError, data, error } = pipelineQuery
  useEffect(() => {
    if (!isPending && !isError) {
      setRows(data)
    }
    if (isError) {
      throw error
    }
  }, [isPending, isError, data, setRows, error])
  const getDetailPanelContent = useCallback<NonNullable<DataGridProProps['getDetailPanelContent']>>(
    ({ row }) => <DetailPanelContent row={row} />,
    []
  )
  const columns: GridColDef[] = [
    {
      ...GRID_DETAIL_PANEL_TOGGLE_COL_DEF,
      renderCell: params => <CustomDetailPanelToggle id={params.id} value={params.value} row={params.row} />
    },
    {
      field: 'name',
      headerName: 'Name',
      editable: true,
      flex: 2,
      valueGetter: params => params.row.descriptor.name,
      valueSetter: (params: GridValueSetterParams) => {
        params.row.descriptor.name = params.value
        return params.row
      }
    },
    {
      field: 'description',
      headerName: 'Description',
      editable: true,
      flex: 3,
      valueGetter: params => params.row.descriptor.description,
      valueSetter: (params: GridValueSetterParams) => {
        params.row.descriptor.description = params.value
        return params.row
      }
    },
    {
      field: 'modification',
      headerName: 'Changes',
      flex: 1,
      renderCell: (params: GridRenderCellParams) => {
        return <PipelineRevisionStatusChip pipeline={params.row} />
      }
    },
    {
      field: 'status',
      headerName: 'Status',
      flex: 1,
      renderCell: PipelineStatusCell
    },
    {
      field: 'actions',
      headerName: 'Actions',
      flex: 1.0,
      renderCell: PipelineActions
    }
  ]

  // Makes sure we can edit name and description in the table
  const apiRef = useGridApiRef()
  const queryClient = useQueryClient()
  const { pushMessage } = useStatusNotification()
  const mutation = useMutation<
    UpdatePipelineResponse,
    ApiError,
    { pipeline_id: PipelineId; request: UpdatePipelineRequest }
  >({
    mutationFn: args => PipelinesService.updatePipeline(args.pipeline_id, args.request)
  })
  const onUpdateRow = (newRow: Pipeline, oldRow: Pipeline) => {
    mutation.mutate(
      {
        pipeline_id: newRow.descriptor.pipeline_id,
        request: {
          name: newRow.descriptor.name,
          description: newRow.descriptor.description,
          program_name: newRow.descriptor.program_name
        }
      },
      {
        onError: (error: ApiError) => {
          invalidateQuery(queryClient, PipelineManagerQuery.pipelines())
          invalidateQuery(queryClient, PipelineManagerQuery.pipelineStatus(newRow.descriptor.pipeline_id))
          pushMessage({ message: error.body.message, key: new Date().getTime(), color: 'error' })
          apiRef.current.updateRows([oldRow])
        }
      }
    )

    return newRow
  }

  const btnAdd = (
    <Button variant='contained' size='small' href='/streaming/builder/' id='btn-add-pipeline' key='0'>
      Add pipeline
    </Button>
  )

  const [expandedRows, setExpandedRows] = useLocalStorage({
    key: LS_PREFIX + 'pipelines/expanded',
    defaultValue: [] as GridRowId[]
  })
  const [hash, setHash] = useHashPart()

  // Cannot initialize in useState because hash is not available during SSR
  useEffect(() => {
    setExpandedRows(expandedRows =>
      (expandedRows.includes(hash) ? expandedRows : [...expandedRows, hash]).filter(
        row =>
          data?.find(
            p =>
              p.descriptor.pipeline_id === row &&
              [
                PipelineStatus.PROVISIONING,
                PipelineStatus.INITIALIZING,
                PipelineStatus.PAUSED,
                PipelineStatus.RUNNING,
                PipelineStatus.SHUTTING_DOWN
              ].includes(p.state.current_status)
          )
      )
    )
  }, [hash, setExpandedRows, data])

  const updateExpandedRows = (newExpandedRows: GridRowId[]) => {
    if (newExpandedRows.length < expandedRows.length && !newExpandedRows.includes(hash)) {
      setHash('')
    }
    setExpandedRows(newExpandedRows)
  }

  const gridPersistence = useDataGridPresentationLocalStorage({
    key: LS_PREFIX + 'settings/streaming/management/grid'
  })

  return (
    <Card>
      <DataGridPro
        autoHeight
        apiRef={apiRef}
        getRowId={(row: Pipeline) => row.descriptor.pipeline_id}
        columns={columns}
        rowThreshold={0}
        getDetailPanelHeight={() => 'auto'}
        getDetailPanelContent={getDetailPanelContent}
        slots={{
          toolbar: DataGridToolbar,
          footer: DataGridFooter
        }}
        rows={filteredData.length ? filteredData : rows}
        pageSizeOptions={[7, 10, 25, 50]}
        paginationModel={paginationModel}
        onPaginationModelChange={setPaginationModel}
        processRowUpdate={onUpdateRow}
        loading={isPending}
        slotProps={{
          baseButton: {
            variant: 'outlined'
          },
          toolbar: {
            children: [
              btnAdd,
              <GridToolbarFilterButton key='1' />,
              <ResetColumnViewButton key='2' {...gridPersistence} />,
              <div style={{ marginLeft: 'auto' }} key='3' />,
              <DataGridSearch fetchRows={pipelineQuery} setFilteredData={setFilteredData} key='99' />
            ]
          },
          footer: {
            children: btnAdd
          }
        }}
        detailPanelExpandedRowIds={expandedRows}
        onDetailPanelExpandedRowIdsChange={updateExpandedRows}
        {...gridPersistence}
      />
    </Card>
  )
}

const usePipelineStatus = (params: { row: Pipeline }) => {
  const pipeline = params.row.descriptor
  const curProgramQuery = useQuery({
    ...PipelineManagerQuery.programs(),
    enabled: pipeline.program_name !== null,
    refetchInterval: 2000
  })
  const { data: pipelines } = useQuery({
    ...PipelineManagerQuery.pipelines()
  })

  const programStatus =
    pipeline.program_name === null
      ? ('NoProgram' as const)
      : curProgramQuery.isPending || curProgramQuery.isError
        ? ('NotReady' as const)
        : (programData => {
            invariant(programData, 'Program data should be available')
            return match(programData.status)
              .with('Success', () => 'Ready' as const)
              .with('CompilingRust', () => 'CompilingRust' as const)
              .with('None', 'CompilingSql', () => 'NotReady' as const)
              .with('Pending', () => 'Pending' as const)
              .otherwise(() => 'Error' as const)
          })(curProgramQuery.data.find(p => p.name === pipeline.program_name))

  const currentStatus =
    pipelines?.find(p => p.descriptor.pipeline_id === pipeline.pipeline_id)?.state.current_status ??
    PipelineStatus.UNKNOWN
  return tuple(currentStatus, programStatus)
}

const PipelineStatusCell = (params: { row: Pipeline } & GridRenderCellParams) => {
  const [status, programStatus] = usePipelineStatus(params)

  const shutdownPipelineClick = usePipelineMutation(mutationShutdownPipeline)

  const chip = match([status, programStatus])
    .with([PipelineStatus.UNKNOWN, P._], () => <CustomChip rounded size='small' skin='light' label={status} />)
    .with([PipelineStatus.SHUTDOWN, 'NotReady'], () => (
      <CustomChip rounded size='small' skin='light' color='primary' label='Compiling' />
    ))
    .with([PipelineStatus.SHUTDOWN, 'Pending'], () => (
      <CustomChip rounded size='small' skin='light' color='info' label='Queued' />
    ))
    .with([PipelineStatus.SHUTDOWN, 'CompilingRust'], () => (
      <CustomChip rounded size='small' skin='light' color='primary' label='Compiling binary' />
    ))
    .with([PipelineStatus.SHUTDOWN, 'Error'], () => (
      <CustomChip rounded size='small' skin='light' color='error' label='Program error' />
    ))
    .with([PipelineStatus.SHUTDOWN, P._], () => <CustomChip rounded size='small' skin='light' label={status} />)
    .with([PipelineStatus.INITIALIZING, P._], () => (
      <CustomChip rounded size='small' skin='light' color='secondary' label={status} />
    ))
    .with([PipelineStatus.PROVISIONING, P._], () => (
      <CustomChip rounded size='small' skin='light' color='secondary' label={status} />
    ))
    .with([PipelineStatus.CREATE_FAILURE, P._], () => (
      <CustomChip rounded size='small' skin='light' color='error' label={status} />
    ))
    .with([PipelineStatus.STARTING, P._], () => (
      <CustomChip rounded size='small' skin='light' color='secondary' label={status} />
    ))
    .with([PipelineStatus.STARTUP_FAILURE, P._], () => (
      <CustomChip rounded size='small' skin='light' color='error' label={status} />
    ))
    .with([PipelineStatus.RUNNING, P._], () => (
      <CustomChip rounded size='small' skin='light' color='success' label={status} />
    ))
    .with([PipelineStatus.PAUSING, P._], () => (
      <CustomChip rounded size='small' skin='light' color='info' label={status} />
    ))
    .with([PipelineStatus.PAUSED, 'NotReady'], () => (
      <CustomChip rounded size='small' skin='light' color='primary' label='Compiling' />
    ))
    .with([PipelineStatus.PAUSED, 'Pending'], () => (
      <CustomChip rounded size='small' skin='light' color='info' label='Queued' />
    ))
    .with([PipelineStatus.PAUSED, 'CompilingRust'], () => (
      <CustomChip rounded size='small' skin='light' color='primary' label='Compiling binary' />
    ))
    .with([PipelineStatus.PAUSED, 'Error'], () => (
      <CustomChip rounded size='small' skin='light' color='error' label='Program error' />
    ))
    .with([PipelineStatus.PAUSED, P._], () => (
      <CustomChip rounded size='small' skin='light' color='info' label={status} />
    ))
    .with([PipelineStatus.FAILED, P._], () => (
      <Tooltip title={params.row.state.error?.message || 'Unknown Error'} disableInteractive>
        <CustomChip
          rounded
          size='small'
          skin='light'
          color='error'
          label={status}
          onDelete={() => shutdownPipelineClick(params.row.descriptor.pipeline_id)}
        />
      </Tooltip>
    ))
    .with([PipelineStatus.SHUTTING_DOWN, P._], () => (
      <CustomChip rounded size='small' skin='light' color='secondary' label={status} />
    ))
    .exhaustive()

  return (
    <Badge badgeContent={params.row.warn_cnt} color='warning'>
      <Badge badgeContent={params.row.error_cnt} color='error'>
        {chip}
      </Badge>
    </Badge>
  )
}

const PipelineActions = (params: { row: Pipeline }) => {
  const pipeline = params.row.descriptor

  const [status, programStatus] = usePipelineStatus(params)

  const startPipelineClick = usePipelineMutation(mutationStartPipeline)
  const pausePipelineClick = usePipelineMutation(mutationPausePipeline)
  const shutdownPipelineClick = usePipelineMutation(mutationShutdownPipeline)
  const deletePipelineClick = usePipelineMutation(mutationDeletePipeline)

  const { showDeleteDialog } = useDeleteDialog()

  const actions = {
    pause: () => (
      <Tooltip title='Pause Pipeline' key='pause'>
        <IconButton className='pauseButton' size='small' onClick={() => pausePipelineClick(pipeline.pipeline_id)}>
          <IconPauseCircle fontSize={20} />
        </IconButton>
      </Tooltip>
    ),
    start: () => (
      <Tooltip title='Start Pipeline' key='start'>
        <IconButton className='startButton' size='small' onClick={() => startPipelineClick(pipeline.pipeline_id)}>
          <IconPlayCircle fontSize={20} />
        </IconButton>
      </Tooltip>
    ),
    spinner: () => (
      <Tooltip title={status} key='spinner'>
        <IconButton size='small'>
          <Icon270RingWithBg fontSize={20} />
        </IconButton>
      </Tooltip>
    ),
    shutdown: () => (
      <Tooltip title='Shutdown Pipeline' key='shutdown'>
        <IconButton className='shutdownButton' size='small' onClick={() => shutdownPipelineClick(pipeline.pipeline_id)}>
          <IconStopCircle fontSize={20} />
        </IconButton>
      </Tooltip>
    ),
    inspect: () => (
      <Tooltip title='Inspect' key='inspect'>
        <IconButton size='small' component={Link} href='#'>
          <IconShow fontSize={20} />
        </IconButton>
      </Tooltip>
    ),
    edit: () => (
      <Tooltip title='Edit Pipeline' key='edit'>
        <IconButton
          className='editButton'
          size='small'
          href={`/streaming/builder/?pipeline_id=${pipeline.pipeline_id}`}
        >
          <IconPencil fontSize={20} />
        </IconButton>
      </Tooltip>
    ),
    delete: () => (
      <Tooltip title='Delete Pipeline' key='delete'>
        <IconButton
          className='deleteButton'
          size='small'
          onClick={showDeleteDialog(
            'Delete',
            `${pipeline.name.replace(/^[Pp]ipeline\s+|\s+[Pp]ipeline$/, '') || 'unnamed'} pipeline`,
            () => deletePipelineClick(pipeline.pipeline_id)
          )}
        >
          <IconTrashAlt fontSize={20} />
        </IconButton>
      </Tooltip>
    ),
    spacer: () => (
      <IconButton size='small' sx={{ opacity: 0 }} disabled key='spacer'>
        <IconStopCircle fontSize={20} />
      </IconButton>
    )
  }

  const enabled = match([status, programStatus])
    .returnType<(keyof typeof actions)[]>()
    .with([PipelineStatus.SHUTDOWN, 'Ready'], () => ['start', 'edit', 'delete'])
    .with([PipelineStatus.SHUTDOWN, P._], () => ['spacer', 'edit', 'delete'])
    .with([PipelineStatus.PROVISIONING, P._], () => ['spinner', 'edit'])
    .with([PipelineStatus.INITIALIZING, P._], () => ['spinner', 'edit'])
    .with([PipelineStatus.STARTING, P._], () => ['spinner', 'edit'])
    .with([PipelineStatus.RUNNING, P._], () => ['pause', 'edit', 'shutdown'])
    .with([PipelineStatus.PAUSING, P._], () => ['spinner', 'edit'])
    .with([PipelineStatus.PAUSED, 'Ready'], () => ['start', 'edit', 'shutdown'])
    .with([PipelineStatus.SHUTTING_DOWN, P._], () => ['spinner', 'edit'])
    .with([PipelineStatus.FAILED, P._], () => ['spacer', 'edit', 'shutdown'])
    .otherwise(() => ['spacer', 'edit'])

  return (
    <>
      {/* the className attributes are used by webui-tester */}
      {enabled.map(e => actions[e]())}
    </>
  )
}
