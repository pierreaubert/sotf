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

use super::param_specs::denoiser::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::any::Any;
use std::sync::Arc;

mod config;
mod fft;
mod masking;
mod mcra;
mod noise_profile;
mod polyphonic;
mod transient;
mod wiener;

pub use config::DenoiserPluginParams;

// ============================================================================
// Exposed Data Structure
// ============================================================================

/// Data exposed by the denoiser for monitoring
#[derive(Debug, Clone)]
pub struct DenoiserData {
    /// Estimated noise floor per frequency band (in dB)
    /// Averaged across channels, downsampled to ~30 bands for display
    pub noise_floor_db: Vec<f32>,

    /// Current SNR estimate per frequency band (in dB)
    pub snr_db: Vec<f32>,

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

    // Decision-Directed SNR parameters
    param_dd_enabled: ParameterId,
    dd_enabled: bool,
    param_dd_alpha: ParameterId,
    dd_alpha: f32,
    prev_power: Vec<Vec<f32>>, // [channels][spectrum_size] previous frame power

    // Psychoacoustic masking
    param_psychoacoustic_masking: ParameterId,
    psychoacoustic_masking: bool,
    bark_map: Vec<f32>,             // [spectrum_size] frequency-to-Bark mapping
    masking_threshold: Vec<f32>,    // [spectrum_size] scratch for masking thresholds
    masking_signal_power: Vec<f32>, // [spectrum_size] scratch for signal power

    // Noise profile capture
    param_learn_noise: ParameterId,
    param_use_captured_profile: ParameterId,
    param_clear_profile: ParameterId,
    use_captured_profile: bool,
    noise_profile: Option<Vec<Vec<f32>>>,  // [channels][spectrum_size]
    learning_accumulator: Vec<Vec<f32>>,   // [channels][spectrum_size]
    learning_frames_count: usize,
    learning_frames_target: usize,
    is_learning: bool,

    // Pre-computed coefficients
    attack_coeff: f32,
    release_coeff: f32,
    floor_linear: f32,

    // Hann window
    window: Vec<f32>,

    // Processing buffers (per-channel)
    time_domain: Vec<Vec<f32>>,          // [channels][fft_size]
    freq_domain: Vec<Vec<Complex<f32>>>, // [channels][spectrum_size]

    // MCRA state (per-channel, per-bin)
    noise_psd: Vec<Vec<f32>>,       // Estimated noise power spectrum
    smoothed_psd: Vec<Vec<f32>>,    // Smoothed signal PSD (S_tmp)
    min_psd: Vec<Vec<f32>>,         // Minimum PSD tracker (S_min)
    speech_presence: Vec<Vec<f32>>, // Speech presence probability (p)
    frame_counter: Vec<usize>,      // Per-channel frame count

    // Wiener filter state
    gain: Vec<Vec<f32>>,          // Current Wiener gains per bin
    smoothed_gain: Vec<Vec<f32>>, // Temporally smoothed gains

    // Frequency smoothing scratch buffer
    freq_smooth_temp: Vec<f32>, // [spectrum_size] scratch for smoothing across bins

    // Overlap-add buffers
    input_buffer: Vec<f32>, // Interleaved input accumulator
    input_buffer_fill: usize,
    temp_input_block: Vec<f32>,        // Pre-allocated block for FFT input
    output_accumulator: Vec<Vec<f32>>, // Per-channel output overlap-add
    output_accumulator_fill: usize,
    next_add_position: usize,

    // Output time-domain buffers
    time_out_channels: Vec<Vec<f32>>,

    // MCRA parameters
    mcra_alpha_s: f32,
    mcra_alpha_p: f32,
    mcra_l: usize,
    mcra_delta: f32,

    // Transient Suppressor
    transient_suppressor: transient::TransientSuppressor,

    // Data exposure for UI
    avg_reduction_db: f32,
    learning_active: bool,
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

        // Generate Hann window
        let window = Self::generate_hann_window(fft_size);

        // Allocate buffers
        let time_domain = vec![vec![0.0_f32; fft_size]; channels];
        let freq_domain = vec![vec![Complex::new(0.0, 0.0); spectrum_size]; channels];

        // MCRA state
        let noise_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let smoothed_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let min_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let speech_presence = vec![vec![0.0_f32; spectrum_size]; channels];
        let frame_counter = vec![0_usize; channels];

        // Wiener filter state
        let gain = vec![vec![1.0_f32; spectrum_size]; channels];
        let smoothed_gain = vec![vec![1.0_f32; spectrum_size]; channels];

