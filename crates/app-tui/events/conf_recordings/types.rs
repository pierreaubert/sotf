use std::sync::{Arc, Mutex};

/// Background result slot for a probe capture. The generation is captured
/// when the task is spawned so a late completion cannot overwrite a newer
/// probe run.
pub(super) type ProbeCaptureResultSlot = Arc<
    Mutex<
        Option<(
            u64,
            Result<
                (
                    sotf_audio_player::recording_types::DelayProbeResults,
                    String,
                ),
                String,
            >,
        )>,
    >,
>;

/// Background result slot for an SPL calibration capture.
pub(super) type SplCaptureResultSlot = Arc<
    Mutex<
        Option<(
            u64,
            Result<sotf_audio::signal_recorder::SplCalibrationResult, String>,
        )>,
    >,
>;

/// Background result slot for a bass-anchor capture.
pub(super) type BassAnchorCaptureResultSlot = Arc<
    Mutex<
        Option<(
            u64,
            Result<
                (
                    sotf_audio_player::recording_types::BassAnchorResults,
                    String,
                ),
                String,
            >,
        )>,
    >,
>;

/// Recording-result slot shared with the capture thread. The `u64` is the
/// capture generation at spawn time (task 10 / F1): `poll_recording` compares
/// it against `RecordingScreenModel::capture_generation` and discards the
/// result when a newer capture has since been spawned.
pub(super) type RecordingResultSlot = Arc<
    Mutex<
        Option<
            (
                u64,
                Result<
                    (
                        Vec<(usize, sotf_audio_player::recording_types::RecordingResult)>,
                        Option<sotf_audio_player::recording_types::TransferMatrixLoopbackRecording>,
                    ),
                    String,
                >,
            ),
        >,
    >,
>;
