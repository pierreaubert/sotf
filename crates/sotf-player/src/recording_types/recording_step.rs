/// Recording screen workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingStep {
    /// Step 1: Configure devices and channel mapping
    #[default]
    Config,
    /// Step 2: SPL calibration — plays a 1 kHz reference tone; user
    /// enters the dBSPL their external meter reads at the listening
    /// position. GD-Opt v2 uses the captured offset to target sweep
    /// levels deterministically (GD-Opt v2 plan §2.6, §2.11 Q4 —
    /// `docs/gd_opt_v2_plan.md` in the autoeq repo).
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
    /// (GD-Opt v2 plan §2.6, `docs/gd_opt_v2_plan.md` in the autoeq repo).
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
