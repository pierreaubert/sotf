pub mod params;

use crate::params::PARAMS as DC;
use plugins_denoiser::transient::TransientSuppressor;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclickPluginParams {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_sensitivity")]
    pub sensitivity: f32,
}

fn d_enabled() -> bool {
    pk(DC, "enabled").default_bool()
}
fn d_sensitivity() -> f32 {
    pk(DC, "sensitivity").default_f32()
}

impl Default for DeclickPluginParams {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            sensitivity: d_sensitivity(),
        }
    }
}

pub struct DeclickPlugin {
    channels: usize,
    enabled: bool,
    sensitivity: f32,
    suppressor: TransientSuppressor,
    cached_parameters: Vec<Parameter>,
}

impl DeclickPlugin {
    pub fn new(channels: usize) -> Self {
        Self::from_params(channels, DeclickPluginParams::default())
    }

    pub fn from_params(channels: usize, params: DeclickPluginParams) -> Self {
        let mut suppressor = TransientSuppressor::new(channels);
        suppressor.set_sensitivity(params.sensitivity);
        let mut plugin = Self {
            channels,
            enabled: params.enabled,
            sensitivity: params.sensitivity,
            suppressor,
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.sensitivity as f64),
            _ => None,
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(DC, |i| self.param_value(i));
    }
}

impl InPlacePlugin for DeclickPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Declick", "1.0.0", "SotF")
            .with_description("Time-domain click and transient repair")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        param_bridge::set_parameter(DC, &id, &value, |i, v| match i {
            0 => self.enabled = v > 0.5,
            1 => {
                self.sensitivity = v as f32;
                self.suppressor.set_sensitivity(self.sensitivity);
            }
            _ => {}
        })?;
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(DC, id, |i| self.param_value(i))
    }

    fn initialize(&mut self, _sample_rate: u32) -> PluginResult<()> {
        Ok(())
    }

    fn reset(&mut self) {
        self.suppressor.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let expected = context
            .num_frames
            .checked_mul(self.channels)
            .ok_or_else(|| "Frame/channel count overflow".to_string())?;
        if buffer.len() != expected {
            return Err(format!(
                "Buffer size mismatch: expected {}, got {}",
                expected,
                buffer.len()
            ));
        }
        if self.enabled {
            self.suppressor.process(buffer);
        }
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }
}
