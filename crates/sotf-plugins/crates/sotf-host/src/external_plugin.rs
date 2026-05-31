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

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::PluginError;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use crate::serialization::{PluginPreset, SerializablePlugin};

use serde::{Deserialize, Serialize};
use std::any::Any;
#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
use std::ffi::c_void;

#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
use libloading::Library;

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
}

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginScanSummary {
    pub total: usize,
    pub discovered: usize,
    pub loadable: usize,
    pub unsupported_by_build: usize,
}

impl PluginScanSummary {
    pub fn record(&mut self, status: PluginScanStatus) {
        self.total += 1;
        match status {
            PluginScanStatus::Discovered => self.discovered += 1,
            PluginScanStatus::Loadable => self.loadable += 1,
            PluginScanStatus::UnsupportedByBuild => self.unsupported_by_build += 1,
        }
    }
}

pub const EXTERNAL_PLUGIN_PRESET_ID: &str = "external-plugin";

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

/// Metadata about a discovered external plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Discovery/loadability status from the scanner or descriptor source.
    #[serde(default)]
    pub scan_status: PluginScanStatus,
}

/// Stable placeholder schema for saving/restoring external plugin state.
///
/// Native CLAP/VST3/AU loaders can later fill `opaque_state` with the format's
/// binary state blob. Until then, descriptor and sandbox metadata still round-trip
/// through presets/projects without pretending the native state was loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalPluginState {
    pub schema_version: u32,
    pub descriptor: PluginDescriptor,
    pub format: PluginFormat,
    pub plugin_id: String,
    pub plugin_path: PathBuf,
    pub sandbox_mode: ExternalPluginSandboxMode,
    pub opaque_state: Vec<u8>,
}

