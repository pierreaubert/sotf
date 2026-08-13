use super::apply::apply_filter_left;
use super::apply::apply_filter_left_blended;
use super::apply::apply_filter_pair;
use super::apply::apply_filter_pair_blended;
use super::apply::apply_filter_right;
use super::apply::apply_filter_right_blended;
use super::compute::compute_room_params_hash;
use super::compute::compute_room_reflection_data;
pub use super::config::*;
use super::filters::{
    HrtfTransferFunctions, XtcFilters, compute_geometry_cache,
    compute_xtc_filters_full_with_cache_and_hrtf,
};
use super::load::load_hrtf_for_xtc;
use super::load::load_roomeq_recommended_filters;
use super::load::validate_roomeq_recommended_source;
use super::misc::MAX_PROCESS_FRAMES;
use super::reflections::{RoomReflectionData, build_reflection_data_ir};
use super::types::{FilterUpdateRequest, PendingFilterUpdate};
use super::xtc_data::XtcData;
use crate::params::PARAMS as XT;
use arc_swap::{ArcSwap, ArcSwapOption};
use math_audio_dsp::stft::generate_hann_window;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainParams};
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{deinterleave_stereo, flush_denormals_inplace, window_mul_simd};
use std::any::Any;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

/// FFT / STFT configuration and shared resources.
pub(super) struct XtcFftConfig {
    /// FFT size (must be power of 2)
    pub(super) fft_size: usize,

    /// Hop size for overlap-add (75% overlap = fft_size / 4)
    pub(super) hop_size: usize,

    /// Sample rate
    pub(super) sample_rate: u32,

    /// Forward FFT planner
    pub(super) fft_forward: Arc<dyn RealToComplex<f32>>,

    /// Inverse FFT planner
    pub(super) fft_inverse: Arc<dyn ComplexToReal<f32>>,

    /// Analysis window (Hann)
    pub(super) analysis_window: Vec<f32>,

    /// Combined scale factor: COLA normalization / FFT size
    pub(super) output_scale: f32,
}

/// Input staging buffers.
pub(super) struct XtcInputBuffers {
    /// Input buffer: holds fft_size samples per channel
    /// Uses linear buffer with shift instead of ring buffer to avoid modulo
    pub(super) input_buffer_l: Vec<f32>,
    pub(super) input_buffer_r: Vec<f32>,

    /// Number of samples currently in input buffer (0 to fft_size)
    pub(super) input_fill: usize,

    /// Temporary buffers for block processing (avoid per-call allocation)
    pub(super) temp_input_l: Vec<f32>,
    pub(super) temp_input_r: Vec<f32>,
}

/// Working buffers used during the FFT / IFFT stages.
pub(super) struct XtcWorkBuffers {
    /// Working buffers for FFT
    pub(super) fft_buffer: Vec<f32>,
    pub(super) fft_output_l: Vec<Complex<f32>>,
    pub(super) fft_output_r: Vec<Complex<f32>>,
    pub(super) ifft_input: Vec<Complex<f32>>,
    pub(super) ifft_output: Vec<f32>,

    /// Working buffer for crossfade: holds IFFT of prev_filters result
    pub(super) prev_ifft_output: Vec<f32>,
}

/// Thread-safe filter state, including the current filters, asynchronous updates,
/// crossfade smoothing, and room/HRTF data.
pub(super) struct XtcFilterState {
    /// Thread-safe crosstalk cancellation filters (lock-free via ArcSwap)
    pub(super) filters: Arc<ArcSwap<XtcFilters>>,

    /// Cached filter snapshot loaded once per process() call (avoids per-frame ArcSwap::load)
    pub(super) cached_current_filters: Arc<XtcFilters>,

    /// Completed asynchronous filter update waiting to be adopted by the audio thread.
    pub(super) pending_filter_update: Arc<ArcSwapOption<PendingFilterUpdate>>,

    /// Latest requested asynchronous filter generation; workers use it to drop stale results.
    pub(super) filter_update_generation: Arc<AtomicU64>,

    /// Latest-only control-thread request mailbox. At most one worker exists
    /// per instance; rapid automation overwrites this slot instead of spawning
    /// unbounded expensive jobs on Rayon's global pool.
    pub(super) filter_request: Arc<Mutex<Option<FilterUpdateRequest>>>,
    pub(super) filter_worker_running: Arc<AtomicBool>,
    pub(super) filter_worker_launches: Arc<AtomicU64>,

    /// Previous filter snapshot for crossfading (Block mode)
    pub(super) prev_filters: Option<Arc<XtcFilters>>,

    /// Crossfade progress (0.0 = prev, 1.0 = current)
    pub(super) crossfade_progress: f32,

    /// Cached progress increment per STFT hop (recomputed in update_filters)
    pub(super) progress_per_hop: f32,

    /// Loaded HRTF transfer functions (from SOFA file)
    pub(super) hrtf_transfer_functions: Option<Arc<HrtfTransferFunctions>>,

    /// Cached room reflection data (Optimization 4)
    pub(super) room_reflection_cache: Option<Arc<RoomReflectionData>>,

    /// Hash of room-related parameters for cache invalidation (Optimization 4)
    pub(super) room_params_hash: u64,
}

/// Overlap-add output ring buffer state.
pub(super) struct XtcOutputBuffers {
    /// Output accumulator for overlap-add (flat interleaved ring buffer)
    /// Layout: [L0, R0, L1, R1, ...]
    /// Buffer size in frames is always power-of-2 (4 * fft_size) for efficient masking
    pub(super) output_accumulator: Vec<f32>,
    /// Bitmask for ring buffer frame index (buffer_frames - 1)
    pub(super) output_accumulator_mask: usize,
    /// Number of valid frames in output accumulator
    pub(super) output_accumulator_fill: usize,
    /// Next frame position to add a block (tracks overlap-add offset)
    pub(super) next_add_position: usize,
    /// Current read frame position in the output accumulator ring buffer
    pub(super) output_read_position: usize,
    /// Initial latency counter to ensure OLA buffer is primed before output
    pub(super) latency_filled: usize,
}

/// Output dynamics processing (auto-gain + peak limiter).
pub(super) struct XtcDynamics {
    /// Auto-gain compensation to match output loudness to input
    pub(super) auto_gain: Option<AutoGain>,

