//! [`AudioDriver`] implementation wrapping the macOS CoreAudio HAL shared memory interface.
//!
//! This adapter connects the platform-agnostic [`AudioDriver`] trait to the existing
//! [`HalInputReader`] and [`SharedAudioBuffer`] types.

mod consts;
mod hal_driver;
mod misc;
#[cfg(test)]
mod tests;

pub use hal_driver::*;
