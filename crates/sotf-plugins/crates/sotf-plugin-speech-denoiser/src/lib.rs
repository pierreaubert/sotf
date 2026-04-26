pub mod params;

use crate::params::PARAMS as SP;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_plugin_denoiser::{DenoiserPlugin, DenoiserPluginParams};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechDenoiserPluginParams {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for SpeechDenoiserPluginParams {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
        }
    }
}

pub struct SpeechDenoiserPlugin {
    channels: usize,
    enabled: bool,
    inner: DenoiserPlugin,
    cached_parameters: Vec<Parameter>,
}

impl SpeechDenoiserPlugin {
    pub fn new(channels: usize) -> Self {
        Self::from_params(channels, SpeechDenoiserPluginParams::default())
    }

    pub fn from_params(channels: usize, params: SpeechDenoiserPluginParams) -> Self {
        let dn = DenoiserPluginParams {
            algorithm: 1,
            transient_enabled: false,
            hiss_enabled: false,
            ..DenoiserPluginParams::default()
        };
        let mut plugin = Self {
            channels,
            enabled: params.enabled,
            inner: DenoiserPlugin::from_params(channels, dn),
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(SP, |i| self.param_value(i));
    }
}

impl InPlacePlugin for SpeechDenoiserPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Speech Denoiser", "1.0.0", "SotF")
            .with_description("RNNoise speech denoiser")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        param_bridge::set_parameter(SP, &id, &value, |i, v| {
            if i == 0 {
                self.enabled = v > 0.5;
            }
        })?;
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(SP, id, |i| self.param_value(i))
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.inner.initialize(sample_rate)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        if self.enabled {
            self.inner.process_in_place(buffer, context)
        } else {
            Ok(context.num_frames)
        }
    }

    fn latency_samples(&self) -> usize {
        if self.enabled {
            self.inner.latency_samples()
        } else {
            0
        }
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.inner.get_data()
    }
}
