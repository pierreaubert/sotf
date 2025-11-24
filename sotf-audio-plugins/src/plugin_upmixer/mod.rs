// ============================================================================
// Upmixer Plugin - Stereo to Multi-Channel Surround
// ============================================================================
//
// This plugin converts stereo (2 channels) to multi-channel surround sound
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

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
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
    hr_hop_size: usize,
    /// Forward FFT planner (high-resolution path)
    hr_fft_forward: Arc<dyn RealToComplex<f32>>,
    /// Inverse FFT planner (high-resolution path)
    hr_fft_inverse: Arc<dyn ComplexToReal<f32>>,

    // Parameters
    param_speaker_config: ParameterId,

    /// Front direct gain (gainFS)
    param_gain_front_direct: ParameterId,
    gain_front_direct: f32,

    /// Front ambient gain (gainFA)
    param_gain_front_ambient: ParameterId,
    gain_front_ambient: f32,

    /// Rear ambient gain (gainRA)
    param_gain_rear_ambient: ParameterId,
    gain_rear_ambient: f32,

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
    height_gain: f32,

    /// LFE gain (0.0 to 2.0)
    param_lfe_gain: ParameterId,
    lfe_gain: f32,

    /// Sub-Harmonic Synthesis
    param_enable_subharmonic_synth: ParameterId,
    enable_subharmonic_synth: bool,
    param_subharmonic_gain: ParameterId,
    subharmonic_gain: f32,

    /// High-resolution direct-path enhancement (multires)
    param_enable_hr_direct: ParameterId,
    enable_hr_direct: bool,
    param_hr_sharpen: ParameterId,
    hr_sharpen: f32,

    /// Safety cap on upmixer output peak (in dB)
    param_safety_cap_db: ParameterId,
    safety_cap_db: f32,

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

    // Decorrelation Mode
    param_decorrelation_mode: ParameterId,
    decorrelation_mode: usize, // 0=Velvet, 1=LFO

    // Decorrelation
    decorrelation_filter_left: Vec<Complex<f32>>,
    decorrelation_filter_right: Vec<Complex<f32>>,

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
    /// Next position to add a HR block in the shared accumulator (reserved)
    hr_next_add_position: usize,

    hr_transient_env: f32,
    hr_energy_smooth: f32,

    // Dialogue Detection State
    /// Smoothed spectral centroid (Hz) for dialogue detection
    dialogue_spectral_centroid: f32,
    /// Smoothed temporal envelope variance for dialogue detection
    dialogue_envelope_variance: f32,
    /// Previous frame RMS energy for envelope variance calculation
    dialogue_prev_rms: f32,
    /// Dialogue probability (0.0 = no dialogue, 1.0 = strong dialogue)
    dialogue_probability: f32,
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
            lfe_cutoff_hz > 0.0 && lfe_cutoff_hz < 200.0,
            "LFE cutoff must be between 0-200 Hz"
        );
        assert!(
            (0.0..=1.0).contains(&stereo_width),
            "Stereo width must be between 0.0-1.0"
        );
        assert!(
            bandpass_hz > lfe_cutoff_hz,
            "Bandpass frequency must be greater than LFE cutoff"
        );

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
        let hr_hop_size = hr_fft_size / 2;
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

        // Output accumulator holds up to 3*fft_size samples per channel
        let output_accumulator = vec![vec![0.0; fft_size * 3]; num_output_channels];

        // Allocate output buffers for each channel
        let time_out_channels = vec![vec![0.0; fft_size]; num_output_channels];

        let mut plugin = Self {
            fft_size,
            hop_size,
            sample_rate: 44100, // Will be updated in initialize()
            speaker_config,
            num_output_channels,

            fft_forward,
            fft_inverse,

            hr_fft_size,
            hr_hop_size,
            hr_fft_forward,
            hr_fft_inverse,

            param_speaker_config: ParameterId::from("speaker_config"),
            param_gain_front_direct: ParameterId::from("gain_front_direct"),
            gain_front_direct,

            param_gain_front_ambient: ParameterId::from("gain_front_ambient"),
            gain_front_ambient,

            param_gain_rear_ambient: ParameterId::from("gain_rear_ambient"),
            gain_rear_ambient,

            param_lfe_cutoff_hz: ParameterId::from("lfe_cutoff_hz"),
            lfe_cutoff_hz,

            param_stereo_width: ParameterId::from("stereo_width"),
            stereo_width,

            param_center_spread: ParameterId::from("center_spread"),
            center_spread: default_center_spread(),

            param_bandpass_hz: ParameterId::from("bandpass_hz"),
            bandpass_hz,

            param_height_gain: ParameterId::from("height_gain"),
            height_gain,

            param_lfe_gain: ParameterId::from("lfe_gain"),
            lfe_gain,

            param_enable_subharmonic_synth: ParameterId::from("enable_subharmonic_synth"),
            enable_subharmonic_synth,
            param_subharmonic_gain: ParameterId::from("subharmonic_gain"),
            subharmonic_gain,

            param_enable_hr_direct: ParameterId::from("enable_hr_direct"),
            enable_hr_direct: true, // Enable by default for multi-resolution analysis
            param_hr_sharpen: ParameterId::from("hr_sharpen"),
            hr_sharpen: 1.0,
            param_safety_cap_db: ParameterId::from("safety_cap_db"),
            safety_cap_db: default_safety_cap_db(),
            param_decorrelation_mode: ParameterId::from("decorrelation_mode"),
            decorrelation_mode: 0, // Default to Velvet Noise

            subharmonic_phase: 0.0,
            subharmonic_envelope: 0.0,

            erb_bands: Vec::new(), // Will be calculated in initialize()
            steering_alphas: Vec::new(),
            coherence_instant: Vec::new(),
            smoothed_coherence: Vec::new(),
            decorrelation_filter_left: vec![zero_complex; spectrum_size],
            decorrelation_filter_right: vec![zero_complex; spectrum_size],
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
            hr_next_add_position: 0,

            hr_transient_env: 0.0,
            hr_energy_smooth: 0.0,

            dialogue_spectral_centroid: 0.0,
            dialogue_envelope_variance: 0.0,
            dialogue_prev_rms: 0.0,
            dialogue_probability: 0.0,
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
        plugin.decorrelation_mode = params.decorrelation_mode;
        plugin
    }
}

