pub use super::config::DenoiserPluginParams;
use super::denoiser_data::DenoiserData;
use super::misc::MIN_IN_PLACE_BLOCK_FRAMES;
use super::misc::NUM_DISPLAY_BANDS;
use crate::params::PARAMS as DN;
use math_audio_dsp::simd::ScopedFtz;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_bridge;
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{PluginInfo, PluginResult, ProcessContext};
use sotf_plugin_pnd::analysis::PndAnalyzer;
use std::any::Any;
use std::sync::Arc;

/// Static configuration for the denoiser.
pub(super) struct DenoiserConfig {
    pub channels: usize,
    pub fft_size: usize,
    pub hop_size: usize,
    pub sample_rate: u32,
    pub spectrum_size: usize, // fft_size / 2 + 1
}

/// FFT planners and time/frequency-domain buffers.
pub(super) struct DenoiserFft {
    pub fft_forward: Arc<dyn RealToComplex<f32>>,
    pub fft_inverse: Arc<dyn ComplexToReal<f32>>,
    pub window: Vec<f32>,
    pub synthesis_window: Vec<f32>,
    pub time_domain: Vec<Vec<f32>>, // [channels][fft_size]
    pub freq_domain: Vec<Vec<Complex<f32>>>, // [channels][spectrum_size]
}

/// User-facing runtime parameters.
pub(super) struct DenoiserParameters {
    pub reduction_db: f32,
    pub floor_db: f32,
    pub smoothing: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub low_latency: bool,
    pub polyphonic_detection: bool,
    pub transparency: f32,
    pub psychoacoustic_masking: bool,
    pub spectral_smoothing_enabled: bool,
    pub temporal_smoothing_enabled: bool,
    pub harmonic_percussive: bool,
    pub spatial_denoise: bool,
    pub spatial_strength: f32,
}

/// Pre-computed smoothing coefficients.
pub(super) struct DenoiserCoefficients {
    pub attack_coeff: f32,
    pub release_coeff: f32,
    pub reduction_linear: f32,
    pub floor_linear: f32,
}

/// IMCRA/MCRA noise-estimation state and hyperparameters.
pub(super) struct DenoiserMcra {
    pub noise_psd: Vec<Vec<f32>>, // Estimated noise power spectrum
    pub smoothed_psd: Vec<Vec<f32>>, // Smoothed signal PSD (S_tmp)
    pub min_psd: Vec<Vec<f32>>,   // Minimum PSD tracker — window A
    pub min_psd_b: Vec<Vec<f32>>, // Minimum PSD tracker — window B (IMCRA)
    pub speech_presence: Vec<Vec<f32>>, // Speech presence probability (p)
    pub frame_counter: Vec<usize>, // Per-channel frame count
    pub mcra_alpha_s: f32,
    pub mcra_alpha_p: f32,
    pub mcra_l: usize,
    pub mcra_delta: f32,
}

/// Captured noise profile state.
pub(super) struct DenoiserNoiseProfile {
    pub use_captured_profile: bool,
    pub has_noise_profile: bool,
    pub noise_profile_storage: Vec<Vec<f32>>, // [channels][spectrum_size] pre-allocated
    pub learning_accumulator: Vec<Vec<f32>>,  // [channels][spectrum_size]
    pub learning_frames_count: usize,
    pub learning_frames_target: usize,
    pub is_learning: bool,
}

/// Psychoacoustic masking scratch and precomputed mappings.
pub(super) struct DenoiserMasking {
    pub bark_map: Vec<f32>, // [spectrum_size] frequency-to-Bark mapping
    pub bark_bin_range: Vec<(usize, usize)>, // [spectrum_size] precomputed (lo, hi) bin range within MAX_SPREAD_BARK
    pub masking_threshold: Vec<f32>, // [spectrum_size] scratch for masking thresholds
    pub masking_signal_power: Vec<f32>, // [spectrum_size] scratch for signal power
}

/// Decision-directed SNR parameters and state.
pub(super) struct DenoiserDecisionDirected {
    pub dd_enabled: bool,
    pub dd_alpha: f32,
    pub prev_power: Vec<Vec<f32>>, // [channels][spectrum_size] previous frame power
}

/// Wiener gain buffers and frequency-smoothing state.
pub(super) struct DenoiserGains {
    pub gain: Vec<Vec<f32>>, // Current Wiener gains per bin
    pub smoothed_gain: Vec<Vec<f32>>, // Temporally smoothed gains
    pub freq_smooth_temp: Vec<f32>, // [spectrum_size] scratch for smoothing across bins
    pub freq_smooth_kernel: (f32, f32, f32), // Precomputed (c0, c1, c2) Gaussian weights
}

/// Spectral subtraction parameters.
pub(super) struct DenoiserSpectralSub {
    pub spectral_sub_enabled: bool,
    pub spectral_sub_alpha: f32,
    pub spectral_sub_beta: f32,
}

