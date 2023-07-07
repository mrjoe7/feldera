#![allow(non_snake_case)]

pub mod casts;
pub mod geopoint;
pub mod interval;
pub mod timestamp;
pub mod string;
pub mod operators;

use crate::interval::ShortInterval;
use dbsp::algebra::{Semigroup, SemigroupValue, ZRingValue, F32, F64};
use geopoint::GeoPoint;
use num::{Signed, ToPrimitive};
use rust_decimal::{Decimal, MathematicalOps};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Add;

#[derive(Clone)]
pub struct DefaultOptSemigroup<T>(PhantomData<T>);

// Macro to create variants of a function with 1 argument
// If there exists a function is f_(x: T) -> S, this creates a function
// fN(x: Option<T>) -> Option<S>, defined as
// fN(x) { let x = x?; Some(f_(x)) }.
#[macro_export]
macro_rules! some_function1 {
    ($func_name:ident, $arg_type:ty, $ret_type:ty) => {
        ::paste::paste! {
            pub fn [<$func_name N>]( arg: Option<$arg_type> ) -> Option<$ret_type> {
                let arg = arg?;
                Some([<$func_name _>](arg))
            }
        }
    }
}

// Macro to create variants of a function with 2 arguments
// If there exists a function is f__(x: T, y: S) -> U, this creates
// three functions:
// - f_N(x: T, y: Option<S>) -> Option<U>
// - fN_(x: Option<T>, y: S) -> Option<U>
// - fNN(x: Option<T>, y: Option<S>) -> Option<U>
// The resulting functions return Some only if all arguments are 'Some'.
#[macro_export]
macro_rules! some_function2 {
    ($func_name:ident, $arg_type0:ty, $arg_type1:ty, $ret_type:ty) => {
        ::paste::paste! {
            pub fn [<$func_name NN>]( arg0: Option<$arg_type0>, arg1: Option<$arg_type1> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                Some([<$func_name __>](arg0, arg1))
            }

            pub fn [<$func_name _N>]( arg0: $arg_type0, arg1: Option<$arg_type1> ) -> Option<$ret_type> {
                let arg1 = arg1?;
                Some([<$func_name __>](arg0, arg1))
            }

            pub fn [<$func_name N_>]( arg0: Option<$arg_type0>, arg1: $arg_type1 ) -> Option<$ret_type> {
                let arg0 = arg0?;
                Some([<$func_name __>](arg0, arg1))
            }
        }
    }
}

// Macro to create variants of a function with 4 arguments
// If there exists a function is f____(x: T, y: S, z: V, w: W) -> U, this creates
// fifteen functions:
// - f___N(x: T, y: S, z: V, w: Option<W>) -> Option<U>
// - f__N_(x: T, y: S, z: Option<V>, w: W) -> Option<U>
// - etc.
// The resulting functions return Some only if all arguments are 'Some'.
#[macro_export]
macro_rules! some_function4 {
    ($func_name:ident, $arg_type0:ty, $arg_type1:ty, $arg_type2: ty, $arg_type3: ty, $ret_type:ty) => {
        ::paste::paste! {
            pub fn [<$func_name ___N>]( arg0: $arg_type0, arg1: $arg_type1, arg2: $arg_type2, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name __N_>]( arg0: $arg_type0, arg1: $arg_type1, arg2: Option<$arg_type2>, arg3: $arg_type3 ) -> Option<$ret_type> {
                let arg2 = arg2?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name __NN>]( arg0: $arg_type0, arg1: $arg_type1, arg2: Option<$arg_type2>, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg2 = arg2?;
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name _N__>]( arg0: $arg_type0, arg1: Option<$arg_type1>, arg2: $arg_type2, arg3: $arg_type3 ) -> Option<$ret_type> {
                let arg1 = arg1?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name _N_N>]( arg0: $arg_type0, arg1: Option<$arg_type1>, arg2: $arg_type2, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg1 = arg1?;
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name _NN_>]( arg0: $arg_type0, arg1: Option<$arg_type1>, arg2: Option<$arg_type2>, arg3: $arg_type3 ) -> Option<$ret_type> {
                let arg1 = arg1?;
                let arg2 = arg2?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name _NNN>]( arg0: $arg_type0, arg1: Option<$arg_type1>, arg2: Option<$arg_type2>, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg1 = arg1?;
                let arg2 = arg2?;
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name N___>]( arg0: Option<$arg_type0>, arg1: $arg_type1, arg2: $arg_type2, arg3: $arg_type3 ) -> Option<$ret_type> {
                let arg0 = arg0?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name N__N>]( arg0: Option<$arg_type0>, arg1: $arg_type1, arg2: Option<$arg_type2>, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg2 = arg2?;
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name N_N_>]( arg0: Option<$arg_type0>, arg1: Option<$arg_type1>, arg2: $arg_type2, arg3: $arg_type3 ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name N_NN>]( arg0: Option<$arg_type0>, arg1: Option<$arg_type1>, arg2: Option<$arg_type2>, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                let arg2 = arg2?;
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name NN__>]( arg0: Option<$arg_type0>, arg1: $arg_type1, arg2: $arg_type2, arg3: $arg_type3 ) -> Option<$ret_type> {
                let arg0 = arg0?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name NN_N>]( arg0: Option<$arg_type0>, arg1: $arg_type1, arg2: Option<$arg_type2>, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg2 = arg2?;
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name NNN_>]( arg0: Option<$arg_type0>, arg1: Option<$arg_type1>, arg2: Option<$arg_type2>, arg3: $arg_type3 ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                let arg2 = arg2?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }

            pub fn [<$func_name NNNN>]( arg0: Option<$arg_type0>, arg1: Option<$arg_type1>, arg2: Option<$arg_type2>, arg3: Option<$arg_type3> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                let arg2 = arg2?;
                let arg3 = arg3?;
                Some([<$func_name ____>](arg0, arg1, arg2, arg3))
            }
        }
    }
}

