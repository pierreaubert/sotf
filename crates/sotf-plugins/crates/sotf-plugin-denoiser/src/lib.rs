// ============================================================================
// Denoiser Plugin - Wiener Filter with MCRA Noise Estimation
// ============================================================================
//
// This plugin implements spectral denoising using:
// - MCRA (Minimum Controlled Recursive Averaging) for automatic noise estimation
// - Wiener filter for optimal noise reduction
// - STFT (Short-Time Fourier Transform) with overlap-add for artifact-free processing
//
// The plugin supports both real-time and post-processing modes through
// configurable latency (FFT size).
//
// Algorithm flow:
// 1. Accumulate input samples into FFT-sized blocks
// 2. Apply Hann window and forward FFT
// 3. Estimate noise using MCRA
// 4. Calculate Wiener filter gains
// 5. Apply gains to frequency domain
// 6. Inverse FFT and overlap-add to output

use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
pub mod params;

use crate::params::PARAMS as DN;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_plugin_pnd::analysis::PndAnalyzer;
use std::any::Any;
use std::sync::Arc;

use sotf_host::analyzer::RealTimeCache;

pub mod backend;
pub mod backend_rnnoise;
mod config;
mod fft;
mod hiss;
mod masking;
mod mcra;
mod multi_resolution;
mod noise_profile;
mod polyphonic;
mod spectral_sub;
#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests;
mod transient;
mod wiener;

pub use config::DenoiserPluginParams;

/// Number of frequency bands for display/monitoring
const NUM_DISPLAY_BANDS: usize = 30;

// ============================================================================
// Exposed Data Structure
// ============================================================================

/// Data exposed by the denoiser for monitoring
#[derive(Debug, Clone)]
pub struct DenoiserData {
    /// Estimated noise floor per frequency band (in dB)
    /// Averaged across channels, downsampled to ~30 bands for display
    pub noise_floor_db: Arc<Vec<f32>>,

    /// Current SNR estimate per frequency band (in dB)
    pub snr_db: Arc<Vec<f32>>,

    /// Average gain reduction in dB (positive value = reduction)
    pub avg_reduction_db: f32,

    /// Whether noise learning is currently active (quiet moment detected)
    pub learning_active: bool,

    /// Whether noise profile learning is in progress
    pub is_learning_noise: bool,

    /// Whether a captured noise profile is available
    pub has_captured_profile: bool,

    /// Learning progress (0.0 to 1.0)
    pub learning_progress: f32,

    /// Whether using captured profile
    pub using_captured_profile: bool,
}

impl Default for DenoiserData {
    fn default() -> Self {
        Self {
            noise_floor_db: Arc::new(vec![0.0; NUM_DISPLAY_BANDS]),
            snr_db: Arc::new(vec![0.0; NUM_DISPLAY_BANDS]),
            avg_reduction_db: 0.0,
            learning_active: true,
            is_learning_noise: false,
            has_captured_profile: false,
            learning_progress: 0.0,
            using_captured_profile: false,
        }
    }
}

impl DenoiserData {
    pub fn update(&mut self, other: &DenoiserData) {
        if let Some(mut_nf) = Arc::get_mut(&mut self.noise_floor_db)
            && mut_nf.len() == other.noise_floor_db.len()
        {
            mut_nf.copy_from_slice(&other.noise_floor_db);
        } else {
            self.noise_floor_db = other.noise_floor_db.clone();
        }

        if let Some(mut_snr) = Arc::get_mut(&mut self.snr_db)
            && mut_snr.len() == other.snr_db.len()
        {
            mut_snr.copy_from_slice(&other.snr_db);
        } else {
            self.snr_db = other.snr_db.clone();
        }

        self.avg_reduction_db = other.avg_reduction_db;
        self.learning_active = other.learning_active;
        self.is_learning_noise = other.is_learning_noise;
        self.has_captured_profile = other.has_captured_profile;
        self.learning_progress = other.learning_progress;
        self.using_captured_profile = other.using_captured_profile;
    }
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Spectral denoiser using Wiener filter with MCRA noise estimation
pub struct DenoiserPlugin {
    // Configuration
    channels: usize,
    fft_size: usize,
    hop_size: usize,
    sample_rate: u32,
    spectrum_size: usize, // fft_size / 2 + 1

    // FFT planners
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,

    // Parameters (runtime adjustable)
    reduction_db: f32,
    floor_db: f32,
    smoothing: f32,
    attack_ms: f32,
    release_ms: f32,
    low_latency: bool,
    polyphonic_detection: bool,
    crack_sensitivity: f32,

    // Transparency: blend gain toward 1.0 (0 = full denoising, 1 = pass-through)
    transparency: f32,

    // Decision-Directed SNR parameters
    dd_enabled: bool,
    dd_alpha: f32,
    prev_power: Vec<Vec<f32>>, // [channels][spectrum_size] previous frame power

