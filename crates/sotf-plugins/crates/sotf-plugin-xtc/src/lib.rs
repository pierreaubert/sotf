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

mod config;
mod filters;
pub mod params;
mod reflections;
#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests;
pub mod validation;

pub use config::*;
use filters::{
    HrtfTransferFunctions, XtcFilters, compute_geometry_cache,
    compute_xtc_filters_full_with_cache_and_hrtf,
};
use reflections::{
    RoomReflectionData, build_reflection_data_image_source, build_reflection_data_ir,
};

use crate::params::PARAMS as XT;
use arc_swap::ArcSwap;
use math_audio_dsp::stft::generate_hann_window;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainData, AutoGainParams};
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{
    complex_mul_add_simd, complex_mul_simd, deinterleave_stereo, flush_denormals_inplace,
    window_mul_simd,
};
use std::any::Any;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Diagnostic data from XTC plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XtcData {
    pub auto_gain: AutoGainData,
    pub limiter_envelope: f32,
}

impl Default for XtcData {
    fn default() -> Self {
        Self {
            auto_gain: AutoGainData::default(),
            limiter_envelope: 1.0,
        }
    }
}

// ============================================================================
// HRTF/SOFA file loading for XTC
// ============================================================================

use sotf_host::sofa::{SofaFile, SourcePosition};

