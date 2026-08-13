pub mod params;

use crate::params::PARAMS as HP;
use plugins_denoiser::hiss::HissReducer;
use plugins_denoiser::spectral_hiss::SpectralHissReducer;
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
#[serde(deny_unknown_fields)]
pub struct HissReducerPluginParams {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "d_frequency_hz")]
    pub frequency_hz: f32,
    #[serde(default = "d_strength")]
    pub strength: f32,
    #[serde(default = "d_spectral_mode")]
    pub spectral_mode: bool,
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
fn d_spectral_mode() -> bool {
    pk(HP, "spectral_mode").default_bool()
}

impl Default for HissReducerPluginParams {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            threshold_db: d_threshold_db(),
            frequency_hz: d_frequency_hz(),
            strength: d_strength(),
            spectral_mode: d_spectral_mode(),
        }
    }
}

pub struct HissReducerPlugin {
    channels: usize,
    sample_rate: u32,
    initialized: bool,
    params: HissReducerPluginParams,
    reducer: HissReducer,
    spectral_reducer: SpectralHissReducer,
    cached_parameters: Vec<Parameter>,
}

impl HissReducerPlugin {
    pub fn new(channels: usize) -> Self {
        Self::try_new(channels).expect("HissReducerPlugin requires at least one channel")
    }

    pub fn from_params(channels: usize, params: HissReducerPluginParams) -> Self {
        Self::try_from_params(channels, params)
            .expect("HissReducerPlugin requires at least one channel")
    }

    pub fn try_new(channels: usize) -> PluginResult<Self> {
        Self::try_from_params(channels, HissReducerPluginParams::default())
    }

    pub fn try_from_params(channels: usize, params: HissReducerPluginParams) -> PluginResult<Self> {
        Self::try_from_params_at_sample_rate(channels, 48_000, params)
    }

    pub fn try_from_params_at_sample_rate(
        channels: usize,
        sample_rate: u32,
        params: HissReducerPluginParams,
    ) -> PluginResult<Self> {
        if channels == 0 {
            return Err("HissReducerPlugin requires at least one channel".to_string());
        }
        let params = Self::canonicalize_params(params, sample_rate)?;
        let mut reducer = Self::build_reducer(channels, &params);
        reducer.initialize(sample_rate)?;
        reducer.set_enabled(params.enabled, true);
        let mut spectral_reducer = SpectralHissReducer::new(channels);
        spectral_reducer.set_enabled(params.enabled);
        spectral_reducer.initialize(sample_rate)?;
        spectral_reducer.set_params(params.frequency_hz, params.threshold_db, params.strength);
        let mut plugin = Self {
            channels,
            sample_rate,
            initialized: false,
            reducer,
            spectral_reducer,
            params,
            cached_parameters: Vec::new(),
        };
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    fn maximum_frequency(sample_rate: u32) -> PluginResult<f32> {
        if sample_rate == 0 {
            return Err("sample rate must be nonzero".to_string());
        }
        let minimum = pk(HP, "frequency_hz").min_f64() as f32;
        let maximum = (sample_rate as f32 * 0.45).min(pk(HP, "frequency_hz").max_f64() as f32);
        if maximum < minimum {
            return Err(format!(
                "sample rate {sample_rate} is too low for the {minimum} Hz minimum cutoff"
            ));
        }
        Ok(maximum)
    }

    fn canonicalize_params(
        mut params: HissReducerPluginParams,
        sample_rate: u32,
    ) -> PluginResult<HissReducerPluginParams> {
        let defaults = HissReducerPluginParams::default();
        let maximum_frequency = Self::maximum_frequency(sample_rate)?;
        params.threshold_db = if params.threshold_db.is_finite() {
            params.threshold_db.clamp(
                pk(HP, "threshold_db").min_f64() as f32,
                pk(HP, "threshold_db").max_f64() as f32,
            )
        } else {
            defaults.threshold_db
        };
        params.frequency_hz = if params.frequency_hz.is_finite() {
            params
                .frequency_hz
                .clamp(pk(HP, "frequency_hz").min_f64() as f32, maximum_frequency)
        } else {
            defaults.frequency_hz.min(maximum_frequency)
        };
        params.strength = if params.strength.is_finite() {
            params.strength.clamp(
                pk(HP, "strength").min_f64() as f32,
                pk(HP, "strength").max_f64() as f32,
            )
        } else {
            defaults.strength
        };
        Ok(params)
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
            4 => Some(if self.params.spectral_mode { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(HP, |i| self.param_value(i));
    }

    fn apply_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        match id.as_str() {
            "enabled" => {
                self.params.enabled = value
                    .as_bool()
                    .ok_or_else(|| "enabled must be a bool".to_string())?;
                self.reducer.set_enabled(self.params.enabled, false);
                self.spectral_reducer.set_enabled(self.params.enabled);
            }
            "threshold_db" => {
                self.params.threshold_db = value
                    .as_float()
                    .ok_or_else(|| "threshold_db must be a float".to_string())?;
                self.reducer.set_params(
                    self.params.frequency_hz,
                    self.params.threshold_db,
                    self.params.strength,
                );
                self.spectral_reducer.set_params(
                    self.params.frequency_hz,
                    self.params.threshold_db,
                    self.params.strength,
                );
            }
            "frequency_hz" => {
                self.params.frequency_hz = value
                    .as_float()
                    .ok_or_else(|| "frequency_hz must be a float".to_string())?;
                self.reducer.set_params(
                    self.params.frequency_hz,
                    self.params.threshold_db,
                    self.params.strength,
                );
                self.spectral_reducer.set_params(
                    self.params.frequency_hz,
                    self.params.threshold_db,
                    self.params.strength,
                );
            }
            "strength" => {
                self.params.strength = value
                    .as_float()
                    .ok_or_else(|| "strength must be a float".to_string())?;
                self.reducer.set_params(
                    self.params.frequency_hz,
                    self.params.threshold_db,
                    self.params.strength,
                );
                self.spectral_reducer.set_params(
                    self.params.frequency_hz,
                    self.params.threshold_db,
                    self.params.strength,
                );
            }
            "spectral_mode" => {
                self.params.spectral_mode = value
                    .as_bool()
                    .ok_or_else(|| "spectral_mode must be a bool".to_string())?;
            }
            _ => return Err(format!("Unknown parameter: {id}")),
        }
        Ok(())
    }
}

impl ParametricInPlacePlugin for HissReducerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Hiss Reducer", env!("CARGO_PKG_VERSION"), "SotF")
            .with_description("Persistent low-level high-frequency reducer")
    }

