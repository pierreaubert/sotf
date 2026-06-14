#![allow(clippy::duplicate_mod)]
pub mod params;

#[path = "lib/default.rs"]
mod default;
#[path = "lib/eq_band.rs"]
mod eq_band;
#[path = "lib/fir_designer_plugin.rs"]
mod fir_designer_plugin;
#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use fir_designer_plugin::*;
pub use types::*;