/// Spatial denoising state.
pub(super) struct DenoiserSpatial {
    pub spatial_coherence: Vec<f32>, // Smoothed MSC coherence estimate per bin
    pub spatial_cross: Vec<Complex<f32>>, // Averaged complex cross-spectrum estimate per bin
}

/// Harmonic/percussive separation state.
pub(super) struct DenoiserTonalTransient {
    pub tonal_transient_seps: Vec<math_audio_dsp::tonal_transient::TonalTransientSeparator>,
    pub tt_magnitudes: Vec<f32>, // [spectrum_size]
    pub tt_tonal_mask: Vec<f32>, // [spectrum_size]
    pub tt_transient_mask: Vec<f32>, // [spectrum_size]
}

/// Overlap-add input/output buffers.
pub(super) struct DenoiserIo {
    pub input_buffer: Vec<f32>, // Interleaved input accumulator
    pub input_buffer_fill: usize,
    pub temp_input_block: Vec<f32>, // Pre-allocated block for FFT input
    pub output_accumulator: Vec<Vec<f32>>, // [channels][ring_capacity]
    pub output_ring_mask: usize,           // ring_capacity - 1 (for & masking)
    pub output_read_pos: usize,            // read position in ring
    pub output_write_pos: usize,           // next overlap-add write position
    pub output_accumulator_fill: usize,    // frames available for reading
    pub time_out_channels: Vec<Vec<f32>>,
}

/// Auxiliary per-channel processing objects.
pub(super) struct DenoiserAuxiliary {
    pub pnd_analyzers: Vec<PndAnalyzer>,
    pub formant_preserver: super::wiener::FormantPreserver,
}

/// Cached monitoring/UI data.
pub(super) struct DenoiserUi {
    pub avg_reduction_db: f32,
    pub learning_active: bool,
    pub cache: RealTimeCache<DenoiserData>,
    pub data_update_counter: usize,
    pub cached_noise_floor_buf: Vec<f32>, // [NUM_DISPLAY_BANDS] pre-allocated
    pub cached_snr_buf: Vec<f32>,         // [NUM_DISPLAY_BANDS] pre-allocated
    pub cached_parameters: Vec<Parameter>,
}

/// Multi-resolution dual-STFT state.
pub(super) struct DenoiserMultiRes {
    pub multi_resolution: bool,
    /// `Some(state)` when multi_resolution is enabled, `None` otherwise.
    /// Stored as an Option so that when disabled the extra RAM is not held.
    pub multi_res_state: Option<super::multi_resolution::MultiResState>,
}

/// Spectral denoiser using Wiener filter with MCRA noise estimation
pub struct DenoiserPlugin {
    pub(super) config: DenoiserConfig,
    pub(super) fft: DenoiserFft,
    pub(super) params: DenoiserParameters,
    pub(super) coeffs: DenoiserCoefficients,
    pub(super) mcra: DenoiserMcra,
    pub(super) noise_profile: DenoiserNoiseProfile,
    pub(super) masking: DenoiserMasking,
    pub(super) decision_directed: DenoiserDecisionDirected,
    pub(super) gains: DenoiserGains,
    pub(super) spectral_sub: DenoiserSpectralSub,
    pub(super) spatial: DenoiserSpatial,
    pub(super) tonal_transient: DenoiserTonalTransient,
    pub(super) io: DenoiserIo,
    pub(super) auxiliary: DenoiserAuxiliary,
    pub(super) ui: DenoiserUi,
    pub(super) multi_res: DenoiserMultiRes,
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

        // Generate sqrt(Hann) window for WOLA processing
        // Analysis: sqrt(Hann), Synthesis: sqrt(Hann), Product = Hann → perfect COLA at 50% overlap
        let window = sotf_host::stft_common::generate_sqrt_hann_window(fft_size);
        let inv_fft_size = 1.0 / fft_size as f32;
        let synthesis_window: Vec<f32> = window.iter().map(|w| w * inv_fft_size).collect();

        // Allocate buffers
        let time_domain = vec![vec![0.0_f32; fft_size]; channels];
        let freq_domain = vec![vec![Complex::new(0.0, 0.0); spectrum_size]; channels];

        // MCRA state
        let noise_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let smoothed_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let min_psd = vec![vec![0.0_f32; spectrum_size]; channels];
        let min_psd_b = vec![vec![0.0_f32; spectrum_size]; channels];
        let speech_presence = vec![vec![0.0_f32; spectrum_size]; channels];
        let frame_counter = vec![0_usize; channels];

        // Wiener filter state
        let gain = vec![vec![1.0_f32; spectrum_size]; channels];
        let smoothed_gain = vec![vec![1.0_f32; spectrum_size]; channels];

