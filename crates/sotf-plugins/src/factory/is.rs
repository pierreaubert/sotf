use super::consts::SUPPORTED_PLUGIN_TYPES;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::ExternalPluginTrust;

pub fn is_supported_plugin_type(plugin_type: &str) -> bool {
    let lower = plugin_type.to_lowercase();
    SUPPORTED_PLUGIN_TYPES.contains(&lower.as_str())
}

pub(super) fn is_external_plugin_type(plugin_type: &str) -> bool {
    matches!(plugin_type, "external" | "external_plugin")
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn is_untrusted_external_plugin(trust: ExternalPluginTrust) -> bool {
    matches!(
        trust,
        ExternalPluginTrust::Unknown | ExternalPluginTrust::Untrusted
    )
}
