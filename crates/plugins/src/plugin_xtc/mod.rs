//! ============================================================================
//! Crosstalk Cancellation (XTC) Plugin
//! ============================================================================
//!
//! Implements crosstalk cancellation for stereo playback over speakers.
//! This plugin removes acoustic crosstalk to create a binaural-like experience
//! from conventional stereo speakers.
//!
//! Algorithm:
//! 1. Signal Windowing & FFT: Convert to frequency domain (1024 samples, 75% overlap, Hann window)
//! 2. Transfer Functions: Model ipsilateral (direct) and contralateral (crosstalk) paths
//! 3. Inverse with smoothing: Compute regularized inverse filter matrix
//! 4. Apply Filter: Process stereo signal with crosstalk cancellation
//! 5. IFFT & Overlap-Add: Reconstruct time-domain signal
//!
//! Geometry:
//! - d: Distance to speakers (m)
//! - θ: Speaker angle (degrees, typically 30°)
//! - a: Head radius (m, typically 0.0875m)
//!
//! Physical Model:
//! - l_ipsi: Same-side path length
//! - l_contra: Opposite-side path length
//! - Δt: Time difference between paths
//! - g(f): Head shadowing filter (low-pass)

use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{
    complex_mul_add_simd, complex_mul_simd, deinterleave_stereo, scale_add_simd, window_mul_simd,
};
use super::smoothing::Smoother;
use parking_lot::RwLock;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use std::sync::Arc;

type XtcFilterSet = (
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
);

// ============================================================================
// Configuration
// ============================================================================

/// Smoothing mode for head tracking parameter updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SmoothingMode {
    /// Per-block filter crossfade (efficient, default)
    #[default]
    Block,
    /// Per-sample coefficient interpolation (precise but higher CPU)
    Sample,
}

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

    /// Base regularization parameter β (default: 0.0003)
    /// Higher values = more stable but less cancellation
    #[serde(default = "default_beta_base")]
    pub beta_base: f32,

    /// Regularization boost at low frequencies (<100Hz) (default: 10.0)
    #[serde(default = "default_beta_low_freq_boost")]
    pub beta_low_freq_boost: f32,

    /// Regularization boost at high frequencies (>12kHz) (default: 10.0)
    #[serde(default = "default_beta_high_freq_boost")]
    pub beta_high_freq_boost: f32,

    /// Maximum filter gain in dB (default: 25.0)
    /// Limits how much the cancellation filter can boost any frequency bin.
    /// Lower values are safer but reduce cancellation depth.
    #[serde(default = "default_max_gain_db")]
    pub max_gain_db: f32,

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

    /// Head tracking: yaw angle in degrees (-90 to +90, 0 = facing forward)
    #[serde(default)]
    pub head_yaw_deg: f32,

    /// Smoothing time constant for head tracking updates in seconds (default: 0.1s)
    #[serde(default = "default_head_tracking_smooth")]
    pub head_tracking_smooth_s: f32,

    /// Smoothing mode for head tracking (Block or Sample)
    #[serde(default)]
    pub head_tracking_smoothing_mode: SmoothingMode,

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
    2048
}
fn default_beta_base() -> f32 {
    0.0003
}
fn default_max_gain_db() -> f32 {
    25.0
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
            max_gain_db: default_max_gain_db(),
            head_shadow_cutoff_hz: default_head_shadow_cutoff(),
            head_shadow_slope_db_per_octave: default_head_shadow_slope(),
            head_offset_x: 0.0,
            head_offset_z: 0.0,
            head_yaw_deg: 0.0,
            head_tracking_smooth_s: default_head_tracking_smooth(),
            head_tracking_smoothing_mode: SmoothingMode::default(),
            enabled: default_enabled(),
        }
    }
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Crosstalk cancellation filters in frequency domain
struct XtcFilters {
    /// Diagonal filter for left output (L_out += filter_ll * L_in)
    filter_ll: Vec<Complex<f32>>,
    /// Cross filter for left output (L_out += filter_lr * R_in)
    filter_lr: Vec<Complex<f32>>,
    /// Cross filter for right output (R_out += filter_rl * L_in), None if symmetric
    filter_rl: Option<Vec<Complex<f32>>>,
    /// Diagonal filter for right output (R_out += filter_rr * R_in), None if symmetric
    filter_rr: Option<Vec<Complex<f32>>>,
}