// Macro to create variants of a function with 3 arguments
// If there exists a function is f___(x: T, y: S, z: V) -> U, this creates
// seven functions:
// - f__N(x: T, y: S, z: Option<V>) -> Option<U>
// - f_N_(x: T, y: Option<S>, z: V) -> Option<U>
// - etc.
// The resulting functions return Some only if all arguments are 'Some'.
#[macro_export]
macro_rules! some_function3 {
    ($func_name:ident, $arg_type0:ty, $arg_type1:ty, $arg_type2: ty, $ret_type:ty) => {
        ::paste::paste! {
            pub fn [<$func_name __N>]( arg0: $arg_type0, arg1: $arg_type1, arg2: Option<$arg_type2> ) -> Option<$ret_type> {
                let arg2 = arg2?;
                Some([<$func_name ___>](arg0, arg1, arg2))
            }

            pub fn [<$func_name _N_>]( arg0: $arg_type0, arg1: Option<$arg_type1>, arg2: $arg_type2 ) -> Option<$ret_type> {
                let arg1 = arg1?;
                Some([<$func_name ___>](arg0, arg1, arg2))
            }

            pub fn [<$func_name _NN>]( arg0: $arg_type0, arg1: Option<$arg_type1>, arg2: Option<$arg_type2> ) -> Option<$ret_type> {
                let arg1 = arg1?;
                let arg2 = arg2?;
                Some([<$func_name ___>](arg0, arg1, arg2))
            }

            pub fn [<$func_name N__>]( arg0: Option<$arg_type0>, arg1: $arg_type1, arg2: $arg_type2 ) -> Option<$ret_type> {
                let arg0 = arg0?;
                Some([<$func_name ___>](arg0, arg1, arg2))
            }

            pub fn [<$func_name N_N>]( arg0: Option<$arg_type0>, arg1: $arg_type1, arg2: Option<$arg_type2> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg2 = arg2?;
                Some([<$func_name ___>](arg0, arg1, arg2))
            }

            pub fn [<$func_name NN_>]( arg0: Option<$arg_type0>, arg1: Option<$arg_type1>, arg2: $arg_type2 ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                Some([<$func_name ___>](arg0, arg1, arg2))
            }

            pub fn [<$func_name NNN>]( arg0: Option<$arg_type0>, arg1: Option<$arg_type1>, arg2: Option<$arg_type2> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                let arg2 = arg2?;
                Some([<$func_name ___>](arg0, arg1, arg2))
            }
        }
    }
}

// Macro to create variants of a function with 2 arguments
// If there exists a function is f_t_t(x: T, y: T) -> U, this creates
// three functions:
// - f_tN_t(x: T, y: Option<T>) -> Option<U>
// - f_t_tN(x: Option<T>, y: T) -> Option<U>
// - f_tN_tN(x: Option<T>, y: Option<T>) -> Option<U>
// The resulting functions return Some only if all arguments are 'Some'.
#[macro_export]
macro_rules! some_operator {
    ($func_name: ident, $short_name: ident, $arg_type: ty, $ret_type: ty) => {
        ::paste::paste! {
            #[inline(always)]
            pub fn [<$func_name _ $short_name N _ $short_name N>]( arg0: Option<$arg_type>, arg1: Option<$arg_type> ) -> Option<$ret_type> {
                let arg0 = arg0?;
                let arg1 = arg1?;
                Some([<$func_name _$short_name _ $short_name>](arg0, arg1))
            }

            #[inline(always)]
            pub fn [<$func_name _ $short_name _ $short_name N>]( arg0: $arg_type, arg1: Option<$arg_type> ) -> Option<$ret_type> {
                let arg1 = arg1?;
                Some([<$func_name _ $short_name _ $short_name>](arg0, arg1))
            }

            #[inline(always)]
            pub fn [<$func_name _ $short_name N _ $short_name>]( arg0: Option<$arg_type>, arg1: $arg_type ) -> Option<$ret_type> {
                let arg0 = arg0?;
                Some([<$func_name _ $short_name _ $short_name>](arg0, arg1))
            }
        }
    }
}