impl ExternalPluginState {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn new(
        descriptor: PluginDescriptor,
        sandbox_mode: ExternalPluginSandboxMode,
        opaque_state: Vec<u8>,
    ) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            format: descriptor.format,
            plugin_id: descriptor.id.clone(),
            plugin_path: descriptor.path.clone(),
            descriptor,
            sandbox_mode,
            opaque_state,
        }
    }

    pub fn validate_descriptor_consistency(&self) -> Result<(), String> {
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(format!(
                "Unsupported external plugin state schema version {}",
                self.schema_version
            ));
        }
        if self.format != self.descriptor.format
            || self.plugin_id != self.descriptor.id
            || self.plugin_path != self.descriptor.path
        {
            return Err("External plugin state descriptor fields are inconsistent".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPluginSandboxMode {
    #[default]
    InProcess,
    Isolated,
    Disabled,
}

/// Discovers installed plugins on the system.
pub struct PluginScanner {
    /// Discovered plugins
    pub plugins: Vec<PluginDescriptor>,
    seen_paths: HashSet<PathBuf>,
    scan_status_mode: PluginScanStatusMode,
}

impl Default for PluginScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginScanner {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            seen_paths: HashSet::new(),
            scan_status_mode: PluginScanStatusMode::default(),
        }
    }

    pub fn with_scan_status_mode(scan_status_mode: PluginScanStatusMode) -> Self {
        Self {
            plugins: Vec::new(),
            seen_paths: HashSet::new(),
            scan_status_mode,
        }
    }

    pub fn set_scan_status_mode(&mut self, scan_status_mode: PluginScanStatusMode) {
        self.scan_status_mode = scan_status_mode;
    }

    pub fn scan_status_mode(&self) -> PluginScanStatusMode {
        self.scan_status_mode
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
        self.scan_directory_recursive(dir, format);
    }

    fn scan_directory_recursive(&mut self, dir: &Path, format: PluginFormat) {
        let entries = match fs::read_dir(dir) {
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

            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    log::warn!(
                        "external_plugin: cannot read type of {}: {e}",
                        path.display()
                    );
                    continue;
                }
            };

            if file_type.is_dir() {
                if Self::matches_extension(&path, format) {
                    self.add_plugin(path);
                } else {
                    self.scan_directory_recursive(&path, format);
                }
                continue;
            }

            if file_type.is_file() && Self::matches_extension(&path, format) {
                self.add_plugin(path);
            }
        }
    }

    fn matches_extension(path: &Path, format: PluginFormat) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case(format.extension()))
    }

    fn add_plugin(&mut self, path: PathBuf) {
        let path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return,
        };

        if !self.seen_paths.insert(path.clone()) {
            return;
        }

        let name = path
            .file_stem()
            .or_else(|| path.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let format = self.detect_format(&path);
        self.plugins.push(PluginDescriptor {
            id: format!("{}.{}", format.extension(), name),
            name,
            vendor: "Unknown".into(),
            version: "Unknown".into(),
            format,
            path,
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: self.scan_status_for_format(format),
        });
    }

    fn scan_status_for_format(&self, format: PluginFormat) -> PluginScanStatus {
        match self.scan_status_mode {
            PluginScanStatusMode::DiscoveryOnly => PluginScanStatus::Discovered,
            PluginScanStatusMode::BuildCapability => format.build_scan_status(),
        }
    }

    fn detect_format(&self, path: &Path) -> PluginFormat {
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("clap") {
            PluginFormat::Clap
        } else if ext.eq_ignore_ascii_case("vst3") {
            PluginFormat::Vst3
        } else {
            PluginFormat::AudioUnit
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

    pub fn summary(&self) -> PluginScanSummary {
        let mut summary = PluginScanSummary::default();
        for plugin in &self.plugins {
            summary.record(plugin.scan_status);
        }
        summary
    }
}

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

impl PluginDescriptor {
    fn validate(&self) -> Result<(), String> {
        if !self.path.exists() {
            return Err(format!(
                "plugin path does not exist: {}",
                self.path.display()
            ));
        }

        if self.audio_outputs == 0 {
            return Err(format!(
                "plugin {} has zero output channels",
                self.path.display()
            ));
        }

        let expected_ext = self.format.extension();
        if self
            .path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_none_or(|ext| !ext.eq_ignore_ascii_case(expected_ext))
        {
            return Err(format!(
                "path {} does not match plugin format {:?}",
                self.path.display(),
                self.format
            ));
        }

        Ok(())
    }
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

pub fn plan_external_plugin_hosting(descriptor: &PluginDescriptor) -> ExternalPluginHostingPlan {
    let backend = select_hosting_backend(descriptor.format);
    let feature = format_feature(descriptor.format).to_string();
    let native_backend_available = backend != ExternalHostingBackend::Passthrough;
    let scan_status = descriptor.format.build_scan_status();
    let reason = if native_backend_available {
        None
    } else {
        Some(format!(
            "{} native hosting feature '{}' is disabled; '{}' will run as deterministic passthrough",
            format_label(descriptor.format),
            feature,
            descriptor.name
        ))
    };

    ExternalPluginHostingPlan {
        format: descriptor.format,
        feature,
        scan_status,
        backend,
        native_backend_available,
        reason,
    }
}

pub struct ExternalPlugin {
    descriptor: PluginDescriptor,
    input_channels: usize,
    output_channels: usize,
    _sample_rate: u32,
    parameters: Vec<Parameter>,
    hosting_backend: ExternalHostingBackend,
    restore_error: Option<String>,
    /// Format-specific plugin instance (opaque)
    _instance: Option<Box<dyn Any + Send>>,
}

impl ExternalPlugin {
    /// Create a new external plugin wrapper from a descriptor.
    ///
    /// Native backend selection is feature-gated by format:
    /// - CLAP: `external-plugin-clap`
    /// - VST3: `external-plugin-vst3`
    /// - AU: `external-plugin-au`
    ///
    /// If a backend is unavailable at compile-time, we fall back to
    /// deterministic passthrough behavior.
    pub fn new(descriptor: &PluginDescriptor, sample_rate: u32) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("sample rate must be positive".into());
        }

        descriptor.validate()?;
        let hosting_plan = plan_external_plugin_hosting(descriptor);
        let instance = try_load_dynamic_backend(descriptor, hosting_plan.backend)?;

        Ok(Self {
            descriptor: descriptor.clone(),
            input_channels: descriptor.audio_inputs,
            output_channels: descriptor.audio_outputs.max(1),
            _sample_rate: sample_rate,
            parameters: Vec::new(),
            hosting_backend: hosting_plan.backend,
            restore_error: None,
            _instance: instance,
        })
    }

    /// Get the plugin descriptor.
    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    pub fn hosting_backend(&self) -> ExternalHostingBackend {
        self.hosting_backend
    }

    pub fn hosting_plan(&self) -> ExternalPluginHostingPlan {
        plan_external_plugin_hosting(&self.descriptor)
    }

    pub fn restore_error(&self) -> Option<&str> {
        self.restore_error.as_deref()
    }

    /// Serialize descriptor and placeholder state for project/preset storage.
    pub fn placeholder_state(&self) -> ExternalPluginState {
        ExternalPluginState::new(
            self.descriptor.clone(),
            ExternalPluginSandboxMode::InProcess,
            Vec::new(),
        )
    }

    /// Recreate an external plugin wrapper from a serialized placeholder state.
    pub fn from_placeholder_state(
        state: &ExternalPluginState,
        sample_rate: u32,
    ) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("sample rate must be positive".into());
        }
        state.validate_descriptor_consistency()?;
        match Self::new(&state.descriptor, sample_rate) {
            Ok(plugin) => Ok(plugin),
            Err(err) => Ok(Self::unavailable_placeholder(
                state.descriptor.clone(),
                sample_rate,
                err,
            )),
        }
    }

    fn unavailable_placeholder(
        descriptor: PluginDescriptor,
        sample_rate: u32,
        restore_error: String,
    ) -> Self {
        Self {
            input_channels: descriptor.audio_inputs,
            output_channels: descriptor.audio_outputs.max(1),
            descriptor,
            _sample_rate: sample_rate,
            parameters: Vec::new(),
            hosting_backend: ExternalHostingBackend::Passthrough,
            restore_error: Some(restore_error),
            _instance: None,
        }
    }

    pub fn to_placeholder_preset(
        &self,
        name: impl Into<String>,
    ) -> Result<PluginPreset, PluginError> {
        let mut preset = PluginPreset::new(
            name.into(),
            EXTERNAL_PLUGIN_PRESET_ID.to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );
        preset.set_external_plugin_state(&self.placeholder_state())?;
        Ok(preset)
    }

    fn expected_input_len(&self, ctx: &ProcessContext) -> usize {
        ctx.num_frames.saturating_mul(self.input_channels)
    }

    fn expected_output_len(&self, ctx: &ProcessContext) -> usize {
        ctx.num_frames.saturating_mul(self.output_channels)
    }

    fn process_passthrough(
        &self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> usize {
        for sample in output.iter_mut().take(self.expected_output_len(ctx)) {
            *sample = 0.0;
        }

        if self.input_channels == 0 {
            return ctx.num_frames;
        }

        let copy_channels = self.output_channels.min(self.input_channels);
        for frame in 0..ctx.num_frames {
            let src_base = frame.saturating_mul(self.input_channels);
            let dst_base = frame.saturating_mul(self.output_channels);
            output[dst_base..dst_base + copy_channels]
                .copy_from_slice(&input[src_base..src_base + copy_channels]);
        }
        ctx.num_frames
    }

    fn process_clap(&self, input: &[f32], output: &mut [f32], ctx: &ProcessContext) -> usize {
        self.process_passthrough(input, output, ctx)
    }

    fn process_vst3(&self, input: &[f32], output: &mut [f32], ctx: &ProcessContext) -> usize {
        self.process_passthrough(input, output, ctx)
    }

    fn process_audio_unit(&self, input: &[f32], output: &mut [f32], ctx: &ProcessContext) -> usize {
        self.process_passthrough(input, output, ctx)
    }
}

