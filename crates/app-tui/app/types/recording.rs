use super::RecordingField;
use super::recording_tui_state::RecordingTuiState;

/// Map a flat field index to its logical identity. Returns `None` past the
/// last valid row.
pub fn recording_field_at(s: &RecordingTuiState, idx: usize) -> Option<RecordingField> {
    use RecordingField::*;
    let n = s.recording_config.num_channels.max(1);
    match idx {
        0 => Some(PlaybackDevice),
        1 => Some(RecordingDevice),
        2 => Some(SpeakerConfig),
        3 => Some(SignalType),
        4 => Some(Duration),
        5 => Some(Level),
        6 => Some(SweepStart),
        7 => Some(SweepEnd),
        8 => Some(OutputDir),
        9 => Some(NumRecordingChannels),
        10 => Some(CtcStrategy),
        11 => Some(CtcLoopbackInput),
        i if i < 12 + n => Some(MicCal(i - 12)),
        i if i < 12 + 2 * n => Some(ChannelInput(i - 12 - n)),
        _ => None,
    }
}

/// Total number of selectable rows for the current state.
pub fn recording_field_count(s: &RecordingTuiState) -> usize {
    12 + 2 * s.recording_config.num_channels.max(1)
}
