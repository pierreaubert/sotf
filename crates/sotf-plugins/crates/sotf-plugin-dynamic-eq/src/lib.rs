#![allow(clippy::duplicate_mod)]
pub mod params;

#[path = "lib/dyn_eq_band.rs"]
mod dyn_eq_band;
#[path = "lib/dyn_eq_band_params.rs"]
mod dyn_eq_band_params;
#[path = "lib/dynamic_eq_data.rs"]
mod dynamic_eq_data;
#[path = "lib/dynamic_eq_plugin.rs"]
mod dynamic_eq_plugin;
#[path = "lib/dynamic_eq_plugin_params.rs"]
mod dynamic_eq_plugin_params;
#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;

pub use dyn_eq_band_params::*;
pub use dynamic_eq_data::*;
pub use dynamic_eq_plugin::*;
pub use dynamic_eq_plugin_params::*;
