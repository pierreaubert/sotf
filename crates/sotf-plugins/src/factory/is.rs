use super::catalog::catalog_entry;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::ExternalPluginTrust;

pub fn is_supported_plugin_type(plugin_type: &str) -> bool {
    catalog_entry(plugin_type).is_some()
}

pub(super) fn is_external_plugin_type(plugin_type: &str) -> bool {
    catalog_entry(plugin_type).is_some_and(|entry| entry.canonical_type == "external")
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn is_untrusted_external_plugin(trust: ExternalPluginTrust) -> bool {
    matches!(
        trust,
        ExternalPluginTrust::Unknown | ExternalPluginTrust::Untrusted
    )
}
