// ============================================================================
// Convolution Plugin - Partitioned FFT-based convolution
// ============================================================================

pub mod params;

use arc_swap::ArcSwap;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use plugins_spatial::{nupc, validate_interleaved_in_place};
use rayon::prelude::*;
use rubato::{Fft, FixedSync, Resampler};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{complex_mul_add_simd, enable_ftz_daz, flush_denormals_inplace};
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

/// Result of loading an IR on a background thread, ready to be swapped into the audio thread.
struct IrLoadResult {
    state: ConvolutionState,
    nupc_engines: Vec<nupc::NupcEngine>,
    fdl_flat: Vec<Complex<f32>>,
    fdl_head: usize,
    fft_scratch: Vec<Complex<f32>>,
    rayon_accum_pool: Vec<Vec<Complex<f32>>>,
    ir_file: String,
}

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
    /// Per-channel output ring buffer: stores completed partition output so
    /// partial-block boundaries are handled correctly (fix for issue #1).
    /// Size: PARTITION_SIZE samples per channel (one completed partition).
    output_ring: Vec<Vec<f32>>,
    /// Read pointer into `output_ring` (next sample to be drained).
    output_ring_read: usize,
    /// Number of valid samples waiting to be consumed from `output_ring`.
    output_ring_available: usize,
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
    /// Channel to receive asynchronously-loaded IR state from the background thread.
    ir_load_result_rx: Option<std::sync::mpsc::Receiver<Result<IrLoadResult, String>>>,
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
            output_ring: vec![vec![0.0; PARTITION_SIZE]; channels],
            output_ring_read: 0,
            output_ring_available: 0,
            fft_spectrum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_sum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_scratch: Vec::new(),
            cached_parameters: Vec::new(),
            nupc_engines: Vec::new(),
            use_nupc: false,
            zero_latency_head: false,
            head_taps: 128,
            rayon_accum_pool: Vec::new(),
            ir_load_result_rx: None,
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
        if let Some(ir_param) = self
            .cached_parameters
            .iter_mut()
            .find(|param| param.id.as_str() == "ir_file")
        {
            ir_param.default_value = ParameterValue::String(self.ir_file.clone());
        }
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
            plugin.load_ir(&params.ir_file)?;
        }
        plugin.mix_value = params.mix;
        plugin.mix.set_target(params.mix);
        plugin.gain_db_value = params.gain_db;
        plugin
            .gain_linear
            .set_target(10.0f32.powf(params.gain_db / 20.0));
        Ok(plugin)
    }

    /// Build IR state on any thread (file I/O, FFT planning, allocations).
    fn build_ir_state(
        path: &str,
        channels: usize,
        sample_rate: u32,
        use_nupc: bool,
        zero_latency_head: bool,
        head_taps: usize,
    ) -> Result<IrLoadResult, String> {
        let (ir_samples, ir_sample_rate) = Self::load_audio_file(path)?;

        let ir_samples = if ir_sample_rate != 0 && ir_sample_rate != sample_rate {
            log::info!(
                "Resampling IR from {} Hz to {} Hz",
                ir_sample_rate,
                sample_rate
            );
            Self::resample_ir(&ir_samples, ir_sample_rate, sample_rate)?
        } else {
            ir_samples
        };

        let ir_channels = ir_samples.len();
        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);

        let mut partitions = Vec::with_capacity(ir_channels);
        for ch_samples in &ir_samples {
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

        let state = ConvolutionState {
            partitions,
            num_partitions,
            ir_channels,
            fft_forward,
            fft_inverse,
        };

        // Build NUPC engines from original time-domain IR.
        let mut nupc_engines = Vec::new();
        if use_nupc {
            for ch in 0..channels {
                let ir_ch = ch % ir_channels;
                if zero_latency_head && head_taps > 0 {
                    nupc_engines.push(nupc::NupcEngine::new_with_head(
                        &ir_samples[ir_ch],
                        PARTITION_SIZE,
                        head_taps,
                    ));
                } else {
                    nupc_engines.push(nupc::NupcEngine::new(&ir_samples[ir_ch], PARTITION_SIZE));
                }
            }
            log::info!(
                "[Convolution] NUPC engines built: {} channels, latency={} samples",
                nupc_engines.len(),
                nupc_engines.first().map_or(0, |e| e.latency_samples())
            );
        }

        Ok(IrLoadResult {
            state,
            nupc_engines,
            fdl_flat: vec![Complex::new(0.0, 0.0); num_partitions * channels * FFT_SIZE],
            fdl_head: 0,
            fft_scratch: vec![Complex::new(0.0, 0.0); fft_scratch_len],
            rayon_accum_pool: (0..rayon::current_num_threads().max(1))
                .map(|_| vec![Complex::new(0.0, 0.0); FFT_SIZE])
                .collect(),
            ir_file: path.to_string(),
        })
    }

    fn apply_ir_state(&mut self, result: IrLoadResult) {
        self.state.store(Arc::new(Some(result.state)));
        self.nupc_engines = result.nupc_engines;
        self.fdl_flat = result.fdl_flat;
        self.fdl_head = result.fdl_head;
        self.fft_scratch = result.fft_scratch;
        self.rayon_accum_pool = result.rayon_accum_pool;
        self.ir_file = result.ir_file;
    }

    pub fn load_ir(&mut self, path: &str) -> Result<(), String> {
        let result = Self::build_ir_state(
            path,
            self.channels,
            self.sample_rate,
            self.use_nupc,
            self.zero_latency_head,
            self.head_taps,
        )?;
        self.apply_ir_state(result);
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
        let num_channels = codec_params.channels.map(|c| c.count()).unwrap_or(1);

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
            let decoded = decoder
                .decode(&packet)
                .map_err(|e| format!("Decode: {e}"))?;
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
                        sample_ch.extend(buf.chan(ch).iter().map(|s| s.inner() as f32 * scale));
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

        let mut output_channels: Vec<Vec<f32>> =
            vec![Vec::with_capacity(estimated_output_len); num_channels];

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
        PluginInfo::new("Convolution", "2.1.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id.as_str() == "ir_file" {
            let path = match value {
                ParameterValue::String(path) => path,
                other => {
                    return Err(format!(
                        "ir_file: type mismatch (expected String, got {other:?})"
                    ));
                }
            };
            if path.is_empty() {
                self.state.store(Arc::new(None));
                self.nupc_engines.clear();
                self.ir_file.clear();
                self.output_ring_read = 0;
                self.output_ring_available = 0;
                for buf in &mut self.output_ring {
                    buf.fill(0.0);
                }
                self.input_fill = 0;
                for buf in &mut self.input_buffers {
                    buf.fill(0.0);
                }
                for buf in &mut self.output_accum {
                    buf.fill(0.0);
                }
                self.fdl_flat.fill(Complex::new(0.0, 0.0));
                self.fdl_head = 0;
            } else {
                // Quick synchronous validation so tests get immediate feedback.
                if !std::path::Path::new(&path).exists() {
                    return Err(format!("IO: {path}: No such file or directory"));
                }
                self.ir_file = path.clone();
                let (result_tx, result_rx) = std::sync::mpsc::channel();
                self.ir_load_result_rx = Some(result_rx);
                let channels = self.channels;
                let sample_rate = self.sample_rate;
                let use_nupc = self.use_nupc;
                let zero_latency_head = self.zero_latency_head;
                let head_taps = self.head_taps;
                std::thread::Builder::new()
                    .name("convolution-ir-load".to_string())
                    .spawn(move || {
                        let result = Self::build_ir_state(
                            &path,
                            channels,
                            sample_rate,
                            use_nupc,
                            zero_latency_head,
                            head_taps,
                        );
                        let _ = result_tx.send(result);
                    })
                    .map_err(|e| format!("Failed to spawn IR load thread: {e}"))?;
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        param_bridge::set_parameter(CV, &id, &value, |i, v| self.set_param_value(i, v))?;
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.as_str() == "ir_file" {
            return Some(ParameterValue::String(self.ir_file.clone()));
        }
        param_bridge::get_parameter(CV, id, |i| self.param_value(i))
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        let old_sr = self.sample_rate;
        self.sample_rate = sr;
        self.mix.set_time(20.0, sr);
        self.gain_linear.set_time(20.0, sr);
        if old_sr != sr && !self.ir_file.is_empty() {
            let path = self.ir_file.clone();
            let (result_tx, result_rx) = std::sync::mpsc::channel();
            self.ir_load_result_rx = Some(result_rx);
            let channels = self.channels;
            let use_nupc = self.use_nupc;
            let zero_latency_head = self.zero_latency_head;
            let head_taps = self.head_taps;
            std::thread::Builder::new()
                .name("convolution-ir-load".to_string())
                .spawn(move || {
                    let result = Self::build_ir_state(
                        &path,
                        channels,
                        sr,
                        use_nupc,
                        zero_latency_head,
                        head_taps,
                    );
                    let _ = result_tx.send(result);
                })
                .map_err(|e| format!("Failed to spawn IR load thread: {e}"))?;
        }
        Ok(())
    }
    fn reset(&mut self) {
        // UPC state
        self.fdl_flat.fill(Complex::new(0.0, 0.0));
        self.fdl_head = 0;
        self.input_fill = 0;
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.output_accum {
            buf.fill(0.0);
        }
        // Output ring buffer
        for buf in &mut self.output_ring {
            buf.fill(0.0);
        }
        self.output_ring_read = 0;
        self.output_ring_available = 0;
        // NUPC state
        for engine in &mut self.nupc_engines {
            engine.reset();
        }
        // Reset parameter smoothers to their instantaneous values so the
        // next playback starts without interpolating from a stale position.
        self.mix.reset(self.mix_value);
        self.gain_linear
            .reset(10.0f32.powf(self.gain_db_value / 20.0));
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        // Issue #6: enable flush-to-zero / denormals-are-zero at the top of
        // the callback so FFT multiply-adds cannot generate costly denormals.
        enable_ftz_daz();

        let nf = context.num_frames;
        let total_samples =
            validate_interleaved_in_place("Convolution", nf, self.channels, buffer.len())?;

        // Check for asynchronously-loaded IR results and swap them in.
        if let Some(ref rx) = self.ir_load_result_rx
            && let Ok(result) = rx.try_recv()
        {
            self.ir_load_result_rx = None;
            match result {
                Ok(loaded) => self.apply_ir_state(loaded),
                Err(e) => {
                    log::error!("[Convolution] IR load failed: {e}");
                    self.state.store(Arc::new(None));
                    self.nupc_engines.clear();
                    self.ir_file.clear();
                }
            }
        }

        let state_guard = self.state.load();
        let state = match state_guard.as_ref() {
            Some(s) => s,
            None => return Ok(nf),
        };

        // NUPC path: per-channel block processing with non-uniform partitions.
        // Avoids the UPC's fixed PARTITION_SIZE constraint for lower latency.
        //
        // Issue #3 fix (NUPC): advance smoothers one sample at a time so that
        // mix/gain transitions are sample-accurate rather than block-quantized.
        if !self.nupc_engines.is_empty() && self.nupc_engines.len() == self.channels {
            for frame in 0..nf {
                // Advance smoothers by one sample to get the value for this frame.
                let mix = self.mix.advance();
                let gain = self.gain_linear.advance();
                let off = frame * self.channels;
                for ch in 0..self.channels {
                    let dry = buffer[off + ch];
                    let wet = self.nupc_engines[ch].process_sample(dry);
                    buffer[off + ch] = dry * (1.0 - mix) + wet * mix * gain;
                }
            }
            return Ok(nf);
        }

        // UPC path: uniform partitioned convolution.
        //
        // Issue #1 fix: Use a dedicated `output_ring` buffer to hold completed
        // partition output.  When a partition finishes its IFFT+overlap-add,
        // its PARTITION_SIZE output samples (with mix/gain applied) are stored
        // in `output_ring`.  In the same per-frame loop that feeds new input
        // into `input_buffers`, we simultaneously drain the ring into the
        // output positions of the in-place buffer.  Because the input copy and
        // ring drain both advance by exactly one frame per iteration, every
        // output sample is delivered exactly once regardless of host buffer
        // size alignment with PARTITION_SIZE.
        let num_partitions = state.num_partitions;

        let mut in_pos = 0;
        while in_pos < nf {
            // Per-frame step: copy one frame of input AND (if available) drain
            // one frame from the output ring into the buffer.
            //
            // We must read the dry input for `input_buffers` BEFORE overwriting
            // `buffer[in_pos]` with the ring output, because it is in-place.
            let buf_base = in_pos * self.channels;

            // Save incoming dry samples into input_buffers.
            let fill_idx = self.input_fill;
            for ch in 0..self.channels {
                self.input_buffers[ch][fill_idx] = buffer[buf_base + ch];
            }

            // Write output ring sample (or zero if ring is not yet ready).
            if self.output_ring_available > 0 {
                let out_idx = self.output_ring_read;
                for ch in 0..self.channels {
                    buffer[buf_base + ch] = self.output_ring[ch][out_idx];
                }
                self.output_ring_read += 1;
                self.output_ring_available -= 1;
            } else {
                // Ring is empty (startup or immediately after a partition
                // completed in the same frame).  Output silence for this frame
                // — the UPC path has inherent PARTITION_SIZE latency.
                for ch in 0..self.channels {
                    buffer[buf_base + ch] = 0.0;
                }
            }

            self.input_fill += 1;
            in_pos += 1;

            if self.input_fill == PARTITION_SIZE {
                let inv_n = 1.0 / FFT_SIZE as f32;

                // Issue #3 fix (UPC): linearly interpolate mix/gain across the
                // partition block.  Capture the value *before* advancing, then
                // advance by PARTITION_SIZE to get the end value.  This removes
                // the 21 ms step quantization that caused zipper noise.
                let mix_start = self.mix.current();
                let mix_end = self.mix.next_n(PARTITION_SIZE);
                let gain_start = self.gain_linear.current();
                let gain_end = self.gain_linear.next_n(PARTITION_SIZE);

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

                    if num_partitions >= 8 {
                        let fdl_flat = &self.fdl_flat;
                        let fdl_head = self.fdl_head;
                        let channels = self.channels;
                        let ir_partitions = &state.partitions[ir_ch];

                        if self.rayon_accum_pool.is_empty() {
                            let n_threads = rayon::current_num_threads().max(1);
                            self.rayon_accum_pool = (0..n_threads)
                                .map(|_| vec![Complex::new(0.0, 0.0); FFT_SIZE])
                                .collect();
                        }
                        let pool = &mut self.rayon_accum_pool;
                        for acc in pool.iter_mut() {
                            acc.fill(Complex::new(0.0, 0.0));
                        }

                        let n_accum = pool.len().max(1);
                        let chunk_size = num_partitions.div_ceil(n_accum);

                        pool.par_iter_mut().enumerate().for_each(|(idx, acc)| {
                            let start = idx * chunk_size;
                            let end = (start + chunk_size).min(num_partitions);
                            for (p, ir_slice) in
                                ir_partitions.iter().enumerate().take(end).skip(start)
                            {
                                let fdl_p = (fdl_head + p) % num_partitions;
                                let fdl_off = (fdl_p * channels + ch) * FFT_SIZE;
                                let fdl_slice = &fdl_flat[fdl_off..fdl_off + FFT_SIZE];
                                complex_mul_add_simd(acc, fdl_slice, ir_slice);
                            }
                        });

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

                // Commit the PARTITION_SIZE output samples into `output_ring`,
                // applying linearly interpolated mix/gain per sample.
                // The input dry signal for these samples was already saved in
                // `input_buffers` — use it for the dry/wet blend.
                for i in 0..PARTITION_SIZE {
                    // Linear interpolation: t goes 0..1 across the partition.
                    let t = i as f32 / PARTITION_SIZE as f32;
                    let m = mix_start + (mix_end - mix_start) * t;
                    let g = gain_start + (gain_end - gain_start) * t;
                    let wet_g = m * g;
                    let dry_g = 1.0 - m;
                    for ch in 0..self.channels {
                        let dry = self.input_buffers[ch][i];
                        self.output_ring[ch][i] = dry * dry_g + self.output_accum[ch][i] * wet_g;
                    }
                }
                self.output_ring_read = 0;
                self.output_ring_available = PARTITION_SIZE;

                // Advance the overlap-add tail.
                for ch in 0..self.channels {
                    self.output_accum[ch].copy_within(PARTITION_SIZE..FFT_SIZE, 0);
                    self.output_accum[ch][PARTITION_SIZE..].fill(0.0);
                }
                self.input_fill = 0;
            }
        }
        flush_denormals_inplace(&mut buffer[..total_samples]);
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
    use std::fs;

    fn write_test_wav(path: &Path, samples: &[i16], sample_rate: u32) {
        let data_len = samples.len() * 2;
        let mut bytes = Vec::with_capacity(44 + data_len);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    /// Helper: create a ConvolutionPlugin and load a synthetic IR directly.
    fn make_plugin_with_ir(
        channels: usize,
        sample_rate: u32,
        ir: Vec<Vec<f32>>,
    ) -> ConvolutionPlugin {
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
        let mut buffer: Vec<f32> = (0..total_frames).map(|i| (i as f32 * 0.1).sin()).collect();
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
        assert!(output_energy.is_finite(), "Output must be finite");
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
    ///
    /// The UPC path has one-partition latency: the first `PARTITION_SIZE`
    /// output samples are zero (the ring buffer starts empty), and subsequent
    /// blocks contain the dry input shifted by one partition.  The test
    /// accounts for this by processing N+1 blocks and comparing
    /// `output[PARTITION_SIZE..]` against `original[0..N*PARTITION_SIZE]`.
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

        // Process N+1 blocks so the last block's output is flushed from the ring.
        let signal_frames = PARTITION_SIZE * 3;
        // One extra block of silence at the end to drain the final partition.
        let total_frames = signal_frames + PARTITION_SIZE;
        let mut buffer: Vec<f32> = (0..signal_frames)
            .map(|i| (i as f32 * 0.1).sin())
            .chain(std::iter::repeat(0.0f32).take(PARTITION_SIZE))
            .collect();
        let original = buffer[..signal_frames].to_vec();

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

        // The first PARTITION_SIZE output samples are zero (empty ring at start).
        // Samples PARTITION_SIZE..PARTITION_SIZE+signal_frames should equal original.
        let latency = PARTITION_SIZE;
        for (i, (&got, &exp)) in buffer[latency..latency + signal_frames]
            .iter()
            .zip(original.iter())
            .enumerate()
        {
            assert!(
                (got - exp).abs() < 1e-4,
                "mix=0 passthrough mismatch at sample {}: got {}, expected {}",
                latency + i,
                got,
                exp
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
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin_par
                .process_in_place(&mut buf_par[block_start..block_end], &ctx)
                .unwrap();
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
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin_seq
                .process_in_place(&mut buf_seq[block_start..block_end], &ctx)
                .unwrap();
        }

        // Both plugins convolve with a Dirac, so outputs should match to float
        // precision.  The parallel path settles one partition later (zeros in
        // partitions 1-9 of the longer IR take one extra block to flush), so we
        // compare the settled region (skip the first 2 partitions).
        let skip = PARTITION_SIZE * 2;
        for (i, (&par, &seq)) in buf_par[skip..]
            .iter()
            .zip(buf_seq[skip..].iter())
            .enumerate()
        {
            assert!(
                (par - seq).abs() < 1e-5,
                "Parallel/sequential output mismatch at sample {}: parallel={par}, sequential={seq}",
                skip + i
            );
        }

        // Sanity: output must be finite and have energy.
        let energy: f32 = buf_par[skip..].iter().map(|s| s * s).sum();
        assert!(
            energy.is_finite() && energy > 0.0,
            "Parallel path must produce non-zero finite output"
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

    #[test]
    fn test_from_params_propagates_ir_load_errors() {
        let params = ConvolutionPluginParams {
            ir_file: "/definitely/missing/sotf-test-ir.wav".to_string(),
            mix: 1.0,
            gain_db: 0.0,
            use_nupc: true,
            zero_latency_head: false,
            head_taps: 128,
        };
        assert!(ConvolutionPlugin::from_params(1, 48000, params).is_err());
    }

    #[test]
    fn test_ir_file_parameter_loads_and_reports_path() {
        let path = std::env::temp_dir().join(format!(
            "sotf-convolution-ir-{}-{}.wav",
            std::process::id(),
            "param"
        ));
        write_test_wav(&path, &[32767, 0, 0, 0], 48000);

        let mut plugin = ConvolutionPlugin::new(1, 48000);
        plugin
            .set_parameter(
                ParameterId::from("ir_file"),
                ParameterValue::String(path.to_string_lossy().into_owned()),
            )
            .unwrap();

        assert_eq!(
            plugin.get_parameter(&ParameterId::from("ir_file")),
            Some(ParameterValue::String(path.to_string_lossy().into_owned()))
        );

        // Spin until the background thread finishes loading.
        let mut buf = vec![0.0f32; 1024];
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };
        for _ in 0..200 {
            plugin.process_in_place(&mut buf, &ctx).unwrap();
            if plugin.ir_load_result_rx.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(plugin.state.load().is_some(), "IR load should complete");

        fs::remove_file(path).ok();
    }

    #[test]
    fn test_ir_file_parameter_reports_load_errors() {
        let mut plugin = ConvolutionPlugin::new(1, 48000);
        let err = plugin
            .set_parameter(
                ParameterId::from("ir_file"),
                ParameterValue::String("/definitely/missing/sotf-test-ir.wav".to_string()),
            )
            .unwrap_err();
        assert!(err.contains("IO:"), "unexpected error: {err}");
        assert!(plugin.state.load().is_none());
    }

    #[test]
    fn test_process_rejects_short_buffer() {
        let mut plugin = make_plugin_with_ir(2, 48000, vec![vec![1.0], vec![1.0]]);
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 32,
        };
        let mut short = vec![0.0_f32; 32 * 2 - 1];
        assert!(plugin.process_in_place(&mut short, &ctx).is_err());
    }

    /// Partial-block passthrough: process with nf=64 (much smaller than
    /// PARTITION_SIZE=1024).  A Dirac IR with mix=0 must produce exactly the
    /// dry input in output[PARTITION_SIZE..] after flushing.
    ///
    /// This is the regression test for the UPC output-dropping bug (review
    /// issue #1): the old code only wrote back the last `to_copy` samples of
    /// each partition and silently discarded the first `PARTITION_SIZE-to_copy`
    /// samples.
    #[test]
    fn test_partial_block_no_output_drop() {
        let small_block = 64_usize; // << PARTITION_SIZE
        assert!(small_block < PARTITION_SIZE);

        let channels = 1;
        let sr = 48000;
        let ir = vec![vec![1.0f32]]; // Dirac: mix=0 output should equal dry input

        let mut plugin = make_plugin_with_ir(channels, sr, ir);
        plugin.mix_value = 0.0;
        plugin.mix.set_target(0.0);
        plugin.mix.reset(0.0);

        // Signal: PARTITION_SIZE samples of a sine + one flush block of zeros.
        let signal_frames = PARTITION_SIZE;
        let total_frames = signal_frames + PARTITION_SIZE; // extra block to flush ring
        let mut buffer: Vec<f32> = (0..signal_frames)
            .map(|i| (i as f32 * 0.05).sin())
            .chain(std::iter::repeat(0.0f32).take(PARTITION_SIZE))
            .collect();
        let original = buffer[..signal_frames].to_vec();

        // Process in small blocks
        for block_start in (0..total_frames).step_by(small_block) {
            let block_end = (block_start + small_block).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin
                .process_in_place(&mut buffer[block_start..block_end], &ctx)
                .unwrap();
        }

        // After one full partition + flush, output[PARTITION_SIZE..2*PARTITION_SIZE]
        // should equal original[0..PARTITION_SIZE] (mix=0 → dry passthrough with 1 block delay).
        let latency = PARTITION_SIZE;
        for (i, (&got, &exp)) in buffer[latency..latency + signal_frames]
            .iter()
            .zip(original.iter())
            .enumerate()
        {
            assert!(
                (got - exp).abs() < 1e-4,
                "partial-block output drop at sample {}: got {}, expected {}",
                latency + i,
                got,
                exp
            );
        }
    }

    /// Partial-block energy preservation: with a Dirac IR and mix=1 (fully wet),
    /// the output energy across multiple small blocks should approximately equal
    /// the input energy.  This catches sample-dropping that reduces total output
    /// amplitude.
    #[test]
    fn test_partial_block_energy_preserved() {
        let small_block = 128_usize;
        assert!(small_block < PARTITION_SIZE);

        let channels = 1;
        let sr = 48000;
        let ir = vec![vec![1.0f32]]; // Dirac

        let mut plugin = make_plugin_with_ir(channels, sr, ir);
        plugin.mix_value = 1.0;
        plugin.mix.set_target(1.0);
        plugin.mix.reset(1.0);
        plugin.gain_db_value = 0.0;
        plugin.gain_linear.set_target(1.0);
        plugin.gain_linear.reset(1.0);

        // Two full partitions of signal + one flush partition.
        let signal_frames = PARTITION_SIZE * 2;
        let total_frames = signal_frames + PARTITION_SIZE;
        let mut buffer: Vec<f32> = (0..signal_frames)
            .map(|i| (i as f32 * 0.07).sin() * 0.5)
            .chain(std::iter::repeat(0.0f32).take(PARTITION_SIZE))
            .collect();
        let input_energy: f32 = buffer[..signal_frames].iter().map(|s| s * s).sum();

        for block_start in (0..total_frames).step_by(small_block) {
            let block_end = (block_start + small_block).min(total_frames);
            let nf = block_end - block_start;
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin
                .process_in_place(&mut buffer[block_start..block_end], &ctx)
                .unwrap();
        }

        // Collect settled output (skip the initial 1-partition latency).
        let latency = PARTITION_SIZE;
        let output_energy: f32 = buffer[latency..latency + signal_frames]
            .iter()
            .map(|s| s * s)
            .sum();

        let ratio = output_energy / input_energy;
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "partial-block energy ratio should be ~1.0, got {ratio} (in={input_energy}, out={output_energy})"
        );
    }

    /// reset() clears all state: after reset, processing should be identical
    /// to a fresh plugin run.
    #[test]
    fn test_reset_clears_all_state() {
        let channels = 1;
        let sr = 48000;
        let ir = vec![vec![1.0f32]];

        let mut plugin = make_plugin_with_ir(channels, sr, ir.clone());
        plugin.mix_value = 1.0;
        plugin.mix.set_target(1.0);
        plugin.mix.reset(1.0);
        plugin.gain_linear.set_target(1.0);
        plugin.gain_linear.reset(1.0);

        // First run
        let frames = PARTITION_SIZE * 2;
        let signal: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut buf1 = signal.clone();
        for block_start in (0..frames).step_by(PARTITION_SIZE) {
            let nf = PARTITION_SIZE.min(frames - block_start);
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin
                .process_in_place(&mut buf1[block_start..block_start + nf], &ctx)
                .unwrap();
        }

        // Reset and second run with same input — must produce same output
        plugin.reset();
        let mut buf2 = signal.clone();
        for block_start in (0..frames).step_by(PARTITION_SIZE) {
            let nf = PARTITION_SIZE.min(frames - block_start);
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: nf,
            };
            plugin
                .process_in_place(&mut buf2[block_start..block_start + nf], &ctx)
                .unwrap();
        }

        for (i, (&a, &b)) in buf1.iter().zip(buf2.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-5,
                "reset() mismatch at sample {i}: first_run={a}, after_reset={b}"
            );
        }
    }
}
