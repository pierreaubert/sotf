// ============================================================================
// Upmixer Plugin - Stereo to Multi-Channel Surround
// ============================================================================
//
// This plugin converts stereo (2 channels) to multichannel surround sound
// using FFT-based Direct/Ambient decomposition and VBAP panning.
//
// Supports multiple configurations: 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6
//
// Algorithm:
// 1. FFT-based frequency-domain analysis
// 2. Separate direct sound (common to L/R) from ambient (difference)
// 3. Apply VBAP (Vector Base Amplitude Panning) to distribute sound to speakers
// 4. Height channels controlled by height_gain parameter
//
// Output channel mapping depends on selected configuration

use super::param_specs::upmixer::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::smoothing::Smoother;
use super::speaker_config::{SpeakerConfig, get_speaker_config};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

// Module declarations for refactored functionality
mod bass;
mod config;
mod decorrelation;
mod detection;
mod fft;
mod frequency_domain;
mod height;
mod hr_processing;
mod output;
mod panning;
mod process;
mod setup;

// Re-export configuration types from config module
pub use config::*;

/*
const PHASE_SHIFT_0   : Complex<f32> = Complex::new(1.0, 0.0); // +1
const PHASE_SHIFT_90: Complex<f32> = Complex::new(0.0, 1.0); // +i
const PHASE_SHIFT_180: Complex<f32> = Complex::new(-1.0, 0.0); // -1
const PHASE_SHIFT_270: Complex<f32> = Complex::new(0.0, -1.0); // -i
*/

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Stereo to multi-channel surround upmixer using FFT-based Direct/Ambient decomposition
pub struct UpmixerPlugin {
    /// FFT size (must be power of 2)
    fft_size: usize,
    /// Hop size for overlap-add (fft_size / 2 for 50% overlap)
    hop_size: usize,
    /// Sample rate
    sample_rate: u32,

    /// Speaker configuration
    speaker_config: &'static SpeakerConfig,
    /// Number of output channels (dynamic based on config)
    num_output_channels: usize,

    /// Forward FFT planner (low-resolution path)
    fft_forward: Arc<dyn RealToComplex<f32>>,
    /// Inverse FFT planner (low-resolution path)
    fft_inverse: Arc<dyn ComplexToReal<f32>>,

    /// High-resolution FFT size for direct-path enhancement
    hr_fft_size: usize,
    /// High-resolution hop size (hr_fft_size / 2)
    hr_fft_forward: Arc<dyn RealToComplex<f32>>,
    /// Inverse FFT planner (high-resolution path)
    hr_fft_inverse: Arc<dyn ComplexToReal<f32>>,

    // Parameters
    param_speaker_config: ParameterId,

    /// Front direct gain (gainFS)
    param_gain_front_direct: ParameterId,
    gain_front_direct: Smoother,

    /// Front ambient gain (gainFA)
    param_gain_front_ambient: ParameterId,
    gain_front_ambient: Smoother,

    /// Rear ambient gain (gainRA)
    param_gain_rear_ambient: ParameterId,
    gain_rear_ambient: Smoother,

    /// LFE cutoff frequency in Hz
    param_lfe_cutoff_hz: ParameterId,
    lfe_cutoff_hz: f32,

    /// Stereo width (0.0 = wide, 1.0 = narrow, 0.5 = balanced)
    param_stereo_width: ParameterId,
    stereo_width: f32,

    param_center_spread: ParameterId,
    center_spread: f32,

    /// Bandpass frequency in Hz (must be > lfe_cutoff_hz)
    param_bandpass_hz: ParameterId,
    bandpass_hz: f32,

    /// Height channel gain (0.0 to 2.0)
    param_height_gain: ParameterId,
    height_gain: Smoother,

    /// LFE gain (0.0 to 2.0)
    param_lfe_gain: ParameterId,
    lfe_gain: Smoother,

    /// Sub-Harmonic Synthesis
    param_enable_subharmonic_synth: ParameterId,
    enable_subharmonic_synth: bool,
    param_subharmonic_gain: ParameterId,
    subharmonic_gain: Smoother,

    /// High-resolution direct-path enhancement (multires)
    param_enable_hr_direct: ParameterId,
    enable_hr_direct: bool,
    param_hr_sharpen: ParameterId,
    hr_sharpen: f32,

    /// Safety cap on upmixer output peak (in dB)
    param_safety_cap_db: ParameterId,
    safety_cap_db: f32,
    /// Previous safety scale for smoothing between blocks
    prev_safety_scale: f32,

    // Sub-harmonic synth state
    subharmonic_phase: f32,
    /// Envelope for smoothing sub-harmonic synthesis on/off transitions
    /// Ranges from 0.0 (off) to 1.0 (on), with smooth attack/release
    subharmonic_envelope: f32,

    // ERB Banding
    erb_bands: Vec<usize>, // Start bin indices for each band

    // Logic Steering State
    steering_alphas: Vec<f32>, // Per-band alpha
    coherence_instant: Vec<f32>,
    smoothed_coherence: Vec<f32>,
    /// Ring buffer for median-filtered coherence (5-element per ERB band)
    coherence_history: Vec<[f32; 5]>,
    /// Current write index in coherence_history ring buffer
    coherence_history_idx: usize,

    // Decorrelation Mode
    param_decorrelation_mode: ParameterId,
    decorrelation_mode: usize, // 0=Velvet, 1=LFO

    // Sub-harmonic synthesis parameters
    param_subharmonic_freq_hz: ParameterId,
    subharmonic_freq_hz: f32,
    param_subharmonic_attack_ms: ParameterId,
    subharmonic_attack_ms: f32,
    param_subharmonic_release_ms: ParameterId,
    subharmonic_release_ms: f32,

    // Decorrelation parameters
    param_decorrelation_lfo_rate_hz: ParameterId,
    decorrelation_lfo_rate_hz: f32,
    param_velvet_noise_duration_ms: ParameterId,
    velvet_noise_duration_ms: f32,
    param_velvet_noise_density: ParameterId,
    velvet_noise_density: f32,

    // Height channel parameters
    param_height_hf_cap_hz: ParameterId,
    height_hf_cap_hz: f32,
    param_height_transient_reduction: ParameterId,
    height_transient_reduction: f32,
    param_height_direct_leak: ParameterId,
    height_direct_leak: f32,

    // Surround routing parameters
    param_surround_direct_bleed: ParameterId,
    surround_direct_bleed: f32,
    param_rear_ambient_boost: ParameterId,
    rear_ambient_boost: f32,
    param_rear_late_reflection: ParameterId,
    rear_late_reflection: f32,

    // Ambient/coherence parameters
    param_ambient_boost: ParameterId,
    ambient_boost: f32,

    // Dialogue detection parameters
    param_dialogue_weight: ParameterId,
    dialogue_weight: f32,
    param_voice_freq_min_hz: ParameterId,
    voice_freq_min_hz: f32,
    param_voice_freq_max_hz: ParameterId,
    voice_freq_max_hz: f32,

    // Diagnostic bypass parameters
    param_bypass_decorrelation: ParameterId,
    bypass_decorrelation: bool,
    param_bypass_transient_detection: ParameterId,
    bypass_transient_detection: bool,
    param_bypass_all_processing: ParameterId,
    bypass_all_processing: bool,

    // Decorrelation
    decorrelation_filter_left: Vec<Complex<f32>>,
    decorrelation_filter_right: Vec<Complex<f32>>,
    /// Per-output-channel decorrelation filters (one per surround/height channel)
    /// Front speakers and LFE get identity filters
    decorrelation_filters: Vec<Vec<Complex<f32>>>,

    // LFO Decorrelation State
    decor_base_phases_left: Vec<f32>,
    decor_base_phases_right: Vec<f32>,
    decor_lfo_phase: f32,

    // PCA State (per band)
    pca_cov_xx: Vec<f32>,
    pca_cov_yy: Vec<f32>,
    pca_cov_xy: Vec<Complex<f32>>,

    // Crossover magnitude tables (Linkwitz-Riley between mains and LFE)
    lfe_low_gains: Vec<f32>,
    mains_high_gains: Vec<f32>,

    // Height channel mask per positive-frequency bin (HF emphasis + coherence gating)
    height_band_gains: Vec<f32>,
    // Temporal smoothing buffer for height gains (previous frame)
    height_band_gains_prev: Vec<f32>,
    // Temporary buffer for height gain smoothing (avoid real-time allocation)
    height_band_gains_temp: Vec<f32>,

