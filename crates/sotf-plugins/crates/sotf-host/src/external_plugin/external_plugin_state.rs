use super::plugin_descriptor::PluginDescriptor;
use super::plugin_format::PluginFormat;
use super::types::ExternalPluginSandboxMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