        // Overlap-add buffers
        // Input buffer needs to hold fft_size samples
        let input_buffer = vec![0.0_f32; fft_size * channels * 2];
        // Output accumulator needs extra space for overlap-add (4x fft_size)
        let output_accumulator = vec![vec![0.0_f32; fft_size * 4]; channels];
        let time_out_channels = vec![vec![0.0_f32; fft_size]; channels];

        Self {
            channels,
            fft_size,
            hop_size,
            sample_rate: 44100, // Updated in initialize()
            spectrum_size,

            fft_forward,
            fft_inverse,

            param_reduction_db: ParameterId::from("reduction_db"),
            reduction_db: REDUCTION_DB_DEFAULT,

            param_floor_db: ParameterId::from("floor_db"),
            floor_db: FLOOR_DB_DEFAULT,

            param_smoothing: ParameterId::from("smoothing"),
            smoothing: SMOOTHING_DEFAULT,

            param_attack_ms: ParameterId::from("attack_ms"),
            attack_ms: ATTACK_MS_DEFAULT,

            param_release_ms: ParameterId::from("release_ms"),
            release_ms: RELEASE_MS_DEFAULT,

            param_low_latency: ParameterId::from("low_latency"),
            low_latency,

            param_polyphonic_detection: ParameterId::from("polyphonic_detection"),
            polyphonic_detection: POLYPHONIC_DETECTION_DEFAULT,

            param_crack_sensitivity: ParameterId::from("crack_sensitivity"),
            crack_sensitivity: 10.0,

            param_dd_enabled: ParameterId::from("dd_enabled"),
            dd_enabled: DD_ENABLED_DEFAULT,
            param_dd_alpha: ParameterId::from("dd_alpha"),
            dd_alpha: DD_ALPHA_DEFAULT,
            prev_power: vec![vec![0.0_f32; spectrum_size]; channels],

            param_psychoacoustic_masking: ParameterId::from("psychoacoustic_masking"),
            psychoacoustic_masking: PSYCHOACOUSTIC_MASKING_DEFAULT,
            bark_map: vec![0.0_f32; spectrum_size],
            masking_threshold: vec![0.0_f32; spectrum_size],
            masking_signal_power: vec![0.0_f32; spectrum_size],

            param_learn_noise: ParameterId::from("learn_noise"),
            param_use_captured_profile: ParameterId::from("use_captured_profile"),
            param_clear_profile: ParameterId::from("clear_profile"),
            use_captured_profile: USE_CAPTURED_PROFILE_DEFAULT,
            noise_profile: None,
            learning_accumulator: vec![vec![0.0_f32; spectrum_size]; channels],
            learning_frames_count: 0,
            learning_frames_target: LEARN_FRAMES,
            is_learning: false,

            attack_coeff: 0.0,
            release_coeff: 0.0,
            floor_linear: 10.0_f32.powf(FLOOR_DB_DEFAULT / 20.0),

            window,

            time_domain,
            freq_domain,

            noise_psd,
            smoothed_psd,
            min_psd,
            speech_presence,
            frame_counter,

            gain,
            smoothed_gain,

            freq_smooth_temp: vec![0.0_f32; spectrum_size],

            input_buffer,
            input_buffer_fill: 0,
            temp_input_block: vec![0.0_f32; fft_size * channels],
            output_accumulator,
            output_accumulator_fill: 0,
            next_add_position: 0,

            time_out_channels,

            mcra_alpha_s: MCRA_ALPHA_S_DEFAULT,
            mcra_alpha_p: MCRA_ALPHA_P_DEFAULT,
            mcra_l: MCRA_L_DEFAULT,
            mcra_delta: MCRA_DELTA_DEFAULT,

            transient_suppressor: transient::TransientSuppressor::new(channels),

            avg_reduction_db: 0.0,
            learning_active: true,
        }
    }

    /// Create a new denoiser plugin from configuration parameters
    pub fn from_params(channels: usize, params: DenoiserPluginParams) -> Self {
        let mut plugin = Self::new(channels, params.low_latency);

        plugin.reduction_db = params
            .reduction_db
            .clamp(REDUCTION_DB_MIN, REDUCTION_DB_MAX);
        plugin.floor_db = params.floor_db.clamp(FLOOR_DB_MIN, FLOOR_DB_MAX);
        plugin.smoothing = params.smoothing.clamp(SMOOTHING_MIN, SMOOTHING_MAX);
        plugin.attack_ms = params.attack_ms.clamp(ATTACK_MS_MIN, ATTACK_MS_MAX);
        plugin.release_ms = params.release_ms.clamp(RELEASE_MS_MIN, RELEASE_MS_MAX);
        plugin.polyphonic_detection = params.polyphonic_detection;
        plugin.crack_sensitivity = params.crack_sensitivity.max(1.0);
        plugin
            .transient_suppressor
            .set_sensitivity(plugin.crack_sensitivity);

        plugin.mcra_alpha_s = params.mcra_alpha_s;
        plugin.mcra_alpha_p = params.mcra_alpha_p;
        plugin.mcra_l = params.mcra_l.max(1);
        plugin.mcra_delta = params.mcra_delta;

        plugin.dd_enabled = params.dd_enabled;
        plugin.dd_alpha = params.dd_alpha.clamp(DD_ALPHA_MIN, DD_ALPHA_MAX);
        plugin.psychoacoustic_masking = params.psychoacoustic_masking;
        plugin.use_captured_profile = params.use_captured_profile;

        plugin.floor_linear = 10.0_f32.powf(plugin.floor_db / 20.0);

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

        // Phase 2: MCRA noise estimation
        for ch in 0..self.channels {
            if self.is_initializing(ch) {
                // Bootstrap noise estimate from first frames
                self.initialize_mcra_from_frame(ch);
            }
            self.update_mcra(ch);
        }

        // Phase 2b: Noise profile learning (if active)
        if self.is_learning {
            self.accumulate_noise_frame();
        }

        // Phase 3: Calculate Gains (Wiener or Polyphonic)
        if self.polyphonic_detection {
            self.calculate_polyphonic_gains();
        } else {
            self.calculate_wiener_gains();
        }

        // Phase 4: Apply gains and inverse FFT
        self.apply_gains_and_inverse_fft();

        // Phase 5: Overlap-add to output accumulator
        self.overlap_add_to_accumulator();
    }

    /// Add processed block to output accumulator using overlap-add
    fn overlap_add_to_accumulator(&mut self) {
        // FFT scaling: 1/fft_size
        // COLA scaling: 2.0 for Hann window at 50% overlap
        let combined_scale = 2.0 / self.fft_size as f32;

        for ch in 0..self.channels {
            let accum = &mut self.output_accumulator[ch];
            let time_out = &self.time_out_channels[ch];

            // Determine valid range to avoid bounds checks in the inner loop
            let start_pos = self.next_add_position;
            let end_pos = (start_pos + self.fft_size).min(accum.len());
            let write_len = end_pos.saturating_sub(start_pos);

            // Vectorizable loop: strict bounds derived above allow compiler to optimize
            for i in 0..write_len {
                let sample = time_out[i] * combined_scale;
                // Flush denormals
                let sample = if sample.abs() < 1e-30 { 0.0 } else { sample };
                accum[start_pos + i] += sample;
            }
        }

        // Advance position by hop_size for next block
        self.next_add_position += self.hop_size;

        // Update fill level
        self.output_accumulator_fill =
            (self.output_accumulator_fill + self.hop_size).min(self.output_accumulator[0].len());
    }

    /// Drain available samples from output accumulator to output buffer
    fn drain_output(&mut self, output: &mut [f32], num_frames: usize) -> usize {
        let _samples_needed = num_frames * self.channels;
        let samples_available = self.next_add_position.saturating_sub(self.hop_size);

        let samples_to_drain = samples_available.min(num_frames);

        for frame in 0..samples_to_drain {
            for ch in 0..self.channels {
                let out_idx = frame * self.channels + ch;
                if out_idx < output.len() {
                    output[out_idx] = self.output_accumulator[ch][frame];
                }
            }
        }

        // Shift accumulator
        if samples_to_drain > 0 {
            for ch in 0..self.channels {
                self.output_accumulator[ch].copy_within(samples_to_drain.., 0);
                // Clear the shifted region
                let clear_start = self.output_accumulator[ch].len() - samples_to_drain;
                self.output_accumulator[ch][clear_start..].fill(0.0);
            }
            self.next_add_position -= samples_to_drain;
            self.output_accumulator_fill = self
                .output_accumulator_fill
                .saturating_sub(samples_to_drain);
        }

        samples_to_drain
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
        vec![
            Parameter::new_float(
                "reduction_db",
                "Reduction",
                REDUCTION_DB_DEFAULT,
                REDUCTION_DB_MIN,
                REDUCTION_DB_MAX,
            )
            .with_description("Noise reduction strength (dB)")
            .with_group("General")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "floor_db",
                "Floor",
                FLOOR_DB_DEFAULT,
                FLOOR_DB_MIN,
                FLOOR_DB_MAX,
            )
            .with_description("Minimum gain floor to prevent musical noise (dB)")
            .with_group("General")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "smoothing",
                "Smoothing",
                SMOOTHING_DEFAULT,
                SMOOTHING_MIN,
                SMOOTHING_MAX,
            )
            .with_description("Temporal smoothing factor")
            .with_group("Timing")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "attack_ms",
                "Attack",
                ATTACK_MS_DEFAULT,
                ATTACK_MS_MIN,
                ATTACK_MS_MAX,
            )
            .with_description("Attack time for gain changes (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "release_ms",
                "Release",
                RELEASE_MS_DEFAULT,
                RELEASE_MS_MIN,
                RELEASE_MS_MAX,
            )
            .with_description("Release time for gain changes (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("low_latency", "Low Latency", LOW_LATENCY_DEFAULT)
                .with_description("Use smaller FFT for lower latency (requires reinit)")
                .with_group("Performance")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool(
                "polyphonic_detection",
                "Polyphonic Detection",
                POLYPHONIC_DETECTION_DEFAULT,
            )
            .with_description("Enable polyphonic note detection mode (gates non-tonal content)")
            .with_group("Detection")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float("crack_sensitivity", "Crack Sens.", 10.0, 1.0, 100.0)
                .with_description("Sensitivity of transient suppressor (higher = less sensitive)")
                .with_group("Detection")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "psychoacoustic_masking",
                "Psychoacoustic Masking",
                PSYCHOACOUSTIC_MASKING_DEFAULT,
            )
            .with_description("Skip denoising for perceptually masked noise bins")
            .with_group("Processing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("dd_enabled", "DD SNR", DD_ENABLED_DEFAULT)
                .with_description("Enable Decision-Directed SNR estimation (Ephraim-Malah)")
                .with_group("Processing")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "dd_alpha",
                "DD Alpha",
                DD_ALPHA_DEFAULT,
                DD_ALPHA_MIN,
                DD_ALPHA_MAX,
            )
            .with_description("Decision-directed smoothing factor (higher = more smoothing)")
            .with_group("Processing")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("learn_noise", "Learn Noise", false)
                .with_description("Start capturing noise profile from current audio")
                .with_group("Noise Profile")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "use_captured_profile",
                "Use Profile",
                USE_CAPTURED_PROFILE_DEFAULT,
            )
            .with_description("Use captured noise profile instead of live estimation")
            .with_group("Noise Profile")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("clear_profile", "Clear Profile", false)
                .with_description("Clear the captured noise profile")
                .with_group("Noise Profile")
                .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_reduction_db {
            self.reduction_db = value
                .as_float()
                .ok_or("Invalid reduction_db value")?
                .clamp(REDUCTION_DB_MIN, REDUCTION_DB_MAX);
        } else if id == self.param_floor_db {
            self.floor_db = value
                .as_float()
                .ok_or("Invalid floor_db value")?
                .clamp(FLOOR_DB_MIN, FLOOR_DB_MAX);
            self.floor_linear = 10.0_f32.powf(self.floor_db / 20.0);
        } else if id == self.param_smoothing {
            self.smoothing = value
                .as_float()
                .ok_or("Invalid smoothing value")?
                .clamp(SMOOTHING_MIN, SMOOTHING_MAX);
        } else if id == self.param_attack_ms {
            self.attack_ms = value
                .as_float()
                .ok_or("Invalid attack_ms value")?
                .clamp(ATTACK_MS_MIN, ATTACK_MS_MAX);
            self.update_envelope_coefficients();
        } else if id == self.param_release_ms {
            self.release_ms = value
                .as_float()
                .ok_or("Invalid release_ms value")?
                .clamp(RELEASE_MS_MIN, RELEASE_MS_MAX);
            self.update_envelope_coefficients();
        } else if id == self.param_low_latency {
            // Note: Changing low_latency requires reinitializing the plugin
            // This is typically not done at runtime
            self.low_latency = value.as_bool().ok_or("Invalid low_latency value")?;
        } else if id == self.param_polyphonic_detection {
            self.polyphonic_detection = value
                .as_bool()
                .ok_or("Invalid polyphonic_detection value")?;
        } else if id == self.param_crack_sensitivity {
            self.crack_sensitivity = value
                .as_float()
                .ok_or("Invalid crack_sensitivity value")?
                .max(1.0);
            self.transient_suppressor
                .set_sensitivity(self.crack_sensitivity);
        } else if id == self.param_psychoacoustic_masking {
            self.psychoacoustic_masking = value
                .as_bool()
                .ok_or("Invalid psychoacoustic_masking value")?;
        } else if id == self.param_dd_enabled {
            self.dd_enabled = value.as_bool().ok_or("Invalid dd_enabled value")?;
        } else if id == self.param_dd_alpha {
            self.dd_alpha = value
                .as_float()
                .ok_or("Invalid dd_alpha value")?
                .clamp(DD_ALPHA_MIN, DD_ALPHA_MAX);
        } else if id == self.param_learn_noise {
            let trigger = value.as_bool().ok_or("Invalid learn_noise value")?;
            if trigger {
                self.start_learning();
            }
        } else if id == self.param_use_captured_profile {
            self.use_captured_profile = value
                .as_bool()
                .ok_or("Invalid use_captured_profile value")?;
        } else if id == self.param_clear_profile {
            let trigger = value.as_bool().ok_or("Invalid clear_profile value")?;
            if trigger {
                self.clear_noise_profile();
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
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
        } else if id == &self.param_psychoacoustic_masking {
            Some(ParameterValue::Bool(self.psychoacoustic_masking))
        } else if id == &self.param_dd_enabled {
            Some(ParameterValue::Bool(self.dd_enabled))
        } else if id == &self.param_dd_alpha {
            Some(ParameterValue::Float(self.dd_alpha))
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

        // Reset buffers
        self.input_buffer.fill(0.0);
        self.input_buffer_fill = 0;
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;

        self.avg_reduction_db = 0.0;
        self.learning_active = true;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        // Pre-process: Time-domain transient suppression (de-clicking)
        self.transient_suppressor.process(buffer);

        let num_frames = context.num_frames;
        let total_samples = num_frames * self.channels;

        // Accumulate input
        let space_available = self.input_buffer.len() - self.input_buffer_fill;
        let samples_to_copy = total_samples.min(space_available);
        self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
            .copy_from_slice(&buffer[..samples_to_copy]);
        self.input_buffer_fill += samples_to_copy;

        // Process complete FFT blocks
        let block_samples = self.fft_size * self.channels;
        while self.input_buffer_fill >= block_samples {
            self.process_fft_block();
        }

        // Drain output to buffer
        let frames_output = self.drain_output(buffer, num_frames);

        // If we couldn't fill all output, zero the rest (initial latency)
        if frames_output < num_frames {
            let zero_start = frames_output * self.channels;
            buffer[zero_start..total_samples].fill(0.0);
        }

        Ok(frames_output)
    }

    fn latency_samples(&self) -> usize {
        // Latency is fft_size due to overlap-add buffering
        self.fft_size
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(Arc::new(DenoiserData {
            noise_floor_db: self.get_noise_floor_db(),
            snr_db: self.get_snr_db(),
            avg_reduction_db: self.avg_reduction_db,
            learning_active: self.learning_active,
            is_learning_noise: self.is_learning,
            has_captured_profile: self.noise_profile.is_some(),
            learning_progress: self.learning_progress(),
            using_captured_profile: self.use_captured_profile,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_denoiser_creation() {
        let denoiser = DenoiserPlugin::new(2, false);
        assert_eq!(denoiser.channels(), 2);
        assert_eq!(denoiser.fft_size, 2048);
    }

    #[test]
    fn test_denoiser_low_latency() {
        let denoiser = DenoiserPlugin::new(2, true);
        assert_eq!(denoiser.fft_size, 512);
    }

    #[test]
    fn test_denoiser_from_params() {
        let params = DenoiserPluginParams {
            reduction_db: 20.0,
            floor_db: -40.0,
            ..Default::default()
        };
        let denoiser = DenoiserPlugin::from_params(2, params);
        assert_eq!(denoiser.reduction_db, 20.0);
        assert_eq!(denoiser.floor_db, -40.0);
    }

    #[test]
    fn test_hann_window() {
        let window = DenoiserPlugin::generate_hann_window(8);
        assert_eq!(window.len(), 8);
        // Hann window should be symmetric and peak at center
        assert!((window[0] - 0.0).abs() < 0.01);
        assert!((window[4] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parameter_set_get() {
        let mut denoiser = DenoiserPlugin::new(2, false);
        denoiser.initialize(48000).unwrap();

        denoiser
            .set_parameter(
                ParameterId::from("reduction_db"),
                ParameterValue::Float(25.0),
            )
            .unwrap();
        denoiser
            .set_parameter(ParameterId::from("floor_db"), ParameterValue::Float(-35.0))
            .unwrap();

        let reduction = denoiser.get_parameter(&ParameterId::from("reduction_db"));
        let floor = denoiser.get_parameter(&ParameterId::from("floor_db"));

        assert_eq!(reduction, Some(ParameterValue::Float(25.0)));
        assert_eq!(floor, Some(ParameterValue::Float(-35.0)));
    }
}
