pub mod params;

use crate::params::PARAMS as HP;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_plugin_denoiser::{DenoiserPlugin, DenoiserPluginParams};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HissReducerPluginParams {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "d_frequency_hz")]
    pub frequency_hz: f32,
    #[serde(default = "d_strength")]
    pub strength: f32,
    #[serde(default = "d_low_latency")]
    pub low_latency: bool,
}

fn d_enabled() -> bool {
    pk(HP, "enabled").default_bool()
}
fn d_threshold_db() -> f32 {
    pk(HP, "threshold_db").default_f32()
}
fn d_frequency_hz() -> f32 {
    pk(HP, "frequency_hz").default_f32()
}
fn d_strength() -> f32 {
    pk(HP, "strength").default_f32()
}
fn d_low_latency() -> bool {
    pk(HP, "low_latency").default_bool()
}

impl Default for HissReducerPluginParams {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            threshold_db: d_threshold_db(),
            frequency_hz: d_frequency_hz(),
            strength: d_strength(),
            low_latency: d_low_latency(),
        }
    }
}

pub struct HissReducerPlugin {
    channels: usize,
    sample_rate: u32,
    initialized: bool,
    params: HissReducerPluginParams,
    inner: DenoiserPlugin,
    cached_parameters: Vec<Parameter>,
}

impl HissReducerPlugin {
    pub fn new(channels: usize) -> Self {
        Self::from_params(channels, HissReducerPluginParams::default())
    }

    pub fn from_params(channels: usize, params: HissReducerPluginParams) -> Self {
        let mut plugin = Self {
            channels,
            sample_rate: 44100,
            initialized: false,
            inner: DenoiserPlugin::from_params(channels, Self::inner_params(&params)),
            params,
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    fn inner_params(params: &HissReducerPluginParams) -> DenoiserPluginParams {
        DenoiserPluginParams {
            reduction_db: 0.0,
            transient_enabled: false,
            hiss_enabled: true,
            hiss_threshold_db: params.threshold_db,
            hiss_frequency_hz: params.frequency_hz,
            hiss_strength: params.strength,
            low_latency: params.low_latency,
            algorithm: 0,
            ..DenoiserPluginParams::default()
        }
    }

    fn rebuild_inner(&mut self) -> PluginResult<()> {
        self.inner = DenoiserPlugin::from_params(self.channels, Self::inner_params(&self.params));
        if self.initialized {
            self.inner.initialize(self.sample_rate)?;
        }
        Ok(())
    }

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.params.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.params.threshold_db as f64),
            2 => Some(self.params.frequency_hz as f64),
            3 => Some(self.params.strength as f64),
            4 => Some(if self.params.low_latency { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(HP, |i| self.param_value(i));
    }
}

impl InPlacePlugin for HissReducerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Hiss Reducer", "1.0.0", "SotF")
            .with_description("Stationary high-frequency hiss reducer")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let idx = param_bridge::set_parameter(HP, &id, &value, |i, v| match i {
            0 => self.params.enabled = v > 0.5,
            1 => self.params.threshold_db = v as f32,
            2 => self.params.frequency_hz = v as f32,
            3 => self.params.strength = v as f32,
            4 => self.params.low_latency = v > 0.5,
            _ => {}
        })?;
        if idx != 0 {
            self.rebuild_inner()?;
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(HP, id, |i| self.param_value(i))
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.initialized = true;
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
        if self.params.enabled {
            self.inner.process_in_place(buffer, context)
        } else {
            Ok(context.num_frames)
        }
    }

    fn latency_samples(&self) -> usize {
        if self.params.enabled {
            self.inner.latency_samples()
        } else {
            0
        }
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.inner.get_data()
    }
}
