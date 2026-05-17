pub mod params;

use crate::params::PARAMS as HP;
use plugins_denoiser::hiss::HissReducer;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};

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

impl Default for HissReducerPluginParams {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            threshold_db: d_threshold_db(),
            frequency_hz: d_frequency_hz(),
            strength: d_strength(),
        }
    }
}

pub struct HissReducerPlugin {
    channels: usize,
    sample_rate: u32,
    initialized: bool,
    params: HissReducerPluginParams,
    reducer: HissReducer,
    cached_parameters: Vec<Parameter>,
}

impl HissReducerPlugin {
    pub fn new(channels: usize) -> Self {
        Self::from_params(channels, HissReducerPluginParams::default())
    }

    pub fn from_params(channels: usize, params: HissReducerPluginParams) -> Self {
        let mut plugin = Self {
            channels,
            // Match HissReducer::new()'s internal default so the stored
            // sample_rate is consistent with the reducer's initial coefficients
            // before initialize() is called.
            sample_rate: 48000,
            initialized: false,
            reducer: Self::build_reducer(channels, &params),
            params,
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        plugin
    }

    fn build_reducer(channels: usize, params: &HissReducerPluginParams) -> HissReducer {
        let mut reducer = HissReducer::new(channels);
        reducer.set_params(params.frequency_hz, params.threshold_db, params.strength);
        reducer
    }

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.params.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.params.threshold_db as f64),
            2 => Some(self.params.frequency_hz as f64),
            3 => Some(self.params.strength as f64),
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
            _ => {}
        })?;
        // For continuous parameters (threshold, frequency, strength) update the
        // reducer in-place via set_params() to preserve DSP state (IIR history,
        // envelope followers) and avoid audible clicks. Only the enabled flag
        // (idx == 0) needs no reducer update at all.
        if idx != 0 {
            self.reducer.set_params(
                self.params.frequency_hz,
                self.params.threshold_db,
                self.params.strength,
            );
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
        self.reducer.initialize(sample_rate);
        Ok(())
    }

    fn reset(&mut self) {
        self.reducer.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        if self.params.enabled {
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
            self.reducer.process(buffer);
        }
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.reducer.latency_samples()
    }
}
