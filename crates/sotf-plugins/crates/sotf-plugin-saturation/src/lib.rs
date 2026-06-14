pub mod params;

#[path = "lib/default.rs"]
mod default;
#[path = "lib/misc.rs"]
mod misc;
#[path = "lib/saturation_mode.rs"]
mod saturation_mode;
#[path = "lib/saturation_plugin.rs"]
mod saturation_plugin;
#[path = "lib/saturation_plugin_params.rs"]
mod saturation_plugin_params;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;

pub use saturation_mode::*;
pub use saturation_plugin::*;
pub use saturation_plugin_params::*;
