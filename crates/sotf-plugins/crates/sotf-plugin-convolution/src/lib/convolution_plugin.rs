use super::misc::FFT_SIZE;
use super::misc::PARTITION_SIZE;
use super::types::ConvolutionPluginParams;
use super::types::ConvolutionState;
use super::types::IrLoadResult;
use super::types::{ConvolutionLoadStatus, IrLoadCompletion, RetiredIrState};
use crate::params::PARAMS as CV;
use arc_swap::ArcSwap;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use plugins_spatial::{nupc, validate_interleaved_in_place};
use rubato::{Fft, FixedSync, Resampler};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{complex_mul_add_simd, enable_ftz_daz};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use symphonia::core::audio::{Audio, GenericAudioBufferRef};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::formats::probe::{Hint, Probe};
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

struct IrLoadRequest {
    generation: u64,
    path: String,
    channels: usize,
    sample_rate: u32,
    use_nupc: bool,
    zero_latency_head: bool,
    head_taps: usize,
    result_tx: std::sync::mpsc::Sender<IrLoadCompletion>,
}

static IR_LOADER: OnceLock<mpsc::Sender<IrLoadRequest>> = OnceLock::new();
const MAX_IR_CHANNELS: usize = 32;
const MAX_IR_SECONDS: usize = 30;
const MAX_IR_MEMORY_BYTES: usize = 512 * 1024 * 1024;
const TRANSITION_SAMPLES: usize = 128;

fn get_ir_loader() -> &'static mpsc::Sender<IrLoadRequest> {
    IR_LOADER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<IrLoadRequest>();
        // Bound the pool so rapid IR switches do not exhaust the OS thread budget.
        let num_workers = rayon::current_num_threads().clamp(1, 4);
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..num_workers {
            let rx = Arc::clone(&rx);
            std::thread::Builder::new()
                .name(format!("convolution-ir-load-{i}"))
                .spawn(move || {
                    loop {
                        let req = match rx.lock().unwrap().recv() {
                            Ok(req) => req,
                            Err(_) => break,
                        };
                        let result = ConvolutionPlugin::build_ir_state(
                            &req.path,
                            req.channels,
                            req.sample_rate,
                            req.use_nupc,
                            req.zero_latency_head,
                            req.head_taps,
                        );
                        let _ = req.result_tx.send(IrLoadCompletion {
                            generation: req.generation,
                            result,
                        });
                    }
                })
                .expect("failed to spawn convolution IR loader thread");
        }
        tx
    })
}

static IR_RECLAIMER: OnceLock<mpsc::SyncSender<RetiredIrState>> = OnceLock::new();
static IR_ERROR_RECLAIMER: OnceLock<mpsc::SyncSender<String>> = OnceLock::new();

fn get_ir_reclaimer() -> &'static mpsc::SyncSender<RetiredIrState> {
    IR_RECLAIMER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<RetiredIrState>(8);
        std::thread::Builder::new()
            .name("convolution-ir-reclaimer".into())
            .spawn(move || {
                while let Ok(retired) = rx.recv() {
                    let RetiredIrState {
                        state,
                        nupc_engines,
                        fdl_flat,
                        fft_scratch,
                        rayon_accum_pool,
                    } = retired;
                    drop((state, nupc_engines, fdl_flat, fft_scratch, rayon_accum_pool));
                }
            })
            .expect("failed to spawn convolution IR reclaimer thread");
        tx
    })
}

fn get_ir_error_reclaimer() -> &'static mpsc::SyncSender<String> {
    IR_ERROR_RECLAIMER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<String>(8);
        std::thread::Builder::new()
            .name("convolution-ir-error-reclaimer".into())
            .spawn(move || {
                while let Ok(error) = rx.recv() {
                    log::error!("[Convolution] IR load failed: {error}");
                }
            })
            .expect("failed to spawn convolution error reclaimer thread");
        tx
    })
}

/// Retain one payload for a later retry when a bounded reclamation queue is
/// full. If that retry slot is already occupied, or the reclaimer has stopped,
/// intentionally leak the payload rather than running an unbounded destructor
/// on the realtime thread.
pub(super) fn defer_or_forget<T>(pending: &mut Option<T>, error: mpsc::TrySendError<T>) {
    match error {
        mpsc::TrySendError::Full(value) if pending.is_none() => *pending = Some(value),
        mpsc::TrySendError::Full(value) | mpsc::TrySendError::Disconnected(value) => {
            std::mem::forget(value);
        }
    }
}

