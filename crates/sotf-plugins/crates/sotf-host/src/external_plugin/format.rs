use super::plugin_format::PluginFormat;

pub(super) fn format_feature(format: PluginFormat) -> &'static str {
    match format {
        PluginFormat::Clap => "external-plugin-clap",
        PluginFormat::Vst3 => "external-plugin-vst3",
        PluginFormat::AudioUnit => "external-plugin-au",
    }
}

pub(super) fn format_label(format: PluginFormat) -> &'static str {
    match format {
        PluginFormat::Clap => "CLAP",
        PluginFormat::Vst3 => "VST3",
        PluginFormat::AudioUnit => "AudioUnit",
    }
}
