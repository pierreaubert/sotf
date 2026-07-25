pub use self::config::BinauralDecoderParams;
pub use self::error::BinauralError;
pub use self::room::{Reflection, ReflectionHrtf, RoomModel};

pub mod config;
pub mod error;
pub mod filter;
pub mod hrtf;
pub mod hrtf_database;
pub mod params;
pub mod room;

#[path = "lib/binaural_decoder_plugin.rs"]
mod binaural_decoder_plugin;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use binaural_decoder_plugin::*;
