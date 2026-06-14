use sotf_audio::plugins::upmixer_output_channels;

pub(super) fn upmixer_settings_output_channels(
    speaker_config: &str,
    binaural_preview: bool,
) -> usize {
    if binaural_preview {
        2
    } else {
        upmixer_output_channels(speaker_config)
    }
}
