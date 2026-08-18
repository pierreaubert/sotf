use serde::{Deserialize, Serialize};

/// Raw loopback WAV captured for a speaker/position take. This is separate
/// from visible mic recordings so the UI can keep ear mic rows clean while
/// still giving roomeq the loopback path required for raw-sweep CTC solving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferMatrixLoopbackRecording {
    pub speaker_index: usize,
    pub mic_position_index: usize,
    pub wav_path: String,
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
    /// Capture completed but the engine's per-take quality verdict is
    /// `trustworthy == false` (Task 9, review §4 item 1). The take result
    /// (including its [`TakeQualitySummary`]) is stored on the channel; the
    /// user must explicitly accept it (→ `Done`) or re-record (→ `Recording`).
    /// `ReviewNeeded` channels are NOT saved: every save path filters on
    /// `Done`.
    ReviewNeeded,
}

/// UI-facing summary of the engine's per-take quality verdict (Task 9).
///
/// Extracted from `sotf_audio::signal_recorder::CaptureAnalysis` by
/// [`crate::recording_helpers::summarize_take_quality`] and stored on
/// [`RecordingResult`] so the verdict survives from capture completion to
/// the point where the user reviews it. NOTE (task-9 review B1): this is
/// **in-memory only** — the session save serializes `InlineMeasurement`
/// (frequencies/magnitude/phase/paths), which drops the quality summary, so
/// after save → reload every result comes back with `quality: None` and the
/// UI renders "no data". `None` on a result means "captured before quality
/// gating existed" or "loaded from a session file" — never fabricate one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TakeQualitySummary {
    /// math-dsp verdict: false when any issue was raised (lag confidence,
    /// clipping, low coherence/SNR, severe drift advisory).
    pub trustworthy: bool,
    /// Quality score in [0, 1] from math-dsp.
    pub score: f32,
    /// Human-readable issues raised by the quality assessment (engine /
    /// math-dsp wording, English).
    pub issues: Vec<String>,
    /// Mean per-bin coherence across accepted takes (repeat captures with
    /// ≥ 4 accepted takes only).
    pub mean_coherence: Option<f32>,
    /// Median SNR (dB) against the pre-silence noise floor (repeat captures
    /// with a usable noise window only).
    pub median_snr_db: Option<f32>,
    /// Fraction of captured samples at full scale (clipping indicator).
    pub clip_fraction: f32,
    /// Estimated playback/capture clock drift in true ppm. `None` means the
    /// estimation was unavailable (low-confidence window) — this is NOT the
    /// same as 0 ppm and must never be rendered as "0 ppm" (Task-7 review
    /// carry-forward).
    pub drift_ppm: Option<f64>,
    /// True when the capture was time-rescaled for clock drift before
    /// analysis.
    pub drift_corrected: bool,
    /// Input samples dropped to ring-buffer overruns during the capture (R6).
    pub dropped_samples: u64,
    /// Takes accepted into the averaged measurement.
    pub accepted_count: usize,
    /// Takes rejected as outliers during repeat-sweep averaging.
    pub rejected_count: usize,
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
    /// Per-take quality verdict from the engine (Task 9). `None` for results
    /// captured before quality gating or loaded from older session files.
    #[serde(default)]
    pub quality: Option<TakeQualitySummary>,
}
