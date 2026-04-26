//! Shared recording domain types used by both GPUI and TUI apps.

use serde::{Deserialize, Serialize};

/// Recording screen workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingStep {
    /// Step 1: Configure devices and channel mapping
    #[default]
    Config,
    /// Step 2: SPL calibration — plays a 1 kHz reference tone; user
    /// enters the dBSPL their external meter reads at the listening
    /// position. GD-Opt v2 uses the captured offset to target sweep
    /// levels deterministically (`docs/gd_opt_v2_plan.md` §2.6, §2.11 Q4).
    SplCalibration,
    /// Step 3: Record frequency response for each channel
    Capture,
    /// Step 4: Tone-burst probe for per-channel acoustic delay detection.
    /// Runs once across all channels while the mic is still set up so
    /// the arrival times can flow directly into the Room EQ optimizer
    /// without a separate measurement session.
    Probe,
    /// Step 5: Bass anchor — plays a low-frequency tone burst (20 Hz ×
    /// 5 cycles by default) per channel and records the fundamental's
    /// phase. GD-Opt v2 feeds the per-channel anchor into the sweep
    /// unwrap as a hard constraint on the first bass bin
    /// (`docs/gd_opt_v2_plan.md` §2.6).
    BassAnchor,
    /// Step 6: Evaluate recordings and view frequency response
    Evaluating,
    /// Step 7: Save recordings to disk
    Saving,
}

impl RecordingStep {
    /// Enumerate all steps in UI order. Both frontends iterate this so
    /// the wizard tab bar and step dispatch never drift from the enum.
    pub fn all() -> &'static [RecordingStep] {
        &[
            RecordingStep::Config,
            RecordingStep::SplCalibration,
            RecordingStep::Capture,
            RecordingStep::Probe,
            RecordingStep::BassAnchor,
            RecordingStep::Evaluating,
            RecordingStep::Saving,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            RecordingStep::Config => "Config",
            RecordingStep::SplCalibration => "SPL Cal",
            RecordingStep::Capture => "Capture",
            RecordingStep::Probe => "Probe",
            RecordingStep::BassAnchor => "Bass Anchor",
            RecordingStep::Evaluating => "Evaluate",
            RecordingStep::Saving => "Save",
        }
    }

    pub fn next(&self) -> Option<RecordingStep> {
        match self {
            RecordingStep::Config => Some(RecordingStep::SplCalibration),
            RecordingStep::SplCalibration => Some(RecordingStep::Capture),
            RecordingStep::Capture => Some(RecordingStep::Probe),
            RecordingStep::Probe => Some(RecordingStep::BassAnchor),
            RecordingStep::BassAnchor => Some(RecordingStep::Evaluating),
            RecordingStep::Evaluating => Some(RecordingStep::Saving),
            RecordingStep::Saving => None,
        }
    }

    pub fn previous(&self) -> Option<RecordingStep> {
        match self {
            RecordingStep::Config => None,
            RecordingStep::SplCalibration => Some(RecordingStep::Config),
            RecordingStep::Capture => Some(RecordingStep::SplCalibration),
            RecordingStep::Probe => Some(RecordingStep::Capture),
            RecordingStep::BassAnchor => Some(RecordingStep::Probe),
            RecordingStep::Evaluating => Some(RecordingStep::BassAnchor),
            RecordingStep::Saving => Some(RecordingStep::Evaluating),
        }
    }
}

/// Status of the probe capture (Recording wizard Step 3).
///
/// Mirrors `DelayDetectionStatus` from `room_eq_types` — wall-clock
/// progress via `started_at_ms`, `Failed(String)` for error reporting.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ProbeCaptureStatus {
    #[default]
    Idle,
    Running {
        started_at_ms: u64,
    },
    Complete,
    Failed(String),
}