        // Overlap-add buffers
        // Input buffer needs to hold fft_size samples (interleaved)
        let input_buffer = vec![0.0_f32; fft_size * channels * 2];
        // Output ring buffer includes the advertised in-place block size plus
        // one FFT-sized overlap tail that may be written before draining.
        let ring_capacity = Self::output_ring_capacity_for_fft(fft_size);
        let output_accumulator = vec![vec![0.0_f32; ring_capacity]; channels];
        let time_out_channels = vec![vec![0.0_f32; fft_size]; channels];

        // PND Analyzers for polyphonic detection
        let pnd_analyzers = (0..channels)
            .map(|_| PndAnalyzer::new(2048, 44100, 50.0))
            .collect();

        let mut p = Self {
            config: DenoiserConfig {
                channels,
                fft_size,
                hop_size,
                sample_rate: 44100, // Updated in initialize()
                spectrum_size,
            },

            fft: DenoiserFft {
                fft_forward,
                fft_inverse,
                window,
                synthesis_window,
                time_domain,
                freq_domain,
            },

            params: DenoiserParameters {
                reduction_db: pk(DN, "reduction_db").default_f32(),
                floor_db: pk(DN, "floor_db").default_f32(),
                smoothing: pk(DN, "smoothing").default_f32(),
                attack_ms: pk(DN, "attack_ms").default_f32(),
                release_ms: pk(DN, "release_ms").default_f32(),
                low_latency,
                polyphonic_detection: pk(DN, "polyphonic_detection").default_bool(),
                transparency: pk(DN, "transparency").default_f32(),
                psychoacoustic_masking: pk(DN, "psychoacoustic_masking").default_bool(),
                spectral_smoothing_enabled: pk(DN, "spectral_smoothing_enabled").default_bool(),
                temporal_smoothing_enabled: pk(DN, "temporal_smoothing_enabled").default_bool(),
                harmonic_percussive: false,
                spatial_denoise: false,
                spatial_strength: 0.5,
            },

            coeffs: DenoiserCoefficients {
                attack_coeff: Self::time_to_coeff(pk(DN, "attack_ms").default_f32(), 44100, hop_size),
                release_coeff: Self::time_to_coeff(pk(DN, "release_ms").default_f32(), 44100, hop_size),
                reduction_linear: 10.0_f32.powf(pk(DN, "reduction_db").default_f32() / 10.0),
                floor_linear: 10.0_f32.powf(pk(DN, "floor_db").default_f32() / 20.0),
            },

            mcra: DenoiserMcra {
                noise_psd,
                smoothed_psd,
                min_psd,
                min_psd_b,
                speech_presence,
                frame_counter,
                mcra_alpha_s: pk(DN, "mcra_alpha_s").default_f32(),
                mcra_alpha_p: pk(DN, "mcra_alpha_p").default_f32(),
                mcra_l: pk(DN, "mcra_l").default_usize(),
                mcra_delta: pk(DN, "mcra_delta").default_f32(),
            },

            noise_profile: DenoiserNoiseProfile {
                use_captured_profile: pk(DN, "use_captured_profile").default_bool(),
                has_noise_profile: false,
                noise_profile_storage: vec![vec![0.0_f32; spectrum_size]; channels],
                learning_accumulator: vec![vec![0.0_f32; spectrum_size]; channels],
                learning_frames_count: 0,
                learning_frames_target: crate::params::LEARN_FRAMES,
                is_learning: false,
            },

            masking: DenoiserMasking {
                bark_map: vec![0.0_f32; spectrum_size],
                bark_bin_range: vec![(0, 0); spectrum_size],
                masking_threshold: vec![0.0_f32; spectrum_size],
                masking_signal_power: vec![0.0_f32; spectrum_size],
            },

            decision_directed: DenoiserDecisionDirected {
                dd_enabled: pk(DN, "dd_enabled").default_bool(),
                dd_alpha: pk(DN, "dd_alpha").default_f32(),
                prev_power: vec![vec![0.0_f32; spectrum_size]; channels],
            },

            gains: DenoiserGains {
                gain,
                smoothed_gain,
                freq_smooth_temp: vec![0.0_f32; spectrum_size],
                freq_smooth_kernel: Self::compute_smoothing_kernel(pk(DN, "smoothing").default_f32()),
            },

            spectral_sub: DenoiserSpectralSub {
                spectral_sub_enabled: pk(DN, "spectral_sub_enabled").default_bool(),
                spectral_sub_alpha: pk(DN, "spectral_sub_alpha").default_f32(),
                spectral_sub_beta: pk(DN, "spectral_sub_beta").default_f32(),
            },

            spatial: DenoiserSpatial {
                spatial_coherence: vec![1.0_f32; spectrum_size],
                spatial_cross: vec![Complex::new(1.0_f32, 0.0_f32); spectrum_size],
            },

            tonal_transient: DenoiserTonalTransient {
                tonal_transient_seps: (0..channels)
                    .map(|_| {
                        math_audio_dsp::tonal_transient::TonalTransientSeparator::new(
                            spectrum_size,
                            7,
                            7,
                        )
                    })
                    .collect(),
                tt_magnitudes: vec![0.0; spectrum_size],
                tt_tonal_mask: vec![0.0; spectrum_size],
                tt_transient_mask: vec![0.0; spectrum_size],
            },

            io: DenoiserIo {
                input_buffer,
                input_buffer_fill: 0,
                temp_input_block: vec![0.0_f32; fft_size * channels],
                output_accumulator,
                output_ring_mask: ring_capacity - 1,
                output_read_pos: 0,
                output_write_pos: 0,
                output_accumulator_fill: 0,
                time_out_channels,
            },

            auxiliary: DenoiserAuxiliary {
                pnd_analyzers,
                formant_preserver: super::wiener::FormantPreserver::new(spectrum_size),
            },

            ui: DenoiserUi {
                avg_reduction_db: 0.0,
                learning_active: true,
                cache: RealTimeCache::new(DenoiserData::default()),
                data_update_counter: 0,
                cached_noise_floor_buf: vec![0.0; NUM_DISPLAY_BANDS],
                cached_snr_buf: vec![0.0; NUM_DISPLAY_BANDS],
                cached_parameters: Vec::new(),
            },

            multi_res: DenoiserMultiRes {
                multi_resolution: pk(DN, "multi_resolution").default_bool(),
                multi_res_state: None, // allocated on first enable
            },
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.params.reduction_db as f64),
            1 => Some(self.params.floor_db as f64),
            2 => Some(self.params.smoothing as f64),
            3 => Some(self.params.attack_ms as f64),
            4 => Some(self.params.release_ms as f64),
            5 => Some(if self.params.low_latency { 1.0 } else { 0.0 }),
            6 => Some(if self.params.polyphonic_detection { 1.0 } else { 0.0 }),
            7 => Some(self.mcra.mcra_alpha_s as f64),
            8 => Some(self.mcra.mcra_alpha_p as f64),
            9 => Some(self.mcra.mcra_l as f64),
            10 => Some(self.mcra.mcra_delta as f64),
            11 => Some(self.params.transparency as f64),
            12 => Some(if self.decision_directed.dd_enabled { 1.0 } else { 0.0 }),
            13 => Some(self.decision_directed.dd_alpha as f64),
            14 => Some(if self.params.psychoacoustic_masking {
                1.0
            } else {
                0.0
            }),
            15 => Some(if self.params.spectral_smoothing_enabled {
                1.0
            } else {
                0.0
            }),
            16 => Some(if self.params.temporal_smoothing_enabled {
                1.0
            } else {
                0.0
            }),
            17 => Some(if self.spectral_sub.spectral_sub_enabled { 1.0 } else { 0.0 }),
            18 => Some(self.spectral_sub.spectral_sub_alpha as f64),
            19 => Some(self.spectral_sub.spectral_sub_beta as f64),
            20 => Some(if self.noise_profile.is_learning { 1.0 } else { 0.0 }),
            21 => Some(if self.noise_profile.use_captured_profile { 1.0 } else { 0.0 }),
            22 => Some(0.0), // clear_profile: trigger-only, always reads as false
            23 => Some(if self.auxiliary.formant_preserver.enabled {
                1.0
            } else {
                0.0
            }),
            24 => Some(self.auxiliary.formant_preserver.strength as f64),
            25 => Some(if self.multi_res.multi_resolution { 1.0 } else { 0.0 }),
            26 => Some(if self.params.harmonic_percussive { 1.0 } else { 0.0 }),
            27 => Some(if self.params.spatial_denoise { 1.0 } else { 0.0 }),
            28 => Some(self.params.spatial_strength as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    /// Side effects are dispatched separately after param_bridge::set_parameter.
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.params.reduction_db = value as f32,
            1 => self.params.floor_db = value as f32,
            2 => self.params.smoothing = value as f32,
            3 => self.params.attack_ms = value as f32,
            4 => self.params.release_ms = value as f32,
            5 => self.params.low_latency = value > 0.5,
            6 => self.params.polyphonic_detection = value > 0.5,
            7 => self.mcra.mcra_alpha_s = value as f32,
            8 => self.mcra.mcra_alpha_p = value as f32,
            9 => self.mcra.mcra_l = value as usize,
            10 => self.mcra.mcra_delta = value as f32,
            11 => self.params.transparency = value as f32,
            12 => self.decision_directed.dd_enabled = value > 0.5,
            13 => self.decision_directed.dd_alpha = value as f32,
            14 => self.params.psychoacoustic_masking = value > 0.5,
            15 => self.params.spectral_smoothing_enabled = value > 0.5,
            16 => self.params.temporal_smoothing_enabled = value > 0.5,
            17 => self.spectral_sub.spectral_sub_enabled = value > 0.5,
            18 => self.spectral_sub.spectral_sub_alpha = value as f32,
            19 => self.spectral_sub.spectral_sub_beta = value as f32,
            20 => {} // learn_noise: side effect handled in set_parameter
            21 => self.noise_profile.use_captured_profile = value > 0.5,
            22 => {} // clear_profile: side effect handled in set_parameter
            23 => self.auxiliary.formant_preserver.enabled = value > 0.5,
            24 => self.auxiliary.formant_preserver.strength = value as f32,
            25 => self.multi_res.multi_resolution = value > 0.5,
            26 => self.params.harmonic_percussive = value > 0.5,
            27 => self.params.spatial_denoise = value > 0.5,
            28 => self.params.spatial_strength = value as f32,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.ui.cached_parameters = param_bridge::build_parameters(DN, |i| self.param_value(i));
    }

