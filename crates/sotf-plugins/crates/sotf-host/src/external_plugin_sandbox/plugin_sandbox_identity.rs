use super::misc::sanitize_path_component;
use crate::external_plugin::PluginDescriptor;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginSandboxIdentity {
    pub plugin_id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub format: String,
    pub path: PathBuf,
}

impl PluginSandboxIdentity {
    pub fn from_descriptor(descriptor: &PluginDescriptor) -> Self {
        Self {
            plugin_id: descriptor.id.clone(),
            name: descriptor.name.clone(),
            vendor: descriptor.vendor.clone(),
            version: descriptor.version.clone(),
            format: format!("{:?}", descriptor.format),
            path: descriptor.path.clone(),
        }
    }

    pub fn stable_preset_component(&self) -> String {
        sanitize_path_component(&format!(
            "{}-{}-{}",
            self.format, self.vendor, self.plugin_id
        ))
    }
}
