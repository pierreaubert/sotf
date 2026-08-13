pub mod params;

use crate::params::PARAMS as DC;
use plugins_denoiser::transient::TransientSuppressor;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclickPluginParams {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_sensitivity")]
    pub sensitivity: f32,
    #[serde(default = "d_link_channels")]
    pub link_channels: bool,
}

fn d_enabled() -> bool {
    pk(DC, "enabled").default_bool()
}
fn d_sensitivity() -> f32 {
    pk(DC, "sensitivity").default_f32()
}
fn d_link_channels() -> bool {
    pk(DC, "link_channels").default_bool()
}

impl Default for DeclickPluginParams {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            sensitivity: d_sensitivity(),
            link_channels: d_link_channels(),
        }
    }
}

pub struct DeclickPlugin {
    channels: usize,
    enabled: bool,
    sensitivity: f32,
    link_channels: bool,
    suppressor: TransientSuppressor,
    initialized_sample_rate: u32,
    cached_parameters: Vec<Parameter>,
}

impl DeclickPlugin {
    pub fn new(channels: usize, sample_rate: u32) -> Result<Self, String> {
        Self::from_params(channels, sample_rate, DeclickPluginParams::default())
    }

    pub fn from_params(
        channels: usize,
        sample_rate: u32,
        params: DeclickPluginParams,
    ) -> Result<Self, String> {
        if channels == 0 {
            return Err("declick requires at least one channel".into());
        }
        if sample_rate == 0 {
            return Err("declick sample rate must be greater than zero".into());
        }
        let sensitivity = if params.sensitivity.is_finite() {
            params.sensitivity.clamp(1.0, 100.0)
        } else {
            d_sensitivity()
        };
        let mut suppressor = TransientSuppressor::new(channels, sample_rate)?;
        suppressor.set_sensitivity_immediate(sensitivity);
        suppressor.set_enabled_immediate(params.enabled);
        suppressor.set_link_channels(params.link_channels);
        let mut plugin = Self {
            channels,
            enabled: params.enabled,
            sensitivity,
            link_channels: params.link_channels,
            suppressor,
            initialized_sample_rate: sample_rate,
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.sensitivity as f64),
            2 => Some(if self.link_channels { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(DC, |i| self.param_value(i));
    }

    fn update_cached_values(&mut self) {
        self.cached_parameters[0].default_value = ParameterValue::Bool(self.enabled);
        self.cached_parameters[1].default_value = ParameterValue::Float(self.sensitivity);
        self.cached_parameters[2].default_value = ParameterValue::Bool(self.link_channels);
    }
}

impl ParametricInPlacePlugin for DeclickPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Declick", "1.1.0", "SotF")
            .with_description("Lookahead click detection and robust interpolation")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Dynamics
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(
            PluginCostClass::Dynamics,
            None,
            self.latency_samples(),
            false,
        )
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        values.insert(
            ParameterId::from("enabled"),
            ParameterValue::Bool(self.enabled),
        );
        values.insert(
            ParameterId::from("sensitivity"),
            ParameterValue::Float(self.sensitivity),
        );
        values.insert(
            ParameterId::from("link_channels"),
            ParameterValue::Bool(self.link_channels),
        );
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        // Validate the complete batch before mutating DSP state. The cache is
        // updated in place, so successful automation does not rebuild a Vec.
        let mut enabled = self.enabled;
        let mut sensitivity = self.sensitivity;
        let mut link_channels = self.link_channels;
        for (id, value) in &values {
            param_bridge::set_parameter(DC, id, value, |i, v| match i {
                0 => enabled = v > 0.5,
                1 => sensitivity = v as f32,
                2 => link_channels = v > 0.5,
                _ => {}
            })?;
        }
        if enabled != self.enabled {
            self.enabled = enabled;
            self.suppressor.set_enabled(enabled);
        }
        if sensitivity != self.sensitivity {
            self.sensitivity = sensitivity;
            self.suppressor.set_sensitivity(sensitivity);
        }
        if link_channels != self.link_channels {
            self.link_channels = link_channels;
            self.suppressor.set_link_channels(link_channels);
        }
        self.update_cached_values();
        Ok(())
    }

    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        let mut numeric = 0.0;
        let index = param_bridge::set_parameter(DC, &id, &value, |_, value| numeric = value)?;
        match index {
            0 => {
                self.enabled = numeric > 0.5;
                self.suppressor.set_enabled(self.enabled);
            }
            1 => {
                self.sensitivity = numeric as f32;
                self.suppressor.set_sensitivity(self.sensitivity);
            }
            2 => {
                self.link_channels = numeric > 0.5;
                self.suppressor.set_link_channels(self.link_channels);
            }
            _ => unreachable!("PARAMS index must be handled"),
        }
        self.update_cached_values();
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("declick sample rate must be greater than zero".into());
        }
        self.suppressor.set_sample_rate(sample_rate)?;
        self.initialized_sample_rate = sample_rate;
        self.suppressor.reset();
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
        if self.channels == 0 {
            return Err("declick requires at least one channel".into());
        }
        if context.sample_rate != self.initialized_sample_rate {
            return Err(format!(
                "declick sample-rate mismatch: initialized at {}, context is {}",
                self.initialized_sample_rate, context.sample_rate
            ));
        }
        self.suppressor.process(buffer)?;
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.suppressor.latency_samples()
    }
}