    /// Create a new denoiser plugin from configuration parameters
    pub fn from_params(channels: usize, params: DenoiserPluginParams) -> Self {
        let mut plugin = Self::new(channels, params.low_latency);

        plugin.params.reduction_db = params.reduction_db.clamp(
            pk(DN, "reduction_db").min_f64() as f32,
            pk(DN, "reduction_db").max_f64() as f32,
        );
        plugin.params.floor_db = params.floor_db.clamp(
            pk(DN, "floor_db").min_f64() as f32,
            pk(DN, "floor_db").max_f64() as f32,
        );
        plugin.params.smoothing = params.smoothing.clamp(
            pk(DN, "smoothing").min_f64() as f32,
            pk(DN, "smoothing").max_f64() as f32,
        );
        plugin.params.attack_ms = params.attack_ms.clamp(
            pk(DN, "attack_ms").min_f64() as f32,
            pk(DN, "attack_ms").max_f64() as f32,
        );
        plugin.params.release_ms = params.release_ms.clamp(
            pk(DN, "release_ms").min_f64() as f32,
            pk(DN, "release_ms").max_f64() as f32,
        );
        plugin.params.polyphonic_detection = params.polyphonic_detection;

        plugin.mcra.mcra_alpha_s = params.mcra_alpha_s;
        plugin.mcra.mcra_alpha_p = params.mcra_alpha_p;
        plugin.mcra.mcra_l = params.mcra_l.max(1);
        plugin.mcra.mcra_delta = params.mcra_delta;

        plugin.params.transparency = params.transparency.clamp(
            pk(DN, "transparency").min_f64() as f32,
            pk(DN, "transparency").max_f64() as f32,
        );
        plugin.decision_directed.dd_enabled = params.dd_enabled;
        plugin.decision_directed.dd_alpha = params.dd_alpha.clamp(
            pk(DN, "dd_alpha").min_f64() as f32,
            pk(DN, "dd_alpha").max_f64() as f32,
        );
        plugin.params.psychoacoustic_masking = params.psychoacoustic_masking;
        plugin.noise_profile.use_captured_profile = params.use_captured_profile;
        plugin.params.spectral_smoothing_enabled = params.spectral_smoothing_enabled;
        plugin.params.temporal_smoothing_enabled = params.temporal_smoothing_enabled;

        plugin.spectral_sub.spectral_sub_enabled = params.spectral_sub_enabled;
        plugin.spectral_sub.spectral_sub_alpha = params.spectral_sub_alpha.clamp(
            pk(DN, "spectral_sub_alpha").min_f64() as f32,
            pk(DN, "spectral_sub_alpha").max_f64() as f32,
        );
        plugin.spectral_sub.spectral_sub_beta = params.spectral_sub_beta.clamp(
            pk(DN, "spectral_sub_beta").min_f64() as f32,
            pk(DN, "spectral_sub_beta").max_f64() as f32,
        );

        plugin.coeffs.reduction_linear = 10.0_f32.powf(plugin.params.reduction_db / 10.0);
        plugin.coeffs.floor_linear = 10.0_f32.powf(plugin.params.floor_db / 20.0);
        plugin.gains.freq_smooth_kernel = Self::compute_smoothing_kernel(plugin.params.smoothing);

        plugin.auxiliary.formant_preserver.enabled = params.formant_preservation;
        plugin.auxiliary.formant_preserver.strength = params.formant_strength.clamp(
            pk(DN, "formant_strength").min_f64() as f32,
            pk(DN, "formant_strength").max_f64() as f32,
        );

        plugin.multi_res.multi_resolution = params.multi_resolution;
        if plugin.multi_res.multi_resolution {
            plugin.multi_res.multi_res_state = Some(super::multi_resolution::MultiResState::new(
                channels,
                plugin.mcra.mcra_alpha_s,
                plugin.mcra.mcra_alpha_p,
                plugin.mcra.mcra_l,
                plugin.mcra.mcra_delta,
            ));
        }

        plugin.rebuild_cached_parameters();
        plugin
    }

