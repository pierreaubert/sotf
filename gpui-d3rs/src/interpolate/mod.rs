//! Interpolation utilities
//!
//! This module provides functions for interpolating between values,
//! inspired by d3-interpolate. It supports numbers, colors, arrays,
//! strings, transforms, and more.
//!
//! # Example
//!
//! ```rust
//! use d3rs::interpolate::{interpolate, interpolate_round};
//!
//! // Interpolate between numbers
//! let lerp = interpolate(0.0, 100.0);
//! assert_eq!(lerp(0.0), 0.0);
//! assert_eq!(lerp(0.5), 50.0);
//! assert_eq!(lerp(1.0), 100.0);
//!
//! // Interpolate with rounding
//! let round_lerp = interpolate_round(0, 10);
//! assert_eq!(round_lerp(0.25), 3);
//! ```

mod number;
mod color;
mod array;
mod string;
mod transform;
mod piecewise;
pub mod zoom;

pub use number::*;
pub use color::*;
pub use array::*;
pub use string::*;
pub use transform::*;
pub use piecewise::*;
