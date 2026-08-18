use crate::app::App;
use crate::app::RecordingField;

pub(super) fn recording_step_prev_wrap(
    s: sotf_audio_player::recording_types::RecordingStep,
) -> sotf_audio_player::recording_types::RecordingStep {
    use sotf_audio_player::recording_types::RecordingStep;
    match s {
        RecordingStep::Config => RecordingStep::Saving,
        RecordingStep::SplCalibration => RecordingStep::Config,
        RecordingStep::Capture => RecordingStep::SplCalibration,
        RecordingStep::Probe => RecordingStep::Capture,
        RecordingStep::BassAnchor => RecordingStep::Probe,
        RecordingStep::Evaluating => RecordingStep::BassAnchor,
        RecordingStep::Saving => RecordingStep::Evaluating,
    }
}

pub(super) fn recording_step_next_wrap(
    s: sotf_audio_player::recording_types::RecordingStep,
) -> sotf_audio_player::recording_types::RecordingStep {
    use sotf_audio_player::recording_types::RecordingStep;
    match s {
        RecordingStep::Config => RecordingStep::SplCalibration,
        RecordingStep::SplCalibration => RecordingStep::Capture,
        RecordingStep::Capture => RecordingStep::Probe,
        RecordingStep::Probe => RecordingStep::BassAnchor,
        RecordingStep::BassAnchor => RecordingStep::Evaluating,
        RecordingStep::Evaluating => RecordingStep::Saving,
        RecordingStep::Saving => RecordingStep::Config,
    }
}

pub(crate) fn recording_field_value_string_kind(app: &App, field: &RecordingField) -> String {
    use RecordingField::*;
    match field {
        Duration => format!("{:.1}", app.recording.model.signal_duration_secs),
        Level => format!("{:.1}", app.recording.model.signal_level_db),
        SweepStart => format!("{:.0}", app.recording.model.sweep_start_freq),
        SweepEnd => format!("{:.0}", app.recording.model.sweep_end_freq),
        NumRecordingChannels => app
            .recording
            .model
            .recording_config
            .num_channels
            .to_string(),
        CtcLoopbackInput => app
            .recording
            .model
            .recording_config
            .ctc_loopback_input_channel
            .map(|c| (c + 1).to_string())
            .unwrap_or_else(|| "1".to_string()),
        NumSweeps => app.recording.model.num_sweeps.to_string(),
        NumPositions => app
            .recording
            .model
            .recording_config
            .num_positions
            .to_string(),
        ChannelInput(i) => app
            .recording
            .model
            .recording_config
            .channel_mappings
            .get(*i)
            .map(|c| (c + 1).to_string())
            .unwrap_or_else(|| "1".to_string()),
        _ => String::new(),
    }
}
