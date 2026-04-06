//! Shared recording domain types used by both GPUI and TUI apps.

use serde::{Deserialize, Serialize};

/// Recording screen workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingStep {
    /// Step 1: Configure devices and channel mapping
    #[default]
    Config,
    /// Step 2: Record frequency response for each channel
    Capture,
    /// Step 3: Evaluate recordings and view frequency response
    Evaluating,
    /// Step 4: Save recordings to disk
    Saving,
}

/// Smoothing options for frequency response plots (1/N octave)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlotSmoothing {
    /// No smoothing (raw data)
    #[default]
    None,
    /// 1/1 octave smoothing
    Octave1,
    /// 1/3 octave smoothing
    Octave3,
    /// 1/6 octave smoothing
    Octave6,
    /// 1/24 octave smoothing
    Octave24,
}

impl PlotSmoothing {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlotSmoothing::None => "None",
            PlotSmoothing::Octave1 => "1/1 octave",
            PlotSmoothing::Octave3 => "1/3 octave",
            PlotSmoothing::Octave6 => "1/6 octave",
            PlotSmoothing::Octave24 => "1/24 octave",
        }
    }

    /// Get the smoothing factor (fraction of octave)
    pub fn octave_fraction(&self) -> Option<f32> {
        match self {
            PlotSmoothing::None => None,
            PlotSmoothing::Octave1 => Some(1.0),
            PlotSmoothing::Octave3 => Some(1.0 / 3.0),
            PlotSmoothing::Octave6 => Some(1.0 / 6.0),
            PlotSmoothing::Octave24 => Some(1.0 / 24.0),
        }
    }

    pub fn all() -> &'static [PlotSmoothing] {
        &[
            PlotSmoothing::None,
            PlotSmoothing::Octave1,
            PlotSmoothing::Octave3,
            PlotSmoothing::Octave6,
            PlotSmoothing::Octave24,
        ]
    }
}

/// State of a single channel's recording
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelRecordingState {
    /// Not yet recorded
    Empty,
    /// Currently recording
    Recording,
    /// Successfully recorded
    Done,
    /// Recording failed
    Error,
}

/// Configuration for a single speaker's channel mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMapping {
    /// Physical channel indices on the interface (1+ channels)
    pub interface_channels: Vec<usize>,
    /// Channel group name (e.g., "L", "R", "C", "LFE", "SL", "SR")
    pub group_name: String,
}

impl ChannelMapping {
    /// Create a new single-channel mapping
    pub fn single(interface_channel: usize, group_name: impl Into<String>) -> Self {
        Self {
            interface_channels: vec![interface_channel],
            group_name: group_name.into(),
        }
    }

    /// Create a new multi-channel mapping
    pub fn multi(interface_channels: Vec<usize>, group_name: impl Into<String>) -> Self {
        Self {
            interface_channels,
            group_name: group_name.into(),
        }
    }

    /// Check if this speaker is in multi-channel mode
    pub fn is_multi(&self) -> bool {
        self.interface_channels.len() > 1
    }

    /// Get the primary interface channel (first channel in the list)
    pub fn interface_channel(&self) -> usize {
        self.interface_channels.first().copied().unwrap_or(0)
    }

    /// Get the number of channels for this speaker
    pub fn channel_count(&self) -> usize {
        self.interface_channels.len()
    }

    /// Add a channel to this speaker (converts to multi mode if needed)
    pub fn add_channel(&mut self, interface_channel: usize) {
        self.interface_channels.push(interface_channel);
    }

    /// Remove a channel from this speaker by index
    /// Returns true if removed, false if it would leave 0 channels
    pub fn remove_channel(&mut self, channel_index: usize) -> bool {
        if self.interface_channels.len() <= 1 {
            return false;
        }
        if channel_index < self.interface_channels.len() {
            self.interface_channels.remove(channel_index);
            true
        } else {
            false
        }
    }
}

/// Playback device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackDeviceConfig {
    pub device_id: String,
    pub device_name: String,
    pub num_channels: usize,
    pub sample_rate: u32,
    pub available_sample_rates: Vec<u32>,
    pub speaker_configuration: SpeakerConfiguration,
    pub channel_mappings: Vec<ChannelMapping>,
}

impl Default for PlaybackDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 2,
            sample_rate: 48000,
            available_sample_rates: vec![44100, 48000, 88200, 96000, 176400, 192000],
            speaker_configuration: SpeakerConfiguration::Stereo,
            channel_mappings: vec![
                ChannelMapping::single(0, "L"),
                ChannelMapping::single(1, "R"),
            ],
        }
    }
}