/// Load HRTF data from a SOFA file and compute frequency-domain transfer functions
/// for the XTC plant matrix at the configured speaker angles.
///
/// For each speaker position (left at +angle, right at -angle), we extract the
/// HRTF for both ears, giving us a full 2x2 plant matrix C(f).
fn load_hrtf_for_xtc(
    hrtf_path: &str,
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> Result<Option<HrtfTransferFunctions>, String> {
    let path = std::path::Path::new(hrtf_path);
    if !path.exists() {
        return Err(format!("HRTF file not found: {}", hrtf_path));
    }

    let sofa = SofaFile::load(path)?;

    // Reject SOFA files with mismatched sample rate — using unmatched
    // HRTF data shifts all spectral features and corrupts the plant matrix.
    if let Some(sofa_sr) = sofa.data_sample_rate
        && (sofa_sr - sample_rate as f32).abs() > 1.0
    {
        return Err(format!(
            "SOFA sample rate ({} Hz) differs from plugin sample rate ({} Hz). \
             Resample the SOFA file or match sample rates.",
            sofa_sr, sample_rate
        ));
    }

    // Speaker positions: left speaker at +angle, right speaker at -angle
    // (azimuth in SOFA convention, elevation 0, distance = speaker distance)
    let left_speaker = SourcePosition::new(params.speaker_angle_deg, 0.0, params.distance_m);
    let right_speaker = SourcePosition::new(-params.speaker_angle_deg, 0.0, params.distance_m);

    // Get HRTF for left speaker position (contains left ear + right ear responses)
    let hrtf_left_speaker = sofa
        .get_hrtf_at_position(&left_speaker)
        .ok_or_else(|| "No HRTF measurement found for left speaker angle".to_string())?;

    // Get HRTF for right speaker position
    let hrtf_right_speaker = sofa
        .get_hrtf_at_position(&right_speaker)
        .ok_or_else(|| "No HRTF measurement found for right speaker angle".to_string())?;

    // FFT the impulse responses to get frequency-domain transfer functions
    let fft_size = (num_bins - 1) * 2;
    let mut planner = realfft::RealFftPlanner::new();
    let fft_forward = planner.plan_fft_forward(fft_size);

    // Helper: FFT an IR, zero-padding or truncating to fft_size
    let fft_ir = |ir: &[f32]| -> Vec<Complex<f32>> {
        let mut padded = vec![0.0_f32; fft_size];
        let copy_len = ir.len().min(fft_size);
        padded[..copy_len].copy_from_slice(&ir[..copy_len]);

        let mut output = vec![Complex::new(0.0, 0.0); num_bins];
        fft_forward
            .process(&mut padded, &mut output)
            .expect("FFT processing failed");
        output
    };

    // Plant matrix:
    //   C = [[h_ll, h_lr],    Speaker L->EarL, Speaker R->EarL
    //        [h_rl, h_rr]]    Speaker L->EarR, Speaker R->EarR
    //
    // Left speaker HRTF: ir_left = L speaker -> L ear, ir_right = L speaker -> R ear
    // Right speaker HRTF: ir_left = R speaker -> L ear, ir_right = R speaker -> R ear
    let h_ll = fft_ir(&hrtf_left_speaker.ir_left); // Speaker L -> Left ear
    let h_rl = fft_ir(&hrtf_left_speaker.ir_right); // Speaker L -> Right ear
    let h_lr = fft_ir(&hrtf_right_speaker.ir_left); // Speaker R -> Left ear
    let h_rr = fft_ir(&hrtf_right_speaker.ir_right); // Speaker R -> Right ear

    Ok(Some(HrtfTransferFunctions {
        h_ll,
        h_lr,
        h_rl,
        h_rr,
    }))
}

// ============================================================================
// Helper functions for Optimization 4: Room reflection caching
// ============================================================================

use std::collections::hash_map::DefaultHasher;

/// Compute a hash of room-related parameters for cache invalidation.
///
/// Includes all parameters that affect room reflection computation.
fn compute_room_params_hash(params: &XtcPluginParams) -> u64 {
    let mut hasher = DefaultHasher::new();
    params.room_width_m.to_bits().hash(&mut hasher);
    params.room_depth_m.to_bits().hash(&mut hasher);
    params.wall_absorption.to_bits().hash(&mut hasher);
    params.distance_m.to_bits().hash(&mut hasher);
    params.speaker_angle_deg.to_bits().hash(&mut hasher);
    params.head_offset_x.to_bits().hash(&mut hasher);
    params.head_offset_z.to_bits().hash(&mut hasher);
    params.head_radius_m.to_bits().hash(&mut hasher);
    params.room_reflections_enabled.hash(&mut hasher);
    params.reflection_beta_boost.to_bits().hash(&mut hasher);
    if let Some(ref ir_path) = params.room_ir_file {
        ir_path.hash(&mut hasher);
    }
    if let Some(ref hrtf_path) = params.hrtf_file {
        hrtf_path.hash(&mut hasher);
    }
    params.kappa_target.to_bits().hash(&mut hasher);
    hasher.finish()
}

/// Compute room reflection data if enabled.
///
/// Returns None if room reflections are disabled.
///
/// `fft_forward` is passed to `build_reflection_data_ir` to reuse the pre-planned
/// FFT instead of creating a fresh planner on every call (Optimization 4).
fn compute_room_reflection_data(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
    fft_forward: Option<Arc<dyn RealToComplex<f32>>>,
) -> Option<Arc<RoomReflectionData>> {
    if !params.room_reflections_enabled {
        return None;
    }

    let data = if let Some(ref ir_path) = params.room_ir_file {
        build_reflection_data_ir(ir_path, sample_rate, num_bins, fft_forward).ok()?
    } else {
        build_reflection_data_image_source(params, sample_rate, num_bins)
    };

    Some(Arc::new(data))
}

// ============================================================================
// Plugin Implementation
// ============================================================================

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

    /// Output accumulator for overlap-add (flat interleaved ring buffer)
    /// Layout: [L0, R0, L1, R1, ...]
    /// Buffer size in frames is always power-of-2 (4 * fft_size) for efficient masking
    output_accumulator: Vec<f32>,
    /// Bitmask for ring buffer frame index (buffer_frames - 1)
    output_accumulator_mask: usize,
    /// Number of valid frames in output accumulator
    output_accumulator_fill: usize,
    /// Next frame position to add a block (tracks overlap-add offset)
    next_add_position: usize,
    /// Current read frame position in the output accumulator ring buffer
    output_read_position: usize,

    /// Temporary buffers for block processing (avoid per-call allocation)
    temp_input_l: Vec<f32>,
    temp_input_r: Vec<f32>,

    /// Working buffers for FFT
    fft_buffer: Vec<f32>,
    fft_output_l: Vec<Complex<f32>>,
    fft_output_r: Vec<Complex<f32>>,
    ifft_input: Vec<Complex<f32>>,
    ifft_output: Vec<f32>,

    /// Working buffer for crossfade: holds IFFT of prev_filters result
    prev_ifft_output: Vec<f32>,

    /// Thread-safe crosstalk cancellation filters (lock-free via ArcSwap)
    filters: Arc<ArcSwap<XtcFilters>>,

    /// Cached filter snapshot loaded once per process() call (avoids per-frame ArcSwap::load)
    cached_current_filters: Arc<XtcFilters>,

    /// Previous filter snapshot for crossfading (Block mode)
    prev_filters: Option<Arc<XtcFilters>>,

    /// Crossfade progress (0.0 = prev, 1.0 = current)
    crossfade_progress: f32,

    /// Cached progress increment per STFT hop (recomputed in update_filters)
    progress_per_hop: f32,

    /// Loaded HRTF transfer functions (from SOFA file)
    hrtf_transfer_functions: Option<HrtfTransferFunctions>,

    /// Cached room reflection data (Optimization 4)
    room_reflection_cache: Option<Arc<RoomReflectionData>>,

    /// Hash of room-related parameters for cache invalidation (Optimization 4)
    room_params_hash: u64,

    /// Auto-gain compensation to match output loudness to input
    auto_gain: Option<AutoGain>,

    /// Per-sample limiter envelope (0.0..=1.0). Smooth attack and release.
    /// Prevents output from exceeding ±0.95 after XTC filter summation + auto-gain.
    limiter_envelope: f32,

    /// Per-sample attack coefficient for the limiter (~0.2ms time constant).
    limiter_attack_coeff: f32,

    /// Per-sample release coefficient for the limiter (~50ms release).
    limiter_release_coeff: f32,

    /// Initial latency counter to ensure OLA buffer is primed before output
    latency_filled: usize,

    /// Diagnostic data cache (Real-time safe)
    cache: RealTimeCache<XtcData>,

    /// Counter to throttle diagnostic cache updates
    cache_update_counter: usize,

    cached_parameters: Vec<Parameter>,
}