pub struct ConvolutionPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    pub(super) ir_file: String,
    pub(super) mix: Smoother,
    pub(super) mix_value: f32,
    pub(super) gain_linear: Smoother,
    pub(super) gain_db_value: f32,
    pub(super) state: Arc<ArcSwap<Option<ConvolutionState>>>,
    pub(super) input_buffers: Vec<Vec<f32>>,
    pub(super) input_fill: usize,
    /// Flattened Frequency Domain Line (FDL): [partition * channels * FFT_SIZE + channel * FFT_SIZE + bin]
    pub(super) fdl_flat: Vec<Complex<f32>>,
    pub(super) fdl_head: usize, // ring buffer head for FDL (avoids rotate_right)
    pub(super) output_accum: Vec<Vec<f32>>,
    /// Per-channel output ring buffer: stores completed partition output so
    /// partial-block boundaries are handled correctly (fix for issue #1).
    /// Size: PARTITION_SIZE samples per channel (one completed partition).
    pub(super) output_ring: Vec<Vec<f32>>,
    /// Read pointer into `output_ring` (next sample to be drained).
    pub(super) output_ring_read: usize,
    /// Number of valid samples waiting to be consumed from `output_ring`.
    pub(super) output_ring_available: usize,
    pub(super) mix_envelope: Vec<f32>,
    pub(super) gain_envelope: Vec<f32>,
    // Pre-allocated scratch buffers (avoid heap allocs in audio callback)
    pub(super) fft_spectrum: Vec<Complex<f32>>,
    pub(super) fft_sum: Vec<Complex<f32>>,
    pub(super) fft_scratch: Vec<Complex<f32>>,
    pub(super) cached_parameters: Vec<Parameter>,
    /// When use_nupc is true, per-channel NUPC engines for low-latency convolution
    pub(super) nupc_engines: Vec<nupc::NupcEngine>,
    pub(super) use_nupc: bool,
    pub(super) zero_latency_head: bool,
    pub(super) head_taps: usize,
    /// Per-channel dry delay used to align partial NUPC mixes when no
    /// zero-latency time-domain head is active.
    pub(super) nupc_dry_delay: Vec<Vec<f32>>,
    pub(super) nupc_dry_delay_pos: usize,
    /// Pre-allocated accumulator buffers for rayon fold/reduce (one per rayon thread).
    /// Avoids heap allocation in the audio processing hot path.
    pub(super) rayon_accum_pool: Vec<Vec<Complex<f32>>>,
    /// Channel to receive asynchronously-loaded IR state from the background thread.
    pub(super) ir_load_result_rx: Option<std::sync::mpsc::Receiver<IrLoadCompletion>>,
    pub(super) desired_generation: u64,
    pub(super) load_status: AtomicU8,
    pub(super) retired_pending: Option<RetiredIrState>,
    pub(super) failed_error_pending: Option<String>,
    pub(super) inactive_dry_delay: Vec<Vec<f32>>,
    pub(super) inactive_dry_delay_pos: usize,
    pub(super) last_output: Vec<f32>,
    pub(super) transition_from: Vec<f32>,
    pub(super) transition_remaining: usize,
}