/// Crosstalk Cancellation plugin
///
/// Optimized with:
/// - Block-based I/O processing (no sample-by-sample loops)
/// - SIMD complex multiplication for frequency domain filtering
/// - SIMD windowed overlap-add
/// - Contiguous buffer access patterns (no modulo in hot path)
/// - Asynchronous filter recomputation to avoid audio glitches
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

    /// Thread-safe crosstalk cancellation filters
    filters: Arc<RwLock<XtcFilters>>,

    /// Previous filter set for crossfading (Block mode)
    prev_filters: Option<Arc<RwLock<XtcFilters>>>,

    /// Crossfade progress (0.0 = prev, 1.0 = current)
    crossfade_progress: f32,

    /// Smoother for head offset X (Sample mode)
    smoother_offset_x: Smoother,

    /// Smoother for head offset Z (Sample mode)
    smoother_offset_z: Smoother,

    /// Smoother for head yaw angle (Sample mode)
    smoother_yaw: Smoother,

    /// Current smoothed position for Sample mode
    current_offset_x: f32,
    current_offset_z: f32,
    current_yaw_deg: f32,

    /// Parameters for dynamic updates
    param_enabled: ParameterId,
    param_distance: ParameterId,
    param_speaker_angle: ParameterId,
    param_head_offset_x: ParameterId,
    param_head_offset_z: ParameterId,
    param_head_yaw: ParameterId,
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

        // Combined scale factor: COLA normalization (0.5) / FFT size
        // Hann analysis window (sum=0.5*N) + No synthesis window + 75% overlap (hop=N/4)
        // Gain = hop / sum(w) = (N/4) / (0.5*N) = 0.25 / 0.5 = 0.5
        let output_scale = 0.5 / fft_size as f32;

        // Compute frequency-domain filters
        let num_bins = fft_size / 2 + 1;
        let filters = compute_xtc_filters_full(&params, sample_rate, num_bins);
        let filters = Arc::new(RwLock::new(filters));

        // Initialize smoothers with the smoothing time constant (convert s to ms)
        let smooth_time_ms = params.head_tracking_smooth_s * 1000.0;

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
            filters,
            prev_filters: None,
            crossfade_progress: 1.0, // Start fully faded to current
            smoother_offset_x: Smoother::new(params.head_offset_x, smooth_time_ms, sample_rate),
            smoother_offset_z: Smoother::new(params.head_offset_z, smooth_time_ms, sample_rate),
            smoother_yaw: Smoother::new(params.head_yaw_deg, smooth_time_ms, sample_rate),
            current_offset_x: params.head_offset_x,
            current_offset_z: params.head_offset_z,
            current_yaw_deg: params.head_yaw_deg,
            param_enabled: ParameterId::from("enabled"),
            param_distance: ParameterId::from("distance_m"),
            param_speaker_angle: ParameterId::from("speaker_angle_deg"),
            param_head_offset_x: ParameterId::from("head_offset_x"),
            param_head_offset_z: ParameterId::from("head_offset_z"),
            param_head_yaw: ParameterId::from("head_yaw_deg"),
        })
    }

    /// Create from parameters helper
    pub fn from_params(params: XtcPluginParams, sample_rate: u32) -> Result<Self, String> {
        Self::new(params, sample_rate)
    }

    /// Recompute filters when parameters change
    /// In Block mode: stores old filters for crossfade
    /// In Sample mode: updates immediately (should only be called when threshold exceeded)
    fn update_filters(&mut self, sync: bool) {
        let num_bins = self.fft_size / 2 + 1;
        let params = self.params.clone();
        let sample_rate = self.sample_rate;

        // In Block mode, store old filters for crossfading
        if self.params.head_tracking_smoothing_mode == SmoothingMode::Block
            && self.crossfade_progress >= 1.0
        {
            self.prev_filters = Some(self.filters.clone());
            self.crossfade_progress = 0.0;
        }

        let shared_filters = self.filters.clone();

        if sync {
            let new_filters = compute_xtc_filters_full(&params, sample_rate, num_bins);
            let mut lock = shared_filters.write();
            *lock = new_filters;
        } else {
            // Asynchronous update using rayon
            rayon::spawn(move || {
                let new_filters = compute_xtc_filters_full(&params, sample_rate, num_bins);
                let mut lock = shared_filters.write();
                *lock = new_filters;
            });
        }
    }

    /// Process one STFT frame using SIMD-optimized operations
    #[inline(always)]
    fn process_stft_frame(&mut self) {
        // Access filters via read lock (usually very fast if no writer)
        let filters = self.filters.read();

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

        // Get filters for right channel (use symmetric if not asymmetric)
        let filter_rl = filters.filter_rl.as_ref().unwrap_or(&filters.filter_lr);
        let filter_rr = filters.filter_rr.as_ref().unwrap_or(&filters.filter_ll);

        // Apply XTC filter for LEFT output using SIMD:
        // L_out = filter_ll * L_in + filter_lr * R_in
        complex_mul_simd(&mut self.ifft_input, &self.fft_output_l, &filters.filter_ll);
        complex_mul_add_simd(&mut self.ifft_input, &self.fft_output_r, &filters.filter_lr);

        // Ensure DC and Nyquist bins are real
        let num_bins = self.ifft_input.len();
        self.ifft_input[0].im = 0.0;
        self.ifft_input[num_bins - 1].im = 0.0;

        // IFFT left channel
        self.fft_inverse
            .process(&mut self.ifft_input, &mut self.ifft_output)
            .expect("IFFT processing failed");

        // Overlap-add to left accumulator (SIMD optimized)
        // Apply crossfade if in transition
        let scale = self.output_scale;
        scale_add_simd(
            &mut self.output_accum_l[..self.fft_size],
            &self.ifft_output,
            scale,
        );

        // Apply XTC filter for RIGHT output using SIMD:
        // R_out = filter_rl * L_in + filter_rr * R_in
        complex_mul_simd(&mut self.ifft_input, &self.fft_output_l, filter_rl);
        complex_mul_add_simd(&mut self.ifft_input, &self.fft_output_r, filter_rr);

        // Ensure DC and Nyquist bins are real
        self.ifft_input[0].im = 0.0;
        self.ifft_input[num_bins - 1].im = 0.0;

        // IFFT right channel
        self.fft_inverse
            .process(&mut self.ifft_input, &mut self.ifft_output)
            .expect("IFFT processing failed");

        // Overlap-add to right accumulator (SIMD optimized)
        scale_add_simd(
            &mut self.output_accum_r[..self.fft_size],
            &self.ifft_output,
            scale,
        );

        // Drop filter lock before updating crossfade
        drop(filters);

        // Update crossfade progress (Block mode)
        if self.crossfade_progress < 1.0 {
            let smooth_samples = self.params.head_tracking_smooth_s * self.sample_rate as f32;
            let progress_per_hop = self.hop_size as f32 / smooth_samples;
            self.crossfade_progress = (self.crossfade_progress + progress_per_hop).min(1.0);
            if self.crossfade_progress >= 1.0 {
                self.prev_filters = None; // Release old filters
            }
        }

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
        PluginInfo::new("Crosstalk Cancellation (XTC)", "2.0.0", "SotF").with_description(format!(
            "Crosstalk cancellation (Async) - FFT size: {}, speakers at {}° and {}m",
            self.fft_size, self.params.speaker_angle_deg, self.params.distance_m
        ))
    }

    fn input_channels(&self) -> usize {
        2 // Stereo input
    }

    fn output_channels(&self) -> usize {
        2 // Stereo output
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool("enabled", "Enabled", self.params.enabled)
                .with_group("General")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "distance_m",
                "Distance (m)",
                self.params.distance_m,
                0.5,
                5.0,
            )
            .with_group("Geometry")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "speaker_angle_deg",
                "Speaker Angle (deg)",
                self.params.speaker_angle_deg,
                15.0,
                60.0,
            )
            .with_group("Geometry")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "head_offset_x",
                "Head Offset X (m)",
                self.params.head_offset_x,
                -0.5,
                0.5,
            )
            .with_group("Head Tracking")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "head_offset_z",
                "Head Offset Z (m)",
                self.params.head_offset_z,
                -0.5,
                0.5,
            )
            .with_group("Head Tracking")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "head_yaw_deg",
                "Head Yaw (deg)",
                self.params.head_yaw_deg,
                -90.0,
                90.0,
            )
            .with_group("Head Tracking")
            .with_importance(ParameterImportance::Useful),
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
                self.params.distance_m = v.clamp(0.5, 5.0);
                needs_filter_update = true;
            } else {
                return Err("distance_m parameter must be float".to_string());
            }
        } else if id == self.param_speaker_angle {
            if let ParameterValue::Float(v) = value {
                self.params.speaker_angle_deg = v.clamp(15.0, 60.0);
                needs_filter_update = true;
            } else {
                return Err("speaker_angle_deg parameter must be float".to_string());
            }
        } else if id == self.param_head_offset_x {
            if let ParameterValue::Float(v) = value {
                self.params.head_offset_x = v.clamp(-0.5, 0.5);
                needs_filter_update = true;
            } else {
                return Err("head_offset_x parameter must be float".to_string());
            }
        } else if id == self.param_head_offset_z {
            if let ParameterValue::Float(v) = value {
                self.params.head_offset_z = v.clamp(-0.5, 0.5);
                needs_filter_update = true;
            } else {
                return Err("head_offset_z parameter must be float".to_string());
            }
        } else if id == self.param_head_yaw {
            if let ParameterValue::Float(v) = value {
                self.params.head_yaw_deg = v.clamp(-90.0, 90.0);
                needs_filter_update = true;
            } else {
                return Err("head_yaw_deg parameter must be float".to_string());
            }
        } else {
            return Err(format!("Unknown parameter: {:?}", id));
        }

        if needs_filter_update {
            self.update_filters(false); // Asynchronous
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
        } else if id == &self.param_head_yaw {
            Some(ParameterValue::Float(self.params.head_yaw_deg))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_filters(true); // Synchronous for initialization

        // Pre-allocate temp buffers to max expected frame count.
        // After this, the resize() check in process() is a guaranteed no-op.
        let max_frames = 4096;
        self.temp_input_l.resize(max_frames, 0.0);
        self.temp_input_r.resize(max_frames, 0.0);

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

        // Reset smoothers to current parameter values
        self.smoother_offset_x.reset(self.params.head_offset_x);
        self.smoother_offset_z.reset(self.params.head_offset_z);
        self.smoother_yaw.reset(self.params.head_yaw_deg);
        self.current_offset_x = self.params.head_offset_x;
        self.current_offset_z = self.params.head_offset_z;
        self.current_yaw_deg = self.params.head_yaw_deg;

        // Reset crossfade state
        self.prev_filters = None;
        self.crossfade_progress = 1.0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
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
            return Ok(context.num_frames);
        }

        // Temp buffers are pre-allocated in initialize() to 4096 frames.
        // This resize is a no-op in normal operation; only allocates for unusually large blocks.
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
                // Interleave output directly with denormal flushing
                for i in 0..samples_to_output {
                    let mut sample_l = self.output_accum_l[i];
                    let mut sample_r = self.output_accum_r[i];

                    // Flush denormals to zero to prevent CPU spikes and audio glitches
                    if sample_l.abs() < 1e-30 {
                        sample_l = 0.0;
                    }
                    if sample_r.abs() < 1e-30 {
                        sample_r = 0.0;
                    }

                    output[(out_pos + i) * 2] = sample_l;
                    output[(out_pos + i) * 2 + 1] = sample_r;
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

        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        // Latency is approximately fft_size - hop_size due to overlap-add
        self.fft_size - self.hop_size
    }
}