    // Precomputed per-bin frequency weights for height mask (hf_ratio^0.7)
    // Depends only on sample_rate, bandpass_hz, height_hf_cap_hz — recomputed in initialize()
    height_freq_weights: Vec<f32>,

    // Cached safety cap linear values (avoid per-block powf)
    // safety_cap_linear = 10^(safety_cap_db / 20)
    safety_cap_linear: f32,
    // safety_cap_min_scale = 10^(-safety_cap_db / 20)
    safety_cap_min_scale: f32,

    /// Panning gains for left source (pre-calculated for each speaker)
    panning_gains_left: Vec<f32>,
    /// Panning gains for right source (pre-calculated for each speaker)
    panning_gains_right: Vec<f32>,

    // Processing buffers (allocated once, reused)
    /// Time domain buffer for left channel
    time_domain_left: Vec<f32>,
    /// Time domain buffer for right channel
    time_domain_right: Vec<f32>,

    /// Frequency domain buffer for left channel
    freq_domain_left: Vec<Complex<f32>>,
    /// Frequency domain buffer for right channel
    freq_domain_right: Vec<Complex<f32>>,

    // Intermediate buffers for upmixing algorithm
    direct: Vec<Complex<f32>>,
    direct_left: Vec<Complex<f32>>,
    direct_right: Vec<Complex<f32>>,
    ambient_left: Vec<Complex<f32>>,
    ambient_right: Vec<Complex<f32>>,
    lfe: Vec<Complex<f32>>,

    // Smoothing buffers for ICC calculation

    // Output time-domain buffers (one per output channel, variable length)
    time_out_channels: Vec<Vec<f32>>,

    /// Input buffer accumulator for block-based processing
    input_buffer: Vec<f32>,
    /// Number of samples currently in input buffer
    input_buffer_fill: usize,

    /// Temporary input block for FFT processing (pre-allocated)
    temp_input_block: Vec<f32>,

    /// Temporary frequency buffer for IFFT mixing (reused per channel)
    temp_freq_out: Vec<Complex<f32>>,

    /// Hann window for FFT (pre-computed)
    window: Vec<f32>,
    /// Hann window for high-resolution FFT path
    hr_window: Vec<f32>,
    /// Output accumulator for overlap-add (holds fft_size samples per channel)
    /// This allows us to accumulate processed blocks and drain them gradually
    output_accumulator: Vec<Vec<f32>>,
    /// Number of valid samples in output accumulator
    output_accumulator_fill: usize,
    /// Next position to add a block (tracks overlap-add offset)
    next_add_position: usize,
    /// Current read position in the output accumulator ring buffer
    output_read_position: usize,
    /// Pre-allocated output block buffer (reused to avoid allocations)
    output_block: Vec<f32>,

    // High-resolution direct-path buffers (allocated once, reused)
    /// Input buffer accumulator for HR path (stereo interleaved)
    hr_input_buffer: Vec<f32>,
    /// Number of samples currently in HR input buffer
    hr_input_buffer_fill: usize,
    /// Temporary HR input block for FFT processing
    hr_temp_input_block: Vec<f32>,
    /// Temporary HR frequency buffer for IFFT mixing
    hr_temp_freq_out: Vec<Complex<f32>>,
    /// Time-domain buffer for left channel (HR path)
    hr_time_domain_left: Vec<f32>,
    /// Time-domain buffer for right channel (HR path)
    hr_time_domain_right: Vec<f32>,
    /// Frequency-domain buffer for left channel (HR path)
    hr_freq_domain_left: Vec<Complex<f32>>,
    /// Frequency-domain buffer for right channel (HR path)
    hr_freq_domain_right: Vec<Complex<f32>>,
    /// Per-channel HR output buffers in frequency/time domain
    hr_time_out_channels: Vec<Vec<f32>>,
    /// Temporary block buffer for HR output mixing
    hr_output_block: Vec<f32>,
    /// Next position to add a HR block in the shared accumulator (reserved)
    hr_next_add_position: usize,

    hr_transient_env: f32,
    hr_energy_smooth: f32,
    /// Previous frame magnitude spectrum for spectral flux calculation
    prev_magnitude_spectrum: Vec<f32>,
    /// Smoothed spectral flux for transient normalization
    spectral_flux_smooth: f32,

    // Dialogue Detection State
    /// Smoothed spectral centroid (Hz) for dialogue detection
    dialogue_spectral_centroid: f32,
    /// Smoothed temporal envelope variance for dialogue detection
    dialogue_envelope_variance: f32,
    /// Previous frame RMS energy for envelope variance calculation
    dialogue_prev_rms: f32,
    /// Dialogue probability (0.0 = no dialogue, 1.0 = strong dialogue)
    dialogue_probability: f32,
    /// Current adaptive decorrelation strength (0.0 to 1.0)
    pub(super) decorrelation_strength: f32,
    /// Pre-calculated blended decorrelation filters (one per channel)
    pub(super) blended_decorrelation_filters: Vec<Vec<Complex<f32>>>,
}