    fn cost_class(&self) -> PluginCostClass {
        if self.params.spectral_mode {
            PluginCostClass::Fft
        } else {
            PluginCostClass::Iir
        }
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(self.cost_class(), None, self.latency_samples(), false)
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
            ParameterValue::Bool(self.params.enabled),
        );
        values.insert(
            ParameterId::from("threshold_db"),
            ParameterValue::Float(self.params.threshold_db),
        );
        values.insert(
            ParameterId::from("frequency_hz"),
            ParameterValue::Float(self.params.frequency_hz),
        );
        values.insert(
            ParameterId::from("strength"),
            ParameterValue::Float(self.params.strength),
        );
        values.insert(
            ParameterId::from("spectral_mode"),
            ParameterValue::Bool(self.params.spectral_mode),
        );
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        let mut next = self.params.clone();
        for (id, value) in &values {
            self.parametric_validate_parameter(id, value)?;
            param_bridge::set_parameter(HP, id, value, |i, v| match i {
                0 => next.enabled = v > 0.5,
                1 => next.threshold_db = v as f32,
                2 => next.frequency_hz = v as f32,
                3 => next.strength = v as f32,
                4 => next.spectral_mode = v > 0.5,
                _ => {}
            })?;
        }
        self.params = Self::canonicalize_params(next, self.sample_rate)?;
        self.reducer.set_enabled(self.params.enabled, false);
        self.spectral_reducer.set_enabled(self.params.enabled);
        self.reducer.set_params(
            self.params.frequency_hz,
            self.params.threshold_db,
            self.params.strength,
        );
        self.spectral_reducer.set_params(
            self.params.frequency_hz,
            self.params.threshold_db,
            self.params.strength,
        );
        Ok(())
    }

    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        self.cached_parameters
            .iter()
            .find(|parameter| &parameter.id == id)
            .ok_or_else(|| format!("Unknown parameter: {id}"))?
            .validate(value)
            .map_err(|error| format!("{id}: {error}"))?;
        if id.as_str() == "frequency_hz" {
            let frequency = value
                .as_float()
                .ok_or_else(|| "frequency_hz must be a float".to_string())?;
            let maximum = Self::maximum_frequency(self.sample_rate)?;
            if frequency > maximum {
                return Err(format!(
                    "frequency_hz must not exceed {maximum} Hz at sample rate {}",
                    self.sample_rate
                ));
            }
        }
        if id.as_str() == "spectral_mode" && self.initialized {
            let requested = value
                .as_bool()
                .ok_or_else(|| "spectral_mode must be a bool".to_string())?;
            if requested != self.params.spectral_mode {
                return Err(
                    "spectral_mode is structural and requires plugin reconstruction".to_string(),
                );
            }
        }
        Ok(())
    }

    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        self.parametric_validate_parameter(&id, &value)?;
        self.apply_parameter(id, value)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.params = Self::canonicalize_params(self.params.clone(), sample_rate)?;
        self.sample_rate = sample_rate;
        self.reducer.initialize(sample_rate)?;
        self.spectral_reducer.set_enabled(self.params.enabled);
        self.spectral_reducer.initialize(sample_rate)?;
        self.reducer.set_params(
            self.params.frequency_hz,
            self.params.threshold_db,
            self.params.strength,
        );
        self.reducer.set_enabled(self.params.enabled, true);
        self.spectral_reducer.set_params(
            self.params.frequency_hz,
            self.params.threshold_db,
            self.params.strength,
        );
        self.initialized = true;
        Ok(())
    }

    fn reset(&mut self) {
        self.reducer.reset();
        self.spectral_reducer.reset();
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

        if !self.initialized {
            return Err("plugin must be initialized before processing".to_string());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "sample rate mismatch: initialized at {}, context is {}",
                self.sample_rate, context.sample_rate
            ));
        }

        if self.params.spectral_mode {
            self.spectral_reducer.process(buffer);
        } else {
            // Keep filter and detector state warm while bypassed; HissReducer
            // owns the click-free wet/dry transition and reaches exact dry.
            self.reducer.process(buffer);
        }
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        if self.params.spectral_mode {
            self.spectral_reducer.latency_samples()
        } else {
            self.reducer.latency_samples()
        }
    }
}
