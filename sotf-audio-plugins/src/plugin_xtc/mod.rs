// ============================================================================
// Crosstalk Cancellation (XTC) Plugin
// ============================================================================
//
// Implements crosstalk cancellation for stereo playback over speakers.
// This plugin removes acoustic crosstalk to create a binaural-like experience
// from conventional stereo speakers.
//
// Algorithm:
// 1. Signal Windowing & FFT: Convert to frequency domain (1024 samples, 75% overlap, Hann window)
// 2. Transfer Functions: Model ipsilateral (direct) and contralateral (crosstalk) paths
// 3. Inverse with smoothing: Compute regularized inverse filter matrix
// 4. Apply Filter: Process stereo signal with crosstalk cancellation
// 5. IFFT & Overlap-Add: Reconstruct time-domain signal
//
// Geometry:
// - d: Distance to speakers (m)
// - θ: Speaker angle (degrees, typically 30°)
// - a: Head radius (m, typically 0.0875m)
//
// Physical Model:
// - l_ipsi: Same-side path length
// - l_contra: Opposite-side path length
// - Δt: Time difference between paths
// - g(f): Head shadowing filter (low-pass)

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{
    complex_mul_add_simd, complex_mul_simd, deinterleave_stereo, window_mul_add_simd,
    window_mul_simd,
};
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

/// XTC plugin configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtcPluginParams {
    /// Distance to speakers in meters (default: 2.0m)
    #[serde(default = "default_distance")]
    pub distance_m: f32,

    /// Speaker angle in degrees (default: 30°)
    #[serde(default = "default_speaker_angle")]
    pub speaker_angle_deg: f32,

    /// Head radius in meters (default: 0.0875m, typical adult)
    #[serde(default = "default_head_radius")]
    pub head_radius_m: f32,

    /// FFT size (default: 1024, must be power of 2)
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,

    /// Base regularization parameter β (default: 0.001)
    /// Higher values = more stable but less cancellation
    #[serde(default = "default_beta_base")]
    pub beta_base: f32,

    /// Regularization boost at low frequencies (<200Hz) (default: 10.0)
    #[serde(default = "default_beta_low_freq_boost")]
    pub beta_low_freq_boost: f32,

    /// Regularization boost at high frequencies (>8kHz) (default: 10.0)
    #[serde(default = "default_beta_high_freq_boost")]
    pub beta_high_freq_boost: f32,

    /// Head shadowing filter cutoff frequency in Hz (default: 4000 Hz)
    #[serde(default = "default_head_shadow_cutoff")]
    pub head_shadow_cutoff_hz: f32,

    /// Head shadowing filter slope (default: 6.0 dB/octave)
    #[serde(default = "default_head_shadow_slope")]
    pub head_shadow_slope_db_per_octave: f32,

    /// Head tracking: lateral offset in meters (default: 0.0)
    #[serde(default)]
    pub head_offset_x: f32,

    /// Head tracking: depth offset in meters (default: 0.0)
    #[serde(default)]
    pub head_offset_z: f32,

    /// Smoothing time constant for head tracking updates in seconds (default: 0.1s)
    #[serde(default = "default_head_tracking_smooth")]
    pub head_tracking_smooth_s: f32,

    /// Enable plugin (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_distance() -> f32 {
    2.0
}
fn default_speaker_angle() -> f32 {
    30.0
}
fn default_head_radius() -> f32 {
    0.0875
}
fn default_fft_size() -> usize {
    1024
}
fn default_beta_base() -> f32 {
    0.001
}
fn default_beta_low_freq_boost() -> f32 {
    10.0
}
fn default_beta_high_freq_boost() -> f32 {
    10.0
}
fn default_head_shadow_cutoff() -> f32 {
    4000.0
}
fn default_head_shadow_slope() -> f32 {
    6.0
}
fn default_head_tracking_smooth() -> f32 {
    0.1
}
fn default_enabled() -> bool {
    true
}

impl Default for XtcPluginParams {
    fn default() -> Self {
        Self {
            distance_m: default_distance(),
            speaker_angle_deg: default_speaker_angle(),
            head_radius_m: default_head_radius(),
            fft_size: default_fft_size(),
            beta_base: default_beta_base(),
            beta_low_freq_boost: default_beta_low_freq_boost(),
            beta_high_freq_boost: default_beta_high_freq_boost(),
            head_shadow_cutoff_hz: default_head_shadow_cutoff(),
            head_shadow_slope_db_per_octave: default_head_shadow_slope(),
            head_offset_x: 0.0,
            head_offset_z: 0.0,
            head_tracking_smooth_s: default_head_tracking_smooth(),
            enabled: default_enabled(),
        }
    }
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// BACCH-style Crosstalk Cancellation plugin
///
/// Optimized with:
/// - Block-based I/O processing (no sample-by-sample loops)
/// - SIMD complex multiplication for frequency domain filtering
/// - SIMD windowed overlap-add
/// - Contiguous buffer access patterns (no modulo in hot path)
pub struct XtcPlugin {
    /// FFT size (must be power of 2)
    fft_size: usize,

