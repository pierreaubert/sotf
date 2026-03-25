//! VST3/CLAP plugin wrappers for SOTF audio plugins via nih-plug.

pub mod params;
#[macro_use]
pub mod wrapper;

/// Wrapper for ParamBridge (unused fields reserved for future param normalization).
pub struct PluginBridgeWrapper {
    _bridge: plugins_bridge::param_bridge::ParamBridge,
}

impl PluginBridgeWrapper {
    pub fn new(bridge: plugins_bridge::param_bridge::ParamBridge) -> Self {
        Self { _bridge: bridge }
    }

    pub fn sync_params_to_plugin(
        &self,
        params: &std::sync::Arc<params::DynamicParams>,
        plugin: &mut dyn sotf_host::plugin::Plugin,
    ) {
        params.sync_to_plugin(plugin);
    }
}

