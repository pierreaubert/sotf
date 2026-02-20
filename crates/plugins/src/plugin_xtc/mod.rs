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
mod reflections;
#[cfg(test)]
mod tests;
pub mod validation;

pub use config::*;
use filters::{compute_geometry_cache, compute_xtc_filters_full_with_cache, XtcFilters};
use reflections::{
    build_reflection_data_image_source, build_reflection_data_ir, RoomReflectionData,
};

use super::auto_gain::{AutoGain, AutoGainParams};
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{
    blend_simd, complex_mul_add_simd, complex_mul_simd, deinterleave_stereo,
    flush_denormals_inplace, interleave_stereo, scale_add_simd, window_mul_simd,
};
use arc_swap::ArcSwap;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

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
    hasher.finish()
}

/// Compute room reflection data if enabled.
///
/// Returns None if room reflections are disabled.
fn compute_room_reflection_data(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> Option<Arc<RoomReflectionData>> {
    if !params.room_reflections_enabled {
        return None;
    }

    let data = if let Some(ref ir_path) = params.room_ir_file {
        build_reflection_data_ir(ir_path, sample_rate, num_bins).ok()?
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

    /// Working buffer for crossfade: holds IFFT of prev_filters result
    prev_ifft_output: Vec<f32>,

    /// Thread-safe crosstalk cancellation filters (lock-free via ArcSwap)
    filters: Arc<ArcSwap<XtcFilters>>,

    /// Previous filter snapshot for crossfading (Block mode)
    prev_filters: Option<Arc<XtcFilters>>,

    /// Crossfade progress (0.0 = prev, 1.0 = current)
    crossfade_progress: f32,

    /// Cached progress increment per STFT hop (recomputed in update_filters)
    progress_per_hop: f32,

    /// Parameters for dynamic updates
    param_enabled: ParameterId,
    param_distance: ParameterId,
    param_speaker_angle: ParameterId,
    param_head_offset_x: ParameterId,
    param_head_offset_z: ParameterId,
    param_head_yaw: ParameterId,
    param_spectral_normalization: ParameterId,
    param_room_reflections: ParameterId,
    param_bypass_xtc_filters: ParameterId,
    param_bypass_spectral_normalization: ParameterId,
    param_bypass_neumann_refinement: ParameterId,
    param_auto_gain_enabled: ParameterId,
    param_auto_gain_max_db: ParameterId,
    param_auto_gain_smoothing_ms: ParameterId,

    /// Cached room reflection data (Optimization 4)
    room_reflection_cache: Option<Arc<RoomReflectionData>>,

    /// Hash of room-related parameters for cache invalidation (Optimization 4)
    room_params_hash: u64,

    /// Auto-gain compensation to match output loudness to input
    auto_gain: Option<AutoGain>,
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

        // Compute initial room reflection data if enabled (Optimization 4)
        let room_params_hash = compute_room_params_hash(&params);
        let room_reflection_cache = if params.room_reflections_enabled {
            compute_room_reflection_data(&params, sample_rate, num_bins)
        } else {
            None
        };

        // Compute geometry cache (Optimization 3)
        let cache = compute_geometry_cache(&params, sample_rate, num_bins);

        let filters = compute_xtc_filters_full_with_cache(
            &params,
            sample_rate,
            num_bins,
            &cache,
            room_reflection_cache.clone(),
        );
        let filters = Arc::new(ArcSwap::from_pointee(filters));

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
            prev_ifft_output: vec![0.0; fft_size],
            filters,
            prev_filters: None,
            crossfade_progress: 1.0, // Start fully faded to current
            progress_per_hop: 0.0,
            param_enabled: ParameterId::from("enabled"),
            param_distance: ParameterId::from("distance_m"),
            param_speaker_angle: ParameterId::from("speaker_angle_deg"),
            param_head_offset_x: ParameterId::from("head_offset_x"),
            param_head_offset_z: ParameterId::from("head_offset_z"),
            param_head_yaw: ParameterId::from("head_yaw_deg"),
            param_spectral_normalization: ParameterId::from("spectral_normalization"),
            param_room_reflections: ParameterId::from("room_reflections_enabled"),
            param_bypass_xtc_filters: ParameterId::from("bypass_xtc_filters"),
            param_bypass_spectral_normalization: ParameterId::from("bypass_spectral_normalization"),
            param_bypass_neumann_refinement: ParameterId::from("bypass_neumann_refinement"),
            param_auto_gain_enabled: ParameterId::from("auto_gain_enabled"),
            param_auto_gain_max_db: ParameterId::from("auto_gain_max_db"),
            param_auto_gain_smoothing_ms: ParameterId::from("auto_gain_smoothing_ms"),
            room_reflection_cache,
            room_params_hash,
            auto_gain,
        })
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
            self.room_reflection_cache =
                compute_room_reflection_data(&self.params, sample_rate, num_bins);
            self.room_params_hash = new_hash;
        }

        // Pre-compute geometry cache (Optimization 3)
        let cache = compute_geometry_cache(&self.params, sample_rate, num_bins);
        let room_data = self.room_reflection_cache.clone();

        let shared_filters = self.filters.clone();

        if sync {
            let new_filters = compute_xtc_filters_full_with_cache(
                &self.params,
                sample_rate,
                num_bins,
                &cache,
                room_data,
            );
            shared_filters.store(Arc::new(new_filters));
        } else {
            // Asynchronous update using rayon
            let params = self.params.clone();
            rayon::spawn(move || {
                let new_filters = compute_xtc_filters_full_with_cache(
                    &params,
                    sample_rate,
                    num_bins,
                    &cache,
                    room_data,
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
            scale_add_simd(
                &mut self.output_accum_l[..fft_size],
                &self.ifft_output,
                scale,
            );

            // Right channel
            self.ifft_input.copy_from_slice(&self.fft_output_r);
            self.ifft_input[0].im = 0.0;
            self.ifft_input[n - 1].im = 0.0;
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");
            scale_add_simd(
                &mut self.output_accum_r[..fft_size],
                &self.ifft_output,
                scale,
            );

            // Mark hop_size more samples as available
            self.output_available += self.hop_size;
            return;
        }

        let is_crossfading = self.crossfade_progress < 1.0 && self.prev_filters.is_some();

        if is_crossfading {
            let alpha = self.crossfade_progress;
            let prev_filters = self.prev_filters.as_ref().unwrap();
            // Single atomic load for both L and R channels (guarantees filter consistency)
            let current_filters = self.filters.load();

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
                &current_filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            // 3. Blend: ifft_output = (1-alpha)*prev + alpha*current
            blend_simd(&mut self.ifft_output, &self.prev_ifft_output, alpha);

            // 4. Overlap-add to left accumulator
            scale_add_simd(
                &mut self.output_accum_l[..fft_size],
                &self.ifft_output,
                scale,
            );

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
                &current_filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");

            // 3. Blend
            blend_simd(&mut self.ifft_output, &self.prev_ifft_output, alpha);

            // 4. Overlap-add to right accumulator
            scale_add_simd(
                &mut self.output_accum_r[..fft_size],
                &self.ifft_output,
                scale,
            );
        } else {
            // Normal path: no crossfade needed
            let filters = self.filters.load();

            // Left channel
            apply_filter_left(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                &filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");
            scale_add_simd(
                &mut self.output_accum_l[..fft_size],
                &self.ifft_output,
                scale,
            );

            // Right channel
            apply_filter_right(
                &mut self.ifft_input,
                &self.fft_output_l,
                &self.fft_output_r,
                &filters,
            );
            self.fft_inverse
                .process(&mut self.ifft_input, &mut self.ifft_output)
                .expect("IFFT processing failed");
            scale_add_simd(
                &mut self.output_accum_r[..fft_size],
                &self.ifft_output,
                scale,
            );
        }

        // Advance crossfade progress
        if self.crossfade_progress < 1.0 {
            self.crossfade_progress = (self.crossfade_progress + self.progress_per_hop).min(1.0);
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
            Parameter::new_bool(
                "spectral_normalization",
                "Spectral Normalization",
                self.params.spectral_normalization,
            )
            .with_group("Advanced")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "room_reflections_enabled",
                "Room Reflections",
                self.params.room_reflections_enabled,
            )
            .with_group("Room")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "bypass_xtc_filters",
                "Bypass XTC Filters",
                self.params.bypass_xtc_filters,
            )
            .with_group("Diagnostic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "bypass_spectral_normalization",
                "Bypass Spectral Normalization",
                self.params.bypass_spectral_normalization,
            )
            .with_group("Diagnostic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "bypass_neumann_refinement",
                "Bypass Neumann Refinement",
                self.params.bypass_neumann_refinement,
            )
            .with_group("Diagnostic")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "auto_gain_enabled",
                "Auto Gain",
                self.params.auto_gain_enabled,
            )
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "auto_gain_max_db",
                "Auto Gain Max (dB)",
                self.params.auto_gain_max_db,
                0.0,
                24.0,
            )
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "auto_gain_smoothing_ms",
                "Auto Gain Smoothing (ms)",
                self.params.auto_gain_smoothing_ms,
                10.0,
                500.0,
            )
            .with_group("Auto Gain")
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
        } else if id == self.param_spectral_normalization {
            if let ParameterValue::Bool(v) = value {
                self.params.spectral_normalization = v;
                needs_filter_update = true;
            } else {
                return Err("spectral_normalization parameter must be bool".to_string());
            }
        } else if id == self.param_room_reflections {
            if let ParameterValue::Bool(v) = value {
                self.params.room_reflections_enabled = v;
                needs_filter_update = true;
            } else {
                return Err("room_reflections_enabled parameter must be bool".to_string());
            }
        } else if id == self.param_bypass_xtc_filters {
            if let ParameterValue::Bool(v) = value {
                self.params.bypass_xtc_filters = v;
            } else {
                return Err("bypass_xtc_filters parameter must be bool".to_string());
            }
        } else if id == self.param_bypass_spectral_normalization {
            if let ParameterValue::Bool(v) = value {
                self.params.bypass_spectral_normalization = v;
                needs_filter_update = true;
            } else {
                return Err("bypass_spectral_normalization parameter must be bool".to_string());
            }
        } else if id == self.param_bypass_neumann_refinement {
            if let ParameterValue::Bool(v) = value {
                self.params.bypass_neumann_refinement = v;
                needs_filter_update = true;
            } else {
                return Err("bypass_neumann_refinement parameter must be bool".to_string());
            }
        } else if id == self.param_auto_gain_enabled {
            if let ParameterValue::Bool(v) = value {
                self.params.auto_gain_enabled = v;
                if v && self.auto_gain.is_none() {
                    self.auto_gain = AutoGain::new(
                        2,
                        self.sample_rate,
                        AutoGainParams {
                            enabled: true,
                            loudness_type: Default::default(),
                            max_gain_db: self.params.auto_gain_max_db,
                            smoothing_ms: self.params.auto_gain_smoothing_ms,
                        },
                    )
                    .ok();
                } else if !v {
                    self.auto_gain = None;
                }
            } else {
                return Err("auto_gain_enabled parameter must be bool".to_string());
            }
        } else if id == self.param_auto_gain_max_db {
            if let ParameterValue::Float(v) = value {
                self.params.auto_gain_max_db = v.clamp(0.0, 24.0);
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_max_gain_db(self.params.auto_gain_max_db);
                }
            } else {
                return Err("auto_gain_max_db parameter must be float".to_string());
            }
        } else if id == self.param_auto_gain_smoothing_ms {
            if let ParameterValue::Float(v) = value {
                self.params.auto_gain_smoothing_ms = v.clamp(10.0, 500.0);
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_smoothing_ms(self.params.auto_gain_smoothing_ms);
                }
            } else {
                return Err("auto_gain_smoothing_ms parameter must be float".to_string());
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
        } else if id == &self.param_spectral_normalization {
            Some(ParameterValue::Bool(self.params.spectral_normalization))
        } else if id == &self.param_room_reflections {
            Some(ParameterValue::Bool(self.params.room_reflections_enabled))
        } else if id == &self.param_bypass_xtc_filters {
            Some(ParameterValue::Bool(self.params.bypass_xtc_filters))
        } else if id == &self.param_bypass_spectral_normalization {
            Some(ParameterValue::Bool(self.params.bypass_spectral_normalization))
        } else if id == &self.param_bypass_neumann_refinement {
            Some(ParameterValue::Bool(self.params.bypass_neumann_refinement))
        } else if id == &self.param_auto_gain_enabled {
            Some(ParameterValue::Bool(self.params.auto_gain_enabled))
        } else if id == &self.param_auto_gain_max_db {
            Some(ParameterValue::Float(self.params.auto_gain_max_db))
        } else if id == &self.param_auto_gain_smoothing_ms {
            Some(ParameterValue::Float(self.params.auto_gain_smoothing_ms))
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

        if let Some(ag) = &mut self.auto_gain {
            ag.set_sample_rate(sample_rate)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn reset(&mut self) {
        // Clear all buffers
        self.input_buffer_l.fill(0.0);
        self.input_buffer_r.fill(0.0);
        self.output_accum_l.fill(0.0);
        self.output_accum_r.fill(0.0);
        self.prev_ifft_output.fill(0.0);
        self.input_fill = 0;
        self.output_available = 0;

        // Reset crossfade state
        self.prev_filters = None;
        self.crossfade_progress = 1.0;

        if let Some(ag) = &mut self.auto_gain {
            ag.reset();
        }
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

        // Measure input loudness for auto-gain (before any processing)
        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_input(input);
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
                // Flush denormals in-place then SIMD interleave
                let n = samples_to_output;
                flush_denormals_inplace(&mut self.output_accum_l[..n]);
                flush_denormals_inplace(&mut self.output_accum_r[..n]);
                interleave_stereo(
                    &self.output_accum_l[..n],
                    &self.output_accum_r[..n],
                    &mut output[out_pos * 2..(out_pos + n) * 2],
                );
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

        // Auto-gain: measure output loudness and apply compensation
        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_output(&output[..out_pos * 2]);
            ag.apply_compensation(&mut output[..out_pos * 2], out_pos);
        }

        // Return actual number of frames produced. DawHost handles silence padding.
        Ok(out_pos)
    }

    fn latency_samples(&self) -> usize {
        // Latency is approximately fft_size - hop_size due to overlap-add
        self.fft_size - self.hop_size
    }
}
