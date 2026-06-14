//! Runtime coordinator: connects MIDI input to plugin parameters
//!
//! Handles the full flow: MIDI message → resolve control → find binding → update parameter.
//! Supports MIDI learn, paging, and LED feedback.

mod midi_mapping_engine;
mod misc;
#[cfg(test)]
mod tests;
mod types;

pub use midi_mapping_engine::*;
pub use types::*;
