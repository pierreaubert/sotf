use super::bass_anchor_capture_status::BassAnchorCaptureStatus;
pub use sotf_audio::signal_recorder::BassAnchorResults;

/// Shared business state for the Recording wizard "BassAnchor" step.
///
/// Lives alongside [`ProbeCaptureState`]. The raw results come from
/// the engine (`BassAnchorResults`) and flow at save time into
/// `RecordingConfiguration.bass_anchor_results`.
#[derive(Debug, Clone)]
pub struct BassAnchorCaptureState {
    /// Steady-state tone centre frequency in Hz. Default 30.0.
    pub bass_freq_hz: f32,
    /// Total tone length in seconds (steady portion + fades). Default 2.0.
    pub bass_duration_s: f32,
    /// Half-Hann fade-in / fade-out length in milliseconds. Default 50.
    pub fade_ms: f32,
    /// Sub-window count for circular-mean / circular-std lock-in
    /// analysis. More windows give a tighter variance estimate; fewer
    /// windows tolerate shorter tones. Default 8.
    pub num_windows: u16,
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
    /// Stereo (mic + loopback) when a loopback channel is configured;
    /// mono otherwise.
    pub wav_path: Option<String>,
    /// Monotonic generation for stale-completion protection in both UIs.
    pub capture_generation: u64,
}

impl Default for BassAnchorCaptureState {
    fn default() -> Self {
        Self {
            bass_freq_hz: 30.0,
            bass_duration_s: 2.0,
            fade_ms: 50.0,
            num_windows: 8,
            silence_duration_ms: 500.0,
            sample_rate: 48_000,
            input_channel: 0,
            status: BassAnchorCaptureStatus::Idle,
            results: None,
            wav_path: None,
            capture_generation: 0,
        }
    }
}

impl BassAnchorCaptureState {
    /// Invalidate prior bass-anchor completions and return the new task
    /// generation.
    pub fn next_capture_generation(&mut self) -> u64 {
        self.capture_generation += 1;
        self.capture_generation
    }

    /// Return whether a completion belongs to the current bass-anchor task.
    pub fn is_current_capture(&self, generation: u64) -> bool {
        self.capture_generation == generation
    }

    pub fn apply_results(&mut self, results: BassAnchorResults, wav_path: Option<String>) {
        self.results = Some(results);
        self.wav_path = wav_path;
        self.status = BassAnchorCaptureStatus::Complete;
    }

    /// `true` when every channel's stability metric is under the 20°
    /// advisory threshold from the GD-Opt v2 plan §2.8
    /// (`docs/gd_opt_v2_plan.md` in the autoeq repo).
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
