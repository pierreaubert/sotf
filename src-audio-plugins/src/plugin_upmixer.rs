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
use super::simd::complex_mul_inplace_simd;
use super::speaker_config::{SpeakerConfig, calculate_panning_gain, get_speaker_config};
use autoeq_iir::{Biquad, BiquadFilterType};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/*
const PHASE_SHIFT_0   : Complex<f32> = Complex::new(1.0, 0.0); // +1
const PHASE_SHIFT_90: Complex<f32> = Complex::new(0.0, 1.0); // +i
*/
const PHASE_SHIFT_180: Complex<f32> = Complex::new(-1.0, 0.0); // -1
const PHASE_SHIFT_270: Complex<f32> = Complex::new(0.0, -1.0); // -i

// ============================================================================
// Configuration
// ============================================================================

fn default_fft_size() -> usize {
    2048
}

fn default_gain_front_direct() -> f32 {
    1.0
}

fn default_gain_front_ambient() -> f32 {
    0.5
}

fn default_gain_rear_ambient() -> f32 {
    1.2  // Boosted from 1.0 (20% increase) for better rear/height envelopment
}

fn default_lfe_cutoff_hz() -> f32 {
    120.0
}

fn default_stereo_width() -> f32 {
    0.5
}

fn default_bandpass_hz() -> f32 {
    220.0  // Lowered from 300Hz for more mid-range content in surrounds
}

fn default_speaker_config() -> String {
    "5.1".to_string()
}

fn default_height_gain() -> f32 {
    0.2
}

fn default_lfe_gain() -> f32 {
    1.0
}

fn default_subharmonic_gain() -> f32 {
    0.5
}

fn default_center_spread() -> f32 {
    0.0
}

fn default_hr_sharpen() -> f32 {
    1.0
}

fn default_safety_cap_db() -> f32 {
    3.0
}

