pub mod params;

#[path = "lib/band_merge_plugin.rs"]
mod band_merge_plugin;
#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use band_merge_plugin::*;
pub use types::*;