#[macro_export]
macro_rules! for_all_int_compare {
    ($func_name: ident, $ret_type: ty) => {
        some_operator!($func_name, i16, i16, bool);
        some_operator!($func_name, i32, i32, bool);
        some_operator!($func_name, i64, i64, bool);
    }
}

#[macro_export]
macro_rules! for_all_numeric_compare {
    ($func_name: ident, $ret_type: ty) => {
        for_all_int_compare!($func_name, bool);
        some_operator!($func_name, f, F32, bool);
        some_operator!($func_name, d, F64, bool);
    }
}

#[macro_export]
macro_rules! for_all_compare {
    ($func_name: ident, $ret_type: ty) => {
        for_all_numeric_compare!($func_name, bool);
        some_operator!($func_name, b, bool, bool);
        some_operator!($func_name, s, String, bool);
    }
}

#[macro_export]
macro_rules! for_all_int {
    ($func_name: ident) => {
        some_operator!($func_name, i16, i16, i16);
        some_operator!($func_name, i32, i32, i32);
        some_operator!($func_name, i64, i64, i64);
    }
}

#[macro_export]
macro_rules! for_all_numeric {
    ($func_name: ident) => {
        for_all_int!($func_name);
        some_operator!($func_name, f, F32, F32);
        some_operator!($func_name, d, F64, F64);
    }
}

impl<T> Semigroup<Option<T>> for DefaultOptSemigroup<T>
where
    T: SemigroupValue,
{
    fn combine(left: &Option<T>, right: &Option<T>) -> Option<T> {
        match (left, right) {
            (None, _) => None,
            (_, None) => None,
            (Some(x), Some(y)) => Some(x.add_by_ref(y)),
        }
    }
}

#[derive(Clone)]
pub struct PairSemigroup<T, R, TS, RS>(PhantomData<(T, R, TS, RS)>);

impl<T, R, TS, RS> Semigroup<(T, R)> for PairSemigroup<T, R, TS, RS>
where
    TS: Semigroup<T>,
    RS: Semigroup<R>,
{
    fn combine(left: &(T, R), right: &(T, R)) -> (T, R) {
        (
            TS::combine(&left.0, &right.0),
            RS::combine(&left.1, &right.1),
        )
    }
}

#[inline(always)]
pub fn wrap_bool(b: Option<bool>) -> bool {
    match b {
        Some(x) => x,
        _ => false,
    }
}

#[inline(always)]
pub fn or_b_b(left: bool, right: bool) -> bool {
    left || right
}

#[inline(always)]
pub fn or_bN_b(left: Option<bool>, right: bool) -> Option<bool> {
    match (left, right) {
        (Some(l), r) => Some(l || r),
        (_, true) => Some(true),
        (_, _) => None::<bool>,
    }
}

#[inline(always)]
pub fn or_b_bN(left: bool, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (l, Some(r)) => Some(l || r),
        (true, _) => Some(true),
        (_, _) => None::<bool>,
    }
}

#[inline(always)]
pub fn or_bN_bN(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l || r),
        (Some(true), _) => Some(true),
        (_, Some(true)) => Some(true),
        (_, _) => None::<bool>,
    }
}

#[inline(always)]
pub fn and_b_b(left: bool, right: bool) -> bool {
    left && right
}

#[inline(always)]
pub fn and_bN_b(left: Option<bool>, right: bool) -> Option<bool> {
    match (left, right) {
        (Some(l), r) => Some(l && r),
        (_, false) => Some(false),
        (_, _) => None::<bool>,
    }
}

#[inline(always)]
pub fn and_b_bN(left: bool, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (l, Some(r)) => Some(l && r),
        (false, _) => Some(false),
        (_, _) => None::<bool>,
    }
}

#[inline(always)]
pub fn and_bN_bN(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l && r),
        (Some(false), _) => Some(false),
        (_, Some(false)) => Some(false),
        (_, _) => None::<bool>,
    }
}

#[inline(always)]
pub fn is_null<T>(value: Option<T>) -> bool {
    value.is_none()
}

