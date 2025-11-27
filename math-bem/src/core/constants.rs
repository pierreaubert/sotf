//! Physical and integration constants
//!
//! This module provides physical constants and re-exports PhysicsParams.

pub use super::types::PhysicsParams;

use std::f64::consts::PI;

/// Pi (3.14159...)
pub const PI_CONST: f64 = PI;

/// 4π
pub const PI4: f64 = 4.0 * PI;

/// 2π
pub const PI2: f64 = 2.0 * PI;

/// Number of spatial dimensions
pub const NDIM: usize = 3;

/// Maximum nodes per element
pub const NNODPE: usize = 4;

/// Small epsilon for numerical comparisons
pub const EPSY: f64 = 1.0e-14;

/// Default speed of sound in air (m/s) at 20°C
pub const DEFAULT_SPEED_OF_SOUND: f64 = 343.0;

/// Default air density (kg/m³) at 20°C
pub const DEFAULT_DENSITY: f64 = 1.21;

/// Maximum number of subelements for singular integration
pub const MSBE: usize = 220;
