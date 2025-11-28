//! Custom GPU-accelerated UI elements
//!
//! This module contains custom Element implementations that use direct GPU rendering
//! for high-performance audio visualization widgets.

pub mod eq_curve;
pub mod level_meter;
pub mod spectrum;

pub use eq_curve::{CompactEQCurve, EQCurveColors, EQCurveElement};
pub use level_meter::{LevelMeterElement, MeterColors};
pub use spectrum::{MeterData, SpectrumColors, SpectrumElement};