#[inline(always)]
pub fn indicator<T>(value: Option<T>) -> i64 {
    match value {
        None => 0,
        Some(_) => 1,
    }
}

pub fn agg_max_N_N<T>(left: Option<T>, right: Option<T>) -> Option<T>
where
    T: Ord + Copy,
{
    match (left, right) {
        (None, _) => right,
        (_, None) => left,
        (Some(x), Some(y)) => Some(x.max(y)),
    }
}

pub fn agg_min_N_N<T>(left: Option<T>, right: Option<T>) -> Option<T>
where
    T: Ord + Copy,
{
    match (left, right) {
        (None, _) => right,
        (_, None) => left,
        (Some(x), Some(y)) => Some(x.min(y)),
    }
}

pub fn agg_max_N_<T>(left: Option<T>, right: T) -> Option<T>
where
    T: Ord + Copy,
{
    match (left, right) {
        (None, _) => Some(right),
        (Some(x), y) => Some(x.max(y)),
    }
}

pub fn agg_min_N_<T>(left: Option<T>, right: T) -> Option<T>
where
    T: Ord + Copy,
{
    match (left, right) {
        (None, _) => Some(right),
        (Some(x), y) => Some(x.min(y)),
    }
}

pub fn agg_max__<T>(left: T, right: T) -> T
where
    T: Ord + Copy,
{
    left.max(right)
}

pub fn agg_min__<T>(left: T, right: T) -> T
where
    T: Ord + Copy,
{
    left.min(right)
}

pub fn agg_plus_N_N<T>(left: Option<T>, right: Option<T>) -> Option<T>
where
    T: Add<T, Output = T> + Copy,
{
    match (left, right) {
        (None, _) => right,
        (_, None) => left,
        (Some(x), Some(y)) => Some(x + y),
    }
}

pub fn agg_plus_N_<T>(left: Option<T>, right: T) -> Option<T>
where
    T: Add<T, Output = T> + Copy,
{
    match (left, right) {
        (None, _) => Some(right),
        (Some(x), y) => Some(x + y),
    }
}

pub fn agg_plus__N<T>(left: T, right: Option<T>) -> Option<T>
where
    T: Add<T, Output = T> + Copy,
{
    match (left, right) {
        (_, None) => Some(left),
        (x, Some(y)) => Some(x + y),
    }
}

pub fn agg_plus__<T>(left: T, right: T) -> T
where
    T: Add<T, Output = T> + Copy,
{
    left + right
}

#[inline(always)]
pub fn div_i16_i16(left: i16, right: i16) -> Option<i16> {
    match right {
        0 => None,
        _ => Some(left / right),
    }
}

#[inline(always)]
pub fn div_i32_i32(left: i32, right: i32) -> Option<i32> {
    match right {
        0 => None,
        _ => Some(left / right),
    }
}

#[inline(always)]
pub fn div_i64_i64(left: i64, right: i64) -> Option<i64> {
    match right {
        0 => None,
        _ => Some(left / right),
    }
}

#[inline(always)]
pub fn div_i16N_i16(left: Option<i16>, right: i16) -> Option<i16> {
    match (left, right) {
        (_, 0) => None,
        (Some(l), r) => Some(l / r),
        (_, _) => None::<i16>,
    }
}

#[inline(always)]
pub fn div_i32N_i32(left: Option<i32>, right: i32) -> Option<i32> {
    match (left, right) {
        (_, 0) => None,
        (Some(l), r) => Some(l / r),
        (_, _) => None::<i32>,
    }
}

#[inline(always)]
pub fn div_i64N_i64(left: Option<i64>, right: i64) -> Option<i64> {
    match (left, right) {
        (_, 0) => None,
        (Some(l), r) => Some(l / r),
        (_, _) => None::<i64>,
    }
}

#[inline(always)]
pub fn div_i16_i16N(left: i16, right: Option<i16>) -> Option<i16> {
    match (left, right) {
        (_, Some(0)) => None,
        (l, Some(r)) => Some(l / r),
        (_, _) => None::<i16>,
    }
}

#[inline(always)]
pub fn div_i32_i32N(left: i32, right: Option<i32>) -> Option<i32> {
    match (left, right) {
        (_, Some(0)) => None,
        (l, Some(r)) => Some(l / r),
        (_, _) => None::<i32>,
    }
}

#[inline(always)]
pub fn div_i64_i64N(left: i64, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (_, Some(0)) => None,
        (l, Some(r)) => Some(l / r),
        (_, _) => None::<i64>,
    }
}

#[inline(always)]
pub fn div_i16N_i16N(left: Option<i16>, right: Option<i16>) -> Option<i16> {
    match (left, right) {
        (_, Some(0)) => None,
        (Some(l), Some(r)) => Some(l / r),
        (_, _) => None::<i16>,
    }
}