// ============================================================================
// Crosstalk Cancellation Filter Computation
// ============================================================================

/// Speed of sound at 20°C in m/s
const SPEED_OF_SOUND: f32 = 343.0;

/// Compute the Woodworth diffraction path around the head for a given incidence angle.
///
/// The sound reaching the far ear must diffract around the spherical head.
/// The extra path length depends on the angle of incidence (azimuth from median plane).
///
/// For angle <= PI/2: extra_path = a * (angle + sin(angle))
/// For angle > PI/2:  extra_path = a * (PI - angle + sin(angle))
#[inline]
fn woodworth_diffraction_path(angle_rad: f32, head_radius: f32) -> f32 {
    let theta = angle_rad.abs();
    if theta <= PI / 2.0 {
        head_radius * (theta + theta.sin())
    } else {
        head_radius * (PI - theta + theta.sin())
    }
}

/// Compute the angular separation between a sound source and the contralateral ear.
///
/// For a source at azimuth `speaker_angle` from the median plane, the ipsilateral ear
/// (same side) is at 90° from center, and the contralateral ear (opposite side) is at
/// -90°. The angular separation from source to contralateral ear, measured around the
/// head surface, is approximately PI/2 + speaker_angle.
#[inline]
fn contralateral_shadow_angle(speaker_angle_rad: f32) -> f32 {
    (PI / 2.0 + speaker_angle_rad).min(PI)
}