impl Plugin for UpmixerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: format!("Stereo to {} Upmixer", self.speaker_config.name),
            version: "2.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: format!(
                "Converts stereo to {} using FFT-based Direct/Ambient decomposition and VBAP panning",
                self.speaker_config.name
            ),
        }
    }

    fn input_channels(&self) -> usize {
        2 // Stereo
    }

    fn output_channels(&self) -> usize {
        self.num_output_channels // Variable based on configuration
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_int("speaker_config", "Configuration", 0, 0, 9).with_description(
                "Speaker configuration index.
0=5.1 (default), 1=7.1, 2=5.1.2, 3=5.1.4,
4=7.1.2, 5=7.1.4, 6=9.1.4, 7=9.1.6,
8=2.0, 9=5.0.
Controls output layout and number of channels.",
            ),
            Parameter::new_float("gain_front_direct", "Front Direct Gain", 1.0, 0.0, 2.0)
                .with_description(
                    "Front direct gain for non-height front speakers.
Range: 0.0-2.0, default 1.0.
Higher values make the front image more focused and dry;
lower values rely more on ambient and surround energy.",
                ),
            Parameter::new_float("gain_front_ambient", "Front Ambient Gain", 0.5, 0.0, 2.0)
                .with_description(
                    "Decorrelated ambient gain routed to front speakers.
Range: 0.0-2.0, default 0.5.
Increase to widen and enliven the front stage;
decrease for a more center-focused, direct front.",
                ),
            Parameter::new_float("gain_rear_ambient", "Rear Ambient Gain", 1.0, 0.0, 2.0)
                .with_description(
                    "Ambient gain for surround and rear channels.
Range: 0.0-2.0, default 1.0.
Use <1.0 for subtle ambience, >1.0 for a more enveloping surround field.",
                ),
            Parameter::new_float("height_gain", "Height Gain", 1.0, 0.0, 2.0).with_description(
                "Gain for height/overhead channels (elevation > 0).
Range: 0.0-2.0, default 1.0.
0.0 disables height channels; higher values raise the contribution
of height speakers relative to the bed layer.",
            ),
            Parameter::new_float("lfe_gain", "LFE Gain", 1.0, 0.0, 2.0).with_description(
                "Gain for LFE/subwoofer channel.
Range: 0.0-2.0, default 1.0.
Controls overall subwoofer level after the mains/LFE crossover.",
            ),
            Parameter::new_float("lfe_cutoff_hz", "LFE Cutoff (Hz)", 120.0, 40.0, 200.0)
                .with_description(
                    "Linkwitz-Riley crossover frequency between mains and LFE.
Range: 40-200 Hz, default 120 Hz.
Lower values keep more bass in mains; higher values route
more low-frequency energy into the subwoofer.",
                ),
            Parameter::new_float("stereo_width", "Stereo Width", 0.5, 0.0, 1.0).with_description(
                "Controls front stereo width for the direct component.
Range: 0.0-1.0, default 0.5.
0.0 keeps L/R wide; 1.0 collapses toward mono/center;
intermediate values balance width and center focus.",
            ),
            Parameter::new_float("center_spread", "Center Spread", 0.0, 0.0, 1.0).with_description(
                "Controls how much direct energy is focused in the physical center vs L/R.
Range: 0.0-1.0, default 0.0.
0.0 sends coherent center energy to the C speaker;
1.0 moves it into a phantom center across L/R.",
            ),
            Parameter::new_float("bandpass_hz", "Upmix Crossover (Hz)", 250.0, 200.0, 1000.0)
                .with_description(
                    "Frequency above which upmixing to surrounds/height is applied.
Range: 200-1000 Hz, default 250 Hz.
Below this frequency content stays mainly in fronts + LFE;
above it participates in the direct/ambient upmix.",
                ),
            Parameter::new_bool("enable_subharmonic_synth", "Sub-Harmonic Synth", false)
                .with_description(
                    "Enables optional sub-harmonic synthesis on the LFE.
Default: off. When enabled, a low-frequency tone is added to the
subwoofer, driven by the LFE envelope for extra rumble.",
                ),
            Parameter::new_float("subharmonic_gain", "Sub-Harmonic Gain", 0.5, 0.0, 1.0)
                .with_description(
                    "Gain for synthesized sub-harmonics when enabled.
Range: 0.0-1.0, default 0.5.
Controls how loud the synthesized low-frequency component is
relative to the original LFE signal.",
                ),
            Parameter::new_bool("enable_hr_direct", "Multi-Resolution Analysis", true)
                .with_description(
                    "Enables multi-resolution analysis for optimal time/frequency resolution.
Default: ON. Uses short FFT (512 samples) for transients and long FFT (2048) for ambient.
Adaptively blends based on transient detection for sharper attacks and smooth ambience.",
                ),
            Parameter::new_float("hr_sharpen", "HR Sharpen", 1.0, 0.0, 1.0).with_description(
                "Depth control for the high-resolution direct path.
Range: 0.0-1.0, default 1.0.
0.0 effectively disables the HR contribution even if enabled;
1.0 applies the full transient-driven HR emphasis and ducking
of the main front field.",
            ),
            Parameter::new_float("safety_cap_db", "Safety Cap (dB)", 3.0, 0.0, 12.0)
                .with_description(
                    "Peak safety cap for the upmixer output.
Range: 0.0-12.0 dB, default 3.0 dB.
If a block's peak level after upmixing would exceed this value
above unity, the block is scaled down to stay within the cap.",
                ),
            Parameter::new_int("decorrelation_mode", "Decorrelation Mode", 0, 0, 1)
                .with_description(
                    "Mode for ambient decorrelation.
0 = Velvet Noise (Static, smooth, no artifacts) - Default
1 = LFO Phase (Dynamic, subtle motion, may have metallic artifacts)",
                ),
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
                self.gain_front_direct = gain;
                return Ok(());
            }
        } else if id == self.param_gain_front_ambient {
            if let Some(gain) = value.as_float() {
                self.gain_front_ambient = gain;
                return Ok(());
            }
        } else if id == self.param_gain_rear_ambient
            && let Some(gain) = value.as_float()
        {
            self.gain_rear_ambient = gain;
            return Ok(());
        } else if id == self.param_height_gain
            && let Some(gain) = value.as_float()
        {
            if (0.0..=2.0).contains(&gain) {
                self.height_gain = gain;
                return Ok(());
            }
            return Err("Height gain must be between 0.0 and 2.0".to_string());
        } else if id == self.param_lfe_gain
            && let Some(gain) = value.as_float()
        {
            if (0.0..=2.0).contains(&gain) {
                self.lfe_gain = gain;
                return Ok(());
            }
            return Err("LFE gain must be between 0.0 and 2.0".to_string());
        } else if id == self.param_lfe_cutoff_hz
            && let Some(cutoff) = value.as_float()
        {
            if cutoff > 0.0 && cutoff < 200.0 && cutoff < self.bandpass_hz {
                self.lfe_cutoff_hz = cutoff;
                self.update_crossover_gains();
                return Ok(());
            }
            return Err("LFE cutoff must be 0-200 Hz and less than bandpass frequency".to_string());
        } else if id == self.param_stereo_width
            && let Some(width) = value.as_float()
        {
            if (0.0..=1.0).contains(&width) {
                self.stereo_width = width;
                return Ok(());
            }
            return Err("Stereo width must be between 0.0 and 1.0".to_string());
        } else if id == self.param_center_spread
            && let Some(spread) = value.as_float()
        {
            if (0.0..=1.0).contains(&spread) {
                self.center_spread = spread;
                return Ok(());
            }
            return Err("Center spread must be between 0.0 and 1.0".to_string());
        } else if id == self.param_bandpass_hz
            && let Some(freq) = value.as_float()
        {
            if freq > self.lfe_cutoff_hz {
                self.bandpass_hz = freq;
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
                self.subharmonic_gain = gain;
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
            if (0.0..=12.0).contains(&val) {
                self.safety_cap_db = val;
                return Ok(());
            }
            return Err("Safety cap must be between 0.0 and 12.0 dB".to_string());
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
            Some(ParameterValue::Float(self.gain_front_direct))
        } else if id == &self.param_gain_front_ambient {
            Some(ParameterValue::Float(self.gain_front_ambient))
        } else if id == &self.param_gain_rear_ambient {
            Some(ParameterValue::Float(self.gain_rear_ambient))
        } else if id == &self.param_height_gain {
            Some(ParameterValue::Float(self.height_gain))
        } else if id == &self.param_lfe_gain {
            Some(ParameterValue::Float(self.lfe_gain))
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
            Some(ParameterValue::Float(self.subharmonic_gain))
        } else if id == &self.param_enable_hr_direct {
            Some(ParameterValue::Bool(self.enable_hr_direct))
        } else if id == &self.param_hr_sharpen {
            Some(ParameterValue::Float(self.hr_sharpen))
        } else if id == &self.param_safety_cap_db {
            Some(ParameterValue::Float(self.safety_cap_db))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
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

        // Generate decorrelation filters
        self.generate_decorrelation_filters();

        // Precompute LR4 crossover gains for mains/LFE split
        self.update_crossover_gains();

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
        self.output_block.fill(0.0);

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

        // Clear height mask; it will be recomputed in process_fft_block
        self.height_band_gains.fill(0.0);
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify input size
        let input_samples = context.num_frames * 2; // stereo
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

        /*
                log::debug!(
                    "[UPMIXER] process() called: input={} frames, output={} frames",
                    context.num_frames,
                    context.num_frames
                );
                log::debug!(
                    "[UPMIXER] Initial state: input_buffer_fill={}, output_accumulator_fill={}, next_add_pos={}",
                    self.input_buffer_fill,
                    self.output_accumulator_fill,
                    self.next_add_position
                );
        */

        // Sanity check for threading issues
        if self.next_add_position > self.fft_size * 3 {
            log::info!(
                "[UPMIXER] WARNING: Corrupted state detected! next_add_pos={} exceeds buffer size {}",
                self.next_add_position,
                self.fft_size * 3
            );
            log::debug!("[UPMIXER] This could indicate a threading issue. Resetting state.");
            self.reset();
        }

        // Initialize output buffer to zero (critical to prevent crackling!)
        output.fill(0.0);

        let mut input_pos = 0;
        let mut output_pos = 0;

        // Main processing loop: interleave input filling, FFT processing, and output draining
        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > 1000 {
                log::error!("[UPMIXER] ERROR: Infinite loop detected after 1000 iterations!");
                log::info!(
                    "[UPMIXER] State: input_pos={}/{}, output_pos={}/{}",
                    input_pos / 2,
                    input.len() / 2,
                    output_pos / self.num_output_channels,
                    output.len() / 5
                );
                log::info!(
                    "[UPMIXER] input_buffer_fill={}, output_accumulator_fill={}, next_add_pos={}",
                    self.input_buffer_fill,
                    self.output_accumulator_fill,
                    self.next_add_position
                );
                break;
            }
            // Step 1: Drain output accumulator if we have data and space
            let frames_available = (output.len() - output_pos) / self.num_output_channels;
            let frames_to_drain = self.output_accumulator_fill.min(frames_available);

            if frames_to_drain > 0 {
                // log::debug!(
                //     "[UPMIXER] Iter {}: DRAIN {} frames (accum_fill={}, frames_avail={})",
                //     iteration,
                //     frames_to_drain,
                //     self.output_accumulator_fill,
                //     frames_available
                // );

                // Copy samples to output
                for i in 0..frames_to_drain {
                    for ch in 0..self.num_output_channels {
                        output[output_pos + i * self.num_output_channels + ch] =
                            self.output_accumulator[ch][i];
                    }
                }
                output_pos += frames_to_drain * self.num_output_channels;

                // Shift accumulator
                for ch in 0..self.num_output_channels {
                    self.output_accumulator[ch]
                        .copy_within(frames_to_drain..self.output_accumulator_fill, 0);
                    // Clear the tail
                    for i in (self.output_accumulator_fill - frames_to_drain)
                        ..self.output_accumulator_fill
                    {
                        self.output_accumulator[ch][i] = 0.0;
                    }
                }
                self.output_accumulator_fill -= frames_to_drain;

                // Update next add position (subtract drained amount)
                self.next_add_position = self.next_add_position.saturating_sub(frames_to_drain);

                // Reset position if accumulator is empty
                if self.output_accumulator_fill == 0 {
                    self.next_add_position = 0;
                }

                // log::debug!(
                //     "[UPMIXER] After drain: accum_fill={}, next_add_pos={}, output_pos={}",
                //     self.output_accumulator_fill,
                //     self.next_add_position,
                //     output_pos / self.num_output_channels
                // );
            }

            // Step 2: Process FFT block if we have input and accumulator space
            // Ensure accumulator won't overflow (need space for fft_size samples)
            let can_process_input = self.input_buffer_fill >= self.fft_size * 2;
            let can_process_space = self.next_add_position + self.fft_size <= self.fft_size * 3;

            if can_process_input && can_process_space {
                // log::debug!(
                //     "[UPMIXER] Iter {}: PROCESS FFT (input_buf_fill={}/{}, next_add_pos={}, space_ok={})",
                //     iteration,
                //     self.input_buffer_fill / 2,
                //     self.fft_size,
                //     self.next_add_position,
                //     can_process_space
                // );

                // Copy to temp buffer
                self.temp_input_block[..self.fft_size * 2]
                    .copy_from_slice(&self.input_buffer[..self.fft_size * 2]);

                // Process FFT block (low-resolution path)
                let temp_input = std::mem::take(&mut self.temp_input_block);
                let mut output_block = std::mem::take(&mut self.output_block);
                self.process_fft_block(&temp_input, &mut output_block);

                // Optional high-resolution direct-path enhancement
                if self.enable_hr_direct && self.gain_front_direct > 0.0 {
                    // Take a centered HR window from the current input block
                    let center = (self.fft_size - self.hr_fft_size) / 2;
                    let start = center * 2; // stereo interleaved
                    let end = start + self.hr_fft_size * 2;

                    if end <= temp_input.len() {
                        let hr_input = &temp_input[start..end];
                        let mut hr_output =
                            vec![0.0_f32; self.hr_fft_size * self.num_output_channels];
                        self.process_hr_block(hr_input, &mut hr_output);

                        // Overlay HR contribution onto the low-res output block
                        let hr_mix = (self.hr_transient_env * self.hr_sharpen).clamp(0.0, 1.0);
                        if hr_mix > 0.0 {
                            for i in 0..self.hr_fft_size {
                                let dst_idx = (center + i) * self.num_output_channels;
                                let src_idx = i * self.num_output_channels;
                                for ch in 0..self.num_output_channels {
                                    output_block[dst_idx + ch] += hr_output[src_idx + ch] * hr_mix;
                                }
                            }
                        }
                    }
                }

                self.temp_input_block = temp_input;

                // Accumulate output (overlap-add) at next_add_position
                for i in 0..self.fft_size {
                    for ch in 0..self.num_output_channels {
                        self.output_accumulator[ch][self.next_add_position + i] +=
                            output_block[i * self.num_output_channels + ch];
                    }
                }

                // Update fill level and next add position
                if self.output_accumulator_fill == 0 {
                    // First block: fills from 0 to fft_size
                    self.output_accumulator_fill = self.fft_size;
                    self.next_add_position = self.hop_size;
                } else {
                    // Subsequent blocks: add hop_size more samples, next block starts hop_size later
                    self.output_accumulator_fill += self.hop_size;
                    self.next_add_position += self.hop_size;
                }

                self.output_block = output_block;

                // Shift input buffer by hop_size (50% overlap)
                let shift_amount = self.hop_size * 2; // stereo
                self.input_buffer
                    .copy_within(shift_amount..self.fft_size * 2, 0);
                self.input_buffer_fill -= shift_amount;

                // log::debug!(
                //     "[UPMIXER] After FFT: accum_fill={}, next_add_pos={}, input_buf_fill={}",
                //     self.output_accumulator_fill,
                //     self.next_add_position,
                //     self.input_buffer_fill / 2
                // );

                continue; // Process more blocks if possible
            } else if !can_process_input || !can_process_space {
                // log::debug!(
                //     "[UPMIXER] Iter {}: SKIP FFT (can_process_input={}, can_process_space={})",
                //     iteration,
                //     can_process_input,
                //     can_process_space
                // );
            }

            // Step 3: Fill input buffer if we have more input
            if input_pos < input.len() {
                let samples_to_copy =
                    (input.len() - input_pos).min(self.fft_size * 2 - self.input_buffer_fill);

                // log::debug!(
                //     "[UPMIXER] Iter {}: FILL {} samples (input_pos={}/{}, input_buf_fill={})",
                //     iteration,
                //     samples_to_copy / 2,
                //     input_pos / 2,
                //     input.len() / 2,
                //     self.input_buffer_fill / 2
                // );

                self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                    .copy_from_slice(&input[input_pos..input_pos + samples_to_copy]);

                self.input_buffer_fill += samples_to_copy;
                input_pos += samples_to_copy;

                // log::debug!(
                //     "[UPMIXER] After fill: input_buf_fill={}, input_pos={}",
                //     self.input_buffer_fill / 2,
                //     input_pos / 2
                // );

                continue; // Try processing again
            }

            // No more work to do - exit loop
            // Exit when: output buffer is full OR (no more input AND can't process AND nothing to drain)
            let cant_process = self.input_buffer_fill < self.fft_size * 2
                || self.next_add_position + self.fft_size > self.fft_size * 3;
            let no_data_to_drain = self.output_accumulator_fill == 0;
            let no_space_to_drain = (output.len() - output_pos) / self.num_output_channels == 0;

            // log::debug!(
            //     "[UPMIXER] Iter {}: CHECK EXIT - no_more_input={}, cant_process={}, no_data={}, no_space={}",
            //     iteration,
            //     input_pos >= input.len(),
            //     cant_process,
            //     no_data_to_drain,
            //     no_space_to_drain
            // );

            // Exit if output buffer is full (most important - prevents deadlock)
            if no_space_to_drain {
                // log::debug!("[UPMIXER] EXITING LOOP: output buffer full");
                break;
            }

            // Exit if no more input and can't process and nothing to drain
            if input_pos >= input.len() && cant_process && no_data_to_drain {
                // log::debug!("[UPMIXER] EXITING LOOP: no more work");
                break;
            }
        }

        // log::debug!("[UPMIXER] Loop finished after {} iterations", iteration);
        // log::debug!(
        //     "[UPMIXER] Final: output_pos={}/{}, accum_fill={}",
        //     output_pos / self.num_output_channels,
        //     output.len() / self.num_output_channels,
        //     self.output_accumulator_fill
        // );

        // Final drain of any remaining output
        let frames_available = (output.len() - output_pos) / self.num_output_channels;
        let frames_to_drain = self.output_accumulator_fill.min(frames_available);

        if frames_to_drain > 0 {
            // log::debug!(
            //     "[UPMIXER] FINAL DRAIN: {} frames (accum_fill={}, frames_avail={})",
            //     frames_to_drain,
            //     self.output_accumulator_fill,
            //     frames_available
            // );

            for i in 0..frames_to_drain {
                for ch in 0..self.num_output_channels {
                    output[output_pos + i * self.num_output_channels + ch] =
                        self.output_accumulator[ch][i];
                }
            }
            // output_pos += frames_to_drain * self.num_output_channels;

            for ch in 0..self.num_output_channels {
                self.output_accumulator[ch]
                    .copy_within(frames_to_drain..self.output_accumulator_fill, 0);
                for i in
                    (self.output_accumulator_fill - frames_to_drain)..self.output_accumulator_fill
                {
                    self.output_accumulator[ch][i] = 0.0;
                }
            }
            self.output_accumulator_fill -= frames_to_drain;

            // Update next add position
            self.next_add_position = self.next_add_position.saturating_sub(frames_to_drain);

            // Reset position if accumulator is empty
            if self.output_accumulator_fill == 0 {
                self.next_add_position = 0;
            }

            // log::debug!(
            //     "[UPMIXER] After final drain: accum_fill={}, next_add_pos={}, total_output={}",
            //     self.output_accumulator_fill,
            //     self.next_add_position,
            //     output_pos / self.num_output_channels
            // );
        }

        // log::debug!(
        //     "[UPMIXER] process() complete: returned {} frames\n",
        //     output_pos / self.num_output_channels
        // );

        // TEMPORARY: Hard clipping protection to prevent high level outputs
        // This is a safety measure while we debug which feature is causing the issue
        let threshold = 1.0; // 0dB
        let mut clipped_samples = 0;
        let mut max_value = 0.0_f32;

        for sample in output.iter_mut() {
            let abs_val = sample.abs();
            if abs_val > max_value {
                max_value = abs_val;
            }

            if abs_val > threshold {
                clipped_samples += 1;
                *sample = sample.signum() * threshold;
            }
        }

        // Log warning if clipping occurred
        if clipped_samples > 0 {
            log::warn!(
                "[UPMIXER] CLIPPING PROTECTION: {} samples clipped, max value was {:.2} dB",
                clipped_samples,
                20.0 * max_value.log10()
            );
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        self.fft_size
    }
}

#[cfg(test)]
mod test;