    pub(super) fn prepared_in_place_frames_for_fft(fft_size: usize) -> usize {
        (fft_size * 4).max(MIN_IN_PLACE_BLOCK_FRAMES)
    }

    pub(super) fn output_ring_capacity_for_fft(fft_size: usize) -> usize {
        (Self::prepared_in_place_frames_for_fft(fft_size) + fft_size).next_power_of_two()
    }

    pub(super) fn max_in_place_frames(&self) -> usize {
        Self::prepared_in_place_frames_for_fft(self.config.fft_size)
    }

    /// Process one FFT block
    pub(super) fn process_fft_block(&mut self) -> Result<(), String> {
        // Extract block from input buffer (fft_size * channels samples)
        let block_samples = self.config.fft_size * self.config.channels;

        // Phase 1: Apply window and forward FFT (must happen before shifting)
        // Copy into pre-allocated scratch before shifting.
        self.io.temp_input_block
            .iter_mut()
            .take(block_samples)
            .zip(self.io.input_buffer.iter().take(block_samples))
            .for_each(|(dst, src)| *dst = *src);
        // Safe because the pointer remains valid for `block_samples` and is only
        // read while `self` is borrowed mutably for this FFT call.
        let input_ptr = self.io.temp_input_block.as_ptr();
        let input_block = unsafe { std::slice::from_raw_parts(input_ptr, block_samples) };
        self.apply_window_and_forward_fft(input_block)?;

        // Shift input buffer (remove processed samples, keeping hop_size overlap)
        let shift_samples = self.config.hop_size * self.config.channels;
        self.io.input_buffer.copy_within(shift_samples.., 0);
        self.io.input_buffer_fill -= shift_samples;

        // Phase 2: Noise estimation (multi-frame bootstrap then IMCRA)
        let bootstrapping = self.update_noise_estimation();

        // Phase 2b: Noise profile learning (if active)
        if self.noise_profile.is_learning {
            self.accumulate_noise_frame();
        }

        // Phase 3: Calculate Gains (skip during bootstrap — gains stay at 1.0)
        if !bootstrapping {
            if self.params.polyphonic_detection {
                self.calculate_polyphonic_gains();
            } else {
                self.calculate_wiener_gains();
            }
        }

        // Phase 3b: Multi-resolution gain combination.
        // The small-FFT path has already been fed samples and computed its own
        // gains.  Blend them into `self.gains.smoothed_gain` based on spectral flux.
        if let Some(ref mrs) = self.multi_res.multi_res_state {
            mrs.combine_gains(&mut self.gains.smoothed_gain, self.config.channels, self.config.spectrum_size);
        }

        // Phase 4: Apply gains and inverse FFT
        self.apply_gains_and_inverse_fft()?;

        // Phase 5: Overlap-add to output accumulator
        self.overlap_add_to_accumulator();

        // Phase 6: Update cached monitoring data (every 8th block to reduce overhead)
        self.ui.data_update_counter += 1;
        if self.ui.data_update_counter >= 8 {
            self.ui.data_update_counter = 0;
            self.update_cached_data();
        }
        Ok(())
    }