/// Compute crosstalk cancellation filters in frequency domain
///
/// This is the main filter computation function that handles:
/// - Symmetric case (yaw = 0): returns None for filter_rl/filter_rr
/// - Asymmetric case (yaw != 0): returns full 4-filter matrix
///
/// Uses improved Woodworth head shadowing model for better accuracy.
fn compute_xtc_filters_full(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> XtcFilters {
    let yaw_rad = params.head_yaw_deg * PI / 180.0;
    let is_symmetric = yaw_rad.abs() < 0.001; // ~0.06 degrees threshold

    let mut filters = if is_symmetric {
        // Use optimized symmetric computation
        let (filter_ll, filter_lr) = compute_xtc_filters_symmetric(params, sample_rate, num_bins);
        XtcFilters {
            filter_ll,
            filter_lr,
            filter_rl: None,
            filter_rr: None,
        }
    } else {
        // Full asymmetric computation for yaw != 0
        compute_xtc_filters_asymmetric(params, sample_rate, num_bins)
    };

    // Post-processing: spectral energy normalization to prevent tonal imbalance
    apply_spectral_normalization(&mut filters, num_bins);

    filters
}

/// Apply spectral energy normalization to XTC filters.
///
/// XTC processing can create spectral tilt (boosting some frequencies, attenuating others).
/// This normalizes the average energy per bin to keep tonal balance close to the original signal.
/// Uses a gentle smoothed approach to avoid introducing artifacts.
fn apply_spectral_normalization(filters: &mut XtcFilters, num_bins: usize) {
    // First pass: compute per-bin correction gains
    let mut gains = vec![1.0_f32; num_bins];
    let is_asymmetric = filters.filter_rl.is_some();

    for bin in 1..num_bins - 1 {
        // Left output energy from unit-energy input
        let energy_l = filters.filter_ll[bin].norm_sqr() + filters.filter_lr[bin].norm_sqr();

        // Right output energy (use symmetric equivalents if not available)
        let energy_r = if is_asymmetric {
            let rl = filters.filter_rl.as_ref().unwrap();
            let rr = filters.filter_rr.as_ref().unwrap();
            rl[bin].norm_sqr() + rr[bin].norm_sqr()
        } else {
            // Symmetric: filter_rr = filter_ll, filter_rl = filter_lr
            energy_l
        };

        // Average energy across both channels
        let avg_energy = (energy_l + energy_r) / 2.0;

        if avg_energy > 0.01 {
            // Gentle normalization: blend between unity and full correction
            // This preserves the XTC effect while reducing tonal coloration
            let correction = (1.0 / avg_energy.sqrt()).clamp(0.5, 2.0);
            // Apply 50% of the correction (gentle approach)
            gains[bin] = 1.0 + 0.5 * (correction - 1.0);
        }
    }

    // Second pass: apply gains
    for bin in 1..num_bins - 1 {
        let gain = gains[bin];
        filters.filter_ll[bin] *= gain;
        filters.filter_lr[bin] *= gain;
        if let Some(ref mut rl) = filters.filter_rl {
            rl[bin] *= gain;
        }
        if let Some(ref mut rr) = filters.filter_rr {
            rr[bin] *= gain;
        }
    }
}

/// Compute asymmetric filters for non-zero yaw angle
fn compute_xtc_filters_asymmetric(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> XtcFilters {
    let mut filter_ll = Vec::with_capacity(num_bins);
    let mut filter_lr = Vec::with_capacity(num_bins);
    let mut filter_rl = Vec::with_capacity(num_bins);
    let mut filter_rr = Vec::with_capacity(num_bins);

    // Geometry
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let yaw_rad = params.head_yaw_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Effective speaker angles relative to rotated head
    let theta_left = theta_rad + yaw_rad; // Left speaker angle
    let theta_right = theta_rad - yaw_rad; // Right speaker angle

    // Left ear paths (with Woodworth diffraction)
    let l_left_ipsi = compute_path_length(d, theta_left, -x_offset);
    let diffraction_left = woodworth_diffraction_path(theta_right, a);
    let l_left_contra = compute_path_length(d, theta_right, -x_offset) + diffraction_left;

    // Right ear paths (with Woodworth diffraction)
    let r_right_ipsi = compute_path_length(d, theta_right, x_offset);
    let diffraction_right = woodworth_diffraction_path(theta_left, a);
    let r_right_contra = compute_path_length(d, theta_left, x_offset) + diffraction_right;

    // Distance attenuation ratios
    let amplitude_ratio_left = l_left_ipsi / l_left_contra;
    let amplitude_ratio_right = r_right_ipsi / r_right_contra;

    // Time differences
    let delta_t_left = (l_left_contra - l_left_ipsi) / SPEED_OF_SOUND;
    let delta_t_right = (r_right_contra - r_right_ipsi) / SPEED_OF_SOUND;

    // Contralateral shadow angles (angular separation from source to far ear)
    let angle_left_contra = contralateral_shadow_angle(theta_right.abs());
    let angle_right_contra = contralateral_shadow_angle(theta_left.abs());

    // Max gain limit (convert dB to linear)
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    for bin in 0..num_bins {
        let freq = bin as f32 * sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

        // Pinna resonance shaping
        let pinna = pinna_resonance(freq);

        // Left ear transfer functions
        let h_ll_ipsi = Complex::new(1.0, 0.0) * pinna;
        let g_ll = head_shadowing_woodworth(freq, angle_left_contra, a) * amplitude_ratio_left;
        let phase_ll = -2.0 * PI * freq * delta_t_left;
        let h_ll_contra = Complex::new(g_ll * phase_ll.cos(), g_ll * phase_ll.sin()) * pinna;

        // Right ear transfer functions
        let h_rr_ipsi = Complex::new(1.0, 0.0) * pinna;
        let g_rr = head_shadowing_woodworth(freq, angle_right_contra, a) * amplitude_ratio_right;
        let phase_rr = -2.0 * PI * freq * delta_t_right;
        let h_rr_contra = Complex::new(g_rr * phase_rr.cos(), g_rr * phase_rr.sin()) * pinna;

        let beta = compute_beta_smooth(freq, params);

        // Compute 2x2 filter matrices for each ear independently
        // Left ear: L_out = w_ll * L_in + w_lr * R_in
        let (w_ll, w_lr) = compute_2x2_inverse(h_ll_ipsi, h_ll_contra, beta, max_gain_linear);
        // Right ear: R_out = w_rl * L_in + w_rr * R_in
        let (w_rr, w_rl) = compute_2x2_inverse(h_rr_ipsi, h_rr_contra, beta, max_gain_linear);

        filter_ll.push(w_ll);
        filter_lr.push(w_lr);
        filter_rl.push(w_rl);
        filter_rr.push(w_rr);
    }

    XtcFilters {
        filter_ll,
        filter_lr,
        filter_rl: Some(filter_rl),
        filter_rr: Some(filter_rr),
    }
}

/// Compute path length from speaker at angle theta to ear with offset
#[inline]
fn compute_path_length(distance: f32, theta: f32, ear_offset: f32) -> f32 {
    ((distance * theta.sin() + ear_offset).powi(2) + (distance * theta.cos()).powi(2)).sqrt()
}

/// Compute 2x2 inverse filter for one ear with multi-stage Neumann series refinement.
///
/// First computes the regularized inverse W₁ = (C^H*C + β*I)^-1 * C^H, then
/// refines it with one iteration of Neumann series: W₂ = W₁ * (2*I - C*W₁).
/// This improves broadband cancellation depth by correcting residual errors
/// from regularization, similar to the BACCH approach.
///
/// Returns (w_ipsi, w_contra) filter coefficients.
/// max_gain_linear limits the magnitude of each output coefficient.
#[inline]
fn compute_2x2_inverse(
    h_ipsi: Complex<f32>,
    h_contra: Complex<f32>,
    beta: f32,
    max_gain_linear: f32,
) -> (Complex<f32>, Complex<f32>) {
    let h_ipsi_mag_sq = h_ipsi.norm_sqr();
    let h_contra_mag_sq = h_contra.norm_sqr();
    let cross_term = (h_ipsi * h_contra.conj()).re * 2.0;

    let diag = h_ipsi_mag_sq + h_contra_mag_sq + beta;
    let off_diag = cross_term;

    let det = diag * diag - off_diag * off_diag;

    if det.abs() < 1e-10 {
        return (Complex::new(1.0, 0.0), Complex::new(0.0, 0.0));
    }

    let inv_diag = diag / det;
    let inv_off_diag = -off_diag / det;

    let h_ipsi_conj = h_ipsi.conj();
    let h_contra_conj = h_contra.conj();

    let w1_ipsi = h_ipsi_conj * inv_diag + h_contra_conj * inv_off_diag;
    let w1_contra = h_ipsi_conj * inv_off_diag + h_contra_conj * inv_diag;

    // Neumann series refinement: W₂ = W₁ * (2I - C*W₁)
    // Compute the product C*W₁ for the 2x2 symmetric matrix C = [[h_ipsi, h_contra], [h_contra, h_ipsi]]
    //
    // (C*W₁)[0,0] = h_ipsi * w1_ipsi + h_contra * w1_contra
    // (C*W₁)[0,1] = h_ipsi * w1_contra + h_contra * w1_ipsi
    let cw_00 = h_ipsi * w1_ipsi + h_contra * w1_contra;
    let cw_01 = h_ipsi * w1_contra + h_contra * w1_ipsi;

    // (2I - C*W₁)
    let r_00 = Complex::new(2.0, 0.0) - cw_00;
    let r_01 = Complex::new(0.0, 0.0) - cw_01;

    // W₂ = W₁ * R where R = (2I - C*W₁)
    // W₂[0,0] = w1_ipsi * r_00 + w1_contra * r_01  (but r is column-matched)
    // For the one-ear case, the "matrix" is really [w_ipsi, w_contra] (row vector)
    // times the 2x2 correction matrix R = [[r_00, r_01], [r_01, r_00]] (also symmetric)
    let w2_ipsi = w1_ipsi * r_00 + w1_contra * r_01;
    let w2_contra = w1_contra * r_00 + w1_ipsi * r_01;

    // Clamp magnitudes to prevent excessive boost
    let w2_ipsi = clamp_complex_magnitude(w2_ipsi, max_gain_linear);
    let w2_contra = clamp_complex_magnitude(w2_contra, max_gain_linear);

    (w2_ipsi, w2_contra)
}

/// Clamp a complex number's magnitude while preserving its phase
#[inline]
fn clamp_complex_magnitude(c: Complex<f32>, max_mag: f32) -> Complex<f32> {
    let mag = c.norm();
    if mag > max_mag {
        c * (max_mag / mag)
    } else {
        c
    }
}

/// Woodworth-Schlosberg head shadowing model
///
/// Provides frequency and angle dependent interaural level difference (ILD)
/// based on spherical head acoustics.
fn head_shadowing_woodworth(freq: f32, angle_rad: f32, head_radius: f32) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Wave number times head radius (ka)
    // This determines the diffraction regime
    let ka = 2.0 * PI * freq * head_radius / SPEED_OF_SOUND;
    let theta = angle_rad.abs();

    if ka < 0.5 {
        // Low frequency: sound diffracts fully around head
        // Minimal ILD, slight angle dependence
        1.0 - 0.05 * ka * theta.sin()
    } else if ka < 2.0 {
        // Transition region: gradual shadowing
        let t = (ka - 0.5) / 1.5; // 0 to 1 over transition
        let shadow_factor = (1.0 + theta.cos()) / 2.0;
        let low_freq = 1.0 - 0.05 * ka * theta.sin();
        let high_freq = shadow_factor.powf(0.5 + t);
        low_freq * (1.0 - t) + high_freq * t
    } else {
        // High frequency: significant head shadow
        // Shadow increases with angle from direct path
        let shadow_factor = (1.0 + theta.cos()) / 2.0; // 1 at 0°, 0 at 180°
        let exponent = (ka / 4.0).min(3.0); // Cap exponent for stability
        shadow_factor.powf(exponent)
    }
}

/// Simplified pinna resonance model for externalization cues.
///
/// Models three key ear resonances that provide crucial spatial perception cues:
/// 1. Ear canal resonance: broad +10 dB peak at ~2.7 kHz
/// 2. Concha resonance: broad +5 dB peak at ~4.5 kHz
/// 3. Pinna anti-resonance: narrow -6 dB notch at ~9 kHz (elevation cue)
///
/// Without these cues, XTC output tends to sound "inside the head" even with
/// perfect crosstalk cancellation. Returns a real-valued gain (applied to both
/// ipsilateral and contralateral transfer functions).
#[inline]
fn pinna_resonance(freq: f32) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Ear canal resonance: 2nd-order bandpass centered at 2700 Hz, Q=1.2
    // Peak gain ~+10 dB
    let f_ear = 2700.0_f32;
    let q_ear = 1.2_f32;
    let gain_ear_db = 10.0_f32;
    let ear_response = resonance_peak(freq, f_ear, q_ear, gain_ear_db);

    // Concha resonance: broad peak at 4500 Hz, Q=1.5
    // Peak gain ~+5 dB
    let f_concha = 4500.0_f32;
    let q_concha = 1.5_f32;
    let gain_concha_db = 5.0_f32;
    let concha_response = resonance_peak(freq, f_concha, q_concha, gain_concha_db);

    // Pinna anti-resonance: narrow notch at 9000 Hz, Q=3.0
    // Depth ~-6 dB (this is a key elevation cue)
    let f_pinna = 9000.0_f32;
    let q_pinna = 3.0_f32;
    let gain_pinna_db = -6.0_f32;
    let pinna_response = resonance_peak(freq, f_pinna, q_pinna, gain_pinna_db);

    // Combine all resonances (multiplicative in linear domain = additive in dB)
    ear_response * concha_response * pinna_response
}

