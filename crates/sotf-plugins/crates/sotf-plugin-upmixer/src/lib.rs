pub use config::*;

mod bass;
mod config;
mod decorrelation;
mod detection;
mod fft;
mod frequency_domain;
mod height;
mod hr_processing;
#[cfg(feature = "onnx")]
mod ml_features;
#[cfg(feature = "onnx")]
mod ml_inference;
mod output;
mod panning;
pub mod params;
mod process;
mod setup;

#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;
#[path = "lib/upmixer_plugin.rs"]
mod upmixer_plugin;

pub use types::*;
pub use upmixer_plugin::*;