impl ConvolutionPlugin {
    pub(super) fn validate_ir_limits(
        ir_samples: &[Vec<f32>],
        ir_sample_rate: u32,
        target_sample_rate: u32,
        output_channels: usize,
        use_nupc: bool,
    ) -> Result<(), String> {
        if ir_sample_rate == 0 {
            return Err("IR file is missing a valid sample rate".into());
        }
        if ir_samples.is_empty() || ir_samples.iter().any(Vec::is_empty) {
            return Err("IR file contains no audio data".into());
        }
        if ir_samples.len() > MAX_IR_CHANNELS {
            return Err(format!(
                "IR has {} channels; maximum supported is {MAX_IR_CHANNELS}",
                ir_samples.len()
            ));
        }
        let max_source_frames = ir_sample_rate as usize * MAX_IR_SECONDS;
        if ir_samples
            .iter()
            .any(|channel| channel.len() > max_source_frames)
        {
            return Err(format!(
                "IR exceeds the {MAX_IR_SECONDS}-second realtime limit"
            ));
        }
        let target_samples = ir_samples.iter().fold(0_usize, |total, channel| {
            total.saturating_add(
                channel
                    .len()
                    .saturating_mul(target_sample_rate as usize)
                    .div_ceil(ir_sample_rate as usize),
            )
        });
        let backend_multiplier = if use_nupc { output_channels } else { 4 };
        let estimated_bytes = target_samples
            .saturating_mul(std::mem::size_of::<f32>())
            .saturating_mul(backend_multiplier);
        if estimated_bytes > MAX_IR_MEMORY_BYTES {
            return Err(format!(
                "IR backend estimate is {} MiB; realtime limit is {} MiB",
                estimated_bytes / (1024 * 1024),
                MAX_IR_MEMORY_BYTES / (1024 * 1024)
            ));
        }
        Ok(())
    }

    pub fn new(channels: usize, sample_rate: u32) -> Self {
        // Prepare background retirement before the instance can reach an audio
        // callback; successful installs and failures then only perform bounded
        // non-allocating channel operations.
        let _ = get_ir_reclaimer();
        let _ = get_ir_error_reclaimer();
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
            mix_envelope: vec![1.0; PARTITION_SIZE],
            gain_envelope: vec![1.0; PARTITION_SIZE],
            fft_spectrum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_sum: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            fft_scratch: Vec::new(),
            cached_parameters: Vec::new(),
            nupc_engines: Vec::new(),
            use_nupc: CV[3].default_bool(),
            zero_latency_head: false,
            head_taps: 128,
            nupc_dry_delay: vec![vec![0.0; PARTITION_SIZE]; channels],
            nupc_dry_delay_pos: 0,
            rayon_accum_pool: Vec::new(),
            ir_load_result_rx: None,
            desired_generation: 0,
            load_status: AtomicU8::new(ConvolutionLoadStatus::Idle as u8),
            retired_pending: None,
            failed_error_pending: None,
            inactive_dry_delay: vec![vec![0.0; PARTITION_SIZE]; channels],
            inactive_dry_delay_pos: 0,
            last_output: vec![0.0; channels],
            transition_from: vec![0.0; channels],
            transition_remaining: 0,
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
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
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {} // ir_file (FilePath -- handled separately)
            1 => {
                let value = value.clamp(CV[1].min_f64(), CV[1].max_f64()) as f32;
                self.mix_value = value;
                self.mix.set_target(value);
            }
            2 => {
                let value = value.clamp(CV[2].min_f64(), CV[2].max_f64()) as f32;
                self.gain_db_value = value;
                self.gain_linear.set_target(10.0f32.powf(value / 20.0));
            }
            3 => self.use_nupc = value > 0.5,
            4 => self.zero_latency_head = value > 0.5,
            5 => self.head_taps = value.clamp(CV[5].min_f64(), CV[5].max_f64()) as usize,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(CV, |i| self.param_value(i));
        if let Some(ir_param) = self
            .cached_parameters
            .iter_mut()
            .find(|param| param.id.as_str() == "ir_file")
        {
            ir_param.default_value = ParameterValue::String(self.ir_file.clone());
        }
    }

    fn current_parameter_schema(&self) -> Vec<Parameter> {
        let mut parameters = self.cached_parameters.clone();
        if let Some(ir_param) = parameters
            .iter_mut()
            .find(|param| param.id.as_str() == "ir_file")
        {
            ir_param.default_value = ParameterValue::String(self.ir_file.clone());
        }
        parameters
    }

    pub fn load_status(&self) -> ConvolutionLoadStatus {
        match self.load_status.load(Ordering::Acquire) {
            1 => ConvolutionLoadStatus::Loading,
            2 => ConvolutionLoadStatus::Ready,
            3 => ConvolutionLoadStatus::Failed,
            _ => ConvolutionLoadStatus::Idle,
        }
    }