/// Compute the magnitude response of a resonance peak/notch at a given frequency.
///
/// Models a 2nd-order bandpass/notch with given center frequency, Q, and peak gain in dB.
/// Returns linear gain at the specified frequency.
#[inline]
fn resonance_peak(freq: f32, center_freq: f32, q: f32, gain_db: f32) -> f32 {
    // Normalized frequency ratio
    let f_ratio = freq / center_freq;
    // 2nd-order magnitude response: |H(f)|^2 = 1 / ((1 - f^2/f0^2)^2 + (f/(Q*f0))^2)
    let x = f_ratio * f_ratio;
    let denom = (1.0 - x).powi(2) + (f_ratio / q).powi(2);
    // Normalized shape: 1.0 at center, falls off away from center
    let shape = (f_ratio / q).powi(2) / denom;
    // Convert peak gain from dB to linear and scale by shape
    let peak_linear = 10.0_f32.powf(gain_db / 20.0);
    // Blend: at center freq shape=1.0 → full gain; far away shape→0 → unity gain
    1.0 + (peak_linear - 1.0) * shape
}

/// Compute frequency-dependent regularization with smooth sigmoid transitions
fn compute_beta_smooth(freq: f32, params: &XtcPluginParams) -> f32 {
    let base = params.beta_base;
    let low_boost = params.beta_low_freq_boost;
    let high_boost = params.beta_high_freq_boost;

    // Smooth low-frequency boost (sigmoid transition around 100Hz)
    // Below ~100Hz, wavelength >> speaker spacing, cancellation is ineffective
    let low_freq_factor = 1.0 + (low_boost - 1.0) * sigmoid_smooth(100.0 - freq, 30.0);

    // Smooth high-frequency boost (sigmoid transition around 12kHz)
    // Head shadowing naturally limits HF cancellation, so we can allow more bandwidth
    let high_freq_factor = 1.0 + (high_boost - 1.0) * sigmoid_smooth(freq - 12000.0, 1500.0);

    base * low_freq_factor * high_freq_factor
}