impl UpmixerPlugin {
    /// Create a new upmixer plugin with speaker configuration
    ///
    /// # Arguments
    /// * `fft_size` - FFT size (must be power of 2, recommended: 2048)
    /// * `speaker_config_id` - Speaker configuration ("5.1", "7.1", "5.1.4", etc.)
    /// * `gain_front_direct` - Gain for direct sound in front channels (default: 1.0)
    /// * `gain_front_ambient` - Gain for ambient sound in front channels (default: 0.5)
    /// * `gain_rear_ambient` - Gain for ambient sound in rear channels (default: 1.0)
    /// * `height_gain` - Gain for height channels (default: 1.0)
    /// * `lfe_gain` - Gain for LFE/subwoofer channel (default: 1.0)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fft_size: usize,
        speaker_config_id: &str,
        gain_front_direct: f32,
        gain_front_ambient: f32,
        gain_rear_ambient: f32,
        lfe_cutoff_hz: f32,
        stereo_width: f32,
        bandpass_hz: f32,
        height_gain: f32,
        lfe_gain: f32,
        enable_subharmonic_synth: bool,
        subharmonic_gain: f32,
    ) -> Self {
        assert!(fft_size.is_power_of_two(), "FFT size must be power of 2");
        assert!(
            (20.0..=180.0).contains(&lfe_cutoff_hz),
            "LFE cutoff must be between 20-180 Hz"
        );
        assert!(
            (0.0..=1.0).contains(&stereo_width),
            "Stereo width must be between 0.0-1.0"
        );
        assert!(
            bandpass_hz > lfe_cutoff_hz,
            "Bandpass frequency must be greater than LFE cutoff"
        );

        let sample_rate = 44100; // Will be updated in initialize()
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        let zero_complex = Complex::new(0.0, 0.0);
        let spectrum_size = fft_size / 2 + 1;

        // Generate Hann window: w[n] = 0.5 * (1 - cos(2*pi*n/N))
        // Using N (not N-1) for perfect COLA with 50% overlap
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / fft_size as f32).cos())
            })
            .collect();

        // 50% overlap requires fft_size/2 hop size
        let hop_size = fft_size / 2;

        // High-resolution path uses a shorter FFT for improved direct-path
        // time resolution. For now this is internal and disabled by default.
        let hr_fft_size = 512;
        let hr_spectrum_size = hr_fft_size / 2 + 1;
        let hr_fft_forward = planner.plan_fft_forward(hr_fft_size);
        let hr_fft_inverse = planner.plan_fft_inverse(hr_fft_size);

        let hr_window: Vec<f32> = (0..hr_fft_size)
            .map(|i| {
                0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / hr_fft_size as f32).cos())
            })
            .collect();

        // Get speaker configuration
        let speaker_config = get_speaker_config(speaker_config_id).unwrap_or_else(|| {
            log::info!(
                "Invalid speaker config '{}', falling back to 5.1",
                speaker_config_id
            );
            get_speaker_config("5.1").unwrap()
        });

        let num_output_channels = speaker_config.total_channels;

        // Panning gains will be calculated by recalculate_panning_gains() below
        // TODO: need to change if input is not stereo
        let panning_gains_left = Vec::with_capacity(num_output_channels);
        let panning_gains_right = Vec::with_capacity(num_output_channels);

        // Output accumulator holds up to 4*fft_size samples per channel
        let output_accumulator = vec![vec![0.0; fft_size * 4]; num_output_channels];

        // Allocate output buffers for each channel
        let time_out_channels = vec![vec![0.0; fft_size]; num_output_channels];

        let mut plugin = Self {
            fft_size,
            hop_size,
            sample_rate,
            speaker_config,
            num_output_channels,

            fft_forward,
            fft_inverse,

            hr_fft_size,
            hr_fft_forward,
            hr_fft_inverse,

            param_speaker_config: ParameterId::from("speaker_config"),
            param_gain_front_direct: ParameterId::from("gain_front_direct"),
            gain_front_direct: Smoother::new(gain_front_direct, 5.0, sample_rate),

            param_gain_front_ambient: ParameterId::from("gain_front_ambient"),
            gain_front_ambient: Smoother::new(gain_front_ambient, 5.0, sample_rate),

            param_gain_rear_ambient: ParameterId::from("gain_rear_ambient"),
            gain_rear_ambient: Smoother::new(gain_rear_ambient, 5.0, sample_rate),

            param_lfe_cutoff_hz: ParameterId::from("lfe_cutoff_hz"),
            lfe_cutoff_hz,

            param_stereo_width: ParameterId::from("stereo_width"),
            stereo_width,

            param_center_spread: ParameterId::from("center_spread"),
            center_spread: default_center_spread(),

            param_bandpass_hz: ParameterId::from("bandpass_hz"),
            bandpass_hz,

            param_height_gain: ParameterId::from("height_gain"),
            height_gain: Smoother::new(height_gain, 5.0, sample_rate),

            param_lfe_gain: ParameterId::from("lfe_gain"),
            lfe_gain: Smoother::new(lfe_gain, 5.0, sample_rate),

            param_enable_subharmonic_synth: ParameterId::from("enable_subharmonic_synth"),
            enable_subharmonic_synth,
            param_subharmonic_gain: ParameterId::from("subharmonic_gain"),
            subharmonic_gain: Smoother::new(subharmonic_gain, 5.0, sample_rate),

            param_enable_hr_direct: ParameterId::from("enable_hr_direct"),
            enable_hr_direct: true, // Enable by default for multi-resolution analysis
            param_hr_sharpen: ParameterId::from("hr_sharpen"),
            hr_sharpen: 1.0,
            param_safety_cap_db: ParameterId::from("safety_cap_db"),
            safety_cap_db: default_safety_cap_db(),
            prev_safety_scale: 1.0, // Start with no gain reduction
            param_decorrelation_mode: ParameterId::from("decorrelation_mode"),
            decorrelation_mode: 0, // Default to Velvet Noise

            // Sub-harmonic synthesis parameters
            param_subharmonic_freq_hz: ParameterId::from("subharmonic_freq_hz"),
            subharmonic_freq_hz: default_subharmonic_freq_hz(),
            param_subharmonic_attack_ms: ParameterId::from("subharmonic_attack_ms"),
            subharmonic_attack_ms: default_subharmonic_attack_ms(),
            param_subharmonic_release_ms: ParameterId::from("subharmonic_release_ms"),
            subharmonic_release_ms: default_subharmonic_release_ms(),

            // Decorrelation parameters
            param_decorrelation_lfo_rate_hz: ParameterId::from("decorrelation_lfo_rate_hz"),
            decorrelation_lfo_rate_hz: default_decorrelation_lfo_rate_hz(),
            param_velvet_noise_duration_ms: ParameterId::from("velvet_noise_duration_ms"),
            velvet_noise_duration_ms: default_velvet_noise_duration_ms(),
            param_velvet_noise_density: ParameterId::from("velvet_noise_density"),
            velvet_noise_density: default_velvet_noise_density(),

            // Height channel parameters
            param_height_hf_cap_hz: ParameterId::from("height_hf_cap_hz"),
            height_hf_cap_hz: default_height_hf_cap_hz(),
            param_height_transient_reduction: ParameterId::from("height_transient_reduction"),
            height_transient_reduction: default_height_transient_reduction(),
            param_height_direct_leak: ParameterId::from("height_direct_leak"),
            height_direct_leak: default_height_direct_leak(),

            // Surround routing parameters
            param_surround_direct_bleed: ParameterId::from("surround_direct_bleed"),
            surround_direct_bleed: default_surround_direct_bleed(),
            param_rear_ambient_boost: ParameterId::from("rear_ambient_boost"),
            rear_ambient_boost: default_rear_ambient_boost(),
            param_rear_late_reflection: ParameterId::from("rear_late_reflection"),
            rear_late_reflection: default_rear_late_reflection(),

            // Ambient/coherence parameters
            param_ambient_boost: ParameterId::from("ambient_boost"),
            ambient_boost: default_ambient_boost(),

            // Dialogue detection parameters
            param_dialogue_weight: ParameterId::from("dialogue_weight"),
            dialogue_weight: default_dialogue_weight(),
            param_voice_freq_min_hz: ParameterId::from("voice_freq_min_hz"),
            voice_freq_min_hz: default_voice_freq_min_hz(),
            param_voice_freq_max_hz: ParameterId::from("voice_freq_max_hz"),
            voice_freq_max_hz: default_voice_freq_max_hz(),

            // Diagnostic bypass parameters
            param_bypass_decorrelation: ParameterId::from("bypass_decorrelation"),
            bypass_decorrelation: default_bypass_decorrelation(),
            param_bypass_transient_detection: ParameterId::from("bypass_transient_detection"),
            bypass_transient_detection: default_bypass_transient_detection(),
            param_bypass_all_processing: ParameterId::from("bypass_all_processing"),
            bypass_all_processing: default_bypass_all_processing(),

            subharmonic_phase: 0.0,
            subharmonic_envelope: 0.0,

            erb_bands: Vec::new(), // Will be calculated in initialize()
            steering_alphas: Vec::new(),
            coherence_instant: Vec::new(),
            smoothed_coherence: Vec::new(),
            coherence_history: Vec::new(),
            coherence_history_idx: 0,
            decorrelation_filter_left: vec![zero_complex; spectrum_size],
            decorrelation_filter_right: vec![zero_complex; spectrum_size],
            decorrelation_filters: Vec::new(),
            decor_base_phases_left: Vec::new(),
            decor_base_phases_right: Vec::new(),
            decor_lfo_phase: 0.0,

            pca_cov_xx: Vec::new(),
            pca_cov_yy: Vec::new(),
            pca_cov_xy: Vec::new(),

            lfe_low_gains: vec![1.0; spectrum_size],
            mains_high_gains: vec![1.0; spectrum_size],

            height_band_gains: vec![0.0; spectrum_size],
            height_band_gains_prev: vec![0.0; spectrum_size],
            height_band_gains_temp: vec![0.0; spectrum_size],

            height_freq_weights: vec![0.0; spectrum_size],

            safety_cap_linear: if default_safety_cap_db() > 0.0 {
                10.0_f32.powf(default_safety_cap_db() / 20.0)
            } else {
                1.0
            },
            safety_cap_min_scale: if default_safety_cap_db() > 0.0 {
                10.0_f32.powf(-default_safety_cap_db() / 20.0)
            } else {
                0.0
            },

            panning_gains_left,
            panning_gains_right,

            // Allocate all buffers
            time_domain_left: vec![0.0; fft_size],
            time_domain_right: vec![0.0; fft_size],
            freq_domain_left: vec![zero_complex; spectrum_size],
            freq_domain_right: vec![zero_complex; spectrum_size],
            direct: vec![zero_complex; spectrum_size],
            direct_left: vec![zero_complex; spectrum_size],
            direct_right: vec![zero_complex; spectrum_size],
            ambient_left: vec![zero_complex; spectrum_size],
            ambient_right: vec![zero_complex; spectrum_size],
            lfe: vec![zero_complex; spectrum_size],

            time_out_channels,

            input_buffer: vec![0.0; fft_size * 2], // stereo
            input_buffer_fill: 0,

            temp_input_block: vec![0.0; fft_size * 2], // Pre-allocated temp buffer
            temp_freq_out: vec![zero_complex; spectrum_size],

            window,
            hr_window,
            output_accumulator,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            output_block: vec![0.0; fft_size * num_output_channels],

            hr_input_buffer: vec![0.0; hr_fft_size * 2],
            hr_input_buffer_fill: 0,
            hr_temp_input_block: vec![0.0; hr_fft_size * 2],
            hr_temp_freq_out: vec![zero_complex; hr_spectrum_size],
            hr_time_domain_left: vec![0.0; hr_fft_size],
            hr_time_domain_right: vec![0.0; hr_fft_size],
            hr_freq_domain_left: vec![zero_complex; hr_spectrum_size],
            hr_freq_domain_right: vec![zero_complex; hr_spectrum_size],
            hr_time_out_channels: vec![vec![0.0; hr_fft_size]; num_output_channels],
            hr_output_block: vec![0.0; hr_fft_size * num_output_channels],
            hr_next_add_position: 0,

            hr_transient_env: 0.0,
            hr_energy_smooth: 0.0,
            prev_magnitude_spectrum: vec![0.0; spectrum_size],
            spectral_flux_smooth: 0.0,

            dialogue_spectral_centroid: 0.0,
            dialogue_envelope_variance: 0.0,
            dialogue_prev_rms: 0.0,
            dialogue_probability: 0.0,
            decorrelation_strength: 1.0,
            blended_decorrelation_filters: Vec::new(),
        };

        // Calculate panning gains for stereo sources (left at +30°, right at -30°)
        plugin.recalculate_panning_gains();

        plugin
    }

    /// Create a new upmixer plugin from configuration parameters
    pub fn from_params(params: UpmixerPluginParams) -> Self {
        let mut plugin = Self::new(
            params.fft_size,
            &params.speaker_config,
            params.gain_front_direct,
            params.gain_front_ambient,
            params.gain_rear_ambient,
            params.lfe_cutoff_hz,
            params.stereo_width,
            params.bandpass_hz,
            params.height_gain,
            params.lfe_gain,
            params.enable_subharmonic_synth,
            params.subharmonic_gain,
        );
        plugin.center_spread = params.center_spread.clamp(0.0, 1.0);
        plugin.enable_hr_direct = params.enable_hr_direct;
        plugin.hr_sharpen = params.hr_sharpen.clamp(0.0, 1.0);
        plugin.safety_cap_db = params.safety_cap_db.max(0.0);
        plugin.update_safety_cap_cache();
        plugin.decorrelation_mode = params.decorrelation_mode;

        // Sub-harmonic synthesis parameters
        plugin.subharmonic_freq_hz = params.subharmonic_freq_hz.clamp(20.0, 80.0);
        plugin.subharmonic_attack_ms = params.subharmonic_attack_ms.clamp(1.0, 100.0);
        plugin.subharmonic_release_ms = params.subharmonic_release_ms.clamp(10.0, 500.0);

        // Decorrelation parameters
        plugin.decorrelation_lfo_rate_hz = params.decorrelation_lfo_rate_hz.clamp(0.01, 1.0);
        plugin.velvet_noise_duration_ms = params.velvet_noise_duration_ms.clamp(10.0, 100.0);
        plugin.velvet_noise_density = params.velvet_noise_density.clamp(500.0, 5000.0);

        // Height channel parameters
        plugin.height_hf_cap_hz = params.height_hf_cap_hz.clamp(8000.0, 20000.0);
        plugin.height_transient_reduction = params.height_transient_reduction.clamp(0.0, 1.0);
        plugin.height_direct_leak = params.height_direct_leak.clamp(0.0, 0.5);

        // Surround routing parameters
        plugin.surround_direct_bleed = params.surround_direct_bleed.clamp(0.0, 1.0);
        plugin.rear_ambient_boost = params.rear_ambient_boost.clamp(1.0, 3.0);
        plugin.rear_late_reflection = params.rear_late_reflection.clamp(0.0, 0.5);

        // Ambient/coherence parameters
        plugin.ambient_boost = params.ambient_boost.clamp(0.5, 2.0);

        // Dialogue detection parameters
        plugin.dialogue_weight = params.dialogue_weight.clamp(0.0, 1.0);
        plugin.voice_freq_min_hz = params.voice_freq_min_hz.clamp(200.0, 800.0);
        plugin.voice_freq_max_hz = params.voice_freq_max_hz.clamp(2000.0, 5000.0);

        // Diagnostic bypass parameters
        plugin.bypass_decorrelation = params.bypass_decorrelation;
        plugin.bypass_transient_detection = params.bypass_transient_detection;
        plugin.bypass_all_processing = params.bypass_all_processing;

        plugin
    }
}