    // Psychoacoustic masking
    psychoacoustic_masking: bool,
    bark_map: Vec<f32>, // [spectrum_size] frequency-to-Bark mapping
    bark_bin_range: Vec<(usize, usize)>, // [spectrum_size] precomputed (lo, hi) bin range within MAX_SPREAD_BARK
    masking_threshold: Vec<f32>,         // [spectrum_size] scratch for masking thresholds
    masking_signal_power: Vec<f32>,      // [spectrum_size] scratch for signal power

    // Noise profile capture
    use_captured_profile: bool,
    has_noise_profile: bool,
    noise_profile_storage: Vec<Vec<f32>>, // [channels][spectrum_size] pre-allocated
    learning_accumulator: Vec<Vec<f32>>,  // [channels][spectrum_size]
    learning_frames_count: usize,
    learning_frames_target: usize,
    is_learning: bool,

    // Pre-computed coefficients
    attack_coeff: f32,
    release_coeff: f32,
    reduction_linear: f32,
    floor_linear: f32,

    // sqrt(Hann) analysis window
    window: Vec<f32>,
    // Precomputed synthesis window: sqrt(Hann)[i] / fft_size
    synthesis_window: Vec<f32>,

    // Processing buffers (per-channel)
    time_domain: Vec<Vec<f32>>,          // [channels][fft_size]
    freq_domain: Vec<Vec<Complex<f32>>>, // [channels][spectrum_size]

    // MCRA state (per-channel, per-bin)
    noise_psd: Vec<Vec<f32>>,       // Estimated noise power spectrum
    smoothed_psd: Vec<Vec<f32>>,    // Smoothed signal PSD (S_tmp)
    min_psd: Vec<Vec<f32>>,         // Minimum PSD tracker — window A
    min_psd_b: Vec<Vec<f32>>,       // Minimum PSD tracker — window B (IMCRA)
    speech_presence: Vec<Vec<f32>>, // Speech presence probability (p)
    frame_counter: Vec<usize>,      // Per-channel frame count

    // Wiener filter state
    gain: Vec<Vec<f32>>,          // Current Wiener gains per bin
    smoothed_gain: Vec<Vec<f32>>, // Temporally smoothed gains

    // Frequency smoothing scratch buffer and precomputed kernel
    freq_smooth_temp: Vec<f32>, // [spectrum_size] scratch for smoothing across bins
    freq_smooth_kernel: (f32, f32, f32), // Precomputed (c0, c1, c2) Gaussian weights

    // Overlap-add buffers
    input_buffer: Vec<f32>, // Interleaved input accumulator
    input_buffer_fill: usize,
    temp_input_block: Vec<f32>, // Pre-allocated block for FFT input

    // Ring-buffer output accumulator (per-channel, power-of-2 capacity)
    output_accumulator: Vec<Vec<f32>>, // [channels][ring_capacity]
    output_ring_mask: usize,           // ring_capacity - 1 (for & masking)
    output_read_pos: usize,            // read position in ring
    output_write_pos: usize,           // next overlap-add write position
    output_accumulator_fill: usize,    // frames available for reading

    // Output time-domain buffers
    time_out_channels: Vec<Vec<f32>>,

    // MCRA parameters
    mcra_alpha_s: f32,
    mcra_alpha_p: f32,
    mcra_l: usize,
    mcra_delta: f32,

    // Technique enable flags
    transient_enabled: bool,
    spectral_smoothing_enabled: bool,
    temporal_smoothing_enabled: bool,

    // Hiss remover
    hiss_enabled: bool,
    hiss_threshold_db: f32,
    hiss_frequency_hz: f32,
    hiss_strength: f32,
    hiss_cutoff_bin: usize,
    hiss_threshold_linear: f32,

    // Spectral subtraction
    spectral_sub_enabled: bool,
    spectral_sub_alpha: f32,
    spectral_sub_beta: f32,

    // Transient Suppressor
    transient_suppressor: transient::TransientSuppressor,

    // PND Analyzers for polyphonic detection
    pnd_analyzers: Vec<PndAnalyzer>,

    // Formant preservation
    formant_preserver: wiener::FormantPreserver,

    // Multi-resolution dual-STFT processing
    multi_resolution: bool,
    /// `Some(state)` when multi_resolution is enabled, `None` otherwise.
    /// Stored as an Option so that when disabled the extra RAM is not held.
    multi_res_state: Option<multi_resolution::MultiResState>,

    // Algorithm selection (Choice param, index 29)
    algorithm: usize,

    // --- Phase 4B: SOTA additions ---
    harmonic_percussive: bool,
    spatial_denoise: bool,
    spatial_strength: f32,
    /// Per-channel tonal/transient separator for harmonic/percussive mode
    tonal_transient_seps: Vec<math_audio_dsp::tonal_transient::TonalTransientSeparator>,
    /// Scratch buffers for tonal/transient masks [spectrum_size]
    tt_magnitudes: Vec<f32>,
    tt_tonal_mask: Vec<f32>,
    tt_transient_mask: Vec<f32>,

