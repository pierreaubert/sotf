#![allow(clippy::duplicate_mod)]
pub mod params;

#[path = "lib/convolution_plugin.rs"]
mod convolution_plugin;
#[path = "lib/default.rs"]
mod default;
#[path = "lib/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
#[path = "lib/types.rs"]
mod types;

pub use convolution_plugin::*;
pub use types::*;