impl Plugin for ExternalPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(
            &self.descriptor.name,
            &self.descriptor.version,
            &self.descriptor.vendor,
        )
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

    fn set_parameter(&mut self, id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        if self.parameters.iter().any(|p| p.id == id) {
            return Ok(());
        }
        Err(format!(
            "parameter '{id}' is not exposed by external plugin '{}'",
            self.descriptor.name
        ))
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> PluginResult<usize> {
        let expected_input = self.expected_input_len(ctx);
        let expected_output = self.expected_output_len(ctx);

        if input.len() < expected_input {
            return Err(format!(
                "external plugin '{}' received {} input samples but expected {expected_input} ({} channels x {} frames)",
                self.descriptor.name,
                input.len(),
                self.input_channels,
                ctx.num_frames
            ));
        }
        if output.len() < expected_output {
            return Err(format!(
                "external plugin '{}' received {} output samples but expected {expected_output} ({} channels x {} frames)",
                self.descriptor.name,
                output.len(),
                self.output_channels,
                ctx.num_frames
            ));
        }

        let frames = match self.hosting_backend {
            ExternalHostingBackend::Passthrough => self.process_passthrough(input, output, ctx),
            ExternalHostingBackend::Clap => self.process_clap(input, output, ctx),
            ExternalHostingBackend::Vst3 => self.process_vst3(input, output, ctx),
            ExternalHostingBackend::AudioUnit => self.process_audio_unit(input, output, ctx),
        };
        Ok(frames)
    }
}