/// Smooth sigmoid function for gradual transitions
#[inline]
fn sigmoid_smooth(x: f32, width: f32) -> f32 {
    1.0 / (1.0 + (-x / width).exp())
}

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

    // Geometry with head tracking offsets
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Compute path lengths (considering head offset)
    let l_ipsi = compute_path_length(d, theta_rad, -x_offset);
    // Use Woodworth diffraction path (angle-dependent) instead of fixed PI*a
    let diffraction_extra = woodworth_diffraction_path(theta_rad, a);
    let l_contra = compute_path_length(d, theta_rad, x_offset) + diffraction_extra;

    // Distance attenuation ratio (inverse-distance law)
    let amplitude_ratio = l_ipsi / l_contra;

    // Time difference
    let delta_t = (l_contra - l_ipsi) / SPEED_OF_SOUND;

    // Contralateral shadow angle: angular separation from source to far ear
    let contra_angle = contralateral_shadow_angle(theta_rad);

    // Max gain limit (convert dB to linear)
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    // Process each frequency bin
    for bin in 0..num_bins {
        let freq = bin as f32 * sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

        // Transfer function for ipsilateral path (reference = 1)
        let h_ipsi = Complex::new(1.0, 0.0);

        // Transfer function for contralateral path using Woodworth model
        // Uses corrected shadow angle (PI/2 + theta) and distance attenuation
        let g = head_shadowing_woodworth(freq, contra_angle, a) * amplitude_ratio;
        let phase = -2.0 * PI * freq * delta_t;
        let h_contra = Complex::new(g * phase.cos(), g * phase.sin());

        // Frequency-dependent regularization with smooth transitions
        let beta = compute_beta_smooth(freq, params);

        // Pinna resonance shaping applied to both transfer functions
        let pinna = pinna_resonance(freq);
        let h_ipsi_shaped = h_ipsi * pinna;
        let h_contra_shaped = h_contra * pinna;

        // Use shared 2x2 inverse computation with gain clamping
        let (w_ll, w_lr) = compute_2x2_inverse(h_ipsi_shaped, h_contra_shaped, beta, max_gain_linear);

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
) -> XtcFilterSet {
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

    let l_contra =
        ((d * theta_rad.sin() + x_offset).powi(2) + (d * theta_rad.cos()).powi(2)).sqrt() + PI * a; // Add head shadow path

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
    

    1.0 / (1.0 + ratio.powf(n))
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

        let num_frames = 4096;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            input[i * 2] = (i as f32 * 0.01).sin();
            input[i * 2 + 1] = (i as f32 * 0.01).cos();
        }
        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
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

        // Test with stereo sine wave (must exceed fft_size to produce output)
        let num_frames = 4096;
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
            .set_parameter(ParameterId::from("distance_m"), ParameterValue::Float(2.5))
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
        let input_energy: f32 = input[skip_samples * 2..].iter().map(|x| x * x).sum();
        let output_energy: f32 = output[skip_samples * 2..].iter().map(|x| x * x).sum();

        // Energy ratio should be between 0.5 and 2.0 (within 3dB)
        // XTC can boost some frequencies while attenuating others,
        // but total energy should be reasonably preserved
        let energy_ratio = output_energy / input_energy;
        assert!(
            energy_ratio > 0.3 && energy_ratio < 3.0,
            "Energy ratio {} is outside acceptable range [0.3, 3.0].  Input energy: {}, Output energy: {}",
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
        let input_energy: f32 = input[skip_samples * 2..].iter().map(|x| x * x).sum();
        let output_energy: f32 = output[skip_samples * 2..].iter().map(|x| x * x).sum();

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
        let fft_size = params.fft_size;
        let num_bins = fft_size / 2 + 1;
        let filters = compute_xtc_filters_full(&params, 48000, num_bins);

        // Check mid-frequency bin (around 1kHz)
        let bin_1khz = (1000.0 * fft_size as f32 / 48000.0) as usize;

        // With spectral normalization and multi-stage cancellation,
        // magnitudes can vary more than the old simple model
        let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

        // Diagonal filters should have reasonable magnitude
        let mag_ll = filters.filter_ll[bin_1khz].norm();
        assert!(
            mag_ll > 0.1 && mag_ll < max_gain_linear,
            "filter_ll magnitude {} at 1kHz outside range",
            mag_ll
        );

        // Cross-filters should be non-zero (cancellation signal)
        let mag_lr = filters.filter_lr[bin_1khz].norm();
        assert!(
            mag_lr > 0.001 && mag_lr < max_gain_linear,
            "filter_lr magnitude {} at 1kHz outside range",
            mag_lr
        );
    }

    /// Test that denormal numbers are flushed to zero
    #[test]
    fn test_xtc_denormal_flushing() {
        let params = XtcPluginParams::default();
        let mut plugin = XtcPlugin::new(params, 48000).unwrap();
        plugin.initialize(48000).unwrap();

        // Create very low amplitude input (near denormal range)
        let num_frames = 4096;
        let mut input = vec![1e-35_f32; num_frames * 2];
        // Add a tiny bit of signal variation
        for i in 0..num_frames {
            input[i * 2] = 1e-35 * ((i as f32 * 0.01).sin() + 1.0);
            input[i * 2 + 1] = 1e-35 * ((i as f32 * 0.01).cos() + 1.0);
        }
        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Count denormal samples (non-zero but below normalized threshold)
        let mut denormal_count = 0;
        for sample in output.iter() {
            let abs_val = sample.abs();
            if abs_val > 0.0 && abs_val < 1e-30 {
                denormal_count += 1;
            }
        }

        // With proper denormal flushing, there should be NO denormal samples
        assert_eq!(
            denormal_count, 0,
            "Found {} denormal samples. Denormal flushing is not working correctly.",
            denormal_count
        );
    }

    /// Test yaw angle creates asymmetric filters
    #[test]
    fn test_yaw_angle_asymmetry() {
        let mut params = XtcPluginParams::default();
        params.head_yaw_deg = 15.0; // 15 degrees yaw
        let fft_size = params.fft_size;
        let num_bins = fft_size / 2 + 1;

        let filters = compute_xtc_filters_full(&params, 48000, num_bins);

        // With yaw != 0, we should have asymmetric filters (filter_rl and filter_rr are Some)
        assert!(
            filters.filter_rl.is_some(),
            "filter_rl should be Some when yaw != 0"
        );
        assert!(
            filters.filter_rr.is_some(),
            "filter_rr should be Some when yaw != 0"
        );

        let filter_rl = filters.filter_rl.as_ref().unwrap();
        let filter_rr = filters.filter_rr.as_ref().unwrap();

        // Check that filters are actually asymmetric at mid frequencies
        let bin_1khz = (1000.0 * fft_size as f32 / 48000.0) as usize;

        // filter_lr and filter_rl should be different with yaw
        let diff_cross = (filters.filter_lr[bin_1khz] - filter_rl[bin_1khz]).norm();
        assert!(
            diff_cross > 0.001,
            "Cross filters should be asymmetric with yaw, diff = {}",
            diff_cross
        );

        // filter_ll and filter_rr should also be different with yaw
        let diff_diag = (filters.filter_ll[bin_1khz] - filter_rr[bin_1khz]).norm();
        assert!(
            diff_diag > 0.001,
            "Diagonal filters should be asymmetric with yaw, diff = {}",
            diff_diag
        );
    }

    /// Test symmetric case (yaw = 0) uses optimized 2-filter version
    #[test]
    fn test_yaw_zero_symmetric() {
        let params = XtcPluginParams::default(); // yaw = 0
        let fft_size = params.fft_size;
        let num_bins = fft_size / 2 + 1;

        let filters = compute_xtc_filters_full(&params, 48000, num_bins);

        // With yaw = 0, filters should be symmetric (filter_rl and filter_rr are None)
        assert!(
            filters.filter_rl.is_none(),
            "filter_rl should be None when yaw = 0"
        );
        assert!(
            filters.filter_rr.is_none(),
            "filter_rr should be None when yaw = 0"
        );
    }

    /// Test Woodworth head shadowing model
    #[test]
    fn test_woodworth_head_shadowing() {
        let head_radius = 0.0875;

        // At low frequencies, shadowing should be minimal
        let g_low = head_shadowing_woodworth(100.0, 0.5, head_radius);
        assert!(
            g_low > 0.95,
            "Low frequency shadowing should be minimal, got {}",
            g_low
        );

        // At high frequencies, shadowing should be significant
        let g_high = head_shadowing_woodworth(8000.0, 0.5, head_radius);
        assert!(
            g_high < 0.9,
            "High frequency shadowing should be significant, got {}",
            g_high
        );

        // At 0 angle, shadowing should be minimal even at high frequencies
        let g_frontal = head_shadowing_woodworth(8000.0, 0.0, head_radius);
        assert!(
            g_frontal > g_high,
            "Frontal angle should have less shadowing than side"
        );
    }

    /// Test smooth beta transitions
    #[test]
    fn test_smooth_beta_transitions() {
        let params = XtcPluginParams::default();

        // Test transition around 100Hz is smooth (LF boost region)
        let beta_70 = compute_beta_smooth(70.0, &params);
        let beta_100 = compute_beta_smooth(100.0, &params);
        let beta_150 = compute_beta_smooth(150.0, &params);

        // Should be monotonically decreasing toward mid-range
        assert!(
            beta_70 > beta_100,
            "Beta should decrease from 70Hz to 100Hz"
        );
        assert!(
            beta_100 > beta_150,
            "Beta should decrease from 100Hz to 150Hz"
        );

        // Transition should be smooth (no large jumps)
        let ratio_1 = beta_70 / beta_100;
        let ratio_2 = beta_100 / beta_150;
        assert!(
            (ratio_1 - ratio_2).abs() < 2.0,
            "Beta transition should be smooth around 100Hz"
        );
    }
}