// ============================================================================
// Free-function filter helpers (avoid borrow checker issues with &mut self)
// ============================================================================

/// Apply XTC filter for left channel: ifft_input = filter_ll * fft_l + filter_lr * fft_r
#[inline(always)]
fn apply_filter_left(
    ifft_input: &mut [Complex<f32>],
    fft_l: &[Complex<f32>],
    fft_r: &[Complex<f32>],
    filters: &XtcFilters,
) {
    complex_mul_simd(ifft_input, fft_l, &filters.filter_ll);
    complex_mul_add_simd(ifft_input, fft_r, &filters.filter_lr);
    let n = ifft_input.len();
    ifft_input[0].im = 0.0;
    ifft_input[n - 1].im = 0.0;
}

/// Apply XTC filter for right channel: ifft_input = filter_rl * fft_l + filter_rr * fft_r
/// Uses symmetric shortcuts when is_symmetric is true.
#[inline(always)]
fn apply_filter_right(
    ifft_input: &mut [Complex<f32>],
    fft_l: &[Complex<f32>],
    fft_r: &[Complex<f32>],
    filters: &XtcFilters,
) {
    let (filter_rl, filter_rr) = if filters.is_symmetric {
        (&filters.filter_lr, &filters.filter_ll)
    } else {
        (
            filters.filter_rl.as_ref().unwrap(),
            filters.filter_rr.as_ref().unwrap(),
        )
    };
    complex_mul_simd(ifft_input, fft_l, filter_rl);
    complex_mul_add_simd(ifft_input, fft_r, filter_rr);
    let n = ifft_input.len();
    ifft_input[0].im = 0.0;
    ifft_input[n - 1].im = 0.0;
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

        // Validate IR file path if provided
        if let Some(ref ir_path) = params
            .room_ir_file
            .as_ref()
            .filter(|p| !std::path::Path::new(p.as_str()).exists())
        {
            return Err(format!("Room IR file not found: {}", ir_path));
        }

        let fft_size = params.fft_size;
        let hop_size = fft_size / 4; // 75% overlap

        // Create FFT planners
        let mut planner = RealFftPlanner::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        // Periodic Hann window for STFT
        let analysis_window = generate_hann_window(fft_size);

        // Combined scale factor: COLA normalization / FFT size
        // For 75% overlap dual-windowing Hann, Sum(w^2) = 1.5.
        // scale = 1.0 / (1.5 * N).
        let output_scale = 1.0 / (fft_size as f32 * 1.5);

        // Compute frequency-domain filters
        let num_bins = fft_size / 2 + 1;

        // Compute initial room reflection data if enabled (Optimization 4)
        let room_params_hash = compute_room_params_hash(&params);
        let room_reflection_cache = if params.room_reflections_enabled {
            // Pass the pre-planned FFT to avoid re-creating the planner (Optimization 4)
            compute_room_reflection_data(&params, sample_rate, num_bins, Some(fft_forward.clone()))
        } else {
            None
        };

        // Load HRTF file if specified
        let hrtf_transfer_functions = if let Some(ref hrtf_path) = params.hrtf_file {
            load_hrtf_for_xtc(hrtf_path, &params, sample_rate, num_bins)?
        } else {
            None
        };

        // Compute geometry cache (Optimization 3)
        let cache = compute_geometry_cache(&params, sample_rate, num_bins);

        let filters = compute_xtc_filters_full_with_cache_and_hrtf(
            &params,
            sample_rate,
            num_bins,
            &cache,
            room_reflection_cache.clone(),
            hrtf_transfer_functions.as_ref(),
        );
        let cached_current_filters = Arc::new(filters);
        let filters = Arc::new(ArcSwap::from(Arc::clone(&cached_current_filters)));

        let auto_gain = if params.auto_gain_enabled {
            Some(
                AutoGain::new(
                    2, // stereo
                    sample_rate,
                    AutoGainParams {
                        enabled: true,
                        loudness_type: Default::default(),
                        max_gain_db: params.auto_gain_max_db,
                        smoothing_ms: params.auto_gain_smoothing_ms,
                    },
                )
                .map_err(|e| format!("AutoGain init failed: {}", e))?,
            )
        } else {
            None
        };

        let mut p = Self {
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
            output_accumulator: vec![0.0; fft_size * 4 * 2], // 4*N frames, stereo
            output_accumulator_mask: (fft_size * 4) - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            // Temp buffers for block processing (max reasonable block size)
            temp_input_l: vec![0.0; 4096],
            temp_input_r: vec![0.0; 4096],
            fft_buffer: vec![0.0; fft_size],
            fft_output_l: vec![Complex::new(0.0, 0.0); num_bins],
            fft_output_r: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_input: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_output: vec![0.0; fft_size],
            prev_ifft_output: vec![0.0; fft_size],
            filters,
            cached_current_filters,
            prev_filters: None,
            crossfade_progress: 1.0, // Start fully faded to current
            progress_per_hop: 0.0,
            hrtf_transfer_functions,
            room_reflection_cache,
            room_params_hash,
            auto_gain,
            limiter_envelope: 1.0,
            limiter_attack_coeff: math_audio_dsp::fast_math::fast_exp(
                -1.0 / (0.2 * 0.001 * sample_rate as f32),
            ),
            limiter_release_coeff: math_audio_dsp::fast_math::fast_exp(
                -1.0 / (50.0 * 0.001 * sample_rate as f32),
            ),
            latency_filled: 0,
            cache: RealTimeCache::new(XtcData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.params.distance_m as f64),
            1 => Some(self.params.speaker_angle_deg as f64),
            2 => Some(self.params.head_radius_m as f64),
            3 => Some(self.params.head_offset_x as f64),
            4 => Some(self.params.head_offset_z as f64),
            5 => Some(self.params.head_yaw_deg as f64),
            6 => Some(self.params.head_tracking_smooth_s as f64),
            7 => Some(self.params.beta_base as f64),
            8 => Some(self.params.beta_low_freq_boost as f64),
            9 => Some(self.params.beta_high_freq_boost as f64),
            10 => Some(self.params.head_shadow_cutoff_hz as f64),
            11 => Some(self.params.head_shadow_slope_db_per_octave as f64),
            12 => Some(self.params.max_gain_db as f64),
            13 => Some(if self.params.spectral_normalization {
                1.0
            } else {
                0.0
            }),
            14 => Some(if self.params.pinna_model_enabled {
                1.0
            } else {
                0.0
            }),
            15 => Some(if self.params.room_reflections_enabled {
                1.0
            } else {
                0.0
            }),
            16 => Some(self.params.room_width_m as f64),
            17 => Some(self.params.room_depth_m as f64),
            18 => Some(self.params.wall_absorption as f64),
            19 => Some(self.params.reflection_beta_boost as f64),
            20 => Some(if self.params.bypass_xtc_filters {
                1.0
            } else {
                0.0
            }),
            21 => Some(if self.params.bypass_spectral_normalization {
                1.0
            } else {
                0.0
            }),
            22 => Some(if self.params.bypass_neumann_refinement {
                1.0
            } else {
                0.0
            }),
            23 => Some(if self.params.auto_gain_enabled {
                1.0
            } else {
                0.0
            }),
            24 => Some(self.params.auto_gain_max_db as f64),
            25 => Some(self.params.auto_gain_smoothing_ms as f64),
            26 => Some(self.params.head_model as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.params.distance_m = value as f32,
            1 => self.params.speaker_angle_deg = value as f32,
            2 => self.params.head_radius_m = value as f32,
            3 => self.params.head_offset_x = value as f32,
            4 => self.params.head_offset_z = value as f32,
            5 => self.params.head_yaw_deg = value as f32,
            6 => self.params.head_tracking_smooth_s = value as f32,
            7 => self.params.beta_base = value as f32,
            8 => self.params.beta_low_freq_boost = value as f32,
            9 => self.params.beta_high_freq_boost = value as f32,
            10 => self.params.head_shadow_cutoff_hz = value as f32,
            11 => self.params.head_shadow_slope_db_per_octave = value as f32,
            12 => self.params.max_gain_db = value as f32,
            13 => self.params.spectral_normalization = value > 0.5,
            14 => self.params.pinna_model_enabled = value > 0.5,
            15 => self.params.room_reflections_enabled = value > 0.5,
            16 => self.params.room_width_m = value as f32,
            17 => self.params.room_depth_m = value as f32,
            18 => self.params.wall_absorption = value as f32,
            19 => self.params.reflection_beta_boost = value as f32,
            20 => self.params.bypass_xtc_filters = value > 0.5,
            21 => self.params.bypass_spectral_normalization = value > 0.5,
            22 => self.params.bypass_neumann_refinement = value > 0.5,
            23 => self.params.auto_gain_enabled = value > 0.5,
            24 => self.params.auto_gain_max_db = value as f32,
            25 => self.params.auto_gain_smoothing_ms = value as f32,
            26 => self.params.head_model = value as usize,
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(XT, |i| self.param_value(i));
        // Append parameters not in PARAMS
        self.cached_parameters.push(Parameter::new_bool(
            "enabled",
            "Enabled",
            self.params.enabled,
        ));
        self.cached_parameters.push(Parameter::new_float(
            "kappa_target",
            "Kappa Target",
            self.params.kappa_target,
            1.0,
            1000.0,
        ));
        self.cached_parameters.push(Parameter::new_string(
            "hrtf_file",
            "HRTF File",
            self.params.hrtf_file.clone().unwrap_or_default(),
        ));
        self.cached_parameters.push(Parameter::new_string(
            "itd_modeling",
            "ITD Mode",
            self.params.itd_modeling.clone(),
        ));
    }

    /// Create from parameters helper
    pub fn from_params(params: XtcPluginParams, sample_rate: u32) -> Result<Self, String> {
        Self::new(params, sample_rate)
    }

    /// Recompute filters when parameters change.
    /// Stores old filters for crossfading to avoid clicks.
    ///
    /// Optimization 3 & 4: Uses geometry cache and room reflection cache to avoid redundant computation.
    fn update_filters(&mut self, sync: bool) {
        let num_bins = self.fft_size / 2 + 1;
        let sample_rate = self.sample_rate;

        // Cache progress_per_hop (only depends on params + sample_rate + hop_size)
        let smooth_samples = self.params.head_tracking_smooth_s * self.sample_rate as f32;
        self.progress_per_hop = self.hop_size as f32 / smooth_samples;

        // Store old filters for crossfading (only if not already mid-crossfade)
        if self.crossfade_progress >= 1.0 {
            self.prev_filters = Some(self.filters.load_full());
            self.crossfade_progress = 0.0;
        }

        // Check if room reflection cache needs updating (Optimization 4)
        let new_hash = compute_room_params_hash(&self.params);
        if new_hash != self.room_params_hash {
            // Reuse the pre-planned FFT to avoid re-creating the planner (Optimization 4)
            self.room_reflection_cache = compute_room_reflection_data(
                &self.params,
                sample_rate,
                num_bins,
                Some(self.fft_forward.clone()),
            );
            self.room_params_hash = new_hash;
        }

        // Pre-compute geometry cache (Optimization 3)
        let cache = compute_geometry_cache(&self.params, sample_rate, num_bins);
        let room_data = self.room_reflection_cache.clone();

        let shared_filters = self.filters.clone();
        let hrtf_data = self.hrtf_transfer_functions.clone();

        if sync {
            let new_filters = compute_xtc_filters_full_with_cache_and_hrtf(
                &self.params,
                sample_rate,
                num_bins,
                &cache,
                room_data,
                hrtf_data.as_ref(),
            );
            shared_filters.store(Arc::new(new_filters));
        } else {
            // Asynchronous update using rayon
            let params = self.params.clone();
            rayon::spawn(move || {
                let new_filters = compute_xtc_filters_full_with_cache_and_hrtf(
                    &params,
                    sample_rate,
                    num_bins,
                    &cache,
                    room_data,
                    hrtf_data.as_ref(),
                );
                shared_filters.store(Arc::new(new_filters));
            });
        }
    }

    /// Process one STFT frame using SIMD-optimized operations.
    ///
    /// During crossfade (after parameter change), blends output from old and new
    /// filters over ~100ms to avoid clicks. This costs 4 IFFTs per frame instead
    /// of the normal 2, but crossfade transitions are brief.
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

        let scale = self.output_scale;
        let fft_size = self.fft_size;
        let mask = self.output_accumulator_mask;

        // Diagnostic bypass: skip all XTC filter math, just IFFT the windowed input.
        // This tests whether the STFT framework (windowing + OLA) itself is clean.
        if self.params.bypass_xtc_filters {
            // Left channel: IFFT the FFT output directly (identity in freq domain)
            self.ifft_input.copy_from_slice(&self.fft_output_l);
            let n = self.ifft_input.len();
            self.ifft_input[0].im = 0.0;
            self.ifft_input[n - 1].im = 0.0;
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            // Accumulate Left
            for i in 0..fft_size {
                let idx = (self.next_add_position + i) & mask;
                let s = self.ifft_output[i] * self.analysis_window[i] * scale;
                self.output_accumulator[idx * 2] += s;
            }

            // Right channel
            self.ifft_input.copy_from_slice(&self.fft_output_r);
            self.ifft_input[0].im = 0.0;
            self.ifft_input[n - 1].im = 0.0;
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            // Accumulate Right
            for i in 0..fft_size {
                let idx = (self.next_add_position + i) & mask;
                let s = self.ifft_output[i] * self.analysis_window[i] * scale;
                self.output_accumulator[idx * 2 + 1] += s;
            }
        } else if self.crossfade_progress < 1.0 && self.prev_filters.is_some() {
            let alpha = self.crossfade_progress;
            let prev_filters = self.prev_filters.as_ref().unwrap();
            // Use cached filter snapshot (loaded once per process() call)
            let current_filters = &self.cached_current_filters;

            // --- Left channel with crossfade ---
            // 1. IFFT with prev_filters into prev_ifft_output
            apply_filter_left(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                prev_filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.prev_ifft_output)
                .expect("IFFT processing failed");

            // 2. IFFT with current filters into ifft_output
            apply_filter_left(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                current_filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            // 3. Blend and Accumulate Left
            for i in 0..fft_size {
                let val = (1.0 - alpha) * self.prev_ifft_output[i] + alpha * self.ifft_output[i];
                let idx = (self.next_add_position + i) & mask;
                let s = val * self.analysis_window[i] * scale;
                self.output_accumulator[idx * 2] += s;
            }

            // --- Right channel with crossfade ---
            // 1. IFFT with prev_filters into prev_ifft_output
            apply_filter_right(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                prev_filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.prev_ifft_output)
                .expect("IFFT processing failed");

            // 2. IFFT with current filters into ifft_output
            apply_filter_right(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                current_filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            // 3. Blend and Accumulate Right
            for i in 0..fft_size {
                let val = (1.0 - alpha) * self.prev_ifft_output[i] + alpha * self.ifft_output[i];
                let idx = (self.next_add_position + i) & mask;
                let s = val * self.analysis_window[i] * scale;
                self.output_accumulator[idx * 2 + 1] += s;
            }
        } else {
            // Normal path: no crossfade needed
            let filters = &self.cached_current_filters;

            // Left channel
            apply_filter_left(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            for i in 0..fft_size {
                let idx = (self.next_add_position + i) & mask;
                let s = self.ifft_output[i] * self.analysis_window[i] * scale;
                self.output_accumulator[idx * 2] += s;
            }

            // Right channel
            apply_filter_right(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            for i in 0..fft_size {
                let idx = (self.next_add_position + i) & mask;
                let s = self.ifft_output[i] * self.analysis_window[i] * scale;
                self.output_accumulator[idx * 2 + 1] += s;
            }
        }

        // Update positions
        self.next_add_position = (self.next_add_position + self.hop_size) & mask;

        // Start draining immediately to match physical latency.
        self.output_accumulator_fill += self.hop_size;
        self.latency_filled += self.hop_size;

        // Advance crossfade progress
        if self.crossfade_progress < 1.0 {
            self.crossfade_progress = (self.crossfade_progress + self.progress_per_hop).min(1.0);
            if self.crossfade_progress >= 1.0 {
                self.prev_filters = None; // Release old filters
            }
        }
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
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

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
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Parameters not in PARAMS — handle separately
        if id.0 == "enabled" {
            self.params.enabled = value
                .as_bool()
                .ok_or_else(|| "enabled must be a boolean".to_string())?;
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "kappa_target" {
            let v = value
                .as_float()
                .ok_or_else(|| "kappa_target must be a float".to_string())?;
            if v.is_finite() {
                self.params.kappa_target = v.clamp(1.0, 1000.0);
                self.update_filters(false);
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "hrtf_file" {
            let v = value
                .as_string()
                .ok_or_else(|| "hrtf_file must be a string".to_string())?;
            if v.is_empty() {
                self.params.hrtf_file = None;
                self.hrtf_transfer_functions = None;
            } else {
                let num_bins = self.fft_size / 2 + 1;
                let hrtf = load_hrtf_for_xtc(v, &self.params, self.sample_rate, num_bins)?;
                self.params.hrtf_file = Some(v.to_string());
                self.hrtf_transfer_functions = hrtf;
            }
            self.update_filters(false);
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.0 == "itd_modeling" {
            let v = value
                .as_string()
                .ok_or_else(|| "itd_modeling must be a string".to_string())?;
            if v != "phase_only" && v != "explicit_delay" {
                return Err(format!(
                    "itd_modeling must be 'phase_only' or 'explicit_delay', got '{}'",
                    v
                ));
            }
            self.params.itd_modeling = v.to_string();
            self.update_filters(false);
            self.rebuild_cached_parameters();
            return Ok(());
        }

        let idx = param_bridge::set_parameter(XT, &id, &value, |i, v| self.set_param_value(i, v))?;

        // Side effects based on parameter index
        let needs_filter_update = match idx {
            0..=5 => true,   // geometry + head tracking
            7..=9 => true,   // beta
            10..=12 => true, // shadow + filter
            13 => true,      // spectral_normalization
            14 => true,      // pinna_model_enabled
            15..=19 => true, // room
            20 => {
                // bypass_xtc_filters
                self.limiter_envelope = 1.0;
                false
            }
            21 | 22 => true, // bypass_spectral_normalization, bypass_neumann_refinement
            23 => {
                // auto_gain_enabled
                if self.params.auto_gain_enabled && self.auto_gain.is_none() {
                    self.auto_gain = Some(AutoGain::new(
                        2,
                        self.sample_rate,
                        AutoGainParams {
                            enabled: true,
                            loudness_type: Default::default(),
                            max_gain_db: self.params.auto_gain_max_db,
                            smoothing_ms: self.params.auto_gain_smoothing_ms,
                        },
                    )?);
                } else if !self.params.auto_gain_enabled {
                    self.auto_gain = None;
                }
                false
            }
            24 => {
                // auto_gain_max_db
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_max_gain_db(self.params.auto_gain_max_db);
                }
                false
            }
            25 => {
                // auto_gain_smoothing_ms
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_smoothing_ms(self.params.auto_gain_smoothing_ms);
                }
                false
            }
            26 => true, // head_model
            _ => false,
        };

        if needs_filter_update {
            self.update_filters(false);
        }
        self.rebuild_cached_parameters();

        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Parameters not in PARAMS — handle separately
        if id.0 == "enabled" {
            return Some(ParameterValue::Bool(self.params.enabled));
        }
        if id.0 == "kappa_target" {
            return Some(ParameterValue::Float(self.params.kappa_target));
        }
        if id.0 == "hrtf_file" {
            return Some(ParameterValue::String(
                self.params.hrtf_file.clone().unwrap_or_default(),
            ));
        }
        if id.0 == "itd_modeling" {
            return Some(ParameterValue::String(self.params.itd_modeling.clone()));
        }
        param_bridge::get_parameter(XT, id, |i| self.param_value(i))
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.limiter_attack_coeff =
            math_audio_dsp::fast_math::fast_exp(-1.0 / (0.2 * 0.001 * sample_rate as f32));
        self.limiter_release_coeff =
            math_audio_dsp::fast_math::fast_exp(-1.0 / (50.0 * 0.001 * sample_rate as f32));
        self.update_filters(true); // Synchronous for initialization

        // Pre-allocate temp buffers to max expected frame count.
        // After this, the resize() check in process() is a guaranteed no-op.
        let max_frames = 4096;
        self.temp_input_l.resize(max_frames, 0.0);
        self.temp_input_r.resize(max_frames, 0.0);

        if let Some(ag) = &mut self.auto_gain {
            ag.set_sample_rate(sample_rate).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn reset(&mut self) {
        // Clear all buffers
        self.input_buffer_l.fill(0.0);
        self.input_buffer_r.fill(0.0);
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.prev_ifft_output.fill(0.0);
        self.input_fill = 0;
        self.latency_filled = 0;

        // Reset crossfade state
        self.prev_filters = None;
        self.crossfade_progress = 1.0;

        if let Some(ag) = &mut self.auto_gain {
            ag.reset();
        }
        self.limiter_envelope = 1.0;
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

        // Measure loudness (throttled to 1/10 blocks to save CPU)
        self.cache_update_counter += 1;
        let mut do_measure = false;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            do_measure = true;
        }

        // Measure input loudness for auto-gain (before any processing)
        if do_measure && let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_input(input);
        }

        // Bypass if disabled
        if !self.params.enabled {
            output.copy_from_slice(input);

            // Still update diagnostic cache when bypassed
            if do_measure {
                let ag_data = self
                    .auto_gain
                    .as_ref()
                    .map(|ag| ag.get_data())
                    .unwrap_or_default();
                self.cache.update(|d| {
                    d.auto_gain = ag_data;
                    d.limiter_envelope = 1.0;
                });
            }
            return Ok(context.num_frames);
        }

        // Snapshot current filters once per process() call (avoids per-frame ArcSwap::load atomic ops)
        self.cached_current_filters = arc_swap::Guard::into_inner(self.filters.load());

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

        let mut input_pos = 0;
        let mut output_pos = 0;
        let mask = self.output_accumulator_mask;

        while output_pos < num_frames {
            // Step 1: Fill input buffer from deinterleaved temp buffers
            if input_pos < num_frames {
                let samples_needed = self.fft_size - self.input_fill;
                let samples_available_in = num_frames - input_pos;
                let to_copy = samples_needed.min(samples_available_in);

                if to_copy > 0 {
                    self.input_buffer_l[self.input_fill..self.input_fill + to_copy]
                        .copy_from_slice(&self.temp_input_l[input_pos..input_pos + to_copy]);
                    self.input_buffer_r[self.input_fill..self.input_fill + to_copy]
                        .copy_from_slice(&self.temp_input_r[input_pos..input_pos + to_copy]);
                    self.input_fill += to_copy;
                    input_pos += to_copy;
                }
            }

            // Step 2: Process ALL possible STFT frames from current input
            while self.input_fill >= self.fft_size {
                self.process_stft_frame();
                self.shift_input_buffer();
            }

            // Step 3: Copy available output to output buffer
            let frames_to_drain = self.output_accumulator_fill.min(num_frames - output_pos);

            if frames_to_drain > 0 {
                for i in 0..frames_to_drain {
                    let read_idx = (self.output_read_position + i) & mask;
                    let acc_base = read_idx * 2;
                    let out_base = (output_pos + i) * 2;
                    output[out_base] = self.output_accumulator[acc_base];
                    output[out_base + 1] = self.output_accumulator[acc_base + 1];
                    // Clear after reading for next overlap-add cycle
                    self.output_accumulator[acc_base] = 0.0;
                    self.output_accumulator[acc_base + 1] = 0.0;
                }
                self.output_read_position = (self.output_read_position + frames_to_drain) & mask;
                self.output_accumulator_fill -= frames_to_drain;
                output_pos += frames_to_drain;
            } else {
                // Break if no progress is possible
                break;
            }
        }

        // Auto-gain: measure the UNCOMPENSATED output from the plugin filters.
        // This ensures the gain calculation is stable and doesn't oscillate.
        if let Some(ag) = &mut self.auto_gain {
            if do_measure {
                let _ = ag.measure_output(&output[..output_pos * 2]);

                // Update diagnostic cache (Real-time safe, throttled)
                let ag_data = ag.get_data();
                let limiter_env = self.limiter_envelope;
                self.cache.update(|d| {
                    d.auto_gain = ag_data;
                    d.limiter_envelope = limiter_env;
                });
            }
            ag.apply_compensation(&mut output[..output_pos * 2], output_pos);
        }

        // Per-sample peak limiter: prevent clipping after XTC filter summation + AutoGain.
        // Smooth attack (~0.2ms) and release (~50ms) to avoid gain modulation artifacts.
        // Skip when filters are bypassed — no amplification occurs.
        if !self.params.bypass_xtc_filters && output_pos > 0 {
            let threshold = 0.95_f32;
            for frame in 0..output_pos {
                let idx_l = frame * 2;
                let idx_r = frame * 2 + 1;
                let peak = output[idx_l].abs().max(output[idx_r].abs());
                let target_gr = if peak > threshold {
                    threshold / peak
                } else {
                    1.0
                };
                if target_gr < self.limiter_envelope {
                    // Smooth attack (~0.2ms) to avoid per-sample gain jumps
                    self.limiter_envelope =
                        target_gr + self.limiter_attack_coeff * (self.limiter_envelope - target_gr);
                } else {
                    self.limiter_envelope = target_gr
                        + self.limiter_release_coeff * (self.limiter_envelope - target_gr);
                }
                output[idx_l] *= self.limiter_envelope;
                output[idx_r] *= self.limiter_envelope;
                // Hard clamp: the one-pole envelope has finite attack time, so a
                // few samples can overshoot during transient onset. Clamp to ±1.0
                // as a safety ceiling — matches standard digital limiter practice.
                output[idx_l] = output[idx_l].clamp(-1.0, 1.0);
                output[idx_r] = output[idx_r].clamp(-1.0, 1.0);
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