#[inline(always)]
pub fn div_i32N_i32N(left: Option<i32>, right: Option<i32>) -> Option<i32> {
    match (left, right) {
        (_, Some(0)) => None,
        (Some(l), Some(r)) => Some(l / r),
        (_, _) => None::<i32>,
    }
}

#[inline(always)]
pub fn div_i64N_i64N(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (_, Some(0)) => None,
        (Some(l), Some(r)) => Some(l / r),
        (_, _) => None::<i64>,
    }
}

#[inline(always)]
pub fn div_f_f(left: F32, right: F32) -> Option<F32> {
    Some(F32::new(left.into_inner() / right.into_inner()))
}

#[inline(always)]
pub fn div_f_fN(left: F32, right: Option<F32>) -> Option<F32> {
    right.map(|right| F32::new(left.into_inner() / right.into_inner()))
}

#[inline(always)]
pub fn div_fN_f(left: Option<F32>, right: F32) -> Option<F32> {
    left.map(|left| F32::new(left.into_inner() / right.into_inner()))
}

#[inline(always)]
pub fn div_fN_fN(left: Option<F32>, right: Option<F32>) -> Option<F32> {
    match (left, right) {
        (None, _) => None,
        (_, None) => None,
        (Some(left), Some(right)) => Some(F32::new(left.into_inner() / right.into_inner())),
    }
}

#[inline(always)]
pub fn div_d_d(left: F64, right: F64) -> Option<F64> {
    Some(F64::new(left.into_inner() / right.into_inner()))
}

#[inline(always)]
pub fn div_d_dN(left: F64, right: Option<F64>) -> Option<F64> {
    right.map(|right| F64::new(left.into_inner() / right.into_inner()))
}

#[inline(always)]
pub fn div_dN_d(left: Option<F64>, right: F64) -> Option<F64> {
    left.map(|left| F64::new(left.into_inner() / right.into_inner()))
}

#[inline(always)]
pub fn div_dN_dN(left: Option<F64>, right: Option<F64>) -> Option<F64> {
    match (left, right) {
        (None, _) => None,
        (_, None) => None,
        (Some(left), Some(right)) => Some(F64::new(left.into_inner() / right.into_inner())),
    }
}

#[inline(always)]
pub fn abs_i16(left: i16) -> i16 {
    left.abs()
}

#[inline(always)]
pub fn abs_i32(left: i32) -> i32 {
    left.abs()
}

#[inline(always)]
pub fn abs_i64(left: i64) -> i64 {
    left.abs()
}

#[inline(always)]
pub fn abs_i16N(left: Option<i16>) -> Option<i16> {
    left.map(|l| l.abs())
}

#[inline(always)]
pub fn abs_i32N(left: Option<i32>) -> Option<i32> {
    left.map(|l| l.abs())
}

#[inline(always)]
pub fn abs_i64N(left: Option<i64>) -> Option<i64> {
    left.map(|l| l.abs())
}

#[inline(always)]
pub fn abs_f(left: F32) -> F32 {
    left.abs()
}

#[inline(always)]
pub fn abs_fN(left: Option<F32>) -> Option<F32> {
    left.map(|l| l.abs())
}

#[inline(always)]
pub fn abs_d(left: F64) -> F64 {
    left.abs()
}

#[inline(always)]
pub fn abs_dN(left: Option<F64>) -> Option<F64> {
    left.map(|l| l.abs())
}

#[inline(always)]
pub fn abs_decimal(left: Decimal) -> Decimal {
    left.abs()
}

#[inline(always)]
pub fn abs_decimalN(left: Option<Decimal>) -> Option<Decimal> {
    left.map(abs_decimal)
}

#[inline(always)]
pub fn ln_decimal(left: Decimal) -> F64 {
    F64::new(left.ln().to_f64().unwrap())
}

#[inline(always)]
pub fn ln_decimalN(left: Option<Decimal>) -> Option<F64> {
    left.map(ln_decimal)
}

#[inline(always)]
pub fn log10_decimal(left: Decimal) -> F64 {
    F64::new(left.log10().to_f64().unwrap())
}

#[inline(always)]
pub fn log10_decimalN(left: Option<Decimal>) -> Option<F64> {
    left.map(log10_decimal)
}

#[inline(always)]
pub fn is_true_b_(left: bool) -> bool {
    left
}

#[inline(always)]
pub fn is_true_bN_(left: Option<bool>) -> bool {
    left == Some(true)
}

#[inline(always)]
pub fn is_false_b_(left: bool) -> bool {
    !left
}

#[inline(always)]
pub fn is_false_bN_(left: Option<bool>) -> bool {
    left == Some(false)
}