impl Plugin for UpmixerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(
            format!("Stereo to {} Upmixer", self.speaker_config.name),
            "2.1.0",
            "SotF",
        )
        .with_description(format!(
            "Converts stereo to {} using FFT-based Direct/Ambient decomposition and VBAP panning (Smoothed)",
            self.speaker_config.name
        ))
    }

    fn input_channels(&self) -> usize {
        2 // Stereo
    }

    fn output_channels(&self) -> usize {
        self.num_output_channels // Variable based on configuration
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_int(
                "speaker_config",
                "Configuration",
                SPEAKER_CONFIG_DEFAULT,
                SPEAKER_CONFIG_MIN,
                SPEAKER_CONFIG_MAX,
            )
            .with_description(
                "Speaker configuration index.
0=5.1 (default), 1=7.1, 2=5.1.2, 3=5.1.4,
4=7.1.2, 5=7.1.4, 6=9.1.4, 7=9.1.6,
8=2.0, 9=5.0.
Controls output layout and number of channels.",
            )
            .with_group("Output")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "gain_front_direct",
                "Front Direct Gain",
                GAIN_FRONT_DIRECT_DEFAULT,
                GAIN_FRONT_DIRECT_MIN,
                GAIN_FRONT_DIRECT_MAX,
            )
            .with_description(
                "Front direct gain for non-height front speakers.
Range: 0.0-2.0, default 1.0.
Higher values make the front image more focused and dry;
lower values rely more on ambient and surround energy.",
            )
            .with_group("Front")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "gain_front_ambient",
                "Front Ambient Gain",
                GAIN_FRONT_AMBIENT_DEFAULT,
                GAIN_FRONT_AMBIENT_MIN,
                GAIN_FRONT_AMBIENT_MAX,
            )
            .with_description(
                "Decorrelated ambient gain routed to front speakers.
Range: 0.0-2.0, default 0.5.
Increase to widen and enliven the front stage;
decrease for a more center-focused, direct front.",
            )
            .with_group("Front")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "gain_rear_ambient",
                "Rear Ambient Gain",
                GAIN_REAR_AMBIENT_DEFAULT,
                GAIN_REAR_AMBIENT_MIN,
                GAIN_REAR_AMBIENT_MAX,
            )
            .with_description(
                "Ambient gain for surround and rear channels.
Range: 0.0-2.0, default 1.0.
Use <1.0 for subtle ambience, >1.0 for a more enveloping surround field.",
            )
            .with_group("Surround")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "height_gain",
                "Height Gain",
                GAIN_HEIGHT_DEFAULT,
                GAIN_HEIGHT_MIN,
                GAIN_HEIGHT_MAX,
            )
            .with_description(
                "Gain for height/overhead channels (elevation > 0).
Range: 0.0-2.0, default 1.0.
0.0 disables height channels; higher values raise the contribution
of height speakers relative to the bed layer.",
            )
            .with_group("Height")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "lfe_gain",
                "LFE Gain",
                LFE_GAIN_DEFAULT,
                LFE_GAIN_MIN,
                LFE_GAIN_MAX,
            )
            .with_description(
                "Gain for LFE/subwoofer channel.
Range: 0.0-2.0, default 1.0.
Controls overall subwoofer level after the mains/LFE crossover.",
            )
            .with_group("LFE")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "lfe_cutoff_hz",
                "LFE Cutoff (Hz)",
                LFE_CUTOFF_HZ_DEFAULT,
                LFE_CUTOFF_HZ_MIN,
                LFE_CUTOFF_HZ_MAX,
            )
            .with_description(
                "Linkwitz-Riley crossover frequency between mains and LFE.
Range: 20-180 Hz, default 120 Hz.
Lower values keep more bass in mains; higher values route
more low-frequency energy into the subwoofer.",
            )
            .with_group("LFE")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "stereo_width",
                "Stereo Width",
                STEREO_WIDTH_DEFAULT,
                STEREO_WIDTH_MIN,
                STEREO_WIDTH_MAX,
            )
            .with_description(
                "Controls front stereo width for the direct component.
Range: 0.0-1.0, default 0.5.
0.0 keeps L/R wide; 1.0 collapses toward mono/center;
intermediate values balance width and center focus.",
            )
            .with_group("Front")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "center_spread",
                "Center Spread",
                CENTER_SPREAD_DEFAULT,
                CENTER_SPREAD_MIN,
                CENTER_SPREAD_MAX,
            )
            .with_description(
                "Controls how much direct energy is focused in the physical center vs L/R.
Range: 0.0-1.0, default 0.0.
0.0 sends coherent center energy to the C speaker;
1.0 moves it into a phantom center across L/R.",
            )
            .with_group("Front")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "bandpass_hz",
                "Upmix Crossover (Hz)",
                BANDPASS_HZ_DEFAULT,
                BANDPASS_HZ_MIN,
                BANDPASS_HZ_MAX,
            )
            .with_description(
                "Frequency above which upmixing to surrounds/height is applied.
Range: 150-350 Hz, default 250 Hz.
Below this frequency content stays mainly in fronts + LFE;
above it participates in the direct/ambient upmix.",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool(
                "enable_subharmonic_synth",
                "Sub-Harmonic Synth",
                ENABLE_SUBHARMONIC_SYNTH_DEFAULT,
            )
            .with_description(
                "Enables optional sub-harmonic synthesis on the LFE.
Default: off. When enabled, a low-frequency tone is added to the
subwoofer, driven by the LFE envelope for extra rumble.",
            )
            .with_group("LFE")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "subharmonic_gain",
                "Sub-Harmonic Gain",
                SUBHARMONIC_GAIN_DEFAULT,
                SUBHARMONIC_GAIN_MIN,
                SUBHARMONIC_GAIN_MAX,
            )
            .with_description(
                "Gain for synthesized sub-harmonics when enabled.
Range: 0.0-1.0, default 0.5.
Controls how loud the synthesized low-frequency component is
relative to the original LFE signal.",
            )
            .with_group("LFE")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "enable_hr_direct",
                "Multi-Resolution Analysis",
                ENABLE_HR_DIRECT_DEFAULT,
            )
            .with_description(
                "Enables multi-resolution analysis for optimal time/frequency resolution.
Default: ON. Uses short FFT (512 samples) for transients and long FFT (2048) for ambient.
Adaptively blends based on transient detection for sharper attacks and smooth ambience.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "hr_sharpen",
                "HR Sharpen",
                HR_SHARPEN_DEFAULT,
                HR_SHARPEN_MIN,
                HR_SHARPEN_MAX,
            )
            .with_description(
                "Depth control for the high-resolution direct path.
Range: 0.0-1.0, default 1.0.
0.0 effectively disables the HR contribution even if enabled;
1.0 applies the full transient-driven HR emphasis and ducking
of the main front field.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "safety_cap_db",
                "Safety Cap (dB)",
                SAFETY_CAP_DB_DEFAULT,
                SAFETY_CAP_DB_MIN,
                SAFETY_CAP_DB_MAX,
            )
            .with_description(
                "Peak safety cap for the upmixer output.
Range: 0.0-3.0 dB, default 3.0 dB.
If a block's peak level after upmixing would exceed this value
above unity, the block is scaled down to stay within the cap.",
            )
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_int(
                "decorrelation_mode",
                "Decorrelation Mode",
                DECORRELATION_MODE_DEFAULT,
                DECORRELATION_MODE_MIN,
                DECORRELATION_MODE_MAX,
            )
            .with_description(
                "Mode for ambient decorrelation.
0 = Velvet Noise (Static, smooth, no artifacts) - Default
1 = LFO Phase (Dynamic, subtle motion, may have metallic artifacts)",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            // Sub-harmonic synthesis parameters
            Parameter::new_float(
                "subharmonic_freq_hz",
                "Sub-Harmonic Frequency",
                SUBHARMONIC_FREQ_HZ_DEFAULT,
                SUBHARMONIC_FREQ_HZ_MIN,
                SUBHARMONIC_FREQ_HZ_MAX,
            )
            .with_description(
                "Sub-harmonic synthesis frequency in Hz.
Range: 20-80 Hz, default 40 Hz.
Lower values produce deeper rumble, higher values are more audible.",
            )
            .with_group("LFE")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "subharmonic_attack_ms",
                "Sub-Harmonic Attack",
                SUBHARMONIC_ATTACK_MS_DEFAULT,
                SUBHARMONIC_ATTACK_MS_MIN,
                SUBHARMONIC_ATTACK_MS_MAX,
            )
            .with_description(
                "Sub-harmonic envelope attack time in milliseconds.
Range: 1-100 ms, default 10 ms.
Faster attack follows LFE transients more closely.",
            )
            .with_group("LFE")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "subharmonic_release_ms",
                "Sub-Harmonic Release",
                SUBHARMONIC_RELEASE_MS_DEFAULT,
                SUBHARMONIC_RELEASE_MS_MIN,
                SUBHARMONIC_RELEASE_MS_MAX,
            )
            .with_description(
                "Sub-harmonic envelope release time in milliseconds.
Range: 10-500 ms, default 50 ms.
Longer release creates smoother decay.",
            )
            .with_group("LFE")
            .with_importance(ParameterImportance::FineTuning),
            // Decorrelation parameters
            Parameter::new_float(
                "decorrelation_lfo_rate_hz",
                "Decorrelation LFO Rate",
                DECORRELATION_LFO_RATE_HZ_DEFAULT,
                DECORRELATION_LFO_RATE_HZ_MIN,
                DECORRELATION_LFO_RATE_HZ_MAX,
            )
            .with_description(
                "LFO rate for decorrelation phase modulation.
Range: 0.01-1.0 Hz, default 0.15 Hz.
Higher values add more motion but may cause artifacts.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "velvet_noise_duration_ms",
                "Velvet Noise Duration",
                VELVET_NOISE_DURATION_MS_DEFAULT,
                VELVET_NOISE_DURATION_MS_MIN,
                VELVET_NOISE_DURATION_MS_MAX,
            )
            .with_description(
                "Velvet noise decorrelator duration in milliseconds.
Range: 10-100 ms, default 30 ms.
Longer duration creates smoother diffusion.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "velvet_noise_density",
                "Velvet Noise Density",
                VELVET_NOISE_DENSITY_DEFAULT,
                VELVET_NOISE_DENSITY_MIN,
                VELVET_NOISE_DENSITY_MAX,
            )
            .with_description(
                "Velvet noise pulse density (pulses per second).
Range: 500-5000, default 2000.
Higher density creates denser, smoother decorrelation.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::FineTuning),
            // Height channel parameters
            Parameter::new_float(
                "height_hf_cap_hz",
                "Height HF Cap",
                HEIGHT_HF_CAP_HZ_DEFAULT,
                HEIGHT_HF_CAP_HZ_MIN,
                HEIGHT_HF_CAP_HZ_MAX,
            )
            .with_description(
                "High-frequency cap for height channels in Hz.
Range: 8000-20000 Hz, default 16000 Hz.
Limits extreme highs in overhead speakers.",
            )
            .with_group("Height")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "height_transient_reduction",
                "Height Transient Reduction",
                HEIGHT_TRANSIENT_REDUCTION_DEFAULT,
                HEIGHT_TRANSIENT_REDUCTION_MIN,
                HEIGHT_TRANSIENT_REDUCTION_MAX,
            )
            .with_description(
                "Transient reduction for height channels.
Range: 0.0-1.0, default 0.6.
Reduces height channel level during transients for coherence.",
            )
            .with_group("Height")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "height_direct_leak",
                "Height Direct Leak",
                HEIGHT_DIRECT_LEAK_DEFAULT,
                HEIGHT_DIRECT_LEAK_MIN,
                HEIGHT_DIRECT_LEAK_MAX,
            )
            .with_description(
                "Direct signal leak into height channels.
Range: 0.0-0.5, default 0.15.
Allows some direct sound into overheads for air and presence.",
            )
            .with_group("Height")
            .with_importance(ParameterImportance::FineTuning),
            // Surround routing parameters
            Parameter::new_float(
                "surround_direct_bleed",
                "Surround Direct Bleed",
                SURROUND_DIRECT_BLEED_DEFAULT,
                SURROUND_DIRECT_BLEED_MIN,
                SURROUND_DIRECT_BLEED_MAX,
            )
            .with_description(
                "Direct signal bleed into surround channels.
Range: 0.0-1.0, default 0.50.
Higher values create more cohesive surround image.",
            )
            .with_group("Surround")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "rear_ambient_boost",
                "Rear Ambient Boost",
                REAR_AMBIENT_BOOST_DEFAULT,
                REAR_AMBIENT_BOOST_MIN,
                REAR_AMBIENT_BOOST_MAX,
            )
            .with_description(
                "Ambient gain boost for rear channels.
Range: 1.0-3.0x, default 1.5x.
Increases envelopment from rear speakers.",
            )
            .with_group("Surround")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "rear_late_reflection",
                "Rear Late Reflection",
                REAR_LATE_REFLECTION_DEFAULT,
                REAR_LATE_REFLECTION_MIN,
                REAR_LATE_REFLECTION_MAX,
            )
            .with_description(
                "Late reflection level for rear height channels.
Range: 0.0-0.5, default 0.10.
Adds late reflections to rear heights for depth.",
            )
            .with_group("Surround")
            .with_importance(ParameterImportance::FineTuning),
            // Ambient parameters
            Parameter::new_float(
                "ambient_boost",
                "Ambient Boost",
                AMBIENT_BOOST_DEFAULT,
                AMBIENT_BOOST_MIN,
                AMBIENT_BOOST_MAX,
            )
            .with_description(
                "Ambient gain boost factor.
Range: 0.5-2.0x, default 1.2x.
Multiplier applied to coherence-derived ambient gain.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::Useful),
            // Dialogue detection parameters
            Parameter::new_float(
                "dialogue_weight",
                "Dialogue Weight",
                DIALOGUE_WEIGHT_DEFAULT,
                DIALOGUE_WEIGHT_MIN,
                DIALOGUE_WEIGHT_MAX,
            )
            .with_description(
                "Maximum dialogue routing weight.
Range: 0.0-1.0, default 0.4.
Higher values route more detected dialogue to center.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "voice_freq_min_hz",
                "Voice Freq Min",
                VOICE_FREQ_MIN_HZ_DEFAULT,
                VOICE_FREQ_MIN_HZ_MIN,
                VOICE_FREQ_MIN_HZ_MAX,
            )
            .with_description(
                "Voice detection frequency range minimum.
Range: 200-800 Hz, default 500 Hz.
Lower bound for dialogue detection analysis.",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "voice_freq_max_hz",
                "Voice Freq Max",
                VOICE_FREQ_MAX_HZ_DEFAULT,
                VOICE_FREQ_MAX_HZ_MIN,
                VOICE_FREQ_MAX_HZ_MAX,
            )
            .with_description(
                "Voice detection frequency range maximum.
Range: 2000-5000 Hz, default 3000 Hz.
Upper bound for dialogue detection analysis.",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_speaker_config {
            if let Some(config_idx) = value.as_int() {
                let config_id = match config_idx {
                    0 => "5.1",
                    1 => "7.1",
                    2 => "5.1.2",
                    3 => "5.1.4",
                    4 => "7.1.2",
                    5 => "7.1.4",
                    6 => "9.1.4",
                    7 => "9.1.6",
                    8 => "2.0",
                    9 => "5.0",
                    _ => return Err("Invalid configuration index".to_string()),
                };
                return self.change_speaker_config(config_id);
            }
        } else if id == self.param_gain_front_direct {
            if let Some(gain) = value.as_float() {
                self.gain_front_direct.set_target(gain);
                return Ok(());
            }
        } else if id == self.param_gain_front_ambient {
            if let Some(gain) = value.as_float() {
                self.gain_front_ambient.set_target(gain);
                return Ok(());
            }
        } else if id == self.param_gain_rear_ambient
            && let Some(gain) = value.as_float()
        {
            self.gain_rear_ambient.set_target(gain);
            return Ok(());
        } else if id == self.param_height_gain
            && let Some(gain) = value.as_float()
        {
            if (0.0..=2.0).contains(&gain) {
                self.height_gain.set_target(gain);
                return Ok(());
            }
            return Err("Height gain must be between 0.0 and 2.0".to_string());
        } else if id == self.param_lfe_gain
            && let Some(gain) = value.as_float()
        {
            if (0.0..=2.0).contains(&gain) {
                self.lfe_gain.set_target(gain);
                return Ok(());
            }
            return Err("LFE gain must be between 0.0 and 2.0".to_string());
        } else if id == self.param_lfe_cutoff_hz
            && let Some(cutoff) = value.as_float()
        {
            if (20.0..=180.0).contains(&cutoff) && cutoff < self.bandpass_hz {
                self.lfe_cutoff_hz = cutoff;
                self.update_crossover_gains();
                return Ok(());
            }
            return Err(
                "LFE cutoff must be 20-180 Hz and less than bandpass frequency".to_string(),
            );
        } else if id == self.param_stereo_width
            && let Some(width) = value.as_float()
        {
            self.stereo_width = width.clamp(0.0, 1.0);
            return Ok(());
        } else if id == self.param_center_spread
            && let Some(spread) = value.as_float()
        {
            self.center_spread = spread.clamp(0.0, 1.0);
            return Ok(());
        } else if id == self.param_bandpass_hz
            && let Some(freq) = value.as_float()
        {
            if freq > self.lfe_cutoff_hz {
                self.bandpass_hz = freq;
                self.precompute_height_freq_weights();
                return Ok(());
            }
            return Err("Bandpass frequency must be greater than LFE cutoff".to_string());
        } else if id == self.param_enable_subharmonic_synth {
            if let Some(enable) = value.as_bool() {
                self.enable_subharmonic_synth = enable;
                return Ok(());
            }
        } else if id == self.param_subharmonic_gain
            && let Some(gain) = value.as_float()
        {
            if (0.0..=1.0).contains(&gain) {
                self.subharmonic_gain.set_target(gain);
                return Ok(());
            }
            return Err("Sub-harmonic gain must be between 0.0 and 1.0".to_string());
        } else if id == self.param_enable_hr_direct {
            if let Some(enable) = value.as_bool() {
                self.enable_hr_direct = enable;
                return Ok(());
            }
        } else if id == self.param_hr_sharpen
            && let Some(sharpen) = value.as_float()
        {
            if (0.0..=1.0).contains(&sharpen) {
                self.hr_sharpen = sharpen;
                return Ok(());
            }
            return Err("HR sharpen must be between 0.0 and 1.0".to_string());
        } else if id == self.param_decorrelation_mode {
            if let Some(mode) = value.as_int() {
                if mode == 0 || mode == 1 {
                    self.decorrelation_mode = mode as usize;
                    // Re-generate filters when mode changes
                    self.generate_decorrelation_filters();
                    return Ok(());
                }
                return Err("Invalid decorrelation mode".to_string());
            }
        } else if id == self.param_safety_cap_db
            && let Some(val) = value.as_float()
        {
            if (0.0..=3.0).contains(&val) {
                self.safety_cap_db = val;
                self.update_safety_cap_cache();
                return Ok(());
            }
            return Err("Safety cap must be between 0.0 and 3.0 dB".to_string());
        }
        // Sub-harmonic synthesis parameters
        else if id == self.param_subharmonic_freq_hz
            && let Some(val) = value.as_float()
        {
            self.subharmonic_freq_hz = val.clamp(20.0, 80.0);
            return Ok(());
        } else if id == self.param_subharmonic_attack_ms
            && let Some(val) = value.as_float()
        {
            self.subharmonic_attack_ms = val.clamp(1.0, 100.0);
            return Ok(());
        } else if id == self.param_subharmonic_release_ms
            && let Some(val) = value.as_float()
        {
            self.subharmonic_release_ms = val.clamp(10.0, 500.0);
            return Ok(());
        }
        // Decorrelation parameters
        else if id == self.param_decorrelation_lfo_rate_hz
            && let Some(val) = value.as_float()
        {
            self.decorrelation_lfo_rate_hz = val.clamp(0.01, 1.0);
            return Ok(());
        } else if id == self.param_velvet_noise_duration_ms
            && let Some(val) = value.as_float()
        {
            self.velvet_noise_duration_ms = val.clamp(10.0, 100.0);
            // Regenerate velvet noise filters with new duration
            if self.decorrelation_mode == 0 {
                self.generate_velvet_noise_decorrelators();
            }
            return Ok(());
        } else if id == self.param_velvet_noise_density
            && let Some(val) = value.as_float()
        {
            self.velvet_noise_density = val.clamp(500.0, 5000.0);
            // Regenerate velvet noise filters with new density
            if self.decorrelation_mode == 0 {
                self.generate_velvet_noise_decorrelators();
            }
            return Ok(());
        }
        // Height channel parameters
        else if id == self.param_height_hf_cap_hz
            && let Some(val) = value.as_float()
        {
            self.height_hf_cap_hz = val.clamp(8000.0, 20000.0);
            self.precompute_height_freq_weights();
            return Ok(());
        } else if id == self.param_height_transient_reduction
            && let Some(val) = value.as_float()
        {
            self.height_transient_reduction = val.clamp(0.0, 1.0);
            return Ok(());
        } else if id == self.param_height_direct_leak
            && let Some(val) = value.as_float()
        {
            self.height_direct_leak = val.clamp(0.0, 0.5);
            return Ok(());
        }
        // Surround routing parameters
        else if id == self.param_surround_direct_bleed
            && let Some(val) = value.as_float()
        {
            self.surround_direct_bleed = val.clamp(0.0, 1.0);
            return Ok(());
        } else if id == self.param_rear_ambient_boost
            && let Some(val) = value.as_float()
        {
            self.rear_ambient_boost = val.clamp(1.0, 3.0);
            return Ok(());
        } else if id == self.param_rear_late_reflection
            && let Some(val) = value.as_float()
        {
            self.rear_late_reflection = val.clamp(0.0, 0.5);
            return Ok(());
        }
        // Ambient parameters
        else if id == self.param_ambient_boost
            && let Some(val) = value.as_float()
        {
            self.ambient_boost = val.clamp(0.5, 2.0);
            return Ok(());
        }
        // Dialogue detection parameters
        else if id == self.param_dialogue_weight
            && let Some(val) = value.as_float()
        {
            self.dialogue_weight = val.clamp(0.0, 1.0);
            return Ok(());
        } else if id == self.param_voice_freq_min_hz
            && let Some(val) = value.as_float()
        {
            self.voice_freq_min_hz = val.clamp(200.0, 800.0);
            return Ok(());
        } else if id == self.param_voice_freq_max_hz
            && let Some(val) = value.as_float()
        {
            self.voice_freq_max_hz = val.clamp(2000.0, 5000.0);
            return Ok(());
        } else if id == self.param_bypass_decorrelation {
            if let Some(enable) = value.as_bool() {
                self.bypass_decorrelation = enable;
                self.generate_decorrelation_filters();
                return Ok(());
            }
        } else if id == self.param_bypass_transient_detection {
            if let Some(enable) = value.as_bool() {
                self.bypass_transient_detection = enable;
                return Ok(());
            }
        } else if id == self.param_bypass_all_processing
            && let Some(enable) = value.as_bool() {
                self.bypass_all_processing = enable;
                return Ok(());
            }
        Err(format!("Unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_speaker_config {
            let config_idx = match self.speaker_config.id {
                "5.1" => 0,
                "7.1" => 1,
                "5.1.2" => 2,
                "5.1.4" => 3,
                "7.1.2" => 4,
                "7.1.4" => 5,
                "9.1.4" => 6,
                "9.1.6" => 7,
                "2.0" => 8,
                "5.0" => 9,
                _ => 0,
            };
            Some(ParameterValue::Int(config_idx))
        } else if id == &self.param_gain_front_direct {
            Some(ParameterValue::Float(self.gain_front_direct.target()))
        } else if id == &self.param_gain_front_ambient {
            Some(ParameterValue::Float(self.gain_front_ambient.target()))
        } else if id == &self.param_gain_rear_ambient {
            Some(ParameterValue::Float(self.gain_rear_ambient.target()))
        } else if id == &self.param_height_gain {
            Some(ParameterValue::Float(self.height_gain.target()))
        } else if id == &self.param_lfe_gain {
            Some(ParameterValue::Float(self.lfe_gain.target()))
        } else if id == &self.param_lfe_cutoff_hz {
            Some(ParameterValue::Float(self.lfe_cutoff_hz))
        } else if id == &self.param_stereo_width {
            Some(ParameterValue::Float(self.stereo_width))
        } else if id == &self.param_center_spread {
            Some(ParameterValue::Float(self.center_spread))
        } else if id == &self.param_bandpass_hz {
            Some(ParameterValue::Float(self.bandpass_hz))
        } else if id == &self.param_enable_subharmonic_synth {
            Some(ParameterValue::Bool(self.enable_subharmonic_synth))
        } else if id == &self.param_subharmonic_gain {
            Some(ParameterValue::Float(self.subharmonic_gain.target()))
        } else if id == &self.param_enable_hr_direct {
            Some(ParameterValue::Bool(self.enable_hr_direct))
        } else if id == &self.param_hr_sharpen {
            Some(ParameterValue::Float(self.hr_sharpen))
        } else if id == &self.param_safety_cap_db {
            Some(ParameterValue::Float(self.safety_cap_db))
        }
        // Sub-harmonic synthesis parameters
        else if id == &self.param_subharmonic_freq_hz {
            Some(ParameterValue::Float(self.subharmonic_freq_hz))
        } else if id == &self.param_subharmonic_attack_ms {
            Some(ParameterValue::Float(self.subharmonic_attack_ms))
        } else if id == &self.param_subharmonic_release_ms {
            Some(ParameterValue::Float(self.subharmonic_release_ms))
        }
        // Decorrelation parameters
        else if id == &self.param_decorrelation_lfo_rate_hz {
            Some(ParameterValue::Float(self.decorrelation_lfo_rate_hz))
        } else if id == &self.param_velvet_noise_duration_ms {
            Some(ParameterValue::Float(self.velvet_noise_duration_ms))
        } else if id == &self.param_velvet_noise_density {
            Some(ParameterValue::Float(self.velvet_noise_density))
        }
        // Height channel parameters
        else if id == &self.param_height_hf_cap_hz {
            Some(ParameterValue::Float(self.height_hf_cap_hz))
        } else if id == &self.param_height_transient_reduction {
            Some(ParameterValue::Float(self.height_transient_reduction))
        } else if id == &self.param_height_direct_leak {
            Some(ParameterValue::Float(self.height_direct_leak))
        }
        // Surround routing parameters
        else if id == &self.param_surround_direct_bleed {
            Some(ParameterValue::Float(self.surround_direct_bleed))
        } else if id == &self.param_rear_ambient_boost {
            Some(ParameterValue::Float(self.rear_ambient_boost))
        } else if id == &self.param_rear_late_reflection {
            Some(ParameterValue::Float(self.rear_late_reflection))
        }
        // Ambient parameters
        else if id == &self.param_ambient_boost {
            Some(ParameterValue::Float(self.ambient_boost))
        }
        // Dialogue detection parameters
        else if id == &self.param_dialogue_weight {
            Some(ParameterValue::Float(self.dialogue_weight))
        } else if id == &self.param_voice_freq_min_hz {
            Some(ParameterValue::Float(self.voice_freq_min_hz))
        } else if id == &self.param_voice_freq_max_hz {
            Some(ParameterValue::Float(self.voice_freq_max_hz))
        } else if id == &self.param_bypass_decorrelation {
            Some(ParameterValue::Bool(self.bypass_decorrelation))
        } else if id == &self.param_bypass_transient_detection {
            Some(ParameterValue::Bool(self.bypass_transient_detection))
        } else if id == &self.param_bypass_all_processing {
            Some(ParameterValue::Bool(self.bypass_all_processing))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        const MIN_SAMPLE_RATE: u32 = 8_000;
        const MAX_SAMPLE_RATE: u32 = 384_000;

        if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate) {
            return Err(format!(
                "Invalid sample rate: {} Hz (valid range: {}-{} Hz)",
                sample_rate, MIN_SAMPLE_RATE, MAX_SAMPLE_RATE
            ));
        }

        self.sample_rate = sample_rate;

        // Calculate ERB bands
        self.calculate_erb_bands();

        // Resize state vectors based on number of bands
        let num_bands = self.erb_bands.len();
        self.steering_alphas = vec![0.15; num_bands];
        self.pca_cov_xx = vec![0.0; num_bands];
        self.pca_cov_yy = vec![0.0; num_bands];
        self.pca_cov_xy = vec![Complex::new(0.0, 0.0); num_bands];
        self.coherence_instant = vec![0.0; num_bands];
        self.smoothed_coherence = vec![0.0; num_bands];
        self.coherence_history = vec![[0.0; 5]; num_bands];
        self.coherence_history_idx = 0;

        // Initialize spectral flux buffers
        let spectrum_size = self.fft_size / 2 + 1;
        self.prev_magnitude_spectrum = vec![0.0; spectrum_size];
        self.spectral_flux_smooth = 0.0;

        // Generate decorrelation filters
        self.generate_decorrelation_filters();

        // Generate per-channel decorrelation filters
        self.generate_per_channel_decorrelation_filters();

        // Precompute LR4 crossover gains for mains/LFE split
        self.update_crossover_gains();

        // Precompute height frequency weights (hf_ratio^0.7 per bin)
        self.precompute_height_freq_weights();

        // Update cached safety cap values
        self.update_safety_cap_cache();

        // Initialize smoothers
        let time_ms = 50.0;
        self.gain_front_direct.set_time(time_ms, sample_rate);
        self.gain_front_ambient.set_time(time_ms, sample_rate);
        self.gain_rear_ambient.set_time(time_ms, sample_rate);
        self.height_gain.set_time(time_ms, sample_rate);
        self.lfe_gain.set_time(time_ms, sample_rate);
        self.subharmonic_gain.set_time(time_ms, sample_rate);

        Ok(())
    }

    fn reset(&mut self) {
        // Clear buffers
        self.input_buffer_fill = 0;
        self.hr_input_buffer_fill = 0;
        let zero = Complex::new(0.0, 0.0);

        // Clear real-valued time domain buffers
        self.time_domain_left.fill(0.0);
        self.time_domain_right.fill(0.0);
        self.hr_time_domain_left.fill(0.0);
        self.hr_time_domain_right.fill(0.0);

        // Clear complex frequency domain buffers
        for buf in [
            &mut self.freq_domain_left,
            &mut self.freq_domain_right,
            &mut self.direct,
            &mut self.direct_left,
            &mut self.direct_right,
            &mut self.ambient_left,
            &mut self.ambient_right,
            &mut self.lfe,
            &mut self.hr_freq_domain_left,
            &mut self.hr_freq_domain_right,
        ]
        .iter_mut()
        {
            buf.fill(zero);
        }

        // Clear output channels
        for channel_buf in self.time_out_channels.iter_mut() {
            channel_buf.fill(0.0);
        }

        // Clear HR output channels
        for channel_buf in self.hr_time_out_channels.iter_mut() {
            channel_buf.fill(0.0);
        }

        self.output_accumulator.iter_mut().for_each(|b| b.fill(0.0));
        self.output_accumulator_fill = 0;
        self.output_block.fill(0.0);
        self.next_add_position = 0;
        self.output_read_position = 0;

        // Clear HR input and temp blocks
        self.hr_input_buffer.fill(0.0);
        self.hr_temp_input_block.fill(0.0);
        self.hr_next_add_position = 0;

        // Reset state vectors
        self.steering_alphas.fill(0.15);
        self.pca_cov_xx.fill(0.0);
        self.pca_cov_yy.fill(0.0);
        self.pca_cov_xy.fill(Complex::new(0.0, 0.0));
        self.coherence_instant.fill(0.0);
        self.smoothed_coherence.fill(0.0);
        self.hr_transient_env = 0.0;
        self.hr_energy_smooth = 0.0;
        self.prev_magnitude_spectrum.fill(0.0);
        self.spectral_flux_smooth = 0.0;
        self.coherence_history_idx = 0;
        for h in &mut self.coherence_history {
            *h = [0.0; 5];
        }

        // Clear height mask; it will be recomputed in process_fft_block
        self.height_band_gains.fill(0.0);
        self.height_band_gains_prev.fill(0.0);
        self.height_band_gains_temp.fill(0.0);
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        // Update smoothers once per block
        self.gain_front_direct.next();
        self.gain_front_ambient.next();
        self.gain_rear_ambient.next();
        self.height_gain.next();
        self.lfe_gain.next();
        self.subharmonic_gain.next();

        // If bypass is enabled, just copy stereo input to output and return
        if self.bypass_all_processing {
            let num_frames = context.num_frames;
            for i in 0..num_frames {
                let left = input[i * 2];
                let right = input[i * 2 + 1];

                output[i * self.num_output_channels] = left; // FL
                if self.num_output_channels > 1 {
                    output[i * self.num_output_channels + 1] = right; // FR
                }
                if self.num_output_channels > 2 {
                    output[i * self.num_output_channels + 2] = (left + right) * 0.5; // C
                }
                for ch in 3..self.num_output_channels {
                    output[i * self.num_output_channels + ch] = 0.0;
                }
            }
            return Ok(context.num_frames);
        }

        // Verify input size
        let input_samples = context.num_frames * 2;
        if input.len() != input_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                input_samples,
                input.len()
            ));
        }

        let output_samples = context.num_frames * self.num_output_channels;
        if output.len() != output_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                output_samples,
                output.len()
            ));
        }

        // Initialize output buffer to zero
        output.fill(0.0);

        let mut input_pos = 0;
        let mut output_pos = 0;
        let buffer_size = self.output_accumulator[0].len();

        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > 1000 {
                break;
            }

            // Step 1: Drain available output from ring buffer
            let frames_to_drain = self
                .output_accumulator_fill
                .min(context.num_frames - output_pos);

            if frames_to_drain > 0 {
                for i in 0..frames_to_drain {
                    let read_idx = (self.output_read_position + i) % buffer_size;
                    let out_frame_idx = (output_pos + i) * self.num_output_channels;
                    for ch in 0..self.num_output_channels {
                        output[out_frame_idx + ch] = self.output_accumulator[ch][read_idx];
                        // Clear after reading for next overlap-add cycle
                        self.output_accumulator[ch][read_idx] = 0.0;
                    }
                }
                self.output_read_position = (self.output_read_position + frames_to_drain) % buffer_size;
                self.output_accumulator_fill -= frames_to_drain;
                output_pos += frames_to_drain;
            }

            // Step 2: Process FFT block if we have enough input
            let can_process_input = self.input_buffer_fill >= self.fft_size * 2;
            if can_process_input {
                // Copy to temp buffer
                self.temp_input_block[..self.fft_size * 2]
                    .copy_from_slice(&self.input_buffer[..self.fft_size * 2]);

                let temp_input = std::mem::take(&mut self.temp_input_block);
                let mut output_block = std::mem::take(&mut self.output_block);
                self.process_fft_block(&temp_input, &mut output_block);

                if self.enable_hr_direct && self.gain_front_direct.current() > 0.0 {
                    let hr_mix = (self.hr_transient_env * self.hr_sharpen).clamp(0.0, 1.0);
                    if hr_mix > 0.01 {
                        let center = (self.fft_size - self.hr_fft_size) / 2;
                        let start = center * 2;
                        let end = start + self.hr_fft_size * 2;

                        if end <= temp_input.len() {
                            let hr_input = &temp_input[start..end];
                            let mut hr_output = std::mem::take(&mut self.hr_output_block);
                            self.process_hr_block(hr_input, &mut hr_output);

                            for i in 0..self.hr_fft_size {
                                let dst_idx = (center + i) * self.num_output_channels;
                                let src_idx = i * self.num_output_channels;
                                let window_val = self.hr_window[i];
                                let scaled_mix = hr_mix * window_val;
                                for ch in 0..self.num_output_channels {
                                    output_block[dst_idx + ch] +=
                                        hr_output[src_idx + ch] * scaled_mix;
                                }
                            }
                            self.hr_output_block = hr_output;
                        }
                    }
                }

                self.temp_input_block = temp_input;

                // Accumulate to ring buffer
                for i in 0..self.fft_size {
                    let write_idx = (self.next_add_position + i) % buffer_size;
                    for ch in 0..self.num_output_channels {
                        self.output_accumulator[ch][write_idx] +=
                            output_block[i * self.num_output_channels + ch];
                    }
                }

                self.output_block = output_block;

                // Advance positions
                self.next_add_position = (self.next_add_position + self.hop_size) % buffer_size;
                self.output_accumulator_fill += self.hop_size;

                // Shift input buffer
                let shift_amount = self.hop_size * 2;
                self.input_buffer
                    .copy_within(shift_amount..self.fft_size * 2, 0);
                self.input_buffer_fill -= shift_amount;

                continue;
            }

            // Step 3: Fill input buffer if we have more input
            if input_pos < context.num_frames {
                let samples_to_copy =
                    (input_samples - input_pos * 2).min(self.fft_size * 2 - self.input_buffer_fill);

                self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                    .copy_from_slice(&input[input_pos * 2..input_pos * 2 + samples_to_copy]);

                self.input_buffer_fill += samples_to_copy;
                input_pos += samples_to_copy / 2;

                continue;
            }

            break;
        }

        // Return actual number of frames produced. DawHost handles silence padding.
        Ok(output_pos)
    }

    fn latency_samples(&self) -> usize {
        self.fft_size
    }
}

#[cfg(test)]
mod test;