    /// Per-sample limiter envelope (0.0..=1.0). Smooth attack and release.
    /// Prevents output from exceeding ±0.95 after XTC filter summation + auto-gain.
    pub(super) limiter_envelope: f32,

    /// Per-sample attack coefficient for the limiter (~0.2ms time constant).
    pub(super) limiter_attack_coeff: f32,

    /// Per-sample release coefficient for the limiter (~50ms release).
    pub(super) limiter_release_coeff: f32,
}

/// Diagnostic and parameter caching state.
pub(super) struct XtcDiagnostics {
    /// Diagnostic data cache (Real-time safe)
    pub(super) cache: RealTimeCache<XtcData>,

    /// Counter to throttle diagnostic cache updates
    pub(super) cache_update_counter: usize,

    pub(super) cached_parameters: Vec<Parameter>,
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
    /// Configuration parameters
    pub(super) params: XtcPluginParams,

    /// FFT / STFT configuration and shared resources
    pub(super) fft: XtcFftConfig,

    /// Input staging buffers
    pub(super) input: XtcInputBuffers,

    /// Overlap-add output ring buffer state
    pub(super) output: XtcOutputBuffers,

    /// Working buffers for FFT / IFFT
    pub(super) work: XtcWorkBuffers,

    /// Filter state, asynchronous updates, crossfade smoothing, room/HRTF data
    pub(super) filter_state: XtcFilterState,

    /// Output dynamics (auto-gain + peak limiter)
    pub(super) dynamics: XtcDynamics,

    /// Diagnostic and parameter caching state
    pub(super) diagnostics: XtcDiagnostics,
}

impl XtcPlugin {
    const STRUCTURAL_SOURCE_ERROR: &'static str =
        "XTC source/artifact changes are structural and require rebuilding the plugin graph";
    /// Validate a source configuration before exposing it through parameters.
    ///
    /// Source changes are structural: an asynchronous recompute may fail after
    /// a setter returns, so accepting a configuration that cannot produce its
    /// requested plant would leave the UI state ahead of the effective filters.
    /// Validate the complete candidate synchronously while the control thread is
    /// still allowed to perform file I/O.
    fn validate_source_configuration(
        params: &XtcPluginParams,
        sample_rate: u32,
        num_bins: usize,
    ) -> Result<(), String> {
        match params.source_mode.as_str() {
            "synthetic" | "roomeq_recommended" if params.hrtf_file.is_some() => Err(format!(
                "source_mode='{}' cannot be combined with hrtf_file; use source_mode='hrtf_file'",
                params.source_mode
            )),
            "hrtf_file" => {
                let hrtf_path = params
                    .hrtf_file
                    .as_deref()
                    .ok_or_else(|| "source_mode='hrtf_file' requires hrtf_file".to_string())?;
                load_hrtf_for_xtc(hrtf_path, params, sample_rate, num_bins).map(|_| ())
            }
            "roomeq_recommended" => {
                validate_roomeq_recommended_source(params, sample_rate, num_bins)
            }
            "synthetic" => Ok(()),
            other => Err(format!(
                "source_mode must be 'synthetic', 'hrtf_file', or 'roomeq_recommended', got '{}'",
                other
            )),
        }
    }

    fn validate_room_ir_configuration(
        params: &XtcPluginParams,
        sample_rate: u32,
        num_bins: usize,
        fft_forward: Arc<dyn RealToComplex<f32>>,
    ) -> Result<(), String> {
        if params.room_reflections_enabled
            && let Some(path) = params.room_ir_file.as_deref()
        {
            build_reflection_data_ir(path, sample_rate, num_bins, Some(fft_forward))?;
        }
        Ok(())
    }