    fn begin_async_load(&mut self, path: String) -> PluginResult<()> {
        self.desired_generation = self.desired_generation.wrapping_add(1);
        let generation = self.desired_generation;
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        self.ir_load_result_rx = Some(result_rx);
        self.load_status
            .store(ConvolutionLoadStatus::Loading as u8, Ordering::Release);
        get_ir_loader()
            .send(IrLoadRequest {
                generation,
                path,
                channels: self.channels,
                sample_rate: self.sample_rate,
                use_nupc: self.use_nupc,
                zero_latency_head: self.zero_latency_head,
                head_taps: self.head_taps,
                result_tx,
            })
            .map_err(|e| format!("Failed to enqueue IR load: {e}"))
    }

    fn queue_retired(&mut self, retired: RetiredIrState) {
        if let Err(error) = get_ir_reclaimer().try_send(retired) {
            defer_or_forget(&mut self.retired_pending, error);
        }
    }

    fn flush_retired(&mut self) {
        if let Some(retired) = self.retired_pending.take() {
            self.queue_retired(retired);
        }
        if let Some(error) = self.failed_error_pending.take()
            && let Err(error) = get_ir_error_reclaimer().try_send(error)
        {
            defer_or_forget(&mut self.failed_error_pending, error);
        }
    }

    fn retire_uninstalled(&mut self, result: IrLoadResult) {
        self.queue_retired(RetiredIrState {
            state: result.state,
            nupc_engines: result.nupc_engines,
            fdl_flat: result.fdl_flat,
            fft_scratch: result.fft_scratch,
            rayon_accum_pool: result.rayon_accum_pool,
        });
    }

    fn clear_ir_state(&mut self) {
        self.transition_from.copy_from_slice(&self.last_output);
        self.transition_remaining = TRANSITION_SAMPLES;
        self.desired_generation = self.desired_generation.wrapping_add(1);
        self.ir_load_result_rx = None;
        let retired = RetiredIrState {
            state: self.state.swap(Arc::new(None)),
            nupc_engines: std::mem::take(&mut self.nupc_engines),
            fdl_flat: std::mem::take(&mut self.fdl_flat),
            fft_scratch: std::mem::take(&mut self.fft_scratch),
            rayon_accum_pool: std::mem::take(&mut self.rayon_accum_pool),
        };
        self.ir_file.clear();
        self.reset_streaming_state();
        self.load_status
            .store(ConvolutionLoadStatus::Idle as u8, Ordering::Release);
        self.queue_retired(retired);
    }

    pub fn from_params(
        channels: usize,
        sample_rate: u32,
        params: ConvolutionPluginParams,
    ) -> Result<Self, String> {
        if channels == 0 {
            return Err("Convolution requires at least one channel".into());
        }
        if sample_rate == 0 {
            return Err("Convolution requires a non-zero sample rate".into());
        }
        if !params.mix.is_finite()
            || params.mix < CV[1].min_f64() as f32
            || params.mix > CV[1].max_f64() as f32
        {
            return Err(format!("Invalid convolution mix: {}", params.mix));
        }
        if !params.gain_db.is_finite()
            || params.gain_db < CV[2].min_f64() as f32
            || params.gain_db > CV[2].max_f64() as f32
        {
            return Err(format!("Invalid convolution gain_db: {}", params.gain_db));
        }
        if params.head_taps < CV[5].min_f64() as usize
            || params.head_taps > CV[5].max_f64() as usize
        {
            return Err(format!(
                "Invalid convolution head_taps: {}",
                params.head_taps
            ));
        }
        let mut plugin = Self::new(channels, sample_rate);
        plugin.use_nupc = params.use_nupc;
        plugin.zero_latency_head = params.zero_latency_head;
        plugin.head_taps = params.head_taps;
        if !params.ir_file.is_empty() {
            plugin.load_ir(&params.ir_file)?;
        }
        plugin.set_param_value(1, params.mix as f64);
        plugin.mix.reset(plugin.mix_value);
        plugin.set_param_value(2, params.gain_db as f64);
        plugin
            .gain_linear
            .reset(10.0f32.powf(plugin.gain_db_value / 20.0));
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    /// Build IR state on any thread (file I/O, FFT planning, allocations).
    pub(super) fn build_ir_state(
        path: &str,
        channels: usize,
        sample_rate: u32,
        use_nupc: bool,
        zero_latency_head: bool,
        head_taps: usize,
    ) -> Result<IrLoadResult, String> {
        let (ir_samples, ir_sample_rate) = Self::load_audio_file(path)?;

        Self::validate_ir_limits(&ir_samples, ir_sample_rate, sample_rate, channels, use_nupc)?;

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
        let (partitions, num_partitions, fft_forward, fft_inverse, fft_scratch_len) = if use_nupc {
            (Vec::new(), 0, None, None, 0)
        } else {
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
            (
                partitions,
                num_partitions,
                Some(fft_forward),
                Some(fft_inverse),
                fft_scratch_len,
            )
        };

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
            let kernels = ir_samples
                .iter()
                .map(|ir| {
                    if zero_latency_head && head_taps > 0 {
                        nupc::NupcKernel::new_with_head(ir, PARTITION_SIZE, head_taps)
                    } else {
                        nupc::NupcKernel::new(ir, PARTITION_SIZE)
                    }
                })
                .collect::<Vec<_>>();
            for ch in 0..channels {
                let ir_ch = ch % ir_channels;
                nupc_engines.push(kernels[ir_ch].instantiate());
            }
            log::info!(
                "[Convolution] NUPC engines built: {} channels, latency={} samples",
                nupc_engines.len(),
                nupc_engines.first().map_or(0, |e| e.latency_samples())
            );
        }

        Ok(IrLoadResult {
            state: Arc::new(Some(state)),
            nupc_engines,
            fdl_flat: vec![Complex::new(0.0, 0.0); num_partitions * channels * FFT_SIZE],
            fdl_head: 0,
            fft_scratch: vec![Complex::new(0.0, 0.0); fft_scratch_len],
            rayon_accum_pool: (0..rayon::current_num_threads().min(num_partitions))
                .map(|_| vec![Complex::new(0.0, 0.0); FFT_SIZE])
                .collect(),
            ir_file: path.to_string(),
        })
    }