    /// Update the cached DenoiserData for UI polling.
    /// Writes into pre-allocated buffers to avoid allocations on the audio thread.
    pub(super) fn update_cached_data(&mut self) {
        self.compute_noise_floor_db();
        self.compute_snr_db();

        let avg_red = self.ui.avg_reduction_db;
        let learn_act = self.ui.learning_active;
        let is_learn = self.noise_profile.is_learning;
        let has_prof = self.noise_profile.has_noise_profile;
        let progress = self.learning_progress();
        let using_prof = self.noise_profile.use_captured_profile;

        let nf_buf = &self.ui.cached_noise_floor_buf;
        let snr_buf = &self.ui.cached_snr_buf;

        self.ui.cache.update(|d| {
            if let Some(mut_nf) = Arc::get_mut(&mut d.noise_floor_db) {
                mut_nf.copy_from_slice(nf_buf);
            }
            if let Some(mut_snr) = Arc::get_mut(&mut d.snr_db) {
                mut_snr.copy_from_slice(snr_buf);
            }
            d.avg_reduction_db = avg_red;
            d.learning_active = learn_act;
            d.is_learning_noise = is_learn;
            d.has_captured_profile = has_prof;
            d.learning_progress = progress;
            d.using_captured_profile = using_prof;
        });
    }

