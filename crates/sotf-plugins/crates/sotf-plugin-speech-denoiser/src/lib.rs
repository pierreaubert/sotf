pub mod params;

use crate::params::PARAMS as SP;
use plugins_denoiser::rnnoise::RnnoiseBackend;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};

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
        self.inner
            .initialize(sample_rate, self.channels)
            .map_err(|e| e.to_string())
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let expected_len = context.num_frames * self.channels;
        if buffer.len() < expected_len {
            return Err(format!(
                "Buffer too small: {} < {}",
                buffer.len(),
                expected_len
            ));
        }

        if context.num_frames % 480 != 0 {
            return Err(format!(
                "RNNoise requires block sizes multiple of 480; got {}",
                context.num_frames
            ));
        }

        let bypass = !self.enabled;
        let frames_written = self
            .inner
            .process(buffer, context.num_frames, self.channels, bypass);
        Ok(frames_written)
    }

    fn latency_samples(&self) -> usize {
        self.inner.latency_samples()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::parameters::ParameterValue;
    use sotf_host::plugin::{InPlacePlugin, ProcessContext};

    #[test]
    fn disabled_is_transparent() {
        let mut plugin = SpeechDenoiserPlugin::new(2);
        plugin
            .set_parameter("enabled".into(), ParameterValue::Bool(false))
            .expect("set enabled");
        plugin.initialize(48000).expect("initialize");

        // Process 960 frames: first 480 discarded (startup delay), second 480 pass through.
        let mut buffer: Vec<f32> = (0..1920)
            .map(|i| ((i % 100) as f32 - 50.0) / 100.0)
            .collect();
        let input = buffer.clone();
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 960,
        };
        let written = plugin.process_in_place(&mut buffer, &context).unwrap();
        // First frame discarded, so only 480 frames written.
        assert_eq!(written, 480);
        // The second 480 frames of input should appear at the start of the output.
        assert_eq!(&buffer[..960], &input[960..1920]);
    }

    #[test]
    fn latency_is_constant_when_disabled() {
        let mut plugin = SpeechDenoiserPlugin::new(1);
        plugin.initialize(48000).expect("initialize");
        assert_eq!(plugin.latency_samples(), 480);

        plugin
            .set_parameter("enabled".into(), ParameterValue::Bool(false))
            .expect("set enabled");
        assert_eq!(plugin.latency_samples(), 480);
    }

    #[test]
    fn rejects_non_multiple_of_480() {
        let mut plugin = SpeechDenoiserPlugin::new(1);
        plugin.initialize(48000).expect("initialize");

        let mut buffer = vec![0.0f32; 512];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 512,
        };
        let result = plugin.process_in_place(&mut buffer, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("480"));
    }

    #[test]
    fn rejects_non_48khz() {
        let mut plugin = SpeechDenoiserPlugin::new(1);
        let result = plugin.initialize(44100);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("48 kHz"));
    }

    #[test]
    fn disabled_preserves_latency() {
        let mut plugin = SpeechDenoiserPlugin::new(1);
        plugin
            .set_parameter("enabled".into(), ParameterValue::Bool(false))
            .expect("set enabled");
        plugin.initialize(48000).expect("initialize");

        // Inject an impulse at sample 0 of the second 480-frame block.
        let mut buffer = vec![0.0f32; 960];
        buffer[480] = 1.0;

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 960,
        };
        let written = plugin.process_in_place(&mut buffer, &context).unwrap();
        // First frame is discarded (startup delay), so only 480 frames output.
        assert_eq!(written, 480);
        // The impulse should appear at the start of the output buffer because
        // the first 480-sample frame was discarded and the second frame is output.
        assert!(
            buffer[0].abs() > 0.99,
            "Bypass should preserve the same startup delay; impulse should appear at sample 0"
        );
    }
}
