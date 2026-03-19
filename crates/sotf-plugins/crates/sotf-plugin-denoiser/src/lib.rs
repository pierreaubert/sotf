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
use sotf_host::param_specs::{denoiser::PARAMS as DN, find_by_key as pk};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
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
    param_reduction_db: ParameterId,
    reduction_db: f32,

    param_floor_db: ParameterId,
    floor_db: f32,

    param_smoothing: ParameterId,
    smoothing: f32,

    param_attack_ms: ParameterId,
    attack_ms: f32,

    param_release_ms: ParameterId,
    release_ms: f32,

    param_low_latency: ParameterId,
    low_latency: bool,

    param_polyphonic_detection: ParameterId,
    polyphonic_detection: bool,

    param_crack_sensitivity: ParameterId,
    crack_sensitivity: f32,

    // Transparency: blend gain toward 1.0 (0 = full denoising, 1 = pass-through)
    param_transparency: ParameterId,
    transparency: f32,

    // Decision-Directed SNR parameters
    param_dd_enabled: ParameterId,
    dd_enabled: bool,
    param_dd_alpha: ParameterId,
    dd_alpha: f32,
    prev_power: Vec<Vec<f32>>, // [channels][spectrum_size] previous frame power

    // Psychoacoustic masking
    param_psychoacoustic_masking: ParameterId,
    psychoacoustic_masking: bool,
    bark_map: Vec<f32>, // [spectrum_size] frequency-to-Bark mapping
    bark_bin_range: Vec<(usize, usize)>, // [spectrum_size] precomputed (lo, hi) bin range within MAX_SPREAD_BARK
    masking_threshold: Vec<f32>,         // [spectrum_size] scratch for masking thresholds
    masking_signal_power: Vec<f32>,      // [spectrum_size] scratch for signal power

    // Noise profile capture
    param_learn_noise: ParameterId,
    param_use_captured_profile: ParameterId,
    param_clear_profile: ParameterId,
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
    param_transient_enabled: ParameterId,
    transient_enabled: bool,
    param_spectral_smoothing_enabled: ParameterId,
    spectral_smoothing_enabled: bool,
    param_temporal_smoothing_enabled: ParameterId,
    temporal_smoothing_enabled: bool,

    // Hiss remover
    param_hiss_enabled: ParameterId,
    hiss_enabled: bool,
    param_hiss_threshold_db: ParameterId,
    hiss_threshold_db: f32,
    param_hiss_frequency_hz: ParameterId,
    hiss_frequency_hz: f32,
    param_hiss_strength: ParameterId,
    hiss_strength: f32,
    hiss_cutoff_bin: usize,
    hiss_threshold_linear: f32,

    // Spectral subtraction
    param_spectral_sub_enabled: ParameterId,
    spectral_sub_enabled: bool,
    param_spectral_sub_alpha: ParameterId,
    spectral_sub_alpha: f32,
    param_spectral_sub_beta: ParameterId,
    spectral_sub_beta: f32,

    // Transient Suppressor
    transient_suppressor: transient::TransientSuppressor,

    // PND Analyzers for polyphonic detection
    pnd_analyzers: Vec<PndAnalyzer>,

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

            param_reduction_db: ParameterId::from("reduction_db"),
            reduction_db: pk(DN, "reduction_db").default_f32(),

            param_floor_db: ParameterId::from("floor_db"),
            floor_db: pk(DN, "floor_db").default_f32(),

            param_smoothing: ParameterId::from("smoothing"),
            smoothing: pk(DN, "smoothing").default_f32(),

            param_attack_ms: ParameterId::from("attack_ms"),
            attack_ms: pk(DN, "attack_ms").default_f32(),

            param_release_ms: ParameterId::from("release_ms"),
            release_ms: pk(DN, "release_ms").default_f32(),

            param_low_latency: ParameterId::from("low_latency"),
            low_latency,

            param_polyphonic_detection: ParameterId::from("polyphonic_detection"),
            polyphonic_detection: pk(DN, "polyphonic_detection").default_bool(),

            param_crack_sensitivity: ParameterId::from("crack_sensitivity"),
            crack_sensitivity: pk(DN, "crack_sensitivity").default_f32(),

            param_transparency: ParameterId::from("transparency"),
            transparency: pk(DN, "transparency").default_f32(),

            param_dd_enabled: ParameterId::from("dd_enabled"),
            dd_enabled: pk(DN, "dd_enabled").default_bool(),
            param_dd_alpha: ParameterId::from("dd_alpha"),
            dd_alpha: pk(DN, "dd_alpha").default_f32(),
            prev_power: vec![vec![0.0_f32; spectrum_size]; channels],

            param_psychoacoustic_masking: ParameterId::from("psychoacoustic_masking"),
            psychoacoustic_masking: pk(DN, "psychoacoustic_masking").default_bool(),
            bark_map: vec![0.0_f32; spectrum_size],
            bark_bin_range: vec![(0, 0); spectrum_size],
            masking_threshold: vec![0.0_f32; spectrum_size],
            masking_signal_power: vec![0.0_f32; spectrum_size],

            param_learn_noise: ParameterId::from("learn_noise"),
            param_use_captured_profile: ParameterId::from("use_captured_profile"),
            param_clear_profile: ParameterId::from("clear_profile"),
            use_captured_profile: pk(DN, "use_captured_profile").default_bool(),
            has_noise_profile: false,
            noise_profile_storage: vec![vec![0.0_f32; spectrum_size]; channels],
            learning_accumulator: vec![vec![0.0_f32; spectrum_size]; channels],
            learning_frames_count: 0,
            learning_frames_target: sotf_host::param_specs::denoiser::LEARN_FRAMES,
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

            param_transient_enabled: ParameterId::from("transient_enabled"),
            transient_enabled: pk(DN, "transient_enabled").default_bool(),
            param_spectral_smoothing_enabled: ParameterId::from("spectral_smoothing_enabled"),
            spectral_smoothing_enabled: pk(DN, "spectral_smoothing_enabled").default_bool(),
            param_temporal_smoothing_enabled: ParameterId::from("temporal_smoothing_enabled"),
            temporal_smoothing_enabled: pk(DN, "temporal_smoothing_enabled").default_bool(),

            param_hiss_enabled: ParameterId::from("hiss_enabled"),
            hiss_enabled: pk(DN, "hiss_enabled").default_bool(),
            param_hiss_threshold_db: ParameterId::from("hiss_threshold_db"),
            hiss_threshold_db: pk(DN, "hiss_threshold_db").default_f32(),
            param_hiss_frequency_hz: ParameterId::from("hiss_frequency_hz"),
            hiss_frequency_hz: pk(DN, "hiss_frequency_hz").default_f32(),
            param_hiss_strength: ParameterId::from("hiss_strength"),
            hiss_strength: pk(DN, "hiss_strength").default_f32(),
            hiss_cutoff_bin: 0, // computed in initialize()
            hiss_threshold_linear: 10.0_f32.powf(pk(DN, "hiss_threshold_db").default_f32() / 10.0),

            param_spectral_sub_enabled: ParameterId::from("spectral_sub_enabled"),
            spectral_sub_enabled: pk(DN, "spectral_sub_enabled").default_bool(),
            param_spectral_sub_alpha: ParameterId::from("spectral_sub_alpha"),
            spectral_sub_alpha: pk(DN, "spectral_sub_alpha").default_f32(),
            param_spectral_sub_beta: ParameterId::from("spectral_sub_beta"),
            spectral_sub_beta: pk(DN, "spectral_sub_beta").default_f32(),

            transient_suppressor: transient::TransientSuppressor::new(channels),
            pnd_analyzers,

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

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "reduction_db",
                "Reduction",
                self.reduction_db,
                pk(DN, "reduction_db").min_f64() as f32,
                pk(DN, "reduction_db").max_f64() as f32,
            )
            .with_unit("dB")
            .with_group("Noise Reduction")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "floor_db",
                "Floor",
                self.floor_db,
                pk(DN, "floor_db").min_f64() as f32,
                pk(DN, "floor_db").max_f64() as f32,
            )
            .with_unit("dB")
            .with_group("Noise Reduction")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "smoothing",
                "Spectral Smoothing",
                self.smoothing,
                pk(DN, "smoothing").min_f64() as f32,
                pk(DN, "smoothing").max_f64() as f32,
            )
            .with_group("Noise Reduction")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "attack_ms",
                "Attack",
                self.attack_ms,
                pk(DN, "attack_ms").min_f64() as f32,
                pk(DN, "attack_ms").max_f64() as f32,
            )
            .with_unit("ms")
            .with_group("Envelope")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "release_ms",
                "Release",
                self.release_ms,
                pk(DN, "release_ms").min_f64() as f32,
                pk(DN, "release_ms").max_f64() as f32,
            )
            .with_unit("ms")
            .with_group("Envelope")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("low_latency", "Low Latency", self.low_latency)
                .with_group("General")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool(
                "polyphonic_detection",
                "Polyphonic Detection",
                self.polyphonic_detection,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "crack_sensitivity",
                "Crack Sensitivity",
                self.crack_sensitivity,
                pk(DN, "crack_sensitivity").min_f64() as f32,
                pk(DN, "crack_sensitivity").max_f64() as f32,
            )
            .with_group("Transient")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "transparency",
                "Transparency",
                self.transparency,
                pk(DN, "transparency").min_f64() as f32,
                pk(DN, "transparency").max_f64() as f32,
            )
            .with_group("Noise Reduction")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "psychoacoustic_masking",
                "Psychoacoustic Masking",
                self.psychoacoustic_masking,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("dd_enabled", "Dialogue Detection", self.dd_enabled)
                .with_group("Analysis")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "dd_alpha",
                "DD Sensitivity",
                self.dd_alpha,
                pk(DN, "dd_alpha").min_f64() as f32,
                pk(DN, "dd_alpha").max_f64() as f32,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("transient_enabled", "Transient", self.transient_enabled)
                .with_group("Analysis")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "spectral_smoothing_enabled",
                "Spectral Smoothing",
                self.spectral_smoothing_enabled,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "temporal_smoothing_enabled",
                "Temporal Smoothing",
                self.temporal_smoothing_enabled,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("hiss_enabled", "Hiss Remover", self.hiss_enabled)
                .with_group("Hiss")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "hiss_threshold_db",
                "Hiss Threshold",
                self.hiss_threshold_db,
                pk(DN, "hiss_threshold_db").min_f64() as f32,
                pk(DN, "hiss_threshold_db").max_f64() as f32,
            )
            .with_unit("dB")
            .with_group("Hiss")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "hiss_frequency_hz",
                "Hiss Frequency",
                self.hiss_frequency_hz,
                pk(DN, "hiss_frequency_hz").min_f64() as f32,
                pk(DN, "hiss_frequency_hz").max_f64() as f32,
            )
            .with_unit("Hz")
            .with_group("Hiss")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "hiss_strength",
                "Hiss Strength",
                self.hiss_strength,
                pk(DN, "hiss_strength").min_f64() as f32,
                pk(DN, "hiss_strength").max_f64() as f32,
            )
            .with_group("Hiss")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool(
                "spectral_sub_enabled",
                "Spectral Subtraction",
                self.spectral_sub_enabled,
            )
            .with_group("Spectral Sub")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "spectral_sub_alpha",
                "Oversubtraction",
                self.spectral_sub_alpha,
                pk(DN, "spectral_sub_alpha").min_f64() as f32,
                pk(DN, "spectral_sub_alpha").max_f64() as f32,
            )
            .with_group("Spectral Sub")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "spectral_sub_beta",
                "Spectral Floor",
                self.spectral_sub_beta,
                pk(DN, "spectral_sub_beta").min_f64() as f32,
                pk(DN, "spectral_sub_beta").max_f64() as f32,
            )
            .with_group("Spectral Sub")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("learn_noise", "Learn Noise", false).with_group("Profile"),
            Parameter::new_bool(
                "use_captured_profile",
                "Use Captured Profile",
                self.use_captured_profile,
            )
            .with_group("Profile"),
            Parameter::new_bool("clear_profile", "Clear Profile", false).with_group("Profile"),
        ];
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
        self.validate_parameter(&id, &value)?;

        if id == self.param_reduction_db {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "reduction_db").default_f32());
            if val.is_finite() {
                self.reduction_db = val.clamp(
                    pk(DN, "reduction_db").min_f64() as f32,
                    pk(DN, "reduction_db").max_f64() as f32,
                );
                self.reduction_linear = 10.0_f32.powf(self.reduction_db / 10.0);
            }
        } else if id == self.param_floor_db {
            let val = value.as_float().unwrap_or(pk(DN, "floor_db").default_f32());
            if val.is_finite() {
                self.floor_db = val.clamp(
                    pk(DN, "floor_db").min_f64() as f32,
                    pk(DN, "floor_db").max_f64() as f32,
                );
                self.floor_linear = 10.0_f32.powf(self.floor_db / 20.0);
            }
        } else if id == self.param_smoothing {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "smoothing").default_f32());
            if val.is_finite() {
                self.smoothing = val.clamp(
                    pk(DN, "smoothing").min_f64() as f32,
                    pk(DN, "smoothing").max_f64() as f32,
                );
                self.freq_smooth_kernel = Self::compute_smoothing_kernel(self.smoothing);
            }
        } else if id == self.param_attack_ms {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "attack_ms").default_f32());
            if val.is_finite() {
                self.attack_ms = val.clamp(
                    pk(DN, "attack_ms").min_f64() as f32,
                    pk(DN, "attack_ms").max_f64() as f32,
                );
                self.update_envelope_coefficients();
            }
        } else if id == self.param_release_ms {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "release_ms").default_f32());
            if val.is_finite() {
                self.release_ms = val.clamp(
                    pk(DN, "release_ms").min_f64() as f32,
                    pk(DN, "release_ms").max_f64() as f32,
                );
                self.update_envelope_coefficients();
            }
        } else if id == self.param_low_latency {
            self.low_latency = value
                .as_bool()
                .unwrap_or(pk(DN, "low_latency").default_bool());
        } else if id == self.param_polyphonic_detection {
            self.polyphonic_detection = value
                .as_bool()
                .unwrap_or(pk(DN, "polyphonic_detection").default_bool());
        } else if id == self.param_crack_sensitivity {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "crack_sensitivity").default_f32());
            if val.is_finite() {
                self.crack_sensitivity = val.max(pk(DN, "crack_sensitivity").min_f64() as f32);
                self.transient_suppressor
                    .set_sensitivity(self.crack_sensitivity);
            }
        } else if id == self.param_transparency {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "transparency").default_f32());
            if val.is_finite() {
                self.transparency = val.clamp(
                    pk(DN, "transparency").min_f64() as f32,
                    pk(DN, "transparency").max_f64() as f32,
                );
            }
        } else if id == self.param_psychoacoustic_masking {
            self.psychoacoustic_masking = value
                .as_bool()
                .unwrap_or(pk(DN, "psychoacoustic_masking").default_bool());
        } else if id == self.param_dd_enabled {
            self.dd_enabled = value
                .as_bool()
                .unwrap_or(pk(DN, "dd_enabled").default_bool());
        } else if id == self.param_dd_alpha {
            let val = value.as_float().unwrap_or(pk(DN, "dd_alpha").default_f32());
            if val.is_finite() {
                self.dd_alpha = val.clamp(
                    pk(DN, "dd_alpha").min_f64() as f32,
                    pk(DN, "dd_alpha").max_f64() as f32,
                );
            }
        } else if id == self.param_transient_enabled {
            self.transient_enabled = value
                .as_bool()
                .unwrap_or(pk(DN, "transient_enabled").default_bool());
        } else if id == self.param_spectral_smoothing_enabled {
            self.spectral_smoothing_enabled = value
                .as_bool()
                .unwrap_or(pk(DN, "spectral_smoothing_enabled").default_bool());
        } else if id == self.param_temporal_smoothing_enabled {
            self.temporal_smoothing_enabled = value
                .as_bool()
                .unwrap_or(pk(DN, "temporal_smoothing_enabled").default_bool());
        } else if id == self.param_hiss_enabled {
            self.hiss_enabled = value
                .as_bool()
                .unwrap_or(pk(DN, "hiss_enabled").default_bool());
        } else if id == self.param_hiss_threshold_db {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "hiss_threshold_db").default_f32());
            if val.is_finite() {
                self.hiss_threshold_db = val.clamp(
                    pk(DN, "hiss_threshold_db").min_f64() as f32,
                    pk(DN, "hiss_threshold_db").max_f64() as f32,
                );
                self.update_hiss_threshold_linear();
            }
        } else if id == self.param_hiss_frequency_hz {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "hiss_frequency_hz").default_f32());
            if val.is_finite() {
                self.hiss_frequency_hz = val.clamp(
                    pk(DN, "hiss_frequency_hz").min_f64() as f32,
                    pk(DN, "hiss_frequency_hz").max_f64() as f32,
                );
                self.update_hiss_cutoff_bin();
            }
        } else if id == self.param_hiss_strength {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "hiss_strength").default_f32());
            if val.is_finite() {
                self.hiss_strength = val.clamp(
                    pk(DN, "hiss_strength").min_f64() as f32,
                    pk(DN, "hiss_strength").max_f64() as f32,
                );
            }
        } else if id == self.param_spectral_sub_enabled {
            self.spectral_sub_enabled = value
                .as_bool()
                .unwrap_or(pk(DN, "spectral_sub_enabled").default_bool());
        } else if id == self.param_spectral_sub_alpha {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "spectral_sub_alpha").default_f32());
            if val.is_finite() {
                self.spectral_sub_alpha = val.clamp(
                    pk(DN, "spectral_sub_alpha").min_f64() as f32,
                    pk(DN, "spectral_sub_alpha").max_f64() as f32,
                );
            }
        } else if id == self.param_spectral_sub_beta {
            let val = value
                .as_float()
                .unwrap_or(pk(DN, "spectral_sub_beta").default_f32());
            if val.is_finite() {
                self.spectral_sub_beta = val.clamp(
                    pk(DN, "spectral_sub_beta").min_f64() as f32,
                    pk(DN, "spectral_sub_beta").max_f64() as f32,
                );
            }
        } else if id == self.param_learn_noise {
            let trigger = value.as_bool().unwrap_or(false);
            if trigger {
                self.start_learning();
            }
        } else if id == self.param_use_captured_profile {
            self.use_captured_profile = value
                .as_bool()
                .unwrap_or(pk(DN, "use_captured_profile").default_bool());
        } else if id == self.param_clear_profile {
            let trigger = value.as_bool().unwrap_or(false);
            if trigger {
                self.clear_noise_profile();
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_reduction_db {
            Some(ParameterValue::Float(self.reduction_db))
        } else if id == &self.param_floor_db {
            Some(ParameterValue::Float(self.floor_db))
        } else if id == &self.param_smoothing {
            Some(ParameterValue::Float(self.smoothing))
        } else if id == &self.param_attack_ms {
            Some(ParameterValue::Float(self.attack_ms))
        } else if id == &self.param_release_ms {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_low_latency {
            Some(ParameterValue::Bool(self.low_latency))
        } else if id == &self.param_polyphonic_detection {
            Some(ParameterValue::Bool(self.polyphonic_detection))
        } else if id == &self.param_crack_sensitivity {
            Some(ParameterValue::Float(self.crack_sensitivity))
        } else if id == &self.param_transparency {
            Some(ParameterValue::Float(self.transparency))
        } else if id == &self.param_psychoacoustic_masking {
            Some(ParameterValue::Bool(self.psychoacoustic_masking))
        } else if id == &self.param_dd_enabled {
            Some(ParameterValue::Bool(self.dd_enabled))
        } else if id == &self.param_dd_alpha {
            Some(ParameterValue::Float(self.dd_alpha))
        } else if id == &self.param_transient_enabled {
            Some(ParameterValue::Bool(self.transient_enabled))
        } else if id == &self.param_spectral_smoothing_enabled {
            Some(ParameterValue::Bool(self.spectral_smoothing_enabled))
        } else if id == &self.param_temporal_smoothing_enabled {
            Some(ParameterValue::Bool(self.temporal_smoothing_enabled))
        } else if id == &self.param_hiss_enabled {
            Some(ParameterValue::Bool(self.hiss_enabled))
        } else if id == &self.param_hiss_threshold_db {
            Some(ParameterValue::Float(self.hiss_threshold_db))
        } else if id == &self.param_hiss_frequency_hz {
            Some(ParameterValue::Float(self.hiss_frequency_hz))
        } else if id == &self.param_hiss_strength {
            Some(ParameterValue::Float(self.hiss_strength))
        } else if id == &self.param_spectral_sub_enabled {
            Some(ParameterValue::Bool(self.spectral_sub_enabled))
        } else if id == &self.param_spectral_sub_alpha {
            Some(ParameterValue::Float(self.spectral_sub_alpha))
        } else if id == &self.param_spectral_sub_beta {
            Some(ParameterValue::Float(self.spectral_sub_beta))
        } else if id == &self.param_learn_noise {
            Some(ParameterValue::Bool(self.is_learning))
        } else if id == &self.param_use_captured_profile {
            Some(ParameterValue::Bool(self.use_captured_profile))
        } else if id == &self.param_clear_profile {
            Some(ParameterValue::Bool(false)) // Trigger-only, always reads as false
        } else {
            None
        }
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
