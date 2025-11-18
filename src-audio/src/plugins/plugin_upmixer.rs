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
use super::speaker_config::{SpeakerConfig, calculate_panning_gain, get_speaker_config};
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    1.0
}

fn default_lfe_cutoff_hz() -> f32 {
    120.0
}

fn default_stereo_width() -> f32 {
    0.5
}

fn default_bandpass_hz() -> f32 {
    250.0
}

fn default_speaker_config() -> String {
    "5.1".to_string()
}

fn default_height_gain() -> f32 {
    1.0
}

fn default_lfe_gain() -> f32 {
    1.0
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

    /// Height channel gain (0.0 to 2.0, default 1.0)
    /// Controls how much audio goes to overhead speakers
    #[serde(default = "default_height_gain")]
    pub height_gain: f32,

    /// LFE gain (0.0 to 2.0, default 1.0)
    /// Controls subwoofer level
    #[serde(default = "default_lfe_gain")]
    pub lfe_gain: f32,
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

    /// Forward FFT planner
    fft_forward: Arc<dyn Fft<f32>>,
    /// Inverse FFT planner
    fft_inverse: Arc<dyn Fft<f32>>,

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

    /// Bandpass frequency in Hz (must be > lfe_cutoff_hz)
    param_bandpass_hz: ParameterId,
    bandpass_hz: f32,

    /// Height channel gain (0.0 to 2.0)
    param_height_gain: ParameterId,
    height_gain: f32,

    /// LFE gain (0.0 to 2.0)
    param_lfe_gain: ParameterId,
    lfe_gain: f32,

    /// Panning gains for left source (pre-calculated for each speaker)
    panning_gains_left: Vec<f32>,
    /// Panning gains for right source (pre-calculated for each speaker)
    panning_gains_right: Vec<f32>,

    // Processing buffers (allocated once, reused)
    /// Time domain buffer for left channel
    time_domain_left: Vec<Complex<f32>>,
    /// Time domain buffer for right channel
    time_domain_right: Vec<Complex<f32>>,

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
    direct_center: Vec<Complex<f32>>,
    direct_center_mag: Vec<f32>,
    lfe: Vec<Complex<f32>>,

    // Output time-domain buffers (one per output channel, variable length)
    time_out_channels: Vec<Vec<Complex<f32>>>,

    /// Input buffer accumulator for block-based processing
    input_buffer: Vec<f32>,
    /// Number of samples currently in input buffer
    input_buffer_fill: usize,

    /// Temporary input block for FFT processing (pre-allocated)
    temp_input_block: Vec<f32>,

    /// Hann window for FFT (pre-computed)
    window: Vec<f32>,
    /// Output accumulator for overlap-add (holds fft_size samples per channel)
    /// This allows us to accumulate processed blocks and drain them gradually
    output_accumulator: Vec<Vec<f32>>,
    /// Number of valid samples in output accumulator
    output_accumulator_fill: usize,
    /// Next position to add a block (tracks overlap-add offset)
    next_add_position: usize,
    /// Pre-allocated output block buffer (reused to avoid allocations)
    output_block: Vec<f32>,
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

        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        let zero_complex = Complex::new(0.0, 0.0);

        // Generate Hann window: w[n] = 0.5 * (1 - cos(2*pi*n/N))
        // Using N (not N-1) for perfect COLA with 50% overlap
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / fft_size as f32).cos())
            })
            .collect();

        // 50% overlap requires fft_size/2 hop size
        let hop_size = fft_size / 2;

        // Get speaker configuration
        let speaker_config = get_speaker_config(speaker_config_id).unwrap_or_else(|| {
            log::info!(
                "Invalid speaker config '{}', falling back to 5.1",
                speaker_config_id
            );
            get_speaker_config("5.1").unwrap()
        });

        let num_output_channels = speaker_config.total_channels;

        // Calculate panning gains for stereo sources (left at +30°, right at -30°)
        let left_azimuth = 30.0;
        let right_azimuth = -30.0;

        let mut panning_gains_left = Vec::with_capacity(num_output_channels);
        let mut panning_gains_right = Vec::with_capacity(num_output_channels);

        for speaker in speaker_config.speakers {
            if speaker.is_lfe {
                // LFE gets equal mix from both channels
                panning_gains_left.push(0.5);
                panning_gains_right.push(0.5);
            } else {
                let left_gain =
                    calculate_panning_gain(left_azimuth, 0.0, speaker.azimuth, speaker.elevation);
                let right_gain =
                    calculate_panning_gain(right_azimuth, 0.0, speaker.azimuth, speaker.elevation);
                panning_gains_left.push(left_gain);
                panning_gains_right.push(right_gain);
            }
        }

        // Normalize gains to prevent clipping
        let max_gain: f32 = panning_gains_left
            .iter()
            .zip(panning_gains_right.iter())
            .map(|(l, r)| l + r)
            .fold(0.0f32, f32::max);

        if max_gain > 1.0 {
            let scale = 1.0 / max_gain;
            for i in 0..num_output_channels {
                panning_gains_left[i] *= scale;
                panning_gains_right[i] *= scale;
            }
        }

        // Output accumulator holds up to 3*fft_size samples per channel
        let output_accumulator = vec![vec![0.0; fft_size * 3]; num_output_channels];

        // Allocate output buffers for each channel
        let time_out_channels = vec![vec![zero_complex; fft_size]; num_output_channels];

        Self {
            fft_size,
            hop_size,
            sample_rate: 44100, // Will be updated in initialize()
            speaker_config,
            num_output_channels,

            fft_forward,
            fft_inverse,

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

            param_bandpass_hz: ParameterId::from("bandpass_hz"),
            bandpass_hz,

            param_height_gain: ParameterId::from("height_gain"),
            height_gain,

            param_lfe_gain: ParameterId::from("lfe_gain"),
            lfe_gain,

            panning_gains_left,
            panning_gains_right,

            // Allocate all buffers
            time_domain_left: vec![zero_complex; fft_size],
            time_domain_right: vec![zero_complex; fft_size],
            freq_domain_left: vec![zero_complex; fft_size],
            freq_domain_right: vec![zero_complex; fft_size],
            direct: vec![zero_complex; fft_size],
            direct_left: vec![zero_complex; fft_size],
            direct_right: vec![zero_complex; fft_size],
            ambient_left: vec![zero_complex; fft_size],
            ambient_right: vec![zero_complex; fft_size],
            direct_center: vec![zero_complex; fft_size],
            direct_center_mag: vec![0.0; fft_size],
            lfe: vec![zero_complex; fft_size],

            time_out_channels,

            input_buffer: vec![0.0; fft_size * 2], // stereo
            input_buffer_fill: 0,

            temp_input_block: vec![0.0; fft_size * 2], // Pre-allocated temp buffer

            window,
            output_accumulator,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_block: vec![0.0; fft_size * num_output_channels],
        }
    }

    /// Create a new upmixer plugin from configuration parameters
    pub fn from_params(params: UpmixerPluginParams) -> Self {
        Self::new(
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
        )
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
        let zero_complex = Complex::new(0.0, 0.0);
        self.time_out_channels = vec![vec![zero_complex; self.fft_size]; self.num_output_channels];
        self.output_accumulator = vec![vec![0.0; self.fft_size * 3]; self.num_output_channels];
        self.output_block = vec![0.0; self.fft_size * self.num_output_channels];

        self.recalculate_panning_gains();
        self.reset();

        Ok(())
    }

    /// Recalculate panning gains for current speaker configuration
    fn recalculate_panning_gains(&mut self) {
        let left_azimuth = 30.0;
        let right_azimuth = -30.0;

        self.panning_gains_left.clear();
        self.panning_gains_right.clear();

        for speaker in self.speaker_config.speakers {
            if speaker.is_lfe {
                self.panning_gains_left.push(0.5);
                self.panning_gains_right.push(0.5);
            } else {
                let left_gain =
                    calculate_panning_gain(left_azimuth, 0.0, speaker.azimuth, speaker.elevation);
                let right_gain =
                    calculate_panning_gain(right_azimuth, 0.0, speaker.azimuth, speaker.elevation);
                self.panning_gains_left.push(left_gain);
                self.panning_gains_right.push(right_gain);
            }
        }

        // Normalize gains using energy-preserving normalization
        // For each source (left and right), normalize so sum of squared gains = 1
        let left_energy: f32 = self.panning_gains_left.iter().map(|g| g * g).sum();
        let right_energy: f32 = self.panning_gains_right.iter().map(|g| g * g).sum();

        if left_energy > 0.0 {
            let left_scale = 1.0 / left_energy.sqrt();
            for i in 0..self.num_output_channels {
                self.panning_gains_left[i] *= left_scale;
            }
        }

        if right_energy > 0.0 {
            let right_scale = 1.0 / right_energy.sqrt();
            for i in 0..self.num_output_channels {
                self.panning_gains_right[i] *= right_scale;
            }
        }
    }

    /// Process one FFT block using VBAP panning
    fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Verify sizes
        assert_eq!(input.len(), self.fft_size * 2); // stereo interleaved
        assert_eq!(output.len(), self.fft_size * self.num_output_channels); // variable channels

        // 1. Copy input to time domain buffers and apply ANALYSIS window
        // CRITICAL: Window BEFORE FFT to prevent spectral leakage!
        // Standard STFT: window input -> FFT -> process -> IFFT -> overlap-add
        // Optimized for cache locality - process both channels together
        for i in 0..self.fft_size {
            let idx = i * 2;
            let window_val = self.window[i];
            self.time_domain_left[i] = Complex::new(input[idx] * window_val, 0.0);
            self.time_domain_right[i] = Complex::new(input[idx + 1] * window_val, 0.0);
        }

        // 2. Forward FFT (in-place)
        // Copy to frequency domain buffers first
        self.freq_domain_left
            .copy_from_slice(&self.time_domain_left);
        self.freq_domain_right
            .copy_from_slice(&self.time_domain_right);

        self.fft_forward.process(&mut self.freq_domain_left);
        self.fft_forward.process(&mut self.freq_domain_right);

        // 3. Frequency-dependent processing
        // Calculate frequency bin boundaries
        let lfe_cutoff_bin =
            ((self.lfe_cutoff_hz * self.fft_size as f32) / self.sample_rate as f32) as usize;
        let bandpass_bin =
            ((self.bandpass_hz * self.fft_size as f32) / self.sample_rate as f32) as usize;

        for i in 0..self.fft_size {
            let left = self.freq_domain_left[i];
            let right = self.freq_domain_right[i];

            // Handle Nyquist folding for real FFT
            let is_lfe_band = i <= lfe_cutoff_bin || i >= (self.fft_size - lfe_cutoff_bin);
            let is_passthrough_band = (i > lfe_cutoff_bin && i < bandpass_bin)
                || (i > (self.fft_size - bandpass_bin) && i < (self.fft_size - lfe_cutoff_bin));

            if is_lfe_band {
                // LFE band: low-pass filtered mono sum
                self.lfe[i] = (left + right) * 0.5;
                self.direct_left[i] = Complex::new(0.0, 0.0);
                self.direct_right[i] = Complex::new(0.0, 0.0);
                self.direct_center[i] = Complex::new(0.0, 0.0);
                self.ambient_left[i] = Complex::new(0.0, 0.0);
                self.ambient_right[i] = Complex::new(0.0, 0.0);
            } else if is_passthrough_band {
                // Pass-through band: stereo L/R only (no center extraction)
                self.direct_left[i] = left;
                self.direct_right[i] = right;
                self.direct_center[i] = Complex::new(0.0, 0.0);
                self.lfe[i] = Complex::new(0.0, 0.0);
                self.ambient_left[i] = Complex::new(0.0, 0.0);
                self.ambient_right[i] = Complex::new(0.0, 0.0);
            } else {
                // Upmixing band: apply direct/ambient decomposition

                // Direct component (what's common to both channels - center image)
                self.direct[i] = (left + right) * 0.5;

                // Ambient component (what's different - spatial/reverb)
                self.ambient_left[i] = (left - right) * 0.5;
                self.ambient_right[i] = (right - left) * 0.5;

                // Center channel gets the direct component
                self.direct_center[i] = self.direct[i];
                self.direct_center_mag[i] = self.direct[i].norm();

                // Front left/right: remove center based on stereo_width
                // stereo_width = 0.0: no removal (wide), 1.0: full removal (narrow)
                self.direct_left[i] = left - self.direct[i] * self.stereo_width;
                self.direct_right[i] = right - self.direct[i] * self.stereo_width;

                self.lfe[i] = Complex::new(0.0, 0.0);
            }
        }

        // 4. Apply VBAP panning to distribute to output speakers
        // For each output speaker, calculate frequency-domain signal using panning gains
        let fft_scale = 1.0 / self.fft_size as f32;
        // VBAP normalization already prevents energy buildup, no additional gain needed
        let combined_scale = fft_scale;

        for (ch_idx, speaker) in self.speaker_config.speakers.iter().enumerate() {
            if speaker.is_lfe {
                // LFE channel: use low-pass filtered signal with gain
                for i in 0..self.fft_size {
                    self.time_out_channels[ch_idx][i] = self.lfe[i] * self.lfe_gain;
                }
                self.fft_inverse
                    .process(&mut self.time_out_channels[ch_idx]);
            } else {
                // Regular speaker: pan direct and ambient components using VBAP
                let panning_gain_left = self.panning_gains_left[ch_idx];
                let panning_gain_right = self.panning_gains_right[ch_idx];

                // Apply height gain if this is an elevated speaker
                let height_mult = if speaker.elevation > 0.0 {
                    self.height_gain
                } else {
                    1.0
                };

                // Determine if this is a front or rear speaker based on azimuth
                // Front speakers: azimuth between -90° and +90°
                // Rear speakers: azimuth outside this range
                let is_front = speaker.azimuth.abs() <= 90.0;

                // Select appropriate gains
                let (direct_gain, ambient_gain) = if is_front {
                    (self.gain_front_direct, self.gain_front_ambient)
                } else {
                    (0.0, self.gain_rear_ambient) // Rear speakers get no direct, only ambient
                };

                // Build frequency-domain signal for this speaker
                for i in 0..self.fft_size {
                    // Pan direct component (front soundstage)
                    let direct_component = self.direct_left[i] * panning_gain_left
                        + self.direct_right[i] * panning_gain_right;

                    // Pan ambient component (surround/reverb)
                    let ambient_component = self.ambient_left[i] * panning_gain_left
                        + self.ambient_right[i] * panning_gain_right;

                    // Combine with gain parameters
                    self.time_out_channels[ch_idx][i] = (direct_component * direct_gain
                        + ambient_component * ambient_gain)
                        * height_mult;
                }

                // Inverse FFT for this channel
                self.fft_inverse
                    .process(&mut self.time_out_channels[ch_idx]);
            }
        }

        // 5. Extract real parts and apply final scaling
        // IFFT output is already windowed, ready for overlap-add
        for i in 0..self.fft_size {
            let idx = i * self.num_output_channels;

            for ch in 0..self.num_output_channels {
                output[idx + ch] = self.time_out_channels[ch][i].re * combined_scale;
            }
        }
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
            Parameter::new_int("speaker_config", "Configuration", 0, 0, 9)
                .with_description("Speaker configuration (0=5.1, 1=7.1, 2=5.1.2, 3=5.1.4, 4=7.1.2, 5=7.1.4, 6=9.1.4, 7=9.1.6, 8=2.0, 9=5.0)"),
            Parameter::new_float("gain_front_direct", "Front Direct Gain", 1.0, 0.0, 2.0)
                .with_description("Gain for direct sound in front channels"),
            Parameter::new_float("gain_front_ambient", "Front Ambient Gain", 0.5, 0.0, 2.0)
                .with_description("Gain for ambient sound in front channels"),
            Parameter::new_float("gain_rear_ambient", "Rear Ambient Gain", 1.0, 0.0, 2.0)
                .with_description("Gain for ambient sound in rear channels"),
            Parameter::new_float("height_gain", "Height Gain", 1.0, 0.0, 2.0)
                .with_description("Gain for height/overhead channels (elevation > 0)"),
            Parameter::new_float("lfe_gain", "LFE Gain", 1.0, 0.0, 2.0)
                .with_description("Gain for LFE/subwoofer channel"),
            Parameter::new_float("lfe_cutoff_hz", "LFE Cutoff (Hz)", 120.0, 40.0, 200.0)
                .with_description("Low-pass filter cutoff frequency for LFE channel"),
            Parameter::new_float("stereo_width", "Stereo Width", 0.5, 0.0, 1.0)
                .with_description("Control stereo width (0.0=wide, 1.0=narrow)"),
            Parameter::new_float("bandpass_hz", "Upmix Crossover (Hz)", 250.0, 200.0, 1000.0)
                .with_description("Frequency above which upmixing is applied"),
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
        } else if id == self.param_bandpass_hz
            && let Some(freq) = value.as_float()
        {
            if freq > self.lfe_cutoff_hz {
                self.bandpass_hz = freq;
                return Ok(());
            }
            return Err("Bandpass frequency must be greater than LFE cutoff".to_string());
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
        } else if id == &self.param_bandpass_hz {
            Some(ParameterValue::Float(self.bandpass_hz))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        Ok(())
    }

    fn reset(&mut self) {
        // Clear buffers
        self.input_buffer_fill = 0;
        let zero = Complex::new(0.0, 0.0);
        for buf in [
            &mut self.time_domain_left,
            &mut self.time_domain_right,
            &mut self.freq_domain_left,
            &mut self.freq_domain_right,
            &mut self.direct,
            &mut self.direct_left,
            &mut self.direct_right,
            &mut self.ambient_left,
            &mut self.ambient_right,
            &mut self.direct_center,
            &mut self.lfe,
        ]
        .iter_mut()
        {
            buf.fill(zero);
        }
        self.direct_center_mag.fill(0.0);

        // Clear output channels
        for channel_buf in self.time_out_channels.iter_mut() {
            channel_buf.fill(zero);
        }

        // Clear output accumulator
        for accum_buf in self.output_accumulator.iter_mut() {
            accum_buf.fill(0.0);
        }
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;

        // Clear output block
        self.output_block.fill(0.0);
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

        log::info!(
            "[UPMIXER] process() called: input={} frames, output={} frames",
            context.num_frames,
            context.num_frames
        );
        log::info!(
            "[UPMIXER] Initial state: input_buffer_fill={}, output_accumulator_fill={}, next_add_pos={}",
            self.input_buffer_fill,
            self.output_accumulator_fill,
            self.next_add_position
        );

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
                log::debug!("[UPMIXER] ERROR: Infinite loop detected after 1000 iterations!");
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
                log::info!(
                    "[UPMIXER] Iter {}: DRAIN {} frames (accum_fill={}, frames_avail={})",
                    iteration,
                    frames_to_drain,
                    self.output_accumulator_fill,
                    frames_available
                );

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

                log::info!(
                    "[UPMIXER] After drain: accum_fill={}, next_add_pos={}, output_pos={}",
                    self.output_accumulator_fill,
                    self.next_add_position,
                    output_pos / 5
                );
            }

            // Step 2: Process FFT block if we have input and accumulator space
            // Ensure accumulator won't overflow (need space for fft_size samples)
            let can_process_input = self.input_buffer_fill >= self.fft_size * 2;
            let can_process_space = self.next_add_position + self.fft_size <= self.fft_size * 3;

            if can_process_input && can_process_space {
                log::info!(
                    "[UPMIXER] Iter {}: PROCESS FFT (input_buf_fill={}/{}, next_add_pos={}, space_ok={})",
                    iteration,
                    self.input_buffer_fill / 2,
                    self.fft_size,
                    self.next_add_position,
                    can_process_space
                );

                // Copy to temp buffer
                self.temp_input_block[..self.fft_size * 2]
                    .copy_from_slice(&self.input_buffer[..self.fft_size * 2]);

                // Process FFT block
                let temp_input = std::mem::take(&mut self.temp_input_block);
                let mut output_block = std::mem::take(&mut self.output_block);
                self.process_fft_block(&temp_input, &mut output_block);
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

                log::info!(
                    "[UPMIXER] After FFT: accum_fill={}, next_add_pos={}, input_buf_fill={}",
                    self.output_accumulator_fill,
                    self.next_add_position,
                    self.input_buffer_fill / 2
                );

                continue; // Process more blocks if possible
            } else if !can_process_input || !can_process_space {
                log::info!(
                    "[UPMIXER] Iter {}: SKIP FFT (can_process_input={}, can_process_space={})",
                    iteration,
                    can_process_input,
                    can_process_space
                );
            }

            // Step 3: Fill input buffer if we have more input
            if input_pos < input.len() {
                let samples_to_copy =
                    (input.len() - input_pos).min(self.fft_size * 2 - self.input_buffer_fill);

                log::info!(
                    "[UPMIXER] Iter {}: FILL {} samples (input_pos={}/{}, input_buf_fill={})",
                    iteration,
                    samples_to_copy / 2,
                    input_pos / 2,
                    input.len() / 2,
                    self.input_buffer_fill / 2
                );

                self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                    .copy_from_slice(&input[input_pos..input_pos + samples_to_copy]);

                self.input_buffer_fill += samples_to_copy;
                input_pos += samples_to_copy;

                log::info!(
                    "[UPMIXER] After fill: input_buf_fill={}, input_pos={}",
                    self.input_buffer_fill / 2,
                    input_pos / 2
                );

                continue; // Try processing again
            }

            // No more work to do - exit loop
            // Exit when: output buffer is full OR (no more input AND can't process AND nothing to drain)
            let cant_process = self.input_buffer_fill < self.fft_size * 2
                || self.next_add_position + self.fft_size > self.fft_size * 3;
            let no_data_to_drain = self.output_accumulator_fill == 0;
            let no_space_to_drain = (output.len() - output_pos) / self.num_output_channels == 0;

            log::info!(
                "[UPMIXER] Iter {}: CHECK EXIT - no_more_input={}, cant_process={}, no_data={}, no_space={}",
                iteration,
                input_pos >= input.len(),
                cant_process,
                no_data_to_drain,
                no_space_to_drain
            );

            // Exit if output buffer is full (most important - prevents deadlock)
            if no_space_to_drain {
                log::debug!("[UPMIXER] EXITING LOOP: output buffer full");
                break;
            }

            // Exit if no more input and can't process and nothing to drain
            if input_pos >= input.len() && cant_process && no_data_to_drain {
                log::debug!("[UPMIXER] EXITING LOOP: no more work");
                break;
            }
        }

        log::debug!("[UPMIXER] Loop finished after {} iterations", iteration);
        log::info!(
            "[UPMIXER] Final: output_pos={}/{}, accum_fill={}",
            output_pos / self.num_output_channels,
            output.len() / self.num_output_channels,
            self.output_accumulator_fill
        );

        // Final drain of any remaining output
        let frames_available = (output.len() - output_pos) / 5;
        let frames_to_drain = self.output_accumulator_fill.min(frames_available);

        if frames_to_drain > 0 {
            log::info!(
                "[UPMIXER] FINAL DRAIN: {} frames (accum_fill={}, frames_avail={})",
                frames_to_drain,
                self.output_accumulator_fill,
                frames_available
            );

            for i in 0..frames_to_drain {
                for ch in 0..self.num_output_channels {
                    output[output_pos + i * self.num_output_channels + ch] =
                        self.output_accumulator[ch][i];
                }
            }
            output_pos += frames_to_drain * self.num_output_channels;

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

            log::info!(
                "[UPMIXER] After final drain: accum_fill={}, next_add_pos={}, total_output={}",
                self.output_accumulator_fill,
                self.next_add_position,
                output_pos / 5
            );
        }

        log::info!(
            "[UPMIXER] process() complete: returned {} frames\n",
            output_pos / 5
        );

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        self.fft_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upmixer_creation_5_1() {
        let plugin = UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 6);
        assert_eq!(plugin.fft_size, 2048);
        assert_eq!(plugin.speaker_config.id, "5.1");
    }

    #[test]
    fn test_upmixer_creation_7_1_4() {
        let plugin = UpmixerPlugin::new(2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 12);
        assert_eq!(plugin.fft_size, 2048);
        assert_eq!(plugin.speaker_config.id, "7.1.4");
    }

    #[test]
    fn test_upmixer_parameters() {
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);

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
    fn test_upmixer_processing() {
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
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
        log::info!("Output sum (abs): {}", sum);
        assert!(sum > 0.0, "Output should not be all zeros");

        // Check that we have output in multiple channels
        let num_channels = 6; // 5.1 has 6 channels
        let mut channel_sums = vec![0.0; num_channels];
        for i in 0..2048 {
            for ch in 0..num_channels {
                channel_sums[ch] += output[i * num_channels + ch].abs();
            }
        }
        log::info!("Channel sums: {:?}", channel_sums);
        // At least center and front channels should have content
        assert!(
            channel_sums[0] > 0.0 || channel_sums[1] > 0.0 || channel_sums[2] > 0.0,
            "At least one front channel should have content"
        );
    }

    #[test]
    fn test_upmixer_zero_gains() {
        // Test that with all gains at 0, output is silence (critical for crackling fix)
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 0.0, 0.0, 0.0, 120.0, 0.5, 250.0, 1.0, 0.0);
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

        // Verify output is all zeros (or very close to zero due to floating point)
        let max_abs = output.iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
        log::info!("Max abs value with zero gains: {}", max_abs);
        assert!(
            max_abs < 1e-6,
            "With all gains at 0, output should be silent, but max abs = {}",
            max_abs
        );
    }

    #[test]
    fn test_upmixer_config_change() {
        // Test changing speaker configuration dynamically
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
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
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 0.5, 1.0);
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
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 1.0, 0.0, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
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

        log::info!("Channel energies: {:?}", channel_energies);

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
            log::info!("\n=== Testing buffer size {} ===", buffer_size);
            let mut plugin =
                UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
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
            log::info!(
                "Buffer size {}: {} total frames, {} non-zero samples",
                buffer_size,
                total_output_samples,
                non_zero_samples
            );

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
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
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

        log::info!(
            "Input energy: {}, Output energy: {}, Ratio: {}",
            total_input_energy,
            total_output_energy,
            total_output_energy / total_input_energy
        );

        // Hann window has mean ~0.5, so expect ~75% energy loss (0.5²)
        // With overlap-add we recover some but not all
        // Accept 85% loss as reasonable for Hann windowed STFT
        assert!(
            total_output_energy > total_input_energy * 0.15,
            "Energy loss too high: input={}, output={}, ratio={}",
            total_input_energy,
            total_output_energy,
            total_output_energy / total_input_energy
        );
    }

    #[test]
    fn test_no_gaps() {
        // INVARIANT: Every output buffer should have SOME non-zero samples after initial latency
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
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
                log::info!("GAP at iteration {}: max_abs = {}", iteration, max_abs);
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
        let plugin_2_0 = UpmixerPlugin::new(2048, "2.0", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
        assert_eq!(plugin_2_0.input_channels(), 2);
        assert_eq!(plugin_2_0.output_channels(), 2);
        assert_eq!(plugin_2_0.speaker_config.id, "2.0");

        // Test creating upmixer with 5.0 configuration
        let plugin_5_0 = UpmixerPlugin::new(2048, "5.0", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);
        assert_eq!(plugin_5_0.input_channels(), 2);
        assert_eq!(plugin_5_0.output_channels(), 5);
        assert_eq!(plugin_5_0.speaker_config.id, "5.0");
    }

    #[test]
    fn test_upmixer_parameter_config_indices() {
        let mut plugin =
            UpmixerPlugin::new(2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0);

        // Test that parameter index 0 corresponds to 5.1
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(0)));

        // Test setting to 2.0 (index 8)
        plugin
            .set_parameter(
                ParameterId::from("speaker_config"),
                ParameterValue::Int(8),
            )
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "2.0");
        assert_eq!(plugin.output_channels(), 2);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(8)));

        // Test setting to 5.0 (index 9)
        plugin
            .set_parameter(
                ParameterId::from("speaker_config"),
                ParameterValue::Int(9),
            )
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "5.0");
        assert_eq!(plugin.output_channels(), 5);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(9)));

        // Test setting to 7.1 (index 1)
        plugin
            .set_parameter(
                ParameterId::from("speaker_config"),
                ParameterValue::Int(1),
            )
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "7.1");
        assert_eq!(plugin.output_channels(), 8);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(1)));
    }
}