impl SerializablePlugin for ExternalPlugin {
    fn serialize(&self) -> Result<PluginPreset, PluginError> {
        self.to_placeholder_preset(self.descriptor.name.clone())
    }

    fn deserialize(&mut self, preset: &PluginPreset) -> Result<(), PluginError> {
        if !preset.is_compatible(EXTERNAL_PLUGIN_PRESET_ID) {
            return Err(PluginError::InvalidConfiguration(format!(
                "external plugin preset expected plugin_id '{}', got '{}'",
                EXTERNAL_PLUGIN_PRESET_ID, preset.plugin_id
            )));
        }

        self.parameters_from_map(&preset.parameters)?;

        let state = preset.external_plugin_state()?.ok_or_else(|| {
            PluginError::InvalidConfiguration(
                "external plugin preset is missing external plugin state".to_string(),
            )
        })?;
        if state.format != self.descriptor.format
            || state.plugin_id != self.descriptor.id
            || state.plugin_path != self.descriptor.path
        {
            return Err(PluginError::InvalidConfiguration(format!(
                "external plugin preset targets '{}' at {}, not '{}' at {}",
                state.plugin_id,
                state.plugin_path.display(),
                self.descriptor.id,
                self.descriptor.path.display()
            )));
        }

        Ok(())
    }

    fn parameters_to_map(&self) -> HashMap<String, ParameterValue> {
        HashMap::new()
    }

    fn parameters_from_map(
        &mut self,
        params: &HashMap<String, ParameterValue>,
    ) -> Result<(), PluginError> {
        if params.is_empty() {
            Ok(())
        } else {
            Err(PluginError::InvalidConfiguration(
                "external plugin placeholder presets do not store host-side parameters".to_string(),
            ))
        }
    }
}

fn select_hosting_backend(format: PluginFormat) -> ExternalHostingBackend {
    match format {
        PluginFormat::Clap => {
            if cfg!(feature = "external-plugin-clap") {
                ExternalHostingBackend::Clap
            } else {
                ExternalHostingBackend::Passthrough
            }
        }
        PluginFormat::Vst3 => {
            if cfg!(feature = "external-plugin-vst3") {
                ExternalHostingBackend::Vst3
            } else {
                ExternalHostingBackend::Passthrough
            }
        }
        PluginFormat::AudioUnit => {
            if cfg!(feature = "external-plugin-au") {
                ExternalHostingBackend::AudioUnit
            } else {
                ExternalHostingBackend::Passthrough
            }
        }
    }
}

fn format_feature(format: PluginFormat) -> &'static str {
    match format {
        PluginFormat::Clap => "external-plugin-clap",
        PluginFormat::Vst3 => "external-plugin-vst3",
        PluginFormat::AudioUnit => "external-plugin-au",
    }
}

fn format_label(format: PluginFormat) -> &'static str {
    match format {
        PluginFormat::Clap => "CLAP",
        PluginFormat::Vst3 => "VST3",
        PluginFormat::AudioUnit => "AudioUnit",
    }
}

#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
#[derive(Debug)]
struct LoadedDynamicLibrary {
    #[allow(dead_code)]
    path: PathBuf,
    #[allow(dead_code)]
    library: Library,
}

fn try_load_dynamic_backend(
    descriptor: &PluginDescriptor,
    backend: ExternalHostingBackend,
) -> Result<Option<Box<dyn Any + Send>>, String> {
    match backend {
        ExternalHostingBackend::Passthrough => Ok(None),
        ExternalHostingBackend::Clap => load_clap_backend(descriptor).map(Some),
        ExternalHostingBackend::Vst3 => load_vst3_backend(descriptor).map(Some),
        ExternalHostingBackend::AudioUnit => load_audio_unit_backend(descriptor).map(Some),
    }
}

#[cfg(feature = "external-plugin-clap")]
fn load_clap_backend(descriptor: &PluginDescriptor) -> Result<Box<dyn Any + Send>, String> {
    load_dynamic_library_with_symbols(descriptor, &[b"clap_entry\0"], "CLAP")
}

#[cfg(not(feature = "external-plugin-clap"))]
fn load_clap_backend(_descriptor: &PluginDescriptor) -> Result<Box<dyn Any + Send>, String> {
    Err("CLAP backend feature is disabled".to_string())
}