impl PlaybackDeviceConfig {
    /// Calculate total number of interface channels from all speaker mappings
    pub fn total_interface_channels(&self) -> usize {
        self.channel_mappings
            .iter()
            .map(|m| m.channel_count())
            .sum()
    }

    /// Update num_channels to match total interface channels
    pub fn sync_channel_count(&mut self) {
        self.num_channels = self.total_interface_channels();
    }
}

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
    /// Calibration file path for each input channel (parallel to channel_mappings)
    #[serde(default)]
    pub mic_calibration_paths: Vec<Option<String>>,
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

/// A saved microphone setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrophonePreset {
    pub name: String,
    pub device_name: String,
    /// Physical input channels used
    pub channel_mappings: Vec<usize>,
    /// Calibration file per channel (parallel to channel_mappings)
    pub mic_calibration_paths: Vec<Option<String>>,
}

/// Persistent config for saved mic presets
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MicrophonePresetsConfig {
    pub presets: Vec<MicrophonePreset>,
}

/// Recording for a single channel with results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRecording {
    /// Speaker/output channel index (into playback_config.channel_mappings)
    pub channel_index: usize,
    pub channel_name: String,
    /// Microphone input index (into recording_config.channel_mappings)
    #[serde(default)]
    pub mic_index: usize,
    pub state: ChannelRecordingState,
    pub result: Option<RecordingResult>,
    /// Per-speaker sweep start frequency in Hz
    #[serde(default = "default_sweep_start_freq")]
    pub sweep_start_freq: f32,
    /// Per-speaker sweep end frequency in Hz
    #[serde(default = "default_sweep_end_freq")]
    pub sweep_end_freq: f32,
}

fn default_sweep_start_freq() -> f32 {
    20.0
}

fn default_sweep_end_freq() -> f32 {
    20000.0
}

impl ChannelRecording {
    /// Create a new channel recording with default freq range based on channel name.
    /// LFE/Sub channels default to 10-500 Hz; all others to 20-20000 Hz.
    pub fn new(channel_index: usize, channel_name: String) -> Self {
        Self::with_mic(channel_index, channel_name, 0)
    }

    /// Create a new channel recording for a specific mic index.
    pub fn with_mic(channel_index: usize, channel_name: String, mic_index: usize) -> Self {
        let name_lower = channel_name.to_ascii_lowercase();
        // Strip " (mic N)" suffix so LFE detection works in multi-mic mode
        let base_name = name_lower
            .find(" (mic ")
            .map_or(name_lower.as_str(), |pos| &name_lower[..pos]);
        let is_lfe = base_name == "lfe" || base_name == "sub";
        Self {
            channel_index,
            channel_name,
            mic_index,
            state: ChannelRecordingState::Empty,
            result: None,
            sweep_start_freq: if is_lfe { 10.0 } else { 20.0 },
            sweep_end_freq: if is_lfe { 500.0 } else { 20000.0 },
        }
    }
}

/// Result of a single channel recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    pub channel: usize,
    pub wav_path: Option<String>,
    pub csv_path: Option<String>,
    pub frequencies: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub phase_deg: Vec<f32>,
    // Advanced metrics
    pub impulse_response: Option<Vec<f32>>,
    pub impulse_time_ms: Option<Vec<f32>>,
    pub thd_percent: Option<Vec<f32>>,
    pub harmonic_distortion_db: Option<Vec<Vec<f32>>>,
    pub excess_group_delay_ms: Option<Vec<f32>>,
    pub rt60_ms: Option<Vec<f32>>,
    pub clarity_c50_db: Option<Vec<f32>>,
    pub clarity_c80_db: Option<Vec<f32>>,
    pub spectrogram_db: Option<Vec<Vec<f32>>>,
}

/// Signal type for test signal generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingSignalType {
    Sweep,
    WhiteNoise,
    PinkNoise,
    /// Narrowband allpass probe for delay/gain detection (800-2000Hz)
    DelayProbe,
}

impl RecordingSignalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecordingSignalType::Sweep => "Sweep",
            RecordingSignalType::WhiteNoise => "White Noise",
            RecordingSignalType::PinkNoise => "Pink Noise",
            RecordingSignalType::DelayProbe => "Delay Probe",
        }
    }

    /// Signal types available for per-channel recording.
    /// `DelayProbe` is excluded here because it uses a separate multi-channel
    /// workflow (`probe_channel_delays`) rather than per-channel sweep recording.
    pub fn all() -> &'static [RecordingSignalType] {
        &[
            RecordingSignalType::Sweep,
            RecordingSignalType::WhiteNoise,
            RecordingSignalType::PinkNoise,
        ]
    }
}