    /// Create a new XTC plugin
    pub fn new(params: XtcPluginParams, sample_rate: u32) -> Result<Self, String> {
        match params.source_mode.as_str() {
            "synthetic" if params.hrtf_file.is_some() => {
                return Err(
                    "source_mode='synthetic' cannot be combined with hrtf_file; use source_mode='hrtf_file'"
                        .to_string(),
                );
            }
            "hrtf_file" if params.hrtf_file.is_none() => {
                return Err("source_mode='hrtf_file' requires hrtf_file".to_string());
            }
            "synthetic" | "hrtf_file" | "roomeq_recommended" => {}
            other => {
                return Err(format!(
                    "source_mode must be 'synthetic', 'hrtf_file', or 'roomeq_recommended', got '{}'",
                    other
                ));
            }
        }

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
        Self::validate_room_ir_configuration(&params, sample_rate, num_bins, fft_forward.clone())?;

        // Compute initial room reflection data if enabled (Optimization 4)
        let room_params_hash = compute_room_params_hash(&params);
        let room_reflection_cache = if params.room_reflections_enabled {
            // Pass the pre-planned FFT to avoid re-creating the planner (Optimization 4)
            compute_room_reflection_data(&params, sample_rate, num_bins, Some(fft_forward.clone()))
        } else {
            None
        };

        // Load HRTF file if specified. The roomEQ recommended source bypasses
        // geometry/HRTF solving and loads its co-designed filters directly.
        let hrtf_transfer_functions = if params.source_mode == "hrtf_file" {
            let hrtf_path = params.hrtf_file.as_deref().expect("validated above");
            load_hrtf_for_xtc(hrtf_path, &params, sample_rate, num_bins)?.map(Arc::new)
        } else {
            None
        };

        let filters = if params.source_mode == "roomeq_recommended" {
            let matrix_path = params.recommended_matrix_file.as_deref().ok_or_else(|| {
                "source_mode='roomeq_recommended' requires recommended_matrix_file".to_string()
            })?;
            load_roomeq_recommended_filters(matrix_path, sample_rate, num_bins)?
        } else {
            // Compute geometry cache (Optimization 3)
            let cache = compute_geometry_cache(&params, sample_rate, num_bins);
            compute_xtc_filters_full_with_cache_and_hrtf(
                &params,
                sample_rate,
                num_bins,
                &cache,
                room_reflection_cache.clone(),
                hrtf_transfer_functions.as_deref(),
            )
        };
        let output_channels = filters.output_channels();
        let cached_current_filters = Arc::new(filters);
        let filters = Arc::new(ArcSwap::from(Arc::clone(&cached_current_filters)));

        let auto_gain = if params.auto_gain_enabled && output_channels == 2 {
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
            params: params.clone(),
            fft: XtcFftConfig {
                fft_size,
                hop_size,
                sample_rate,
                fft_forward,
                fft_inverse,
                analysis_window,
                output_scale,
            },
            input: XtcInputBuffers {
                input_buffer_l: vec![0.0; fft_size],
                input_buffer_r: vec![0.0; fft_size],
                input_fill: 0,
                temp_input_l: vec![0.0; MAX_PROCESS_FRAMES],
                temp_input_r: vec![0.0; MAX_PROCESS_FRAMES],
            },
            output: XtcOutputBuffers {
                output_accumulator: vec![0.0; fft_size * 4 * output_channels],
                output_accumulator_mask: (fft_size * 4) - 1,
                output_accumulator_fill: 0,
                next_add_position: 0,
                output_read_position: 0,
                latency_filled: 0,
            },
            work: XtcWorkBuffers {
                fft_buffer: vec![0.0; fft_size],
                fft_output_l: vec![Complex::new(0.0, 0.0); num_bins],
                fft_output_r: vec![Complex::new(0.0, 0.0); num_bins],
                ifft_input: vec![Complex::new(0.0, 0.0); num_bins],
                ifft_output: vec![0.0; fft_size],
                prev_ifft_output: vec![0.0; fft_size],
            },
            filter_state: XtcFilterState {
                filters,
                cached_current_filters,
                pending_filter_update: Arc::new(ArcSwapOption::empty()),
                filter_update_generation: Arc::new(AtomicU64::new(0)),
                filter_request: Arc::new(Mutex::new(None)),
                filter_worker_running: Arc::new(AtomicBool::new(false)),
                filter_worker_launches: Arc::new(AtomicU64::new(0)),
                prev_filters: None,
                crossfade_progress: 1.0, // Start fully faded to current
                progress_per_hop: 0.0,
                hrtf_transfer_functions,
                room_reflection_cache,
                room_params_hash,
            },
            dynamics: XtcDynamics {
                auto_gain,
                limiter_envelope: 1.0,
                limiter_attack_coeff: math_audio_dsp::fast_math::fast_exp(
                    -1.0 / (0.2 * 0.001 * sample_rate as f32),
                ),
                limiter_release_coeff: math_audio_dsp::fast_math::fast_exp(
                    -1.0 / (50.0 * 0.001 * sample_rate as f32),
                ),
            },
            diagnostics: XtcDiagnostics {
                cache: RealTimeCache::new(XtcData::default()),
                cache_update_counter: 0,
                cached_parameters: Vec::new(),
            },
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
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
            16 => None, // room_ir_file (FilePath) is handled as structural string state.
            17 => Some(self.params.room_width_m as f64),
            18 => Some(self.params.room_depth_m as f64),
            19 => Some(self.params.wall_absorption as f64),
            20 => Some(self.params.reflection_beta_boost as f64),
            21 => Some(if self.params.bypass_xtc_filters {
                1.0
            } else {
                0.0
            }),
            22 => Some(if self.params.bypass_spectral_normalization {
                1.0
            } else {
                0.0
            }),
            23 => Some(if self.params.bypass_neumann_refinement {
                1.0
            } else {
                0.0
            }),
            24 => Some(if self.params.auto_gain_enabled {
                1.0
            } else {
                0.0
            }),
            25 => Some(self.params.auto_gain_max_db as f64),
            26 => Some(self.params.auto_gain_smoothing_ms as f64),
            27 => Some(self.params.head_model as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
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
            16 => {}
            17 => self.params.room_width_m = value as f32,
            18 => self.params.room_depth_m = value as f32,
            19 => self.params.wall_absorption = value as f32,
            20 => self.params.reflection_beta_boost = value as f32,
            21 => self.params.bypass_xtc_filters = value > 0.5,
            22 => self.params.bypass_spectral_normalization = value > 0.5,
            23 => self.params.bypass_neumann_refinement = value > 0.5,
            24 => self.params.auto_gain_enabled = value > 0.5,
            25 => self.params.auto_gain_max_db = value as f32,
            26 => self.params.auto_gain_smoothing_ms = value as f32,
            27 => self.params.head_model = value as usize,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.diagnostics.cached_parameters =
            param_bridge::build_parameters(XT, |i| self.param_value(i));
        // Append parameters not in PARAMS
        self.diagnostics.cached_parameters.push(Parameter::new_bool(
            "enabled",
            "Enabled",
            self.params.enabled,
        ));
        self.diagnostics
            .cached_parameters
            .push(Parameter::new_float(
                "kappa_target",
                "Kappa Target",
                self.params.kappa_target,
                1.0,
                1000.0,
            ));
        self.diagnostics
            .cached_parameters
            .push(Parameter::new_string(
                "hrtf_file",
                "HRTF File",
                self.params.hrtf_file.clone().unwrap_or_default(),
            ));
        self.diagnostics
            .cached_parameters
            .push(Parameter::new_string(
                "source_mode",
                "Source Mode",
                self.params.source_mode.clone(),
            ));
        self.diagnostics
            .cached_parameters
            .push(Parameter::new_string(
                "recommended_matrix_file",
                "roomEQ Matrix",
                self.params
                    .recommended_matrix_file
                    .clone()
                    .unwrap_or_default(),
            ));
        self.diagnostics
            .cached_parameters
            .push(Parameter::new_string(
                "itd_modeling",
                "ITD Mode",
                self.params.itd_modeling.clone(),
            ));
    }

    /// Create from parameters helper
    pub fn from_params(params: XtcPluginParams, sample_rate: u32) -> Result<Self, String> {
        Self::new(params, sample_rate)
    }

    pub(super) fn set_crossfade_rate(&mut self) {
        let smooth_samples = self.params.head_tracking_smooth_s * self.fft.sample_rate as f32;
        self.filter_state.progress_per_hop = if smooth_samples > 0.0 {
            self.fft.hop_size as f32 / smooth_samples
        } else {
            1.0
        };
    }

    /// Recompute filters when parameters change.
    ///
    /// Optimization 3 & 4: Uses geometry cache and room reflection cache to avoid redundant computation.
    pub(super) fn update_filters(&mut self, sync: bool) {
        let num_bins = self.fft.fft_size / 2 + 1;
        let sample_rate = self.fft.sample_rate;

        self.set_crossfade_rate();

        if sync {
            let new_hash = compute_room_params_hash(&self.params);
            let room_data = if new_hash != self.filter_state.room_params_hash {
                compute_room_reflection_data(
                    &self.params,
                    sample_rate,
                    num_bins,
                    Some(self.fft.fft_forward.clone()),
                )
            } else {
                self.filter_state.room_reflection_cache.clone()
            };
            self.filter_state.room_reflection_cache = room_data.clone();
            self.filter_state.room_params_hash = new_hash;

            let hrtf_data = if self.params.source_mode == "hrtf_file" {
                let Some(hrtf_path) = self.params.hrtf_file.as_deref() else {
                    return;
                };
                let Ok(data) = load_hrtf_for_xtc(hrtf_path, &self.params, sample_rate, num_bins)
                else {
                    return;
                };
                data.map(Arc::new)
            } else {
                None
            };
            self.filter_state.hrtf_transfer_functions = hrtf_data.clone();

            let new_filters = if self.params.source_mode == "roomeq_recommended" {
                let Some(matrix_path) = self.params.recommended_matrix_file.as_deref() else {
                    return;
                };
                let Ok(filters) =
                    load_roomeq_recommended_filters(matrix_path, sample_rate, num_bins)
                else {
                    return;
                };
                filters
            } else {
                let cache = compute_geometry_cache(&self.params, sample_rate, num_bins);
                compute_xtc_filters_full_with_cache_and_hrtf(
                    &self.params,
                    sample_rate,
                    num_bins,
                    &cache,
                    room_data.clone(),
                    hrtf_data.as_deref(),
                )
            };
            let new_filters = Arc::new(new_filters);
            let previous_output_channels =
                self.filter_state.cached_current_filters.output_channels();
            let next_output_channels = new_filters.output_channels();
            if previous_output_channels != next_output_channels {
                return;
            }
            self.filter_state.filters.store(Arc::clone(&new_filters));
            self.filter_state.cached_current_filters = new_filters;
            self.reconfigure_auto_gain_for_layout();
            self.filter_state.pending_filter_update.store(None);
        } else {
            // Latest-only coalescing worker. Expensive recomputation never
            // fans out onto Rayon's global pool under high-rate automation.
            let generation = self
                .filter_state
                .filter_update_generation
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            let request = FilterUpdateRequest {
                generation,
                params: self.params.clone(),
                sample_rate,
                num_bins,
                expected_output_channels: self.output_channels(),
                fft_forward: self.fft.fft_forward.clone(),
            };
            *self
                .filter_state
                .filter_request
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(request);
            if self
                .filter_state
                .filter_worker_running
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }

            let request_mailbox = self.filter_state.filter_request.clone();
            let worker_running = self.filter_state.filter_worker_running.clone();
            let worker_running_on_error = worker_running.clone();
            let worker_launches = self.filter_state.filter_worker_launches.clone();
            let pending_filter_update = self.filter_state.pending_filter_update.clone();
            let requested_generation = self.filter_state.filter_update_generation.clone();
            let spawn_result = std::thread::Builder::new()
                .name("xtc-filter-worker".to_string())
                .spawn(move || {
                    worker_launches.fetch_add(1, Ordering::Relaxed);
                    loop {
                        let request = request_mailbox
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .take();
                        if let Some(request) = request {
                            if let Some(update) = Self::compute_filter_update(&request)
                                && requested_generation.load(Ordering::Acquire)
                                    == request.generation
                            {
                                pending_filter_update.store(Some(Arc::new(update)));
                            }
                            continue;
                        }

                        worker_running.store(false, Ordering::Release);
                        let has_raced_request = request_mailbox
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .is_some();
                        if has_raced_request
                            && worker_running
                                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                                .is_ok()
                        {
                            continue;
                        }
                        break;
                    }
                });
            if spawn_result.is_err() {
                worker_running_on_error.store(false, Ordering::Release);
            }
        }
    }

    fn compute_filter_update(request: &FilterUpdateRequest) -> Option<PendingFilterUpdate> {
        let params = &request.params;
        let room_params_hash = compute_room_params_hash(params);
        let room_data = compute_room_reflection_data(
            params,
            request.sample_rate,
            request.num_bins,
            Some(request.fft_forward.clone()),
        );
        let hrtf_data = if params.source_mode == "hrtf_file" {
            let hrtf_path = params.hrtf_file.as_deref()?;
            load_hrtf_for_xtc(hrtf_path, params, request.sample_rate, request.num_bins)
                .ok()?
                .map(Arc::new)
        } else {
            None
        };
        let new_filters = if params.source_mode == "roomeq_recommended" {
            let matrix_path = params.recommended_matrix_file.as_deref()?;
            load_roomeq_recommended_filters(matrix_path, request.sample_rate, request.num_bins)
                .ok()?
        } else {
            let cache = compute_geometry_cache(params, request.sample_rate, request.num_bins);
            compute_xtc_filters_full_with_cache_and_hrtf(
                params,
                request.sample_rate,
                request.num_bins,
                &cache,
                room_data.clone(),
                hrtf_data.as_deref(),
            )
        };
        if new_filters.output_channels() != request.expected_output_channels {
            return None;
        }
        Some(PendingFilterUpdate {
            generation: request.generation,
            filters: Arc::new(new_filters),
            hrtf_transfer_functions: hrtf_data,
            room_reflection_cache: room_data,
            room_params_hash,
        })
    }

    pub(super) fn adopt_pending_filters(&mut self) {
        let update = self.filter_state.pending_filter_update.load_full();
        let Some(update) = update.as_ref() else {
            return;
        };

        if update.generation
            != self
                .filter_state
                .filter_update_generation
                .load(Ordering::Acquire)
        {
            self.filter_state.pending_filter_update.store(None);
            return;
        }

        let previous = self.filter_state.filters.load_full();
        let previous_output_channels = previous.output_channels();
        self.filter_state.filters.store(Arc::clone(&update.filters));
        self.filter_state.cached_current_filters = Arc::clone(&update.filters);
        self.filter_state.hrtf_transfer_functions = update.hrtf_transfer_functions.clone();
        self.filter_state.room_reflection_cache = update.room_reflection_cache.clone();
        self.filter_state.room_params_hash = update.room_params_hash;
        let next_output_channels = update.filters.output_channels();
        if previous_output_channels != next_output_channels {
            self.filter_state.pending_filter_update.store(None);
            return;
        }
        self.filter_state.prev_filters = Some(previous);
        self.filter_state.crossfade_progress = 0.0;
        self.filter_state.pending_filter_update.store(None);
    }

    fn reconfigure_auto_gain_for_layout(&mut self) {
        if self.output_channels() != 2 || !self.params.auto_gain_enabled {
            self.dynamics.auto_gain = None;
            return;
        }
        if self.dynamics.auto_gain.is_none() {
            self.dynamics.auto_gain = AutoGain::new(
                2,
                self.fft.sample_rate,
                AutoGainParams {
                    enabled: true,
                    loudness_type: Default::default(),
                    max_gain_db: self.params.auto_gain_max_db,
                    smoothing_ms: self.params.auto_gain_smoothing_ms,
                },
            )
            .ok();
        }
    }

    /// Process one STFT frame using SIMD-optimized operations.
    ///
    /// During crossfade (after parameter change), blends output from old and new
    /// filters over ~100ms to avoid clicks. This costs 4 IFFTs per frame instead
    /// of the normal 2, but crossfade transitions are brief.
    #[inline(always)]
    pub(super) fn process_stft_frame(&mut self) {
        // Window and FFT left channel (SIMD optimized)
        window_mul_simd(
            &mut self.work.fft_buffer,
            &self.input.input_buffer_l,
            &self.fft.analysis_window,
        );
        self.fft
            .fft_forward
            .process(&mut self.work.fft_buffer, &mut self.work.fft_output_l)
            .expect("FFT processing failed");

        // Window and FFT right channel (SIMD optimized)
        window_mul_simd(
            &mut self.work.fft_buffer,
            &self.input.input_buffer_r,
            &self.fft.analysis_window,
        );
        self.fft
            .fft_forward
            .process(&mut self.work.fft_buffer, &mut self.work.fft_output_r)
            .expect("FFT processing failed");

        let scale = self.fft.output_scale;
        let fft_size = self.fft.fft_size;
        let mask = self.output.output_accumulator_mask;

        // Diagnostic bypass: skip all XTC filter math, just IFFT the windowed input.
        // This tests whether the STFT framework (windowing + OLA) itself is clean.
        if self.params.bypass_xtc_filters {
            let output_channels = self.filter_state.cached_current_filters.output_channels();

            // Left channel: IFFT the FFT output directly (identity in freq domain)
            self.work
                .ifft_input
                .copy_from_slice(&self.work.fft_output_l);
            let n = self.work.ifft_input.len();
            self.work.ifft_input[0].im = 0.0;
            self.work.ifft_input[n - 1].im = 0.0;
            self.fft
                .fft_inverse
                .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                .expect("IFFT processing failed");

            // Accumulate Left
            for i in 0..fft_size {
                let idx = (self.output.next_add_position + i) & mask;
                let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                self.output.output_accumulator[idx * output_channels] += s;
            }

            // Right channel
            self.work
                .ifft_input
                .copy_from_slice(&self.work.fft_output_r);
            self.work.ifft_input[0].im = 0.0;
            self.work.ifft_input[n - 1].im = 0.0;
            self.fft
                .fft_inverse
                .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                .expect("IFFT processing failed");

            // Accumulate Right
            for i in 0..fft_size {
                let idx = (self.output.next_add_position + i) & mask;
                let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                if output_channels > 1 {
                    self.output.output_accumulator[idx * output_channels + 1] += s;
                }
            }
        } else if let Some(speaker_filters) = self
            .filter_state
            .cached_current_filters
            .speaker_filters
            .as_ref()
        {
            let current_filters = &self.filter_state.cached_current_filters;
            let output_channels = current_filters.output_channels();
            let can_crossfade = self.filter_state.crossfade_progress < 1.0
                && self
                    .filter_state
                    .prev_filters
                    .as_ref()
                    .and_then(|prev| prev.speaker_filters.as_ref())
                    .is_some_and(|prev| prev.len() == output_channels);
            let alpha = self.filter_state.crossfade_progress;

            for (speaker_idx, filters_for_speaker) in speaker_filters.iter().enumerate() {
                if can_crossfade {
                    let prev_filters = self.filter_state.prev_filters.as_ref().unwrap();
                    let prev_speaker_filters = prev_filters.speaker_filters.as_ref().unwrap();
                    // Blend prev and current filters in frequency domain → 1 IFFT instead of 2.
                    apply_filter_pair_blended(
                        &mut self.work.ifft_input,
                        &self.work.fft_output_l,
                        &self.work.fft_output_r,
                        &prev_speaker_filters[speaker_idx][0],
                        &prev_speaker_filters[speaker_idx][1],
                        &filters_for_speaker[0],
                        &filters_for_speaker[1],
                        alpha,
                    );
                    self.fft
                        .fft_inverse
                        .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                        .expect("IFFT processing failed");

                    for i in 0..fft_size {
                        let idx = (self.output.next_add_position + i) & mask;
                        let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                        self.output.output_accumulator[idx * output_channels + speaker_idx] += s;
                    }
                } else {
                    apply_filter_pair(
                        &mut self.work.ifft_input,
                        &self.work.fft_output_l,
                        &self.work.fft_output_r,
                        &filters_for_speaker[0],
                        &filters_for_speaker[1],
                    );
                    self.fft
                        .fft_inverse
                        .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                        .expect("IFFT processing failed");

                    for i in 0..fft_size {
                        let idx = (self.output.next_add_position + i) & mask;
                        let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                        self.output.output_accumulator[idx * output_channels + speaker_idx] += s;
                    }
                }
            }
        } else if self.filter_state.crossfade_progress < 1.0
            && let Some(prev_filters) = self.filter_state.prev_filters.as_ref()
        {
            let alpha = self.filter_state.crossfade_progress;
            // Use cached filter snapshot (loaded once per process() call)
            let current_filters = &self.filter_state.cached_current_filters;

            // --- Left channel with frequency-domain crossfade (1 IFFT instead of 2) ---
            // Blend prev and current filters per bin, then run a single IFFT.
            // Valid because IFFT is linear: (1-α)·IFFT(prev) + α·IFFT(curr) = IFFT(blended).
            apply_filter_left_blended(
                &mut self.work.ifft_input,
                &self.work.fft_output_l,
                &self.work.fft_output_r,
                prev_filters,
                current_filters,
                alpha,
            );
            self.fft
                .fft_inverse
                .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                .expect("IFFT processing failed");

            for i in 0..fft_size {
                let idx = (self.output.next_add_position + i) & mask;
                let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                self.output.output_accumulator[idx * 2] += s;
            }

            // --- Right channel with frequency-domain crossfade (1 IFFT instead of 2) ---
            apply_filter_right_blended(
                &mut self.work.ifft_input,
                &self.work.fft_output_l,
                &self.work.fft_output_r,
                prev_filters,
                current_filters,
                alpha,
            );
            self.fft
                .fft_inverse
                .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                .expect("IFFT processing failed");

            for i in 0..fft_size {
                let idx = (self.output.next_add_position + i) & mask;
                let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                self.output.output_accumulator[idx * 2 + 1] += s;
            }
        } else {
            // Normal path: no crossfade needed
            let filters = &self.filter_state.cached_current_filters;

            // Left channel
            apply_filter_left(
                &mut self.work.ifft_input,
                &self.work.fft_output_l,
                &self.work.fft_output_r,
                filters,
            );
            self.fft
                .fft_inverse
                .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                .expect("IFFT processing failed");

            for i in 0..fft_size {
                let idx = (self.output.next_add_position + i) & mask;
                let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                self.output.output_accumulator[idx * 2] += s;
            }

            // Right channel
            apply_filter_right(
                &mut self.work.ifft_input,
                &self.work.fft_output_l,
                &self.work.fft_output_r,
                filters,
            );
            self.fft
                .fft_inverse
                .process(&mut self.work.ifft_input, &mut self.work.ifft_output)
                .expect("IFFT processing failed");

            for i in 0..fft_size {
                let idx = (self.output.next_add_position + i) & mask;
                let s = self.work.ifft_output[i] * self.fft.analysis_window[i] * scale;
                self.output.output_accumulator[idx * 2 + 1] += s;
            }
        }

        // Update positions
        self.output.next_add_position = (self.output.next_add_position + self.fft.hop_size) & mask;

        // Start draining immediately to match physical latency.
        self.output.output_accumulator_fill += self.fft.hop_size;
        self.output.latency_filled += self.fft.hop_size;

        // Advance crossfade progress
        if self.filter_state.crossfade_progress < 1.0 {
            self.filter_state.crossfade_progress = (self.filter_state.crossfade_progress
                + self.filter_state.progress_per_hop)
                .min(1.0);
            if self.filter_state.crossfade_progress >= 1.0 {
                self.filter_state.prev_filters = None; // Release old filters
            }
        }
    }

    /// Shift input buffer left by hop_size and clear tail
    #[inline(always)]
    pub(super) fn shift_input_buffer(&mut self) {
        let overlap = self.fft.fft_size - self.fft.hop_size;
        self.input
            .input_buffer_l
            .copy_within(self.fft.hop_size.., 0);
        self.input
            .input_buffer_r
            .copy_within(self.fft.hop_size.., 0);
        // Clear the tail (will be filled with new samples)
        self.input.input_buffer_l[overlap..].fill(0.0);
        self.input.input_buffer_r[overlap..].fill(0.0);
        self.input.input_fill = overlap;
    }
}

impl Plugin for XtcPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Crosstalk Cancellation (XTC)", "2.0.0", "SotF").with_description(format!(
            "Crosstalk cancellation (Async) - FFT size: {}, speakers at {}° and {}m",
            self.fft.fft_size, self.params.speaker_angle_deg, self.params.distance_m
        ))
    }

    fn input_channels(&self) -> usize {
        2 // Stereo input
    }

    fn output_channels(&self) -> usize {
        self.filter_state.cached_current_filters.output_channels()
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        if self.params.auto_gain_enabled {
            return PluginCompileMetadata::boundary(PluginCostClass::Fft, self.latency_samples());
        }
        PluginCompileMetadata::linear_transform(
            PluginCostClass::Fft,
            None,
            self.latency_samples(),
            true,
            true,
            false,
        )
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.diagnostics.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Parameters not in PARAMS — handle separately
        if id.as_str() == "enabled" {
            self.params.enabled = value
                .as_bool()
                .ok_or_else(|| "enabled must be a boolean".to_string())?;
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "kappa_target" {
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
        if id.as_str() == "hrtf_file" {
            let v = value
                .as_string()
                .ok_or_else(|| "hrtf_file must be a string".to_string())?;
            let mut candidate = self.params.clone();
            candidate.hrtf_file = (!v.is_empty()).then(|| v.to_string());
            if candidate.hrtf_file != self.params.hrtf_file {
                return Err(Self::STRUCTURAL_SOURCE_ERROR.to_string());
            }
            Self::validate_source_configuration(
                &candidate,
                self.fft.sample_rate,
                self.fft.fft_size / 2 + 1,
            )?;
            self.params = candidate;
            self.update_filters(false);
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "room_ir_file" {
            let v = value
                .as_string()
                .ok_or_else(|| "room_ir_file must be a string".to_string())?;
            let candidate = (!v.is_empty()).then(|| v.to_string());
            if candidate != self.params.room_ir_file {
                return Err(Self::STRUCTURAL_SOURCE_ERROR.to_string());
            }
            return Ok(());
        }
        if id.as_str() == "source_mode" {
            let v = value
                .as_string()
                .ok_or_else(|| "source_mode must be a string".to_string())?;
            let mut candidate = self.params.clone();
            candidate.source_mode = v.to_string();
            if candidate.source_mode != self.params.source_mode {
                return Err(Self::STRUCTURAL_SOURCE_ERROR.to_string());
            }
            Self::validate_source_configuration(
                &candidate,
                self.fft.sample_rate,
                self.fft.fft_size / 2 + 1,
            )?;
            self.params = candidate;
            self.update_filters(false);
            self.rebuild_cached_parameters();
            return Ok(());
        }
        if id.as_str() == "recommended_matrix_file" {
            let v = value
                .as_string()
                .ok_or_else(|| "recommended_matrix_file must be a string".to_string())?;
            let candidate = (!v.is_empty()).then(|| v.to_string());
            if candidate != self.params.recommended_matrix_file {
                return Err(Self::STRUCTURAL_SOURCE_ERROR.to_string());
            }
            return Ok(());
        }
        if id.as_str() == "itd_modeling" {
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

        if id.as_str() == "room_reflections_enabled"
            && value
                .as_bool()
                .ok_or_else(|| "room_reflections_enabled must be boolean".to_string())?
        {
            let mut candidate = self.params.clone();
            candidate.room_reflections_enabled = true;
            Self::validate_room_ir_configuration(
                &candidate,
                self.fft.sample_rate,
                self.fft.fft_size / 2 + 1,
                self.fft.fft_forward.clone(),
            )?;
        }

        let idx = param_bridge::set_parameter(XT, &id, &value, |i, v| self.set_param_value(i, v))?;

        // Side effects based on parameter index
        let needs_filter_update = match idx {
            0..=5 => true,   // geometry + head tracking
            7..=9 => true,   // beta
            10..=12 => true, // shadow + filter
            13 => true,      // spectral_normalization
            14 => true,      // pinna_model_enabled
            15..=20 => true, // room, including room_ir_file
            21 => {
                // bypass_xtc_filters
                self.dynamics.limiter_envelope = 1.0;
                false
            }
            22 | 23 => true, // bypass_spectral_normalization, bypass_neumann_refinement
            24 => {
                // auto_gain_enabled
                if self.output_channels() != 2 {
                    self.dynamics.auto_gain = None;
                } else if self.params.auto_gain_enabled && self.dynamics.auto_gain.is_none() {
                    self.dynamics.auto_gain = Some(AutoGain::new(
                        2,
                        self.fft.sample_rate,
                        AutoGainParams {
                            enabled: true,
                            loudness_type: Default::default(),
                            max_gain_db: self.params.auto_gain_max_db,
                            smoothing_ms: self.params.auto_gain_smoothing_ms,
                        },
                    )?);
                } else if !self.params.auto_gain_enabled {
                    self.dynamics.auto_gain = None;
                }
                false
            }
            25 => {
                // auto_gain_max_db
                if let Some(ag) = &mut self.dynamics.auto_gain {
                    ag.set_max_gain_db(self.params.auto_gain_max_db);
                }
                false
            }
            26 => {
                // auto_gain_smoothing_ms
                if let Some(ag) = &mut self.dynamics.auto_gain {
                    ag.set_smoothing_ms(self.params.auto_gain_smoothing_ms);
                }
                false
            }
            27 => true, // head_model
            _ => false,
        };

        if needs_filter_update {
            self.update_filters(false);
        }
        self.rebuild_cached_parameters();

        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.as_str() == "fft_size" {
            return Some(ParameterValue::Int(self.params.fft_size as i32));
        }
        // Parameters not in PARAMS — handle separately
        if id.as_str() == "enabled" {
            return Some(ParameterValue::Bool(self.params.enabled));
        }
        if id.as_str() == "kappa_target" {
            return Some(ParameterValue::Float(self.params.kappa_target));
        }
        if id.as_str() == "hrtf_file" {
            return Some(ParameterValue::String(
                self.params.hrtf_file.clone().unwrap_or_default(),
            ));
        }
        if id.as_str() == "room_ir_file" {
            return Some(ParameterValue::String(
                self.params.room_ir_file.clone().unwrap_or_default(),
            ));
        }
        if id.as_str() == "source_mode" {
            return Some(ParameterValue::String(self.params.source_mode.clone()));
        }
        if id.as_str() == "recommended_matrix_file" {
            return Some(ParameterValue::String(
                self.params
                    .recommended_matrix_file
                    .clone()
                    .unwrap_or_default(),
            ));
        }
        if id.as_str() == "itd_modeling" {
            return Some(ParameterValue::String(self.params.itd_modeling.clone()));
        }
        param_bridge::get_parameter(XT, id, |i| self.param_value(i))
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.diagnostics.cache.load() as Arc<dyn Any + Send + Sync>)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.fft.sample_rate = sample_rate;
        self.dynamics.limiter_attack_coeff =
            math_audio_dsp::fast_math::fast_exp(-1.0 / (0.2 * 0.001 * sample_rate as f32));
        self.dynamics.limiter_release_coeff =
            math_audio_dsp::fast_math::fast_exp(-1.0 / (50.0 * 0.001 * sample_rate as f32));
        self.update_filters(true); // Synchronous for initialization

        // Pre-allocate temp buffers to the validated maximum XTC block size.
        // After this, the resize() check in process() is a guaranteed no-op.
        self.input.temp_input_l.resize(MAX_PROCESS_FRAMES, 0.0);
        self.input.temp_input_r.resize(MAX_PROCESS_FRAMES, 0.0);

        if let Some(ag) = &mut self.dynamics.auto_gain {
            ag.set_sample_rate(sample_rate).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn reset(&mut self) {
        // Clear all buffers
        self.input.input_buffer_l.fill(0.0);
        self.input.input_buffer_r.fill(0.0);
        self.output.output_accumulator.fill(0.0);
        self.output.output_accumulator_fill = 0;
        self.output.next_add_position = 0;
        self.output.output_read_position = 0;
        self.work.prev_ifft_output.fill(0.0);
        self.input.input_fill = 0;
        self.output.latency_filled = 0;

        // Reset crossfade state
        self.filter_state.prev_filters = None;
        self.filter_state.crossfade_progress = 1.0;

        if let Some(ag) = &mut self.dynamics.auto_gain {
            ag.reset();
        }
        self.dynamics.limiter_envelope = 1.0;
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        self.adopt_pending_filters();
        let output_channels = self.output_channels();

        // Verify buffer sizes (stereo input, dynamic speaker output for roomEQ matrices)
        if input.len() != num_frames * 2 {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                num_frames * 2,
                input.len()
            ));
        }
        if output.len() != num_frames * output_channels {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                num_frames * output_channels,
                output.len()
            ));
        }

        // Measure loudness (throttled to 1/10 blocks to save CPU)
        self.diagnostics.cache_update_counter += 1;
        let mut do_measure = false;
        if self.diagnostics.cache_update_counter >= 10 {
            self.diagnostics.cache_update_counter = 0;
            do_measure = true;
        }

        // Measure input loudness for auto-gain (before any processing)
        if do_measure && let Some(ag) = &mut self.dynamics.auto_gain {
            let _ = ag.measure_input(input);
        }

        // Bypass if disabled
        if !self.params.enabled {
            for frame in 0..num_frames {
                let input_base = frame * 2;
                let output_base = frame * output_channels;
                output[output_base] = input[input_base];
                if output_channels > 1 {
                    output[output_base + 1] = input[input_base + 1];
                }
                for ch in 2..output_channels {
                    output[output_base + ch] = 0.0;
                }
            }

            // Still update diagnostic cache when bypassed
            if do_measure {
                let ag_data = self
                    .dynamics
                    .auto_gain
                    .as_ref()
                    .map(|ag| ag.get_data())
                    .unwrap_or_default();
                self.diagnostics.cache.update(|d| {
                    d.auto_gain = ag_data;
                    d.limiter_envelope = 1.0;
                });
            }
            return Ok(context.num_frames);
        }

        // Snapshot current filters once per process() call (avoids per-frame ArcSwap::load atomic ops)
        self.filter_state.cached_current_filters =
            arc_swap::Guard::into_inner(self.filter_state.filters.load());

        let mut output_pos = 0;
        let mask = self.output.output_accumulator_mask;
        let mut block_start = 0;

        while block_start < num_frames && output_pos < num_frames {
            let block_frames = self.input.temp_input_l.len().min(num_frames - block_start);
            let input_start = block_start * 2;
            let input_end = input_start + block_frames * 2;

            deinterleave_stereo(
                &input[input_start..input_end],
                &mut self.input.temp_input_l[..block_frames],
                &mut self.input.temp_input_r[..block_frames],
            );

            let mut input_pos = 0;
            while output_pos < num_frames {
                // Step 1: Fill input buffer from deinterleaved temp buffers
                if input_pos < block_frames {
                    let samples_needed = self.fft.fft_size - self.input.input_fill;
                    let samples_available_in = block_frames - input_pos;
                    let to_copy = samples_needed.min(samples_available_in);

                    if to_copy > 0 {
                        self.input.input_buffer_l
                            [self.input.input_fill..self.input.input_fill + to_copy]
                            .copy_from_slice(
                                &self.input.temp_input_l[input_pos..input_pos + to_copy],
                            );
                        self.input.input_buffer_r
                            [self.input.input_fill..self.input.input_fill + to_copy]
                            .copy_from_slice(
                                &self.input.temp_input_r[input_pos..input_pos + to_copy],
                            );
                        self.input.input_fill += to_copy;
                        input_pos += to_copy;
                    }
                }

                // Step 2: Process ALL possible STFT frames from current input
                while self.input.input_fill >= self.fft.fft_size {
                    self.process_stft_frame();
                    self.shift_input_buffer();
                }

                // Step 3: Copy available output to output buffer
                let frames_to_drain = self
                    .output
                    .output_accumulator_fill
                    .min(num_frames - output_pos);

                if frames_to_drain > 0 {
                    for i in 0..frames_to_drain {
                        let read_idx = (self.output.output_read_position + i) & mask;
                        let acc_base = read_idx * output_channels;
                        let out_base = (output_pos + i) * output_channels;
                        output[out_base..out_base + output_channels].copy_from_slice(
                            &self.output.output_accumulator[acc_base..acc_base + output_channels],
                        );
                        // Clear after reading for next overlap-add cycle
                        self.output.output_accumulator[acc_base..acc_base + output_channels]
                            .fill(0.0);
                    }
                    self.output.output_read_position =
                        (self.output.output_read_position + frames_to_drain) & mask;
                    self.output.output_accumulator_fill -= frames_to_drain;
                    output_pos += frames_to_drain;
                } else if input_pos >= block_frames {
                    break;
                }
            }

            block_start += block_frames;
        }

        // Auto-gain: measure the UNCOMPENSATED output from the plugin filters.
        // This ensures the gain calculation is stable and doesn't oscillate.
        if let Some(ag) = &mut self.dynamics.auto_gain {
            if do_measure {
                let _ = ag.measure_output(&output[..output_pos * output_channels]);

                // Update diagnostic cache (Real-time safe, throttled)
                let ag_data = ag.get_data();
                let limiter_env = self.dynamics.limiter_envelope;
                self.diagnostics.cache.update(|d| {
                    d.auto_gain = ag_data;
                    d.limiter_envelope = limiter_env;
                });
            }
            ag.apply_compensation(&mut output[..output_pos * output_channels], output_pos);
        }

        // Per-sample peak limiter: prevent clipping after XTC filter summation + AutoGain.
        // Smooth attack (~0.2ms) and release (~50ms) to avoid gain modulation artifacts.
        // Skip when filters are bypassed — no amplification occurs.
        if !self.params.bypass_xtc_filters && output_pos > 0 {
            let threshold = 0.95_f32;
            for frame in 0..output_pos {
                let base = frame * output_channels;
                let frame_slice = &output[base..base + output_channels];
                let peak = frame_slice
                    .iter()
                    .map(|sample| sample.abs())
                    .fold(0.0, f32::max);
                let target_gr = if peak > threshold {
                    threshold / peak
                } else {
                    1.0
                };
                if target_gr < self.dynamics.limiter_envelope {
                    // Smooth attack (~0.2ms) to avoid per-sample gain jumps
                    self.dynamics.limiter_envelope = target_gr
                        + self.dynamics.limiter_attack_coeff
                            * (self.dynamics.limiter_envelope - target_gr);
                } else {
                    self.dynamics.limiter_envelope = target_gr
                        + self.dynamics.limiter_release_coeff
                            * (self.dynamics.limiter_envelope - target_gr);
                }
                for ch in 0..output_channels {
                    let idx = base + ch;
                    output[idx] *= self.dynamics.limiter_envelope;
                    // Hard clamp: the one-pole envelope has finite attack time, so a
                    // few samples can overshoot during transient onset. Clamp to ±1.0
                    // as a safety ceiling — matches standard digital limiter practice.
                    output[idx] = output[idx].clamp(-1.0, 1.0);
                }
            }
        }

        output[output_pos * output_channels..].fill(0.0);

        // STFT plugins must return context.num_frames (not output_pos) to prevent
        // ring buffer underrun in the host. Unproduced frames are already zeroed.
        flush_denormals_inplace(output);
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        // Output becomes observable in the host block that completes the first
        // FFT frame. Reporting one full frame keeps latency independent of host
        // block size and bounds compensation error to one block.
        self.fft.fft_size
    }
}