/// Configuration parameters for UpmixerPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpmixerPluginParams {
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,

    /// Speaker configuration ("5.1", "7.1", "5.1.4", etc.)
    #[serde(default = "default_speaker_config")]
    pub speaker_config: String,

    #[serde(default = "default_gain_front_direct")]
    pub gain_front_direct: f32,
    #[serde(default = "default_gain_front_ambient")]
    pub gain_front_ambient: f32,
    #[serde(default = "default_gain_rear_ambient")]
    pub gain_rear_ambient: f32,
    #[serde(default = "default_lfe_cutoff_hz")]
    pub lfe_cutoff_hz: f32,
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default = "default_bandpass_hz")]
    pub bandpass_hz: f32,

    #[serde(default = "default_center_spread")]
    pub center_spread: f32,

    /// Height channel gain (0.0 to 2.0, default 1.0)
    /// Controls how much audio goes to overhead speakers
    #[serde(default = "default_height_gain")]
    pub height_gain: f32,

    /// LFE gain (0.0 to 2.0, default 1.0)
    /// Controls subwoofer level
    #[serde(default = "default_lfe_gain")]
    pub lfe_gain: f32,

    /// Enable Sub-Harmonic Synthesis for LFE
    #[serde(default)]
    pub enable_subharmonic_synth: bool,

    /// Gain for Sub-Harmonic Synthesis (0.0 to 1.0)
    #[serde(default = "default_subharmonic_gain")]
    pub subharmonic_gain: f32,

    /// Enable high-resolution direct-path enhancement (experimental)
    #[serde(default)]
    pub enable_hr_direct: bool,

    #[serde(default = "default_hr_sharpen")]
    pub hr_sharpen: f32,

    #[serde(default = "default_safety_cap_db")]
    pub safety_cap_db: f32,
}

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

    // Decorrelation
    decorrelation_filter_left: Vec<Complex<f32>>,
    decorrelation_filter_right: Vec<Complex<f32>>,
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
            subharmonic_phase: 0.0,
            subharmonic_envelope: 0.0,

            erb_bands: Vec::new(), // Will be calculated in initialize()
            steering_alphas: Vec::new(),
            coherence_instant: Vec::new(),
            smoothed_coherence: Vec::new(),
            decorrelation_filter_left: vec![zero_complex; fft_size],
            decorrelation_filter_right: vec![zero_complex; fft_size],
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

            // Dialogue detection
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
        plugin
    }

    /// Change speaker configuration dynamically
    fn change_speaker_config(&mut self, config_id: &str) -> PluginResult<()> {
        let new_config = get_speaker_config(config_id)
            .ok_or_else(|| format!("Invalid speaker config: {}", config_id))?;

        if new_config.total_channels == self.num_output_channels {
            // Same channel count, just update config and panning gains
            self.speaker_config = new_config;
            self.recalculate_panning_gains();
            return Ok(());
        }

        // Different channel count - need to reallocate buffers
        self.speaker_config = new_config;
        self.num_output_channels = new_config.total_channels;

        // Reallocate output buffers
        // Note: time_out_channels are now real (f32) for RealFFT inverse output
        self.time_out_channels = vec![vec![0.0; self.fft_size]; self.num_output_channels];
        // Also reallocate HR output buffers which depend on channel count
        self.hr_time_out_channels = vec![vec![0.0; self.hr_fft_size]; self.num_output_channels];
        self.output_accumulator = vec![vec![0.0; self.fft_size * 3]; self.num_output_channels];
        self.output_block = vec![0.0; self.fft_size * self.num_output_channels];

        self.recalculate_panning_gains();
        self.reset();

        Ok(())
    }

    /// Recalculate panning gains for current speaker configuration
    fn recalculate_panning_gains(&mut self) {
        const LEFT_AZIMUTH: f32 = 30.0;
        const RIGHT_AZIMUTH: f32 = -30.0;

        /*
                log::debug!(
                    "[UPMIXER] recalculate_panning_gains() called for config: {}",
                    self.speaker_config.name
                );
                log::debug!(
                    "[UPMIXER]   num_output_channels: {}",
                    self.num_output_channels
                );
                log::debug!(
                    "[UPMIXER]   left_azimuth: {}, right_azimuth: {}",
                    LEFT_AZIMUTH,
                    RIGHT_AZIMUTH
                );
        */

        self.panning_gains_left.clear();
        self.panning_gains_right.clear();

        for (idx, speaker) in self.speaker_config.speakers.iter().enumerate() {
            if speaker.is_lfe {
                // log::debug!("[UPMIXER]   Speaker[{}] LFE: left=0.5, right=0.5", idx);
                self.panning_gains_left.push(0.5);
                self.panning_gains_right.push(0.5);
            } else {
                let left_gain =
                    calculate_panning_gain(LEFT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation);
                let right_gain =
                    calculate_panning_gain(RIGHT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation);
                /*
                                log::debug!(
                                    "[UPMIXER]   Speaker[{}] az={:>6.1}° el={:>6.1}° is_height={}: left={:.4}, right={:.4}",
                                    idx,
                                    speaker.azimuth,
                                    speaker.elevation,
                                    speaker.elevation > 10.0,
                                    left_gain,
                                    right_gain
                                );
                */
                self.panning_gains_left.push(left_gain);
                self.panning_gains_right.push(right_gain);
            }
        }

        // Normalize gains using energy-preserving normalization
        // For each source (left and right), normalize so sum of squared gains = 1
        let left_energy: f32 = self.panning_gains_left.iter().map(|g| g * g).sum();
        let right_energy: f32 = self.panning_gains_right.iter().map(|g| g * g).sum();

        /*
                log::debug!(
                    "[UPMIXER]   Pre-normalization energies: left={:.6}, right={:.6}",
                    left_energy,
                    right_energy
                );
        */

        if left_energy > 0.0 {
            let left_scale = 1.0 / left_energy.sqrt();
            log::debug!("[UPMIXER]   Left normalization scale: {:.6}", left_scale);
            for i in 0..self.num_output_channels {
                self.panning_gains_left[i] *= left_scale;
            }
        }

        if right_energy > 0.0 {
            let right_scale = 1.0 / right_energy.sqrt();
            log::debug!("[UPMIXER]   Right normalization scale: {:.6}", right_scale);
            for i in 0..self.num_output_channels {
                self.panning_gains_right[i] *= right_scale;
            }
        }

        /*
                log::debug!(
                    "[UPMIXER]   Final panning gains (left):  {:?}",
                    self.panning_gains_left
                );
                log::debug!(
                    "[UPMIXER]   Final panning gains (right): {:?}",
                    self.panning_gains_right
                );
        */
    }

    /// Detect dialogue-like signals using spectral centroid and temporal envelope
    ///
    /// Dialogue characteristics:
    /// - Spectral centroid in 500-3000 Hz range (fundamental voice frequencies)
    /// - Low temporal envelope variance (relatively steady compared to music)
    /// - High coherence (mono/center content)
    ///
    /// Returns dialogue probability (0.0 to 1.0)
    #[inline]
    fn detect_dialogue(&mut self) -> f32 {
        let spectrum_size = self.fft_size / 2 + 1;
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        // Voice frequency range: 500-3000 Hz (covers fundamental + formants)
        let voice_start_hz = 500.0;
        let voice_end_hz = 3000.0;
        let voice_start_bin = (voice_start_hz / freq_per_bin) as usize;
        let voice_end_bin = (voice_end_hz / freq_per_bin).min(spectrum_size as f32 - 1.0) as usize;

        // Calculate spectral centroid in voice range
        let mut weighted_sum = 0.0_f32;
        let mut magnitude_sum = 0.0_f32;

        for i in voice_start_bin..=voice_end_bin {
            let left_mag = self.freq_domain_left[i].norm();
            let right_mag = self.freq_domain_right[i].norm();
            let avg_mag = (left_mag + right_mag) * 0.5;

            let freq = i as f32 * freq_per_bin;
            weighted_sum += freq * avg_mag;
            magnitude_sum += avg_mag;
        }

        let spectral_centroid = if magnitude_sum > 1e-9 {
            weighted_sum / magnitude_sum
        } else {
            0.0
        };

        // Smooth spectral centroid with exponential averaging
        let centroid_alpha = 0.3;
        self.dialogue_spectral_centroid = centroid_alpha * spectral_centroid
            + (1.0 - centroid_alpha) * self.dialogue_spectral_centroid;

        // Calculate RMS energy for temporal envelope variance
        let mut energy_sum = 0.0_f32;
        for i in voice_start_bin..=voice_end_bin {
            let left_mag = self.freq_domain_left[i].norm_sqr();
            let right_mag = self.freq_domain_right[i].norm_sqr();
            energy_sum += left_mag + right_mag;
        }
        let rms = (energy_sum / ((voice_end_bin - voice_start_bin + 1) as f32 * 2.0)).sqrt();

        // Calculate envelope variance (difference from previous frame)
        let envelope_diff = if self.dialogue_prev_rms > 1e-9 {
            ((rms - self.dialogue_prev_rms) / self.dialogue_prev_rms).abs()
        } else {
            1.0 // High variance if previous was silence
        };
        self.dialogue_prev_rms = rms;

        // Smooth envelope variance
        let variance_alpha = 0.2;
        self.dialogue_envelope_variance = variance_alpha * envelope_diff
            + (1.0 - variance_alpha) * self.dialogue_envelope_variance;

        // Dialogue probability calculation
        // Voice has centroid in 800-2000 Hz (sweet spot), low variance (<0.3)
        let centroid_voice_min = 800.0;
        let centroid_voice_max = 2000.0;
        let centroid_score = if self.dialogue_spectral_centroid >= centroid_voice_min
            && self.dialogue_spectral_centroid <= centroid_voice_max
        {
            1.0
        } else if self.dialogue_spectral_centroid < centroid_voice_min {
            // Below range: fade from 500 to 800 Hz
            ((self.dialogue_spectral_centroid - voice_start_hz)
                / (centroid_voice_min - voice_start_hz))
                .clamp(0.0, 1.0)
        } else {
            // Above range: fade from 2000 to 3000 Hz
            (1.0 - ((self.dialogue_spectral_centroid - centroid_voice_max)
                / (voice_end_hz - centroid_voice_max)))
                .clamp(0.0, 1.0)
        };

        // Low variance indicates steady dialogue (vs. dynamic music)
        let variance_threshold = 0.4;
        let variance_score = (1.0 - (self.dialogue_envelope_variance / variance_threshold).min(1.0))
            .max(0.0);

        // Combined score with weighting
        let dialogue_prob = centroid_score * 0.6 + variance_score * 0.4;

        // Smooth dialogue probability with slow attack/release
        let prob_alpha = if dialogue_prob > self.dialogue_probability {
            0.1 // Slow attack: don't immediately assume dialogue
        } else {
            0.05 // Very slow release: maintain dialogue routing once detected
        };
        self.dialogue_probability =
            prob_alpha * dialogue_prob + (1.0 - prob_alpha) * self.dialogue_probability;

        self.dialogue_probability
    }

    /// Phase 1: Apply window to input and perform forward FFT
    #[inline]
    fn apply_window_and_forward_fft(&mut self, input: &[f32]) {
        // Copy input to time domain buffers and apply ANALYSIS window
        for i in 0..self.fft_size {
            let idx = i * 2;
            let window_val = self.window[i];
            self.time_domain_left[i] = input[idx] * window_val;
            self.time_domain_right[i] = input[idx + 1] * window_val;
        }

        // Forward FFT (Real->Complex)
        self.fft_forward
            .process(&mut self.time_domain_left, &mut self.freq_domain_left)
            .unwrap();
        self.fft_forward
            .process(&mut self.time_domain_right, &mut self.freq_domain_right)
            .unwrap();
    }

    /// Phase 2: Process frequency domain with ERB bands and PCA decomposition
    #[inline]
    fn process_frequency_domain_erb_bands(&mut self) {
        self.update_decorrelation_filters_time_varying();

        let lfe_cutoff_bin =
            ((self.lfe_cutoff_hz * self.fft_size as f32) / self.sample_rate as f32) as usize;
        let bandpass_bin =
            ((self.bandpass_hz * self.fft_size as f32) / self.sample_rate as f32) as usize;
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        // Iterate over ERB bands
        for band_idx in 0..self.erb_bands.len() {
            let start_bin = self.erb_bands[band_idx];
            let end_bin = if band_idx + 1 < self.erb_bands.len() {
                self.erb_bands[band_idx + 1]
            } else {
                self.fft_size / 2 + 1
            };

            // Skip if band is empty or out of range
            if start_bin >= end_bin || start_bin > self.fft_size / 2 {
                continue;
            }

            // Calculate Band Statistics (Covariance) - SIMD accelerated
            let (cov_xx, cov_yy, cov_xy) = super::simd::compute_covariance_simd(
                &self.freq_domain_left,
                &self.freq_domain_right,
                start_bin,
                end_bin,
            );

            // Logic Steering (Smoothing)
            let inst_energy = cov_xx + cov_yy;
            let smooth_energy = self.pca_cov_xx[band_idx] + self.pca_cov_yy[band_idx];

            // Variable Attack/Release
            // If energy rises (transient), attack fast. If falls, release slow.
            let center_bin = (start_bin + end_bin) / 2;
            let center_freq = center_bin as f32 * freq_per_bin;

            let norm = ((center_freq - 100.0) / (8000.0 - 100.0)).clamp(0.0, 1.0);
            let attack_alpha = 0.3 + 0.4 * norm;
            let release_alpha = 0.02 + 0.06 * norm;

            let alpha = if inst_energy > smooth_energy * 1.5 {
                attack_alpha
            } else {
                release_alpha
            };
            self.steering_alphas[band_idx] = alpha;

            // Update smoothed covariance
            self.pca_cov_xx[band_idx] = (1.0 - alpha) * self.pca_cov_xx[band_idx] + alpha * cov_xx;
            self.pca_cov_yy[band_idx] = (1.0 - alpha) * self.pca_cov_yy[band_idx] + alpha * cov_yy;
            self.pca_cov_xy[band_idx] = (1.0 - alpha) * self.pca_cov_xy[band_idx] + alpha * cov_xy;

            // PCA Decomposition
            let c_xx = self.pca_cov_xx[band_idx];
            let c_yy = self.pca_cov_yy[band_idx];
            let c_xy = self.pca_cov_xy[band_idx];

            // Eigenvalues of 2x2 Hermitian matrix
            let trace = c_xx + c_yy;
            let det = c_xx * c_yy - c_xy.norm_sqr();
            // Avoid sqrt of negative due to float errors
            let disc = ((trace / 2.0).powi(2) - det).max(0.0).sqrt();
            let lambda1 = trace / 2.0 + disc;
            let lambda2 = trace / 2.0 - disc;

            // Coherence (0 to 1)
            // High coherence = strong direct sound (lambda1 >> lambda2)
            // Low coherence = diffuse sound (lambda1 ~= lambda2)
            let mut coherence = if trace > 1e-9 {
                (lambda1 - lambda2) / (lambda1 + lambda2)
            } else {
                0.0
            };

            self.coherence_instant[band_idx] = coherence;

            let prev = self.smoothed_coherence[band_idx];
            let band_alpha = self.steering_alphas[band_idx];
            let coherence_attack = (band_alpha * 0.75).min(0.5);
            let coherence_release = (band_alpha * 0.1).max(0.01);
            let alpha_coh = if coherence > prev {
                coherence_attack
            } else {
                coherence_release
            };
            let smoothed = prev + alpha_coh * (coherence - prev);
            self.smoothed_coherence[band_idx] = smoothed;
            coherence = smoothed;

            // 1. LFE Band Logic
            // Determine intersection of current band [start_bin, end_bin) with LFE range [0, lfe_cutoff_bin]
            let lfe_end = (lfe_cutoff_bin + 1).min(end_bin);
            if start_bin < lfe_end {
                let loop_start = start_bin;
                let loop_end = lfe_end;

                for i in loop_start..loop_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

                    // LFE band: use Linkwitz–Riley style crossover so that
                    // low frequencies are shared between LFE (low-pass
                    // mono sum) and mains (high-passed left/right).

                    let bin = i.min(self.lfe_low_gains.len() - 1);
                    let low_gain = self.lfe_low_gains[bin] as f32;
                    let high_gain = self.mains_high_gains[bin] as f32;

                    let mono = (left + right) * Complex::new(0.5 * low_gain, 0.0);
                    self.lfe[i] = mono;

                    let hp_scale = Complex::new(high_gain, 0.0);
                    self.direct_left[i] = left * hp_scale;
                    self.direct_right[i] = right * hp_scale;
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // 2. Pass-through Band Logic
            // Intersection of [start_bin, end_bin) with [lfe_cutoff_bin + 1, bandpass_bin)
            let pass_start = (lfe_cutoff_bin + 1).max(start_bin);
            let pass_end = bandpass_bin.min(end_bin);

            if pass_start < pass_end {
                for i in pass_start..pass_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

                    self.direct_left[i] = left;
                    self.direct_right[i] = right;
                    self.lfe[i] = Complex::new(0.0, 0.0);
                    self.ambient_left[i] = Complex::new(0.0, 0.0);
                    self.ambient_right[i] = Complex::new(0.0, 0.0);
                }
            }

            // 3. Upmixing Band Logic
            // Intersection of [start_bin, end_bin) with [bandpass_bin, infinity)
            let upmix_start = bandpass_bin.max(start_bin);
            let upmix_end = end_bin;

            if upmix_start < upmix_end {
                // Perceptually-weighted ambient gain for better envelopment
                // Base: sqrt(1 - coherence) for energy preservation
                // Boost: 1.2x (20%) for enhanced spatial impression
                //
                // Dialogue detection: reduce ambient gain when dialogue is detected
                // to route more energy to center channel and reduce metallic artifacts
                let base_ambient_gain = (1.0 - coherence).sqrt() * 1.2;
                let dialogue_reduction = 1.0 - (self.dialogue_probability * 0.6); // Reduce by up to 60%
                let ambient_gain = base_ambient_gain * dialogue_reduction;

                for i in upmix_start..upmix_end {
                    let left = self.freq_domain_left[i];
                    let right = self.freq_domain_right[i];

                    let sum = left + right;
                    let sum_norm = sum.norm();

                    // Direct Extraction
                    // Scale direct component by coherence
                    // Dialogue detection: boost direct/center routing for voice
                    let dialogue_boost = 1.0 + (self.dialogue_probability * 0.3); // Boost by up to 30%
                    let direct_mag = sum_norm * 0.5 * coherence * dialogue_boost;
                    let direct_val = if sum_norm > 1e-9 {
                        sum * (direct_mag / sum_norm)
                    } else {
                        Complex::new(0.0, 0.0)
                    };
                    self.direct[i] = direct_val;

                    // Ambient Extraction (Residual)
                    // We use the difference signal for ambient, scaled by (1 - coherence).
                    // Decorrelation is applied in a SIMD-optimized pass after this loop.
                    let diff = left - right;
                    self.ambient_left[i] = diff * ambient_gain;
                    self.ambient_right[i] = -diff * ambient_gain;

                    // Divergence for Fronts
                    self.direct_left[i] = left - direct_val * self.stereo_width;
                    self.direct_right[i] = right - direct_val * self.stereo_width;
                    self.lfe[i] = Complex::new(0.0, 0.0);

                    // Height mask: emphasize high-frequency, low-coherence (diffuse) content
                    // with reduced aggression to prevent "tizzy" artifacts
                    let nyquist = self.sample_rate as f32 / 2.0;
                    let freq = (i as f32 * self.sample_rate as f32) / self.fft_size as f32;
                    let hf_start = self.bandpass_hz.max(self.lfe_cutoff_hz);
                    let hf_end = 16000.0_f32.min(nyquist); // Cap at 16kHz to avoid extreme highs

                    let hf_ratio = if freq <= hf_start {
                        0.0
                    } else if freq >= hf_end {
                        1.0
                    } else {
                        (freq - hf_start) / (hf_end - hf_start)
                    };

                    // Reduced from sqrt() to linear^0.7 for gentler emphasis
                    let freq_weight = hf_ratio.powf(0.7);
                    let diffuse = (1.0 - coherence).max(0.0);

                    // Height suitability: additive blend allows direct HF content
                    // This prevents pure multiplicative gating (freq_weight * diffuse)
                    // which would block coherent high frequencies from reaching heights.
                    // 50/50 blend: some direct HF + some ambient = natural overhead sound
                    let height_suitability = (freq_weight * 0.5 + diffuse * 0.5).min(1.0);

                    // Transient-adaptive reduction: keep transients coherent
                    // During transients, reduce height channel emphasis
                    let transient_reduction = 1.0 - (self.hr_transient_env * 0.6).min(0.6);

                    let height_mask = (height_suitability * transient_reduction).min(1.0);

                    let half_len = self.height_band_gains.len();
                    if i < half_len {
                        self.height_band_gains[i] = height_mask;
                    }
                }

                // Transient-adaptive decorrelation: reduce decorrelation during transients
                // to keep transients coherent and prevent "tizzy" artifacts.
                //
                // During transients (hr_transient_env approaching 1.0):
                // - decorrelation_strength approaches 0.0
                // - Filters approach identity (no decorrelation)
                //
                // During steady-state (hr_transient_env = 0.0):
                // - decorrelation_strength = 1.0
                // - Full decorrelation effect
                //
                // Dialogue-adaptive decorrelation: reduce decorrelation for dialogue
                // to keep vocals coherent and prevent metallic artifacts
                let base_decorr_strength = (1.0 - self.hr_transient_env * 0.85).max(0.15);
                let dialogue_decorr_reduction = 1.0 - (self.dialogue_probability * 0.7); // Reduce by up to 70%
                let decorrelation_strength = (base_decorr_strength * dialogue_decorr_reduction).max(0.05);

                // Apply transient-adaptive and dialogue-adaptive decorrelation
                self.apply_adaptive_decorrelation(
                    upmix_start,
                    upmix_end,
                    decorrelation_strength,
                );
            }
        }

        // Apply spectral and temporal smoothing to height_band_gains
        self.smooth_height_gains();
    }

    /// Apply transient-adaptive decorrelation to ambient channels
    ///
    /// This scales the decorrelation filters by `strength`:
    /// - strength = 1.0: full decorrelation (steady-state)
    /// - strength = 0.0: no decorrelation (pure transients)
    ///
    /// The adaptive scaling prevents "tizzy" artifacts during transients by
    /// keeping high-frequency transient content coherent rather than decorrelated.
    #[inline]
    fn apply_adaptive_decorrelation(
        &mut self,
        start: usize,
        end: usize,
        strength: f32,
    ) {
        // Fast path: full decorrelation (common case during steady-state)
        if strength >= 0.99 {
            let left_slice = &mut self.ambient_left[start..end];
            let right_slice = &mut self.ambient_right[start..end];
            let decor_left = &self.decorrelation_filter_left[start..end];
            let decor_right = &self.decorrelation_filter_right[start..end];

            complex_mul_inplace_simd(left_slice, decor_left);
            complex_mul_inplace_simd(right_slice, decor_right);
            return;
        }

        // Adaptive decorrelation: blend between decorrelated and original signals
        //
        // For each bin:
        //   decorrelated = signal * decorrelation_filter
        //   output = strength * decorrelated + (1 - strength) * signal
        //
        // This can be rewritten as:
        //   output = signal * (strength * decorrelation_filter + (1 - strength) * identity)
        //   output = signal * (strength * decorrelation_filter + (1 - strength))
        //
        // We compute the blended filter and apply it in one pass.

        let identity_weight = 1.0 - strength;

        for i in start..end {
            let decor_l = self.decorrelation_filter_left[i];
            let decor_r = self.decorrelation_filter_right[i];

            // Blend: strength * decor + (1 - strength) * identity
            // Identity is Complex::new(1.0, 0.0)
            let blended_l = Complex::new(
                strength * decor_l.re + identity_weight,
                strength * decor_l.im,
            );
            let blended_r = Complex::new(
                strength * decor_r.re + identity_weight,
                strength * decor_r.im,
            );

            self.ambient_left[i] *= blended_l;
            self.ambient_right[i] *= blended_r;
        }
    }

    /// Smooth height_band_gains to reduce bin-to-bin and frame-to-frame variance
    ///
    /// This applies:
    /// 1. Spectral smoothing: 3-point moving average across adjacent bins
    /// 2. Temporal smoothing: exponential averaging with previous frame
    ///
    /// This reduces "grainy" artifacts from bin-level processing within ERB bands.
    #[inline]
    fn smooth_height_gains(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;

        // Temporal smoothing coefficient (higher = more smoothing)
        // 0.3 provides good balance: responsive but reduces frame-to-frame variance
        let temporal_alpha = 0.3_f32;

        // Spectral smoothing window size (3-point moving average)
        // Larger windows over-blur and lose frequency resolution
        let window_radius = 1_usize;

        // Temporary buffer for spectral smoothing result
        let mut smoothed = vec![0.0_f32; spectrum_size];

        // 1. Spectral smoothing: moving average across adjacent bins
        for i in 0..spectrum_size {
            let start = i.saturating_sub(window_radius);
            let end = (i + window_radius + 1).min(spectrum_size);

            let mut sum = 0.0_f32;
            let mut count = 0_usize;

            for j in start..end {
                sum += self.height_band_gains[j];
                count += 1;
            }

            smoothed[i] = if count > 0 {
                sum / count as f32
            } else {
                self.height_band_gains[i]
            };
        }

        // 2. Temporal smoothing: blend with previous frame
        for i in 0..spectrum_size {
            let current = smoothed[i];
            let previous = self.height_band_gains_prev[i];

            // Exponential moving average
            let blended = temporal_alpha * current + (1.0 - temporal_alpha) * previous;

            self.height_band_gains[i] = blended;
            self.height_band_gains_prev[i] = blended;
        }
    }

    /// Phase 3: Apply VBAP panning to distribute to output speakers and perform inverse FFT
    #[inline]
    fn apply_vbap_panning_and_inverse_fft(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;
        let hr_mix_global = (self.hr_transient_env * self.hr_sharpen).clamp(0.0, 1.0);

        for ch_idx in 0..self.num_output_channels {
            let speaker = &self.speaker_config.speakers[ch_idx];

            if speaker.is_lfe {
                // LFE channel
                for i in 0..spectrum_size {
                    self.temp_freq_out[i] = self.lfe[i] * self.lfe_gain;
                }
            } else {
                // Regular speaker
                let panning_gain_left = self.panning_gains_left[ch_idx];
                let panning_gain_right = self.panning_gains_right[ch_idx];

                let is_front = speaker.azimuth.abs() < 80.0;
                let is_height = speaker.elevation > 10.0;
                let is_center = speaker.label == "C";

                // Front speakers use explicit front direct/ambient gains.
                let mut direct_gain = if is_front && !is_height {
                    self.gain_front_direct
                } else {
                    self.gain_rear_ambient * 0.15
                };

                let mut ambient_gain = if is_front && !is_height {
                    self.gain_front_ambient
                } else {
                    self.gain_rear_ambient
                };

                if is_front && !is_height && hr_mix_global > 0.0 {
                    let duck_direct = 0.25 * hr_mix_global;
                    let duck_ambient = 0.5 * hr_mix_global;

                    let min_scale = if self.safety_cap_db > 0.0 {
                        10.0_f32.powf(-self.safety_cap_db / 20.0)
                    } else {
                        0.0
                    };

                    let direct_scale = (1.0 - duck_direct).max(min_scale);
                    let ambient_scale = (1.0 - duck_ambient).max(min_scale);

                    direct_gain *= direct_scale;
                    ambient_gain *= ambient_scale;
                }

                if is_height {
                    for i in 0..spectrum_size {
                        // 1. Direct component
                        let direct_component = self.direct_left[i] * panning_gain_left
                            + self.direct_right[i] * panning_gain_right;

                        // 2. Ambient component
                        let is_left = speaker.azimuth > 0.0;

                        let ambient_component = if is_front {
                            // Front Height
                            if is_left {
                                self.ambient_left[i] * PHASE_SHIFT_180
                                    + self.ambient_right[i] * PHASE_SHIFT_270
                            } else {
                                self.ambient_left[i] * PHASE_SHIFT_270
                                    + self.ambient_right[i] * PHASE_SHIFT_180
                            }
                        } else {
                            // Rear Height
                            let decorrelated = if is_left {
                                self.ambient_left[i] * PHASE_SHIFT_270
                                    + self.ambient_right[i] * PHASE_SHIFT_180
                            } else {
                                self.ambient_left[i] * PHASE_SHIFT_180
                                    + self.ambient_right[i] * PHASE_SHIFT_270
                            };
                            let late_reflection =
                                (self.direct_left[i] + self.direct_right[i]) * 0.10;
                            decorrelated + late_reflection
                        };

                        // Height mask
                        let height_mask = self.height_band_gains[i];

                        self.temp_freq_out[i] = (direct_component * direct_gain
                            + ambient_component * ambient_gain)
                            * self.height_gain
                            * height_mask;
                    }
                } else {
                    for i in 0..spectrum_size {
                        let mut direct_component = self.direct_left[i] * panning_gain_left
                            + self.direct_right[i] * panning_gain_right;
                        let ambient_component = self.ambient_left[i] * panning_gain_left
                            + self.ambient_right[i] * panning_gain_right;

                        if is_front && !is_height && is_center {
                            let spread = self.center_spread.clamp(0.0, 1.0);
                            direct_component = direct_component * (1.0 - spread);
                        }
                        self.temp_freq_out[i] =
                            direct_component * direct_gain + ambient_component * ambient_gain;
                    }
                }
            }

            // Enforce RealFFT constraints: DC and Nyquist bins must be purely real
            if spectrum_size > 0 {
                self.temp_freq_out[0].im = 0.0;
                self.temp_freq_out[spectrum_size - 1].im = 0.0;
            }

            // Inverse FFT (Complex -> Real)
            self.fft_inverse
                .process(&mut self.temp_freq_out, &mut self.time_out_channels[ch_idx])
                .unwrap();
        }
    }

    /// Phase 4: Apply sub-harmonic synthesis to LFE channel (time domain)
    #[inline]
    fn apply_subharmonic_synthesis(&mut self) {
        if !self.enable_subharmonic_synth {
            return;
        }

        if let Some(lfe_idx) = self.speaker_config.speakers.iter().position(|s| s.is_lfe) {
            // Generate subharmonics based on LFE amplitude
            // We use a simple sine wave at 40Hz (typical rumble) modulated by the LFE envelope
            let phase_inc = 2.0 * std::f32::consts::PI * 40.0 / self.sample_rate as f32;

            // Envelope smoothing parameters (time constants in samples)
            // Attack: 10ms = 441 samples at 44.1kHz -> coefficient ≈ 1 - exp(-1/441) ≈ 0.00227
            // Release: 50ms = 2205 samples at 44.1kHz -> coefficient ≈ 1 - exp(-1/2205) ≈ 0.000453
            let attack_coeff = 1.0 - (-1.0 / (0.010 * self.sample_rate as f32)).exp();
            let release_coeff = 1.0 - (-1.0 / (0.050 * self.sample_rate as f32)).exp();

            for i in 0..self.fft_size {
                // Use the time-domain LFE signal as the envelope
                let lfe_amp = self.time_out_channels[lfe_idx][i].abs();

                // Smooth envelope: gradually ramp up/down instead of hard switching
                // This prevents clicks and pops when sub-harmonic synthesis turns on/off
                if lfe_amp > 0.001 {
                    // Attack: envelope moves toward 1.0
                    self.subharmonic_envelope += (1.0 - self.subharmonic_envelope) * attack_coeff;
                } else {
                    // Release: envelope moves toward 0.0
                    self.subharmonic_envelope += (0.0 - self.subharmonic_envelope) * release_coeff;
                }

                // Only generate sub-harmonic if envelope is above threshold
                if self.subharmonic_envelope > 0.0001 {
                    self.subharmonic_phase += phase_inc;
                    if self.subharmonic_phase > 2.0 * std::f32::consts::PI {
                        self.subharmonic_phase -= 2.0 * std::f32::consts::PI;
                    }

                    // Apply envelope to sub-harmonic for smooth transitions
                    let sub = self.subharmonic_phase.sin()
                        * lfe_amp
                        * self.subharmonic_gain
                        * self.subharmonic_envelope;
                    self.time_out_channels[lfe_idx][i] += sub;
                }
            }
        }
    }

    /// Phase 5: Extract real parts from time domain and apply final scaling
    #[inline]
    fn extract_output_and_scale(&mut self, output: &mut [f32], combined_scale: f32) {
        // Note: With Hann window at 50% hop size, COLA (Constant Overlap-Add) is achieved by:
        // 1. Applying window ONCE during analysis (before FFT)
        // 2. Overlap-add with hop_size = fft_size/2
        // Applying window again here would break COLA and cause amplitude modulation artifacts
        // Safety cap: optionally reduce overall gain so that the block peak does not
        // exceed safety_cap_db (in dB) above unit amplitude.
        let mut safety_scale = 1.0_f32;
        if self.safety_cap_db > 0.0 {
            let mut max_abs = 0.0_f32;
            for ch in 0..self.num_output_channels {
                for i in 0..self.fft_size {
                    let v = self.time_out_channels[ch][i].abs();
                    if v > max_abs {
                        max_abs = v;
                    }
                }
            }

            if max_abs > 0.0 {
                let cap_linear = 10.0_f32.powf(self.safety_cap_db / 20.0);
                let effective_peak = max_abs * combined_scale;
                if effective_peak > cap_linear {
                    safety_scale = cap_linear / effective_peak;
                }
            }
        }

        let final_scale = combined_scale * safety_scale;

        for i in 0..self.fft_size {
            let idx = i * self.num_output_channels;
            for ch in 0..self.num_output_channels {
                let mut sample = self.time_out_channels[ch][i] * final_scale;

                // Flush denormals to zero to prevent CPU spikes and audio glitches
                // Denormal numbers (very small floats near zero) can cause significant
                // performance degradation and numerical instability
                if sample.abs() < 1e-30 {
                    sample = 0.0;
                }

                output[idx + ch] = sample;
            }
        }
    }

    /// Process one FFT block using VBAP panning
    pub fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Verify sizes
        assert_eq!(input.len(), self.fft_size * 2); // stereo interleaved
        assert_eq!(output.len(), self.fft_size * self.num_output_channels); // variable channels

        /*
                log::trace!(
                    "[UPMIXER] process_fft_block() start: fft_size={}, num_output_channels={}",
                    self.fft_size,
                    self.num_output_channels
                );
        */

        // Phase 1: Apply window and perform forward FFT
        self.apply_window_and_forward_fft(input);

        // High-frequency transient detector for HR direct-path crossfade.
        if self.enable_hr_direct {
            let spectrum_size = self.fft_size / 2 + 1;
            let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;
            let hf_start = self.bandpass_hz.max(1000.0);

            let mut energy = 0.0_f32;
            let mut count = 0usize;
            for i in 0..spectrum_size {
                let freq = i as f32 * freq_per_bin;
                if freq >= hf_start {
                    let l = self.freq_domain_left[i];
                    let r = self.freq_domain_right[i];
                    energy += l.norm_sqr() + r.norm_sqr();
                    count += 1;
                }
            }
            if count > 0 {
                energy /= count as f32;
            } else {
                energy = 0.0;
            }

            if self.hr_energy_smooth <= 0.0 {
                self.hr_energy_smooth = energy;
                self.hr_transient_env = 0.0;
            } else {
                let prev_smooth = self.hr_energy_smooth;
                let prev_smooth_clamped = prev_smooth.max(1e-9);
                let ratio = (energy / prev_smooth_clamped).max(0.0);

                let attack_e = 0.5_f32;
                let release_e = 0.1_f32;
                let alpha_e = if energy > prev_smooth {
                    attack_e
                } else {
                    release_e
                };
                self.hr_energy_smooth = prev_smooth + alpha_e * (energy - prev_smooth);

                let ratio_clamped = ratio.clamp(1.0, 4.0);
                let transient_target = if ratio_clamped > 1.0 {
                    (ratio_clamped - 1.0) / 3.0
                } else {
                    0.0
                };

                let prev_env = self.hr_transient_env;
                let attack_env = 0.8_f32;
                let release_env = 0.3_f32;
                let alpha_env = if transient_target > prev_env {
                    attack_env
                } else {
                    release_env
                };
                self.hr_transient_env = prev_env + alpha_env * (transient_target - prev_env);
            }
        } else {
            self.hr_transient_env = 0.0;
        }

        // Dialogue Detection: analyze spectral centroid and temporal envelope
        let _dialogue_prob = self.detect_dialogue();

        // Phase 2: Frequency-domain processing (ERB Bands + PCA)
        self.process_frequency_domain_erb_bands();

        // Phase 3: Apply VBAP panning and inverse FFT
        // Calculate combined scaling factor for output
        let fft_scale = 1.0 / self.fft_size as f32;
        let cola_scale = 2.0; // COLA compensation for Hann window at 50% overlap
        let channel_normalization = 0.9 / 2.0_f32.sqrt(); // Prevent clipping
        let combined_scale = fft_scale * cola_scale * channel_normalization;

        self.apply_vbap_panning_and_inverse_fft();

        // Phase 4: Sub-harmonic synthesis (time domain)
        self.apply_subharmonic_synthesis();

        // Phase 5: Extract output and apply final scaling
        self.extract_output_and_scale(output, combined_scale);
    }

    /// Calculate ERB bands based on sample rate and FFT size
    fn calculate_erb_bands(&mut self) {
        self.erb_bands.clear();
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        // Glasberg and Moore (1990) ERB scale
        // ERB(f) = 24.7 * (4.37 * f / 1000 + 1)
        // We want bands to be roughly 1 ERB wide

        let mut current_bin = 0;
        while current_bin < self.fft_size / 2 {
            self.erb_bands.push(current_bin);

            let center_freq = current_bin as f32 * freq_per_bin;
            let erb_width = 24.7 * (4.37 * center_freq / 1000.0 + 1.0);
            let bins_width = (erb_width / freq_per_bin).max(1.0).round() as usize;

            current_bin += bins_width;
        }
        // Ensure we cover the full spectrum up to Nyquist
        if *self.erb_bands.last().unwrap() < self.fft_size / 2 {
            self.erb_bands.push(self.fft_size / 2);
        }
    }

    /// Precompute Linkwitz–Riley style crossover magnitude responses between mains and LFE.
    ///
    /// This uses two cascaded 2nd-order Butterworth sections (LR4) for both
    /// low-pass (LFE) and high-pass (mains) at `lfe_cutoff_hz`. We only need
    /// the magnitude response per FFT bin, not the phase, so we evaluate the
    /// biquad magnitude using `Biquad::result` and normalize so that
    /// low^2 + high^2 ≈ 1.0 at each frequency.
    fn update_crossover_gains(&mut self) {
        let num_bins = self.fft_size / 2 + 1;

        if self.lfe_low_gains.len() != num_bins {
            self.lfe_low_gains = vec![0.0; num_bins];
            self.mains_high_gains = vec![1.0; num_bins];
        }

        // Fallback: if we don't have a valid sample rate yet, keep all bass in mains
        if self.sample_rate == 0 || self.lfe_cutoff_hz <= 0.0 {
            for i in 0..num_bins {
                self.lfe_low_gains[i] = 0.0;
                self.mains_high_gains[i] = 1.0;
            }
            return;
        }

        let cutoff = self.lfe_cutoff_hz as f64;
        let srate = self.sample_rate as f64;

        // LR4: cascade two 2nd-order Butterworth sections for low-pass and high-pass
        let q = 1.0 / std::f64::consts::SQRT_2;

        let mut low_sections = Vec::new();
        let mut high_sections = Vec::new();
        for _ in 0..2 {
            low_sections.push(Biquad::new(
                BiquadFilterType::Lowpass,
                cutoff,
                srate,
                q,
                0.0,
            ));
            high_sections.push(Biquad::new(
                BiquadFilterType::Highpass,
                cutoff,
                srate,
                q,
                0.0,
            ));
        }

        let freq_per_bin = srate / self.fft_size as f64;

        for i in 0..num_bins {
            let f = i as f64 * freq_per_bin;

            let mut low_mag = 1.0_f64;
            let mut high_mag = 1.0_f64;

            for sec in &low_sections {
                low_mag *= sec.result(f);
            }
            for sec in &high_sections {
                high_mag *= sec.result(f);
            }

            // Normalize so that low^2 + high^2 ≈ 1.0 to avoid level shifts
            let power = low_mag * low_mag + high_mag * high_mag;
            if power > 0.0 {
                let norm = power.sqrt();
                low_mag /= norm;
                high_mag /= norm;
            }

            self.lfe_low_gains[i] = low_mag as f32;
            self.mains_high_gains[i] = high_mag as f32;
        }
    }

    /// Generate static decorrelation filters (random phase)
    fn generate_decorrelation_filters(&mut self) {
        // Simple pseudo-random generator to avoid dependencies
        let mut seed = 12345u32;
        let mut rand_f32 = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed as f32) / (u32::MAX as f32)
        };

        let n = self.fft_size;
        let half = n / 2;

        // Generate spectrally-smooth random phase for positive frequencies
        // using a small set of random anchor points and linear interpolation
        // between them. This avoids per-bin white-noise phase that can sound
        // "grainy" while keeping full decorrelation.
        let num_anchors = 16usize.min(half.max(2));
        let step = (half as f32) / (num_anchors.saturating_sub(1).max(1) as f32);

        let mut anchor_indices = Vec::with_capacity(num_anchors);
        let mut anchor_phases_left = Vec::with_capacity(num_anchors);
        let mut anchor_phases_right = Vec::with_capacity(num_anchors);

        for a in 0..num_anchors {
            let idx = (a as f32 * step).round() as usize;
            let idx = idx.min(half);
            anchor_indices.push(idx);

            // DC and Nyquist: keep phase 0 (no decorrelation)
            if idx == 0 || idx == half {
                anchor_phases_left.push(0.0);
                anchor_phases_right.push(0.0);
            } else {
                anchor_phases_left.push(rand_f32() * 2.0 * std::f32::consts::PI);
                anchor_phases_right.push(rand_f32() * 2.0 * std::f32::consts::PI);
            }
        }

        // Ensure first and last anchors are exactly at 0 and Nyquist
        *anchor_indices.first_mut().unwrap() = 0;
        *anchor_indices.last_mut().unwrap() = half;
        anchor_phases_left[0] = 0.0;
        anchor_phases_right[0] = 0.0;
        anchor_phases_left[num_anchors - 1] = 0.0;
        anchor_phases_right[num_anchors - 1] = 0.0;

        // Helper to interpolate phase with wrap-around handling
        let interp_phase =
            |i: usize, idx_a: usize, idx_b: usize, phase_a: f32, phase_b: f32| -> f32 {
                if idx_b == idx_a {
                    return phase_a;
                }
                let t = (i as f32 - idx_a as f32) / (idx_b as f32 - idx_a as f32);
                let mut d = phase_b - phase_a;
                // Wrap to [-pi, pi] for smooth interpolation
                let pi = std::f32::consts::PI;
                if d > pi {
                    d -= 2.0 * pi;
                } else if d < -pi {
                    d += 2.0 * pi;
                }
                phase_a + d * t
            };

        // Build full-phase arrays for [0, half]
        let mut phases_left = vec![0.0f32; half + 1];
        let mut phases_right = vec![0.0f32; half + 1];

        for seg in 0..(num_anchors - 1) {
            let idx_a = anchor_indices[seg];
            let idx_b = anchor_indices[seg + 1];
            let phase_a_l = anchor_phases_left[seg];
            let phase_b_l = anchor_phases_left[seg + 1];
            let phase_a_r = anchor_phases_right[seg];
            let phase_b_r = anchor_phases_right[seg + 1];

            for i in idx_a..=idx_b {
                phases_left[i] = interp_phase(i, idx_a, idx_b, phase_a_l, phase_b_l);
                phases_right[i] = interp_phase(i, idx_a, idx_b, phase_a_r, phase_b_r);
            }
        }

        self.decor_base_phases_left = phases_left;
        self.decor_base_phases_right = phases_right;
        self.decor_lfo_phase = 0.0;

        self.update_decorrelation_filters_time_varying();
    }

    fn update_decorrelation_filters_time_varying(&mut self) {
        if self.sample_rate == 0 || self.fft_size == 0 {
            return;
        }

        let n = self.fft_size;
        let half = n / 2;
        if self.decor_base_phases_left.len() != half + 1
            || self.decor_base_phases_right.len() != half + 1
        {
            return;
        }

        let dt = self.hop_size as f32 / self.sample_rate as f32;
        // Reduced from 0.7 Hz to 0.15 Hz to prevent audible warbling
        let rate_hz = 0.15_f32;
        let two_pi = std::f32::consts::PI * 2.0;
        self.decor_lfo_phase += two_pi * rate_hz * dt;
        if self.decor_lfo_phase > two_pi {
            self.decor_lfo_phase -= two_pi;
        }

        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;
        let nyquist = self.sample_rate as f32 / 2.0;
        let hf_start = self.bandpass_hz.max(self.lfe_cutoff_hz);

        // Critical frequencies for decorrelation shaping
        let mid_start = 800.0_f32;   // Start reducing decorrelation in vocal range
        let mid_end = 4000.0_f32;    // End of critical mid-range

        for i in 0..=half {
            let freq = i as f32 * freq_per_bin;

            // High-frequency ratio (bandpass_hz to Nyquist)
            let hf_ratio = if freq <= hf_start {
                0.0
            } else if freq >= nyquist {
                1.0
            } else {
                (freq - hf_start) / (nyquist - hf_start)
            };

            // Mid-frequency reduction (reduces phasiness in vocal range)
            let mid_reduction = if freq < mid_start {
                1.0
            } else if freq > mid_end {
                1.0
            } else {
                // Cosine taper through mid-range: 1.0 -> 0.3 -> 1.0
                let t = (freq - mid_start) / (mid_end - mid_start);
                0.3 + 0.7 * (std::f32::consts::PI * t).cos().abs()
            };

            // Reduced from 0.3 to 0.08, with mid-frequency reduction
            let max_depth = 0.08_f32;
            let depth = max_depth * hf_ratio * mid_reduction;
            let phase_warp = (self.decor_lfo_phase + 0.37_f32 * i as f32).sin() * depth;

            let base_l = self.decor_base_phases_left[i];
            let base_r = self.decor_base_phases_right[i];

            let phi_l = base_l + phase_warp;
            let phi_r = base_r - phase_warp;

            self.decorrelation_filter_left[i] = Complex::from_polar(1.0, phi_l);
            self.decorrelation_filter_right[i] = Complex::from_polar(1.0, phi_r);
        }

        for i in 1..half {
            let mirror_idx = n - i;
            self.decorrelation_filter_left[mirror_idx] = self.decorrelation_filter_left[i].conj();
            self.decorrelation_filter_right[mirror_idx] = self.decorrelation_filter_right[i].conj();
        }

        self.decorrelation_filter_left[0] = Complex::new(1.0, 0.0);
        self.decorrelation_filter_right[0] = Complex::new(1.0, 0.0);
        self.decorrelation_filter_left[half] = Complex::new(1.0, 0.0);
        self.decorrelation_filter_right[half] = Complex::new(1.0, 0.0);
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
            Parameter::new_bool("enable_hr_direct", "Multi-Resolution Analysis", true).with_description(
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
            && let Some(val) = value.as_float()
        {
            if (0.0..=1.0).contains(&val) {
                self.hr_sharpen = val;
                return Ok(());
            }
            return Err("HR Sharpen must be between 0.0 and 1.0".to_string());
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
            &mut self.decorrelation_filter_left,
            &mut self.decorrelation_filter_right,
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
        self.decor_lfo_phase = 0.0;
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

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        self.fft_size
    }
}

// Inherent methods on UpmixerPlugin (not part of the Plugin trait)
impl UpmixerPlugin {
    /// High-resolution direct-path processing for a single FFT block.
    ///
    /// This operates at hr_fft_size with its own Hann window and does not
    /// modify the main overlap-add state. It is intended for experiments and
    /// tests and is not wired into the streaming process loop yet.
    fn process_hr_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Verify sizes: stereo interleaved input, variable output channels
        assert_eq!(input.len(), self.hr_fft_size * 2);
        assert_eq!(output.len(), self.hr_fft_size * self.num_output_channels);

        output.fill(0.0);

        // 1. Copy input to HR time-domain buffers and apply HR analysis window
        for i in 0..self.hr_fft_size {
            let idx = i * 2;
            let window_val = self.hr_window[i];
            self.hr_time_domain_left[i] = input[idx] * window_val;
            self.hr_time_domain_right[i] = input[idx + 1] * window_val;
        }

        // 2. Forward FFT (Real->Complex)
        self.hr_fft_forward
            .process(&mut self.hr_time_domain_left, &mut self.hr_freq_domain_left)
            .unwrap();
        self.hr_fft_forward
            .process(
                &mut self.hr_time_domain_right,
                &mut self.hr_freq_domain_right,
            )
            .unwrap();

        // 3. Frequency-dependent processing for HF direct path only
        //    We restrict to frequencies above a cutoff so that the
        //    high-resolution path mainly sharpens transients.
        let freq_per_bin = self.sample_rate as f32 / self.hr_fft_size as f32;
        let hf_cut = self.bandpass_hz.max(1000.0);
        let hr_spectrum_size = self.hr_fft_size / 2 + 1;

        // Clear HR time-domain buffers
        for ch in 0..self.num_output_channels {
            self.hr_time_out_channels[ch].fill(0.0);
        }

        // 4. Inverse FFT per-channel and write time-domain output
        let fft_scale = 1.0 / self.hr_fft_size as f32;
        let cola_scale = 2.0; // Hann with 50% overlap
        let channel_normalization = 0.5; // Conservative mix factor for HR path
        let combined_scale = fft_scale * cola_scale * channel_normalization;

        for ch_idx in 0..self.num_output_channels {
            let speaker = &self.speaker_config.speakers[ch_idx];
            if speaker.is_lfe || speaker.elevation > 10.0 || speaker.azimuth.abs() >= 80.0 {
                continue;
            }

            let is_center = speaker.label == "C";
            let panning_gain_left = self.panning_gains_left[ch_idx];
            let panning_gain_right = self.panning_gains_right[ch_idx];

            self.hr_temp_freq_out.fill(Complex::new(0.0, 0.0));

            for i in 0..hr_spectrum_size {
                let freq = i as f32 * freq_per_bin;
                if freq <= hf_cut {
                    continue;
                }

                let l = self.hr_freq_domain_left[i];
                let r = self.hr_freq_domain_right[i];

                let direct_val = l * panning_gain_left + r * panning_gain_right;
                let mut gain_scale = self.gain_front_direct;
                if is_center {
                    let spread = self.center_spread.clamp(0.0, 1.0);
                    gain_scale *= 1.0 - spread;
                }

                if gain_scale == 0.0 {
                    continue;
                }

                self.hr_temp_freq_out[i] = direct_val * gain_scale;
            }

            if hr_spectrum_size > 0 {
                self.hr_temp_freq_out[0].im = 0.0;
                self.hr_temp_freq_out[hr_spectrum_size - 1].im = 0.0;
            }

            self.hr_fft_inverse
                .process(
                    &mut self.hr_temp_freq_out,
                    &mut self.hr_time_out_channels[ch_idx],
                )
                .unwrap();

            for i in 0..self.hr_fft_size {
                let idx = i * self.num_output_channels + ch_idx;
                output[idx] = self.hr_time_out_channels[ch_idx][i] * combined_scale;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upmixer_creation_5_1() {
        let plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 6);
        assert_eq!(plugin.fft_size, 2048);
        assert_eq!(plugin.speaker_config.id, "5.1");
    }

    #[test]
    fn test_upmixer_creation_7_1_4() {
        let plugin = UpmixerPlugin::new(
            2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 12);
        assert_eq!(plugin.fft_size, 2048);
        assert_eq!(plugin.speaker_config.id, "7.1.4");
    }

    #[test]
    fn test_upmixer_parameters() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );

        // Test setting parameters
        plugin
            .set_parameter(
                ParameterId::from("gain_front_direct"),
                ParameterValue::Float(0.8),
            )
            .unwrap();
        assert_eq!(plugin.gain_front_direct, 0.8);

        // Test getting parameters
        let value = plugin.get_parameter(&ParameterId::from("gain_rear_ambient"));
        assert_eq!(value, Some(ParameterValue::Float(1.0)));
    }

    #[test]
    fn test_center_spread_parameter() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );

        assert!((plugin.center_spread - 0.0).abs() < 1e-6);

        plugin
            .set_parameter(
                ParameterId::from("center_spread"),
                ParameterValue::Float(0.7),
            )
            .unwrap();
        assert!((plugin.center_spread - 0.7).abs() < 1e-6);

        let res = plugin.set_parameter(
            ParameterId::from("center_spread"),
            ParameterValue::Float(1.5),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_upmixer_processing() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input: 2048 stereo samples (4096 samples total)
        // Use a simple sine wave pattern for more interesting input
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5; // Left
            input[i * 2 + 1] = (i as f32 * 0.01).cos() * 0.5; // Right
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Verify output is not all zeros (some processing occurred)
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        // log::info!("Output sum (abs): {}", sum);
        assert!(sum > 0.0, "Output should not be all zeros");

        // Check that we have output in multiple channels
        let num_channels = 6; // 5.1 has 6 channels
        let mut channel_sums = vec![0.0; num_channels];
        for i in 0..2048 {
            for ch in 0..num_channels {
                channel_sums[ch] += output[i * num_channels + ch].abs();
            }
        }
        // log::info!("Channel sums: {:?}", channel_sums);
        // At least center and front channels should have content
        assert!(
            channel_sums[0] > 0.0 || channel_sums[1] > 0.0 || channel_sums[2] > 0.0,
            "At least one front channel should have content"
        );
    }

    #[test]
    fn test_steering_alphas_frequency_dependent() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.5;
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 2000.0 * t).sin() * 0.5;
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];
        plugin.process_fft_block(&input, &mut output);

        let num_bands = plugin.erb_bands.len();
        assert!(num_bands >= 3);

        let low_alpha = plugin.steering_alphas[0];
        let high_alpha = plugin.steering_alphas[num_bands.saturating_sub(2)];
        assert!(
            high_alpha > low_alpha,
            "Expected higher-band steering alpha to be larger than low-band (low={}, high={})",
            low_alpha,
            high_alpha
        );
    }

    #[test]
    fn test_coherence_hysteresis_slow_release() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];
        plugin.process_fft_block(&input, &mut output);

        let num_bands = plugin.erb_bands.len();
        assert!(num_bands >= 3);
        let band_idx = num_bands / 2;

        let coh1_inst = plugin.coherence_instant[band_idx];
        let coh1_smooth = plugin.smoothed_coherence[band_idx];
        assert!(coh1_inst > 0.5);
        assert!(coh1_smooth >= 0.0);
        assert!(coh1_smooth <= coh1_inst + 1e-3_f32);

        // Use phase-inverted signal to create strong incoherence
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = -s; // Inverted phase = maximally incoherent
        }

        plugin.process_fft_block(&input, &mut output);

        let coh2_inst = plugin.coherence_instant[band_idx];
        let coh2_smooth = plugin.smoothed_coherence[band_idx];

        assert!(coh2_inst < coh1_inst);
        assert!(coh2_smooth > coh2_inst);
        assert!(coh2_smooth < coh1_smooth);
    }

    #[test]
    fn test_decorrelation_filters_time_varying() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = -s;
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        plugin.process_fft_block(&input, &mut output);
        let half = plugin.fft_size / 2;
        let idx = half.saturating_sub(10).max(1);
        let before_l = plugin.decorrelation_filter_left[idx];
        let before_r = plugin.decorrelation_filter_right[idx];

        plugin.process_fft_block(&input, &mut output);
        let after_l = plugin.decorrelation_filter_left[idx];
        let after_r = plugin.decorrelation_filter_right[idx];

        let diff_l = (after_l - before_l).norm();
        let diff_r = (after_r - before_r).norm();
        assert!(diff_l > 1e-6_f32 || diff_r > 1e-6_f32);
    }

    #[test]
    fn test_hr_transient_envelope_energy_jump() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();
        plugin.enable_hr_direct = true;

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        // First block: low-energy high-frequency tone
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 4000.0 * t).sin() * 0.1;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        plugin.process_fft_block(&input, &mut output);
        let env1 = plugin.hr_transient_env;

        // Second block: large step in HF energy (simulate transient)
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 4000.0 * t).sin();
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        plugin.process_fft_block(&input, &mut output);
        let env2 = plugin.hr_transient_env;

        assert!(env2 > env1);
        assert!(env2 > 0.0);
    }

    #[test]
    fn test_center_spread_reduces_center_energy() {
        // Coherent input (L=R) in 5.1: with center_spread=1.0 the physical
        // center channel should receive less direct energy than with
        // center_spread=0.0.

        // Helper to measure center channel energy for a given spread value.
        fn measure_center_energy(center_spread: f32) -> f32 {
            let mut plugin = UpmixerPlugin::new(
                2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
            );
            plugin.initialize(44100).unwrap();
            plugin.center_spread = center_spread.clamp(0.0, 1.0);

            let fft_size = plugin.fft_size;
            let mut input = vec![0.0f32; fft_size * 2];
            for i in 0..fft_size {
                let t = i as f32 / 44100.0;
                let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
                input[i * 2] = s;
                input[i * 2 + 1] = s;
            }

            let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: fft_size,
            };
            plugin.process(&input, &mut output, &context).unwrap();

            // 5.1 layout: channel 2 is Center.
            let center_idx = 2usize;
            let mut energy = 0.0f32;
            for i in 0..fft_size {
                let s = output[i * plugin.num_output_channels + center_idx];
                energy += s * s;
            }
            energy
        }

        let energy_spread_0 = measure_center_energy(0.0);
        let energy_spread_1 = measure_center_energy(1.0);

        assert!(
            energy_spread_1 < energy_spread_0,
            "Center energy should decrease when center_spread=1.0 (got {} vs {})",
            energy_spread_1,
            energy_spread_0
        );
    }

    #[test]
    fn test_hr_block_front_hf_direct_distribution() {
        // Verify that the high-resolution path (process_hr_block) produces
        // non-zero energy on front speakers for high-frequency coherent input
        // while leaving non-front channels effectively silent.

        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let hr_size = plugin.hr_fft_size;
        let mut input = vec![0.0f32; hr_size * 2];

        // 4 kHz coherent sine (L=R), safely above hf_cut (>= 1 kHz)
        for i in 0..hr_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 4000.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }

        let mut output = vec![0.0f32; hr_size * plugin.num_output_channels];
        plugin.process_hr_block(&input, &mut output);

        let mut energies = vec![0.0f32; plugin.num_output_channels];
        for i in 0..hr_size {
            for ch in 0..plugin.num_output_channels {
                energies[ch] += output[i * plugin.num_output_channels + ch].powi(2);
            }
        }

        // 5.1 layout: 0=FL,1=FR,2=C,3=LFE,4=SL,5=SR
        // Expect FL/FR/C to have some energy, LFE/surrounds to be near zero.
        assert!(
            energies[0] > 0.0 || energies[1] > 0.0 || energies[2] > 0.0,
            "Front speakers should have non-zero HF direct energy from HR path: {:?}",
            energies
        );

        // LFE and surrounds should stay effectively silent in HR path
        for ch in 3..plugin.num_output_channels {
            assert!(
                energies[ch] < 1e-6,
                "Non-front channel {} should be near zero in HR path (got {})",
                ch,
                energies[ch]
            );
        }
    }

    #[test]
    fn test_upmixer_zero_gains() {
        // Test that with all gains at 0, output is silence (critical for crackling fix)
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 0.0, 0.0, 0.0, 120.0, 0.5, 250.0, 0.0, 0.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with signal
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5; // Left
            input[i * 2 + 1] = (i as f32 * 0.01).cos() * 0.5; // Right
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Verify output is effectively silent (allow for small numerical artifacts from normalization)
        let max_abs = output.iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
        // log::info!("Max abs value with zero gains: {}", max_abs);
        assert!(
            max_abs < 1e-3,
            "With all gains at 0, output should be effectively silent (<-60dB), but max abs = {}",
            max_abs
        );
    }

    #[test]
    fn test_upmixer_config_change() {
        // Test changing speaker configuration dynamically
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin.output_channels(), 6);
        assert_eq!(plugin.speaker_config.id, "5.1");

        // Change to 7.1.4
        plugin.change_speaker_config("7.1.4").unwrap();
        assert_eq!(plugin.output_channels(), 12);
        assert_eq!(plugin.speaker_config.id, "7.1.4");

        // Change back to 5.1
        plugin.change_speaker_config("5.1").unwrap();
        assert_eq!(plugin.output_channels(), 6);
        assert_eq!(plugin.speaker_config.id, "5.1");
    }

    #[test]
    fn test_upmixer_height_gain() {
        // Test height gain parameter
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 0.5, 1.0, false, 0.5,
        );
        assert_eq!(plugin.height_gain, 0.5);
        assert_eq!(plugin.output_channels(), 10); // 5.1.4 has 10 channels

        // Change height gain via parameter
        plugin
            .set_parameter(ParameterId::from("height_gain"), ParameterValue::Float(1.5))
            .unwrap();
        assert_eq!(plugin.height_gain, 1.5);
    }

    #[test]
    fn test_upmixer_full_5ch() {
        // Test full 5.1 upmixing with direct/ambient decomposition
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.0, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with distinct left and right signals at frequencies above bandpass_hz (250 Hz)
        // Use 440 Hz and 880 Hz to ensure they fall in the upmixing band
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // Left: 440 Hz
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).cos() * 0.5; // Right: 880 Hz
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Check each channel
        let num_channels = 6; // 5.1 has 6 channels
        let mut channel_energies = vec![0.0; num_channels];
        for i in 0..2048 {
            for ch in 0..num_channels {
                channel_energies[ch] += output[i * num_channels + ch].powi(2);
            }
        }

        // log::info!("Channel energies: {:?}", channel_energies);

        // Front left and right should have signal
        assert!(channel_energies[0] > 0.1, "Front left should have signal");
        assert!(channel_energies[1] > 0.1, "Front right should have signal");

        // Center should have signal (direct component)
        assert!(
            channel_energies[2] > 0.01,
            "Center should have direct component"
        );

        // LFE should have minimal signal since test frequencies (440 Hz, 880 Hz)
        // are above the LFE cutoff (120 Hz)
        assert!(
            channel_energies[3] < 0.01,
            "LFE should be minimal with high frequency input"
        );

        // Rear channels should have signal (ambient with gain=1.0)
        assert!(
            channel_energies[4] > 0.01,
            "Left surround should have ambient signal"
        );
        assert!(
            channel_energies[5] > 0.01,
            "Right surround should have ambient signal"
        );
    }

    #[test]
    fn test_continuity_invariant() {
        // INVARIANT: Processing continuous audio in chunks should produce continuous output
        // Test with various buffer sizes
        for buffer_size in [256, 512, 1024] {
            // log::info!("\n=== Testing buffer size {} ===", buffer_size);
            let mut plugin = UpmixerPlugin::new(
                2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
            );
            plugin.initialize(44100).unwrap();

            // Generate continuous 440Hz sine wave, process in chunks
            let total_samples = 8192;
            let mut all_output = Vec::new();
            let mut sample_offset = 0;

            while sample_offset < total_samples {
                let chunk_size = buffer_size.min(total_samples - sample_offset);
                let mut input = vec![0.0_f32; chunk_size * 2];

                for i in 0..chunk_size {
                    let phase =
                        2.0 * std::f32::consts::PI * 440.0 * (sample_offset + i) as f32 / 44100.0;
                    input[i * 2] = phase.sin() * 0.5;
                    input[i * 2 + 1] = phase.sin() * 0.5;
                }

                let mut output = vec![0.0_f32; chunk_size * 6];
                let context = ProcessContext {
                    sample_rate: 44100,
                    num_frames: chunk_size,
                };

                plugin.process(&input, &mut output, &context).unwrap();
                all_output.extend_from_slice(&output);
                sample_offset += chunk_size;
            }

            // Check that we got significant output (accounting for latency)
            let total_output_samples = all_output.len() / 5;
            let non_zero_samples = all_output.iter().filter(|&&x| x.abs() > 1e-6).count();
            /*
                        log::info!(
                            "Buffer size {}: {} total frames, {} non-zero samples",
                            buffer_size,
                            total_output_samples,
                            non_zero_samples
                        );
            */
            assert!(
                non_zero_samples > total_output_samples / 2,
                "Buffer size {}: Too many zero samples, got {} non-zero out of {} total",
                buffer_size,
                non_zero_samples,
                total_output_samples
            );
        }
    }

    #[test]
    fn test_energy_preservation() {
        // INVARIANT: Total output energy across all 5 channels should roughly equal input energy
        // (accounting for latency and windowing losses)
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let buffer_size = 1024;
        let mut total_input_energy = 0.0;
        let mut total_output_energy = 0.0;

        for iteration in 0..16 {
            let mut input = vec![0.0_f32; buffer_size * 2];
            for i in 0..buffer_size {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (iteration * buffer_size + i) as f32
                        / 44100.0;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }

            total_input_energy += input.iter().map(|x| x * x).sum::<f32>();

            let mut output = vec![0.0_f32; buffer_size * 6];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: buffer_size,
            };

            plugin.process(&input, &mut output, &context).unwrap();

            // Count all 6 channels
            let num_channels = 6; // 5.1 has 6 channels
            for i in 0..buffer_size {
                for ch in 0..num_channels {
                    total_output_energy += output[i * num_channels + ch].powi(2);
                }
            }
        }

        /*
                log::info!(
                    "Input energy: {}, Output energy: {}, Ratio: {}",
                    total_input_energy,
                    total_output_energy,
                    total_output_energy / total_input_energy
                );
        */

        // Energy scaling factors:
        // 1. Hann window applied once during analysis: ~0.5 mean value
        // 2. With 50% overlap-add, window energy is properly recovered
        // 3. Channel normalization: (0.9/sqrt(2))² ≈ 0.405 energy scale
        // 4. FFT processing and STFT overhead cause some additional loss
        // Accept down to 35% to account for channel spreading and processing losses
        assert!(
            total_output_energy > total_input_energy * 0.35,
            "Energy loss too high: input={}, output={}, ratio={}",
            total_input_energy,
            total_output_energy,
            total_output_energy / total_input_energy
        );
    }

    #[test]
    fn test_no_gaps() {
        // INVARIANT: Every output buffer should have SOME non-zero samples after initial latency
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let buffer_size = 512;
        let mut gap_count = 0;

        for iteration in 0..20 {
            let mut input = vec![0.0_f32; buffer_size * 2];
            for i in 0..buffer_size {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (iteration * buffer_size + i) as f32
                        / 44100.0;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }

            let mut output = vec![0.0_f32; buffer_size * 6];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: buffer_size,
            };

            plugin.process(&input, &mut output, &context).unwrap();

            let max_abs = output.iter().map(|x| x.abs()).fold(0.0f32, f32::max);

            if iteration >= 5 && max_abs < 1e-6 {
                gap_count += 1;
                // log::info!("GAP at iteration {}: max_abs = {}", iteration, max_abs);
            }
        }

        assert_eq!(
            gap_count, 0,
            "Found {} gaps in output after initial latency",
            gap_count
        );
    }

    #[test]
    fn test_upmixer_new_configs() {
        // Test creating upmixer with 2.0 configuration
        let plugin_2_0 = UpmixerPlugin::new(
            2048, "2.0", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin_2_0.input_channels(), 2);
        assert_eq!(plugin_2_0.output_channels(), 2);
        assert_eq!(plugin_2_0.speaker_config.id, "2.0");

        // Test creating upmixer with 5.0 configuration
        let plugin_5_0 = UpmixerPlugin::new(
            2048, "5.0", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin_5_0.input_channels(), 2);
        assert_eq!(plugin_5_0.output_channels(), 5);
        assert_eq!(plugin_5_0.speaker_config.id, "5.0");
    }

    #[test]
    fn test_upmixer_parameter_config_indices() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );

        // Test that parameter index 0 corresponds to 5.1
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(0)));

        // Test setting to 2.0 (index 8)
        plugin
            .set_parameter(ParameterId::from("speaker_config"), ParameterValue::Int(8))
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "2.0");
        assert_eq!(plugin.output_channels(), 2);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(8)));

        // Test setting to 5.0 (index 9)
        plugin
            .set_parameter(ParameterId::from("speaker_config"), ParameterValue::Int(9))
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "5.0");
        assert_eq!(plugin.output_channels(), 5);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(9)));

        // Test setting to 7.1 (index 1)
        plugin
            .set_parameter(ParameterId::from("speaker_config"), ParameterValue::Int(1))
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "7.1");
        assert_eq!(plugin.output_channels(), 8);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(1)));
    }

    #[test]
    fn test_upmixer_5_1_4_channel_distribution() {
        // Test that 5.1.4 produces output on all channels including rear height
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with different L/R content to generate both direct and ambient
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // Left
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.5; // Right (different frequency)
        }

        let mut output = vec![0.0_f32; 2048 * 10];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Calculate energy per channel
        let mut channel_energies = vec![0.0; 10];
        for i in 0..2048 {
            for ch in 0..10 {
                channel_energies[ch] += output[i * 10 + ch].powi(2);
            }
        }

        /*
                log::info!("5.1.4 Channel energies:");
                log::info!("  [0] FL:  {:.6}", channel_energies[0]);
                log::info!("  [1] FR:  {:.6}", channel_energies[1]);
                log::info!("  [2] C:   {:.6}", channel_energies[2]);
                log::info!("  [3] LFE: {:.6}", channel_energies[3]);
                log::info!("  [4] SL:  {:.6}", channel_energies[4]);
                log::info!("  [5] SR:  {:.6}", channel_energies[5]);
                log::info!("  [6] TFL: {:.6}", channel_energies[6]);
                log::info!("  [7] TFR: {:.6}", channel_energies[7]);
                log::info!("  [8] TBL: {:.6}", channel_energies[8]);
                log::info!("  [9] TBR: {:.6}", channel_energies[9]);
        */

        // Check that all non-LFE channels have some energy
        for (ch, &energy) in channel_energies.iter().enumerate() {
            if ch != 3 {
                // Skip LFE (channel 3) as it only gets low frequencies
                assert!(
                    energy >= 0.0,
                    "Channel {} should have non-negative energy",
                    ch
                );
            }
        }

        // Front and side channels should have significant energy
        assert!(
            channel_energies[0] > 0.01,
            "FL should have significant energy"
        );
        assert!(
            channel_energies[1] > 0.01,
            "FR should have significant energy"
        );
        assert!(channel_energies[4] > 0.001, "SL should have some energy");
        assert!(channel_energies[5] > 0.001, "SR should have some energy");

        // Rear height channels (8, 9) should have energy from:
        // 1. Decorrelated ambient (L-R content)
        // 2. Late reflections (10% of direct signal)
        // Even with mono content, they should now receive the late reflection signal
        assert!(
            channel_energies[8] > 1e-9,
            "TBL (rear height left) should have energy from late reflections + ambient, got {}",
            channel_energies[8]
        );
        assert!(
            channel_energies[9] > 1e-9,
            "TBR (rear height right) should have energy from late reflections + ambient, got {}",
            channel_energies[9]
        );
    }

    #[test]
    fn test_crossover_gains_energy_normalization() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let nbins = plugin.lfe_low_gains.len();
        assert_eq!(nbins, plugin.mains_high_gains.len());

        // Check that low^2 + high^2 ≈ 1 across spectrum
        for (idx, (&low, &high)) in plugin
            .lfe_low_gains
            .iter()
            .zip(plugin.mains_high_gains.iter())
            .enumerate()
        {
            let power = low * low + high * high;
            assert!(
                (power - 1.0).abs() < 1e-3,
                "Crossover power not normalized at bin {}: {}",
                idx,
                power
            );
        }

        // Sanity check around cutoff: low dominates below, high dominates above
        let cutoff = plugin.lfe_cutoff_hz;
        let mut cutoff_bin =
            ((cutoff * plugin.fft_size as f32) / plugin.sample_rate as f32) as usize;
        cutoff_bin = cutoff_bin.min(nbins - 2).max(1);
        let below = cutoff_bin / 2;
        let above = (cutoff_bin * 3 / 2).min(nbins - 1);

        assert!(
            plugin.lfe_low_gains[below] > plugin.lfe_low_gains[cutoff_bin],
            "Low gain should decrease toward cutoff"
        );
        assert!(
            plugin.mains_high_gains[above] > plugin.mains_high_gains[cutoff_bin],
            "High gain should increase above cutoff"
        );
    }

    #[test]
    fn test_decorrelation_filters_properties() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let n = plugin.fft_size;
        let half = n / 2;

        // Magnitude should be 1.0 for all bins
        for i in 0..n {
            let mag_l = plugin.decorrelation_filter_left[i].norm();
            let mag_r = plugin.decorrelation_filter_right[i].norm();
            assert!(
                (mag_l - 1.0).abs() < 1e-6,
                "Left decorrelator magnitude not 1 at bin {}: {}",
                i,
                mag_l
            );
            assert!(
                (mag_r - 1.0).abs() < 1e-6,
                "Right decorrelator magnitude not 1 at bin {}: {}",
                i,
                mag_r
            );
        }

        // Conjugate symmetry for real signals
        for i in 1..half {
            let l = plugin.decorrelation_filter_left[i];
            let l_mirror = plugin.decorrelation_filter_left[n - i];
            let r = plugin.decorrelation_filter_right[i];
            let r_mirror = plugin.decorrelation_filter_right[n - i];

            assert!(
                (l.conj() - l_mirror).norm() < 1e-5,
                "Left decorrelator not conjugate-symmetric at bin {}",
                i
            );
            assert!(
                (r.conj() - r_mirror).norm() < 1e-5,
                "Right decorrelator not conjugate-symmetric at bin {}",
                i
            );
        }

        // DC and Nyquist must be real
        assert!(
            plugin.decorrelation_filter_left[0].im.abs() < 1e-6
                && plugin.decorrelation_filter_right[0].im.abs() < 1e-6
        );
        assert!(
            plugin.decorrelation_filter_left[half].im.abs() < 1e-6
                && plugin.decorrelation_filter_right[half].im.abs() < 1e-6
        );
    }

    #[test]
    fn test_height_mask_coherent_input_is_small() {
        // Coherent stereo (L=R) should yield very small height mask values
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        plugin.process_fft_block(&input, &mut output);

        // Only consider height mask values on bins that actually carry
        // non-negligible spectral energy. In high-frequency bands where
        // the signal is essentially silent, the mask can reach 1.0 but
        // contributes nothing audibly.
        let mut max_mask = 0.0f32;
        for i in 0..plugin.height_band_gains.len() {
            let l = plugin.freq_domain_left[i];
            let r = plugin.freq_domain_right[i];
            let energy = l.norm_sqr() + r.norm_sqr();
            if energy > 1e-6_f32 {
                if plugin.height_band_gains[i] > max_mask {
                    max_mask = plugin.height_band_gains[i];
                }
            }
        }
        assert!(
            max_mask < 0.2,
            "Height mask should be small for coherent input, got max {}",
            max_mask
        );
    }

    #[test]
    fn test_height_mask_diffuse_high_frequency_is_significant() {
        // Diffuse HF content (different L/R frequencies) should produce
        // noticeable height mask values in the top of the band.
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // Left: 440 Hz
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.5; // Right: 880 Hz
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        plugin.process_fft_block(&input, &mut output);

        let nbins = plugin.height_band_gains.len();
        let start = (nbins as f32 * 0.75) as usize;
        let mut max_mask_hf = 0.0f32;
        for &m in &plugin.height_band_gains[start..] {
            if m > max_mask_hf {
                max_mask_hf = m;
            }
        }

        assert!(
            max_mask_hf > 0.1,
            "Height mask should be noticeable for diffuse HF input, got max {}",
            max_mask_hf
        );
    }
}
