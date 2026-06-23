pub use sotf_host::lr4_crossover::CROSSOVER_PRESETS;

pub mod params;

#[path = "lib/band_compressor_params.rs"]
mod band_compressor_params;
#[path = "lib/multiband_compressor_data.rs"]
mod multiband_compressor_data;
#[path = "lib/multiband_compressor_plugin.rs"]
mod multiband_compressor_plugin;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use band_compressor_params::*;
pub use multiband_compressor_data::*;
pub use multiband_compressor_plugin::*;
pub use types::*;
