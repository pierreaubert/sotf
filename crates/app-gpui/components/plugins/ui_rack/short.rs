use super::super::ui_plugin_shell::plugin_short_name;
use sotf_audio_player::PluginType;

pub(crate) fn short_name(
    plugin_type: &PluginType,
    is_input_mon: bool,
    is_output_mon: bool,
) -> &'static str {
    plugin_short_name(plugin_type, is_input_mon, is_output_mon, false)
}

pub(super) fn short_name_with_permanent(
    plugin_type: &PluginType,
    is_input_mon: bool,
    is_output_mon: bool,
    is_permanent: bool,
) -> &'static str {
    plugin_short_name(plugin_type, is_input_mon, is_output_mon, is_permanent)
}