#[inline(always)]
pub fn is_not_true_b_(left: bool) -> bool {
    !left
}

#[inline(always)]
pub fn is_not_true_bN_(left: Option<bool>) -> bool {
    match left {
        Some(true) => false,
        Some(false) => true,
        _ => true,
    }
}

#[inline(always)]
pub fn is_not_false_b_(left: bool) -> bool {
    left
}

#[inline(always)]
pub fn is_not_false_bN_(left: Option<bool>) -> bool {
    match left {
        Some(true) => true,
        Some(false) => false,
        _ => true,
    }
}

#[inline(always)]
pub fn is_distinct__<T>(left: T, right: T) -> bool
where
    T: Eq,
{
    left != right
}

#[inline(always)]
pub fn is_distinct_N_N<T>(left: Option<T>, right: Option<T>) -> bool
where
    T: Eq,
{
    match (left, right) {
        (Some(a), Some(b)) => a != b,
        (None, None) => false,
        _ => true,
    }
}

#[inline(always)]
pub fn is_distinct__N<T>(left: T, right: Option<T>) -> bool
where
    T: Eq,
{
    match right {
        Some(b) => left != b,
        None => true,
    }
}

#[inline(always)]
pub fn is_distinct_N_<T>(left: Option<T>, right: T) -> bool
where
    T: Eq,
{
    match left {
        Some(a) => a != right,
        None => true,
    }
}

pub fn weighted_push<T, W>(vec: &mut Vec<T>, value: &T, weight: W)
where
    W: ZRingValue,
    T: Clone,
{
    let mut w = weight;
    let negone = W::one().neg();
    while w != W::zero() {
        vec.push(value.clone());
        w = w.add_by_ref(&negone);
    }
}

pub fn st_distance_geopoint_geopoint(left: GeoPoint, right: GeoPoint) -> F64 {
    left.distance(&right)
}

pub fn st_distance_geopointN_geopoint(left: Option<GeoPoint>, right: GeoPoint) -> Option<F64> {
    left.map(|x| st_distance_geopoint_geopoint(x, right))
}

pub fn st_distance_geopoint_geopointN(left: GeoPoint, right: Option<GeoPoint>) -> Option<F64> {
    right.map(|x| st_distance_geopoint_geopoint(left, x))
}

pub fn st_distance_geopointN_geopointN(left: Option<GeoPoint>, right: Option<GeoPoint>) -> Option<F64> {
    match (left, right) {
        (None, _) => None,
        (_, None) => None,
        (Some(x), Some(y)) => Some(st_distance_geopoint_geopoint(x, y)),
    }
}

pub fn times_ShortInterval_i64(left: ShortInterval, right: i64) -> ShortInterval {
    left * right
}

pub fn times_ShortIntervalN_i64(left: Option<ShortInterval>, right: i64) -> Option<ShortInterval> {
    left.map(|x| x * right)
}

pub fn times_ShortInterval_i64N(left: ShortInterval, right: Option<i64>) -> Option<ShortInterval> {
    right.map(|x| left * x)
}

pub fn times_ShortIntervalN_i64N(
    left: Option<ShortInterval>,
    right: Option<i64>,
) -> Option<ShortInterval> {
    match (left, right) {
        (None, _) => None,
        (_, None) => None,
        (Some(x), Some(y)) => Some(x * y),
    }
}

/***** decimals ***** */

#[inline(always)]
pub fn round_decimal<T>(left: Decimal, right: T) -> Decimal
where
    u32: TryFrom<T>,
    <u32 as TryFrom<T>>::Error: Debug,
{
    left.round_dp(u32::try_from(right).unwrap())
}

#[inline(always)]
pub fn round_decimalN<T>(left: Option<Decimal>, right: T) -> Option<Decimal>
where
    u32: TryFrom<T>,
    <u32 as TryFrom<T>>::Error: Debug,
{
    left.map(|x| round_decimal(x, right))
}

#[inline(always)]
pub fn truncate_decimal<T>(left: Decimal, right: T) -> Decimal
where
    u32: TryFrom<T>,
    <u32 as TryFrom<T>>::Error: Debug,
{
    left.trunc_with_scale(u32::try_from(right).unwrap())
}

#[inline(always)]
pub fn truncate_decimalN<T>(left: Option<Decimal>, right: T) -> Option<Decimal>
where
    u32: TryFrom<T>,
    <u32 as TryFrom<T>>::Error: Debug,
{
    left.map(|x| truncate_decimal(x, right))
}

#[inline(always)]
pub fn times_decimal_decimal(left: Decimal, right: Decimal) -> Decimal {
    left * right
}

#[inline(always)]
pub fn times_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<Decimal> {
    left.map(|l| l * right)
}

