use super::external_hosting_backend::select_hosting_backend;
use super::format::format_feature;
use super::format::format_label;
use super::plugin_format::PluginFormat;
use super::types::ExternalHostingBackend;
use super::types::PluginFormatCapability;

/// Build-time format hosting capability matrix.
pub fn plugin_format_capabilities() -> Vec<PluginFormatCapability> {
    [
        PluginFormat::Clap,
        PluginFormat::Vst3,
        PluginFormat::AudioUnit,
    ]
    .into_iter()
    .map(plugin_format_capability)
    .collect()
}

fn plugin_format_capability(format: PluginFormat) -> PluginFormatCapability {
    let feature = format_feature(format).to_string();
    let scan_status = format.build_scan_status();
    let backend = select_hosting_backend(format);
    let native_backend_available = backend != ExternalHostingBackend::Passthrough;
    let reason = if native_backend_available {
        None
    } else {
        Some(format!(
            "{} native hosting feature '{}' is disabled; discovered plugins will be reported as unsupported-by-build",
            format_label(format),
            feature
        ))
    };

    PluginFormatCapability {
        format,
        feature,
        scan_status,
        backend,
        native_backend_available,
        reason,
    }
}
