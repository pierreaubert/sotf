pub use config::AmbisonicsDecoderConfig;

pub mod config;
pub mod decode_matrix;
pub mod params;
pub mod spherical_harmonics;

#[path = "lib/ambisonics_decoder_plugin.rs"]
mod ambisonics_decoder_plugin;
#[path = "lib/consts.rs"]
mod consts;
#[path = "lib/types.rs"]
mod types;

pub use ambisonics_decoder_plugin::*;
pub use types::*;
