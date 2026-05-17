// ============================================================================
// External Plugin Hosting — Framework for hosting third-party VST3/CLAP/AU plugins
// ============================================================================
//
// This module provides the architecture for hosting external (third-party) audio
// plugins inside the SOTF engine. It defines:
//
// - `PluginFormat`: Supported plugin formats (CLAP, VST3, AU)
// - `PluginDescriptor`: Metadata about a discovered plugin
// - `PluginScanner`: Discovers installed plugins on the system
// - `ExternalPlugin`: Wraps an external plugin instance as a `Plugin` trait object
//
// The actual format-specific hosting (CLAP via clack-host, VST3 via vst3-sys,
// AU via coreaudio-rs) is behind feature flags and can be implemented incrementally.

use crate::parameters::{Parameter, ParameterId, ParameterValue};

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
}
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::path::{Path, PathBuf};

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
}

/// Metadata about a discovered external plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptor {
    /// Unique plugin identifier (format-specific)
    pub id: String,
    /// Display name
    pub name: String,
    /// Vendor/manufacturer
    pub vendor: String,
    /// Version string
    pub version: String,
    /// Plugin format
    pub format: PluginFormat,
    /// Path to the plugin file/bundle
    pub path: PathBuf,
    /// Number of audio inputs (0 for instruments)
    pub audio_inputs: usize,
    /// Number of audio outputs
    pub audio_outputs: usize,
    /// Whether this is an instrument (generates audio from MIDI)
    pub is_instrument: bool,
    /// Plugin categories/tags
    pub categories: Vec<String>,
}

/// Discovers installed plugins on the system.
pub struct PluginScanner {
    /// Discovered plugins
    pub plugins: Vec<PluginDescriptor>,
}

impl PluginScanner {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Scan standard plugin directories for all supported formats.
    pub fn scan_all(&mut self) {
        self.scan_format(PluginFormat::Clap);
        self.scan_format(PluginFormat::Vst3);
        #[cfg(target_os = "macos")]
        self.scan_format(PluginFormat::AudioUnit);
    }

    /// Scan for plugins of a specific format.
    pub fn scan_format(&mut self, format: PluginFormat) {
        for dir in Self::search_paths(format) {
            if dir.exists() {
                self.scan_directory(&dir, format);
            }
        }
    }

    /// Get standard search paths for a plugin format.
    fn search_paths(format: PluginFormat) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        match format {
            PluginFormat::Clap => {
                // Standard CLAP locations
                #[cfg(target_os = "macos")]
                {
                    if let Some(home) = home_dir() {
                        paths.push(home.join("Library/Audio/Plug-Ins/CLAP"));
                    }
                    paths.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
                }
                #[cfg(target_os = "linux")]
                {
                    if let Some(home) = home_dir() {
                        paths.push(home.join(".clap"));
                    }
                    paths.push(PathBuf::from("/usr/lib/clap"));
                }
                #[cfg(target_os = "windows")]
                {
                    if let Ok(pf) = std::env::var("COMMONPROGRAMFILES") {
                        paths.push(PathBuf::from(pf).join("CLAP"));
                    }
                }
            }
            PluginFormat::Vst3 => {
                #[cfg(target_os = "macos")]
                {
                    if let Some(home) = home_dir() {
                        paths.push(home.join("Library/Audio/Plug-Ins/VST3"));
                    }
                    paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
                }
                #[cfg(target_os = "linux")]
                {
                    if let Some(home) = home_dir() {
                        paths.push(home.join(".vst3"));
                    }
                    paths.push(PathBuf::from("/usr/lib/vst3"));
                }
                #[cfg(target_os = "windows")]
                {
                    if let Ok(pf) = std::env::var("COMMONPROGRAMFILES") {
                        paths.push(PathBuf::from(pf).join("VST3"));
                    }
                }
            }
            PluginFormat::AudioUnit => {
                #[cfg(target_os = "macos")]
                {
                    if let Some(home) = home_dir() {
                        paths.push(home.join("Library/Audio/Plug-Ins/Components"));
                    }
                    paths.push(PathBuf::from("/Library/Audio/Plug-Ins/Components"));
                }
            }
        }
        paths
    }

    /// Scan a directory for plugin files of the given format.
    fn scan_directory(&mut self, dir: &Path, format: PluginFormat) {
        let ext = format.extension();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!(
                    "external_plugin: cannot read plugin dir {}: {e}",
                    dir.display()
                );
                return;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::warn!(
                        "external_plugin: unreadable entry under {}: {e}",
                        dir.display()
                    );
                    continue;
                }
            };
            {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == ext) {
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    self.plugins.push(PluginDescriptor {
                        id: format!("{}.{name}", format.extension()),
                        name: name.clone(),
                        vendor: "Unknown".into(),
                        version: "Unknown".into(),
                        format,
                        path,
                        audio_inputs: 2,
                        audio_outputs: 2,
                        is_instrument: false,
                        categories: Vec::new(),
                    });
                }
            }
        }
    }

    /// Find a plugin by name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&PluginDescriptor> {
        let lower = name.to_lowercase();
        self.plugins.iter().find(|p| p.name.to_lowercase() == lower)
    }

    /// List all discovered plugins.
    pub fn list(&self) -> &[PluginDescriptor] {
        &self.plugins
    }
}