#[inline(always)]
pub fn times_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<Decimal> {
    right.map(|r| left * r)
}

#[inline(always)]
pub fn times_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<Decimal> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l * r),
        _ => None::<Decimal>,
    }
}

#[inline(always)]
pub fn div_decimal_decimal(left: Decimal, right: Decimal) -> Option<Decimal> {
    if right.is_zero() {
        None
    } else {
        Some(left / right)
    }
}

#[inline(always)]
pub fn div_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<Decimal> {
    match left {
        None => None,
        Some(l) => div_decimal_decimal(l, right),
    }
}

#[inline(always)]
pub fn div_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<Decimal> {
    match right {
        Some(r) => div_decimal_decimal(left, r),
        _ => None::<Decimal>,
    }
}

#[inline(always)]
pub fn div_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<Decimal> {
    match (left, right) {
        (Some(l), Some(r)) => div_decimal_decimal(l, r),
        _ => None::<Decimal>,
    }
}

#[inline(always)]
pub fn plus_decimal_decimal(left: Decimal, right: Decimal) -> Decimal {
    left + right
}

#[inline(always)]
pub fn plus_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<Decimal> {
    left.map(|l| l + right)
}

#[inline(always)]
pub fn plus_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<Decimal> {
    right.map(|r| left + r)
}

#[inline(always)]
pub fn plus_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<Decimal> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l + r),
        _ => None::<Decimal>,
    }
}

#[inline(always)]
pub fn minus_decimal_decimal(left: Decimal, right: Decimal) -> Decimal {
    left - right
}

#[inline(always)]
pub fn minus_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<Decimal> {
    left.map(|l| l - right)
}

#[inline(always)]
pub fn minus_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<Decimal> {
    right.map(|r| left - r)
}

#[inline(always)]
pub fn minus_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<Decimal> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l - r),
        _ => None::<Decimal>,
    }
}

#[inline(always)]
pub fn mod_decimal_decimal(left: Decimal, right: Decimal) -> Decimal {
    left % right
}

#[inline(always)]
pub fn mod_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<Decimal> {
    left.map(|l| l % right)
}

#[inline(always)]
pub fn mod_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<Decimal> {
    right.map(|r| left % r)
}

#[inline(always)]
pub fn mod_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<Decimal> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l % r),
        _ => None::<Decimal>,
    }
}

#[inline(always)]
pub fn lt_decimal_decimal(left: Decimal, right: Decimal) -> bool {
    left < right
}

#[inline(always)]
pub fn lt_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<bool> {
    left.map(|l| l < right)
}

#[inline(always)]
pub fn lt_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<bool> {
    right.map(|r| left < r)
}

#[inline(always)]
pub fn lt_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l < r),
        _ => None::<bool>,
    }
}

#[inline(always)]
pub fn eq_decimal_decimal(left: Decimal, right: Decimal) -> bool {
    left == right
}

#[inline(always)]
pub fn eq_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<bool> {
    left.map(|l| l == right)
}

#[inline(always)]
pub fn eq_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<bool> {
    right.map(|r| left == r)
}

#[inline(always)]
pub fn eq_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l == r),
        _ => None::<bool>,
    }
}

#[inline(always)]
pub fn gt_decimal_decimal(left: Decimal, right: Decimal) -> bool {
    left > right
}

#[inline(always)]
pub fn gt_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<bool> {
    left.map(|l| l > right)
}

#[inline(always)]
pub fn gt_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<bool> {
    right.map(|r| left > r)
}

#[inline(always)]
pub fn gt_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l > r),
        _ => None::<bool>,
    }
}

#[inline(always)]
pub fn gte_decimal_decimal(left: Decimal, right: Decimal) -> bool {
    left >= right
}

#[inline(always)]
pub fn gte_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<bool> {
    left.map(|l| l >= right)
}

#[inline(always)]
pub fn gte_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<bool> {
    right.map(|r| left >= r)
}

#[inline(always)]
pub fn gte_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l >= r),
        _ => None::<bool>,
    }
}

#[inline(always)]
pub fn neq_decimal_decimal(left: Decimal, right: Decimal) -> bool {
    left != right
}

#[inline(always)]
pub fn neq_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<bool> {
    left.map(|l| l != right)
}

#[inline(always)]
pub fn neq_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<bool> {
    right.map(|r| left != r)
}

#[inline(always)]
pub fn neq_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l != r),
        _ => None::<bool>,
    }
}

#[inline(always)]
pub fn lte_decimal_decimal(left: Decimal, right: Decimal) -> bool {
    left <= right
}

#[inline(always)]
pub fn lte_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<bool> {
    left.map(|l| l <= right)
}

#[inline(always)]
pub fn lte_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<bool> {
    right.map(|r| left <= r)
}

