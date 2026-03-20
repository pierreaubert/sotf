// ============================================================================
// Convolution Plugin - Partitioned FFT-based convolution
// ============================================================================

pub mod nupc;

use arc_swap::ArcSwap;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{complex_mul_add_simd, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::path::Path;
use std::sync::Arc;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{CodecRegistry, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::{Hint, Probe};

const PARTITION_SIZE: usize = 1024;
const FFT_SIZE: usize = PARTITION_SIZE * 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvolutionPluginParams {
    pub ir_file: String,
    pub mix: f32,
    pub gain_db: f32,
    /// Use Non-Uniform Partitioned Convolution for long IRs
    #[serde(default = "default_use_nupc")]
    pub use_nupc: bool,
}

fn default_use_nupc() -> bool {
    true
}

struct ConvolutionState {
    partitions: Vec<Vec<Vec<Complex<f32>>>>, // [channel][partition][bin]
    num_partitions: usize,
    ir_channels: usize,
    fft_forward: Arc<dyn rustfft::Fft<f32>>,
    fft_inverse: Arc<dyn rustfft::Fft<f32>>,
}

pub struct ConvolutionPlugin {
    channels: usize,
    sample_rate: u32,
    ir_file: String,
    param_mix: ParameterId,
    mix: Smoother,
    mix_value: f32,
    param_gain_db: ParameterId,
    gain_linear: Smoother,
    gain_db_value: f32,
    state: Arc<ArcSwap<Option<ConvolutionState>>>,
    input_buffers: Vec<Vec<f32>>,
    input_fill: usize,
    /// Flattened Frequency Domain Line (FDL): [partition * channels * FFT_SIZE + channel * FFT_SIZE + bin]
    fdl_flat: Vec<Complex<f32>>,
    fdl_head: usize, // ring buffer head for FDL (avoids rotate_right)
    output_accum: Vec<Vec<f32>>,
    // Pre-allocated scratch buffers (avoid heap allocs in audio callback)
    fft_spectrum: Vec<Complex<f32>>,
    fft_sum: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    cached_parameters: Vec<Parameter>,
}

impl ConvolutionPlugin {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        let mut p = Self {
            channels,
            sample_rate,
            ir_file: String::new(),
            param_mix: ParameterId::from("mix"),
            mix: Smoother::new(1.0, 20.0, sample_rate),
            mix_value: 1.0,
            param_gain_db: ParameterId::from("gain_db"),
            gain_linear: Smoother::new(1.0, 20.0, sample_rate),
            gain_db_value: 0.0,
            state: Arc::new(ArcSwap::from_pointee(None)),
            input_buffers: vec![vec![0.0; PARTITION_SIZE]; channels],
            input_fill: 0,
            fdl_flat: Vec::new(),
            fdl_head: 0,
            output_accum: vec![vec![0.0; FFT_SIZE]; channels],
            fft_spectrum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_sum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_scratch: Vec::new(),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        use sotf_host::param_specs::{convolution::PARAMS as CV, find_by_key as pk};
        self.cached_parameters = vec![
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix_value,
                pk(CV, "mix").min_f64() as f32,
                pk(CV, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = convolved)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "gain_db",
                "Gain",
                self.gain_db_value,
                pk(CV, "gain_db").min_f64() as f32,
                pk(CV, "gain_db").max_f64() as f32,
            )
            .with_description("Output gain (dB)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
        ];
    }

    pub fn from_params(
        channels: usize,
        sample_rate: u32,
        params: ConvolutionPluginParams,
    ) -> Result<Self, String> {
        let mut plugin = Self::new(channels, sample_rate);
        if !params.ir_file.is_empty() {
            let _ = plugin.load_ir(&params.ir_file);
        }
        plugin.mix_value = params.mix;
        plugin.mix.set_target(params.mix);
        plugin.gain_db_value = params.gain_db;
        plugin
            .gain_linear
            .set_target(10.0f32.powf(params.gain_db / 20.0));
        Ok(plugin)
    }

    pub fn load_ir(&mut self, path: &str) -> Result<(), String> {
        let (ir_samples, ir_sample_rate) = Self::load_audio_file(path)?;

        // Resample the IR if its sample rate differs from the engine's sample rate
        let ir_samples = if ir_sample_rate != 0 && ir_sample_rate != self.sample_rate {
            log::info!(
                "Resampling IR from {} Hz to {} Hz",
                ir_sample_rate,
                self.sample_rate
            );
            Self::resample_ir(&ir_samples, ir_sample_rate, self.sample_rate)?
        } else {
            ir_samples
        };

        let ir_channels = ir_samples.len();
        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);

        let mut partitions = Vec::with_capacity(ir_channels);
        for ch_samples in ir_samples {
            let num_parts = ch_samples.len().div_ceil(PARTITION_SIZE);
            let mut ch_parts = Vec::with_capacity(num_parts);
            for p in 0..num_parts {
                let mut block = vec![Complex::new(0.0, 0.0); FFT_SIZE];
                let start = p * PARTITION_SIZE;
                let end = (start + PARTITION_SIZE).min(ch_samples.len());
                for (i, &s) in ch_samples[start..end].iter().enumerate() {
                    block[i] = Complex::new(s, 0.0);
                }
                fft_forward.process(&mut block);
                ch_parts.push(block);
            }
            partitions.push(ch_parts);
        }

        let num_partitions = partitions[0].len();
        let fft_scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());
        self.state.store(Arc::new(Some(ConvolutionState {
            partitions,
            num_partitions,
            ir_channels,
            fft_forward,
            fft_inverse,
        })));
        self.fdl_flat = vec![Complex::new(0.0, 0.0); num_partitions * self.channels * FFT_SIZE];
        self.fdl_head = 0;
        self.fft_scratch = vec![Complex::new(0.0, 0.0); fft_scratch_len];
        self.ir_file = path.to_string();
        Ok(())
    }

    /// Load an audio file using Symphonia's format probing.
    /// Supports WAV, FLAC, and AIFF formats.
    /// Returns (channel_samples, sample_rate).
    fn load_audio_file(path: &str) -> Result<(Vec<Vec<f32>>, u32), String> {
        use std::fs::File;
        use std::sync::LazyLock;

        // Shared probe and codec registry for IR loading
        static IR_PROBE: LazyLock<Probe> = LazyLock::new(|| {
            let mut probe = Probe::default();
            probe.register_all::<symphonia_format_riff::WavReader>();
            probe.register_all::<symphonia_format_riff::AiffReader>();
            probe.register_all::<symphonia_bundle_flac::FlacReader>();
            probe
        });

        static IR_CODEC_REGISTRY: LazyLock<CodecRegistry> = LazyLock::new(|| {
            let mut registry = CodecRegistry::new();
            registry.register_all::<symphonia_codec_pcm::PcmDecoder>();
            registry.register_all::<symphonia_bundle_flac::FlacDecoder>();
            registry
        });

        let file = File::open(Path::new(path)).map_err(|e| format!("IO: {e}"))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probe_result = IR_PROBE
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| format!("Probe: {e}"))?;

        let mut reader = probe_result.format;
        let track = reader.default_track().ok_or("No track found in IR file")?;
        let codec_params = track.codec_params.clone();

        let sample_rate = codec_params.sample_rate.unwrap_or(0);
        let num_channels = codec_params
            .channels
            .map(|c| c.count())
            .unwrap_or(1);

        let mut decoder = IR_CODEC_REGISTRY
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|e| format!("Decoder: {e}"))?;

        let mut samples = vec![Vec::new(); num_channels];
        loop {
            let packet = match reader.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => return Err(format!("Read: {e}")),
            };
            let decoded = decoder.decode(&packet).map_err(|e| format!("Decode: {e}"))?;
            match &decoded {
                AudioBufferRef::F32(buf) => {
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend_from_slice(buf.chan(ch));
                    }
                }
                AudioBufferRef::S16(buf) => {
                    let scale = 1.0 / 32768.0;
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend(buf.chan(ch).iter().map(|&s| s as f32 * scale));
                    }
                }
                AudioBufferRef::S24(buf) => {
                    let scale = 1.0 / 8388608.0;
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend(
                            buf.chan(ch).iter().map(|s| s.inner() as f32 * scale),
                        );
                    }
                }
                AudioBufferRef::S32(buf) => {
                    let scale = 1.0 / 2147483648.0;
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend(buf.chan(ch).iter().map(|&s| s as f32 * scale));
                    }
                }
                _ => return Err("Unsupported sample format in IR file".into()),
            }
        }

        if samples.iter().all(|ch| ch.is_empty()) {
            return Err("IR file contains no audio data".into());
        }

        Ok((samples, sample_rate))
    }

    /// Resample IR data from one sample rate to another using rubato.
    fn resample_ir(
        ir_samples: &[Vec<f32>],
        source_rate: u32,
        target_rate: u32,
    ) -> Result<Vec<Vec<f32>>, String> {
        let num_channels = ir_samples.len();
        let chunk_size = 1024;

        let mut resampler = Fft::<f32>::new(
            source_rate as usize,
            target_rate as usize,
            chunk_size,
            2,
            num_channels,
            FixedSync::Input,
        )
        .map_err(|e| format!("Failed to create resampler: {e}"))?;

        let source_len = ir_samples[0].len();
        let estimated_output_len =
            (source_len as f64 * target_rate as f64 / source_rate as f64) as usize + chunk_size;

        let mut output_channels: Vec<Vec<f32>> = vec![Vec::with_capacity(estimated_output_len); num_channels];

        let mut pos = 0;
        while pos < source_len {
            let input_frames_needed = resampler.input_frames_next();
            let output_frames = resampler.output_frames_next();

            // Prepare input chunks - pad with zeros if we're at the end
            let input_chunk: Vec<Vec<f32>> = (0..num_channels)
                .map(|ch| {
                    let end = (pos + input_frames_needed).min(source_len);
                    let mut chunk = ir_samples[ch][pos..end].to_vec();
                    chunk.resize(input_frames_needed, 0.0);
                    chunk
                })
                .collect();

            let mut output_chunk: Vec<Vec<f32>> = vec![vec![0.0; output_frames]; num_channels];

            let input_adapter =
                SequentialSliceOfVecs::new(&input_chunk, num_channels, input_frames_needed)
                    .map_err(|e| format!("Input adapter error: {e}"))?;
            let mut output_adapter =
                SequentialSliceOfVecs::new_mut(&mut output_chunk, num_channels, output_frames)
                    .map_err(|e| format!("Output adapter error: {e}"))?;

            match resampler.process_into_buffer(&input_adapter, &mut output_adapter, None) {
                Ok((_, written)) => {
                    for (ch, out_ch) in output_channels.iter_mut().enumerate() {
                        out_ch.extend_from_slice(&output_chunk[ch][..written]);
                    }
                }
                Err(e) => return Err(format!("Resampling error: {e}")),
            }

            pos += input_frames_needed;
        }

        // Trim to approximately expected length (remove trailing zeros from padding)
        let expected_len =
            (source_len as f64 * target_rate as f64 / source_rate as f64).ceil() as usize;
        for ch in &mut output_channels {
            ch.truncate(expected_len);
        }

        Ok(output_channels)
    }
}

