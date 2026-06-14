use std::sync::{Arc, Mutex};

pub(super) type RecordingResultSlot = Arc<
    Mutex<
        Option<
            Result<
                (
                    Vec<(usize, sotf_audio_player::recording_types::RecordingResult)>,
                    Option<sotf_audio_player::recording_types::TransferMatrixLoopbackRecording>,
                ),
                String,
            >,
        >,
    >,
>;