    pub(super) fn apply_ir_state(&mut self, result: IrLoadResult) {
        self.transition_from.copy_from_slice(&self.last_output);
        self.transition_remaining = TRANSITION_SAMPLES;
        let retired = RetiredIrState {
            state: self.state.swap(result.state),
            nupc_engines: std::mem::replace(&mut self.nupc_engines, result.nupc_engines),
            fdl_flat: std::mem::replace(&mut self.fdl_flat, result.fdl_flat),
            fft_scratch: std::mem::replace(&mut self.fft_scratch, result.fft_scratch),
            rayon_accum_pool: std::mem::replace(
                &mut self.rayon_accum_pool,
                result.rayon_accum_pool,
            ),
        };
        self.fdl_head = result.fdl_head;
        self.ir_file = result.ir_file;
        self.reset_streaming_state();
        self.load_status
            .store(ConvolutionLoadStatus::Ready as u8, Ordering::Release);
        self.queue_retired(retired);
    }

    fn reset_streaming_state(&mut self) {
        self.fdl_flat.fill(Complex::new(0.0, 0.0));
        self.fdl_head = 0;
        self.input_fill = 0;
        for buffer in &mut self.input_buffers {
            buffer.fill(0.0);
        }
        for buffer in &mut self.output_accum {
            buffer.fill(0.0);
        }
        for buffer in &mut self.output_ring {
            buffer.fill(0.0);
        }
        self.output_ring_read = 0;
        self.output_ring_available = 0;
        for engine in &mut self.nupc_engines {
            engine.reset();
        }
        for buffer in &mut self.nupc_dry_delay {
            buffer.fill(0.0);
        }
        self.nupc_dry_delay_pos = 0;
        for buffer in &mut self.inactive_dry_delay {
            buffer.fill(0.0);
        }
        self.inactive_dry_delay_pos = 0;
    }

    fn finish_output_block(&mut self, buffer: &mut [f32], frames: usize) {
        for frame in 0..frames {
            let base = frame * self.channels;
            let fade = if self.transition_remaining > 0 {
                let progressed = TRANSITION_SAMPLES - self.transition_remaining + 1;
                self.transition_remaining -= 1;
                progressed as f32 / TRANSITION_SAMPLES as f32
            } else {
                1.0
            };
            for ch in 0..self.channels {
                let output = if fade < 1.0 {
                    self.transition_from[ch] * (1.0 - fade) + buffer[base + ch] * fade
                } else {
                    buffer[base + ch]
                };
                buffer[base + ch] = output;
                self.last_output[ch] = output;
            }
        }
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
        self.rebuild_cached_parameters();
        Ok(())
    }

