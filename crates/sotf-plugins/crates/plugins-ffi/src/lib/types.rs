use super::PluginFfiHostKind;

pub(super) fn current_host_kind() -> PluginFfiHostKind {
    if cfg!(target_os = "windows") {
        PluginFfiHostKind::Vst3
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        PluginFfiHostKind::AudioUnitV3
    } else {
        PluginFfiHostKind::Unknown
    }
}
