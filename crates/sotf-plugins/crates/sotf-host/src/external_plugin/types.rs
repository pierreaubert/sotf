use super::plugin_format::PluginFormat;
use serde::{Deserialize, Serialize};

/// Scanner status for a discovered external plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PluginScanStatus {
    /// The plugin was found on disk, but loadability has not been evaluated.
    #[default]
    Discovered,
    /// The plugin format has a native backend in this build.
    Loadable,
    /// The plugin format is recognized, but this build lacks the native loader feature.
    UnsupportedByBuild,
}

/// Build-time hosting capability for one external plugin format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginFormatCapability {
    pub format: PluginFormat,
    pub feature: String,
    pub scan_status: PluginScanStatus,
    pub backend: ExternalHostingBackend,
    pub native_backend_available: bool,
    pub reason: Option<String>,
}

/// How the scanner should annotate discovered plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PluginScanStatusMode {
    /// Preserve raw discovery status; callers can probe/build-annotate later.
    DiscoveryOnly,
    /// Mark each result according to this build's native hosting capability.
    #[default]
    BuildCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPluginSandboxMode {
    #[default]
    InProcess,
    Isolated,
    Disabled,
}

/// An external plugin instance that implements the `Plugin` trait.
///
/// This is the bridge between external plugin formats and SOTF's plugin system.
/// Format-specific hosting is staged in this order:
/// 1) CLAP
/// 2) VST3
/// 3) Audio Unit
///
/// Until a native backend is enabled, the plugin runs in deterministic
/// passthrough mode so graph behavior remains stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalHostingBackend {
    Passthrough,
    Clap,
    Vst3,
    AudioUnit,
}

/// Host-side plan for loading an external plugin descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalPluginHostingPlan {
    pub format: PluginFormat,
    pub feature: String,
    pub scan_status: PluginScanStatus,
    pub backend: ExternalHostingBackend,
    pub native_backend_available: bool,
    pub reason: Option<String>,
}
