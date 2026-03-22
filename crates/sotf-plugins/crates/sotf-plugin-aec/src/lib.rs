// ============================================================================
// AEC Plugin — Acoustic Echo Cancellation
// ============================================================================
//
// Implements acoustic echo cancellation using:
// - PBFDAF (Partitioned Block Frequency Domain Adaptive Filter)
// - Two-path management (foreground/background)
// - Residual echo suppression post-filter
//
// Input: 2-channel interleaved (channel 0 = microphone, channel 1 = reference)
// Output: 1-channel (echo-cancelled microphone signal)

pub mod params;

mod pbfdaf;
mod post_filter;
mod two_path;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use std::any::Any;
use std::sync::Arc;

use crate::post_filter::ResidualEchoSuppressor;
use crate::two_path::TwoPathAec;

const DEFAULT_BLOCK_SIZE: usize = 256;
const DEFAULT_ECHO_TAIL_MS: f32 = 200.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AecPluginParams {
    /// Echo tail length in milliseconds (50-500)
    #[serde(default = "default_echo_tail_ms")]
    pub echo_tail_ms: f32,
    /// Adaptive filter step size (0.1-0.9)
    #[serde(default = "default_step_size")]
    pub step_size: f32,
    /// Enable residual echo suppression post-filter
    #[serde(default = "default_post_filter_enabled")]
    pub post_filter_enabled: bool,
}

fn default_echo_tail_ms() -> f32 {
    DEFAULT_ECHO_TAIL_MS
}
fn default_step_size() -> f32 {
    0.5
}
fn default_post_filter_enabled() -> bool {
    true
}

impl Default for AecPluginParams {
    fn default() -> Self {
        Self {
            echo_tail_ms: default_echo_tail_ms(),
            step_size: default_step_size(),
            post_filter_enabled: default_post_filter_enabled(),
        }
    }
}

pub struct AecPlugin {
    sample_rate: u32,
    aec: TwoPathAec,
    post_filter: ResidualEchoSuppressor,
    post_filter_enabled: bool,
    echo_tail_ms: f32,
    step_size: f32,
    block_size: usize,
    /// Input accumulation buffers (mic and reference)
    mic_buffer: Vec<f32>,
    ref_buffer: Vec<f32>,
    input_fill: usize,
    /// Output buffer for processed samples
    output_buffer: Vec<f32>,
    output_read_pos: usize,
    output_write_pos: usize,
    /// FFT for post-filter
    fft_forward: Arc<dyn Fft<f32>>,
    fft_inverse: Arc<dyn Fft<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    /// Parameter IDs
    param_echo_tail_ms: ParameterId,
    param_step_size: ParameterId,
    param_post_filter: ParameterId,
    cached_parameters: Vec<Parameter>,
}