    /// Load an audio file using Symphonia's format probing.
    /// Supports WAV, FLAC, and AIFF formats.
    /// Returns (channel_samples, sample_rate).
    pub(super) fn load_audio_file(path: &str) -> Result<(Vec<Vec<f32>>, u32), String> {
        use std::fs::File;
        use std::sync::LazyLock;

        // Shared probe and codec registry for IR loading
        static IR_PROBE: LazyLock<Probe> = LazyLock::new(|| {
            let mut probe = Probe::default();
            probe.register_format::<symphonia_format_riff::WavReader>();
            probe.register_format::<symphonia_format_riff::AiffReader>();
            probe.register_format::<symphonia_bundle_flac::FlacReader>();
            probe
        });

        static IR_CODEC_REGISTRY: LazyLock<CodecRegistry> = LazyLock::new(|| {
            let mut registry = CodecRegistry::new();
            registry.register_audio_decoder::<symphonia_codec_pcm::PcmDecoder>();
            registry.register_audio_decoder::<symphonia_bundle_flac::FlacDecoder>();
            registry
        });

        let file = File::open(Path::new(path)).map_err(|e| format!("IO: {e}"))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probe_result = IR_PROBE
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| format!("Probe: {e}"))?;

        let mut reader = probe_result;
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or("No track found in IR file")?;
        let codec_params = match track.codec_params.clone() {
            Some(CodecParameters::Audio(params)) => params,
            _ => return Err("IR file does not contain an audio track".into()),
        };

        let sample_rate = codec_params.sample_rate.unwrap_or(0);
        let num_channels = codec_params
            .channels
            .as_ref()
            .map(|c| c.count())
            .unwrap_or(1);

        let mut decoder = IR_CODEC_REGISTRY
            .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
            .map_err(|e| format!("Decoder: {e}"))?;

