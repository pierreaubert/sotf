pub mod params;

#[path = "lib/default.rs"]
mod default;
#[path = "lib/dither_plugin.rs"]
mod dither_plugin;
#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use dither_plugin::*;
pub use types::*;
