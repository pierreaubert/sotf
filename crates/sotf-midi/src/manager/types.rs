use crate::message::MidiMessage;
use std::sync::Arc;

/// Callback type for MIDI input messages
pub type MidiCallback = Arc<dyn Fn(MidiMessage) + Send + Sync>;
