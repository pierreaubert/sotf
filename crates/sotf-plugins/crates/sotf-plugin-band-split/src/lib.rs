pub mod params;

#[path = "lib/band_split_plugin.rs"]
mod band_split_plugin;
#[path = "lib/crossover_mode.rs"]
mod crossover_mode;
#[path = "lib/default.rs"]
mod default;
#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use band_split_plugin::*;
pub use types::*;
