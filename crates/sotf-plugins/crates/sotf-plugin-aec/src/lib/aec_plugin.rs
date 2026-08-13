use super::misc::DEFAULT_BLOCK_SIZE;
use super::misc::DEFAULT_ECHO_TAIL_MS;
use crate::params::Params as AecPluginParams;
use crate::post_filter::ResidualEchoSuppressor;
use crate::two_path::TwoPathAec;
use realfft::{ComplexToReal, RealFftPlanner};
use rustfft::num_complex::Complex;
use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use std::any::Any;
use std::sync::Arc;

pub struct AecPlugin {
    pub(super) sample_rate: u32,
    pub(super) aec: TwoPathAec,
    pub(super) post_filter: ResidualEchoSuppressor,
    pub(super) post_filter_enabled: bool,
    pub(super) post_filter_mix: f32,
    pub(super) post_filter_mix_target: f32,
    pub(super) post_filter_mix_step: f32,
    pub(super) echo_tail_ms: f32,
    pub(super) step_size: f32,
    pub(super) block_size: usize,
    /// Input accumulation buffers (mic and reference)
    pub(super) mic_buffer: Vec<f32>,
    pub(super) ref_buffer: Vec<f32>,
    pub(super) input_fill: usize,
    /// Output buffer for processed samples
    pub(super) output_buffer: Vec<f32>,
    pub(super) output_read_pos: usize,
    pub(super) output_write_pos: usize,
    pub(super) output_len: usize,
    /// Real inverse FFT for post-filter reconstruction.
    pub(super) fft_inverse: Arc<dyn ComplexToReal<f32>>,
    pub(super) fft_scratch: Vec<Complex<f32>>,
    /// Pre-allocated buffer for post-filter IFFT output (time domain)
    pub(super) post_filter_time_buf: Vec<f32>,
    /// Pre-allocated buffer for post-filter IFFT input (frequency domain)
    pub(super) post_filter_ifft_buf: Vec<Complex<f32>>,
    /// Parameter IDs
    pub(super) param_echo_tail_ms: ParameterId,
    pub(super) param_step_size: ParameterId,
    pub(super) param_post_filter: ParameterId,
    pub(super) cached_parameters: Vec<Parameter>,
    pub(super) initialized: bool,
}