/// Result of a delay probing session across all channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayProbeResults {
    /// Per-channel delay detection results
    pub channels: Vec<DelayProbeChannelResult>,
    /// Sample rate used for probing
    pub sample_rate: u32,
    /// Computed alignment delays in ms (to add to each channel)
    pub alignment_delays_ms: Vec<f64>,
}

/// Delay probe result for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayProbeChannelResult {
    /// Channel name (e.g. "L", "R", "C")
    pub channel_name: String,
    /// Channel output index
    pub channel_index: usize,
    /// Detected arrival time in ms (acoustic propagation delay)
    pub arrival_ms: f64,
    /// Relative gain in dB
    pub gain_db: f64,
    /// Detection confidence (SNR in dB, higher = more reliable)
    pub snr_db: f64,
}

/// Speaker configuration presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeakerConfiguration {
    Stereo,       // 2.0
    Stereo21,     // 2.1
    Surround50,   // 5.0
    Surround51,   // 5.1
    Surround71,   // 7.1
    Surround91,   // 9.1
    Immersive512, // 5.1.2
    Immersive514, // 5.1.4
    Immersive712, // 7.1.2
    Immersive714, // 7.1.4
    Immersive912, // 9.1.2
    Immersive914, // 9.1.4
    Immersive916, // 9.1.6
    Custom,       // User-defined
}