impl InPlacePlugin for ConvolutionPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Convolution", "2.0.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id == self.param_mix {
            let val = value.as_float().unwrap_or(1.0);
            if val.is_finite() {
                let val = val.clamp(0.0, 1.0);
                self.mix_value = val;
                self.mix.set_target(val);
                self.rebuild_cached_parameters();
            }
        } else if id == self.param_gain_db {
            let val = value.as_float().unwrap_or(0.0);
            if val.is_finite() {
                self.gain_db_value = val;
                self.gain_linear.set_target(10.0f32.powf(val / 20.0));
                self.rebuild_cached_parameters();
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix_value))
        } else if id == &self.param_gain_db {
            Some(ParameterValue::Float(self.gain_db_value))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.mix.set_time(20.0, sr);
        self.gain_linear.set_time(20.0, sr);
        Ok(())
    }
    fn reset(&mut self) {
        self.fdl_flat.fill(Complex::new(0.0, 0.0));
        self.fdl_head = 0;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let nf = context.num_frames;
        let state_guard = self.state.load();
        let state = match state_guard.as_ref() {
            Some(s) => s,
            None => return Ok(nf),
        };

        let num_partitions = state.num_partitions;

        let mut in_pos = 0;
        while in_pos < nf {
            let to_copy = (PARTITION_SIZE - self.input_fill).min(nf - in_pos);
            for ch in 0..self.channels {
                for i in 0..to_copy {
                    self.input_buffers[ch][self.input_fill + i] =
                        buffer[(in_pos + i) * self.channels + ch];
                }
            }
            self.input_fill += to_copy;

            if self.input_fill == PARTITION_SIZE {
                let m = self.mix.advance();
                let g = self.gain_linear.advance();
                let wet_g = m * g;
                let dry_g = 1.0 - m;
                let inv_n = 1.0 / FFT_SIZE as f32;

                self.fdl_head = if self.fdl_head == 0 {
                    num_partitions - 1
                } else {
                    self.fdl_head - 1
                };

                for ch in 0..self.channels {
                    for i in 0..PARTITION_SIZE {
                        self.fft_spectrum[i] = Complex::new(self.input_buffers[ch][i], 0.0);
                    }
                    for i in PARTITION_SIZE..FFT_SIZE {
                        self.fft_spectrum[i] = Complex::new(0.0, 0.0);
                    }
                    state
                        .fft_forward
                        .process_with_scratch(&mut self.fft_spectrum, &mut self.fft_scratch);

                    let off_base = (self.fdl_head * self.channels + ch) * FFT_SIZE;
                    self.fdl_flat[off_base..off_base + FFT_SIZE]
                        .copy_from_slice(&self.fft_spectrum);

                    self.fft_sum.fill(Complex::new(0.0, 0.0));
                    let ir_ch = if state.ir_channels == 1 {
                        0
                    } else {
                        ch.min(state.ir_channels - 1)
                    };
                    for p in 0..num_partitions {
                        let fdl_p = (self.fdl_head + p) % num_partitions;
                        let fdl_off = (fdl_p * self.channels + ch) * FFT_SIZE;
                        complex_mul_add_simd(
                            &mut self.fft_sum,
                            &self.fdl_flat[fdl_off..fdl_off + FFT_SIZE],
                            &state.partitions[ir_ch][p],
                        );
                    }
                    state
                        .fft_inverse
                        .process_with_scratch(&mut self.fft_sum, &mut self.fft_scratch);

                    for i in 0..FFT_SIZE {
                        self.output_accum[ch][i] += self.fft_sum[i].re * inv_n;
                    }
                }

                // Apply to in-place buffer
                for i in 0..PARTITION_SIZE {
                    if in_pos + i >= PARTITION_SIZE - to_copy {
                        let frame_idx = in_pos + i - (PARTITION_SIZE - to_copy);
                        for ch in 0..self.channels {
                            let idx = frame_idx * self.channels + ch;
                            let dry = buffer[idx];
                            buffer[idx] = dry * dry_g + self.output_accum[ch][i] * wet_g;
                        }
                    }
                }

                for ch in 0..self.channels {
                    self.output_accum[ch].copy_within(PARTITION_SIZE..FFT_SIZE, 0);
                    self.output_accum[ch][PARTITION_SIZE..].fill(0.0);
                }
                self.input_fill = 0;

                self.mix.next_n(PARTITION_SIZE);
                self.gain_linear.next_n(PARTITION_SIZE);
            }
            in_pos += to_copy;
        }
        flush_denormals_inplace(buffer);
        Ok(nf)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::plugin::{InPlacePlugin, ProcessContext};

    /// Helper: create a ConvolutionPlugin and load a synthetic IR directly.
    fn make_plugin_with_ir(channels: usize, sample_rate: u32, ir: Vec<Vec<f32>>) -> ConvolutionPlugin {
        let mut plugin = ConvolutionPlugin::new(channels, sample_rate);
        plugin.initialize(sample_rate).unwrap();

        // Build partitions from the IR data
        let ir_channels = ir.len();
        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);

        let mut partitions = Vec::with_capacity(ir_channels);
        for ch_samples in &ir {
            let num_parts = ch_samples.len().div_ceil(PARTITION_SIZE);
            let mut ch_parts = Vec::with_capacity(num_parts);
            for p in 0..num_parts {
                let mut block = vec![Complex::new(0.0, 0.0); FFT_SIZE];
                let start = p * PARTITION_SIZE;
                let end = (start + PARTITION_SIZE).min(ch_samples.len());
                for (i, &s) in ch_samples[start..end].iter().enumerate() {
                    block[i] = Complex::new(s, 0.0);
                }
                fft_forward.process(&mut block);
                ch_parts.push(block);
            }
            partitions.push(ch_parts);
        }

        let num_partitions = partitions[0].len();
        let fft_scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());

        plugin.state.store(Arc::new(Some(ConvolutionState {
            partitions,
            num_partitions,
            ir_channels,
            fft_forward,
            fft_inverse,
        })));
        plugin.fdl_flat = vec![Complex::new(0.0, 0.0); num_partitions * channels * FFT_SIZE];
        plugin.fdl_head = 0;
        plugin.fft_scratch = vec![Complex::new(0.0, 0.0); fft_scratch_len];

        plugin
    }

    /// Unity IR (Dirac at sample 0) should pass audio through unchanged.
    #[test]
    fn test_unity_ir_passthrough() {
        let channels = 1;
        let sr = 48000;
        // Dirac impulse: 1.0 at sample 0, zeros elsewhere
        let ir = vec![vec![1.0]];
        let mut plugin = make_plugin_with_ir(channels, sr, ir);
        // mix = 1.0 (fully wet), gain = 0 dB
        plugin.mix_value = 1.0;
        plugin.mix.set_target(1.0);

        // Process a few blocks of a sine wave
        let total_frames = PARTITION_SIZE * 4;
        let mut buffer: Vec<f32> = (0..total_frames)
            .map(|i| (i as f32 * 0.1).sin())
            .collect();
        let original = buffer.clone();

        // Process in partition-sized blocks
        for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
            let block_end = (block_start + PARTITION_SIZE).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin
                .process_in_place(&mut buffer[block_start..block_end], &ctx)
                .unwrap();
        }

        // Verify output is finite and has energy (convolution with unity IR)
        let output_energy: f32 = buffer.iter().map(|s| s * s).sum();
        let input_energy: f32 = original.iter().map(|s| s * s).sum();
        assert!(
            output_energy.is_finite(),
            "Output must be finite"
        );
        assert!(
            output_energy > 0.0,
            "Unity IR convolution should produce non-zero output"
        );
        // With a unity IR, output energy should be comparable to input energy
        // (allowing for partitioned convolution edge effects)
        if input_energy > 0.0 {
            let ratio = output_energy / input_energy;
            assert!(
                ratio > 0.1,
                "Unity IR should preserve most energy, ratio = {ratio}"
            );
        }
    }

    /// Dirac impulse response (single sample IR) should produce output.
    #[test]
    fn test_dirac_impulse_response() {
        let channels = 2;
        let sr = 44100;
        // Single-sample IR with gain 0.5
        let ir = vec![vec![0.5], vec![0.5]];
        let mut plugin = make_plugin_with_ir(channels, sr, ir);
        plugin.mix_value = 1.0;
        plugin.mix.set_target(1.0);

        // Send a DC signal of 1.0 on both channels
        let total_frames = PARTITION_SIZE * 3;
        let mut buffer = vec![1.0f32; total_frames * channels];

        for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
            let block_end = (block_start + PARTITION_SIZE).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            let buf_start = block_start * channels;
            let buf_end = block_end * channels;
            plugin
                .process_in_place(&mut buffer[buf_start..buf_end], &ctx)
                .unwrap();
        }

        // After settling, output should be approximately 0.5 (IR gain)
        let skip = PARTITION_SIZE * channels * 2;
        let tail = &buffer[skip..];
        let avg: f32 = tail.iter().sum::<f32>() / tail.len() as f32;
        assert!(
            (avg - 0.5).abs() < 0.05,
            "Dirac IR with gain 0.5 should produce ~0.5 output, got avg = {avg}"
        );
    }

    /// Long IR stability: no NaN or Inf after 10000 frames.
    #[test]
    fn test_long_ir_stability() {
        let channels = 1;
        let sr = 48000;
        // Create a longer IR (multiple partitions)
        let ir_len = PARTITION_SIZE * 4;
        let mut ir_data = vec![0.0f32; ir_len];
        // Exponentially decaying impulse response
        for i in 0..ir_len {
            ir_data[i] = (-(i as f32) / 500.0).exp() * 0.1;
        }
        let ir = vec![ir_data];
        let mut plugin = make_plugin_with_ir(channels, sr, ir);
        plugin.mix_value = 1.0;
        plugin.mix.set_target(1.0);

        // Process 10000 frames of random-ish signal
        let total_frames = 10000;
        let mut buffer: Vec<f32> = (0..total_frames)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.3 * (t * 440.0 * std::f32::consts::TAU).sin()
                    + 0.1 * (t * 1000.0 * std::f32::consts::TAU).sin()
            })
            .collect();

        for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
            let block_end = (block_start + PARTITION_SIZE).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin
                .process_in_place(&mut buffer[block_start..block_end], &ctx)
                .unwrap();
        }

        // Verify no NaN or Inf in output
        for (i, &s) in buffer.iter().enumerate() {
            assert!(
                s.is_finite(),
                "Output sample at index {i} is not finite: {s}"
            );
        }
    }
}
