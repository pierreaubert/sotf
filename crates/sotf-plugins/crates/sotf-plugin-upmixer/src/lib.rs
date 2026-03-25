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

use math_audio_dsp::fast_math::fast_pow10;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
pub mod params;

use crate::params::PARAMS as UP;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::speaker_config::{SpeakerConfig, get_speaker_config};
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
#[cfg(feature = "onnx")]
mod ml_features;
#[cfg(feature = "onnx")]
mod ml_inference;
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
    /// When true, output is binaural (2ch) instead of surround
    binaural_preview: bool,

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
    stereo_width: Smoother,

    param_center_spread: ParameterId,
    center_spread: Smoother,

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
    hr_sharpen: Smoother,

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
    /// Smoothed amplitude envelope for sub-harmonic modulation (prevents raw AM distortion)
    subharmonic_amp_envelope: f32,

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
    height_transient_reduction: Smoother,
    param_height_direct_leak: ParameterId,
    height_direct_leak: Smoother,

    // Surround routing parameters
    param_surround_direct_bleed: ParameterId,
    surround_direct_bleed: Smoother,
    param_rear_ambient_boost: ParameterId,
    rear_ambient_boost: Smoother,
    param_rear_late_reflection: ParameterId,
    rear_late_reflection: Smoother,

    // Ambient/coherence parameters
    param_ambient_boost: ParameterId,
    ambient_boost: Smoother,

    // Dialogue detection parameters
    param_dialogue_weight: ParameterId,
    dialogue_weight: Smoother,
    param_voice_freq_min_hz: ParameterId,
    voice_freq_min_hz: f32,
    param_voice_freq_max_hz: ParameterId,
    voice_freq_max_hz: f32,

    // Dialogue detection sub-weights
    param_dialogue_centroid_weight: ParameterId,
    dialogue_centroid_weight: f32,
    param_dialogue_variance_weight: ParameterId,
    dialogue_variance_weight: f32,
    param_dialogue_coherence_weight: ParameterId,
    dialogue_coherence_weight: f32,

    // ML vocal detection parameters
    param_enable_ml_detection: ParameterId,
    enable_ml_detection: bool,
    param_ml_model_path: ParameterId,
    ml_model_path: String,
    #[cfg(feature = "onnx")]
    mfcc_extractor: Option<ml_features::MfccExtractor>,
    #[cfg(feature = "onnx")]
    ml_inference_handle: Option<ml_inference::MlInferenceHandle>,

    // Low-latency mode
    param_low_latency: ParameterId,
    low_latency: bool,

    // Diagnostic bypass parameters
    param_bypass_decorrelation: ParameterId,
    bypass_decorrelation: bool,
    param_bypass_transient_detection: ParameterId,
    bypass_transient_detection: bool,
    param_bypass_all_processing: ParameterId,
    bypass_all_processing: bool,

    // Frequency resolution for ERB band analysis
    param_frequency_resolution: ParameterId,
    frequency_resolution: String,

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
    /// Precomputed per-bin LFO depth table (depends on sample_rate, fft_size, bandpass_hz, lfe_cutoff_hz)
    cached_lfo_depth_table: Vec<f32>,

    // PCA State (per band)
    pca_cov_xx: Vec<f32>,
    pca_cov_yy: Vec<f32>,
    pca_cov_xy: Vec<Complex<f32>>,

    // Crossover complex gain tables (Linkwitz-Riley between mains and LFE)
    // Complex values preserve phase information for accurate crossover behavior
    lfe_low_gains: Vec<Complex<f32>>,
    mains_high_gains: Vec<Complex<f32>>,

    // Height channel mask per positive-frequency bin (HF emphasis + coherence gating)
    height_band_gains: Vec<f32>,
    // Temporal smoothing buffer for height gains (previous frame)
    height_band_gains_prev: Vec<f32>,
    // Temporary buffer for height gain smoothing (avoid real-time allocation)
    height_band_gains_temp: Vec<f32>,

    // Energy correction smoothing for L/R decomposition (eliminates ERB band-edge gain jumps)
    energy_correction_per_bin: Vec<f32>,
    energy_correction_temp: Vec<f32>,
    energy_correction_prev: Vec<f32>,

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

    // Cached per-speaker flags (computed in recalculate_panning_gains, avoids string/float comparisons in hot path)
    /// True if speaker is front (|azimuth| < 80)
    cached_is_front: Vec<bool>,
    /// True if speaker is height (elevation > 10)
    cached_is_height: Vec<bool>,
    /// True if speaker is center (label == "C")
    cached_is_center: Vec<bool>,
    /// Indices of channels that the HR path processes (front, non-LFE, non-height)
    cached_hr_active_channels: Vec<usize>,

    // Cached bin indices for ERB-band and dialogue processing (recomputed in initialize()
    // and when lfe_cutoff_hz, bandpass_hz, voice_freq_min/max_hz, or sample_rate changes).
    /// Hz per FFT bin: sample_rate / fft_size
    cached_freq_per_bin: f32,
    /// Bin index of the LFE crossover cutoff
    cached_lfe_cutoff_bin: usize,
    /// Bin index of the bandpass upper bound
    cached_bandpass_bin: usize,
    /// First bin of the voice frequency range for dialogue detection
    cached_voice_start_bin: usize,
    /// Last bin of the voice frequency range for dialogue detection
    cached_voice_end_bin: usize,
    /// Normalized dialogue centroid weight (w_c in the weighted sum)
    cached_dialogue_w_c: f32,
    /// Normalized dialogue variance weight (w_v in the weighted sum)
    cached_dialogue_w_v: f32,
    /// Normalized dialogue coherence weight (w_coh in the weighted sum)
    cached_dialogue_w_coh: f32,

    // Cached sub-harmonic envelope coefficients (recomputed in initialize() and when
    // subharmonic_freq_hz, subharmonic_attack_ms, subharmonic_release_ms change).
    /// Phase increment per sample: 2π * subharmonic_freq_hz / sample_rate
    cached_subharmonic_phase_inc: f32,
    /// One-pole attack coefficient for envelope follower
    cached_subharmonic_attack_coeff: f32,
    /// One-pole release coefficient for envelope follower
    cached_subharmonic_release_coeff: f32,

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
    /// Pre-computed raised-cosine edge taper table for height channels (avoids cos() in hot path)
    edge_taper_table: Vec<f32>,
    /// Output accumulator for overlap-add (flat interleaved ring buffer)
    /// Layout: [ch0_f0, ch1_f0, ..., ch0_f1, ch1_f1, ...]
    /// Buffer size in frames is always power-of-2 (4 * fft_size) for efficient masking
    output_accumulator: Vec<f32>,
    /// Bitmask for ring buffer frame index (buffer_frames - 1), replaces % operator
    output_accumulator_mask: usize,
    /// Number of valid frames in output accumulator
    output_accumulator_fill: usize,
    /// Next frame position to add a block (tracks overlap-add offset)
    next_add_position: usize,
    /// Current read frame position in the output accumulator ring buffer
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
    /// Pre-allocated temp buffer for delay-compensated HR input (avoids hot-path allocation)
    hr_delay_temp: Vec<f32>,
    /// Exact temporal delay buffer to phase-align the fast HR OLA path with the slow Main OLA path
    hr_delay_buffer: Vec<f32>,
    /// Ring buffer cursor for delay buffer
    hr_delay_cursor: usize,
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
    /// Next position to add a HR block in the shared accumulator (reserved)
    hr_next_add_position: usize,

    // HR output accumulator
    hr_output_accumulator: Vec<f32>,
    hr_output_accumulator_mask: usize,
    hr_output_accumulator_fill: usize,
    hr_output_read_position: usize,

    /// Smooth envelope for enable_hr_direct toggle (0.0=off, 1.0=on)
    hr_direct_envelope: f32,

    hr_transient_env: f32,
    height_transient_env_slow: f32,
    hr_energy_smooth: f32,
    /// Previous frame magnitude spectrum for spectral flux calculation
    prev_magnitude_spectrum: Vec<f32>,
    /// Smoothed spectral flux for transient normalization
    spectral_flux_smooth: f32,

    // --- Intensity-vector DOA state (per ERB band) ---
    /// Smoothed DOA angle per ERB band (radians, from atan2 of active intensity)
    doa_angle: Vec<f32>,

    // --- Height channel spectral flux gating ---
    /// Previous frame magnitude spectrum for height spectral flux (per bin)
    height_prev_magnitude: Vec<f32>,
    /// Smoothed spectral flux for height onset detection
    height_spectral_flux_smooth: f32,
    /// Per-bin height gate multiplier from spectral flux / coherence gating
    height_flux_gate: Vec<f32>,

    // Smoothing state
    prev_hr_scale: f32,

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
    pub(crate) decorrelation_strength: f32,
    /// Previous decorrelation strength for skipping redundant filter blends
    prev_decorrelation_strength: f32,
    /// Pre-calculated blended decorrelation filters (one per channel)
    pub(crate) blended_decorrelation_filters: Vec<Vec<Complex<f32>>>,

    /// Smoother for lfe_cutoff_hz to prevent clicks when changing crossover frequency
    lfe_cutoff_hz_smoother: Smoother,
    /// Smoother for bandpass_hz to prevent clicks when changing upmix crossover
    bandpass_hz_smoother: Smoother,
    /// Smoother for height_hf_cap_hz to prevent clicks when changing height HF cap
    height_hf_cap_hz_smoother: Smoother,
    /// Smoother for safety_cap_db to prevent clicks when changing safety cap
    safety_cap_db_smoother: Smoother,

    /// Cross-fade counter for decorrelation mode/bypass transitions (blocks remaining)
    decorrelation_crossfade_remaining: usize,
    /// Saved blended filters for cross-fading during decorrelation transitions
    prev_blended_filters_for_crossfade: Vec<Vec<Complex<f32>>>,

    // Multi-source extraction (2nd eigenvector)
    /// Enable secondary source extraction using the 2nd PCA eigenvector.
    /// When a band contains two uncorrelated sources, the 2nd eigenvector captures
    /// the direction perpendicular to the dominant source and routes it to surrounds.
    param_multi_source_extraction: ParameterId,
    multi_source_extraction: bool,
    /// Threshold ratio lambda2/lambda1 above which the 2nd source is considered real.
    /// Range: 0.05-0.5, default 0.1.
    param_multi_source_threshold: ParameterId,
    multi_source_threshold: f32,
    /// Per-bin frequency-domain buffer for the secondary source (2nd eigenvector projection).
    /// Only populated when multi_source_extraction is enabled.
    direct2: Vec<rustfft::num_complex::Complex<f32>>,
    /// Per-bin DOA angle (radians) for the secondary source.
    /// Copied from the ERB band's DOA angle during frequency domain processing.
    /// Used by panning.rs to steer direct2 to the correct surround speaker.
    direct2_doa_per_bin: Vec<f32>,

    /// Initial latency counter to ensure OLA buffer is primed before output
    latency_filled: usize,
    cached_parameters: Vec<Parameter>,
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

        // Pre-compute raised-cosine edge taper for height channels (64 values)
        const TAPER_LEN: usize = 64;
        let edge_taper_table: Vec<f32> = (0..TAPER_LEN)
            .map(|i| {
                let t = i as f32 / TAPER_LEN as f32;
                0.5 * (1.0 - (std::f32::consts::PI * t).cos())
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

        // Panning gains will be calculated by recalculate_panning_gains() below.
        // The upmixer is stereo-only; input_channels() returns 2.
        let panning_gains_left = Vec::with_capacity(num_output_channels);
        let panning_gains_right = Vec::with_capacity(num_output_channels);

        // Output accumulator: flat interleaved ring buffer
        // 4 * fft_size frames (power-of-2 since fft_size is power-of-2)
        let accumulator_frames = fft_size * 4;
        debug_assert!(accumulator_frames.is_power_of_two());
        let output_accumulator = vec![0.0; accumulator_frames * num_output_channels];
        let output_accumulator_mask = accumulator_frames - 1;

        // Allocate output buffers for each channel
        let time_out_channels = vec![vec![0.0; fft_size]; num_output_channels];

        let mut plugin = Self {
            fft_size,
            hop_size,
            sample_rate,
            speaker_config,
            num_output_channels,
            binaural_preview: false,

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
            stereo_width: Smoother::new(stereo_width, 5.0, sample_rate),

            param_center_spread: ParameterId::from("center_spread"),
            center_spread: Smoother::new(default_center_spread(), 5.0, sample_rate),

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
            hr_sharpen: Smoother::new(1.0, 5.0, sample_rate),
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
            height_transient_reduction: Smoother::new(
                default_height_transient_reduction(),
                5.0,
                sample_rate,
            ),
            param_height_direct_leak: ParameterId::from("height_direct_leak"),
            height_direct_leak: Smoother::new(default_height_direct_leak(), 5.0, sample_rate),

            // Surround routing parameters
            param_surround_direct_bleed: ParameterId::from("surround_direct_bleed"),
            surround_direct_bleed: Smoother::new(default_surround_direct_bleed(), 5.0, sample_rate),
            param_rear_ambient_boost: ParameterId::from("rear_ambient_boost"),
            rear_ambient_boost: Smoother::new(default_rear_ambient_boost(), 5.0, sample_rate),
            param_rear_late_reflection: ParameterId::from("rear_late_reflection"),
            rear_late_reflection: Smoother::new(default_rear_late_reflection(), 5.0, sample_rate),

            // Ambient/coherence parameters
            param_ambient_boost: ParameterId::from("ambient_boost"),
            ambient_boost: Smoother::new(default_ambient_boost(), 5.0, sample_rate),

            // Dialogue detection parameters
            param_dialogue_weight: ParameterId::from("dialogue_weight"),
            dialogue_weight: Smoother::new(default_dialogue_weight(), 5.0, sample_rate),
            param_voice_freq_min_hz: ParameterId::from("voice_freq_min_hz"),
            voice_freq_min_hz: default_voice_freq_min_hz(),
            param_voice_freq_max_hz: ParameterId::from("voice_freq_max_hz"),
            voice_freq_max_hz: default_voice_freq_max_hz(),

            param_dialogue_centroid_weight: ParameterId::from("dialogue_centroid_weight"),
            dialogue_centroid_weight: default_dialogue_centroid_weight(),
            param_dialogue_variance_weight: ParameterId::from("dialogue_variance_weight"),
            dialogue_variance_weight: default_dialogue_variance_weight(),
            param_dialogue_coherence_weight: ParameterId::from("dialogue_coherence_weight"),
            dialogue_coherence_weight: default_dialogue_coherence_weight(),

            // ML vocal detection parameters
            param_enable_ml_detection: ParameterId::from("enable_ml_detection"),
            enable_ml_detection: false,
            param_ml_model_path: ParameterId::from("ml_model_path"),
            ml_model_path: String::new(),
            #[cfg(feature = "onnx")]
            mfcc_extractor: None,
            #[cfg(feature = "onnx")]
            ml_inference_handle: None,

            // Low-latency mode
            param_low_latency: ParameterId::from("low_latency"),
            low_latency: false,

            // Diagnostic bypass parameters
            param_bypass_decorrelation: ParameterId::from("bypass_decorrelation"),
            bypass_decorrelation: default_bypass_decorrelation(),
            param_bypass_transient_detection: ParameterId::from("bypass_transient_detection"),
            bypass_transient_detection: default_bypass_transient_detection(),
            param_bypass_all_processing: ParameterId::from("bypass_all_processing"),
            bypass_all_processing: default_bypass_all_processing(),

            // Frequency resolution for ERB band analysis
            param_frequency_resolution: ParameterId::from("frequency_resolution"),
            frequency_resolution: default_frequency_resolution(),

            subharmonic_phase: 0.0,
            subharmonic_envelope: 0.0,
            subharmonic_amp_envelope: 0.0,

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
            cached_lfo_depth_table: Vec::new(),

            pca_cov_xx: Vec::new(),
            pca_cov_yy: Vec::new(),
            pca_cov_xy: Vec::new(),

            lfe_low_gains: vec![Complex::new(1.0, 0.0); spectrum_size],
            mains_high_gains: vec![Complex::new(1.0, 0.0); spectrum_size],

            height_band_gains: vec![0.0; spectrum_size],
            height_band_gains_prev: vec![0.0; spectrum_size],
            height_band_gains_temp: vec![0.0; spectrum_size],

            energy_correction_per_bin: vec![1.0; spectrum_size],
            energy_correction_temp: vec![1.0; spectrum_size],
            energy_correction_prev: vec![1.0; spectrum_size],

            height_freq_weights: vec![0.0; spectrum_size],

            safety_cap_linear: if default_safety_cap_db() >= 0.0 {
                fast_pow10(default_safety_cap_db() / 20.0)
            } else {
                1.0
            },
            safety_cap_min_scale: if default_safety_cap_db() >= 0.0 {
                fast_pow10(-default_safety_cap_db() / 20.0)
            } else {
                0.0
            },

            panning_gains_left,
            panning_gains_right,

            cached_is_front: Vec::new(),
            cached_is_height: Vec::new(),
            cached_is_center: Vec::new(),
            cached_hr_active_channels: Vec::new(),

            // Bin-index caches — will be populated in initialize() once sample_rate is known
            cached_freq_per_bin: 0.0,
            cached_lfe_cutoff_bin: 0,
            cached_bandpass_bin: 0,
            cached_voice_start_bin: 0,
            cached_voice_end_bin: 0,
            cached_dialogue_w_c: 0.333,
            cached_dialogue_w_v: 0.333,
            cached_dialogue_w_coh: 0.334,

            // Sub-harmonic coefficient caches — will be populated in initialize()
            cached_subharmonic_phase_inc: 0.0,
            cached_subharmonic_attack_coeff: 0.0,
            cached_subharmonic_release_coeff: 0.0,

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
            edge_taper_table,
            output_accumulator,
            output_accumulator_mask,
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

            // Pre-allocated temp buffer for delay-compensated HR input
            hr_delay_temp: vec![0.0; fft_size * 2],
            // Delay buffer to align physical OLA overlap latency:
            // main_latency = fft_size - hop_size, hr_latency = hr_fft_size - hr_fft_size/2
            // delay = (main_latency - hr_latency) * 2 (stereo interleaved)
            hr_delay_buffer: vec![
                0.0;
                ((fft_size - hop_size) - (hr_fft_size - (hr_fft_size / 2))) * 2
            ],
            hr_delay_cursor: 0,

            // Sized to match main ring buffer (fft_size * 4) since all input feeds HR before drain
            hr_output_accumulator: vec![0.0; fft_size * 4 * num_output_channels],
            hr_output_accumulator_mask: (fft_size * 4) - 1,
            hr_output_accumulator_fill: 0,
            hr_next_add_position: 0,
            hr_output_read_position: 0,

            hr_direct_envelope: 1.0, // HR enabled by default
            hr_transient_env: 0.0,
            height_transient_env_slow: 0.0,
            hr_energy_smooth: 0.0,
            prev_magnitude_spectrum: vec![0.0; spectrum_size],
            spectral_flux_smooth: 0.0,

            // Intensity-vector DOA state (will be resized in calculate_erb_bands)
            doa_angle: Vec::new(),

            // Height spectral flux gating
            height_prev_magnitude: vec![0.0; spectrum_size],
            height_spectral_flux_smooth: 0.0,
            height_flux_gate: vec![0.0; spectrum_size],

            prev_hr_scale: 0.0,

            dialogue_spectral_centroid: 0.0,
            dialogue_envelope_variance: 0.0,
            dialogue_prev_rms: 0.0,
            dialogue_probability: 0.0,
            decorrelation_strength: 1.0,
            prev_decorrelation_strength: -1.0, // Force initial computation
            blended_decorrelation_filters: vec![
                vec![Complex::new(1.0, 0.0); fft_size / 2 + 1];
                num_output_channels
            ],

            lfe_cutoff_hz_smoother: Smoother::new(lfe_cutoff_hz, 5.0, sample_rate),
            bandpass_hz_smoother: Smoother::new(bandpass_hz, 5.0, sample_rate),
            height_hf_cap_hz_smoother: Smoother::new(default_height_hf_cap_hz(), 5.0, sample_rate),
            safety_cap_db_smoother: Smoother::new(default_safety_cap_db(), 5.0, sample_rate),

            decorrelation_crossfade_remaining: 0,
            prev_blended_filters_for_crossfade: Vec::new(),

            latency_filled: 0,

            param_multi_source_extraction: ParameterId::from("multi_source_extraction"),
            multi_source_extraction: false,
            param_multi_source_threshold: ParameterId::from("multi_source_threshold"),
            multi_source_threshold: 0.1,
            direct2: vec![zero_complex; spectrum_size],
            direct2_doa_per_bin: vec![0.0; spectrum_size],

            cached_parameters: Vec::new(),
        };

        // Calculate panning gains for stereo sources (left at +30°, right at -30°)
        plugin.recalculate_panning_gains();
        plugin.rebuild_cached_parameters();

        plugin
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_int(
                "speaker_config",
                "Configuration",
                match self.speaker_config.id {
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
                },
                pk(UP, "speaker_config").min_f64() as i32,
                pk(UP, "speaker_config").max_f64() as i32,
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
                self.gain_front_direct.target(),
                pk(UP, "gain_front_direct").min_f64() as f32,
                pk(UP, "gain_front_direct").max_f64() as f32,
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
                self.gain_front_ambient.target(),
                pk(UP, "gain_front_ambient").min_f64() as f32,
                pk(UP, "gain_front_ambient").max_f64() as f32,
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
                self.gain_rear_ambient.target(),
                pk(UP, "gain_rear_ambient").min_f64() as f32,
                pk(UP, "gain_rear_ambient").max_f64() as f32,
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
                self.height_gain.target(),
                pk(UP, "height_gain").min_f64() as f32,
                pk(UP, "height_gain").max_f64() as f32,
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
                self.lfe_gain.target(),
                pk(UP, "lfe_gain").min_f64() as f32,
                pk(UP, "lfe_gain").max_f64() as f32,
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
                self.lfe_cutoff_hz,
                pk(UP, "lfe_cutoff_hz").min_f64() as f32,
                pk(UP, "lfe_cutoff_hz").max_f64() as f32,
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
                self.stereo_width.target(),
                pk(UP, "stereo_width").min_f64() as f32,
                pk(UP, "stereo_width").max_f64() as f32,
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
                self.center_spread.target(),
                pk(UP, "center_spread").min_f64() as f32,
                pk(UP, "center_spread").max_f64() as f32,
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
                self.bandpass_hz,
                pk(UP, "bandpass_hz").min_f64() as f32,
                pk(UP, "bandpass_hz").max_f64() as f32,
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
                self.enable_subharmonic_synth,
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
                self.subharmonic_gain.target(),
                pk(UP, "subharmonic_gain").min_f64() as f32,
                pk(UP, "subharmonic_gain").max_f64() as f32,
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
                self.enable_hr_direct,
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
                self.hr_sharpen.target(),
                pk(UP, "hr_sharpen").min_f64() as f32,
                pk(UP, "hr_sharpen").max_f64() as f32,
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
                self.safety_cap_db,
                pk(UP, "safety_cap_db").min_f64() as f32,
                pk(UP, "safety_cap_db").max_f64() as f32,
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
                self.decorrelation_mode as i32,
                pk(UP, "decorrelation_mode").min_f64() as i32,
                pk(UP, "decorrelation_mode").max_f64() as i32,
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
                self.subharmonic_freq_hz,
                pk(UP, "subharmonic_freq_hz").min_f64() as f32,
                pk(UP, "subharmonic_freq_hz").max_f64() as f32,
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
                self.subharmonic_attack_ms,
                pk(UP, "subharmonic_attack_ms").min_f64() as f32,
                pk(UP, "subharmonic_attack_ms").max_f64() as f32,
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
                self.subharmonic_release_ms,
                pk(UP, "subharmonic_release_ms").min_f64() as f32,
                pk(UP, "subharmonic_release_ms").max_f64() as f32,
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
                self.decorrelation_lfo_rate_hz,
                pk(UP, "decorrelation_lfo_rate_hz").min_f64() as f32,
                pk(UP, "decorrelation_lfo_rate_hz").max_f64() as f32,
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
                self.velvet_noise_duration_ms,
                pk(UP, "velvet_noise_duration_ms").min_f64() as f32,
                pk(UP, "velvet_noise_duration_ms").max_f64() as f32,
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
                self.velvet_noise_density,
                pk(UP, "velvet_noise_density").min_f64() as f32,
                pk(UP, "velvet_noise_density").max_f64() as f32,
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
                self.height_hf_cap_hz,
                pk(UP, "height_hf_cap_hz").min_f64() as f32,
                pk(UP, "height_hf_cap_hz").max_f64() as f32,
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
                self.height_transient_reduction.target(),
                pk(UP, "height_transient_reduction").min_f64() as f32,
                pk(UP, "height_transient_reduction").max_f64() as f32,
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
                self.height_direct_leak.target(),
                pk(UP, "height_direct_leak").min_f64() as f32,
                pk(UP, "height_direct_leak").max_f64() as f32,
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
                self.surround_direct_bleed.target(),
                pk(UP, "surround_direct_bleed").min_f64() as f32,
                pk(UP, "surround_direct_bleed").max_f64() as f32,
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
                self.rear_ambient_boost.target(),
                pk(UP, "rear_ambient_boost").min_f64() as f32,
                pk(UP, "rear_ambient_boost").max_f64() as f32,
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
                self.rear_late_reflection.target(),
                pk(UP, "rear_late_reflection").min_f64() as f32,
                pk(UP, "rear_late_reflection").max_f64() as f32,
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
                self.ambient_boost.target(),
                pk(UP, "ambient_boost").min_f64() as f32,
                pk(UP, "ambient_boost").max_f64() as f32,
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
                self.dialogue_weight.target(),
                pk(UP, "dialogue_weight").min_f64() as f32,
                pk(UP, "dialogue_weight").max_f64() as f32,
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
                self.voice_freq_min_hz,
                pk(UP, "voice_freq_min_hz").min_f64() as f32,
                pk(UP, "voice_freq_min_hz").max_f64() as f32,
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
                self.voice_freq_max_hz,
                pk(UP, "voice_freq_max_hz").min_f64() as f32,
                pk(UP, "voice_freq_max_hz").max_f64() as f32,
            )
            .with_description(
                "Voice detection frequency range maximum.
Range: 2000-5000 Hz, default 3000 Hz.
Upper bound for dialogue detection analysis.",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "dialogue_centroid_weight",
                "Dialogue Centroid Weight",
                self.dialogue_centroid_weight,
                pk(UP, "dialogue_centroid_weight").min_f64() as f32,
                pk(UP, "dialogue_centroid_weight").max_f64() as f32,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "dialogue_variance_weight",
                "Dialogue Variance Weight",
                self.dialogue_variance_weight,
                pk(UP, "dialogue_variance_weight").min_f64() as f32,
                pk(UP, "dialogue_variance_weight").max_f64() as f32,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "dialogue_coherence_weight",
                "Dialogue Coherence Weight",
                self.dialogue_coherence_weight,
                pk(UP, "dialogue_coherence_weight").min_f64() as f32,
                pk(UP, "dialogue_coherence_weight").max_f64() as f32,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            // ML vocal detection parameters
            Parameter::new_bool(
                "enable_ml_detection",
                "ML Vocal Detection",
                self.enable_ml_detection,
            )
            .with_description(
                "Enable ML-based vocal detection using an ONNX model.
Default: off. When enabled and a valid model path is set,
replaces the heuristic dialogue detector with ML inference.
Falls back to heuristic if model loading fails.",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_string("ml_model_path", "ML Model Path", self.ml_model_path.clone())
                .with_description(
                    "Path to the ONNX model file for ML vocal detection.
Must be a valid file path to an ONNX model with input shape [1, 40]
and output shape [1, 1] (sigmoid probability).",
                )
                .with_group("Analysis")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool(
                "low_latency",
                "Low Latency",
                self.low_latency,
            )
            .with_description(
                "Low-latency mode: uses 1024-point FFT (~21ms at 48kHz) instead of 2048 (~43ms).
Halves analysis latency at the cost of coarser frequency resolution in spatial analysis.
Useful for live monitoring or real-time applications where latency matters.
Note: changing this requires re-initialization (takes effect on next initialize()).",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_string(
                "frequency_resolution",
                "Frequency Resolution",
                self.frequency_resolution.clone(),
            )
            .with_description(
                "ERB band frequency resolution for spatial analysis.
\"erb\" = standard ERB bands (~40-50 bands, default).
\"fine_erb\" = half-ERB width (~100 bands, finer spatial resolution).
\"per_bin\" = one band per FFT bin (~1025 bands, maximum resolution).
Note: changing this requires re-initialization (takes effect on next initialize()).",
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool(
                "bypass_decorrelation",
                "Bypass Decorrelation",
                self.bypass_decorrelation,
            )
            .with_group("Diagnostic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "bypass_transient_detection",
                "Bypass Transient Detection",
                self.bypass_transient_detection,
            )
            .with_group("Diagnostic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "bypass_all_processing",
                "Bypass All",
                self.bypass_all_processing,
            )
            .with_group("Diagnostic")
            .with_importance(ParameterImportance::Useful),
            // Multi-source extraction
            Parameter::new_bool(
                "multi_source_extraction",
                "Multi-Source Extraction",
                self.multi_source_extraction,
            )
            .with_description(
                "Enable secondary source extraction using the 2nd PCA eigenvector.
Default: off. When enabled and two uncorrelated sources are detected in a band
(lambda2/lambda1 > multi_source_threshold), the secondary source is routed to
L/R surround based on its direction of arrival.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "multi_source_threshold",
                "Multi-Source Threshold",
                self.multi_source_threshold,
                pk(UP, "multi_source_threshold").min_f64() as f32,
                pk(UP, "multi_source_threshold").max_f64() as f32,
            )
            .with_description(
                "Lambda ratio threshold for 2nd eigenvector activation.
Range: 0.05-0.5, default 0.1.
The secondary source is extracted only when lambda2/lambda1 exceeds this value,
ensuring the 2nd eigenvector captures a real source and not noise.",
            )
            .with_group("Enhancement")
            .with_importance(ParameterImportance::FineTuning),
            // Phase 4G
            Parameter::new_bool("binaural_preview", "Binaural Preview", self.binaural_preview)
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
        ];
    }

    /// Create a new upmixer plugin from configuration parameters
    pub fn from_params(params: UpmixerPluginParams) -> Self {
        // Low-latency mode halves the FFT size from 2048 to 1024 (21ms vs 43ms at 48kHz).
        // If the user explicitly set a custom fft_size, low_latency overrides it.
        let fft_size = if params.low_latency { 1024 } else { params.fft_size };
        let mut plugin = Self::new(
            fft_size,
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
        plugin
            .center_spread
            .set_target(params.center_spread.clamp(0.0, 1.0));
        plugin.enable_hr_direct = params.enable_hr_direct;
        plugin.hr_direct_envelope = if params.enable_hr_direct { 1.0 } else { 0.0 };
        plugin
            .hr_sharpen
            .set_target(params.hr_sharpen.clamp(0.0, 1.0));
        plugin.safety_cap_db = params.safety_cap_db.max(0.0);
        plugin
            .safety_cap_db_smoother
            .set_target(params.safety_cap_db.max(0.0));
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
        plugin
            .height_hf_cap_hz_smoother
            .set_target(params.height_hf_cap_hz.clamp(8000.0, 20000.0));
        plugin
            .height_transient_reduction
            .set_target(params.height_transient_reduction.clamp(0.0, 1.0));
        plugin
            .height_direct_leak
            .set_target(params.height_direct_leak.clamp(0.0, 0.5));

        // Surround routing parameters
        plugin
            .surround_direct_bleed
            .set_target(params.surround_direct_bleed.clamp(0.0, 1.0));
        plugin
            .rear_ambient_boost
            .set_target(params.rear_ambient_boost.clamp(1.0, 3.0));
        plugin
            .rear_late_reflection
            .set_target(params.rear_late_reflection.clamp(0.0, 0.5));

        // Ambient/coherence parameters
        plugin
            .ambient_boost
            .set_target(params.ambient_boost.clamp(0.5, 2.0));

        // Dialogue detection parameters
        plugin
            .dialogue_weight
            .set_target(params.dialogue_weight.clamp(0.0, 1.0));
        plugin.voice_freq_min_hz = params.voice_freq_min_hz.clamp(200.0, 800.0);
        plugin.voice_freq_max_hz = params.voice_freq_max_hz.clamp(2000.0, 5000.0);

        // Dialogue detection sub-weights
        plugin.dialogue_centroid_weight = params.dialogue_centroid_weight.clamp(0.0, 1.0);
        plugin.dialogue_variance_weight = params.dialogue_variance_weight.clamp(0.0, 1.0);
        plugin.dialogue_coherence_weight = params.dialogue_coherence_weight.clamp(0.0, 1.0);

        // ML vocal detection parameters
        plugin.enable_ml_detection = params.enable_ml_detection;
        plugin.ml_model_path = params.ml_model_path;

        // Low-latency mode
        plugin.low_latency = params.low_latency;

        // Diagnostic bypass parameters
        plugin.bypass_decorrelation = params.bypass_decorrelation;
        plugin.bypass_transient_detection = params.bypass_transient_detection;
        plugin.bypass_all_processing = params.bypass_all_processing;

        // Frequency resolution (construction-only: stored and applied in initialize())
        plugin.frequency_resolution = params.frequency_resolution;

        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Reallocate all FFT-size-dependent buffers and reset STFT state.
    /// Called when `low_latency` is toggled at runtime.
    /// This will cause a brief audio glitch (~20-40ms) as the ring buffers are drained.
    fn resize_fft(&mut self, new_fft_size: usize) {
        debug_assert!(new_fft_size.is_power_of_two());
        let old_fft_size = self.fft_size;
        if new_fft_size == old_fft_size {
            return;
        }

        self.fft_size = new_fft_size;
        self.hop_size = new_fft_size / 2;

        // Recreate FFT planners
        let mut planner = RealFftPlanner::<f32>::new();
        self.fft_forward = planner.plan_fft_forward(new_fft_size);
        self.fft_inverse = planner.plan_fft_inverse(new_fft_size);

        // Regenerate Hann window
        self.window = (0..new_fft_size)
            .map(|i| {
                0.5 * (1.0
                    - ((2.0 * std::f32::consts::PI * i as f32) / new_fft_size as f32).cos())
            })
            .collect();

        let spectrum_size = new_fft_size / 2 + 1;
        let zero_complex = Complex::new(0.0, 0.0);
        let nch = self.num_output_channels;

        // Buffers sized to fft_size
        self.time_domain_left = vec![0.0; new_fft_size];
        self.time_domain_right = vec![0.0; new_fft_size];
        self.output_block = vec![0.0; new_fft_size * nch];
        self.time_out_channels = vec![vec![0.0; new_fft_size]; nch];

        // Buffers sized to spectrum_size
        self.freq_domain_left = vec![zero_complex; spectrum_size];
        self.freq_domain_right = vec![zero_complex; spectrum_size];
        self.direct = vec![zero_complex; spectrum_size];
        self.direct_left = vec![zero_complex; spectrum_size];
        self.direct_right = vec![zero_complex; spectrum_size];
        self.ambient_left = vec![zero_complex; spectrum_size];
        self.ambient_right = vec![zero_complex; spectrum_size];
        self.lfe = vec![zero_complex; spectrum_size];
        self.temp_freq_out = vec![zero_complex; spectrum_size];
        self.decorrelation_filter_left = vec![zero_complex; spectrum_size];
        self.decorrelation_filter_right = vec![zero_complex; spectrum_size];
        self.lfe_low_gains = vec![Complex::new(1.0, 0.0); spectrum_size];
        self.mains_high_gains = vec![Complex::new(1.0, 0.0); spectrum_size];
        self.height_band_gains = vec![0.0; spectrum_size];
        self.height_band_gains_prev = vec![0.0; spectrum_size];
        self.height_band_gains_temp = vec![0.0; spectrum_size];
        self.energy_correction_per_bin = vec![1.0; spectrum_size];
        self.energy_correction_temp = vec![1.0; spectrum_size];
        self.energy_correction_prev = vec![1.0; spectrum_size];
        self.height_freq_weights = vec![0.0; spectrum_size];
        self.prev_magnitude_spectrum = vec![0.0; spectrum_size];
        self.height_prev_magnitude = vec![0.0; spectrum_size];
        self.height_flux_gate = vec![0.0; spectrum_size];
        self.blended_decorrelation_filters =
            vec![vec![Complex::new(1.0, 0.0); spectrum_size]; nch];
        self.prev_decorrelation_strength = -1.0; // Force recompute on next process()
        self.direct2 = vec![zero_complex; spectrum_size];
        self.direct2_doa_per_bin = vec![0.0; spectrum_size];

        // Buffers sized to fft_size * 2 (stereo interleaved)
        self.input_buffer = vec![0.0; new_fft_size * 2];
        self.temp_input_block = vec![0.0; new_fft_size * 2];

        // Ring buffer (fft_size * 4 frames)
        let accumulator_frames = new_fft_size * 4;
        self.output_accumulator = vec![0.0; accumulator_frames * nch];
        self.output_accumulator_mask = accumulator_frames - 1;

        // HR-path buffers that depend on main fft_size
        let hop_size = self.hop_size;
        let hr_fft_size = self.hr_fft_size;
        self.hr_delay_temp = vec![0.0; new_fft_size * 2];
        self.hr_delay_buffer =
            vec![0.0; ((new_fft_size - hop_size) - (hr_fft_size - (hr_fft_size / 2))) * 2];
        self.hr_delay_cursor = 0;
        self.hr_output_accumulator = vec![0.0; accumulator_frames * nch];
        self.hr_output_accumulator_mask = accumulator_frames - 1;

        // Reset all STFT state counters
        self.input_buffer_fill = 0;
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.hr_input_buffer_fill = 0;
        self.hr_output_accumulator_fill = 0;
        self.hr_next_add_position = 0;
        self.hr_output_read_position = 0;
        self.latency_filled = 0;

        // Re-initialize all derived state (ERB bands, decorrelation filters, crossover
        // gains, bin caches, smoothers, MFCC, etc.)
        let _ = self.initialize(self.sample_rate);

        self.rebuild_cached_parameters();
    }

    /// Try to start the ML inference thread if enabled and model path is set.
    /// Logs a warning and falls back to heuristic if it fails.
    #[cfg(feature = "onnx")]
    fn try_start_ml_inference(&mut self) {
        // Shut down any existing handle first
        self.ml_inference_handle = None;

        if !self.enable_ml_detection || self.ml_model_path.is_empty() {
            return;
        }

        match ml_inference::MlInferenceHandle::new(&self.ml_model_path) {
            Ok(handle) => {
                log::info!(
                    "ML vocal detection started with model: {}",
                    self.ml_model_path
                );
                self.ml_inference_handle = Some(handle);
            }
            Err(e) => {
                log::warn!(
                    "Failed to start ML vocal detection, falling back to heuristic: {}",
                    e
                );
                self.ml_inference_handle = None;
            }
        }
    }

    #[cfg(not(feature = "onnx"))]
    fn try_start_ml_inference(&mut self) {
        // ONNX runtime not available — ML vocal detection disabled
    }

    #[cfg(feature = "onnx")]
    fn try_ml_inference(&mut self) -> Option<f32> {
        if self.enable_ml_detection
            && self.ml_inference_handle.is_some()
            && let Some(ref mut extractor) = self.mfcc_extractor
        {
            let features =
                *extractor.compute(&self.freq_domain_left, &self.freq_domain_right);
            if let Some(ref mut handle) = self.ml_inference_handle {
                handle.send_features(&features);
                return handle.read_v_prob();
            }
        }
        None
    }

    #[cfg(not(feature = "onnx"))]
    fn try_ml_inference(&mut self) -> Option<f32> {
        None
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
        // binaural_preview is a host hint — the upmixer always outputs surround channels.
        // The host should chain a BinauralDecoderPlugin downstream when this flag is set.
        self.num_output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_speaker_config {
            let config_idx = value
                .as_int()
                .ok_or_else(|| "speaker_config must be an integer".to_string())?;
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
        } else if id == self.param_gain_front_direct {
            let gain = value
                .as_float()
                .ok_or_else(|| "gain_front_direct must be a float".to_string())?;
            if gain.is_finite() {
                self.gain_front_direct.set_target(gain);
            }
        } else if id == self.param_gain_front_ambient {
            let gain = value
                .as_float()
                .ok_or_else(|| "gain_front_ambient must be a float".to_string())?;
            if gain.is_finite() {
                self.gain_front_ambient.set_target(gain);
            }
        } else if id == self.param_gain_rear_ambient {
            let gain = value
                .as_float()
                .ok_or_else(|| "gain_rear_ambient must be a float".to_string())?;
            if gain.is_finite() {
                self.gain_rear_ambient.set_target(gain);
            }
        } else if id == self.param_height_gain {
            let gain = value
                .as_float()
                .ok_or_else(|| "height_gain must be a float".to_string())?;
            if gain.is_finite() {
                self.height_gain.set_target(gain.clamp(0.0, 2.0));
            }
        } else if id == self.param_lfe_gain {
            let gain = value
                .as_float()
                .ok_or_else(|| "lfe_gain must be a float".to_string())?;
            if gain.is_finite() {
                self.lfe_gain.set_target(gain.clamp(0.0, 2.0));
            }
        } else if id == self.param_lfe_cutoff_hz {
            let cutoff = value
                .as_float()
                .ok_or_else(|| "lfe_cutoff_hz must be a float".to_string())?;
            if cutoff.is_finite() && (20.0..=180.0).contains(&cutoff) && cutoff < self.bandpass_hz {
                self.lfe_cutoff_hz_smoother.set_target(cutoff);
            }
        } else if id == self.param_stereo_width {
            let width = value
                .as_float()
                .ok_or_else(|| "stereo_width must be a float".to_string())?;
            if width.is_finite() {
                self.stereo_width.set_target(width.clamp(0.0, 1.0));
            }
        } else if id == self.param_center_spread {
            let spread = value
                .as_float()
                .ok_or_else(|| "center_spread must be a float".to_string())?;
            if spread.is_finite() {
                self.center_spread.set_target(spread.clamp(0.0, 1.0));
            }
        } else if id == self.param_bandpass_hz {
            let freq = value
                .as_float()
                .ok_or_else(|| "bandpass_hz must be a float".to_string())?;
            if freq.is_finite() && freq > self.lfe_cutoff_hz {
                self.bandpass_hz_smoother.set_target(freq);
            }
        } else if id == self.param_enable_subharmonic_synth {
            self.enable_subharmonic_synth = value
                .as_bool()
                .ok_or_else(|| "enable_subharmonic_synth must be a boolean".to_string())?;
        } else if id == self.param_subharmonic_gain {
            let gain = value
                .as_float()
                .ok_or_else(|| "subharmonic_gain must be a float".to_string())?;
            if gain.is_finite() {
                self.subharmonic_gain.set_target(gain.clamp(0.0, 1.0));
            }
        } else if id == self.param_enable_hr_direct {
            self.enable_hr_direct = value
                .as_bool()
                .ok_or_else(|| "enable_hr_direct must be a boolean".to_string())?;
        } else if id == self.param_hr_sharpen {
            let sharpen = value
                .as_float()
                .ok_or_else(|| "hr_sharpen must be a float".to_string())?;
            if sharpen.is_finite() {
                self.hr_sharpen.set_target(sharpen.clamp(0.0, 1.0));
            }
        } else if id == self.param_decorrelation_mode {
            let mode = value
                .as_int()
                .ok_or_else(|| "decorrelation_mode must be an integer".to_string())?;
            if mode == 0 || mode == 1 {
                // Swap current → prev for crossfade (zero-alloc when prev is pre-allocated)
                std::mem::swap(
                    &mut self.prev_blended_filters_for_crossfade,
                    &mut self.blended_decorrelation_filters,
                );
                // Ensure current filters have correct dimensions (reuse prev's old allocation or allocate)
                let spec_size = self.fft_size / 2 + 1;
                let num_ch = self.num_output_channels;
                if self.blended_decorrelation_filters.len() != num_ch {
                    self.blended_decorrelation_filters =
                        vec![vec![Complex::new(1.0, 0.0); spec_size]; num_ch];
                } else {
                    for ch_filters in &mut self.blended_decorrelation_filters {
                        ch_filters.fill(Complex::new(1.0, 0.0));
                    }
                }
                self.decorrelation_crossfade_remaining = 5;
                self.decorrelation_mode = mode as usize;
                self.generate_decorrelation_filters();
                self.prev_decorrelation_strength = -1.0; // Force reblend
            }
        } else if id == self.param_safety_cap_db {
            let val = value
                .as_float()
                .ok_or_else(|| "safety_cap_db must be a float".to_string())?;
            if val.is_finite() {
                self.safety_cap_db_smoother.set_target(val.clamp(0.0, 3.0));
            }
        }
        // Sub-harmonic synthesis parameters
        else if id == self.param_subharmonic_freq_hz {
            let val = value
                .as_float()
                .ok_or_else(|| "subharmonic_freq_hz must be a float".to_string())?;
            if val.is_finite() {
                self.subharmonic_freq_hz = val.clamp(20.0, 80.0);
                self.recache_subharmonic_coeffs();
            }
        } else if id == self.param_subharmonic_attack_ms {
            let val = value
                .as_float()
                .ok_or_else(|| "subharmonic_attack_ms must be a float".to_string())?;
            if val.is_finite() {
                self.subharmonic_attack_ms = val.clamp(1.0, 100.0);
                self.recache_subharmonic_coeffs();
            }
        } else if id == self.param_subharmonic_release_ms {
            let val = value
                .as_float()
                .ok_or_else(|| "subharmonic_release_ms must be a float".to_string())?;
            if val.is_finite() {
                self.subharmonic_release_ms = val.clamp(10.0, 500.0);
                self.recache_subharmonic_coeffs();
            }
        }
        // Decorrelation parameters
        else if id == self.param_decorrelation_lfo_rate_hz {
            let val = value
                .as_float()
                .ok_or_else(|| "decorrelation_lfo_rate_hz must be a float".to_string())?;
            if val.is_finite() {
                self.decorrelation_lfo_rate_hz = val.clamp(0.01, 1.0);
            }
        } else if id == self.param_velvet_noise_duration_ms {
            let val = value
                .as_float()
                .ok_or_else(|| "velvet_noise_duration_ms must be a float".to_string())?;
            if val.is_finite() {
                self.velvet_noise_duration_ms = val.clamp(10.0, 100.0);
                // Regenerate velvet noise filters with new duration
                if self.decorrelation_mode == 0 {
                    self.generate_velvet_noise_decorrelators();
                }
            }
        } else if id == self.param_velvet_noise_density {
            let val = value
                .as_float()
                .ok_or_else(|| "velvet_noise_density must be a float".to_string())?;
            if val.is_finite() {
                self.velvet_noise_density = val.clamp(500.0, 5000.0);
                // Regenerate velvet noise filters with new density
                if self.decorrelation_mode == 0 {
                    self.generate_velvet_noise_decorrelators();
                }
            }
        }
        // Height channel parameters
        else if id == self.param_height_hf_cap_hz {
            let val = value
                .as_float()
                .ok_or_else(|| "height_hf_cap_hz must be a float".to_string())?;
            if val.is_finite() {
                self.height_hf_cap_hz_smoother
                    .set_target(val.clamp(8000.0, 20000.0));
            }
        } else if id == self.param_height_transient_reduction {
            let val = value
                .as_float()
                .ok_or_else(|| "height_transient_reduction must be a float".to_string())?;
            if val.is_finite() {
                self.height_transient_reduction
                    .set_target(val.clamp(0.0, 1.0));
            }
        } else if id == self.param_height_direct_leak {
            let val = value
                .as_float()
                .ok_or_else(|| "height_direct_leak must be a float".to_string())?;
            if val.is_finite() {
                self.height_direct_leak.set_target(val.clamp(0.0, 0.5));
            }
        }
        // Surround routing parameters
        else if id == self.param_surround_direct_bleed {
            let val = value
                .as_float()
                .ok_or_else(|| "surround_direct_bleed must be a float".to_string())?;
            if val.is_finite() {
                self.surround_direct_bleed.set_target(val.clamp(0.0, 1.0));
            }
        } else if id == self.param_rear_ambient_boost {
            let val = value
                .as_float()
                .ok_or_else(|| "rear_ambient_boost must be a float".to_string())?;
            if val.is_finite() {
                self.rear_ambient_boost.set_target(val.clamp(1.0, 3.0));
            }
        } else if id == self.param_rear_late_reflection {
            let val = value
                .as_float()
                .ok_or_else(|| "rear_late_reflection must be a float".to_string())?;
            if val.is_finite() {
                self.rear_late_reflection.set_target(val.clamp(0.0, 0.5));
            }
        }
        // Ambient parameters
        else if id == self.param_ambient_boost {
            let val = value
                .as_float()
                .ok_or_else(|| "ambient_boost must be a float".to_string())?;
            if val.is_finite() {
                self.ambient_boost.set_target(val.clamp(0.5, 2.0));
            }
        }
        // Dialogue detection parameters
        else if id == self.param_dialogue_weight {
            let val = value
                .as_float()
                .ok_or_else(|| "dialogue_weight must be a float".to_string())?;
            if val.is_finite() {
                self.dialogue_weight.set_target(val.clamp(0.0, 1.0));
            }
        } else if id == self.param_voice_freq_min_hz {
            let val = value
                .as_float()
                .ok_or_else(|| "voice_freq_min_hz must be a float".to_string())?;
            if val.is_finite() {
                self.voice_freq_min_hz = val.clamp(200.0, 800.0);
                self.recache_bin_indices();
            }
        } else if id == self.param_voice_freq_max_hz {
            let val = value
                .as_float()
                .ok_or_else(|| "voice_freq_max_hz must be a float".to_string())?;
            if val.is_finite() {
                self.voice_freq_max_hz = val.clamp(2000.0, 5000.0);
                self.recache_bin_indices();
            }
        } else if id == self.param_dialogue_centroid_weight {
            let val = value
                .as_float()
                .ok_or_else(|| "dialogue_centroid_weight must be a float".to_string())?;
            if val.is_finite() {
                self.dialogue_centroid_weight = val.clamp(0.0, 1.0);
                self.recache_dialogue_weights();
            }
        } else if id == self.param_dialogue_variance_weight {
            let val = value
                .as_float()
                .ok_or_else(|| "dialogue_variance_weight must be a float".to_string())?;
            if val.is_finite() {
                self.dialogue_variance_weight = val.clamp(0.0, 1.0);
                self.recache_dialogue_weights();
            }
        } else if id == self.param_dialogue_coherence_weight {
            let val = value
                .as_float()
                .ok_or_else(|| "dialogue_coherence_weight must be a float".to_string())?;
            if val.is_finite() {
                self.dialogue_coherence_weight = val.clamp(0.0, 1.0);
                self.recache_dialogue_weights();
            }
        }
        // ML vocal detection parameters
        else if id == self.param_enable_ml_detection {
            let enable = value
                .as_bool()
                .ok_or_else(|| "enable_ml_detection must be a boolean".to_string())?;
            self.enable_ml_detection = enable;
            self.try_start_ml_inference();
        } else if id == self.param_ml_model_path {
            let path = value
                .as_string()
                .ok_or_else(|| "ml_model_path must be a string".to_string())?;
            self.ml_model_path = path.to_string();
            if self.enable_ml_detection {
                self.try_start_ml_inference();
            }
        } else if id == self.param_low_latency {
            let enable = value
                .as_bool()
                .ok_or_else(|| "low_latency must be a boolean".to_string())?;
            if enable != self.low_latency {
                self.low_latency = enable;
                let new_fft_size = if enable { 1024 } else { 2048 };
                self.resize_fft(new_fft_size);
            }
        } else if id == self.param_bypass_decorrelation {
            let enable = value
                .as_bool()
                .ok_or_else(|| "bypass_decorrelation must be a boolean".to_string())?;
            // Swap current → prev for crossfade (zero-alloc when prev is pre-allocated)
            std::mem::swap(
                &mut self.prev_blended_filters_for_crossfade,
                &mut self.blended_decorrelation_filters,
            );
            let spec_size = self.fft_size / 2 + 1;
            let num_ch = self.num_output_channels;
            if self.blended_decorrelation_filters.len() != num_ch {
                self.blended_decorrelation_filters =
                    vec![vec![Complex::new(1.0, 0.0); spec_size]; num_ch];
            } else {
                for ch_filters in &mut self.blended_decorrelation_filters {
                    ch_filters.fill(Complex::new(1.0, 0.0));
                }
            }
            self.decorrelation_crossfade_remaining = 5;
            self.bypass_decorrelation = enable;
            self.generate_decorrelation_filters();
            self.prev_decorrelation_strength = -1.0; // Force reblend
        } else if id == self.param_bypass_transient_detection {
            self.bypass_transient_detection = value
                .as_bool()
                .ok_or_else(|| "bypass_transient_detection must be a boolean".to_string())?;
        } else if id == self.param_bypass_all_processing {
            self.bypass_all_processing = value
                .as_bool()
                .ok_or_else(|| "bypass_all_processing must be a boolean".to_string())?;
        } else if id == self.param_frequency_resolution {
            // frequency_resolution changes the ERB band count which resizes per-band state.
            // This is a construction-time parameter — set via from_params(), not at runtime.
            return Err("frequency_resolution is a construction-only parameter (requires plugin rebuild)".to_string());
        } else if id == self.param_multi_source_extraction {
            self.multi_source_extraction = value
                .as_bool()
                .ok_or_else(|| "multi_source_extraction must be a boolean".to_string())?;
        } else if id == self.param_multi_source_threshold {
            let val = value
                .as_float()
                .ok_or_else(|| "multi_source_threshold must be a float".to_string())?;
            if val.is_finite() {
                self.multi_source_threshold = val.clamp(0.05, 0.5);
            }
        } else if id.0 == "binaural_preview" {
            self.binaural_preview = value.as_bool().unwrap_or(false);
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }

        self.rebuild_cached_parameters();
        Ok(())
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
            Some(ParameterValue::Float(self.lfe_cutoff_hz_smoother.target()))
        } else if id == &self.param_stereo_width {
            Some(ParameterValue::Float(self.stereo_width.target()))
        } else if id == &self.param_center_spread {
            Some(ParameterValue::Float(self.center_spread.target()))
        } else if id == &self.param_bandpass_hz {
            Some(ParameterValue::Float(self.bandpass_hz_smoother.target()))
        } else if id == &self.param_enable_subharmonic_synth {
            Some(ParameterValue::Bool(self.enable_subharmonic_synth))
        } else if id == &self.param_subharmonic_gain {
            Some(ParameterValue::Float(self.subharmonic_gain.target()))
        } else if id == &self.param_enable_hr_direct {
            Some(ParameterValue::Bool(self.enable_hr_direct))
        } else if id == &self.param_hr_sharpen {
            Some(ParameterValue::Float(self.hr_sharpen.target()))
        } else if id == &self.param_safety_cap_db {
            Some(ParameterValue::Float(self.safety_cap_db_smoother.target()))
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
            Some(ParameterValue::Float(
                self.height_hf_cap_hz_smoother.target(),
            ))
        } else if id == &self.param_height_transient_reduction {
            Some(ParameterValue::Float(
                self.height_transient_reduction.target(),
            ))
        } else if id == &self.param_height_direct_leak {
            Some(ParameterValue::Float(self.height_direct_leak.target()))
        }
        // Surround routing parameters
        else if id == &self.param_surround_direct_bleed {
            Some(ParameterValue::Float(self.surround_direct_bleed.target()))
        } else if id == &self.param_rear_ambient_boost {
            Some(ParameterValue::Float(self.rear_ambient_boost.target()))
        } else if id == &self.param_rear_late_reflection {
            Some(ParameterValue::Float(self.rear_late_reflection.target()))
        }
        // Ambient parameters
        else if id == &self.param_ambient_boost {
            Some(ParameterValue::Float(self.ambient_boost.target()))
        }
        // Dialogue detection parameters
        else if id == &self.param_dialogue_weight {
            Some(ParameterValue::Float(self.dialogue_weight.target()))
        } else if id == &self.param_voice_freq_min_hz {
            Some(ParameterValue::Float(self.voice_freq_min_hz))
        } else if id == &self.param_voice_freq_max_hz {
            Some(ParameterValue::Float(self.voice_freq_max_hz))
        } else if id == &self.param_dialogue_centroid_weight {
            Some(ParameterValue::Float(self.dialogue_centroid_weight))
        } else if id == &self.param_dialogue_variance_weight {
            Some(ParameterValue::Float(self.dialogue_variance_weight))
        } else if id == &self.param_dialogue_coherence_weight {
            Some(ParameterValue::Float(self.dialogue_coherence_weight))
        }
        // ML vocal detection parameters
        else if id == &self.param_enable_ml_detection {
            Some(ParameterValue::Bool(self.enable_ml_detection))
        } else if id == &self.param_ml_model_path {
            Some(ParameterValue::String(self.ml_model_path.clone()))
        } else if id == &self.param_low_latency {
            Some(ParameterValue::Bool(self.low_latency))
        } else if id == &self.param_bypass_decorrelation {
            Some(ParameterValue::Bool(self.bypass_decorrelation))
        } else if id == &self.param_bypass_transient_detection {
            Some(ParameterValue::Bool(self.bypass_transient_detection))
        } else if id == &self.param_bypass_all_processing {
            Some(ParameterValue::Bool(self.bypass_all_processing))
        } else if id == &self.param_frequency_resolution {
            Some(ParameterValue::String(self.frequency_resolution.clone()))
        } else if id == &self.param_multi_source_extraction {
            Some(ParameterValue::Bool(self.multi_source_extraction))
        } else if id == &self.param_multi_source_threshold {
            Some(ParameterValue::Float(self.multi_source_threshold))
        } else if id.0 == "binaural_preview" {
            Some(ParameterValue::Bool(self.binaural_preview))
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

        // Initialize height spectral flux gate buffers
        self.height_prev_magnitude = vec![0.0; spectrum_size];
        self.height_spectral_flux_smooth = 0.0;
        self.height_flux_gate = vec![0.15; spectrum_size]; // Start at floor value

        // Generate decorrelation filters
        self.generate_decorrelation_filters();

        // Generate per-channel decorrelation filters
        self.generate_per_channel_decorrelation_filters();

        // Pre-allocate blended decorrelation filters to avoid hot-path allocation
        let spectrum_size = self.fft_size / 2 + 1;
        self.blended_decorrelation_filters =
            vec![vec![Complex::new(1.0, 0.0); spectrum_size]; self.num_output_channels];
        self.prev_decorrelation_strength = -1.0; // Force recompute on first process()

        // Precompute LFO depth table (per-bin, depends on sample_rate/fft_size/bandpass/lfe cutoff)
        self.precompute_lfo_depth_table();

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
        self.stereo_width.set_time(time_ms, sample_rate);
        self.center_spread.set_time(time_ms, sample_rate);
        self.ambient_boost.set_time(time_ms, sample_rate);
        self.dialogue_weight.set_time(time_ms, sample_rate);
        self.surround_direct_bleed.set_time(time_ms, sample_rate);
        self.rear_ambient_boost.set_time(time_ms, sample_rate);
        self.rear_late_reflection.set_time(time_ms, sample_rate);
        self.height_direct_leak.set_time(time_ms, sample_rate);
        self.height_transient_reduction
            .set_time(time_ms, sample_rate);
        self.hr_sharpen.set_time(time_ms, sample_rate);
        self.lfe_cutoff_hz_smoother.set_time(time_ms, sample_rate);
        self.bandpass_hz_smoother.set_time(time_ms, sample_rate);
        self.height_hf_cap_hz_smoother
            .set_time(time_ms, sample_rate);
        self.safety_cap_db_smoother.set_time(time_ms, sample_rate);

        // Set FTZ/DAZ CPU flags once at initialization so the processing thread inherits
        // them for all subsequent process() calls. This avoids calling enable_ftz_daz()
        // on every block in the hot path.
        enable_ftz_daz();

        // Cache bin indices that depend on sample_rate and fft_size
        self.recache_bin_indices();

        // Cache sub-harmonic envelope coefficients that depend on sample_rate
        self.recache_subharmonic_coeffs();

        // Initialize MFCC extractor (needed if ML is toggled on later)
        #[cfg(feature = "onnx")]
        {
            self.mfcc_extractor = Some(ml_features::MfccExtractor::new(sample_rate, self.fft_size));
        }

        // Start ML inference thread if enabled
        self.try_start_ml_inference();

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

        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.output_block.fill(0.0);
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.latency_filled = 0;

        // Clear HR input and temp blocks
        self.hr_input_buffer.fill(0.0);
        self.hr_temp_input_block.fill(0.0);
        self.hr_delay_buffer.fill(0.0);
        self.hr_delay_cursor = 0;

        self.hr_output_accumulator.fill(0.0);
        self.hr_output_accumulator_fill = 0;
        self.hr_next_add_position = 0;
        self.hr_output_read_position = 0;

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
        self.doa_angle.fill(0.0);
        self.height_prev_magnitude.fill(0.0);
        self.height_spectral_flux_smooth = 0.0;
        self.height_flux_gate.fill(0.15);
        self.prev_hr_scale = 0.0;
        self.coherence_history_idx = 0;
        for h in &mut self.coherence_history {
            *h = [0.0; 5];
        }

        // Force recomputation of blended decorrelation filters
        self.prev_decorrelation_strength = -1.0;

        // Initialize height mask to floor value to avoid startup ramp artifact
        self.height_band_gains
            .fill(frequency_domain::HEIGHT_MASK_FLOOR);
        self.height_band_gains_prev
            .fill(frequency_domain::HEIGHT_MASK_FLOOR);
        self.height_band_gains_temp.fill(0.0);

        // Reset energy correction smoothing to unity
        self.energy_correction_per_bin.fill(1.0);
        self.energy_correction_temp.fill(1.0);
        self.energy_correction_prev.fill(1.0);

        self.prev_safety_scale = 1.0;

        // Reset MFCC extractor state
        #[cfg(feature = "onnx")]
        if let Some(ref mut extractor) = self.mfcc_extractor {
            extractor.reset();
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let n = context.num_frames;
        // Update smoothers by the number of frames in this block
        self.gain_front_direct.next_n(n);
        self.gain_front_ambient.next_n(n);
        self.gain_rear_ambient.next_n(n);
        self.height_gain.next_n(n);
        self.lfe_gain.next_n(n);
        self.subharmonic_gain.next_n(n);
        self.stereo_width.next_n(n);
        self.center_spread.next_n(n);
        self.ambient_boost.next_n(n);
        self.dialogue_weight.next_n(n);
        self.surround_direct_bleed.next_n(n);
        self.rear_ambient_boost.next_n(n);
        self.rear_late_reflection.next_n(n);
        self.height_direct_leak.next_n(n);
        self.height_transient_reduction.next_n(n);
        self.hr_sharpen.next_n(n);

        // Update frequency/table-generating parameter smoothers and regenerate tables if changed
        {
            let prev_lfe = self.lfe_cutoff_hz;
            let prev_bp = self.bandpass_hz;
            let prev_hf = self.height_hf_cap_hz;
            let prev_sc = self.safety_cap_db;

            let new_lfe = self.lfe_cutoff_hz_smoother.next_n(n);
            let new_bp = self.bandpass_hz_smoother.next_n(n);
            let new_hf = self.height_hf_cap_hz_smoother.next_n(n);
            let new_sc = self.safety_cap_db_smoother.next_n(n);

            if (new_lfe - prev_lfe).abs() > 0.01 {
                self.lfe_cutoff_hz = new_lfe;
                self.update_crossover_gains();
                self.recache_bin_indices();
            }
            if (new_bp - prev_bp).abs() > 0.01 {
                self.bandpass_hz = new_bp;
                self.precompute_height_freq_weights();
                self.recache_bin_indices();
            }
            if (new_hf - prev_hf).abs() > 0.1 {
                self.height_hf_cap_hz = new_hf;
                self.precompute_height_freq_weights();
            }
            if (new_sc - prev_sc).abs() > 0.001 {
                self.safety_cap_db = new_sc;
                self.update_safety_cap_cache();
            }
        }

        // Update HR direct envelope for smooth enable/disable transitions
        {
            let hr_target = if self.enable_hr_direct { 1.0 } else { 0.0 };
            let hr_alpha = if hr_target > self.hr_direct_envelope {
                0.1
            } else {
                0.05
            };
            self.hr_direct_envelope += hr_alpha * (hr_target - self.hr_direct_envelope);
            if self.hr_direct_envelope < 1e-4 {
                self.hr_direct_envelope = 0.0;
            }
        }

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
        let mask = self.output_accumulator_mask;
        let nch = self.num_output_channels;

        while output_pos < context.num_frames {
            // Step 1: Fill input buffer if we have more input
            if input_pos < context.num_frames {
                let samples_to_copy =
                    (input_samples - input_pos * 2).min(self.fft_size * 2 - self.input_buffer_fill);

                if samples_to_copy > 0 {
                    let input_slice = &input[input_pos * 2..input_pos * 2 + samples_to_copy];

                    self.input_buffer
                        [self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                        .copy_from_slice(input_slice);

                    // Also add to HR input buffer if HR is enabled.
                    // First pass input through the delay buffer to temporally align
                    // the HR path with the main path (compensates for the difference
                    // in OLA latency: main_latency - hr_latency).
                    if self.hr_direct_envelope > 0.0 {
                        let delay_temp = &mut self.hr_delay_temp[..samples_to_copy];
                        if self.hr_delay_buffer.is_empty() {
                            delay_temp.copy_from_slice(input_slice);
                        } else {
                            for i in 0..samples_to_copy {
                                delay_temp[i] = self.hr_delay_buffer[self.hr_delay_cursor];
                                self.hr_delay_buffer[self.hr_delay_cursor] = input_slice[i];
                                self.hr_delay_cursor += 1;
                                if self.hr_delay_cursor >= self.hr_delay_buffer.len() {
                                    self.hr_delay_cursor = 0;
                                }
                            }
                        }

                        let mut remaining_hr_samples = samples_to_copy;
                        let mut hr_input_offset = 0;

                        while remaining_hr_samples > 0 {
                            let hr_capacity = self.hr_fft_size * 2 - self.hr_input_buffer_fill;
                            let hr_chunk = remaining_hr_samples.min(hr_capacity);

                            self.hr_input_buffer
                                [self.hr_input_buffer_fill..self.hr_input_buffer_fill + hr_chunk]
                                .copy_from_slice(
                                    &self.hr_delay_temp
                                        [hr_input_offset..hr_input_offset + hr_chunk],
                                );

                            self.hr_input_buffer_fill += hr_chunk;
                            hr_input_offset += hr_chunk;
                            remaining_hr_samples -= hr_chunk;

                            // Process HR block if buffer is full
                            if self.hr_input_buffer_fill >= self.hr_fft_size * 2 {
                                // Copy to temp block
                                self.hr_temp_input_block[..self.hr_fft_size * 2]
                                    .copy_from_slice(&self.hr_input_buffer[..self.hr_fft_size * 2]);

                                // Temporary take
                                let temp_input = std::mem::take(&mut self.hr_temp_input_block);
                                self.process_hr_block(&temp_input);
                                self.hr_temp_input_block = temp_input;

                                // 50% overlap hop: hr_fft_size interleaved samples = hr_fft_size/2 frames
                                let hr_hop = self.hr_fft_size;
                                let remaining = self.hr_input_buffer_fill - hr_hop;
                                self.hr_input_buffer
                                    .copy_within(hr_hop..self.hr_input_buffer_fill, 0);
                                self.hr_input_buffer_fill = remaining;
                            }
                        }
                    }

                    self.input_buffer_fill += samples_to_copy;
                    input_pos += samples_to_copy / 2;
                }
            }

            // Step 2: Process FFT block if we have enough input
            while self.input_buffer_fill >= self.fft_size * 2 {
                // Copy to temp buffer
                self.temp_input_block[..self.fft_size * 2]
                    .copy_from_slice(&self.input_buffer[..self.fft_size * 2]);

                let temp_input = std::mem::take(&mut self.temp_input_block);
                let mut output_block = std::mem::take(&mut self.output_block);
                self.process_fft_block(&temp_input, &mut output_block);

                self.temp_input_block = temp_input;

                // Accumulate to ring buffer (flat interleaved)
                for i in 0..self.fft_size {
                    let write_idx = (self.next_add_position + i) & mask;
                    let acc_base = write_idx * nch;
                    let src_base = i * nch;
                    for ch in 0..nch {
                        self.output_accumulator[acc_base + ch] += output_block[src_base + ch];
                    }
                }

                self.output_block = output_block;

                // Advance positions
                self.next_add_position = (self.next_add_position + self.hop_size) & mask;

                self.output_accumulator_fill += self.hop_size;
                self.latency_filled += self.hop_size;

                // Shift input buffer
                let shift_amount = self.hop_size * 2;
                self.input_buffer
                    .copy_within(shift_amount..self.input_buffer_fill, 0);
                self.input_buffer_fill -= shift_amount;
            }

            // Step 3: Drain available output from ring buffer (flat interleaved)
            let frames_to_drain = self
                .output_accumulator_fill
                .min(context.num_frames - output_pos);

            if frames_to_drain > 0 {
                for i in 0..frames_to_drain {
                    let read_idx = (self.output_read_position + i) & mask;
                    let acc_base = read_idx * nch;
                    let out_base = (output_pos + i) * nch;
                    for ch in 0..nch {
                        output[out_base + ch] = self.output_accumulator[acc_base + ch];
                        // Clear after reading for next overlap-add cycle
                        self.output_accumulator[acc_base + ch] = 0.0;
                    }
                }
                self.output_read_position = (self.output_read_position + frames_to_drain) & mask;
                self.output_accumulator_fill -= frames_to_drain;

                // Drain HR output and mix into the frames we just wrote
                if self.hr_direct_envelope > 0.0 {
                    self.mix_hr_output(&mut output[output_pos * nch..], frames_to_drain);
                } else if self.hr_output_accumulator_fill > 0 {
                    // HR disabled — reset ring buffer state so no stale data lingers
                    self.hr_output_accumulator_fill = 0;
                    self.hr_output_read_position = 0;
                    self.hr_next_add_position = 0;
                }

                output_pos += frames_to_drain;
            } else {
                // Break if no progress is possible
                break;
            }
        }

        // Return actual number of frames produced. DawHost handles silence padding.
        flush_denormals_inplace(output);
        Ok(output_pos)
    }

    fn latency_samples(&self) -> usize {
        self.fft_size
    }
}

#[cfg(test)]
mod test;
