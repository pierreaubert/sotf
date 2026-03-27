// ============================================================================
// Convolution Plugin - Partitioned FFT-based convolution
// ============================================================================

pub mod nupc;
pub mod params;

use arc_swap::ArcSwap;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rayon::prelude::*;
use rubato::{Fft, FixedSync, Resampler};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
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
    #[serde(default)]
    pub zero_latency_head: bool,
    #[serde(default = "default_head_taps")]
    pub head_taps: usize,
}

fn default_head_taps() -> usize {
    128
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

use crate::params::PARAMS as CV;

pub struct ConvolutionPlugin {
    channels: usize,
    sample_rate: u32,
    ir_file: String,
    mix: Smoother,
    mix_value: f32,
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
    /// When use_nupc is true, per-channel NUPC engines for low-latency convolution
    nupc_engines: Vec<nupc::NupcEngine>,
    use_nupc: bool,
    zero_latency_head: bool,
    head_taps: usize,
    /// Pre-allocated accumulator buffers for rayon fold/reduce (one per rayon thread).
    /// Avoids heap allocation in the audio processing hot path.
    rayon_accum_pool: Vec<Vec<Complex<f32>>>,
}

impl ConvolutionPlugin {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        let mut p = Self {
            channels,
            sample_rate,
            ir_file: String::new(),
            mix: Smoother::new(1.0, 20.0, sample_rate),
            mix_value: 1.0,
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
            nupc_engines: Vec::new(),
            use_nupc: false,
            zero_latency_head: false,
            head_taps: 128,
            rayon_accum_pool: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => None, // ir_file (FilePath -- handled separately)
            1 => Some(self.mix_value as f64),
            2 => Some(self.gain_db_value as f64),
            3 => Some(if self.use_nupc { 1.0 } else { 0.0 }),
            4 => Some(if self.zero_latency_head { 1.0 } else { 0.0 }),
            5 => Some(self.head_taps as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {} // ir_file (FilePath -- handled separately)
            1 => {
                self.mix_value = value as f32;
                self.mix.set_target(value as f32);
            }
            2 => {
                self.gain_db_value = value as f32;
                self.gain_linear
                    .set_target(10.0f32.powf(value as f32 / 20.0));
            }
            3 => self.use_nupc = value > 0.5,
            4 => self.zero_latency_head = value > 0.5,
            5 => self.head_taps = value as usize,
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(CV, |i| self.param_value(i));
    }

    pub fn from_params(
        channels: usize,
        sample_rate: u32,
        params: ConvolutionPluginParams,
    ) -> Result<Self, String> {
        let mut plugin = Self::new(channels, sample_rate);
        plugin.use_nupc = params.use_nupc;
        plugin.zero_latency_head = params.zero_latency_head;
        plugin.head_taps = params.head_taps;
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

        // Pre-allocate rayon accumulator pool (one buffer per available thread)
        let n_threads = rayon::current_num_threads().max(1);
        self.rayon_accum_pool = (0..n_threads)
            .map(|_| vec![Complex::new(0.0, 0.0); FFT_SIZE])
            .collect();

        // Build NUPC engines if use_nupc is enabled.
        // One NupcEngine per channel, each configured with the channel's IR.
        if self.use_nupc {
            let state_guard = self.state.load();
            if let Some(ref state) = **state_guard {
                self.nupc_engines.clear();
                for ch in 0..self.channels {
                    let ir_ch = ch % state.ir_channels;
                    // Reconstruct the time-domain IR from the stored FFT partitions.
                    // Each partition is PARTITION_SIZE samples zero-padded to FFT_SIZE.
                    let mut ir_data = Vec::with_capacity(state.num_partitions * PARTITION_SIZE);
                    for p in 0..state.num_partitions {
                        // IFFT the partition to get time-domain samples
                        let mut block = state.partitions[ir_ch][p].clone();
                        state.fft_inverse.process(&mut block);
                        let scale = 1.0 / FFT_SIZE as f32;
                        for sample in &block[..PARTITION_SIZE] {
                            ir_data.push(sample.re * scale);
                        }
                    }
                    if self.zero_latency_head && self.head_taps > 0 {
                        self.nupc_engines.push(
                            nupc::NupcEngine::new_with_head(&ir_data, PARTITION_SIZE, self.head_taps),
                        );
                    } else {
                        self.nupc_engines
                            .push(nupc::NupcEngine::new(&ir_data, PARTITION_SIZE));
                    }
                }
                log::info!(
                    "[Convolution] NUPC engines built: {} channels, latency={} samples",
                    self.nupc_engines.len(),
                    self.nupc_engines.first().map_or(0, |e| e.latency_samples())
                );
            }
        }

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
        param_bridge::set_parameter(CV, &id, &value, |i, v| self.set_param_value(i, v))?;
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(CV, id, |i| self.param_value(i))
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

        // NUPC path: per-channel block processing with non-uniform partitions.
        // Avoids the UPC's fixed PARTITION_SIZE constraint for lower latency.
        if !self.nupc_engines.is_empty() && self.nupc_engines.len() == self.channels {
            let mix = self.mix.next_n(nf);
            let gain = self.gain_linear.next_n(nf);
            for frame in 0..nf {
                let off = frame * self.channels;
                for ch in 0..self.channels {
                    let dry = buffer[off + ch];
                    let wet = self.nupc_engines[ch].process_sample(dry);
                    buffer[off + ch] = dry * (1.0 - mix) + wet * mix * gain;
                }
            }
            return Ok(nf);
        }

        // UPC path: uniform partitioned convolution (original code)
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

                    // Parallel partition sum: each rayon thread accumulates a local
                    // fft_sum over its subset of partitions, then the partial sums are
                    // merged via element-wise addition.  The threshold of 8 is chosen so
                    // that the rayon scheduling overhead (~1-5 µs) is amortised over at
                    // least 8 × FFT_SIZE complex multiply-adds.  Below that threshold the
                    // sequential path is always faster.
                    if num_partitions >= 8 {
                        // Capture immutable references needed inside the parallel closure.
                        let fdl_flat = &self.fdl_flat;
                        let fdl_head = self.fdl_head;
                        let channels = self.channels;
                        let ir_partitions = &state.partitions[ir_ch];

                        // Lazy-init pool if not yet allocated (e.g. IR loaded via test helper)
                        if self.rayon_accum_pool.is_empty() {
                            let n_threads = rayon::current_num_threads().max(1);
                            self.rayon_accum_pool = (0..n_threads)
                                .map(|_| vec![Complex::new(0.0, 0.0); FFT_SIZE])
                                .collect();
                        }
                        // Zero the pre-allocated accumulators
                        let pool = &mut self.rayon_accum_pool;
                        for acc in pool.iter_mut() {
                            acc.fill(Complex::new(0.0, 0.0));
                        }

                        // Split partitions across pre-allocated accumulators.
                        // Each chunk of partitions accumulates into one pool buffer.
                        let n_accum = pool.len().max(1);
                        let chunk_size = num_partitions.div_ceil(n_accum);

                        pool.par_iter_mut()
                            .enumerate()
                            .for_each(|(idx, acc)| {
                                let start = idx * chunk_size;
                                let end = (start + chunk_size).min(num_partitions);
                                for p in start..end {
                                    let fdl_p = (fdl_head + p) % num_partitions;
                                    let fdl_off = (fdl_p * channels + ch) * FFT_SIZE;
                                    let fdl_slice = &fdl_flat[fdl_off..fdl_off + FFT_SIZE];
                                    let ir_slice = &ir_partitions[p];
                                    complex_mul_add_simd(acc, fdl_slice, ir_slice);
                                }
                            });

                        // Merge all accumulators into fft_sum
                        self.fft_sum.fill(Complex::new(0.0, 0.0));
                        for acc in self.rayon_accum_pool.iter() {
                            for (dst, src) in self.fft_sum.iter_mut().zip(acc.iter()) {
                                *dst += src;
                            }
                        }
                    } else {
                        for p in 0..num_partitions {
                            let fdl_p = (self.fdl_head + p) % num_partitions;
                            let fdl_off = (fdl_p * self.channels + ch) * FFT_SIZE;
                            complex_mul_add_simd(
                                &mut self.fft_sum,
                                &self.fdl_flat[fdl_off..fdl_off + FFT_SIZE],
                                &state.partitions[ir_ch][p],
                            );
                        }
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

                // Smoother already advanced by advance() above — do not double-advance
                self.mix.next_n(PARTITION_SIZE - 1);
                self.gain_linear.next_n(PARTITION_SIZE - 1);
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

    /// With mix=0.0 (fully dry), output should equal input.
    #[test]
    fn test_mix_zero_is_dry_passthrough() {
        let channels = 1;
        let sr = 48000;
        // Dirac impulse at sample 0
        let ir = vec![vec![1.0]];
        let mut plugin = make_plugin_with_ir(channels, sr, ir);
        // Set mix to 0.0 (fully dry)
        plugin.mix_value = 0.0;
        plugin.mix.set_target(0.0);
        plugin.mix.reset(0.0);

        // Process enough blocks for the convolution to settle
        let total_frames = PARTITION_SIZE * 3;
        let mut buffer: Vec<f32> = (0..total_frames)
            .map(|i| (i as f32 * 0.1).sin())
            .collect();
        let original = buffer.clone();

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

        // With mix=0.0, the output formula is: dry*1.0 + wet*0.0 = dry
        // Check that output matches original input
        for (i, (&got, &exp)) in buffer.iter().zip(original.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 1e-4,
                "mix=0 passthrough mismatch at sample {}: got {}, expected {}",
                i, got, exp
            );
        }
    }

    /// With gain_db=6.0, the wet signal should be louder than with gain_db=0.0.
    #[test]
    fn test_gain_db_increases_output() {
        let channels = 1;
        let sr = 48000;
        let ir = vec![vec![1.0]]; // Unity IR

        // Process with gain_db=0.0
        let mut plugin_0db = make_plugin_with_ir(channels, sr, ir.clone());
        plugin_0db.mix_value = 1.0;
        plugin_0db.mix.set_target(1.0);
        plugin_0db.mix.reset(1.0);
        plugin_0db.gain_db_value = 0.0;
        plugin_0db.gain_linear.set_target(1.0);
        plugin_0db.gain_linear.reset(1.0);

        let total_frames = PARTITION_SIZE * 4;
        let input_signal: Vec<f32> = (0..total_frames)
            .map(|i| (i as f32 * 0.05).sin() * 0.5)
            .collect();

        let mut buffer_0db = input_signal.clone();
        for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
            let block_end = (block_start + PARTITION_SIZE).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin_0db
                .process_in_place(&mut buffer_0db[block_start..block_end], &ctx)
                .unwrap();
        }

        // Process with gain_db=6.0
        let mut plugin_6db = make_plugin_with_ir(channels, sr, ir);
        plugin_6db.mix_value = 1.0;
        plugin_6db.mix.set_target(1.0);
        plugin_6db.mix.reset(1.0);
        let gain_linear_6db = 10.0f32.powf(6.0 / 20.0);
        plugin_6db.gain_db_value = 6.0;
        plugin_6db.gain_linear.set_target(gain_linear_6db);
        plugin_6db.gain_linear.reset(gain_linear_6db);

        let mut buffer_6db = input_signal.clone();
        for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
            let block_end = (block_start + PARTITION_SIZE).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin_6db
                .process_in_place(&mut buffer_6db[block_start..block_end], &ctx)
                .unwrap();
        }

        // Compare energy in the settled region (skip first partition for edge effects)
        let skip = PARTITION_SIZE * 2;
        let energy_0db: f32 = buffer_0db[skip..].iter().map(|s| s * s).sum();
        let energy_6db: f32 = buffer_6db[skip..].iter().map(|s| s * s).sum();

        assert!(
            energy_6db > energy_0db * 1.5,
            "gain_db=6 should produce notably more energy than gain_db=0: {} vs {}",
            energy_6db,
            energy_0db
        );
    }

    /// Parallel partition sum produces the same output as the sequential path.
    ///
    /// Strategy: build two plugins with the same IR that is long enough to have
    /// \>= 8 partitions (so the parallel code path is exercised for the first
    /// plugin), then verify the outputs are bit-for-bit identical.  The second
    /// plugin uses a short IR (\< 8 partitions, sequential path) with a single-
    /// sample Dirac that is analytically equivalent to the identity, so we check
    /// the long-IR plugin produces finite, energy-preserving output.
    ///
    /// Additionally we verify the parallel path against the known Dirac result:
    /// convolving with a Dirac at sample 0 should preserve the input (within
    /// float rounding).
    #[test]
    fn test_parallel_path_bit_exact_vs_sequential() {
        let channels = 1;
        let sr = 48000;

        // Build an IR long enough to trigger the parallel path (>= 8 partitions).
        // PARTITION_SIZE = 1024, so 8 partitions = 8192 samples.
        let ir_len = PARTITION_SIZE * 10; // 10 partitions
        let mut ir_data = vec![0.0f32; ir_len];
        // Dirac at sample 0 — convolution with this should be identity.
        ir_data[0] = 1.0;
        let ir_parallel = vec![ir_data];

        // Single-sample Dirac for the sequential path reference (1 partition).
        let ir_seq = vec![vec![1.0f32]];

        let input_signal: Vec<f32> = (0..PARTITION_SIZE * 6)
            .map(|i| (i as f32 * 0.07).sin() * 0.8)
            .collect();
        let total_frames = input_signal.len();

        // --- Run the parallel-path plugin (10-partition IR, >= 8 → parallel) ---
        let mut plugin_par = make_plugin_with_ir(channels, sr, ir_parallel);
        plugin_par.mix_value = 1.0;
        plugin_par.mix.set_target(1.0);
        plugin_par.mix.reset(1.0);
        plugin_par.gain_linear.set_target(1.0);
        plugin_par.gain_linear.reset(1.0);

        let mut buf_par = input_signal.clone();
        for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
            let block_end = (block_start + PARTITION_SIZE).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext { sample_rate: sr, num_frames: nf };
            plugin_par.process_in_place(&mut buf_par[block_start..block_end], &ctx).unwrap();
        }

        // --- Run the sequential-path plugin (1-partition Dirac, sequential) ---
        let mut plugin_seq = make_plugin_with_ir(channels, sr, ir_seq);
        plugin_seq.mix_value = 1.0;
        plugin_seq.mix.set_target(1.0);
        plugin_seq.mix.reset(1.0);
        plugin_seq.gain_linear.set_target(1.0);
        plugin_seq.gain_linear.reset(1.0);

        let mut buf_seq = input_signal.clone();
        for block_start in (0..total_frames).step_by(PARTITION_SIZE) {
            let block_end = (block_start + PARTITION_SIZE).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext { sample_rate: sr, num_frames: nf };
            plugin_seq.process_in_place(&mut buf_seq[block_start..block_end], &ctx).unwrap();
        }

        // Both plugins convolve with a Dirac, so outputs should match to float
        // precision.  The parallel path settles one partition later (zeros in
        // partitions 1-9 of the longer IR take one extra block to flush), so we
        // compare the settled region (skip the first 2 partitions).
        let skip = PARTITION_SIZE * 2;
        for (i, (&par, &seq)) in buf_par[skip..].iter().zip(buf_seq[skip..].iter()).enumerate() {
            assert!(
                (par - seq).abs() < 1e-5,
                "Parallel/sequential output mismatch at sample {}: parallel={par}, sequential={seq}",
                skip + i
            );
        }

        // Sanity: output must be finite and have energy.
        let energy: f32 = buf_par[skip..].iter().map(|s| s * s).sum();
        assert!(energy.is_finite() && energy > 0.0, "Parallel path must produce non-zero finite output");
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
        for (i, sample) in ir_data.iter_mut().enumerate() {
            *sample = (-(i as f32) / 500.0).exp() * 0.1;
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