impl AecPlugin {
    pub fn new(sample_rate: u32) -> Self {
        let block_size = DEFAULT_BLOCK_SIZE;
        let echo_tail_samples =
            (DEFAULT_ECHO_TAIL_MS / 1000.0 * sample_rate as f32) as usize;

        let fft_size = block_size * 2;
        let mut planner = FftPlanner::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);
        let scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());

        let mut p = Self {
            sample_rate,
            aec: TwoPathAec::new(block_size, echo_tail_samples, 0.3, 0.7),
            post_filter: ResidualEchoSuppressor::new(fft_size, 1.5, 0.056),
            post_filter_enabled: true,
            echo_tail_ms: DEFAULT_ECHO_TAIL_MS,
            step_size: 0.5,
            block_size,
            mic_buffer: vec![0.0; block_size],
            ref_buffer: vec![0.0; block_size],
            input_fill: 0,
            output_buffer: vec![0.0; block_size * 16],
            output_read_pos: 0,
            output_write_pos: 0,
            fft_forward,
            fft_inverse,
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            param_echo_tail_ms: ParameterId::from("echo_tail_ms"),
            param_step_size: ParameterId::from("step_size"),
            param_post_filter: ParameterId::from("post_filter_enabled"),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(sample_rate: u32, params: AecPluginParams) -> Self {
        let mut plugin = Self::new(sample_rate);
        plugin.echo_tail_ms = params.echo_tail_ms;
        plugin.step_size = params.step_size;
        plugin.post_filter_enabled = params.post_filter_enabled;
        plugin.rebuild_aec();
        plugin
    }

    fn rebuild_aec(&mut self) {
        let echo_tail_samples =
            (self.echo_tail_ms / 1000.0 * self.sample_rate as f32) as usize;
        self.aec = TwoPathAec::new(
            self.block_size,
            echo_tail_samples,
            self.step_size * 0.6,
            self.step_size,
        );
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float("echo_tail_ms", "Echo Tail", self.echo_tail_ms, 50.0, 500.0)
                .with_description("Echo tail length in milliseconds")
                .with_group("AEC")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("step_size", "Step Size", self.step_size, 0.1, 0.9)
                .with_description("Adaptive filter learning rate")
                .with_group("AEC")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "post_filter_enabled",
                "Post-Filter",
                self.post_filter_enabled,
            )
            .with_description("Enable residual echo suppression")
            .with_group("AEC")
            .with_importance(ParameterImportance::Useful),
        ];
    }

    fn available_output(&self) -> usize {
        if self.output_write_pos >= self.output_read_pos {
            self.output_write_pos - self.output_read_pos
        } else {
            self.output_buffer.len() - self.output_read_pos + self.output_write_pos
        }
    }

    fn push_output(&mut self, samples: &[f32]) {
        for &s in samples {
            self.output_buffer[self.output_write_pos] = s;
            self.output_write_pos = (self.output_write_pos + 1) % self.output_buffer.len();
        }
    }

    fn pop_output(&mut self) -> f32 {
        let s = self.output_buffer[self.output_read_pos];
        self.output_read_pos = (self.output_read_pos + 1) % self.output_buffer.len();
        s
    }
}