    // Data exposure for UI — cached to avoid allocations in get_data()
    avg_reduction_db: f32,
    learning_active: bool,
    cache: RealTimeCache<DenoiserData>,
    data_update_counter: usize,
    cached_noise_floor_buf: Vec<f32>, // [NUM_DISPLAY_BANDS] pre-allocated
    cached_snr_buf: Vec<f32>,         // [NUM_DISPLAY_BANDS] pre-allocated
    cached_parameters: Vec<Parameter>,
}

impl DenoiserPlugin {
    /// Create a new denoiser plugin
    pub fn new(channels: usize, low_latency: bool) -> Self {
        // Choose FFT size based on latency mode
        let fft_size = if low_latency { 512 } else { 2048 };
        let hop_size = fft_size / 2;
        let spectrum_size = fft_size / 2 + 1;

        // Create FFT planners
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        // Generate sqrt(Hann) window for WOLA processing
        // Analysis: sqrt(Hann), Synthesis: sqrt(Hann), Product = Hann → perfect COLA at 50% overlap
        let window = sotf_host::stft_common::generate_sqrt_hann_window(fft_size);
        let inv_fft_size = 1.0 / fft_size as f32;
        let synthesis_window: Vec<f32> = window.iter().map(|w| w * inv_fft_size).collect();

        // Allocate buffers
        let time_domain = vec![vec![0.0_f32; fft_size]; channels];
        let freq_domain = vec![vec![Complex::new(0.0, 0.0); spectrum_size]; channels];

        // MCRA state
        let noise_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let smoothed_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let min_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let min_psd_b = vec![vec![0.0_f32; spectrum_size]; channels];
        let speech_presence = vec![vec![0.0_f32; spectrum_size]; channels];
        let frame_counter = vec![0_usize; channels];

        // Wiener filter state
        let gain = vec![vec![1.0_f32; spectrum_size]; channels];
        let smoothed_gain = vec![vec![1.0_f32; spectrum_size]; channels];

        // Overlap-add buffers
        // Input buffer needs to hold fft_size samples (interleaved)
        let input_buffer = vec![0.0_f32; fft_size * channels * 2];
        // Output ring buffer: power-of-2 capacity >= fft_size * 4
        let ring_capacity = (fft_size * 4).next_power_of_two();
        let output_accumulator = vec![vec![0.0_f32; ring_capacity]; channels];
        let time_out_channels = vec![vec![0.0_f32; fft_size]; channels];

        // PND Analyzers for polyphonic detection
        let pnd_analyzers = (0..channels)
            .map(|_| PndAnalyzer::new(2048, 44100, 50.0))
            .collect();

        let mut p = Self {
            channels,
            fft_size,
            hop_size,
            sample_rate: 44100, // Updated in initialize()
            spectrum_size,

            fft_forward,
            fft_inverse,

            reduction_db: pk(DN, "reduction_db").default_f32(),
            floor_db: pk(DN, "floor_db").default_f32(),
            smoothing: pk(DN, "smoothing").default_f32(),
            attack_ms: pk(DN, "attack_ms").default_f32(),
            release_ms: pk(DN, "release_ms").default_f32(),
            low_latency,
            polyphonic_detection: pk(DN, "polyphonic_detection").default_bool(),
            crack_sensitivity: pk(DN, "crack_sensitivity").default_f32(),
            transparency: pk(DN, "transparency").default_f32(),

            dd_enabled: pk(DN, "dd_enabled").default_bool(),
            dd_alpha: pk(DN, "dd_alpha").default_f32(),
            prev_power: vec![vec![0.0_f32; spectrum_size]; channels],

            psychoacoustic_masking: pk(DN, "psychoacoustic_masking").default_bool(),
            bark_map: vec![0.0_f32; spectrum_size],
            bark_bin_range: vec![(0, 0); spectrum_size],
            masking_threshold: vec![0.0_f32; spectrum_size],
            masking_signal_power: vec![0.0_f32; spectrum_size],

            use_captured_profile: pk(DN, "use_captured_profile").default_bool(),
            has_noise_profile: false,
            noise_profile_storage: vec![vec![0.0_f32; spectrum_size]; channels],
            learning_accumulator: vec![vec![0.0_f32; spectrum_size]; channels],
            learning_frames_count: 0,
            learning_frames_target: crate::params::LEARN_FRAMES,
            is_learning: false,

            attack_coeff: Self::time_to_coeff(pk(DN, "attack_ms").default_f32(), 44100, hop_size),
            release_coeff: Self::time_to_coeff(pk(DN, "release_ms").default_f32(), 44100, hop_size),
            reduction_linear: 10.0_f32.powf(pk(DN, "reduction_db").default_f32() / 10.0),
            floor_linear: 10.0_f32.powf(pk(DN, "floor_db").default_f32() / 20.0),

            window,
            synthesis_window,

            time_domain,
            freq_domain,

            noise_psd,
            smoothed_psd,
            min_psd,
            min_psd_b,
            speech_presence,
            frame_counter,

            gain,
            smoothed_gain,

            freq_smooth_temp: vec![0.0_f32; spectrum_size],
            freq_smooth_kernel: Self::compute_smoothing_kernel(pk(DN, "smoothing").default_f32()),

            input_buffer,
            input_buffer_fill: 0,
            temp_input_block: vec![0.0_f32; fft_size * channels],
            output_accumulator,
            output_ring_mask: ring_capacity - 1,
            output_read_pos: 0,
            output_write_pos: 0,
            output_accumulator_fill: 0,

            time_out_channels,

            mcra_alpha_s: pk(DN, "mcra_alpha_s").default_f32(),
            mcra_alpha_p: pk(DN, "mcra_alpha_p").default_f32(),
            mcra_l: pk(DN, "mcra_l").default_usize(),
            mcra_delta: pk(DN, "mcra_delta").default_f32(),

            transient_enabled: pk(DN, "transient_enabled").default_bool(),
            spectral_smoothing_enabled: pk(DN, "spectral_smoothing_enabled").default_bool(),
            temporal_smoothing_enabled: pk(DN, "temporal_smoothing_enabled").default_bool(),

            hiss_enabled: pk(DN, "hiss_enabled").default_bool(),
            hiss_threshold_db: pk(DN, "hiss_threshold_db").default_f32(),
            hiss_frequency_hz: pk(DN, "hiss_frequency_hz").default_f32(),
            hiss_strength: pk(DN, "hiss_strength").default_f32(),
            hiss_cutoff_bin: 0, // computed in initialize()
            hiss_threshold_linear: 10.0_f32.powf(pk(DN, "hiss_threshold_db").default_f32() / 10.0),

            spectral_sub_enabled: pk(DN, "spectral_sub_enabled").default_bool(),
            spectral_sub_alpha: pk(DN, "spectral_sub_alpha").default_f32(),
            spectral_sub_beta: pk(DN, "spectral_sub_beta").default_f32(),

            transient_suppressor: transient::TransientSuppressor::new(channels),
            pnd_analyzers,

            formant_preserver: wiener::FormantPreserver::new(spectrum_size),

            multi_resolution: pk(DN, "multi_resolution").default_bool(),

            algorithm: pk(DN, "algorithm").default_usize(),

            harmonic_percussive: false,
            spatial_denoise: false,
            spatial_strength: 0.5,
            tonal_transient_seps: (0..channels)
                .map(|_| math_audio_dsp::tonal_transient::TonalTransientSeparator::new(spectrum_size, 7, 7))
                .collect(),
            tt_magnitudes: vec![0.0; spectrum_size],
            tt_tonal_mask: vec![0.0; spectrum_size],
            tt_transient_mask: vec![0.0; spectrum_size],
            multi_res_state: None, // allocated on first enable

            avg_reduction_db: 0.0,
            learning_active: true,
            cache: RealTimeCache::new(DenoiserData::default()),
            data_update_counter: 0,
            cached_noise_floor_buf: vec![0.0; NUM_DISPLAY_BANDS],
            cached_snr_buf: vec![0.0; NUM_DISPLAY_BANDS],
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly (36 params).
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.reduction_db as f64),
            1 => Some(self.floor_db as f64),
            2 => Some(self.smoothing as f64),
            3 => Some(self.attack_ms as f64),
            4 => Some(self.release_ms as f64),
            5 => Some(if self.low_latency { 1.0 } else { 0.0 }),
            6 => Some(if self.polyphonic_detection { 1.0 } else { 0.0 }),
            7 => Some(self.crack_sensitivity as f64),
            8 => Some(self.mcra_alpha_s as f64),
            9 => Some(self.mcra_alpha_p as f64),
            10 => Some(self.mcra_l as f64),
            11 => Some(self.mcra_delta as f64),
            12 => Some(self.transparency as f64),
            13 => Some(if self.dd_enabled { 1.0 } else { 0.0 }),
            14 => Some(self.dd_alpha as f64),
            15 => Some(if self.psychoacoustic_masking { 1.0 } else { 0.0 }),
            16 => Some(if self.transient_enabled { 1.0 } else { 0.0 }),
            17 => Some(if self.spectral_smoothing_enabled { 1.0 } else { 0.0 }),
            18 => Some(if self.temporal_smoothing_enabled { 1.0 } else { 0.0 }),
            19 => Some(if self.hiss_enabled { 1.0 } else { 0.0 }),
            20 => Some(self.hiss_threshold_db as f64),
            21 => Some(self.hiss_frequency_hz as f64),
            22 => Some(self.hiss_strength as f64),
            23 => Some(if self.spectral_sub_enabled { 1.0 } else { 0.0 }),
            24 => Some(self.spectral_sub_alpha as f64),
            25 => Some(self.spectral_sub_beta as f64),
            26 => Some(if self.is_learning { 1.0 } else { 0.0 }),
            27 => Some(if self.use_captured_profile { 1.0 } else { 0.0 }),
            28 => Some(0.0), // clear_profile: trigger-only, always reads as false
            29 => Some(self.algorithm as f64),
            30 => Some(if self.formant_preserver.enabled { 1.0 } else { 0.0 }),
            31 => Some(self.formant_preserver.strength as f64),
            32 => Some(if self.multi_resolution { 1.0 } else { 0.0 }),
            33 => Some(if self.harmonic_percussive { 1.0 } else { 0.0 }),
            34 => Some(if self.spatial_denoise { 1.0 } else { 0.0 }),
            35 => Some(self.spatial_strength as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly (36 params).
    /// Side effects are dispatched separately after param_bridge::set_parameter.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.reduction_db = value as f32,
            1 => self.floor_db = value as f32,
            2 => self.smoothing = value as f32,
            3 => self.attack_ms = value as f32,
            4 => self.release_ms = value as f32,
            5 => self.low_latency = value > 0.5,
            6 => self.polyphonic_detection = value > 0.5,
            7 => self.crack_sensitivity = value as f32,
            8 => self.mcra_alpha_s = value as f32,
            9 => self.mcra_alpha_p = value as f32,
            10 => self.mcra_l = value as usize,
            11 => self.mcra_delta = value as f32,
            12 => self.transparency = value as f32,
            13 => self.dd_enabled = value > 0.5,
            14 => self.dd_alpha = value as f32,
            15 => self.psychoacoustic_masking = value > 0.5,
            16 => self.transient_enabled = value > 0.5,
            17 => self.spectral_smoothing_enabled = value > 0.5,
            18 => self.temporal_smoothing_enabled = value > 0.5,
            19 => self.hiss_enabled = value > 0.5,
            20 => self.hiss_threshold_db = value as f32,
            21 => self.hiss_frequency_hz = value as f32,
            22 => self.hiss_strength = value as f32,
            23 => self.spectral_sub_enabled = value > 0.5,
            24 => self.spectral_sub_alpha = value as f32,
            25 => self.spectral_sub_beta = value as f32,
            26 => {} // learn_noise: side effect handled in set_parameter
            27 => self.use_captured_profile = value > 0.5,
            28 => {} // clear_profile: side effect handled in set_parameter
            29 => self.algorithm = value as usize,
            30 => self.formant_preserver.enabled = value > 0.5,
            31 => self.formant_preserver.strength = value as f32,
            32 => self.multi_resolution = value > 0.5,
            33 => self.harmonic_percussive = value > 0.5,
            34 => self.spatial_denoise = value > 0.5,
            35 => self.spatial_strength = value as f32,
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(DN, |i| self.param_value(i));
    }

    /// Create a new denoiser plugin from configuration parameters
    pub fn from_params(channels: usize, params: DenoiserPluginParams) -> Self {
        let mut plugin = Self::new(channels, params.low_latency);

        plugin.reduction_db = params.reduction_db.clamp(
            pk(DN, "reduction_db").min_f64() as f32,
            pk(DN, "reduction_db").max_f64() as f32,
        );
        plugin.floor_db = params.floor_db.clamp(
            pk(DN, "floor_db").min_f64() as f32,
            pk(DN, "floor_db").max_f64() as f32,
        );
        plugin.smoothing = params.smoothing.clamp(
            pk(DN, "smoothing").min_f64() as f32,
            pk(DN, "smoothing").max_f64() as f32,
        );
        plugin.attack_ms = params.attack_ms.clamp(
            pk(DN, "attack_ms").min_f64() as f32,
            pk(DN, "attack_ms").max_f64() as f32,
        );
        plugin.release_ms = params.release_ms.clamp(
            pk(DN, "release_ms").min_f64() as f32,
            pk(DN, "release_ms").max_f64() as f32,
        );
        plugin.polyphonic_detection = params.polyphonic_detection;
        plugin.crack_sensitivity = params
            .crack_sensitivity
            .max(pk(DN, "crack_sensitivity").min_f64() as f32);
        plugin
            .transient_suppressor
            .set_sensitivity(plugin.crack_sensitivity);

        plugin.mcra_alpha_s = params.mcra_alpha_s;
        plugin.mcra_alpha_p = params.mcra_alpha_p;
        plugin.mcra_l = params.mcra_l.max(1);
        plugin.mcra_delta = params.mcra_delta;

        plugin.transparency = params.transparency.clamp(
            pk(DN, "transparency").min_f64() as f32,
            pk(DN, "transparency").max_f64() as f32,
        );
        plugin.dd_enabled = params.dd_enabled;
        plugin.dd_alpha = params.dd_alpha.clamp(
            pk(DN, "dd_alpha").min_f64() as f32,
            pk(DN, "dd_alpha").max_f64() as f32,
        );
        plugin.psychoacoustic_masking = params.psychoacoustic_masking;
        plugin.use_captured_profile = params.use_captured_profile;
        plugin.transient_enabled = params.transient_enabled;
        plugin.spectral_smoothing_enabled = params.spectral_smoothing_enabled;
        plugin.temporal_smoothing_enabled = params.temporal_smoothing_enabled;

        plugin.hiss_enabled = params.hiss_enabled;
        plugin.hiss_threshold_db = params.hiss_threshold_db;
        plugin.hiss_frequency_hz = params.hiss_frequency_hz;
        plugin.hiss_strength = params.hiss_strength.clamp(
            pk(DN, "hiss_strength").min_f64() as f32,
            pk(DN, "hiss_strength").max_f64() as f32,
        );
        plugin.update_hiss_threshold_linear();

        plugin.spectral_sub_enabled = params.spectral_sub_enabled;
        plugin.spectral_sub_alpha = params.spectral_sub_alpha.clamp(
            pk(DN, "spectral_sub_alpha").min_f64() as f32,
            pk(DN, "spectral_sub_alpha").max_f64() as f32,
        );
        plugin.spectral_sub_beta = params.spectral_sub_beta.clamp(
            pk(DN, "spectral_sub_beta").min_f64() as f32,
            pk(DN, "spectral_sub_beta").max_f64() as f32,
        );

        plugin.reduction_linear = 10.0_f32.powf(plugin.reduction_db / 10.0);
        plugin.floor_linear = 10.0_f32.powf(plugin.floor_db / 20.0);
        plugin.freq_smooth_kernel = Self::compute_smoothing_kernel(plugin.smoothing);

        plugin.formant_preserver.enabled = params.formant_preservation;
        plugin.formant_preserver.strength = params.formant_strength.clamp(
            pk(DN, "formant_strength").min_f64() as f32,
            pk(DN, "formant_strength").max_f64() as f32,
        );

        plugin.multi_resolution = params.multi_resolution;
        if plugin.multi_resolution {
            plugin.multi_res_state = Some(multi_resolution::MultiResState::new(
                channels,
                plugin.mcra_alpha_s,
                plugin.mcra_alpha_p,
                plugin.mcra_l,
                plugin.mcra_delta,
            ));
        }

        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Process one FFT block
    fn process_fft_block(&mut self) {
        // Extract block from input buffer (fft_size * channels samples)
        let block_samples = self.fft_size * self.channels;

        // Phase 1: Apply window and forward FFT (must happen before shifting)
        // Copy the block to avoid borrow conflicts with the shift operation
        // Use pre-allocated buffer instead of local Vec::new() / to_vec()
        let mut input_block = std::mem::take(&mut self.temp_input_block);
        input_block[..block_samples].copy_from_slice(&self.input_buffer[..block_samples]);
        self.apply_window_and_forward_fft(&input_block);
        self.temp_input_block = input_block; // Restore buffer

        // Shift input buffer (remove processed samples, keeping hop_size overlap)
        let shift_samples = self.hop_size * self.channels;
        self.input_buffer.copy_within(shift_samples.., 0);
        self.input_buffer_fill -= shift_samples;

        // Phase 2: Noise estimation (multi-frame bootstrap then IMCRA)
        let bootstrapping = self.update_noise_estimation();

        // Phase 2b: Noise profile learning (if active)
        if self.is_learning {
            self.accumulate_noise_frame();
        }

        // Phase 3: Calculate Gains (skip during bootstrap — gains stay at 1.0)
        if !bootstrapping {
            if self.polyphonic_detection {
                self.calculate_polyphonic_gains();
            } else {
                self.calculate_wiener_gains();
            }
        }

        // Phase 3b: Multi-resolution gain combination.
        // The small-FFT path has already been fed samples and computed its own
        // gains.  Blend them into `self.smoothed_gain` based on spectral flux.
        if let Some(ref mrs) = self.multi_res_state {
            mrs.combine_gains(&mut self.smoothed_gain, self.channels, self.spectrum_size);
        }

        // Phase 4: Apply gains and inverse FFT
        self.apply_gains_and_inverse_fft();

        // Phase 5: Overlap-add to output accumulator
        self.overlap_add_to_accumulator();

        // Phase 6: Update cached monitoring data (every 8th block to reduce overhead)
        self.data_update_counter += 1;
        if self.data_update_counter >= 8 {
            self.data_update_counter = 0;
            self.update_cached_data();
        }
    }

    /// Update the cached DenoiserData for UI polling.
    /// Writes into pre-allocated buffers to avoid allocations on the audio thread.
    fn update_cached_data(&mut self) {
        self.compute_noise_floor_db();
        self.compute_snr_db();

        let avg_red = self.avg_reduction_db;
        let learn_act = self.learning_active;
        let is_learn = self.is_learning;
        let has_prof = self.has_noise_profile;
        let progress = self.learning_progress();
        let using_prof = self.use_captured_profile;

        let nf_buf = &self.cached_noise_floor_buf;
        let snr_buf = &self.cached_snr_buf;

        self.cache.update(|d| {
            if let Some(mut_nf) = Arc::get_mut(&mut d.noise_floor_db) {
                mut_nf.copy_from_slice(nf_buf);
            }
            if let Some(mut_snr) = Arc::get_mut(&mut d.snr_db) {
                mut_snr.copy_from_slice(snr_buf);
            }
            d.avg_reduction_db = avg_red;
            d.learning_active = learn_act;
            d.is_learning_noise = is_learn;
            d.has_captured_profile = has_prof;
            d.learning_progress = progress;
            d.using_captured_profile = using_prof;
        });
    }

    /// Add processed block to output ring-buffer accumulator using overlap-add.
    /// Uses modular indexing (& mask) — no bulk shifts.
    fn overlap_add_to_accumulator(&mut self) {
        // WOLA: apply synthesis window (sqrt(Hann) / fft_size) before overlap-add.
        // Analysis sqrt(Hann) * synthesis sqrt(Hann) = Hann → perfect COLA at 50% overlap.
        let mask = self.output_ring_mask;

        for ch in 0..self.channels {
            let accum = &mut self.output_accumulator[ch];
            let time_out = &self.time_out_channels[ch];

            for (i, (t, w)) in time_out
                .iter()
                .zip(self.synthesis_window.iter())
                .enumerate()
                .take(self.fft_size)
            {
                let idx = (self.output_write_pos + i) & mask;
                accum[idx] += t * w;
            }
        }

        // Advance write position by hop_size for next block
        self.output_write_pos = (self.output_write_pos + self.hop_size) & mask;
        self.output_accumulator_fill += self.hop_size;
    }

    /// Drain available frames from ring-buffer accumulator to output buffer.
    /// Returns the number of frames actually drained.
    fn drain_output(
        &mut self,
        output: &mut [f32],
        output_pos: usize,
        frames_wanted: usize,
    ) -> usize {
        let frames_to_drain = self.output_accumulator_fill.min(frames_wanted);
        let mask = self.output_ring_mask;

        for frame in 0..frames_to_drain {
            let ring_idx = (self.output_read_pos + frame) & mask;
            let out_base = (output_pos + frame) * self.channels;
            for ch in 0..self.channels {
                output[out_base + ch] = self.output_accumulator[ch][ring_idx];
                // Clear after reading for next overlap-add cycle
                self.output_accumulator[ch][ring_idx] = 0.0;
            }
        }

        self.output_read_pos = (self.output_read_pos + frames_to_drain) & mask;
        self.output_accumulator_fill -= frames_to_drain;

        frames_to_drain
    }
}

impl InPlacePlugin for DenoiserPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Denoiser", "1.0.0", "SotF")
            .with_description("Wiener filter denoiser with MCRA noise estimation")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let idx = param_bridge::set_parameter(DN, &id, &value, |i, v| self.set_param_value(i, v))?;
        // Side effects based on which parameter changed
        match idx {
            0 => {
                // reduction_db -> recompute reduction_linear
                self.reduction_linear = 10.0_f32.powf(self.reduction_db / 10.0);
            }
            1 => {
                // floor_db -> recompute floor_linear
                self.floor_linear = 10.0_f32.powf(self.floor_db / 20.0);
            }
            2 => {
                // smoothing -> recompute frequency smoothing kernel
                self.freq_smooth_kernel = Self::compute_smoothing_kernel(self.smoothing);
            }
            3 | 4 => {
                // attack_ms or release_ms -> recompute envelope coefficients
                self.update_envelope_coefficients();
            }
            7 => {
                // crack_sensitivity -> update transient suppressor
                self.transient_suppressor
                    .set_sensitivity(self.crack_sensitivity);
            }
            20 => {
                // hiss_threshold_db -> recompute hiss_threshold_linear
                self.update_hiss_threshold_linear();
            }
            21 => {
                // hiss_frequency_hz -> recompute hiss_cutoff_bin
                self.update_hiss_cutoff_bin();
            }
            26 => {
                // learn_noise (trigger param)
                if value.as_bool().unwrap_or(false) {
                    self.start_learning();
                }
            }
            28 => {
                // clear_profile (trigger param)
                if value.as_bool().unwrap_or(false) {
                    self.clear_noise_profile();
                }
            }
            32 => {
                // multi_resolution -> allocate/deallocate state
                if self.multi_resolution && self.multi_res_state.is_none() {
                    self.multi_res_state = Some(multi_resolution::MultiResState::new(
                        self.channels,
                        self.mcra_alpha_s,
                        self.mcra_alpha_p,
                        self.mcra_l,
                        self.mcra_delta,
                    ));
                } else if !self.multi_resolution {
                    self.multi_res_state = None;
                }
            }
            _ => {}
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(DN, id, |i| self.param_value(i))
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_envelope_coefficients();
        self.precompute_bark_mapping();
        self.update_hiss_cutoff_bin();

        // Update PND analyzers with correct sample rate
        for analyzer in &mut self.pnd_analyzers {
            *analyzer = PndAnalyzer::new(2048, sample_rate, 50.0);
        }

        Ok(())
    }

    fn reset(&mut self) {
        // Reset MCRA state
        for ch in 0..self.channels {
            self.reset_mcra(ch);
            self.gain[ch].fill(1.0);
            self.smoothed_gain[ch].fill(1.0);
            self.prev_power[ch].fill(0.0);
            self.learning_accumulator[ch].fill(0.0);
            self.output_accumulator[ch].fill(0.0);
            self.time_out_channels[ch].fill(0.0);
        }
        self.is_learning = false;
        self.learning_frames_count = 0;

        // Reset transient suppressor
        self.transient_suppressor.reset();

        // Reset PND analyzers
        for analyzer in &mut self.pnd_analyzers {
            analyzer.reset();
        }

        // Reset buffers
        self.input_buffer.fill(0.0);
        self.input_buffer_fill = 0;
        self.output_read_pos = 0;
        self.output_write_pos = 0;
        self.output_accumulator_fill = 0;

        // Reset formant preserver working buffers
        self.formant_preserver.log_mag_scratch.fill(0.0);
        self.formant_preserver.envelope.fill(0.0);

        // Reset multi-resolution state
        if let Some(ref mut mrs) = self.multi_res_state {
            mrs.reset();
        }

        self.avg_reduction_db = 0.0;
        self.learning_active = true;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        // Set FTZ+DAZ to flush denormals at hardware level (zero per-sample cost)
        #[cfg(target_arch = "x86_64")]
        let _old_mxcsr = unsafe {
            let mut old: u32 = 0;
            std::arch::asm!("stmxcsr [{}]", in(reg) &mut old, options(nostack, preserves_flags));
            let new = old | 0x8040; // FTZ + DAZ
            std::arch::asm!("ldmxcsr [{}]", in(reg) &new, options(nostack, preserves_flags));
            old
        };

        // Pre-process: Time-domain transient suppression (de-clicking)
        if self.transient_enabled {
            self.transient_suppressor.process(buffer);
        }

        let num_frames = context.num_frames;
        let total_samples = num_frames * self.channels;
        let block_samples = self.fft_size * self.channels;

        // Phase 0: Feed samples to PND analyzers
        if self.polyphonic_detection {
            for i in 0..num_frames {
                for ch in 0..self.channels {
                    self.pnd_analyzers[ch].analyze(&[buffer[i * self.channels + ch]]);
                }
            }
        }

        // Phase 1: Accumulate ALL input into input_buffer.
        // This is an in-place plugin (same buffer for input/output), so we must
        // consume all input before writing any output to avoid data corruption.
        // Loop to handle cases where input exceeds remaining buffer space:
        // process FFT blocks to free space, then continue accumulating.
        //
        // When multi-resolution is enabled we simultaneously feed the same raw
        // samples into the small-FFT accumulator.  We do this first so that the
        // small-FFT gains are ready when process_fft_block() is called below.
        if self.multi_res_state.is_some() {
            // Feed the whole input block into the small-FFT path before
            // the main loop starts.  `feed_and_process` is self-contained
            // and does not touch `buffer` after reading — safe to call here.
            let attack = self.attack_coeff;
            let release = self.release_coeff;
            let reduction = self.reduction_linear;
            let floor = self.floor_linear;
            let channels = self.channels;
            if let Some(ref mut mrs) = self.multi_res_state {
                mrs.feed_and_process(&buffer[..total_samples], channels,
                                      attack, release, reduction, floor);
            }
        }

        let mut input_pos: usize = 0;
        while input_pos < total_samples {
            let space_available = self.input_buffer.len() - self.input_buffer_fill;
            let remaining_input = total_samples - input_pos;
            let samples_to_copy = remaining_input.min(space_available);

            self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                .copy_from_slice(&buffer[input_pos..input_pos + samples_to_copy]);
            self.input_buffer_fill += samples_to_copy;
            input_pos += samples_to_copy;

            // Process FFT blocks to free input buffer space
            while self.input_buffer_fill >= block_samples {
                self.process_fft_block();
            }
        }

        // Phase 2: Process any remaining complete FFT blocks
        while self.input_buffer_fill >= block_samples {
            self.process_fft_block();
        }

        // Phase 3: Drain output to buffer
        let mut output_pos: usize = 0;
        if self.output_accumulator_fill > 0 {
            output_pos = self.drain_output(buffer, 0, num_frames);
        }

        // Zero-fill any remaining output (initial latency period)
        if output_pos < num_frames {
            let zero_start = output_pos * self.channels;
            buffer[zero_start..total_samples].fill(0.0);
        }

        // Restore MXCSR
        #[cfg(target_arch = "x86_64")]
        unsafe {
            std::arch::asm!("ldmxcsr [{}]", in(reg) &_old_mxcsr, options(nostack, preserves_flags));
        }

        // STFT convention: always return num_frames. Buffer is zero-padded for
        // unfilled portions, so downstream plugins see valid (silent) data.
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        // Latency is fft_size due to overlap-add buffering
        self.fft_size
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}