/// An external plugin instance that implements the `Plugin` trait.
///
/// This is the bridge between external plugin formats and SOTF's plugin system.
/// Currently a stub — actual format-specific loading requires:
/// - CLAP: `clack-host` crate
/// - VST3: `vst3-sys` crate
/// - AU: `coreaudio-rs` AudioUnit API
pub struct ExternalPlugin {
    descriptor: PluginDescriptor,
    input_channels: usize,
    output_channels: usize,
    _sample_rate: u32,
    parameters: Vec<Parameter>,
    /// Format-specific plugin instance (opaque)
    _instance: Option<Box<dyn Any + Send>>,
}

impl ExternalPlugin {
    /// Create a new external plugin wrapper from a descriptor.
    ///
    /// Note: This currently creates a passthrough stub. Actual plugin loading
    /// requires format-specific implementation behind feature flags.
    pub fn new(descriptor: &PluginDescriptor, sample_rate: u32) -> Result<Self, String> {
        Ok(Self {
            descriptor: descriptor.clone(),
            input_channels: descriptor.audio_inputs,
            output_channels: descriptor.audio_outputs,
            _sample_rate: sample_rate,
            parameters: Vec::new(),
            _instance: None,
        })
    }

    /// Get the plugin descriptor.
    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }
}

impl Plugin for ExternalPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(&self.descriptor.name, &self.descriptor.version, &self.descriptor.vendor)
    }

    fn input_channels(&self) -> usize {
        self.input_channels
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.parameters.clone()
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        // TODO: Forward to plugin instance
        Ok(())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        // TODO: Query plugin instance
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> PluginResult<usize> {
        // Stub: passthrough (copy input to output)
        // Real implementation would call the plugin's process function
        let copy_len = input.len().min(output.len());
        output[..copy_len].copy_from_slice(&input[..copy_len]);
        Ok(ctx.num_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_scanner_search_paths() {
        // Verify search paths are non-empty for at least one format
        let paths = PluginScanner::search_paths(PluginFormat::Clap);
        assert!(!paths.is_empty(), "Should have CLAP search paths");
    }

    #[test]
    fn test_plugin_scanner_scan_nonexistent() {
        let mut scanner = PluginScanner::new();
        scanner.scan_directory(Path::new("/nonexistent/path"), PluginFormat::Clap);
        assert!(scanner.plugins.is_empty());
    }

    #[test]
    fn test_external_plugin_passthrough() {
        let desc = PluginDescriptor {
            id: "test.plugin".into(),
            name: "Test Plugin".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: PathBuf::from("/fake/path"),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec![],
        };

        let mut plugin = ExternalPlugin::new(&desc, 48000).unwrap();
        let input = vec![0.5f32; 2048];
        let mut output = vec![0.0f32; 2048];
        let ctx = ProcessContext::new(48000, 1024);

        let frames = plugin.process(&input, &mut output, &ctx).unwrap();
        assert_eq!(frames, 1024);
        // Passthrough: output should match input
        for i in 0..2048 {
            assert!((output[i] - input[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn test_plugin_format_extension() {
        assert_eq!(PluginFormat::Clap.extension(), "clap");
        assert_eq!(PluginFormat::Vst3.extension(), "vst3");
        assert_eq!(PluginFormat::AudioUnit.extension(), "component");
    }
}