#[cfg(feature = "external-plugin-vst3")]
fn load_vst3_backend(descriptor: &PluginDescriptor) -> Result<Box<dyn Any + Send>, String> {
    load_dynamic_library_with_symbols(descriptor, &[b"GetPluginFactory\0"], "VST3")
}

#[cfg(not(feature = "external-plugin-vst3"))]
fn load_vst3_backend(_descriptor: &PluginDescriptor) -> Result<Box<dyn Any + Send>, String> {
    Err("VST3 backend feature is disabled".to_string())
}

#[cfg(feature = "external-plugin-au")]
fn load_audio_unit_backend(descriptor: &PluginDescriptor) -> Result<Box<dyn Any + Send>, String> {
    load_dynamic_library_with_symbols(
        descriptor,
        &[b"AudioComponentFactoryFunction\0"],
        "AudioUnit",
    )
}

#[cfg(not(feature = "external-plugin-au"))]
fn load_audio_unit_backend(_descriptor: &PluginDescriptor) -> Result<Box<dyn Any + Send>, String> {
    Err("AudioUnit backend feature is disabled".to_string())
}

#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
fn load_dynamic_library_with_symbols(
    descriptor: &PluginDescriptor,
    symbols: &[&[u8]],
    format_name: &str,
) -> Result<Box<dyn Any + Send>, String> {
    let library_path = resolve_dynamic_library_path(descriptor)?;
    let library = unsafe { Library::new(&library_path) }.map_err(|err| {
        format!(
            "failed to load {format_name} plugin library '{}': {err}",
            library_path.display()
        )
    })?;

    for symbol in symbols {
        unsafe {
            library.get::<*const c_void>(symbol).map_err(|err| {
                format!(
                    "{format_name} plugin '{}' is missing required symbol '{}': {err}",
                    library_path.display(),
                    String::from_utf8_lossy(&symbol[..symbol.len().saturating_sub(1)])
                )
            })?
        };
    }

    Ok(Box::new(LoadedDynamicLibrary {
        path: library_path,
        library,
    }))
}

#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
fn resolve_dynamic_library_path(descriptor: &PluginDescriptor) -> Result<PathBuf, String> {
    if descriptor.path.is_file() {
        return Ok(descriptor.path.clone());
    }
    if !descriptor.path.is_dir() {
        return Err(format!(
            "plugin path '{}' is neither file nor directory",
            descriptor.path.display()
        ));
    }

    let file_stem = descriptor
        .path
        .file_stem()
        .or_else(|| descriptor.path.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut candidates = Vec::new();

    if !file_stem.is_empty() {
        for extension in dynamic_library_extensions() {
            candidates.push(descriptor.path.join(format!("{file_stem}.{extension}")));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let macos_dir = descriptor.path.join("Contents").join("MacOS");
        if !file_stem.is_empty() {
            candidates.push(macos_dir.join(&file_stem));
            for extension in dynamic_library_extensions() {
                candidates.push(macos_dir.join(format!("{file_stem}.{extension}")));
            }
        }
    }

    if let Some(candidate) = candidates.into_iter().find(|p| p.is_file()) {
        return Ok(candidate);
    }

    find_dynamic_library_in_dir(&descriptor.path, 4).ok_or_else(|| {
        format!(
            "could not locate dynamic library inside plugin bundle '{}'",
            descriptor.path.display()
        )
    })
}

#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
fn dynamic_library_extensions() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        return &["dll"];
    }
    #[cfg(target_os = "macos")]
    {
        return &["dylib", "bundle"];
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return &["so"];
    }
    #[allow(unreachable_code)]
    &[]
}

