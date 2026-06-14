use super::types::PluginSandboxBackendCapabilities;

pub trait PluginSandboxBackend {
    fn backend_id(&self) -> &'static str;
    fn capabilities(&self) -> PluginSandboxBackendCapabilities;
}
