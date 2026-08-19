use super::spl_calibration_capture_status::SplCalibrationCaptureStatus;
pub use autoeq::roomeq::SplCalibration;
pub use sotf_audio::signal_recorder::SplCalibrationResult;

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
    /// Monotonic generation for stale-completion protection in both UIs.
    pub capture_generation: u64,
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
            capture_generation: 0,
        }
    }
}

impl SplCalibrationCaptureState {
    /// Invalidate prior SPL completions and return the new task generation.
    pub fn next_capture_generation(&mut self) -> u64 {
        self.capture_generation += 1;
        self.capture_generation
    }

    /// Return whether a completion belongs to the current SPL task.
    pub fn is_current_capture(&self, generation: u64) -> bool {
        self.capture_generation == generation
    }

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