#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
fn find_dynamic_library_in_dir(root: &Path, max_depth: usize) -> Option<PathBuf> {
    fn recurse(path: &Path, depth: usize, max_depth: usize) -> Option<PathBuf> {
        if depth > max_depth {
            return None;
        }
        let entries = fs::read_dir(path).ok()?;
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if let Some(found) = recurse(&entry_path, depth + 1, max_depth) {
                    return Some(found);
                }
                continue;
            }
            if entry_path.is_file() {
                let ext = entry_path.extension().and_then(|s| s.to_str());
                if ext.is_some_and(|ext| {
                    dynamic_library_extensions()
                        .iter()
                        .any(|candidate| ext.eq_ignore_ascii_case(candidate))
                }) {
                    return Some(entry_path);
                }
            }
        }
        None
    }

    recurse(root, 0, max_depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        let mut tmp_path = env::temp_dir();
        tmp_path.push(format!(
            "sotf-external-plugin-fake-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&tmp_path).unwrap();
        let plugin_path = tmp_path.join("fake.clap");
        fs::write(&plugin_path, b"stub plugin").unwrap();

        let desc = PluginDescriptor {
            id: "test.plugin".into(),
            name: "Test Plugin".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: plugin_path.clone(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec![],
            scan_status: PluginScanStatus::Discovered,
        };

        let mut plugin = ExternalPlugin::new(&desc, 48000).unwrap();
        let input = vec![0.5f32; 2048];
        let mut output = vec![0.0f32; 2048];
        let ctx = ProcessContext::new(48000, 1024);

        let frames = plugin.process(&input, &mut output, &ctx).unwrap();
        assert_eq!(frames, 1024);
        // Passthrough: first matching channels should match input
        for i in 0..2048 {
            assert!((output[i] - input[i]).abs() < 1e-6);
        }

        fs::remove_file(plugin_path).unwrap();
        fs::remove_dir_all(tmp_path).unwrap();
    }

    #[test]
    fn test_external_plugin_scan_recursive_and_dedup() {
        let root = env::temp_dir().join(format!(
            "sotf-external-plugin-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let nested = root.join("nested");
        let plugin_file = nested.join("my-plugin.clap");

        fs::create_dir_all(&nested).unwrap();
        fs::write(&plugin_file, b"stub").unwrap();

        let mut scanner = PluginScanner::new();
        scanner.scan_directory(&root, PluginFormat::Clap);
        assert_eq!(scanner.plugins.len(), 1);
        assert_eq!(scanner.plugins[0].name, "my-plugin");
        assert_eq!(
            scanner.plugins[0].scan_status,
            PluginFormat::Clap.build_scan_status()
        );
        scanner.scan_directory(&root, PluginFormat::Clap);
        assert_eq!(scanner.plugins.len(), 1);

        fs::remove_file(&plugin_file).unwrap();
        fs::remove_dir_all(&nested).unwrap();
        fs::remove_dir_all(&root).unwrap_or_else(|_| ());
    }

    #[test]
    fn test_external_plugin_scanner_can_preserve_discovered_status() {
        let root = env::temp_dir().join(format!(
            "sotf-external-plugin-discovered-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let plugin_file = root.join("raw-discovery.clap");

        fs::create_dir_all(&root).unwrap();
        fs::write(&plugin_file, b"stub").unwrap();

        let mut scanner = PluginScanner::with_scan_status_mode(PluginScanStatusMode::DiscoveryOnly);
        scanner.scan_directory(&root, PluginFormat::Clap);

        assert_eq!(scanner.plugins.len(), 1);
        assert_eq!(scanner.plugins[0].scan_status, PluginScanStatus::Discovered);

        fs::remove_file(&plugin_file).unwrap();
        fs::remove_dir_all(&root).unwrap_or_else(|_| ());
    }

    #[test]
    fn test_external_plugin_scan_summary_counts_statuses() {
        let descriptor = |id: &str, status: PluginScanStatus| PluginDescriptor {
            id: id.into(),
            name: id.into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: PathBuf::from(format!("/tmp/{id}.clap")),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec![],
            scan_status: status,
        };
        let mut scanner = PluginScanner::new();
        scanner.plugins.push(descriptor(
            "discovered.plugin",
            PluginScanStatus::Discovered,
        ));
        scanner
            .plugins
            .push(descriptor("loadable.plugin", PluginScanStatus::Loadable));
        scanner.plugins.push(descriptor(
            "unsupported.plugin",
            PluginScanStatus::UnsupportedByBuild,
        ));

        let summary = scanner.summary();

        assert_eq!(
            summary,
            PluginScanSummary {
                total: 3,
                discovered: 1,
                loadable: 1,
                unsupported_by_build: 1,
            }
        );
    }

    #[test]
    fn test_external_plugin_capability_matrix_reports_build_support() {
        let matrix = plugin_format_capabilities();
        assert_eq!(matrix.len(), 3);
        let clap = matrix
            .iter()
            .find(|capability| capability.format == PluginFormat::Clap)
            .unwrap();
        assert_eq!(clap.feature, "external-plugin-clap");
        assert_eq!(clap.scan_status, PluginFormat::Clap.build_scan_status());
        assert_eq!(clap.backend, select_hosting_backend(PluginFormat::Clap));
        assert_eq!(
            clap.native_backend_available,
            clap.backend != ExternalHostingBackend::Passthrough
        );
        if clap.native_backend_available {
            assert_eq!(clap.reason, None);
        } else {
            assert!(
                clap.reason
                    .as_deref()
                    .unwrap()
                    .contains("unsupported-by-build")
            );
        }
    }

    #[test]
    fn test_external_plugin_hosting_plan_reports_feature_gate() {
        let desc = PluginDescriptor {
            id: "planned.plugin".into(),
            name: "Planned Plugin".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: PathBuf::from("/tmp/planned-plugin.clap"),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec![],
            scan_status: PluginScanStatus::Discovered,
        };

        let plan = plan_external_plugin_hosting(&desc);

        assert_eq!(plan.format, PluginFormat::Clap);
        assert_eq!(plan.feature, "external-plugin-clap");
        assert_eq!(plan.scan_status, PluginFormat::Clap.build_scan_status());
        assert_eq!(plan.backend, select_hosting_backend(PluginFormat::Clap));
        if plan.backend == ExternalHostingBackend::Passthrough {
            assert!(!plan.native_backend_available);
            assert!(
                plan.reason
                    .as_deref()
                    .unwrap()
                    .contains("deterministic passthrough")
            );
        } else {
            assert!(plan.native_backend_available);
            assert_eq!(plan.reason, None);
        }
    }

    #[test]
    fn test_external_plugin_set_parameter_unknown() {
        let mut tmp_path = env::temp_dir();
        tmp_path.push(format!(
            "sotf-external-plugin-setparam-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&tmp_path).unwrap();
        let plugin_path = tmp_path.join("fake.clap");
        fs::write(&plugin_path, b"stub plugin").unwrap();
        let desc = PluginDescriptor {
            id: "test.plugin".into(),
            name: "Test Plugin".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: plugin_path.clone(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec![],
            scan_status: PluginScanStatus::Discovered,
        };

        let mut plugin = ExternalPlugin::new(&desc, 48_000).unwrap();
        let result = plugin.set_parameter(ParameterId::from("unknown"), ParameterValue::Float(1.0));
        assert!(result.is_err());

        fs::remove_file(plugin_path).unwrap();
        fs::remove_dir_all(tmp_path).unwrap();
    }

    #[test]
    fn test_external_plugin_placeholder_state_round_trips() {
        let tmp_path = env::temp_dir().join(format!(
            "sotf-external-plugin-state-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&tmp_path).unwrap();
        let plugin_path = tmp_path.join("state-test.clap");
        fs::write(&plugin_path, b"stub plugin").unwrap();
        let desc = PluginDescriptor {
            id: "test.state".into(),
            name: "State Test".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: plugin_path.clone(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec!["state".into()],
            scan_status: PluginScanStatus::Discovered,
        };
        let plugin = ExternalPlugin::new(&desc, 48_000).unwrap();
        let mut state = plugin.placeholder_state();
        state.sandbox_mode = ExternalPluginSandboxMode::Isolated;
        state.opaque_state = vec![1, 2, 3, 4];

        let json = serde_json::to_string(&state).unwrap();
        let decoded: ExternalPluginState = serde_json::from_str(&json).unwrap();
        let restored = ExternalPlugin::from_placeholder_state(&decoded, 48_000).unwrap();

        assert_eq!(decoded, state);
        assert_eq!(restored.descriptor(), &desc);
        assert_eq!(decoded.sandbox_mode, ExternalPluginSandboxMode::Isolated);
        assert_eq!(decoded.opaque_state, vec![1, 2, 3, 4]);

        fs::remove_file(plugin_path).unwrap();
        fs::remove_dir_all(tmp_path).unwrap();
    }

    #[test]
    fn test_external_plugin_placeholder_state_restores_missing_plugin_as_unavailable() {
        let missing_path = env::temp_dir().join(format!(
            "sotf-external-plugin-missing-{}.clap",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let desc = PluginDescriptor {
            id: "test.missing".into(),
            name: "Missing Test".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: missing_path,
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec!["state".into()],
            scan_status: PluginScanStatus::Discovered,
        };
        let state = ExternalPluginState::new(
            desc.clone(),
            ExternalPluginSandboxMode::InProcess,
            vec![1, 2, 3],
        );

        let mut restored = ExternalPlugin::from_placeholder_state(&state, 48_000).unwrap();

        assert_eq!(restored.descriptor(), &desc);
        assert_eq!(
            restored.hosting_backend(),
            ExternalHostingBackend::Passthrough
        );
        assert!(
            restored
                .restore_error()
                .unwrap()
                .contains("plugin path does not exist")
        );

        let input = vec![0.25, -0.5, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        let frames = restored
            .process(&input, &mut output, &ProcessContext::new(48_000, 2))
            .unwrap();
        assert_eq!(frames, 2);
        assert_eq!(output, input);
    }

    #[test]
    fn test_external_plugin_serializable_preset_round_trips_placeholder_state() {
        let tmp_path = env::temp_dir().join(format!(
            "sotf-external-plugin-serializable-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&tmp_path).unwrap();
        let plugin_path = tmp_path.join("serializable.clap");
        fs::write(&plugin_path, b"stub plugin").unwrap();
        let desc = PluginDescriptor {
            id: "test.serializable".into(),
            name: "Serializable Test".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: plugin_path.clone(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec!["state".into()],
            scan_status: PluginScanStatus::Discovered,
        };
        let mut plugin = ExternalPlugin::new(&desc, 48_000).unwrap();

        let preset = SerializablePlugin::serialize(&plugin).unwrap();
        let restored_state = preset.external_plugin_state().unwrap().unwrap();

        assert_eq!(preset.plugin_id, EXTERNAL_PLUGIN_PRESET_ID);
        assert_eq!(restored_state.descriptor, desc);
        assert_eq!(
            restored_state.sandbox_mode,
            ExternalPluginSandboxMode::InProcess
        );
        assert!(restored_state.opaque_state.is_empty());
        SerializablePlugin::deserialize(&mut plugin, &preset).unwrap();

        fs::remove_file(plugin_path).unwrap();
        fs::remove_dir_all(tmp_path).unwrap();
    }

    #[test]
    fn test_external_plugin_deserialize_rejects_different_descriptor() {
        let tmp_path = env::temp_dir().join(format!(
            "sotf-external-plugin-mismatch-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&tmp_path).unwrap();
        let plugin_path = tmp_path.join("mismatch.clap");
        fs::write(&plugin_path, b"stub plugin").unwrap();
        let desc = PluginDescriptor {
            id: "test.mismatch".into(),
            name: "Mismatch Test".into(),
            vendor: "Test".into(),
            version: "1.0".into(),
            format: PluginFormat::Clap,
            path: plugin_path.clone(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: vec![],
            scan_status: PluginScanStatus::Discovered,
        };
        let mut plugin = ExternalPlugin::new(&desc, 48_000).unwrap();
        let mut state = plugin.placeholder_state();
        state.plugin_id = "other.plugin".into();
        state.descriptor.id = "other.plugin".into();

        let mut preset = PluginPreset::new(
            "Other".into(),
            EXTERNAL_PLUGIN_PRESET_ID.into(),
            env!("CARGO_PKG_VERSION").into(),
        );
        preset.set_external_plugin_state(&state).unwrap();

        assert!(matches!(
            SerializablePlugin::deserialize(&mut plugin, &preset),
            Err(PluginError::InvalidConfiguration(_))
        ));

        fs::remove_file(plugin_path).unwrap();
        fs::remove_dir_all(tmp_path).unwrap();
    }

    #[test]
    fn test_plugin_format_extension() {
        assert_eq!(PluginFormat::Clap.extension(), "clap");
        assert_eq!(PluginFormat::Vst3.extension(), "vst3");
        assert_eq!(PluginFormat::AudioUnit.extension(), "component");
    }

    #[test]
    fn test_external_plugin_backend_selection_is_feature_gated() {
        assert_eq!(
            select_hosting_backend(PluginFormat::Clap),
            if cfg!(feature = "external-plugin-clap") {
                ExternalHostingBackend::Clap
            } else {
                ExternalHostingBackend::Passthrough
            }
        );
        assert_eq!(
            select_hosting_backend(PluginFormat::Vst3),
            if cfg!(feature = "external-plugin-vst3") {
                ExternalHostingBackend::Vst3
            } else {
                ExternalHostingBackend::Passthrough
            }
        );
        assert_eq!(
            select_hosting_backend(PluginFormat::AudioUnit),
            if cfg!(feature = "external-plugin-au") {
                ExternalHostingBackend::AudioUnit
            } else {
                ExternalHostingBackend::Passthrough
            }
        );
    }
}