    /// Add processed block to output ring-buffer accumulator using overlap-add.
    /// Uses modular indexing (& mask) — no bulk shifts.
    pub(super) fn overlap_add_to_accumulator(&mut self) {
        // WOLA: apply synthesis window (sqrt(Hann) / fft_size) before overlap-add.
        // Analysis sqrt(Hann) * synthesis sqrt(Hann) = Hann → perfect COLA at 50% overlap.
        let mask = self.io.output_ring_mask;

        for ch in 0..self.config.channels {
            let accum = &mut self.io.output_accumulator[ch];
            let time_out = &self.io.time_out_channels[ch];

            for (i, (t, w)) in time_out
                .iter()
                .zip(self.fft.synthesis_window.iter())
                .enumerate()
                .take(self.config.fft_size)
            {
                let idx = (self.io.output_write_pos + i) & mask;
                accum[idx] += t * w;
            }
        }

        // Advance write position by hop_size for next block
        self.io.output_write_pos = (self.io.output_write_pos + self.config.hop_size) & mask;
        self.io.output_accumulator_fill += self.config.hop_size;
    }

    /// Drain available frames from ring-buffer accumulator to output buffer.
    /// Returns the number of frames actually drained.
    pub(super) fn drain_output(
        &mut self,
        output: &mut [f32],
        output_pos: usize,
        frames_wanted: usize,
    ) -> usize {
        let frames_to_drain = self.io.output_accumulator_fill.min(frames_wanted);
        let mask = self.io.output_ring_mask;

        for frame in 0..frames_to_drain {
            let ring_idx = (self.io.output_read_pos + frame) & mask;
            let out_base = (output_pos + frame) * self.config.channels;
            for ch in 0..self.config.channels {
                output[out_base + ch] = self.io.output_accumulator[ch][ring_idx];
                // Clear after reading for next overlap-add cycle
                self.io.output_accumulator[ch][ring_idx] = 0.0;
            }
        }

        self.io.output_read_pos = (self.io.output_read_pos + frames_to_drain) & mask;
        self.io.output_accumulator_fill -= frames_to_drain;

        frames_to_drain
    }
}

