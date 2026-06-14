use super::types::PluginScanStatus;
use serde::{Deserialize, Serialize};

/// Supported external plugin formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginFormat {
    /// CLAP (CLever Audio Plugin) — modern, open standard
    Clap,
    /// VST3 (Virtual Studio Technology 3) — Steinberg standard
    Vst3,
    /// AU (Audio Unit) — macOS/iOS standard
    AudioUnit,
}

impl PluginFormat {
    /// File extension for plugin bundles on the current platform.
    pub fn extension(&self) -> &str {
        match self {
            PluginFormat::Clap => "clap",
            PluginFormat::Vst3 => "vst3",
            PluginFormat::AudioUnit => "component",
        }
    }

    /// Scanner status implied by the current build's native hosting features.
    pub fn build_scan_status(self) -> PluginScanStatus {
        match self {
            PluginFormat::Clap if cfg!(feature = "external-plugin-clap") => {
                PluginScanStatus::Loadable
            }
            PluginFormat::Vst3 if cfg!(feature = "external-plugin-vst3") => {
                PluginScanStatus::Loadable
            }
            PluginFormat::AudioUnit if cfg!(feature = "external-plugin-au") => {
                PluginScanStatus::Loadable
            }
            _ => PluginScanStatus::UnsupportedByBuild,
        }
    }
}