#[inline(always)]
pub fn lte_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<bool> {
    match (left, right) {
        (Some(l), Some(r)) => Some(l <= r),
        _ => None::<bool>,
    }
}

pub fn element<T>(array: Vec<T>) -> Option<T>
where
    T: Copy,
{
    if array.len() == 1 {
        Some(array[0])
    } else {
        None
    }
}

pub fn elementN<T>(array: Vec<Option<T>>) -> Option<T>
where
    T: Copy,
{
    if array.len() == 1 {
        array[0]
    } else {
        None
    }
}

pub fn power_i32_d(left: i32, right: F64) -> F64 {
    F64::new((left as f64).powf(right.into_inner()))
}

pub fn power_i32_dN(left: i32, right: Option<F64>) -> Option<F64> {
    right.map(|r| power_i32_d(left, r))
}

pub fn power_d_d(left: F64, right: F64) -> F64 {
    F64::new(left.into_inner().powf(right.into_inner()))
}

pub fn power_decimal_decimal(left: Decimal, right: Decimal) -> F64 {
    if right == Decimal::new(5, 1) {
        // special case for sqrt, has higher precision than pow
        F64::from(left.sqrt().unwrap().to_f64().unwrap())
    } else {
        F64::from(left.powd(right).to_f64().unwrap())
    }
}

pub fn power_decimalN_decimal(left: Option<Decimal>, right: Decimal) -> Option<F64> {
    left.map(|l| power_decimal_decimal(l, right))
}

pub fn power_decimal_decimalN(left: Decimal, right: Option<Decimal>) -> Option<F64> {
    right.map(|r| power_decimal_decimal(left, r))
}

pub fn power_decimalN_decimalN(left: Option<Decimal>, right: Option<Decimal>) -> Option<F64> {
    match (left, right) {
        (_, None) => None,
        (l, Some(r)) => power_decimalN_decimal(l, r),
    }
}

pub fn plus_u_u(left: usize, right: usize) -> usize {
    left + right
}

//////////////////// floor /////////////////////

#[inline(always)]
pub fn floor_d(value: F64) -> F64 {
    F64::new(value.into_inner().floor())
}

#[inline(always)]
pub fn floor_dN(value: Option<F64>) -> Option<F64> {
    value.map(|x| F64::new(x.into_inner().floor()))
}

#[inline(always)]
pub fn floor_f(value: F32) -> F32 {
    F32::new(value.into_inner().floor())
}

#[inline(always)]
pub fn floor_fN(value: Option<F32>) -> Option<F32> {
    value.map(|x| F32::new(x.into_inner().floor()))
}

#[inline(always)]
pub fn floor_decimal(value: Decimal) -> Decimal {
    value.floor()
}

#[inline(always)]
pub fn floor_decimalN(value: Option<Decimal>) -> Option<Decimal> {
    value.map(floor_decimal)
}

//////////////////// ceil /////////////////////

#[inline(always)]
pub fn ceil_d(value: F64) -> F64 {
    F64::new(value.into_inner().ceil())
}

#[inline(always)]
pub fn ceil_dN(value: Option<F64>) -> Option<F64> {
    value.map(ceil_d)
}

#[inline(always)]
pub fn ceil_f(value: F32) -> F32 {
    F32::new(value.into_inner().ceil())
}

#[inline(always)]
pub fn ceil_fN(value: Option<F32>) -> Option<F32> {
    value.map(ceil_f)
}

#[inline(always)]
pub fn ceil_decimal(value: Decimal) -> Decimal {
    value.ceil()
}

#[inline(always)]
pub fn ceil_decimalN(value: Option<Decimal>) -> Option<Decimal> {
    value.map(ceil_decimal)
}

///////////////////// sign //////////////////////

#[inline(always)]
pub fn sign_d(value: F64) -> F64 {
    // Rust signum never returns 0
    let x = value.into_inner();
    if x == 0f64 {
        value
    } else {
        F64::new(x.signum())
    }
}

#[inline(always)]
pub fn sign_dN(value: Option<F64>) -> Option<F64> {
    value.map(sign_d)
}

#[inline(always)]
pub fn sign_f(value: F32) -> F32 {
    // Rust signum never returns 0
    let x = value.into_inner();
    if x == 0f32 {
        value
    } else {
        F32::new(x.signum())
    }
}

#[inline(always)]
pub fn sign_fN(value: Option<F32>) -> Option<F32> {
    value.map(sign_f)
}

#[inline(always)]
pub fn sign_decimal(value: Decimal) -> Decimal {
    value.signum()
}

#[inline(always)]
pub fn sign_decimalN(value: Option<Decimal>) -> Option<Decimal> {
    value.map(sign_decimal)
}