impl ParametricInPlacePlugin for DenoiserPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Denoiser", "1.0.0", "SotF")
            .with_description("Wiener filter denoiser with MCRA noise estimation")
    }

    fn channels(&self) -> usize {
        self.config.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.ui.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        self.ui.cached_parameters
            .iter()
            .map(|p| (p.id.clone(), p.default_value.clone()))
            .collect()
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            let idx = param_bridge::set_parameter(DN, &id, &value, |i, v| {
                self.set_param_value(i, v);
            })?;
            // Side effects based on which parameter changed
            match idx {
                0 => {
                    // reduction_db -> recompute reduction_linear
                    self.coeffs.reduction_linear = 10.0_f32.powf(self.params.reduction_db / 10.0);
                }
                1 => {
                    // floor_db -> recompute floor_linear
                    self.coeffs.floor_linear = 10.0_f32.powf(self.params.floor_db / 20.0);
                }
                2 => {
                    // smoothing -> recompute frequency smoothing kernel
                    self.gains.freq_smooth_kernel =
                        Self::compute_smoothing_kernel(self.params.smoothing);
                }
                3 | 4 => {
                    // attack_ms or release_ms -> recompute envelope coefficients
                    self.update_envelope_coefficients();
                }
                20
                    // learn_noise (trigger param)
                    if value.as_bool().unwrap_or(false) => {
                        self.start_learning();
                    }
                22
                    // clear_profile (trigger param)
                    if value.as_bool().unwrap_or(false) => {
                        self.clear_noise_profile();
                    }
                25 => {
                    // multi_resolution -> allocate/deallocate state
                    if self.multi_res.multi_resolution && self.multi_res.multi_res_state.is_none() {
                        self.multi_res.multi_res_state = Some(super::multi_resolution::MultiResState::new(
                            self.config.channels,
                            self.mcra.mcra_alpha_s,
                            self.mcra.mcra_alpha_p,
                            self.mcra.mcra_l,
                            self.mcra.mcra_delta,
                        ));
                    } else if !self.multi_res.multi_resolution {
                        self.multi_res.multi_res_state = None;
                    }
                }
                _ => {}
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        let mut values = ParameterSet::new();
        values.insert(id, value);
        self.apply_values(values)
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.config.sample_rate = sample_rate;
        self.update_envelope_coefficients();
        self.precompute_bark_mapping();

        // Update PND analyzers with correct sample rate
        for analyzer in &mut self.auxiliary.pnd_analyzers {
            *analyzer = PndAnalyzer::new(2048, sample_rate, 50.0);
        }

        Ok(())
    }

    fn reset(&mut self) {
        // Reset MCRA state
        for ch in 0..self.config.channels {
            self.reset_mcra(ch);
            self.gains.gain[ch].fill(1.0);
            self.gains.smoothed_gain[ch].fill(1.0);
            self.decision_directed.prev_power[ch].fill(0.0);
            self.noise_profile.learning_accumulator[ch].fill(0.0);
            self.io.output_accumulator[ch].fill(0.0);
            self.io.time_out_channels[ch].fill(0.0);
        }
        self.noise_profile.is_learning = false;
        self.noise_profile.learning_frames_count = 0;

        // Reset PND analyzers
        for analyzer in &mut self.auxiliary.pnd_analyzers {
            analyzer.reset();
        }

        // Reset buffers
        self.io.input_buffer.fill(0.0);
        self.io.input_buffer_fill = 0;
        self.io.output_read_pos = 0;
        self.io.output_write_pos = 0;
        self.io.output_accumulator_fill = 0;

        // Reset formant preserver working buffers
        self.auxiliary.formant_preserver.log_mag_scratch.fill(0.0);
        self.auxiliary.formant_preserver.envelope.fill(0.0);
        self.spatial.spatial_coherence.fill(1.0);
        self.spatial.spatial_cross.fill(Complex::new(1.0_f32, 0.0_f32));

        // Reset multi-resolution state
        if let Some(ref mut mrs) = self.multi_res.multi_res_state {
            mrs.reset();
        }
        self.ui.avg_reduction_db = 0.0;
        self.ui.learning_active = true;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let _ftz_guard = ScopedFtz::new();

        let num_frames = context.num_frames;
        let total_samples = match num_frames.checked_mul(self.config.channels) {
            Some(total_samples) => total_samples,
            None => return Err("Frame/channel count overflow".to_string()),
        };
        if buffer.len() != total_samples {
            return Err(format!(
                "Buffer size mismatch: expected {}, got {}",
                total_samples,
                buffer.len()
            ));
        }

        let max_in_place_frames = self.max_in_place_frames();
        if num_frames > max_in_place_frames {
            return Err(format!(
                "Block too large for in-place denoiser: {} frames exceeds prepared safe maximum {}",
                num_frames, max_in_place_frames
            ));
        }

        let block_samples = self.config.fft_size * self.config.channels;

        // Phase 0: Feed samples to PND analyzers.
        // De-interleave each channel into temp_input_block (first num_frames elements)
        // and pass the whole block at once — one analyze() call per channel instead of
        // one per sample, reducing function-call overhead by num_frames×.
        if self.params.polyphonic_detection {
            let channels = self.config.channels;
            for ch in 0..channels {
                // Reuse temp_input_block[0..num_frames] as a de-interleave scratch.
                for i in 0..num_frames {
                    self.io.temp_input_block[i] = buffer[i * channels + ch];
                }
                self.auxiliary.pnd_analyzers[ch].analyze(&self.io.temp_input_block[..num_frames]);
            }
        }

        // Phase 1: Accumulate ALL input into input_buffer.
        // This is an in-place plugin (same buffer for input/output), so we must
        // consume all input before writing any output to avoid data corruption.
        // Loop to handle cases where input exceeds remaining buffer space:
        // process FFT blocks to free space, then continue accumulating.
        //
        // When multi-resolution is enabled we simultaneously feed the same raw
        // samples into the small-FFT accumulator.  We do this first so that the
        // small-FFT gains are ready when process_fft_block() is called below.
        if self.multi_res.multi_res_state.is_some() {
            // Feed the whole input block into the small-FFT path before
            // the main loop starts.  `feed_and_process` is self-contained
            // and does not touch `buffer` after reading — safe to call here.
            let reduction = self.coeffs.reduction_linear;
            let floor = self.coeffs.floor_linear;
            let channels = self.config.channels;
            if let Some(ref mut mrs) = self.multi_res.multi_res_state {
                mrs.feed_and_process(&buffer[..total_samples], channels, reduction, floor);
            }
        }

        let mut input_pos: usize = 0;
        while input_pos < total_samples {
            let space_available = self.io.input_buffer.len() - self.io.input_buffer_fill;
            let remaining_input = total_samples - input_pos;
            let samples_to_copy = remaining_input.min(space_available);

            self.io.input_buffer[self.io.input_buffer_fill..self.io.input_buffer_fill + samples_to_copy]
                .copy_from_slice(&buffer[input_pos..input_pos + samples_to_copy]);
            self.io.input_buffer_fill += samples_to_copy;
            input_pos += samples_to_copy;

            // Process FFT blocks to free input buffer space
            while self.io.input_buffer_fill >= block_samples {
                self.process_fft_block()?;
            }
        }

        // Phase 2: Process any remaining complete FFT blocks
        while self.io.input_buffer_fill >= block_samples {
            self.process_fft_block()?;
        }

        // Phase 3: Drain output to buffer
        let mut output_pos: usize = 0;
        if self.io.output_accumulator_fill > 0 {
            output_pos = self.drain_output(buffer, 0, num_frames);
        }

        // Zero-fill any remaining output (initial latency period)
        if output_pos < num_frames {
            let zero_start = output_pos * self.config.channels;
            buffer[zero_start..total_samples].fill(0.0);
        }

        // STFT convention: always return num_frames. Buffer is zero-padded for
        // unfilled portions, so downstream plugins see valid (silent) data.
        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.config.fft_size
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.ui.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}