impl AecPlugin {
    pub fn new(sample_rate: u32) -> Self {
        let block_size = DEFAULT_BLOCK_SIZE;
        let echo_tail_samples = (DEFAULT_ECHO_TAIL_MS / 1000.0 * sample_rate as f32) as usize;

        let fft_size = block_size * 2;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_inverse = planner.plan_fft_inverse(fft_size);
        let scratch_len = fft_inverse.get_scratch_len();

        let mut p = Self {
            sample_rate,
            aec: TwoPathAec::new(block_size, echo_tail_samples, 0.3, 0.7),
            post_filter: ResidualEchoSuppressor::new_with_timing(
                block_size + 1,
                1.5,
                0.056,
                block_size,
                sample_rate,
            ),
            post_filter_enabled: true,
            post_filter_mix: 1.0,
            post_filter_mix_target: 1.0,
            post_filter_mix_step: 1.0 / (sample_rate as f32 * 0.010).max(1.0),
            echo_tail_ms: DEFAULT_ECHO_TAIL_MS,
            step_size: 0.5,
            block_size,
            mic_buffer: vec![0.0; block_size],
            ref_buffer: vec![0.0; block_size],
            input_fill: 0,
            // A bounded FIFO implements exactly one AEC block of latency. It is
            // consumed sample-by-sample and replenished whenever a block finishes.
            output_buffer: vec![0.0; block_size],
            output_read_pos: 0,
            output_write_pos: 0,
            output_len: block_size,
            fft_inverse,
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            post_filter_time_buf: vec![0.0; fft_size],
            post_filter_ifft_buf: vec![Complex::new(0.0, 0.0); block_size + 1],
            param_echo_tail_ms: ParameterId::from("echo_tail_ms"),
            param_step_size: ParameterId::from("step_size"),
            param_post_filter: ParameterId::from("post_filter_enabled"),
            cached_parameters: Vec::new(),
            initialized: false,
        };
        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(sample_rate: u32, params: AecPluginParams) -> Result<Self, String> {
        Self::validate_configuration(sample_rate, &params)?;
        let mut plugin = Self::new(sample_rate);
        plugin.echo_tail_ms = params.echo_tail_ms as f32;
        plugin.step_size = params.step_size as f32;
        plugin.post_filter_enabled = params.post_filter_enabled;
        plugin.post_filter_mix = if params.post_filter_enabled { 1.0 } else { 0.0 };
        plugin.post_filter_mix_target = plugin.post_filter_mix;
        plugin.rebuild_aec();
        plugin.rebuild_cached_parameters();
        plugin.initialized = true;
        Ok(plugin)
    }

    fn validate_configuration(sample_rate: u32, params: &AecPluginParams) -> Result<(), String> {
        if sample_rate == 0 {
            return Err("AEC sample rate must be greater than zero".to_string());
        }
        if !params.echo_tail_ms.is_finite() || !(50.0_f64..=500.0).contains(&params.echo_tail_ms) {
            return Err(format!(
                "AEC echo_tail_ms must be finite and in 50..=500, got {}",
                params.echo_tail_ms
            ));
        }
        if !params.step_size.is_finite() || !(0.1_f64..=0.9).contains(&params.step_size) {
            return Err(format!(
                "AEC step_size must be finite and in 0.1..=0.9, got {}",
                params.step_size
            ));
        }
        Ok(())
    }

    pub(super) fn rebuild_aec(&mut self) {
        let echo_tail_samples = (self.echo_tail_ms / 1000.0 * self.sample_rate as f32) as usize;
        self.aec = TwoPathAec::new_with_sample_rate(
            self.block_size,
            echo_tail_samples,
            self.step_size * 0.6,
            self.step_size,
            self.sample_rate,
        );
    }

    fn rebuild_post_filter(&mut self) {
        // The suppressor owns smoothed gains and DTD history. Rebuild it as
        // part of initialization so no state accumulated at the old stream
        // rate can influence the first block of the new stream.
        self.post_filter = ResidualEchoSuppressor::new_with_timing(
            self.block_size + 1,
            1.5,
            0.056,
            self.block_size,
            self.sample_rate,
        );
        self.post_filter_mix_step = 1.0 / (self.sample_rate as f32 * 0.010).max(1.0);
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float("echo_tail_ms", "Echo Tail", self.echo_tail_ms, 50.0, 500.0)
                .with_description("Echo tail length in milliseconds")
                .with_group("AEC")
                .with_update_mode(UpdateMode::Structural)
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("step_size", "Step Size", self.step_size, 0.1, 0.9)
                .with_description("Adaptive filter learning rate")
                .with_group("AEC")
                .with_update_mode(UpdateMode::Structural)
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

    pub(super) fn pop_output(&mut self) -> f32 {
        debug_assert!(self.output_len > 0);
        let s = self.output_buffer[self.output_read_pos];
        self.output_read_pos = (self.output_read_pos + 1) % self.output_buffer.len();
        self.output_len -= 1;
        s
    }

    fn reset_streaming_state(&mut self) {
        self.input_fill = 0;
        self.mic_buffer.fill(0.0);
        self.ref_buffer.fill(0.0);
        self.output_buffer.fill(0.0);
        self.output_read_pos = 0;
        self.output_write_pos = 0;
        self.output_len = self.block_size;
        self.post_filter_time_buf.fill(0.0);
        self.post_filter_ifft_buf.fill(Complex::new(0.0, 0.0));
        self.fft_scratch.fill(Complex::new(0.0, 0.0));
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

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(PluginCostClass::Fft, None, self.latency_samples(), true)
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id == self.param_echo_tail_ms {
            let val = value.as_float().unwrap_or(DEFAULT_ECHO_TAIL_MS);
            if val == self.echo_tail_ms {
                return Ok(());
            }
            if self.initialized {
                return Err("echo_tail_ms is structural; recreate the AEC plugin".to_string());
            }
            self.echo_tail_ms = val.clamp(50.0, 500.0);
            self.rebuild_aec();
            self.rebuild_cached_parameters();
        } else if id == self.param_step_size {
            let val = value.as_float().unwrap_or(0.5);
            if val == self.step_size {
                return Ok(());
            }
            if self.initialized {
                return Err("step_size is structural; recreate the AEC plugin".to_string());
            }
            self.step_size = val.clamp(0.1, 0.9);
            self.rebuild_aec();
            self.rebuild_cached_parameters();
        } else if id == self.param_post_filter {
            let enabled = value.as_bool().unwrap_or(true);
            if enabled == self.post_filter_enabled {
                return Ok(());
            }
            self.post_filter_enabled = enabled;
            self.post_filter_mix_target = if enabled { 1.0 } else { 0.0 };
            if let Some(parameter) = self
                .cached_parameters
                .iter_mut()
                .find(|parameter| parameter.id == self.param_post_filter)
            {
                parameter.default_value = ParameterValue::Bool(enabled);
            }
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
        if sample_rate == 0 {
            return Err("AEC sample rate must be greater than zero".to_string());
        }
        self.sample_rate = sample_rate;
        self.rebuild_aec();
        self.rebuild_post_filter();
        self.reset_streaming_state();
        self.initialized = true;
        Ok(())
    }

    fn reset(&mut self) {
        self.aec.reset();
        self.post_filter.reset();
        self.reset_streaming_state();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let nf = context.num_frames;
        let expected_input = nf
            .checked_mul(2)
            .ok_or_else(|| "Frame/input-channel count overflow".to_string())?;
        if input.len() != expected_input {
            return Err(format!(
                "Input buffer size mismatch: expected {}, got {}",
                expected_input,
                input.len()
            ));
        }
        if output.len() != nf {
            return Err(format!(
                "Output buffer size mismatch: expected {}, got {}",
                nf,
                output.len()
            ));
        }

        // Deinterleave: input is [mic0, ref0, mic1, ref1, ...]
        for i in 0..nf {
            // Consume before replenishing so the generated block always appears
            // exactly `block_size` samples after its input, independent of host
            // callback segmentation.
            output[i] = self.pop_output();
            // Explicit realtime policy: non-finite device/plugin input is
            // replaced by silence before it can poison adaptive state.
            let mic_sample = if input[i * 2].is_finite() {
                input[i * 2]
            } else {
                0.0
            };
            let ref_sample = if input[i * 2 + 1].is_finite() {
                input[i * 2 + 1]
            } else {
                0.0
            };

            self.mic_buffer[self.input_fill] = mic_sample;
            self.ref_buffer[self.input_fill] = ref_sample;
            self.input_fill += 1;

            if self.input_fill == self.block_size {
                // Process one block — copy error output before push (avoids borrow conflict)
                let error = self.aec.process(&self.mic_buffer, &self.ref_buffer);
                let error_len = error.len();
                // The input block has been consumed, so reuse its storage for
                // the dry path and release the mutable AEC borrow.
                self.mic_buffer[..error_len].copy_from_slice(error);

                // Keep suppressor state current in both wet and dry modes;
                // only the final allocation-free mix is toggled.
                {
                    let error_freq = self.aec.last_error_freq();
                    let echo_est_freq = self.aec.last_echo_estimate_freq();
                    let unique_bins = self.block_size + 1;
                    let suppressed = self
                        .post_filter
                        .process(&error_freq[..unique_bins], &echo_est_freq[..unique_bins]);
                    // IFFT the suppressed spectrum to get time-domain output
                    // Copy into pre-allocated buffer since IFFT needs mutable access
                    let n_sup = suppressed.len();
                    // n_sup == fft_size == block_size * 2, which is invariant
                    // at construction time.  A runtime resize here would be an
                    // allocation inside the audio callback.
                    debug_assert_eq!(
                        self.post_filter_ifft_buf.len(),
                        n_sup,
                        "post_filter_ifft_buf size mismatch: expected {n_sup}, got {}",
                        self.post_filter_ifft_buf.len()
                    );
                    let n_sup = self.post_filter_ifft_buf.len();
                    self.post_filter_ifft_buf[..n_sup].copy_from_slice(&suppressed[..n_sup]);
                    self.fft_inverse
                        .process_with_scratch(
                            &mut self.post_filter_ifft_buf[..n_sup],
                            &mut self.post_filter_time_buf,
                            &mut self.fft_scratch,
                        )
                        .map_err(|error| format!("AEC post-filter inverse FFT failed: {error}"))?;
                    let inv_n = 1.0 / (self.block_size * 2) as f32;
                    let b = self.block_size;
                    for i in 0..b.min(error_len) {
                        let target = self.post_filter_mix_target;
                        if self.post_filter_mix < target {
                            self.post_filter_mix =
                                (self.post_filter_mix + self.post_filter_mix_step).min(target);
                        } else if self.post_filter_mix > target {
                            self.post_filter_mix =
                                (self.post_filter_mix - self.post_filter_mix_step).max(target);
                        }
                        let wet = self.post_filter_time_buf[b + i] * inv_n;
                        let dry = self.mic_buffer[i];
                        let sample = dry + self.post_filter_mix * (wet - dry);
                        self.output_buffer[self.output_write_pos] =
                            if sample.is_finite() { sample } else { 0.0 };
                        self.output_write_pos =
                            (self.output_write_pos + 1) % self.output_buffer.len();
                        self.output_len += 1;
                    }
                }
                self.input_fill = 0;
            }
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

impl AecPlugin {
    pub fn post_filter_mix(&self) -> f32 {
        self.post_filter_mix
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
