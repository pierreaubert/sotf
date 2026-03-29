//! Numeric trait for generic filter implementations.
//!
//! [`FilterFloat`] abstracts over `f32` and `f64`, allowing all filter types
//! in this crate to be instantiated with either precision.

use std::fmt::{Debug, Display};
use std::iter::Sum;

/// Trait bound for the numeric type used throughout the filter crate.
///
/// Implemented for `f32` and `f64`. All filter structs are generic over
/// `T: FilterFloat` with a default of `f64` for backward compatibility.
///
/// # Example
///
/// ```rust
/// use math_audio_iir_fir::{Biquad, BiquadFilterType};
///
/// // f64 (default — same as before)
/// let bq64 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 2.0, 3.0);
///
/// // f32 (explicit)
/// let bq32 = Biquad::<f32>::new(
///     BiquadFilterType::Peak, 1000.0f32, 48000.0f32, 2.0f32, 3.0f32,
/// );
/// ```
pub trait FilterFloat:
    num_traits::Float
    + num_traits::FloatConst
    + num_traits::NumAssign
    + num_traits::FromPrimitive
    + Default
    + Copy
    + Send
    + Sync
    + Debug
    + Display
    + Sum
    + ndarray::ScalarOperand
    + serde::Serialize
    + for<'de> serde::Deserialize<'de>
    + 'static
{
}

impl FilterFloat for f32 {}
impl FilterFloat for f64 {}

/// Convert an `f64` literal to `T`.
///
/// Use for constant values in generic filter code:
/// ```ignore
/// let two_pi = lit::<T>(2.0) * T::PI();
/// ```
#[inline(always)]
pub(crate) fn lit<T: FilterFloat>(v: f64) -> T {
    T::from_f64(v).unwrap()
}
