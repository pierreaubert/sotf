#![allow(clippy::duplicate_mod)]
pub mod params;

#[path = "lib/default.rs"]
mod default;
#[path = "lib/eq_band.rs"]
mod eq_band;
#[path = "lib/linear_phase_eq_plugin.rs"]
mod linear_phase_eq_plugin;
#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use linear_phase_eq_plugin::*;
pub use types::*;
