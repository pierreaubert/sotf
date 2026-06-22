pub mod params;

use crate::params::PARAMS as SP;
use plugins_denoiser::rnnoise::RnnoiseBackend;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};

/// RNNoise processes fixed 480-sample frames at 48 kHz.
pub const SPEECH_DENOISER_FRAME_SIZE: usize = 480;

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
    inner: RnnoiseBackend,
    cached_parameters: Vec<Parameter>,
}

impl SpeechDenoiserPlugin {
    pub fn new(channels: usize) -> Self {
        Self::from_params(channels, SpeechDenoiserPluginParams::default())
    }

    pub fn from_params(channels: usize, params: SpeechDenoiserPluginParams) -> Self {
        let mut plugin = Self {
            channels,
            enabled: params.enabled,
            inner: RnnoiseBackend::new(),
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

impl ParametricInPlacePlugin for SpeechDenoiserPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Speech Denoiser", "1.0.0", "SotF")
            .with_description("RNNoise speech denoiser")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Fft
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(PluginCostClass::Fft, None, self.latency_samples(), false)
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
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            match id.as_str() {
                "enabled" => {
                    self.enabled = value.as_bool().unwrap_or(self.enabled);
                }
                _ => return Err(format!("Unknown parameter: {id}")),
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    /// Initialize the plugin at the given sample rate.
    ///
    /// Returns `Err` if `sample_rate != 48000`; RNNoise is hard-coded for
    /// 48 kHz and will silently corrupt the frequency response at any other
    /// rate.
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.inner.initialize(sample_rate, self.channels)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        // Validate buffer length before touching any index.
        let expected_len = context.num_frames * self.channels;
        if buffer.len() < expected_len {
            return Err(format!(
                "Buffer too small: {} < {} (num_frames={} * channels={})",
                buffer.len(),
                expected_len,
                context.num_frames,
                self.channels
            ));
        }

        // RNNoise processes in fixed 480-sample frames.  Arbitrary block
        // sizes cause periodic zero-padding dropouts; reject upfront.
        if !context
            .num_frames
            .is_multiple_of(SPEECH_DENOISER_FRAME_SIZE)
        {
            return Err(format!(
                "RNNoise requires block sizes that are a multiple of {}; got {}",
                SPEECH_DENOISER_FRAME_SIZE, context.num_frames
            ));
        }

        let frames_written =
            self.inner
                .process(buffer, context.num_frames, self.channels, !self.enabled);
        Ok(frames_written)
    }

    /// Returns a fixed latency of 480 samples regardless of the `enabled`
    /// flag.
    ///
    /// Plugin hosts require latency to remain constant after initialisation.
    /// Returning 0 when disabled would cause phase cancellation in parallel
    /// processing chains and misalignment with other latency-compensated
    /// tracks.
    fn latency_samples(&self) -> usize {
        self.inner.latency_samples()
    }
}
