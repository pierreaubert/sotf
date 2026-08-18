use super::ctc_matrix_export_strategy::CtcMatrixExportStrategy;
use super::default::default_num_positions;
use serde::{Deserialize, Serialize};

/// Recording device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingDeviceConfig {
    pub device_id: String,
    pub device_name: String,
    pub num_channels: usize,
    pub sample_rate: u32,
    pub available_sample_rates: Vec<u32>,
    /// Mapping from physical input channels to recording channels
    pub channel_mappings: Vec<usize>,
    /// Calibration file path per mic slot — indexed by mic index, NOT by
    /// hardware input channel (parallel to channel_mappings)
    #[serde(default)]
    pub mic_calibration_paths: Vec<Option<String>>,
    /// Number of measurement positions (seats). Each position runs a full
    /// speaker × mic sweep; between positions the user is prompted to move
    /// the microphones. Defaults to 1 to keep older sessions byte-compatible.
    #[serde(default = "default_num_positions")]
    pub num_positions: usize,
    /// Export strategy for CTC/N-by-M transfer-matrix measurements. Defaults
    /// to the current measured impulse-response path.
    #[serde(default)]
    pub ctc_matrix_strategy: CtcMatrixExportStrategy,
    /// Physical input channel used as loopback/reference when
    /// `ctc_matrix_strategy == RawSweep`. Zero-based, same convention as
    /// `channel_mappings`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctc_loopback_input_channel: Option<usize>,
}

impl Default for RecordingDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 1,
            sample_rate: 48000,
            available_sample_rates: vec![44100, 48000, 88200, 96000, 176400, 192000],
            channel_mappings: vec![0],
            mic_calibration_paths: vec![None],
            num_positions: 1,
            ctc_matrix_strategy: CtcMatrixExportStrategy::default(),
            ctc_loopback_input_channel: None,
        }
    }
}

impl RecordingDeviceConfig {
    /// Get the calibration file path for a given channel index
    pub fn calibration_for_channel(&self, idx: usize) -> Option<&str> {
        self.mic_calibration_paths
            .get(idx)
            .and_then(|p| p.as_deref())
    }

    /// Set the calibration file path for a given channel index, growing the vec if needed
    pub fn set_calibration_for_channel(&mut self, idx: usize, path: Option<String>) {
        // Grow to fit both channel_mappings and the target index
        self.sync_calibration_paths();
        while self.mic_calibration_paths.len() <= idx {
            self.mic_calibration_paths.push(None);
        }
        self.mic_calibration_paths[idx] = path;
    }

    /// Pad mic_calibration_paths to match channel_mappings length
    pub fn sync_calibration_paths(&mut self) {
        while self.mic_calibration_paths.len() < self.channel_mappings.len() {
            self.mic_calibration_paths.push(None);
        }
    }
}
