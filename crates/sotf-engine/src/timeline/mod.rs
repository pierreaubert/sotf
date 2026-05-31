// ============================================================================
// Timeline Module — Multi-track arrangement engine for DAW functionality
// ============================================================================

pub mod audio_input;
pub mod clip;
pub mod midi_track;
pub mod processor;
pub mod recording;
#[allow(
    clippy::module_inception,
    reason = "timeline::timeline is the existing core timeline implementation module"
)]
pub mod timeline;
pub mod track;
pub mod transport;

pub use audio_input::{AudioInput, AudioInputConfig};
pub use clip::{Clip, FadeCurve, Region};
pub use midi_track::{InstrumentPlugin, MidiTrack, NoteEvent, NoteEventKind, TestSynth};
pub use processor::TimelineProcessor;
pub use recording::{RecordingConfig, RecordingResult, RecordingSession};
pub use timeline::Timeline;
pub use track::Track;
pub use transport::Transport;

#[cfg(test)]
mod tests;