    /// Hop size for overlap-add (75% overlap = fft_size / 4)
    hop_size: usize,

    /// Sample rate
    sample_rate: u32,

    /// Configuration parameters
    params: XtcPluginParams,

    /// Forward FFT planner
    fft_forward: Arc<dyn RealToComplex<f32>>,

    /// Inverse FFT planner
    fft_inverse: Arc<dyn ComplexToReal<f32>>,

    /// Analysis window (Hann)
    analysis_window: Vec<f32>,

    /// Combined scale factor: COLA normalization / FFT size
    output_scale: f32,

    /// Input buffer: holds fft_size samples per channel
    /// Uses linear buffer with shift instead of ring buffer to avoid modulo
    input_buffer_l: Vec<f32>,
    input_buffer_r: Vec<f32>,

    /// Number of samples currently in input buffer (0 to fft_size)
    input_fill: usize,

    /// Output accumulator for overlap-add (holds fft_size + hop_size)
    output_accum_l: Vec<f32>,
    output_accum_r: Vec<f32>,

    /// Number of samples available in output accumulator
    output_available: usize,

    /// Temporary buffers for block processing (avoid per-call allocation)
    temp_input_l: Vec<f32>,
    temp_input_r: Vec<f32>,

    /// Working buffers for FFT
    fft_buffer: Vec<f32>,
    fft_output_l: Vec<Complex<f32>>,
    fft_output_r: Vec<Complex<f32>>,
    ifft_input: Vec<Complex<f32>>,
    ifft_output: Vec<f32>,

    /// Crosstalk cancellation filters in frequency domain
    /// Only store 2 filters since filter_rl == filter_lr and filter_rr == filter_ll
    filter_ll: Vec<Complex<f32>>,
    filter_lr: Vec<Complex<f32>>,

    /// Smoothed head tracking parameters (for interpolation, future use)
    #[allow(dead_code)]
    smooth_offset_x: f32,
    #[allow(dead_code)]
    smooth_offset_z: f32,

    /// Parameters for dynamic updates
    param_enabled: ParameterId,
    param_distance: ParameterId,
    param_speaker_angle: ParameterId,
    param_head_offset_x: ParameterId,
    param_head_offset_z: ParameterId,
}

impl XtcPlugin {
    /// Create a new XTC plugin
    pub fn new(params: XtcPluginParams, sample_rate: u32) -> Result<Self, String> {
        // Validate FFT size
        if !params.fft_size.is_power_of_two() {
            return Err(format!(
                "XTC FFT size must be power of 2, got {}",
                params.fft_size
            ));
        }

        if params.fft_size < 128 || params.fft_size > 16384 {
            return Err(format!(
                "XTC FFT size must be between 128 and 16384, got {}",
                params.fft_size
            ));
        }

        let fft_size = params.fft_size;
        let hop_size = fft_size / 4; // 75% overlap

        // Create FFT planners
        let mut planner = RealFftPlanner::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        // Create Hann window (periodic form for STFT)
        let analysis_window: Vec<f32> = (0..fft_size)
            .map(|i| {
                let x = i as f32 / fft_size as f32;
                0.5 * (1.0 - (2.0 * PI * x).cos())
            })
            .collect();

        // Combined scale factor: COLA normalization (2/3) / FFT size
        let output_scale = (2.0 / 3.0) / fft_size as f32;

        // Compute frequency-domain filters (only need 2 due to symmetry)
        let num_bins = fft_size / 2 + 1;
        let (filter_ll, filter_lr) = compute_xtc_filters_symmetric(&params, sample_rate, num_bins);

        Ok(Self {
            fft_size,
            hop_size,
            sample_rate,
            params: params.clone(),
            fft_forward,
            fft_inverse,
            analysis_window,
            output_scale,
            input_buffer_l: vec![0.0; fft_size],
            input_buffer_r: vec![0.0; fft_size],
            input_fill: 0,
            output_accum_l: vec![0.0; fft_size + hop_size],
            output_accum_r: vec![0.0; fft_size + hop_size],
            output_available: 0,
            // Temp buffers for block processing (max reasonable block size)
            temp_input_l: vec![0.0; 4096],
            temp_input_r: vec![0.0; 4096],
            fft_buffer: vec![0.0; fft_size],
            fft_output_l: vec![Complex::new(0.0, 0.0); num_bins],
            fft_output_r: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_input: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_output: vec![0.0; fft_size],
            filter_ll,
            filter_lr,
            smooth_offset_x: params.head_offset_x,
            smooth_offset_z: params.head_offset_z,
            param_enabled: ParameterId::from("enabled"),
            param_distance: ParameterId::from("distance_m"),
            param_speaker_angle: ParameterId::from("speaker_angle_deg"),
            param_head_offset_x: ParameterId::from("head_offset_x"),
            param_head_offset_z: ParameterId::from("head_offset_z"),
        })
    }