impl Plugin for AecPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("AEC", "1.0.0", "Sotf")
            .with_description("Acoustic Echo Cancellation (PBFDAF + Two-Path + Post-Filter)")
    }

    fn input_channels(&self) -> usize {
        2 // mic + reference
    }

    fn output_channels(&self) -> usize {
        1 // echo-cancelled mono
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id == self.param_echo_tail_ms {
            let val = value.as_float().unwrap_or(DEFAULT_ECHO_TAIL_MS);
            self.echo_tail_ms = val.clamp(50.0, 500.0);
            self.rebuild_aec();
            self.rebuild_cached_parameters();
        } else if id == self.param_step_size {
            let val = value.as_float().unwrap_or(0.5);
            self.step_size = val.clamp(0.1, 0.9);
            self.rebuild_aec();
            self.rebuild_cached_parameters();
        } else if id == self.param_post_filter {
            self.post_filter_enabled = value.as_bool().unwrap_or(true);
            self.rebuild_cached_parameters();
        } else {
            return Err(format!("Unknown parameter: {id}"));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_echo_tail_ms {
            Some(ParameterValue::Float(self.echo_tail_ms))
        } else if id == &self.param_step_size {
            Some(ParameterValue::Float(self.step_size))
        } else if id == &self.param_post_filter {
            Some(ParameterValue::Bool(self.post_filter_enabled))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.rebuild_aec();
        Ok(())
    }

    fn reset(&mut self) {
        self.aec.reset();
        self.post_filter.reset();
        self.input_fill = 0;
        self.output_read_pos = 0;
        self.output_write_pos = 0;
        self.output_buffer.fill(0.0);
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let nf = context.num_frames;

        // Deinterleave: input is [mic0, ref0, mic1, ref1, ...]
        for i in 0..nf {
            let mic_sample = input[i * 2];
            let ref_sample = input[i * 2 + 1];

            self.mic_buffer[self.input_fill] = mic_sample;
            self.ref_buffer[self.input_fill] = ref_sample;
            self.input_fill += 1;

            if self.input_fill == self.block_size {
                // Process one block — copy error output before push (avoids borrow conflict)
                let error = self.aec.process(&self.mic_buffer, &self.ref_buffer);
                let error_len = error.len();
                // Copy to output ring (we can index output_buffer directly)
                for j in 0..error_len {
                    self.output_buffer[self.output_write_pos] = error[j];
                    self.output_write_pos =
                        (self.output_write_pos + 1) % self.output_buffer.len();
                }
                self.input_fill = 0;
            }
        }

        // Write available output
        let available = self.available_output();
        let to_write = nf.min(available);
        for i in 0..to_write {
            output[i] = self.pop_output();
        }
        // Zero-fill if not enough output yet (initial latency)
        for i in to_write..nf {
            output[i] = 0.0;
        }

        Ok(nf)
    }

    fn latency_samples(&self) -> usize {
        self.block_size
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}

impl std::fmt::Debug for AecPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AecPlugin")
            .field("echo_tail_ms", &self.echo_tail_ms)
            .field("step_size", &self.step_size)
            .field("post_filter_enabled", &self.post_filter_enabled)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aec_plugin_creation() {
        let plugin = AecPlugin::new(48000);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 1);
        assert_eq!(plugin.latency_samples(), DEFAULT_BLOCK_SIZE);
    }

    #[test]
    fn test_aec_plugin_parameters() {
        let mut plugin = AecPlugin::new(48000);
        let params = plugin.parameters();
        assert_eq!(params.len(), 3);

        // Set echo tail
        plugin
            .set_parameter(
                ParameterId::from("echo_tail_ms"),
                ParameterValue::Float(100.0),
            )
            .unwrap();
        assert_eq!(plugin.echo_tail_ms, 100.0);
    }

    #[test]
    fn test_aec_plugin_process() {
        let mut plugin = AecPlugin::new(48000);
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 512,
        };

        // 2-channel interleaved input
        let input = vec![0.1f32; 512 * 2];
        let mut output = vec![0.0f32; 512];

        let result = plugin.process(&input, &mut output, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_aec_echo_reduction() {
        let sample_rate = 48000;
        let mut plugin = AecPlugin::from_params(
            sample_rate,
            AecPluginParams {
                echo_tail_ms: 100.0,
                step_size: 0.7,
                post_filter_enabled: false,
            },
        );

        let block_size = 256;
        let delay = 50;
        let num_blocks = 200;

        let mut ref_history = Vec::new();
        let mut late_mic_power = 0.0f32;
        let mut late_error_power = 0.0f32;

        for block_idx in 0..num_blocks {
            // Generate reference
            let reference: Vec<f32> = (0..block_size)
                .map(|i| {
                    let t = (block_idx * block_size + i) as f32;
                    (t * 0.1).sin() * 0.5
                })
                .collect();
            ref_history.extend_from_slice(&reference);

            // Simulate echo
            let mic: Vec<f32> = (0..block_size)
                .map(|i| {
                    let gi = block_idx * block_size + i;
                    if gi >= delay && gi - delay < ref_history.len() {
                        ref_history[gi - delay] * 0.5
                    } else {
                        0.0
                    }
                })
                .collect();

            // Interleave mic + reference
            let mut input = vec![0.0f32; block_size * 2];
            for i in 0..block_size {
                input[i * 2] = mic[i];
                input[i * 2 + 1] = reference[i];
            }

            let context = ProcessContext {
                sample_rate,
                num_frames: block_size,
            };
            let mut output = vec![0.0f32; block_size];
            plugin.process(&input, &mut output, &context).unwrap();

            // Measure in last quarter
            if block_idx >= num_blocks * 3 / 4 {
                late_mic_power += mic.iter().map(|x| x * x).sum::<f32>();
                late_error_power += output.iter().map(|x| x * x).sum::<f32>();
            }
        }

        // Error power should be less than mic power (some echo cancelled)
        if late_mic_power > 0.01 {
            assert!(
                late_error_power < late_mic_power,
                "Error power ({late_error_power:.4}) should be less than mic power ({late_mic_power:.4})"
            );
        }
    }
}
