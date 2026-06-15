//! Shared recording domain types used by both GPUI and TUI apps.

pub use autoeq::roomeq::SplCalibration;
pub use sotf_audio::signal_recorder::{
    BassAnchorChannelResult, BassAnchorResults, ProbeDelayChannelResult as DelayProbeChannelResult,
    ProbeDelayResults as DelayProbeResults, SplCalibrationResult,
};

mod bass_anchor_capture_state;
mod bass_anchor_capture_status;
mod calibration_data;
mod channel_mapping;
mod channel_recording;
mod ctc_matrix_export_strategy;
mod default;
mod playback_device_config;
mod plot_smoothing;
mod probe_capture_state;
mod probe_capture_status;
mod recording_device_config;
mod recording_signal_type;
mod recording_step;
mod room_dimension_unit;
mod speaker_configuration;
mod spl_calibration_capture_state;
mod spl_calibration_capture_status;
#[cfg(test)]
mod tests;
mod types;

pub use bass_anchor_capture_state::*;
pub use bass_anchor_capture_status::*;
pub use calibration_data::*;
pub use channel_mapping::*;
pub use channel_recording::*;
pub use ctc_matrix_export_strategy::*;
pub use playback_device_config::*;
pub use plot_smoothing::*;
pub use probe_capture_state::*;
pub use probe_capture_status::*;
pub use recording_device_config::*;
pub use recording_signal_type::*;
pub use recording_step::*;
pub use room_dimension_unit::*;
pub use speaker_configuration::*;
pub use spl_calibration_capture_state::*;
pub use spl_calibration_capture_status::*;
pub use types::*;

/// Shared domain state for the Recording wizard, independent of any UI toolkit.
///
/// This contains only the fields needed to reconstruct Room EQ domain state
/// from a completed recording session. UI-specific cursor/dropdown state stays
/// in the app crates.
#[derive(Debug, Clone)]
pub struct RecordingState {
    /// Playback device configuration used for the session.
    pub playback_config: PlaybackDeviceConfig,
    /// Recording device configuration used for the session.
    pub recording_config: RecordingDeviceConfig,
    /// Per-channel/mic/position recordings produced by the session.
    pub channel_recordings: Vec<ChannelRecording>,
    /// Raw loopback WAVs captured for raw-sweep CTC export.
    pub transfer_matrix_loopbacks: Vec<TransferMatrixLoopbackRecording>,
    /// Path to the reference sweep used for raw-sweep CTC solving.
    pub ctc_reference_sweep_path: Option<String>,
    /// Shared probe-capture (delay-detection) state.
    pub probe_capture: ProbeCaptureState,
    /// Duration of the excitation signal in seconds.
    pub signal_duration_secs: f32,
    /// Directory where recordings are stored.
    pub recording_directory: Option<String>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            channel_recordings: Vec::new(),
            transfer_matrix_loopbacks: Vec::new(),
            ctc_reference_sweep_path: None,
            probe_capture: ProbeCaptureState::default(),
            signal_duration_secs: 5.0,
            recording_directory: None,
        }
    }
}