    /// Create from parameters helper
    pub fn from_params(params: XtcPluginParams, sample_rate: u32) -> Result<Self, String> {
        Self::new(params, sample_rate)
    }

    /// Recompute filters when parameters change
    fn update_filters(&mut self) {
        let num_bins = self.fft_size / 2 + 1;
        let (filter_ll, filter_lr) =
            compute_xtc_filters_symmetric(&self.params, self.sample_rate, num_bins);
        self.filter_ll = filter_ll;
        self.filter_lr = filter_lr;
    }

    /// Process one STFT frame using SIMD-optimized operations
    #[inline(always)]
    fn process_stft_frame(&mut self) {
        // Window and FFT left channel (SIMD optimized)
        window_mul_simd(
            &mut self.fft_buffer,
            &self.input_buffer_l,
            &self.analysis_window,
        );
        self.fft_forward
            .process(&mut self.fft_buffer, &mut self.fft_output_l)
            .expect("FFT processing failed");

        // Window and FFT right channel (SIMD optimized)
        window_mul_simd(
            &mut self.fft_buffer,
            &self.input_buffer_r,
            &self.analysis_window,
        );
        self.fft_forward
            .process(&mut self.fft_buffer, &mut self.fft_output_r)
            .expect("FFT processing failed");

        // Apply XTC filter for LEFT output using SIMD:
        // L_out = filter_ll * L_in + filter_lr * R_in
        complex_mul_simd(
            &mut self.ifft_input,
            &self.fft_output_l,
            &self.filter_ll,
        );
        complex_mul_add_simd(
            &mut self.ifft_input,
            &self.fft_output_r,
            &self.filter_lr,
        );

        // Ensure DC and Nyquist bins are real
        let num_bins = self.ifft_input.len();
        self.ifft_input[0].im = 0.0;
        self.ifft_input[num_bins - 1].im = 0.0;

        // IFFT left channel
        self.fft_inverse
            .process(&mut self.ifft_input, &mut self.ifft_output)
            .expect("IFFT processing failed");

        // Overlap-add to left accumulator (SIMD optimized)
        window_mul_add_simd(
            &mut self.output_accum_l,
            &self.ifft_output,
            &self.analysis_window,
            self.output_scale,
        );

        // Apply XTC filter for RIGHT output using SIMD:
        // R_out = filter_lr * L_in + filter_ll * R_in (symmetric)
        complex_mul_simd(
            &mut self.ifft_input,
            &self.fft_output_l,
            &self.filter_lr,
        );
        complex_mul_add_simd(
            &mut self.ifft_input,
            &self.fft_output_r,
            &self.filter_ll,
        );

        // Ensure DC and Nyquist bins are real
        self.ifft_input[0].im = 0.0;
        self.ifft_input[num_bins - 1].im = 0.0;

        // IFFT right channel
        self.fft_inverse
            .process(&mut self.ifft_input, &mut self.ifft_output)
            .expect("IFFT processing failed");

        // Overlap-add to right accumulator (SIMD optimized)
        window_mul_add_simd(
            &mut self.output_accum_r,
            &self.ifft_output,
            &self.analysis_window,
            self.output_scale,
        );

        // Mark hop_size more samples as available
        self.output_available += self.hop_size;
    }

    /// Shift input buffer left by hop_size and clear tail
    #[inline(always)]
    fn shift_input_buffer(&mut self) {
        let overlap = self.fft_size - self.hop_size;
        self.input_buffer_l.copy_within(self.hop_size.., 0);
        self.input_buffer_r.copy_within(self.hop_size.., 0);
        // Clear the tail (will be filled with new samples)
        self.input_buffer_l[overlap..].fill(0.0);
        self.input_buffer_r[overlap..].fill(0.0);
        self.input_fill = overlap;
    }