impl SpeakerConfiguration {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpeakerConfiguration::Stereo => "2.0",
            SpeakerConfiguration::Stereo21 => "2.1",
            SpeakerConfiguration::Surround50 => "5.0",
            SpeakerConfiguration::Surround51 => "5.1",
            SpeakerConfiguration::Surround71 => "7.1",
            SpeakerConfiguration::Surround91 => "9.1",
            SpeakerConfiguration::Immersive512 => "5.1.2",
            SpeakerConfiguration::Immersive514 => "5.1.4",
            SpeakerConfiguration::Immersive712 => "7.1.2",
            SpeakerConfiguration::Immersive714 => "7.1.4",
            SpeakerConfiguration::Immersive912 => "9.1.2",
            SpeakerConfiguration::Immersive914 => "9.1.4",
            SpeakerConfiguration::Immersive916 => "9.1.6",
            SpeakerConfiguration::Custom => "Custom",
        }
    }

    pub fn all() -> &'static [SpeakerConfiguration] {
        &[
            SpeakerConfiguration::Stereo,
            SpeakerConfiguration::Stereo21,
            SpeakerConfiguration::Surround50,
            SpeakerConfiguration::Surround51,
            SpeakerConfiguration::Surround71,
            SpeakerConfiguration::Surround91,
            SpeakerConfiguration::Immersive512,
            SpeakerConfiguration::Immersive514,
            SpeakerConfiguration::Immersive712,
            SpeakerConfiguration::Immersive714,
            SpeakerConfiguration::Immersive912,
            SpeakerConfiguration::Immersive914,
            SpeakerConfiguration::Immersive916,
            SpeakerConfiguration::Custom,
        ]
    }

    /// Get the number of channels for this configuration
    pub fn channel_count(&self) -> usize {
        match self {
            SpeakerConfiguration::Stereo => 2,
            SpeakerConfiguration::Stereo21 => 3,
            SpeakerConfiguration::Surround50 => 5,
            SpeakerConfiguration::Surround51 => 6,
            SpeakerConfiguration::Surround71 => 8,
            SpeakerConfiguration::Surround91 => 10,
            SpeakerConfiguration::Immersive512 => 8,
            SpeakerConfiguration::Immersive514 => 10,
            SpeakerConfiguration::Immersive712 => 10,
            SpeakerConfiguration::Immersive714 => 12,
            SpeakerConfiguration::Immersive912 => 12,
            SpeakerConfiguration::Immersive914 => 14,
            SpeakerConfiguration::Immersive916 => 16,
            SpeakerConfiguration::Custom => 2,
        }
    }

    /// Get the default channel names for this configuration
    pub fn default_channel_names(&self) -> Vec<&'static str> {
        match self {
            SpeakerConfiguration::Stereo => vec!["L", "R"],
            SpeakerConfiguration::Stereo21 => vec!["L", "R", "LFE"],
            SpeakerConfiguration::Surround50 => vec!["L", "R", "C", "SL", "SR"],
            SpeakerConfiguration::Surround51 => vec!["L", "R", "C", "LFE", "SL", "SR"],
            SpeakerConfiguration::Surround71 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR"],
            SpeakerConfiguration::Surround91 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR"]
            }
            SpeakerConfiguration::Immersive512 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR"]
            }
            SpeakerConfiguration::Immersive514 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR", "TBL", "TBR"]
            }
            SpeakerConfiguration::Immersive712 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "TFL", "TFR"]
            }
            SpeakerConfiguration::Immersive714 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "TFL", "TFR", "TBL", "TBR",
            ],
            SpeakerConfiguration::Immersive912 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR",
            ],
            SpeakerConfiguration::Immersive914 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR", "TBL",
                "TBR",
            ],
            SpeakerConfiguration::Immersive916 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR", "TML",
                "TMR", "TBL", "TBR",
            ],
            SpeakerConfiguration::Custom => vec!["L", "R"],
        }
    }

    /// Try to detect configuration from channel count
    pub fn from_channel_count(count: usize) -> Self {
        match count {
            2 => SpeakerConfiguration::Stereo,
            3 => SpeakerConfiguration::Stereo21,
            5 => SpeakerConfiguration::Surround50,
            6 => SpeakerConfiguration::Surround51,
            8 => SpeakerConfiguration::Surround71,
            10 => SpeakerConfiguration::Surround91,
            _ => SpeakerConfiguration::Custom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_for_channel_returns_none_for_out_of_bounds() {
        let config = RecordingDeviceConfig::default();
        assert!(config.calibration_for_channel(5).is_none());
    }

    #[test]
    fn test_calibration_for_channel_returns_path() {
        let config = RecordingDeviceConfig {
            mic_calibration_paths: vec![Some("/path/to/cal.txt".to_string())],
            ..Default::default()
        };
        assert_eq!(config.calibration_for_channel(0), Some("/path/to/cal.txt"));
    }

    #[test]
    fn test_calibration_for_channel_returns_none_for_none_entry() {
        let config = RecordingDeviceConfig {
            mic_calibration_paths: vec![None, Some("/path.txt".to_string())],
            ..Default::default()
        };
        assert!(config.calibration_for_channel(0).is_none());
        assert_eq!(config.calibration_for_channel(1), Some("/path.txt"));
    }

    #[test]
    fn test_set_calibration_grows_vec_beyond_channel_mappings() {
        let mut config = RecordingDeviceConfig::default();
        // Default has 1 channel_mapping, set calibration for channel 3
        config.set_calibration_for_channel(3, Some("/path.txt".to_string()));
        assert_eq!(config.mic_calibration_paths.len(), 4);
        assert_eq!(config.calibration_for_channel(3), Some("/path.txt"));
        // Intermediate entries should be None
        assert!(config.calibration_for_channel(1).is_none());
        assert!(config.calibration_for_channel(2).is_none());
    }

    #[test]
    fn test_set_calibration_overwrites_existing() {
        let mut config = RecordingDeviceConfig::default();
        config.set_calibration_for_channel(0, Some("/old.txt".to_string()));
        config.set_calibration_for_channel(0, Some("/new.txt".to_string()));
        assert_eq!(config.calibration_for_channel(0), Some("/new.txt"));
    }

    #[test]
    fn test_set_calibration_clear() {
        let mut config = RecordingDeviceConfig::default();
        config.set_calibration_for_channel(0, Some("/path.txt".to_string()));
        config.set_calibration_for_channel(0, None);
        assert!(config.calibration_for_channel(0).is_none());
    }

    #[test]
    fn test_sync_calibration_paths_pads_to_channel_mappings() {
        let mut config = RecordingDeviceConfig {
            channel_mappings: vec![0, 1, 2],
            mic_calibration_paths: vec![Some("/path.txt".to_string())],
            ..Default::default()
        };
        config.sync_calibration_paths();
        assert_eq!(config.mic_calibration_paths.len(), 3);
        assert_eq!(config.calibration_for_channel(0), Some("/path.txt"));
        assert!(config.calibration_for_channel(1).is_none());
        assert!(config.calibration_for_channel(2).is_none());
    }

    #[test]
    fn test_microphone_preset_serde_roundtrip() {
        let preset = MicrophonePreset {
            name: "UMIK-1".to_string(),
            device_name: "UMIK-1 USB".to_string(),
            channel_mappings: vec![0, 1],
            mic_calibration_paths: vec![
                Some("/cal/ch0.txt".to_string()),
                Some("/cal/ch1.txt".to_string()),
            ],
        };
        let json = serde_json::to_string(&preset).unwrap();
        let deserialized: MicrophonePreset = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "UMIK-1");
        assert_eq!(deserialized.mic_calibration_paths.len(), 2);
    }

    #[test]
    fn test_presets_config_serde_roundtrip() {
        let config = MicrophonePresetsConfig {
            presets: vec![MicrophonePreset {
                name: "Test".to_string(),
                device_name: "Device".to_string(),
                channel_mappings: vec![0],
                mic_calibration_paths: vec![None],
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MicrophonePresetsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.presets.len(), 1);
    }

    #[test]
    fn test_channel_recording_lfe_detection() {
        // Exact match
        let rec = ChannelRecording::new(0, "LFE".to_string());
        assert_eq!(rec.sweep_start_freq, 10.0);
        assert_eq!(rec.sweep_end_freq, 500.0);

        // Case-insensitive
        let rec = ChannelRecording::new(0, "lfe".to_string());
        assert_eq!(rec.sweep_start_freq, 10.0);
        assert_eq!(rec.sweep_end_freq, 500.0);

        // "Sub" variant
        let rec = ChannelRecording::new(0, "Sub".to_string());
        assert_eq!(rec.sweep_start_freq, 10.0);
        assert_eq!(rec.sweep_end_freq, 500.0);

        // Non-LFE channel
        let rec = ChannelRecording::new(0, "L".to_string());
        assert_eq!(rec.sweep_start_freq, 20.0);
        assert_eq!(rec.sweep_end_freq, 20000.0);
    }

    #[test]
    fn test_channel_recording_serde_backward_compat() {
        // Old format without sweep freq fields should deserialize with defaults
        let json = r#"{
            "channel_index": 0,
            "channel_name": "L",
            "state": "Empty",
            "result": null
        }"#;
        let rec: ChannelRecording = serde_json::from_str(json).unwrap();
        assert_eq!(rec.sweep_start_freq, 20.0);
        assert_eq!(rec.sweep_end_freq, 20000.0);
    }

    #[test]
    fn test_recording_device_config_backward_compat_deserialization() {
        // Old format without mic_calibration_paths field
        let json = r#"{
            "device_id": "test",
            "device_name": "Test Device",
            "num_channels": 1,
            "sample_rate": 48000,
            "available_sample_rates": [48000],
            "channel_mappings": [0]
        }"#;
        let config: RecordingDeviceConfig = serde_json::from_str(json).unwrap();
        assert!(config.mic_calibration_paths.is_empty());
        assert!(config.calibration_for_channel(0).is_none());
    }

    /// In multi-mic mode, channel names get a " (Mic N)" suffix.
    /// The LFE/Sub detection must still work so those channels get
    /// the narrow 10-500 Hz sweep range, not the default 20-20000 Hz.
    #[test]
    fn test_lfe_sweep_bounds_with_mic_suffix() {
        // Single-mic: plain name → LFE detection works
        let single = ChannelRecording::new(0, "LFE".to_string());
        assert_eq!(single.sweep_start_freq, 10.0, "single-mic LFE start");
        assert_eq!(single.sweep_end_freq, 500.0, "single-mic LFE end");

        let single_sub = ChannelRecording::new(0, "Sub".to_string());
        assert_eq!(single_sub.sweep_start_freq, 10.0, "single-mic Sub start");
        assert_eq!(single_sub.sweep_end_freq, 500.0, "single-mic Sub end");

        // Multi-mic: name has " (Mic N)" suffix → LFE detection must still work
        let multi_lfe = ChannelRecording::with_mic(0, "LFE (Mic 1)".to_string(), 0);
        assert_eq!(multi_lfe.sweep_start_freq, 10.0, "multi-mic LFE start");
        assert_eq!(multi_lfe.sweep_end_freq, 500.0, "multi-mic LFE end");

        let multi_sub = ChannelRecording::with_mic(0, "Sub (Mic 2)".to_string(), 1);
        assert_eq!(multi_sub.sweep_start_freq, 10.0, "multi-mic Sub start");
        assert_eq!(multi_sub.sweep_end_freq, 500.0, "multi-mic Sub end");

        // Non-LFE channels must still get full range
        let multi_l = ChannelRecording::with_mic(0, "L (Mic 1)".to_string(), 0);
        assert_eq!(multi_l.sweep_start_freq, 20.0, "multi-mic L start");
        assert_eq!(multi_l.sweep_end_freq, 20000.0, "multi-mic L end");
    }
}
