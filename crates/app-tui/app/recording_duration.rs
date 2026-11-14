//! Capture-backed duration details for the Recording Config screen.

use sotf_audio_player::recording_helpers::{
    EffectiveAcquisitionConfig, capture_signal_params, effective_acquisition_config,
    measurement_amplitude, signal_type_for,
};
use sotf_audio_player::ui_models::recording::RecordingScreenModel;

/// Build the exact shared acquisition description for the selected recording
/// settings. This keeps the UI's displayed duration aligned with the capture
/// generator, including octave-sweep silence and playback padding.
pub(crate) fn recording_acquisition_config(
    model: &RecordingScreenModel,
) -> Result<EffectiveAcquisitionConfig, String> {
    let (sweep_start, sweep_end) = model
        .current_recording_channel
        .and_then(|index| model.channel_recordings.get(index))
        .map_or((model.sweep_start_freq, model.sweep_end_freq), |channel| {
            (channel.sweep_start_freq, channel.sweep_end_freq)
        });
    recording_acquisition_config_for_range(model, sweep_start, sweep_end)
}

pub(crate) fn recording_acquisition_config_for_range(
    model: &RecordingScreenModel,
    sweep_start: f32,
    sweep_end: f32,
) -> Result<EffectiveAcquisitionConfig, String> {
    let signal_type = signal_type_for(model.signal_type);
    let params = capture_signal_params(
        signal_type,
        sweep_start,
        sweep_end,
        measurement_amplitude(f64::from(model.signal_level_db)),
        Some(model.bass_octave_duration_s),
        Some(model.pre_silence_s),
        model.post_silence_s,
    );

    effective_acquisition_config(
        signal_type,
        &params,
        model.signal_duration_secs,
        model.playback_config.sample_rate,
    )
}