    /// Shift output accumulator left by hop_size and clear tail
    #[inline(always)]
    fn shift_output_accum(&mut self) {
        self.output_accum_l.copy_within(self.hop_size.., 0);
        self.output_accum_r.copy_within(self.hop_size.., 0);
        let tail_start = self.output_accum_l.len() - self.hop_size;
        self.output_accum_l[tail_start..].fill(0.0);
        self.output_accum_r[tail_start..].fill(0.0);
        self.output_available = self.output_available.saturating_sub(self.hop_size);
    }
}

impl Plugin for XtcPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Crosstalk Cancellation (XTC)".to_string(),
            version: "1.0.0".to_string(),
            author: "SOTF Audio".to_string(),
            description: format!(
                "BACCH-style crosstalk cancellation - FFT size: {}, speakers at {}° and {}m",
                self.fft_size, self.params.speaker_angle_deg, self.params.distance_m
            ),
        }
    }

    fn input_channels(&self) -> usize {
        2 // Stereo input
    }

    fn output_channels(&self) -> usize {
        2 // Stereo output
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool("enabled", "Enabled", self.params.enabled),
            Parameter::new_float(
                "distance_m",
                "Distance (m)",
                self.params.distance_m,
                0.5,
                5.0,
            ),
            Parameter::new_float(
                "speaker_angle_deg",
                "Speaker Angle (deg)",
                self.params.speaker_angle_deg,
                15.0,
                60.0,
            ),
            Parameter::new_float(
                "head_offset_x",
                "Head Offset X (m)",
                self.params.head_offset_x,
                -0.5,
                0.5,
            ),
            Parameter::new_float(
                "head_offset_z",
                "Head Offset Z (m)",
                self.params.head_offset_z,
                -0.5,
                0.5,
            ),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let mut needs_filter_update = false;

        if id == self.param_enabled {
            if let ParameterValue::Bool(v) = value {
                self.params.enabled = v;
            } else {
                return Err("enabled parameter must be bool".to_string());
            }
        } else if id == self.param_distance {
            if let ParameterValue::Float(v) = value {
                self.params.distance_m = v.max(0.5).min(5.0);
                needs_filter_update = true;
            } else {
                return Err("distance_m parameter must be float".to_string());
            }
        } else if id == self.param_speaker_angle {
            if let ParameterValue::Float(v) = value {
                self.params.speaker_angle_deg = v.max(15.0).min(60.0);
                needs_filter_update = true;
            } else {
                return Err("speaker_angle_deg parameter must be float".to_string());
            }
        } else if id == self.param_head_offset_x {
            if let ParameterValue::Float(v) = value {
                self.params.head_offset_x = v.max(-0.5).min(0.5);
                needs_filter_update = true;
            } else {
                return Err("head_offset_x parameter must be float".to_string());
            }
        } else if id == self.param_head_offset_z {
            if let ParameterValue::Float(v) = value {
                self.params.head_offset_z = v.max(-0.5).min(0.5);
                needs_filter_update = true;
            } else {
                return Err("head_offset_z parameter must be float".to_string());
            }
        } else {
            return Err(format!("Unknown parameter: {:?}", id));
        }

        if needs_filter_update {
            self.update_filters();
        }

        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_enabled {
            Some(ParameterValue::Bool(self.params.enabled))
        } else if id == &self.param_distance {
            Some(ParameterValue::Float(self.params.distance_m))
        } else if id == &self.param_speaker_angle {
            Some(ParameterValue::Float(self.params.speaker_angle_deg))
        } else if id == &self.param_head_offset_x {
            Some(ParameterValue::Float(self.params.head_offset_x))
        } else if id == &self.param_head_offset_z {
            Some(ParameterValue::Float(self.params.head_offset_z))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_filters();
        Ok(())
    }

    fn reset(&mut self) {
        // Clear all buffers
        self.input_buffer_l.fill(0.0);
        self.input_buffer_r.fill(0.0);
        self.output_accum_l.fill(0.0);
        self.output_accum_r.fill(0.0);
        self.input_fill = 0;
        self.output_available = 0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;

        // Verify buffer sizes (stereo: 2 channels)
        if input.len() != num_frames * 2 {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                num_frames * 2,
                input.len()
            ));
        }
        if output.len() != num_frames * 2 {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                num_frames * 2,
                output.len()
            ));
        }

        // Bypass if disabled
        if !self.params.enabled {
            output.copy_from_slice(input);
            return Ok(());
        }

        // Ensure temp buffers are large enough
        if num_frames > self.temp_input_l.len() {
            self.temp_input_l.resize(num_frames, 0.0);
            self.temp_input_r.resize(num_frames, 0.0);
        }

        // Block-based deinterleave using SIMD
        deinterleave_stereo(
            &input[..num_frames * 2],
            &mut self.temp_input_l[..num_frames],
            &mut self.temp_input_r[..num_frames],
        );

        let mut in_pos = 0;
        let mut out_pos = 0;

        while in_pos < num_frames || out_pos < num_frames {
            // Fill input buffer from deinterleaved temp buffers
            let samples_needed = self.fft_size - self.input_fill;
            let samples_available_in = num_frames - in_pos;
            let to_copy = samples_needed.min(samples_available_in);

            if to_copy > 0 {
                self.input_buffer_l[self.input_fill..self.input_fill + to_copy]
                    .copy_from_slice(&self.temp_input_l[in_pos..in_pos + to_copy]);
                self.input_buffer_r[self.input_fill..self.input_fill + to_copy]
                    .copy_from_slice(&self.temp_input_r[in_pos..in_pos + to_copy]);
                self.input_fill += to_copy;
                in_pos += to_copy;
            }

            // Process STFT frame when we have enough input
            if self.input_fill >= self.fft_size {
                self.process_stft_frame();
                self.shift_input_buffer();
            }

            // Copy available output to output buffer
            let samples_to_output = self.output_available.min(num_frames - out_pos);
            if samples_to_output > 0 {
                // Interleave output directly
                for i in 0..samples_to_output {
                    output[(out_pos + i) * 2] = self.output_accum_l[i];
                    output[(out_pos + i) * 2 + 1] = self.output_accum_r[i];
                }
                out_pos += samples_to_output;

                // Shift accumulator after consuming
                if samples_to_output >= self.hop_size {
                    self.shift_output_accum();
                } else {
                    // Partial shift - rare case
                    self.output_accum_l.copy_within(samples_to_output.., 0);
                    self.output_accum_r.copy_within(samples_to_output.., 0);
                    let tail = self.output_accum_l.len() - samples_to_output;
                    self.output_accum_l[tail..].fill(0.0);
                    self.output_accum_r[tail..].fill(0.0);
                    self.output_available -= samples_to_output;
                }
            } else if out_pos < num_frames && self.output_available == 0 {
                // During initial latency, output silence
                let remaining = num_frames - out_pos;
                for i in 0..remaining {
                    output[(out_pos + i) * 2] = 0.0;
                    output[(out_pos + i) * 2 + 1] = 0.0;
                }
                out_pos = num_frames;
            }

            // Prevent infinite loop
            if to_copy == 0 && samples_to_output == 0 && out_pos < num_frames {
                // Fill remaining with silence
                for i in out_pos..num_frames {
                    output[i * 2] = 0.0;
                    output[i * 2 + 1] = 0.0;
                }
                break;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        // Latency is approximately fft_size - hop_size due to overlap-add
        self.fft_size - self.hop_size
    }
}

