use super::super::parameters::TuiEditablePlugin;

pub(in super::super) fn get_param_count(settings: &sotf_audio_player::PluginSettings) -> usize {
    settings.get_descriptors().len()
}
