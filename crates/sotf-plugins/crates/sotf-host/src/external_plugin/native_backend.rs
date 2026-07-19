use crate::parameters::{Parameter, ParameterId, ParameterValue};

#[derive(Debug, Clone)]
pub(super) struct NativePluginMetadata {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) vendor: String,
    pub(super) version: String,
    pub(super) input_channels: usize,
    pub(super) output_channels: usize,
}

/// Format-specific native instance owned by [`super::ExternalPlugin`].
///
/// Implementations must allocate all audio buffers during construction and
/// perform no allocation, locking, logging, or filesystem work in `process`.
pub(super) trait NativeExternalPluginBackend: Send {
    fn metadata(&self) -> &NativePluginMetadata;

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, id: &ParameterId, _value: &ParameterValue) -> Result<(), String> {
        Err(format!("native external parameter '{id}' is not exposed"))
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
        input_channels: usize,
        output_channels: usize,
    ) -> Result<(), String>;

    fn save_state(&self) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }

    fn load_state(&mut self, _state: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn latency_samples(&self) -> usize {
        0
    }
}