        let mut samples = vec![Vec::new(); num_channels];
        loop {
            let packet = match reader.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => break,
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
                GenericAudioBufferRef::F32(buf) => {
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend_from_slice(buf.plane(ch).ok_or("Missing IR channel")?);
                    }
                }
                GenericAudioBufferRef::S16(buf) => {
                    let scale = 1.0 / 32768.0;
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend(
                            buf.plane(ch)
                                .ok_or("Missing IR channel")?
                                .iter()
                                .map(|&s| s as f32 * scale),
                        );
                    }
                }
                GenericAudioBufferRef::S24(buf) => {
                    let scale = 1.0 / 8388608.0;
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend(
                            buf.plane(ch)
                                .ok_or("Missing IR channel")?
                                .iter()
                                .map(|s| s.inner() as f32 * scale),
                        );
                    }
                }
                GenericAudioBufferRef::S32(buf) => {
                    let scale = 1.0 / 2147483648.0;
                    for (ch, sample_ch) in samples.iter_mut().enumerate() {
                        sample_ch.extend(
                            buf.plane(ch)
                                .ok_or("Missing IR channel")?
                                .iter()
                                .map(|&s| s as f32 * scale),
                        );
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
    pub(super) fn resample_ir(
        ir_samples: &[Vec<f32>],
        source_rate: u32,
        target_rate: u32,
    ) -> Result<Vec<Vec<f32>>, String> {
        if ir_samples.is_empty() {
            return Err("Cannot resample an IR with no channels".into());
        }
        let source_len = ir_samples[0].len();
        if ir_samples.iter().any(|channel| channel.len() != source_len) {
            return Err("Cannot resample an IR with channels of different lengths".into());
        }
        if source_len == 0 {
            return Ok(vec![Vec::new(); ir_samples.len()]);
        }

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

        // Rubato's incremental API exposes the filter's startup delay and requires
        // callers to flush zero input themselves.  The whole-clip API performs both
        // operations: it pumps enough zero input to flush the filter tail, removes
        // exactly `output_delay()` leading frames, and returns the rounded nominal
        // output length.  Allocate its documented temporary capacity once, then
        // retain only the returned clip frames.
        let output_capacity = resampler.process_all_needed_output_len(source_len);
        let mut output_channels = vec![vec![0.0_f32; output_capacity]; num_channels];
        let input_adapter = SequentialSliceOfVecs::new(ir_samples, num_channels, source_len)
            .map_err(|e| format!("Input adapter error: {e}"))?;
        let mut output_adapter =
            SequentialSliceOfVecs::new_mut(&mut output_channels, num_channels, output_capacity)
                .map_err(|e| format!("Output adapter error: {e}"))?;
        let (_, written) = resampler
            .process_all_into_buffer(
                &input_adapter,
                &mut output_adapter,
                source_len,
                None,
            )
            .map_err(|e| format!("Resampling error: {e}"))?;

        for channel in &mut output_channels {
            channel.truncate(written);
        }

        Ok(output_channels)
    }
}

impl ParametricInPlacePlugin for ConvolutionPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Convolution", "2.1.0", "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Convolution
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::boundary(PluginCostClass::Convolution, self.latency_samples())
    }

    fn channels(&self) -> usize {
        self.channels
    }
    fn parameter_schema(&self) -> ParameterSchema {
        self.current_parameter_schema()
    }
    fn current_values(&self) -> ParameterSet {
        self.current_parameter_schema()
            .iter()
            .map(|p| (p.id.clone(), p.default_value.clone()))
            .collect()
    }
    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
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
                    self.clear_ir_state();
                } else {
                    // Quick synchronous validation so tests get immediate feedback.
                    if !std::path::Path::new(&path).exists() {
                        return Err(format!("IO: {path}: No such file or directory"));
                    }
                    self.begin_async_load(path)?;
                }
            } else if matches!(id.as_str(), "use_nupc" | "zero_latency_head" | "head_taps") {
                return Err(format!(
                    "{} is a structural convolution parameter; rebuild the plugin host to change it",
                    id.as_str()
                ));
            } else {
                param_bridge::set_parameter(CV, &id, &value, |i, v| {
                    self.set_param_value(i, v);
                })?;
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        let mut values = ParameterSet::new();
        values.insert(id, value);
        self.apply_values(values)
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        let old_sr = self.sample_rate;
        self.sample_rate = sr;
        self.mix.set_time(20.0, sr);
        self.gain_linear.set_time(20.0, sr);
        if old_sr != sr && !self.ir_file.is_empty() {
            self.begin_async_load(self.ir_file.clone())?;
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
        for buffer in &mut self.nupc_dry_delay {
            buffer.fill(0.0);
        }
        self.nupc_dry_delay_pos = 0;
        for buffer in &mut self.inactive_dry_delay {
            buffer.fill(0.0);
        }
        self.inactive_dry_delay_pos = 0;
        // Reset parameter smoothers to their instantaneous values so the
        // next playback starts without interpolating from a stale position.
        self.mix.reset(self.mix_value);
        self.gain_linear
            .reset(10.0f32.powf(self.gain_db_value / 20.0));
        self.mix_envelope.fill(self.mix_value);
        self.gain_envelope
            .fill(10.0f32.powf(self.gain_db_value / 20.0));
        self.last_output.fill(0.0);
        self.transition_from.fill(0.0);
        self.transition_remaining = 0;
    }

    fn latency_samples(&self) -> usize {
        if self.use_nupc {
            if self.zero_latency_head && self.head_taps > 0 {
                return 0;
            }
            self.nupc_engines.first().map_or(PARTITION_SIZE, |engine| {
                if engine.head_taps() > 0 {
                    0
                } else {
                    engine.latency_samples()
                }
            })
        } else {
            PARTITION_SIZE
        }
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
        validate_interleaved_in_place("Convolution", nf, self.channels, buffer.len())?;

        // Check for asynchronously-loaded IR results and swap them in.
        self.flush_retired();
        if self.retired_pending.is_none()
            && let Some(ref rx) = self.ir_load_result_rx
            && let Ok(completion) = rx.try_recv()
        {
            self.ir_load_result_rx = None;
            if completion.generation != self.desired_generation {
                if let Ok(loaded) = completion.result {
                    self.retire_uninstalled(loaded);
                }
            } else {
                match completion.result {
                    Ok(loaded) => self.apply_ir_state(loaded),
                    Err(e) => {
                        self.load_status
                            .store(ConvolutionLoadStatus::Failed as u8, Ordering::Release);
                        if let Err(error) = get_ir_error_reclaimer().try_send(e) {
                            defer_or_forget(&mut self.failed_error_pending, error);
                        }
                    }
                }
            }
        }

        let state_guard = self.state.load();
        let state = match state_guard.as_ref() {
            Some(s) => s,
            None => {
                let latency = self.latency_samples();
                for frame in 0..nf {
                    let base = frame * self.channels;
                    self.mix.advance();
                    self.gain_linear.advance();
                    for ch in 0..self.channels {
                        let dry = buffer[base + ch];
                        let output = if latency == 0 {
                            dry
                        } else {
                            let delayed = self.inactive_dry_delay[ch][self.inactive_dry_delay_pos];
                            self.inactive_dry_delay[ch][self.inactive_dry_delay_pos] = dry;
                            delayed
                        };
                        buffer[base + ch] = output;
                        self.last_output[ch] = output;
                    }
                    if latency > 0 {
                        self.inactive_dry_delay_pos += 1;
                        if self.inactive_dry_delay_pos == latency {
                            self.inactive_dry_delay_pos = 0;
                        }
                    }
                }
                drop(state_guard);
                self.finish_output_block(buffer, nf);
                return Ok(nf);
            }
        };

        // NUPC path: per-channel block processing with non-uniform partitions.
        // Avoids the UPC's fixed PARTITION_SIZE constraint for lower latency.
        //
        // Issue #3 fix (NUPC): advance smoothers one sample at a time so that
        // mix/gain transitions are sample-accurate rather than block-quantized.
        if !self.nupc_engines.is_empty() && self.nupc_engines.len() == self.channels {
            let wet_latency = if self.nupc_engines[0].head_taps() > 0 {
                0
            } else {
                self.nupc_engines[0].latency_samples()
            };
            debug_assert!(wet_latency <= PARTITION_SIZE);
            for frame in 0..nf {
                // Advance smoothers by one sample to get the value for this frame.
                let mix = self.mix.advance();
                let gain = self.gain_linear.advance();
                let off = frame * self.channels;
                for ch in 0..self.channels {
                    let dry = buffer[off + ch];
                    let wet = self.nupc_engines[ch].process_sample(dry);
                    let aligned_dry = if wet_latency == 0 {
                        dry
                    } else {
                        let delayed = self.nupc_dry_delay[ch][self.nupc_dry_delay_pos];
                        self.nupc_dry_delay[ch][self.nupc_dry_delay_pos] = dry;
                        delayed
                    };
                    buffer[off + ch] = aligned_dry * (1.0 - mix) + wet * mix * gain;
                }
                if wet_latency > 0 {
                    self.nupc_dry_delay_pos += 1;
                    if self.nupc_dry_delay_pos == wet_latency {
                        self.nupc_dry_delay_pos = 0;
                    }
                }
            }
            drop(state_guard);
            self.finish_output_block(buffer, nf);
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
            self.mix_envelope[fill_idx] = self.mix.advance();
            self.gain_envelope[fill_idx] = self.gain_linear.advance();
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
                        .as_ref()
                        .expect("UPC state must have a forward FFT plan")
                        .process_with_scratch(&mut self.fft_spectrum, &mut self.fft_scratch);

                    let off_base = (self.fdl_head * self.channels + ch) * FFT_SIZE;
                    self.fdl_flat[off_base..off_base + FFT_SIZE]
                        .copy_from_slice(&self.fft_spectrum);

                    self.fft_sum.fill(Complex::new(0.0, 0.0));
                    let ir_ch = if state.ir_channels == 1 {
                        0
                    } else {
                        ch % state.ir_channels
                    };

                    // Keep the audio callback on its current thread. Dispatching
                    // short FFT work to Rayon's global pool introduces
                    // unbounded scheduling and wake-up latency.
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
                        .as_ref()
                        .expect("UPC state must have an inverse FFT plan")
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
                    // Envelopes were captured when each input sample arrived,
                    // then delayed with the matching output partition.
                    let m = self.mix_envelope[i];
                    let g = self.gain_envelope[i];
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
        drop(state_guard);
        self.finish_output_block(buffer, nf);
        Ok(nf)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}