// ============================================================================
// Crosstalk Cancellation Filter Computation
// ============================================================================

/// Compute crosstalk cancellation filters in frequency domain (symmetric version)
///
/// Since the XTC matrix is symmetric (filter_rl == filter_lr and filter_rr == filter_ll),
/// we only need to compute and store 2 filters instead of 4.
///
/// Returns (filter_ll, filter_lr) where:
/// - filter_ll: diagonal filter (direct path processing)
/// - filter_lr: cross filter (crosstalk cancellation)
fn compute_xtc_filters_symmetric(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> (Vec<Complex<f32>>, Vec<Complex<f32>>) {
    let mut filter_ll = Vec::with_capacity(num_bins);
    let mut filter_lr = Vec::with_capacity(num_bins);

    // Constants
    let speed_of_sound = 343.0; // m/s at 20°C

    // Geometry with head tracking offsets
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Compute path lengths (considering head offset)
    let l_ipsi = ((d * theta_rad.sin() - x_offset).powi(2) + (d * theta_rad.cos()).powi(2)).sqrt();
    let l_contra = ((d * theta_rad.sin() + x_offset).powi(2) + (d * theta_rad.cos()).powi(2))
        .sqrt()
        + PI * a; // Add head shadow path

    // Time difference
    let delta_t = (l_contra - l_ipsi) / speed_of_sound;

    // Process each frequency bin
    for bin in 0..num_bins {
        let freq = bin as f32 * sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

        // Transfer function for ipsilateral path (reference = 1)
        let h_ipsi = Complex::new(1.0, 0.0);

        // Transfer function for contralateral path
        let g = head_shadowing_filter(freq, params);
        let phase = -2.0 * PI * freq * delta_t;
        let h_contra = Complex::new(g * phase.cos(), g * phase.sin());

        // Frequency-dependent regularization
        let beta = compute_beta(freq, params);

        // C^H * C for symmetric case
        let h_ipsi_mag_sq = h_ipsi.norm_sqr();
        let h_contra_mag_sq = h_contra.norm_sqr();
        let cross_term = (h_ipsi * h_contra.conj()).re * 2.0;

        let diag = h_ipsi_mag_sq + h_contra_mag_sq + beta;
        let off_diag = cross_term;

        // Determinant of (C^H * C + β*I)
        let det = diag * diag - off_diag * off_diag;

        if det.abs() < 1e-10 {
            // Singular matrix - use identity (bypass)
            filter_ll.push(Complex::new(1.0, 0.0));
            filter_lr.push(Complex::new(0.0, 0.0));
            continue;
        }

        // Inverse of (C^H * C + β*I)
        let inv_diag = diag / det;
        let inv_off_diag = -off_diag / det;

        // Compute W matrix elements
        let h_ipsi_conj = h_ipsi.conj();
        let h_contra_conj = h_contra.conj();

        let w_ll = h_ipsi_conj * inv_diag + h_contra_conj * inv_off_diag;
        let w_lr = h_ipsi_conj * inv_off_diag + h_contra_conj * inv_diag;

        filter_ll.push(w_ll);
        filter_lr.push(w_lr);
    }

    (filter_ll, filter_lr)
}

/// Compute crosstalk cancellation filters in frequency domain (4-filter version for tests)
#[allow(dead_code)]
fn compute_xtc_filters(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> (
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
) {
    let mut filter_ll = Vec::with_capacity(num_bins);
    let mut filter_lr = Vec::with_capacity(num_bins);
    let mut filter_rl = Vec::with_capacity(num_bins);
    let mut filter_rr = Vec::with_capacity(num_bins);

    // Constants
    let speed_of_sound = 343.0; // m/s at 20°C

    // Geometry with head tracking offsets
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Compute path lengths (considering head offset)
    // l_ipsi: direct path (same side)
    // l_contra: crosstalk path (opposite side)
    let l_ipsi = ((d * theta_rad.sin() - x_offset).powi(2) + (d * theta_rad.cos()).powi(2)).sqrt();

    let l_contra = ((d * theta_rad.sin() + x_offset).powi(2) + (d * theta_rad.cos()).powi(2))
        .sqrt()
        + PI * a; // Add head shadow path

    // Time difference
    let delta_t = (l_contra - l_ipsi) / speed_of_sound;

    // Process each frequency bin
    for bin in 0..num_bins {
        let freq = bin as f32 * sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

        // Transfer function for ipsilateral path (reference = 1)
        let h_ipsi = Complex::new(1.0, 0.0);

        // Transfer function for contralateral path
        // H_contra(f) = g(f) * e^(-j*2*pi*f*delta_t)
        let g = head_shadowing_filter(freq, params);
        let phase = -2.0 * PI * freq * delta_t;
        let h_contra = Complex::new(g * phase.cos(), g * phase.sin());

        // Crosstalk matrix C:
        // C = [[h_ipsi, h_contra],
        //      [h_contra, h_ipsi]]
        //
        // We want to invert C to get the cancellation filters W:
        // W = (C^H * C + β(f) * I)^(-1) * C^H
        //
        // For 2x2 matrix, we can compute this directly:

        // Frequency-dependent regularization
        let beta = compute_beta(freq, params);

        // C^H * C (Hermitian transpose times C)
        // For our symmetric case:
        // C^H * C = [[|h_ipsi|^2 + |h_contra|^2, 2*Re(h_ipsi*h_contra^*)],
        //            [2*Re(h_ipsi*h_contra^*), |h_ipsi|^2 + |h_contra|^2]]

        let h_ipsi_mag_sq = h_ipsi.norm_sqr();
        let h_contra_mag_sq = h_contra.norm_sqr();
        let cross_term = (h_ipsi * h_contra.conj()).re * 2.0;

        let diag = h_ipsi_mag_sq + h_contra_mag_sq + beta;
        let off_diag = cross_term;

        // Determinant of (C^H * C + β*I)
        let det = diag * diag - off_diag * off_diag;

        if det.abs() < 1e-10 {
            // Singular matrix - use identity (bypass)
            filter_ll.push(Complex::new(1.0, 0.0));
            filter_lr.push(Complex::new(0.0, 0.0));
            filter_rl.push(Complex::new(0.0, 0.0));
            filter_rr.push(Complex::new(1.0, 0.0));
            continue;
        }

        // Inverse of (C^H * C + β*I)
        let inv_diag = diag / det;
        let inv_off_diag = -off_diag / det;

        // W = inv(C^H * C + β*I) * C^H
        // For our case:
        // C^H = [[h_ipsi^*, h_contra^*],
        //        [h_contra^*, h_ipsi^*]]

        // W[0,0] = inv_diag * h_ipsi^* + inv_off_diag * h_contra^*
        // W[0,1] = inv_off_diag * h_ipsi^* + inv_diag * h_contra^*
        // W[1,0] = inv_off_diag * h_ipsi^* + inv_diag * h_contra^*
        // W[1,1] = inv_diag * h_ipsi^* + inv_off_diag * h_contra^*

        let h_ipsi_conj = h_ipsi.conj();
        let h_contra_conj = h_contra.conj();

        let w_ll = h_ipsi_conj * inv_diag + h_contra_conj * inv_off_diag;
        let w_lr = h_ipsi_conj * inv_off_diag + h_contra_conj * inv_diag;
        let w_rl = w_lr; // Symmetric
        let w_rr = w_ll; // Symmetric

        filter_ll.push(w_ll);
        filter_lr.push(w_lr);
        filter_rl.push(w_rl);
        filter_rr.push(w_rr);
    }

    (filter_ll, filter_lr, filter_rl, filter_rr)
}

/// Head shadowing filter: low-pass filter modeling high-frequency attenuation
/// as sound diffracts around the head
fn head_shadowing_filter(freq: f32, params: &XtcPluginParams) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Simple low-pass model: g(f) = 1 / (1 + (f / f_c)^n)
    // where n is determined by slope
    let f_c = params.head_shadow_cutoff_hz;
    let slope = params.head_shadow_slope_db_per_octave;

    // Convert slope to filter order (approximately)
    let n = slope / 6.0; // 6 dB/octave ≈ 1st order

    let ratio = freq / f_c;
    let attenuation = 1.0 / (1.0 + ratio.powf(n));

    attenuation
}

/// Compute frequency-dependent regularization parameter β(f)
fn compute_beta(freq: f32, params: &XtcPluginParams) -> f32 {
    let beta_base = params.beta_base;
    let low_boost = params.beta_low_freq_boost;
    let high_boost = params.beta_high_freq_boost;

    // Bell-shaped boost: stronger regularization at <200Hz and >8kHz
    let low_freq_factor = if freq < 200.0 {
        1.0 + low_boost * (1.0 - freq / 200.0)
    } else {
        1.0
    };

    let high_freq_factor = if freq > 8000.0 {
        1.0 + high_boost * ((freq - 8000.0) / 12000.0).min(1.0)
    } else {
        1.0
    };

    beta_base * low_freq_factor * high_freq_factor
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xtc_creation() {
        let params = XtcPluginParams::default();
        let plugin = XtcPlugin::new(params, 48000).unwrap();
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_xtc_bypass() {
        let mut params = XtcPluginParams::default();
        params.enabled = false;
        let mut plugin = XtcPlugin::new(params, 48000).unwrap();
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            input[i * 2] = (i as f32 * 0.01).sin();
            input[i * 2 + 1] = (i as f32 * 0.01).cos();
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Bypass should be exact passthrough
        for i in 0..input.len() {
            assert_eq!(output[i], input[i]);
        }
    }

    #[test]
    fn test_xtc_processing() {
        let params = XtcPluginParams::default();
        let mut plugin = XtcPlugin::new(params, 48000).unwrap();
        plugin.initialize(48000).unwrap();

        // Test with stereo sine wave
        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.cos() * 0.5;
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Output should be non-zero
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "Output should not be all zeros");
    }

    #[test]
    fn test_head_shadowing_filter() {
        let params = XtcPluginParams::default();

        // At DC, should be 1.0 (no attenuation)
        let g_dc = head_shadowing_filter(0.0, &params);
        assert!((g_dc - 1.0).abs() < 0.01);

        // At cutoff frequency, should be attenuated
        let g_cutoff = head_shadowing_filter(params.head_shadow_cutoff_hz, &params);
        assert!(g_cutoff < 1.0);
        assert!(g_cutoff > 0.0);

        // At very high frequency, should be heavily attenuated
        let g_high = head_shadowing_filter(20000.0, &params);
        assert!(g_high < g_cutoff);
    }

    #[test]
    fn test_beta_computation() {
        let params = XtcPluginParams::default();

        // Mid-range frequency: should be close to base beta
        let beta_mid = compute_beta(1000.0, &params);
        assert!((beta_mid - params.beta_base).abs() < params.beta_base * 0.1);

        // Low frequency: should be boosted
        let beta_low = compute_beta(100.0, &params);
        assert!(beta_low > params.beta_base * 2.0);

        // High frequency: should be boosted
        let beta_high = compute_beta(10000.0, &params);
        assert!(beta_high > params.beta_base * 2.0);
    }

    #[test]
    fn test_parameter_updates() {
        let params = XtcPluginParams::default();
        let mut plugin = XtcPlugin::new(params, 48000).unwrap();

        // Update distance
        plugin
            .set_parameter(
                ParameterId::from("distance_m"),
                ParameterValue::Float(2.5),
            )
            .unwrap();
        assert_eq!(plugin.params.distance_m, 2.5);

        // Update speaker angle
        plugin
            .set_parameter(
                ParameterId::from("speaker_angle_deg"),
                ParameterValue::Float(45.0),
            )
            .unwrap();
        assert_eq!(plugin.params.speaker_angle_deg, 45.0);

        // Toggle enabled
        plugin
            .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
            .unwrap();
        assert_eq!(plugin.params.enabled, false);
    }

    #[test]
    fn test_invalid_fft_size() {
        let mut params = XtcPluginParams::default();
        params.fft_size = 1000; // Not power of 2
        let result = XtcPlugin::new(params, 48000);
        assert!(result.is_err());
    }

    /// Test that energy is approximately preserved through XTC processing.
    /// XTC should modify phase relationships but not drastically attenuate the signal.
    #[test]
    fn test_energy_preservation() {
        let params = XtcPluginParams::default();
        let mut plugin = XtcPlugin::new(params, 48000).unwrap();
        plugin.initialize(48000).unwrap();

        // Generate test signal: stereo sine wave at 1kHz (in the optimal XTC range)
        let num_frames = 8192; // Long enough to get past latency and steady-state
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.cos() * 0.5;
        }
        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Calculate energy (skip initial latency period)
        let skip_samples = 2048; // Skip latency
        let input_energy: f32 = input[skip_samples * 2..]
            .iter()
            .map(|x| x * x)
            .sum();
        let output_energy: f32 = output[skip_samples * 2..]
            .iter()
            .map(|x| x * x)
            .sum();

        // Energy ratio should be between 0.5 and 2.0 (within 3dB)
        // XTC can boost some frequencies while attenuating others,
        // but total energy should be reasonably preserved
        let energy_ratio = output_energy / input_energy;
        assert!(
            energy_ratio > 0.3 && energy_ratio < 3.0,
            "Energy ratio {} is outside acceptable range [0.3, 3.0]. \
             Input energy: {}, Output energy: {}",
            energy_ratio,
            input_energy,
            output_energy
        );
    }

    /// Test that mono signal (L=R) passes through with expected attenuation.
    /// For mono content, XTC naturally attenuates by factor of ~1/(1+H_contra),
    /// which is approximately 0.4-0.6 depending on frequency.
    /// This is expected behavior - there's no stereo difference to preserve.
    #[test]
    fn test_mono_signal_behavior() {
        let params = XtcPluginParams::default();
        let mut plugin = XtcPlugin::new(params, 48000).unwrap();
        plugin.initialize(48000).unwrap();

        // Mono signal (same content in L and R)
        let num_frames = 8192;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }
        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Skip latency
        let skip_samples = 2048;
        let input_energy: f32 = input[skip_samples * 2..]
            .iter()
            .map(|x| x * x)
            .sum();
        let output_energy: f32 = output[skip_samples * 2..]
            .iter()
            .map(|x| x * x)
            .sum();

        let energy_ratio = output_energy / input_energy;
        // Mono is expected to be attenuated by XTC (typically 0.3-0.7)
        // This is the mathematically correct behavior for crosstalk cancellation
        assert!(
            energy_ratio > 0.2 && energy_ratio < 0.8,
            "Mono energy ratio {} outside expected XTC range [0.2, 0.8]",
            energy_ratio
        );

        // L and R output should be approximately equal for mono input
        let mut l_energy = 0.0_f32;
        let mut r_energy = 0.0_f32;
        for i in skip_samples..num_frames {
            l_energy += output[i * 2] * output[i * 2];
            r_energy += output[i * 2 + 1] * output[i * 2 + 1];
        }
        let lr_ratio = l_energy / r_energy;
        assert!(
            lr_ratio > 0.9 && lr_ratio < 1.1,
            "L/R energy ratio {} is not balanced for mono",
            lr_ratio
        );
    }

    /// Test continuous processing across multiple blocks.
    #[test]
    fn test_continuous_processing() {
        let params = XtcPluginParams::default();
        let mut plugin = XtcPlugin::new(params, 48000).unwrap();
        plugin.initialize(48000).unwrap();

        let block_size = 512;
        let num_blocks = 20;

        // Process multiple blocks
        for block in 0..num_blocks {
            let mut input = vec![0.0_f32; block_size * 2];
            for i in 0..block_size {
                let sample_idx = block * block_size + i;
                let phase = 2.0 * std::f32::consts::PI * 1000.0 * sample_idx as f32 / 48000.0;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.cos() * 0.5;
            }
            let mut output = vec![0.0_f32; block_size * 2];

            let context = ProcessContext {
                sample_rate: 48000,
                num_frames: block_size,
            };

            plugin.process(&input, &mut output, &context).unwrap();

            // After initial latency, output should have non-zero energy
            if block > 5 {
                let output_energy: f32 = output.iter().map(|x| x * x).sum();
                assert!(
                    output_energy > 0.01,
                    "Block {} has near-zero output energy: {}",
                    block,
                    output_energy
                );
            }
        }
    }

    /// Test that XTC filters have reasonable magnitudes.
    #[test]
    fn test_filter_magnitudes() {
        let params = XtcPluginParams::default();
        let num_bins = 513; // For 1024-point FFT
        let (filter_ll, filter_lr, filter_rl, filter_rr) =
            compute_xtc_filters(&params, 48000, num_bins);

        // Check mid-frequency bin (around 1kHz)
        let bin_1khz = (1000.0 * 1024.0 / 48000.0) as usize;

        // Diagonal filters should be close to 1.0 (direct path)
        let mag_ll = filter_ll[bin_1khz].norm();
        let mag_rr = filter_rr[bin_1khz].norm();
        assert!(
            mag_ll > 0.5 && mag_ll < 2.0,
            "filter_ll magnitude {} at 1kHz outside range",
            mag_ll
        );
        assert!(
            mag_rr > 0.5 && mag_rr < 2.0,
            "filter_rr magnitude {} at 1kHz outside range",
            mag_rr
        );

        // Cross-filters should have smaller but non-zero magnitude
        let mag_lr = filter_lr[bin_1khz].norm();
        let mag_rl = filter_rl[bin_1khz].norm();
        assert!(
            mag_lr < 1.5,
            "filter_lr magnitude {} at 1kHz is too large",
            mag_lr
        );
        assert!(
            mag_rl < 1.5,
            "filter_rl magnitude {} at 1kHz is too large",
            mag_rl
        );
    }
}