impl ProbeCaptureStatus {
    /// Estimated fraction of the probe capture completed, in
    /// `0.0..=1.0`, computed from wall-clock elapsed vs. the estimated
    /// total duration. Returns `None` when the status is not `Running`
    /// or the estimated total is zero — callers should render an
    /// indeterminate spinner in that case.
    pub fn progress(&self, estimated_total_ms: u64, now_ms: u64) -> Option<f32> {
        match self {
            Self::Running { started_at_ms } if estimated_total_ms > 0 => {
                let elapsed = now_ms.saturating_sub(*started_at_ms);
                Some((elapsed as f32 / estimated_total_ms as f32).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }
}

/// Shared business state for the Recording wizard "Probe" step.
///
/// Lives on both `RecordingState` (app-gpui) and `RecordingTuiState`
/// (app-tui) so the UIs only manage cursor state locally. The raw
/// results come from the engine (`ProbeDelayResults` aliased as
/// [`DelayProbeResults`]) and flow at save time into
/// `RecordingConfiguration.probe_results`.
#[derive(Debug, Clone)]
pub struct ProbeCaptureState {
    /// Duration of each narrowband tone-burst in milliseconds.
    /// Default 1000 ms — long enough for robust cross-correlation
    /// without making the full sweep tediously slow.
    pub probe_duration_ms: f32,
    /// Silence gap between probes in milliseconds. Avoids overlap
    /// between late reflections of one channel and the onset of the
    /// next.
    pub silence_duration_ms: f32,
    /// Sample rate used for the probe, in Hz. Seeded from the
    /// recording device's negotiated sample rate when the Probe step
    /// is entered; falls back to 48 000.
    pub sample_rate: u32,
    /// Microphone input channel (0-based).
    pub input_channel: u16,
    /// Background-measurement status.
    pub status: ProbeCaptureStatus,
    /// Raw detection results (populated on success). Cleared on
    /// Reset / new run.
    pub results: Option<DelayProbeResults>,
    /// Absolute path to the persisted probe WAV once the capture
    /// succeeds. `None` until a successful run writes the file.
    pub wav_path: Option<String>,
}

impl Default for ProbeCaptureState {
    fn default() -> Self {
        Self {
            probe_duration_ms: 1000.0,
            silence_duration_ms: 500.0,
            sample_rate: 48_000,
            input_channel: 0,
            status: ProbeCaptureStatus::Idle,
            results: None,
            wav_path: None,
        }
    }
}

impl ProbeCaptureState {
    /// Seed the state from a fresh set of probe results plus the
    /// filesystem path of the persisted recording. Sets the status
    /// to `Complete` so the UI renders the results table.
    pub fn apply_results(&mut self, results: DelayProbeResults, wav_path: Option<String>) {
        self.results = Some(results);
        self.wav_path = wav_path;
        self.status = ProbeCaptureStatus::Complete;
    }

    /// Build the per-channel arrival-time map passed into
    /// `run_room_optimization_with_probe_arrivals` at Room EQ time.
    /// Returns `None` unless the status is `Complete` — a failed or
    /// in-flight probe must never contaminate the optimizer input.
    pub fn probe_arrival_map(&self) -> Option<std::collections::HashMap<String, f64>> {
        if !matches!(self.status, ProbeCaptureStatus::Complete) {
            return None;
        }
        let results = self.results.as_ref()?;
        let mut map = std::collections::HashMap::with_capacity(results.channels.len());
        for ch in &results.channels {
            if ch.arrival_ms.is_finite() {
                map.insert(ch.channel_name.clone(), ch.arrival_ms);
            }
        }
        if map.is_empty() { None } else { Some(map) }
    }
}

/// Status of the BassAnchor capture (Recording wizard Step 4).
///
/// Mirrors [`ProbeCaptureStatus`] — wall-clock progress via
/// `started_at_ms`, `Failed(String)` for error reporting. Used by
/// the GD-1e BassAnchor wizard step (`docs/gd_opt_v2_plan.md` §2.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum BassAnchorCaptureStatus {
    #[default]
    Idle,
    Running {
        started_at_ms: u64,
    },
    Complete,
    Failed(String),
}

impl BassAnchorCaptureStatus {
    /// Estimated fraction of the bass-anchor capture completed.
    pub fn progress(&self, estimated_total_ms: u64, now_ms: u64) -> Option<f32> {
        match self {
            Self::Running { started_at_ms } if estimated_total_ms > 0 => {
                let elapsed = now_ms.saturating_sub(*started_at_ms);
                Some((elapsed as f32 / estimated_total_ms as f32).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }
}

/// Shared business state for the Recording wizard "BassAnchor" step.
///
/// Lives alongside [`ProbeCaptureState`]. The raw results come from
/// the engine (`BassAnchorResults`) and flow at save time into
/// `RecordingConfiguration.bass_anchor_results`.
#[derive(Debug, Clone)]
pub struct BassAnchorCaptureState {
    /// Tone-burst centre frequency in Hz. Default 20.0.
    pub bass_freq_hz: f32,
    /// Number of cycles in the burst. Default 5.
    pub bass_cycles: u16,
    /// Silence gap between channels in ms. Default 500.
    pub silence_duration_ms: f32,
    /// Sample rate used for the capture (Hz).
    pub sample_rate: u32,
    /// Microphone input channel (0-based).
    pub input_channel: u16,
    /// Background-measurement status.
    pub status: BassAnchorCaptureStatus,
    /// Raw analysis results. Cleared on Reset / new run.
    pub results: Option<BassAnchorResults>,
    /// Absolute path to the persisted bass-anchor WAV once captured.
    pub wav_path: Option<String>,
}

impl Default for BassAnchorCaptureState {
    fn default() -> Self {
        Self {
            bass_freq_hz: 20.0,
            bass_cycles: 5,
            silence_duration_ms: 500.0,
            sample_rate: 48_000,
            input_channel: 0,
            status: BassAnchorCaptureStatus::Idle,
            results: None,
            wav_path: None,
        }
    }
}

impl BassAnchorCaptureState {
    pub fn apply_results(&mut self, results: BassAnchorResults, wav_path: Option<String>) {
        self.results = Some(results);
        self.wav_path = wav_path;
        self.status = BassAnchorCaptureStatus::Complete;
    }

    /// `true` when every channel's stability metric is under the 20°
    /// advisory threshold from `docs/gd_opt_v2_plan.md` §2.8.
    pub fn all_channels_reliable(&self) -> bool {
        match (&self.status, &self.results) {
            (BassAnchorCaptureStatus::Complete, Some(r)) => r
                .channels
                .iter()
                .all(|c| c.bass_anchor_stability_deg < 20.0),
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// GD-Opt v2 Phase GD-1e.5 — SPL calibration step
// ---------------------------------------------------------------------------

/// Status of the SPL Calibration step (Recording wizard Step 2).
///
/// Mirrors the other capture-status enums. `started_at_ms` is
/// wall-clock; `Failed(String)` carries the engine's reason for the
/// UI to surface.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SplCalibrationCaptureStatus {
    #[default]
    Idle,
    Running {
        started_at_ms: u64,
    },
    Complete,
    Failed(String),
}

impl SplCalibrationCaptureStatus {
    pub fn progress(&self, estimated_total_ms: u64, now_ms: u64) -> Option<f32> {
        match self {
            Self::Running { started_at_ms } if estimated_total_ms > 0 => {
                let elapsed = now_ms.saturating_sub(*started_at_ms);
                Some((elapsed as f32 / estimated_total_ms as f32).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }
}

/// Shared business state for the SplCalibration step.
///
/// Collected from the user across two stages:
/// 1. Engine plays a reference tone and returns a `SplCalibrationResult`
///    (peak + RMS sample levels on the mic).
/// 2. User types the dBSPL their external meter reads while the tone
///    plays; that becomes `reported_db_spl`. `spl_offset_db` is
///    derived via
///    `reported_db_spl − 20 · log10(rms_sample_level)` so that a later
///    capture at the same digital gain predicts its own dBSPL without
///    needing the meter.
///
/// On save, this state is converted to the `SplCalibration` struct
/// the autoeq `RecordingConfiguration` carries.
#[derive(Debug, Clone)]
pub struct SplCalibrationCaptureState {
    /// Reference tone frequency (Hz). Default 1000.
    pub reference_freq_hz: f32,
    /// Digital amplitude of the reference tone (0.0..=1.0). Default 0.25
    /// — leaves ~12 dB headroom and reliably hits 75-85 dBSPL on typical
    /// home systems at normal volume.
    pub tone_amp: f32,
    /// Tone duration in seconds. Default 3.0.
    pub duration_s: f32,
    /// Sample rate used for the capture (Hz).
    pub sample_rate: u32,
    /// Playback output channel (0-based). Default 0 (left / mono).
    pub output_channel: u16,
    /// Microphone input channel (0-based).
    pub input_channel: u16,
    /// Capture status.
    pub status: SplCalibrationCaptureStatus,
    /// Raw engine capture result — `None` until a successful run.
    pub engine_result: Option<SplCalibrationResult>,
    /// dBSPL the user read from their external meter. `None` until
    /// the user has entered a value. Combines with
    /// `engine_result.rms_sample_level` to compute `spl_offset_db`.
    pub reported_db_spl: Option<f32>,
}

impl Default for SplCalibrationCaptureState {
    fn default() -> Self {
        Self {
            reference_freq_hz: 1000.0,
            tone_amp: 0.25,
            duration_s: 3.0,
            sample_rate: 48_000,
            output_channel: 0,
            input_channel: 0,
            status: SplCalibrationCaptureStatus::Idle,
            engine_result: None,
            reported_db_spl: None,
        }
    }
}

impl SplCalibrationCaptureState {
    pub fn apply_engine_result(&mut self, result: SplCalibrationResult) {
        self.engine_result = Some(result);
        self.status = SplCalibrationCaptureStatus::Complete;
    }

    /// `true` once the engine has captured a tone AND the user has
    /// typed the dBSPL their meter read. Consumers gate the Save /
    /// Continue action on this.
    pub fn is_ready(&self) -> bool {
        matches!(self.status, SplCalibrationCaptureStatus::Complete)
            && self.engine_result.is_some()
            && self.reported_db_spl.is_some()
    }

    /// Derive the final `SplCalibration` once both the engine capture
    /// and the user-entered meter reading are present.
    pub fn to_spl_calibration(&self) -> Option<SplCalibration> {
        let er = self.engine_result.as_ref()?;
        let reported = self.reported_db_spl?;
        // Use RMS for the cal anchor because peak is noise-sensitive;
        // the `peak_sample_level` field on SplCalibration still gets
        // filled from the engine result for future SPL-level targeting.
        let level = er.rms_sample_level.max(f32::EPSILON);
        let spl_offset_db = reported - 20.0 * level.log10();
        Some(SplCalibration {
            reported_db_spl: reported,
            reference_freq_hz: er.reference_freq_hz,
            peak_sample_level: er.peak_sample_level,
            spl_offset_db,
        })
    }
}

/// Measurement-unit preference for the room-dimensions form on the
/// Save step. UI state only — the canonical unit on disk is always
/// metric (meters). Call [`RoomDimensionUnit::to_meters`] at save
/// time to convert. Both app-tui and app-gpui re-export this type so
/// the conversion constants live in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomDimensionUnit {
    #[default]
    Metric,
    Imperial,
}

impl RoomDimensionUnit {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Metric => "m",
            Self::Imperial => "ft",
        }
    }

    /// Convert a user-entered value in this unit to canonical meters.
    pub fn to_meters(&self, value: f64) -> f64 {
        match self {
            Self::Metric => value,
            // 1 international foot = 0.3048 m exactly.
            Self::Imperial => value * 0.304_8,
        }
    }

    pub fn toggled(&self) -> Self {
        match self {
            Self::Metric => Self::Imperial,
            Self::Imperial => Self::Metric,
        }
    }
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

// The tone-burst delay detection result types live in the engine
// (`sotf_audio::signal_recorder`) because that's where the measurement is
// implemented. Re-export them under player-layer names so UI code only
// has to depend on `sotf_audio_player` types.
pub use autoeq::roomeq::SplCalibration;
pub use sotf_audio::signal_recorder::{
    BassAnchorChannelResult, BassAnchorResults, ProbeDelayChannelResult as DelayProbeChannelResult,
    ProbeDelayResults as DelayProbeResults, SplCalibrationResult,
};

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
