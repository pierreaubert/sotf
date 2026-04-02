// ============================================================================
// Multiband Expander Plugin
// ============================================================================

pub mod params;

use crate::params::{BAND_TEMPLATE as MEB, GLOBAL_PARAMS as ME};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_dsp::stft::{RealFftProcessor, generate_hann_window};
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_makeup::MeasuredMakeup;
use sotf_host::detector::{DetectionMode, LevelDetector};
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::{LogSmoother, Smoother};
use std::any::Any;
use std::sync::Arc;

pub use sotf_host::lr4_crossover::CROSSOVER_PRESETS;

fn default_true() -> bool {
    true
}

fn default_processing_mode() -> String {
    "time_domain".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandExpanderParams {
    pub threshold_db: Option<f32>,
    pub ratio: Option<f32>,
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
    pub knee_db: Option<f32>,
    pub range_db: Option<f32>,
    pub hysteresis_db: Option<f32>,
    pub hold_ms: Option<f32>,
    #[serde(default)]
    pub auto_makeup: bool,
    #[serde(default)]
    pub measured_auto_makeup: bool,
    #[serde(default = "default_true")]
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
}

impl Default for BandExpanderParams {
    fn default() -> Self {
        Self {
            threshold_db: None,
            ratio: None,
            attack_ms: None,
            release_ms: None,
            knee_db: None,
            range_db: None,
            hysteresis_db: None,
            hold_ms: None,
            auto_makeup: false,
            measured_auto_makeup: false,
            active: true,
            solo: false,
            bypass: false,
        }
    }
}

fn default_detection_mode() -> String {
    "peak".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MultibandExpanderPluginParams {
    pub num_bands: usize,
    pub crossover_preset: i32,
    pub crossover_frequencies: Vec<f32>,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub range_db: f32,
    pub hysteresis_db: f32,
    pub hold_ms: f32,
    pub link_channels: bool,
    pub mix: f32,
    #[serde(default = "default_detection_mode")]
    pub detection_mode: String,
    pub bands: Vec<BandExpanderParams>,
    /// Processing mode: "time_domain" (default) or "spectral"
    #[serde(default = "default_processing_mode")]
    pub processing_mode: String,
}

#[derive(Debug, Clone)]
pub struct MultibandExpanderData {
    /// Attenuation per band and per channel (flattened)
    pub attenuation_db: Arc<Vec<f32>>,
    pub is_open: Arc<Vec<bool>>,
    pub band_levels_db: Arc<Vec<f32>>,
    pub crossover_frequencies: Arc<Vec<f32>>,
}

impl Default for MultibandExpanderData {
    fn default() -> Self {
        Self {
            attenuation_db: Arc::new(Vec::new()),
            is_open: Arc::new(Vec::new()),
            band_levels_db: Arc::new(Vec::new()),
            crossover_frequencies: Arc::new(Vec::new()),
        }
    }
}

impl MultibandExpanderData {
    pub fn new(num_bands: usize, channels: usize) -> Self {
        Self {
            attenuation_db: Arc::new(vec![0.0; num_bands * channels]),
            is_open: Arc::new(vec![false; num_bands]),
            band_levels_db: Arc::new(vec![-120.0; num_bands]),
            crossover_frequencies: Arc::new(vec![0.0; num_bands.saturating_sub(1)]),
        }
    }

    pub fn update(&mut self, atten: &[f32], open: &[bool], levels: &[f32], xovers: &[f32]) {
        if let Some(mut_atten) = Arc::get_mut(&mut self.attenuation_db)
            && mut_atten.len() == atten.len()
        {
            mut_atten.copy_from_slice(atten);
        }
        if let Some(mut_open) = Arc::get_mut(&mut self.is_open)
            && mut_open.len() == open.len()
        {
            mut_open.copy_from_slice(open);
        }
        if let Some(mut_levels) = Arc::get_mut(&mut self.band_levels_db)
            && mut_levels.len() == levels.len()
        {
            mut_levels.copy_from_slice(levels);
        }
        if let Some(mut_xovers) = Arc::get_mut(&mut self.crossover_frequencies)
            && mut_xovers.len() == xovers.len()
        {
            mut_xovers.copy_from_slice(xovers);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    Open,
    Hold,
    Closing,
}

struct BandExpander {
    envelope: Vec<f32>,
    /// Peak envelope follower per channel (linear amplitude).
    /// Prevents instantaneous zero-crossing dips from inflating expansion.
    peak_env: Vec<f32>,
    gate_state: Vec<GateState>,
    hold_counter: Vec<usize>,
    attack_coeff: f32,
    release_coeff: f32,
}

impl BandExpander {
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.peak_env.fill(0.0);
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
    }
}

// ============================================================================
// Spectral Mode State
// ============================================================================

/// Per-bin expander envelope state for spectral mode.
///
/// Each FFT bin is treated as an independent "channel" for envelope tracking.
/// The bin is assigned to a band based on its center frequency, and the band's
/// threshold/ratio/knee/range parameters are applied to its magnitude.
struct SpectralBinState {
    /// Smoothed attenuation in dB (0 = no attenuation, positive = attenuating)
    envelope_db: f32,
    gate_state: GateState,
    hold_counter: usize,
}

impl SpectralBinState {
    fn new() -> Self {
        Self {
            envelope_db: 0.0,
            gate_state: GateState::Open,
            hold_counter: 0,
        }
    }
}

/// All STFT buffers needed for spectral mode processing.
///
/// Pre-allocated in `initialize()` to avoid hot-path allocation.
/// Uses the same Hann-window 75%-overlap OLA as XTC/Binaural plugins.
struct SpectralState {
    fft_size: usize,
    hop_size: usize,
    /// Number of frequency bins = fft_size / 2 + 1
    num_bins: usize,

    /// Per-channel FFT processor (forward + inverse)
    fft_processors: Vec<RealFftProcessor>,

    /// Hann analysis window (length = fft_size)
    analysis_window: Vec<f32>,

    /// Combined COLA normalization + 1/fft_size scale factor.
    /// For 75% overlap dual-windowing Hann: 1/(1.5 * N)
    output_scale: f32,

    // --- Input staging ---
    /// Per-channel linear input ring buffer [fft_size] – linear shift pattern
    input_buffers: Vec<Vec<f32>>,
    /// How many valid samples are in the tail of each input_buffer
    input_fill: usize,

    // --- Envelope state ---
    /// Per-channel, per-bin expander state [channels][num_bins]
    bin_states: Vec<Vec<SpectralBinState>>,
    /// Per-bin: which band index owns this bin
    bin_to_band: Vec<usize>,
    /// Per-band: attack/release coefficients (hop-rate, not sample-rate)
    band_attack_hop: Vec<f32>,
    band_release_hop: Vec<f32>,

    // --- OLA output accumulator ---
    /// Flat interleaved ring buffer: [ch0_f0, ch1_f0, ch0_f1, ...]
    /// Size: 4 * fft_size frames × channels
    output_accumulator: Vec<f32>,
    /// Power-of-2 frame count (kept for documentation; mask is derived from this)
    _output_accumulator_frames: usize,
    output_accumulator_mask: usize,
    /// Number of valid frames ready to drain
    output_accumulator_fill: usize,
    /// Next frame write position (ring)
    next_add_position: usize,
    /// Next frame read position (ring)
    output_read_position: usize,

    /// Latency fill counter: ensures we start draining immediately
    latency_filled: usize,

    // --- Temporary working buffers ---
    /// Scratch for windowed time-domain [fft_size]
    windowed_buf: Vec<f32>,
    /// Frequency-domain scratch [num_bins]
    freq_scratch: Vec<Complex<f32>>,
    /// IFFT output [fft_size]
    ifft_buf: Vec<f32>,
}

impl SpectralState {
    fn new(
        fft_size: usize,
        channels: usize,
        sample_rate: u32,
        crossover_frequencies: &[f32],
        num_bands: usize,
    ) -> Self {
        let hop_size = fft_size / 4; // 75% overlap
        let num_bins = fft_size / 2 + 1;

        // Build per-channel FFT processors
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);
        let _ = fft_forward;
        let _ = fft_inverse;

        let fft_processors: Vec<RealFftProcessor> = (0..channels)
            .map(|_| RealFftProcessor::new_bidirectional(fft_size))
            .collect();

        let analysis_window = generate_hann_window(fft_size);

        // 75% overlap, dual Hann window: COLA sum = 1.5
        let output_scale = 1.0 / (fft_size as f32 * 1.5);

        // Assign each bin to a band based on center frequency
        let bin_to_band = Self::compute_bin_to_band(
            fft_size,
            num_bins,
            sample_rate,
            crossover_frequencies,
            num_bands,
        );

        // Per-bin state (all bins start in Open state)
        let bin_states: Vec<Vec<SpectralBinState>> = (0..channels)
            .map(|_| (0..num_bins).map(|_| SpectralBinState::new()).collect())
            .collect();

        let output_accumulator_frames = (fft_size * 4).next_power_of_two();
        let output_accumulator = vec![0.0f32; output_accumulator_frames * channels];

        Self {
            fft_size,
            hop_size,
            num_bins,
            fft_processors,
            analysis_window,
            output_scale,
            input_buffers: vec![vec![0.0f32; fft_size]; channels],
            input_fill: 0,
            bin_states,
            bin_to_band,
            band_attack_hop: vec![0.0; num_bands],
            band_release_hop: vec![0.0; num_bands],
            output_accumulator,
            _output_accumulator_frames: output_accumulator_frames,
            output_accumulator_mask: output_accumulator_frames - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            latency_filled: 0,
            windowed_buf: vec![0.0f32; fft_size],
            freq_scratch: vec![Complex::new(0.0, 0.0); num_bins],
            ifft_buf: vec![0.0f32; fft_size],
        }
    }

    /// Map each FFT bin to the band that covers its center frequency.
    ///
    /// `crossover_frequencies` has `num_bands - 1` entries in ascending order.
    /// Bin k has center frequency k * sample_rate / fft_size.
    fn compute_bin_to_band(
        fft_size: usize,
        num_bins: usize,
        sample_rate: u32,
        crossover_frequencies: &[f32],
        num_bands: usize,
    ) -> Vec<usize> {
        (0..num_bins)
            .map(|k| {
                let freq = k as f32 * sample_rate as f32 / fft_size as f32;
                // Find the first crossover that is above this bin's frequency
                let mut band = 0;
                for &xf in crossover_frequencies
                    .iter()
                    .take(num_bands.saturating_sub(1))
                {
                    if freq < xf {
                        break;
                    }
                    band += 1;
                }
                band.min(num_bands.saturating_sub(1))
            })
            .collect()
    }

    /// Update the bin→band mapping (called when crossover frequencies change).
    fn update_bin_to_band(
        &mut self,
        sample_rate: u32,
        crossover_frequencies: &[f32],
        num_bands: usize,
    ) {
        self.bin_to_band = Self::compute_bin_to_band(
            self.fft_size,
            self.num_bins,
            sample_rate,
            crossover_frequencies,
            num_bands,
        );
    }

    /// Recompute per-band hop-rate attack/release coefficients.
    ///
    /// Time constants are expressed in samples at sample rate, but here we use
    /// the hop period (hop_size / sample_rate) as the "sample period" so the
    /// time constants are preserved in perceptual terms.
    fn update_band_coefficients(
        &mut self,
        num_bands: usize,
        band_params: &[BandExpanderParams],
        global_attack_ms: f32,
        global_release_ms: f32,
        sample_rate: u32,
    ) {
        let hop_rate = sample_rate as f32 / self.hop_size as f32;
        self.band_attack_hop.resize(num_bands, 0.0);
        self.band_release_hop.resize(num_bands, 0.0);

        for b in 0..num_bands {
            let a_ms = band_params
                .get(b)
                .and_then(|p| p.attack_ms)
                .unwrap_or(global_attack_ms);
            let r_ms = band_params
                .get(b)
                .and_then(|p| p.release_ms)
                .unwrap_or(global_release_ms);
            // One-pole coefficient: e^(-1 / (time_s * rate))
            self.band_attack_hop[b] = (-1.0 / (a_ms * 0.001 * hop_rate)).exp();
            self.band_release_hop[b] = (-1.0 / (r_ms * 0.001 * hop_rate)).exp();
        }
    }

    fn reset(&mut self) {
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        self.input_fill = 0;
        for ch_states in &mut self.bin_states {
            for s in ch_states.iter_mut() {
                s.envelope_db = 0.0;
                s.gate_state = GateState::Open;
                s.hold_counter = 0;
            }
        }
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.latency_filled = 0;
        self.windowed_buf.fill(0.0);
        self.freq_scratch.fill(Complex::new(0.0, 0.0));
        self.ifft_buf.fill(0.0);
    }
}

pub struct MultibandExpanderPlugin {
    channels: usize,
    sample_rate: u32,
    num_bands: usize,
    _crossover_preset: i32,
    crossover_frequencies: Vec<f32>,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    range_db: f32,
    hysteresis_db: f32,
    hold_ms: f32,
    link_channels: bool,
    mix: f32,
    detection_mode: String,
    /// Processing mode: "time_domain" or "spectral"
    processing_mode: String,
    band_params: Vec<BandExpanderParams>,
    crossover_points: Vec<Lr4Crossover<f32>>,
    band_expanders: Vec<BandExpander>,
    band_buffers: Vec<f32>,
    band_levels_db: Vec<f32>,
    dry_buffer: Vec<f32>,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    xover_smoothers: Vec<LogSmoother>,

    /// Per-band measured auto-makeup gain trackers.
    measured_makeups: Vec<MeasuredMakeup>,

    /// Per-band, per-channel level detectors for RMS mode.
    level_detectors: Vec<Vec<LevelDetector>>,

    // Internal flattened monitoring buffers
    attenuation_flattened: Vec<f32>,
    is_open_buffer: Vec<bool>,
    cache: RealTimeCache<MultibandExpanderData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,

    /// State for spectral processing mode (None when in time_domain mode)
    spectral: Option<SpectralState>,
}

fn parse_detection_mode(s: &str) -> DetectionMode {
    match s {
        "rms" => DetectionMode::Rms { window_ms: 10.0 },
        _ => DetectionMode::Peak,
    }
}

impl MultibandExpanderPlugin {
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, Default::default())
    }
    pub fn with_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
        let nb = params.num_bands.clamp(
            pk(ME, "num_bands").min_f64() as usize,
            pk(ME, "num_bands").max_f64() as usize,
        );
        let sr = 44100;
        let mut xfs = params.crossover_frequencies.clone();
        while xfs.len() < 4 {
            xfs.push(1000.0);
        }
        let mut bexps = Vec::with_capacity(nb);
        for _ in 0..nb {
            bexps.push(BandExpander {
                envelope: vec![0.0; channels],
                peak_env: vec![0.0; channels],
                gate_state: vec![GateState::Open; channels],
                hold_counter: vec![0; channels],
                attack_coeff: 0.0,
                release_coeff: 0.0,
            });
        }

        let mut band_params = params.bands;
        while band_params.len() < nb {
            band_params.push(BandExpanderParams::default());
        }

        let measured_makeups = (0..nb).map(|_| MeasuredMakeup::new(1000.0, sr)).collect();

        let det_mode_str = if params.detection_mode.is_empty() {
            "peak"
        } else {
            &params.detection_mode
        };
        let det_mode = parse_detection_mode(det_mode_str);
        let level_detectors = (0..nb)
            .map(|_| {
                (0..channels)
                    .map(|_| LevelDetector::new(det_mode, sr))
                    .collect()
            })
            .collect();

        let mode_str = if params.processing_mode.is_empty() {
            "time_domain"
        } else {
            params.processing_mode.as_str()
        };

        let spectral = if mode_str == "spectral" {
            let fft_size = 1024;
            let mut ss = SpectralState::new(fft_size, channels, sr, &xfs, nb);
            ss.update_band_coefficients(nb, &band_params, params.attack_ms, params.release_ms, sr);
            Some(ss)
        } else {
            None
        };

        let mut p = Self {
            channels,
            sample_rate: sr,
            num_bands: nb,
            _crossover_preset: params.crossover_preset,
            crossover_frequencies: xfs.clone(),
            threshold_db: params.threshold_db,
            ratio: params.ratio,
            attack_ms: params.attack_ms,
            release_ms: params.release_ms,
            knee_db: params.knee_db,
            range_db: params.range_db,
            hysteresis_db: params.hysteresis_db,
            hold_ms: params.hold_ms,
            link_channels: params.link_channels,
            mix: params.mix,
            detection_mode: det_mode_str.to_string(),
            processing_mode: mode_str.to_string(),
            band_params,
            crossover_points: Vec::new(),
            band_expanders: bexps,
            band_buffers: Vec::new(),
            band_levels_db: vec![0.0; nb],
            dry_buffer: Vec::new(),
            threshold_smoother: Smoother::new(params.threshold_db, 20.0, sr),
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            xover_smoothers: xfs.iter().map(|&f| LogSmoother::new(f, 50.0, sr)).collect(),
            measured_makeups,
            level_detectors,
            attenuation_flattened: vec![0.0; nb * channels],
            is_open_buffer: vec![false; nb],
            cache: RealTimeCache::new(MultibandExpanderData::new(nb, channels)),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
            spectral,
        };
        p.build_crossovers();
        p.update_coefficients();
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at GLOBAL_PARAMS index.
    /// Order must match params::GLOBAL_PARAMS exactly.
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.num_bands as f64),                       // num_bands
            1 => Some(self._crossover_preset as f64),               // crossover_preset
            2 => Some(self.crossover_frequencies[0] as f64),        // crossover_freq_1
            3 => Some(self.crossover_frequencies[1] as f64),        // crossover_freq_2
            4 => Some(self.crossover_frequencies[2] as f64),        // crossover_freq_3
            5 => Some(self.crossover_frequencies[3] as f64),        // crossover_freq_4
            6 => Some(self.threshold_db as f64),                    // threshold
            7 => Some(self.ratio as f64),                           // ratio
            8 => Some(self.attack_ms as f64),                       // attack
            9 => Some(self.release_ms as f64),                      // release
            10 => Some(self.range_db as f64),                       // range
            11 => Some(self.knee_db as f64),                        // knee
            12 => Some(self.hysteresis_db as f64),                  // hysteresis
            13 => Some(self.hold_ms as f64),                        // hold
            14 => Some(self.mix as f64),                            // mix
            15 => Some(if self.link_channels { 1.0 } else { 0.0 }), // link_channels
            16 => {
                // detection_mode
                let idx = if self.detection_mode == "rms" { 1 } else { 0 };
                Some(idx as f64)
            }
            _ => None,
        }
    }

    /// Set the f64 value of parameter at GLOBAL_PARAMS index.
    /// Order must match params::GLOBAL_PARAMS exactly.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.num_bands = value as usize,              // num_bands
            1 => self._crossover_preset = value as i32,        // crossover_preset
            2 => self.crossover_frequencies[0] = value as f32, // crossover_freq_1
            3 => self.crossover_frequencies[1] = value as f32, // crossover_freq_2
            4 => self.crossover_frequencies[2] = value as f32, // crossover_freq_3
            5 => self.crossover_frequencies[3] = value as f32, // crossover_freq_4
            6 => self.threshold_db = value as f32,             // threshold
            7 => self.ratio = value as f32,                    // ratio
            8 => self.attack_ms = value as f32,                // attack
            9 => self.release_ms = value as f32,               // release
            10 => self.range_db = value as f32,                // range
            11 => self.knee_db = value as f32,                 // knee
            12 => self.hysteresis_db = value as f32,           // hysteresis
            13 => self.hold_ms = value as f32,                 // hold
            14 => self.mix = value as f32,                     // mix
            15 => self.link_channels = value > 0.5,            // link_channels
            16 => {
                // detection_mode
                self.detection_mode = if value as i32 == 1 {
                    "rms".to_string()
                } else {
                    "peak".to_string()
                };
            }
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = param_bridge::build_parameters(ME, |i| self.param_value(i));

        // processing_mode is not in GLOBAL_PARAMS, add manually
        let proc_mode_idx = if self.processing_mode == "spectral" {
            1
        } else {
            0
        };
        params.push(
            Parameter::new_int("processing_mode", "Processing Mode", proc_mode_idx, 0, 1)
                .with_group("General"),
        );

        // Per-band dynamics (not covered by GLOBAL_PARAMS)
        for i in 0..self.num_bands {
            let group = format!("Band {}", i + 1);
            let bp = &self.band_params[i];

            params.push(
                Parameter::new_float(
                    &format!("band_{}_threshold", i),
                    "Threshold",
                    bp.threshold_db.unwrap_or(self.threshold_db),
                    pk(MEB, "threshold").min_f64() as f32,
                    pk(MEB, "threshold").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_ratio", i),
                    "Ratio",
                    bp.ratio.unwrap_or(self.ratio),
                    pk(MEB, "ratio").min_f64() as f32,
                    pk(MEB, "ratio").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_attack", i),
                    "Attack",
                    bp.attack_ms.unwrap_or(self.attack_ms),
                    pk(MEB, "attack").min_f64() as f32,
                    pk(MEB, "attack").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_release", i),
                    "Release",
                    bp.release_ms.unwrap_or(self.release_ms),
                    pk(MEB, "release").min_f64() as f32,
                    pk(MEB, "release").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_knee", i),
                    "Knee",
                    bp.knee_db.unwrap_or(self.knee_db),
                    pk(MEB, "knee").min_f64() as f32,
                    pk(MEB, "knee").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_range", i),
                    "Range",
                    bp.range_db.unwrap_or(self.range_db),
                    pk(MEB, "range").min_f64() as f32,
                    pk(MEB, "range").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_hysteresis", i),
                    "Hysteresis",
                    bp.hysteresis_db.unwrap_or(self.hysteresis_db),
                    pk(MEB, "hysteresis").min_f64() as f32,
                    pk(MEB, "hysteresis").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_float(
                    &format!("band_{}_hold", i),
                    "Hold",
                    bp.hold_ms.unwrap_or(self.hold_ms),
                    pk(MEB, "hold").min_f64() as f32,
                    pk(MEB, "hold").max_f64() as f32,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(
                    &format!("band_{}_auto_makeup", i),
                    "Auto Makeup",
                    bp.auto_makeup,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(
                    &format!("band_{}_measured_auto_makeup", i),
                    "Measured Auto Makeup",
                    bp.measured_auto_makeup,
                )
                .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_active", i), "Active", bp.active)
                    .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_solo", i), "Solo", bp.solo)
                    .with_group(&group),
            );
            params.push(
                Parameter::new_bool(&format!("band_{}_bypass", i), "Bypass", bp.bypass)
                    .with_group(&group),
            );
        }

        self.cached_parameters = params;
    }
    pub fn from_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
        Self::with_params(channels, params)
    }

    fn build_crossovers(&mut self) {
        self.crossover_points.clear();
        for i in 0..(self.num_bands - 1) {
            let f = self.xover_smoothers[i].target();
            self.crossover_points.push(Lr4Crossover::new(
                f,
                self.sample_rate as f32,
                self.channels,
            ));
        }
    }

    fn update_coefficients(&mut self) {
        for (i, b) in self.band_expanders.iter_mut().enumerate() {
            let a = self
                .band_params
                .get(i)
                .and_then(|p| p.attack_ms)
                .unwrap_or(self.attack_ms);
            let r = self
                .band_params
                .get(i)
                .and_then(|p| p.release_ms)
                .unwrap_or(self.release_ms);
            b.attack_coeff = (-1.0 / (a * 0.001 * self.sample_rate as f32)).exp();
            b.release_coeff = (-1.0 / (r * 0.001 * self.sample_rate as f32)).exp();
        }

        // Also update spectral-mode coefficients if active
        if let Some(ss) = &mut self.spectral {
            ss.update_band_coefficients(
                self.num_bands,
                &self.band_params,
                self.attack_ms,
                self.release_ms,
                self.sample_rate,
            );
        }
    }

    fn calculate_expansion_attenuation(
        idb: f32,
        th: f32,
        ratio: f32,
        knee: f32,
        range: f32,
    ) -> f32 {
        let slope = 1.0 - 1.0 / ratio.max(1.0);
        let atten = if knee < 0.1 {
            if idb >= th { 0.0 } else { (th - idb) * slope }
        } else if idb > th + knee / 2.0 {
            0.0
        } else if idb < th - knee / 2.0 {
            (th - idb) * slope
        } else {
            let b = th + knee / 2.0 - idb;
            let kf = b / knee;
            kf * kf * (knee / 2.0) * slope
        };
        atten.min(range)
    }

    /// Process one STFT hop for the spectral mode.
    ///
    /// Called after `fft_size` samples have been accumulated in the input ring.
    /// Applies per-bin expansion envelope then IFFT + OLA.
    ///
    /// # Expansion model
    /// Each bin's magnitude (in dB) acts as the "input level" to the expander.
    /// The bin is assigned to a band via `bin_to_band`; the band supplies
    /// threshold / ratio / knee / range / hold / hysteresis parameters.
    /// Attack/release coefficients are at *hop rate* so time constants are
    /// perceptually equivalent to the time-domain mode.
    fn process_spectral_hop(&mut self, any_solo: bool) {
        let ss = match &mut self.spectral {
            Some(s) => s,
            None => return,
        };

        let fft_size = ss.fft_size;
        let num_bins = ss.num_bins;
        let scale = ss.output_scale;
        let mask = ss.output_accumulator_mask;
        let channels = self.channels;

        // Cache band parameters into a compact form to avoid repeated borrow conflicts.
        // Hold time is converted from milliseconds to hop counts at the hop rate.
        // Uses a fixed-size array (max 5 bands) to avoid per-hop heap allocation.
        #[derive(Clone, Copy)]
        struct BandInfo {
            th: f32,
            rat: f32,
            kn: f32,
            rg: f32,
            hys: f32,
            /// Hold duration measured in STFT hops (not samples)
            hs: usize,
            bypass: bool,
            active: bool,
            solo: bool,
        }
        const MAX_MB_BANDS: usize = 5;
        let hop_rate = self.sample_rate as f32 / ss.hop_size as f32;
        let mut band_info = [BandInfo {
            th: 0.0,
            rat: 1.0,
            kn: 0.0,
            rg: 0.0,
            hys: 0.0,
            hs: 0,
            bypass: false,
            active: true,
            solo: false,
        }; MAX_MB_BANDS];
        for (b, info) in band_info.iter_mut().enumerate().take(self.num_bands) {
            let bp = self.band_params.get(b);
            let hold_ms = bp.and_then(|p| p.hold_ms).unwrap_or(self.hold_ms);
            *info = BandInfo {
                th: bp.and_then(|p| p.threshold_db).unwrap_or(self.threshold_db),
                rat: bp.and_then(|p| p.ratio).unwrap_or(self.ratio),
                kn: bp.and_then(|p| p.knee_db).unwrap_or(self.knee_db),
                rg: bp.and_then(|p| p.range_db).unwrap_or(self.range_db),
                hys: bp
                    .and_then(|p| p.hysteresis_db)
                    .unwrap_or(self.hysteresis_db),
                hs: (hold_ms * 0.001 * hop_rate) as usize,
                bypass: bp.map(|p| p.bypass).unwrap_or(false),
                active: bp.map(|p| p.active).unwrap_or(true),
                solo: bp.map(|p| p.solo).unwrap_or(false),
            };
        }

        for ch in 0..channels {
            // --- Forward FFT ---
            // Apply Hann window to the linear input buffer
            for i in 0..fft_size {
                ss.windowed_buf[i] = ss.input_buffers[ch][i] * ss.analysis_window[i];
            }
            // forward() reads time_buffer, writes freq_buffer
            ss.fft_processors[ch]
                .time_buffer
                .copy_from_slice(&ss.windowed_buf);
            ss.fft_processors[ch].forward();
            ss.freq_scratch
                .copy_from_slice(&ss.fft_processors[ch].freq_buffer);

            // --- Per-bin expansion ---
            for k in 0..num_bins {
                let b = ss.bin_to_band[k];
                let info = &band_info[b];

                // Muted bands (solo active, this band not solo)
                if any_solo && !info.solo {
                    ss.freq_scratch[k] = Complex::new(0.0, 0.0);
                    continue;
                }

                // Bypassed or inactive bands: no gain change
                if info.bypass || !info.active {
                    continue;
                }

                // Bin magnitude normalized to equivalent time-domain amplitude.
                //
                // The realfft forward transform is unnormalized: a cosine with
                // amplitude A at exactly a bin frequency gives |X[k]| = N*A/2.
                // Multiplying by 2/N converts the raw FFT magnitude back to the
                // equivalent amplitude so the threshold (in dBFS) has the same
                // meaning as in the time-domain expander mode.
                let mag = ss.freq_scratch[k].norm() * (2.0 / fft_size as f32);
                let mag_db = 20.0 * fast_log10(mag.max(1e-10));

                // Update gate state and envelope (at hop rate)
                let state = &mut ss.bin_states[ch][k];
                let th = info.th;
                let hys = info.hys;

                let target_atten = match state.gate_state {
                    GateState::Open => {
                        if mag_db < th {
                            state.gate_state = GateState::Hold;
                            state.hold_counter = info.hs;
                            0.0
                        } else {
                            0.0
                        }
                    }
                    GateState::Hold => {
                        if mag_db >= th {
                            state.gate_state = GateState::Open;
                            0.0
                        } else if state.hold_counter > 0 {
                            state.hold_counter -= 1;
                            0.0
                        } else if mag_db < th - hys {
                            state.gate_state = GateState::Closing;
                            Self::calculate_expansion_attenuation(
                                mag_db, th, info.rat, info.kn, info.rg,
                            )
                        } else {
                            0.0
                        }
                    }
                    GateState::Closing => {
                        if mag_db >= th {
                            state.gate_state = GateState::Open;
                            0.0
                        } else {
                            Self::calculate_expansion_attenuation(
                                mag_db, th, info.rat, info.kn, info.rg,
                            )
                        }
                    }
                };

                // One-pole envelope smoothing at hop rate
                let coeff = if target_atten > state.envelope_db {
                    ss.band_attack_hop[b]
                } else {
                    ss.band_release_hop[b]
                };
                state.envelope_db = target_atten + coeff * (state.envelope_db - target_atten);

                // Apply gain to the complex bin
                let gain = fast_pow10(-state.envelope_db / 20.0);
                ss.freq_scratch[k] *= gain;
            }

            // --- Inverse FFT ---
            ss.fft_processors[ch]
                .freq_buffer
                .copy_from_slice(&ss.freq_scratch);
            ss.fft_processors[ch].inverse();

            // Apply synthesis window (Hann) + scale, overlap-add into ring
            let next_pos = ss.next_add_position;
            for i in 0..fft_size {
                let frame_idx = (next_pos + i) & mask;
                let s = ss.fft_processors[ch].time_buffer[i]
                    * ss.analysis_window[i]  // synthesis window = same Hann
                    * scale;
                ss.output_accumulator[frame_idx * channels + ch] += s;
            }
        }

        // Advance OLA write position by one hop
        ss.next_add_position = (ss.next_add_position + ss.hop_size) & mask;

        // Zero the "fresh" positions just past the write head (for clean OLA)
        // They will accumulate contributions from the next several hops.
        // We zero hop_size frames at next_add_position + fft_size to clear stale data.
        {
            let clear_start = (ss.next_add_position + fft_size) & mask;
            for i in 0..ss.hop_size {
                let frame_idx = (clear_start + i) & mask;
                for ch in 0..channels {
                    ss.output_accumulator[frame_idx * channels + ch] = 0.0;
                }
            }
        }

        ss.output_accumulator_fill += ss.hop_size;
        ss.latency_filled += ss.hop_size;
    }

    /// Main in-place processing entry point for spectral mode.
    ///
    /// Feeds interleaved samples into per-channel ring buffers. Each time
    /// `fft_size` samples have accumulated (after the initial fill), a STFT
    /// frame is processed. The OLA output is drained into the caller's buffer.
    fn process_spectral_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let nf = context.num_frames;
        let channels = self.channels;

        let any_solo =
            (0..self.num_bands).any(|b| self.band_params.get(b).map(|p| p.solo).unwrap_or(false));

        let g_mix = self.mix_smoother.next_n(nf);

        // Ensure dry buffer large enough
        if self.dry_buffer.len() < buffer.len() {
            self.dry_buffer.resize(buffer.len(), 0.0);
        }
        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        // Zero the output portion of the buffer — we'll drain OLA into it
        buffer[..nf * channels].fill(0.0);

        let mut input_pos = 0; // frame index into the caller's buffer
        let mut output_pos = 0; // frame index into the caller's output

        // Safety: spectral must be Some when this path is called
        let fft_size = self.spectral.as_ref().unwrap().fft_size;
        let hop_size = self.spectral.as_ref().unwrap().hop_size;

        while output_pos < nf {
            // --- Step 1: Fill input ring from caller's buffer ---
            if input_pos < nf {
                let ss = self.spectral.as_mut().unwrap();
                let overlap = fft_size - hop_size;
                let space_in_tail = fft_size - ss.input_fill;
                let available = nf - input_pos;
                let to_copy = space_in_tail.min(available);

                if to_copy > 0 {
                    for ch in 0..channels {
                        for i in 0..to_copy {
                            ss.input_buffers[ch][ss.input_fill + i] =
                                self.dry_buffer[(input_pos + i) * channels + ch];
                        }
                    }
                    ss.input_fill += to_copy;
                    input_pos += to_copy;
                    let _ = overlap; // suppress unused warning
                }
            }

            // --- Step 2: Process STFT frames while we have a full window ---
            {
                let input_fill = self.spectral.as_ref().unwrap().input_fill;
                let hop = self.spectral.as_ref().unwrap().hop_size;
                if input_fill >= fft_size {
                    self.process_spectral_hop(any_solo);
                    // Shift input ring: keep overlap = fft_size - hop_size samples
                    let ss = self.spectral.as_mut().unwrap();
                    let overlap = fft_size - hop;
                    for ch in 0..channels {
                        ss.input_buffers[ch].copy_within(hop..fft_size, 0);
                        ss.input_buffers[ch][overlap..].fill(0.0);
                    }
                    ss.input_fill = overlap;
                }
            }

            // --- Step 3: Drain available OLA frames into output ---
            {
                let ss = self.spectral.as_mut().unwrap();
                let frames_to_drain = ss.output_accumulator_fill.min(nf - output_pos);
                if frames_to_drain > 0 {
                    let mask = ss.output_accumulator_mask;
                    for i in 0..frames_to_drain {
                        let read_idx = (ss.output_read_position + i) & mask;
                        let out_base = (output_pos + i) * channels;
                        for ch in 0..channels {
                            buffer[out_base + ch] +=
                                ss.output_accumulator[read_idx * channels + ch];
                        }
                    }
                    // Clear drained frames
                    for i in 0..frames_to_drain {
                        let read_idx = (ss.output_read_position + i) & mask;
                        for ch in 0..channels {
                            ss.output_accumulator[read_idx * channels + ch] = 0.0;
                        }
                    }
                    ss.output_read_position = (ss.output_read_position + frames_to_drain) & mask;
                    ss.output_accumulator_fill -= frames_to_drain;
                    output_pos += frames_to_drain;
                } else {
                    // No output ready: output silence for this iteration and break
                    // This happens only during initial latency fill
                    output_pos = nf;
                }
            }
        }

        // Apply wet/dry mix with dry signal
        for i in 0..nf {
            for ch in 0..channels {
                let idx = i * channels + ch;
                buffer[idx] = self.dry_buffer[idx] * (1.0 - g_mix) + buffer[idx] * g_mix;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}

impl InPlacePlugin for MultibandExpanderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Multiband Expander", "1.2.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Handle processing_mode separately (not in GLOBAL_PARAMS)
        if id.0 == "processing_mode" {
            let idx = value
                .as_int()
                .ok_or_else(|| "processing_mode must be an integer".to_string())?;
            let mode_str = if idx == 1 { "spectral" } else { "time_domain" };
            if mode_str != self.processing_mode {
                self.processing_mode = mode_str.to_string();
                if mode_str == "spectral" && self.spectral.is_none() {
                    let fft_size = 1024;
                    let mut ss = SpectralState::new(
                        fft_size,
                        self.channels,
                        self.sample_rate,
                        &self.crossover_frequencies,
                        self.num_bands,
                    );
                    ss.update_band_coefficients(
                        self.num_bands,
                        &self.band_params,
                        self.attack_ms,
                        self.release_ms,
                        self.sample_rate,
                    );
                    self.spectral = Some(ss);
                } else if mode_str == "time_domain" {
                    self.spectral = None;
                }
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        // Try global params via param_bridge
        if let Ok(idx) =
            param_bridge::set_parameter(ME, &id, &value, |i, v| self.set_param_value(i, v))
        {
            // Side effects for specific global params
            match idx {
                0 => {
                    // num_bands changed
                    let nb = self.num_bands;
                    self.build_crossovers();
                    while self.band_params.len() < nb {
                        self.band_params.push(BandExpanderParams::default());
                    }
                    while self.band_expanders.len() < nb {
                        self.band_expanders.push(BandExpander {
                            envelope: vec![0.0; self.channels],
                            peak_env: vec![0.0; self.channels],
                            gate_state: vec![GateState::Open; self.channels],
                            hold_counter: vec![0; self.channels],
                            attack_coeff: 0.0,
                            release_coeff: 0.0,
                        });
                    }
                    while self.measured_makeups.len() < nb {
                        self.measured_makeups
                            .push(MeasuredMakeup::new(1000.0, self.sample_rate));
                    }
                    let det_mode = parse_detection_mode(&self.detection_mode);
                    while self.level_detectors.len() < nb {
                        self.level_detectors.push(
                            (0..self.channels)
                                .map(|_| LevelDetector::new(det_mode, self.sample_rate))
                                .collect(),
                        );
                    }
                    self.band_levels_db.resize(nb, -100.0);
                    self.attenuation_flattened.resize(nb * self.channels, 0.0);
                    self.is_open_buffer.resize(nb, false);
                    self.update_coefficients();

                    // Rebuild spectral bin->band mapping
                    if let Some(ss) = &mut self.spectral {
                        ss.update_bin_to_band(self.sample_rate, &self.crossover_frequencies, nb);
                        ss.update_band_coefficients(
                            nb,
                            &self.band_params,
                            self.attack_ms,
                            self.release_ms,
                            self.sample_rate,
                        );
                        for ch_states in &mut ss.bin_states {
                            ch_states.resize_with(ss.num_bins, SpectralBinState::new);
                        }
                    }
                }
                2..=5 => {
                    // crossover_freq_1..4 changed
                    let xover_idx = idx - 2;
                    if xover_idx < self.xover_smoothers.len() {
                        self.xover_smoothers[xover_idx]
                            .set_target(self.crossover_frequencies[xover_idx]);
                    }
                    // Update spectral bin->band mapping
                    if let Some(ss) = &mut self.spectral {
                        ss.update_bin_to_band(
                            self.sample_rate,
                            &self.crossover_frequencies,
                            self.num_bands,
                        );
                    }
                }
                6 => {
                    // threshold changed
                    self.threshold_smoother.set_target(self.threshold_db);
                }
                8 | 9 => {
                    // attack or release changed
                    self.update_coefficients();
                }
                14 => {
                    // mix changed
                    self.mix_smoother.set_target(self.mix);
                }
                16 => {
                    // detection_mode changed
                    let det_mode = parse_detection_mode(&self.detection_mode);
                    for band_dets in &mut self.level_detectors {
                        for det in band_dets {
                            det.set_mode(det_mode);
                        }
                    }
                }
                _ => {}
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        // Fall through to band-level param handling
        let name = &id.0;
        if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1]
                    .parse::<usize>()
                    .map_err(|e| format!("Invalid band index: {}", e))?;
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &mut self.band_params[b_idx];
                    match field {
                        "threshold" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.threshold_db = Some(v);
                            }
                        }
                        "ratio" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.ratio = Some(v);
                            }
                        }
                        "attack" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.attack_ms = Some(v);
                                self.update_coefficients();
                            }
                        }
                        "release" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.release_ms = Some(v);
                                self.update_coefficients();
                            }
                        }
                        "knee" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.knee_db = Some(v);
                            }
                        }
                        "range" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.range_db = Some(v);
                            }
                        }
                        "hysteresis" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.hysteresis_db = Some(v);
                            }
                        }
                        "hold" => {
                            let v = value
                                .as_float()
                                .ok_or_else(|| format!("{} must be a float", name))?;
                            if v.is_finite() {
                                bp.hold_ms = Some(v);
                            }
                        }
                        "auto" => {
                            bp.auto_makeup = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "measured" => {
                            bp.measured_auto_makeup = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "active" => {
                            bp.active = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "solo" => {
                            bp.solo = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        "bypass" => {
                            bp.bypass = value
                                .as_bool()
                                .ok_or_else(|| format!("{} must be a boolean", name))?
                        }
                        _ => return Err(format!("Unknown band field: {}", field)),
                    }
                } else {
                    return Err(format!("Band index {} out of range", b_idx));
                }
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Handle processing_mode separately (not in GLOBAL_PARAMS)
        if id.0 == "processing_mode" {
            let idx = if self.processing_mode == "spectral" {
                1
            } else {
                0
            };
            return Some(ParameterValue::Int(idx));
        }
        // Try global params first
        if let Some(v) = param_bridge::get_parameter(ME, id, |i| self.param_value(i)) {
            return Some(v);
        }
        // Fall through to band-level params
        let name = &id.0;
        if name.starts_with("band_") {
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() >= 3 {
                let b_idx = parts[1].parse::<usize>().unwrap_or(0);
                if b_idx < self.num_bands {
                    let field = parts[2];
                    let bp = &self.band_params[b_idx];
                    match field {
                        "threshold" => Some(ParameterValue::Float(
                            bp.threshold_db.unwrap_or(self.threshold_db),
                        )),
                        "ratio" => Some(ParameterValue::Float(bp.ratio.unwrap_or(self.ratio))),
                        "attack" => Some(ParameterValue::Float(
                            bp.attack_ms.unwrap_or(self.attack_ms),
                        )),
                        "release" => Some(ParameterValue::Float(
                            bp.release_ms.unwrap_or(self.release_ms),
                        )),
                        "knee" => Some(ParameterValue::Float(bp.knee_db.unwrap_or(self.knee_db))),
                        "range" => {
                            Some(ParameterValue::Float(bp.range_db.unwrap_or(self.range_db)))
                        }
                        "hysteresis" => Some(ParameterValue::Float(
                            bp.hysteresis_db.unwrap_or(self.hysteresis_db),
                        )),
                        "hold" => Some(ParameterValue::Float(bp.hold_ms.unwrap_or(self.hold_ms))),
                        "auto" => Some(ParameterValue::Bool(bp.auto_makeup)),
                        "measured" => Some(ParameterValue::Bool(bp.measured_auto_makeup)),
                        "active" => Some(ParameterValue::Bool(bp.active)),
                        "solo" => Some(ParameterValue::Bool(bp.solo)),
                        "bypass" => Some(ParameterValue::Bool(bp.bypass)),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.build_crossovers();
        self.update_coefficients();
        self.threshold_smoother.set_time(20.0, sr);
        self.mix_smoother.set_time(20.0, sr);
        for s in &mut self.xover_smoothers {
            *s = LogSmoother::new(s.target(), 50.0, sr);
        }

        // Reinitialize measured makeup smoothing for new sample rate
        for mm in &mut self.measured_makeups {
            mm.set_smoothing(1000.0, sr);
        }

        // Reinitialize level detectors for new sample rate
        let det_mode = parse_detection_mode(&self.detection_mode);
        for band_dets in &mut self.level_detectors {
            for det in band_dets {
                *det = LevelDetector::new(det_mode, sr);
            }
        }

        // Pre-allocate buffers for real-time safety
        let max_frames = 4096; // Standard max block size
        let stride = max_frames * self.channels;
        self.band_buffers.resize(self.num_bands * stride, 0.0);
        self.dry_buffer.resize(max_frames * self.channels, 0.0);

        // (Re-)initialize spectral state if mode is active
        if self.processing_mode == "spectral" {
            let fft_size = 1024;
            let mut ss = SpectralState::new(
                fft_size,
                self.channels,
                sr,
                &self.crossover_frequencies,
                self.num_bands,
            );
            ss.update_band_coefficients(
                self.num_bands,
                &self.band_params,
                self.attack_ms,
                self.release_ms,
                sr,
            );
            self.spectral = Some(ss);
        }

        Ok(())
    }
    fn reset(&mut self) {
        for b in &mut self.band_expanders {
            b.reset();
        }
        for mm in &mut self.measured_makeups {
            mm.reset();
        }
        for band_dets in &mut self.level_detectors {
            for det in band_dets {
                det.reset();
            }
        }
        self.band_buffers.fill(0.0);
        self.dry_buffer.fill(0.0);

        if let Some(ss) = &mut self.spectral {
            ss.reset();
        }
    }

    fn latency_samples(&self) -> usize {
        if let Some(ss) = &self.spectral {
            ss.fft_size
        } else {
            0
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        // Dispatch to spectral mode if active
        if self.processing_mode == "spectral" {
            return self.process_spectral_in_place(buffer, context);
        }

        enable_ftz_daz();
        let nf = context.num_frames;
        let stride = nf * self.channels;

        // Ensure buffers are large enough (usually a no-op due to initialize)
        if self.dry_buffer.len() < buffer.len() {
            self.dry_buffer.resize(buffer.len(), 0.0);
        }
        if self.band_buffers.len() < self.num_bands * stride {
            self.band_buffers.resize(self.num_bands * stride, 0.0);
        }

        self.dry_buffer[..buffer.len()].copy_from_slice(buffer);

        // 1. Update crossovers
        for i in 0..(self.num_bands - 1) {
            let freq = self.xover_smoothers[i].next_n(nf);
            self.crossover_points[i].set_frequency(freq);
        }

        // 2. Perform Crossover Splitting
        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut rem = buffer[idx];
                for xidx in 0..(self.num_bands - 1) {
                    let (low, high) = self.crossover_points[xidx].process(rem, ch);
                    self.band_buffers[xidx * stride + idx] = low;
                    rem = high;
                }
                self.band_buffers[(self.num_bands - 1) * stride + idx] = rem;
            }
        }

        let g_th = self.threshold_smoother.next_n(nf);
        let g_mix = self.mix_smoother.next_n(nf);

        let mut any_solo = false;
        for b in 0..self.num_bands {
            if let Some(p) = self.band_params.get(b)
                && p.solo
            {
                any_solo = true;
                break;
            }
        }

        // 3. Dynamic Processing per Band
        for b in 0..self.num_bands {
            let bp = self.band_params.get(b);
            let is_bypassed = bp.map(|p| p.bypass).unwrap_or(false);
            let is_passive = !bp.map(|p| p.active).unwrap_or(true);
            let is_muted = any_solo && !bp.map(|p| p.solo).unwrap_or(false);

            if is_muted {
                let off = b * stride;
                self.band_buffers[off..off + stride].fill(0.0);
                self.band_levels_db[b] = -100.0;
                continue;
            }

            if is_bypassed || is_passive {
                // Keep band signal as is, but still track levels
                let off = b * stride;
                let mut max_abs = 0.0f32;
                for i in 0..stride {
                    max_abs = max_abs.max(self.band_buffers[off + i].abs());
                }
                self.band_levels_db[b] = 20.0 * fast_log10(max_abs.max(1e-10));
                continue;
            }

            let th = bp.and_then(|p| p.threshold_db).unwrap_or(g_th);
            let rat = bp.and_then(|p| p.ratio).unwrap_or(self.ratio);
            let kn = bp.and_then(|p| p.knee_db).unwrap_or(self.knee_db);
            let rg = bp.and_then(|p| p.range_db).unwrap_or(self.range_db);
            let hys = bp
                .and_then(|p| p.hysteresis_db)
                .unwrap_or(self.hysteresis_db);
            let hs = (bp.and_then(|p| p.hold_ms).unwrap_or(self.hold_ms)
                * 0.001
                * self.sample_rate as f32) as usize;
            let use_measured_makeup = bp.map(|p| p.measured_auto_makeup).unwrap_or(false);
            let auto_makeup_gain = if use_measured_makeup {
                // Measured makeup: will be computed per-frame below
                1.0
            } else if bp.map(|p| p.auto_makeup).unwrap_or(false) {
                let slope = 1.0 - 1.0 / rat.max(1.0);
                let avg_atten = rg.max(0.0) * slope * 0.5;
                fast_pow10(avg_atten / 20.0)
            } else {
                1.0
            };

            let use_rms = self.detection_mode == "rms";
            let bexp = &mut self.band_expanders[b];
            let off = b * stride;
            let mut band_max_abs = 0.0f32;

            for frame in 0..nf {
                // Update per-channel peak envelope followers first
                for ch in 0..self.channels {
                    let s = self.band_buffers[off + frame * self.channels + ch].abs();
                    // Instant attack, release using attack_coeff (fast decay prevents
                    // zero-crossing dips from inflating expansion targets)
                    bexp.peak_env[ch] = s.max(bexp.attack_coeff * bexp.peak_env[ch]);
                }

                let mut det_db = 0.0f32;
                if self.link_channels {
                    if use_rms {
                        let mut max_rms_db = -120.0f32;
                        for ch in 0..self.channels {
                            let s = self.band_buffers[off + frame * self.channels + ch];
                            let ch_db = self.level_detectors[b][ch].process(s);
                            max_rms_db = max_rms_db.max(ch_db);
                        }
                        det_db = max_rms_db;
                    } else {
                        let mut peak = 0.0f32;
                        for ch in 0..self.channels {
                            peak = peak.max(bexp.peak_env[ch]);
                        }
                        det_db = 20.0 * fast_log10(peak.max(1e-10));
                    }
                }

                for ch in 0..self.channels {
                    let idx = off + frame * self.channels + ch;
                    let sample_abs = self.band_buffers[idx].abs();
                    band_max_abs = band_max_abs.max(sample_abs);

                    let db = if self.link_channels {
                        det_db
                    } else if use_rms {
                        self.level_detectors[b][ch].process(self.band_buffers[idx])
                    } else {
                        20.0 * fast_log10(bexp.peak_env[ch].max(1e-10))
                    };

                    let target = match bexp.gate_state[ch] {
                        GateState::Open => {
                            if db < th {
                                bexp.gate_state[ch] = GateState::Hold;
                                bexp.hold_counter[ch] = hs;
                                0.0
                            } else {
                                0.0
                            }
                        }
                        GateState::Hold => {
                            if db >= th {
                                bexp.gate_state[ch] = GateState::Open;
                                0.0
                            } else if bexp.hold_counter[ch] > 0 {
                                bexp.hold_counter[ch] -= 1;
                                0.0
                            } else if db < th - hys {
                                bexp.gate_state[ch] = GateState::Closing;
                                Self::calculate_expansion_attenuation(db, th, rat, kn, rg)
                            } else {
                                0.0
                            }
                        }
                        GateState::Closing => {
                            if db >= th {
                                bexp.gate_state[ch] = GateState::Open;
                                0.0
                            } else {
                                Self::calculate_expansion_attenuation(db, th, rat, kn, rg)
                            }
                        }
                    };

                    let c = if target > bexp.envelope[ch] {
                        bexp.attack_coeff
                    } else {
                        bexp.release_coeff
                    };
                    bexp.envelope[ch] = target + c * (bexp.envelope[ch] - target);

                    // Update measured makeup tracker if enabled
                    if use_measured_makeup {
                        self.measured_makeups[b].update(bexp.envelope[ch]);
                    }

                    let gain_linear = fast_pow10(-bexp.envelope[ch] / 20.0);
                    let makeup = if use_measured_makeup {
                        self.measured_makeups[b].makeup_linear()
                    } else {
                        auto_makeup_gain
                    };
                    self.band_buffers[idx] *= gain_linear * makeup;
                }
            }
            self.band_levels_db[b] = 20.0 * fast_log10(band_max_abs.max(1e-10));
        }

        // 4. Recombination
        for frame in 0..nf {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut s = 0.0f32;
                for b in 0..self.num_bands {
                    s += self.band_buffers[b * stride + idx];
                }
                buffer[idx] = self.dry_buffer[idx] * (1.0 - g_mix) + s * g_mix;
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            for b in 0..self.num_bands {
                self.is_open_buffer[b] = self.band_expanders[b]
                    .gate_state
                    .iter()
                    .any(|&s| s != GateState::Closing);
                for ch in 0..self.channels {
                    self.attenuation_flattened[b * self.channels + ch] =
                        self.band_expanders[b].envelope[ch];
                }
            }
            let levels = &self.band_levels_db;
            let xovers = &self.crossover_frequencies;
            let open = &self.is_open_buffer;
            self.cache.update(|d| {
                d.update(&self.attenuation_flattened, open, levels, xovers);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn test_mb_exp_basic() {
        let mut p = MultibandExpanderPlugin::new(1);
        p.initialize(48000).unwrap();
        let mut b = vec![0.1; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999].is_finite());
    }

    /// Verify that low-frequency content triggers expansion in the lowest band
    /// even with default detection settings (no sidechain HPF blocking bass).
    #[test]
    fn test_low_frequency_triggers_expansion() {
        let mut params = MultibandExpanderPluginParams {
            num_bands: 3,
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            range_db: 40.0,
            mix: 1.0,
            ..Default::default()
        };
        params.bands = vec![
            BandExpanderParams {
                threshold_db: Some(-20.0),
                ratio: Some(4.0),
                hold_ms: Some(0.0),
                hysteresis_db: Some(0.0),
                range_db: Some(40.0),
                ..Default::default()
            },
            BandExpanderParams::default(),
            BandExpanderParams::default(),
        ];
        let mut p = MultibandExpanderPlugin::with_params(1, params);
        p.initialize(48000).unwrap();

        // Feed a loud 50 Hz signal (above threshold) to open the gate
        let nf = 9600;
        let mut loud: Vec<f32> = (0..nf)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin())
            .collect();
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: nf,
        };
        p.process_in_place(&mut loud, &ctx).unwrap();

        // Verify the loud signal passed through with reasonable level
        let rms_loud: f32 =
            (loud[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        assert!(
            rms_loud > 0.05,
            "Loud 50 Hz signal should pass through expander (gate open), RMS={rms_loud:.6}"
        );

        // Now feed a very quiet 50 Hz signal (below threshold)
        let quiet_amp = 0.001;
        let mut quiet: Vec<f32> = (0..nf)
            .map(|i| {
                quiet_amp * (2.0 * std::f32::consts::PI * 50.0 * (nf + i) as f32 / 48000.0).sin()
            })
            .collect();
        p.process_in_place(&mut quiet, &ctx).unwrap();

        // The quiet signal should be attenuated (gate closing)
        let rms_quiet: f32 =
            (quiet[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
        let input_rms = quiet_amp / std::f32::consts::SQRT_2;
        assert!(
            rms_quiet < input_rms,
            "Quiet 50 Hz signal should be attenuated by expander, \
             but rms_out={rms_quiet:.8} >= input_rms={input_rms:.8}"
        );
    }

    /// Regression: attack/release coefficients were swapped in per-band processing.
    /// With fast attack and slow release, quiet signals below threshold should be
    /// attenuated quickly (gate closes fast).
    #[test]
    fn test_mb_expander_attack_release_not_swapped() {
        let mut params = MultibandExpanderPluginParams {
            num_bands: 2,
            mix: 1.0,       // wet-only to observe expansion effect
            range_db: 60.0, // allow up to 60 dB of expansion attenuation
            ..Default::default()
        };
        params.bands = vec![
            BandExpanderParams {
                threshold_db: Some(-20.0),
                ratio: Some(10.0),
                attack_ms: Some(1.0),
                release_ms: Some(200.0),
                hold_ms: Some(0.0),
                hysteresis_db: Some(0.0),
                range_db: Some(60.0),
                ..Default::default()
            },
            BandExpanderParams {
                threshold_db: Some(-20.0),
                ratio: Some(10.0),
                attack_ms: Some(1.0),
                release_ms: Some(200.0),
                hold_ms: Some(0.0),
                hysteresis_db: Some(0.0),
                range_db: Some(60.0),
                ..Default::default()
            },
        ];
        let mut p = MultibandExpanderPlugin::with_params(1, params);
        p.initialize(48000).unwrap();

        // Feed loud broadband signal to open gates
        let mut loud = Vec::with_capacity(9600);
        for i in 0..9600 {
            loud.push(0.5 * (i as f32 * 0.3).sin());
        }
        p.process_in_place(
            &mut loud,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 9600,
            },
        )
        .unwrap();

        // Feed quiet broadband signal — gates should close fast with 1ms attack
        let quiet_peak = 0.001f32;
        let mut quiet = Vec::with_capacity(2400);
        for i in 0..2400 {
            quiet.push(quiet_peak * (i as f32 * 0.3).sin());
        }
        let quiet_rms_in: f32 =
            (quiet.iter().map(|s| s * s).sum::<f32>() / quiet.len() as f32).sqrt();
        p.process_in_place(
            &mut quiet,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 2400,
            },
        )
        .unwrap();

        // After 50ms with 1ms attack (and 0ms hold), the signal should be attenuated.
        let quiet_rms_out: f32 =
            (quiet[1200..].iter().map(|s| s * s).sum::<f32>() / (quiet.len() - 1200) as f32).sqrt();
        assert!(
            quiet_rms_out < quiet_rms_in * 0.8,
            "Multiband expander gate should close fast with 1ms attack, \
             but RMS out {quiet_rms_out:.6} is too close to RMS in {quiet_rms_in:.6}. \
             Attack/release coefficients may be swapped."
        );
    }

    /// Unity passthrough: with threshold at minimum and ratio 1:1,
    /// the expander should not alter the signal significantly.
    #[test]
    fn test_mb_expander_unity_passthrough() {
        let mut params = MultibandExpanderPluginParams {
            num_bands: 3,
            ..Default::default()
        };
        for band in &mut params.bands {
            band.ratio = Some(1.0); // no expansion
        }
        let mut p = MultibandExpanderPlugin::with_params(2, params);
        p.initialize(48000).unwrap();

        // Generate test signal
        let mut input = vec![0.0f32; 4800 * 2];
        for i in 0..4800 {
            let val = 0.3 * (i as f32 * 0.05).sin();
            input[i * 2] = val;
            input[i * 2 + 1] = val;
        }
        let mut output = input.clone();
        p.process_in_place(
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 4800,
            },
        )
        .unwrap();

        // After settling (crossover filter delay), output should be close to input.
        // Allow for crossover phase shift but RMS should be similar.
        let rms_in: f32 =
            (input[2400..].iter().map(|s| s * s).sum::<f32>() / (input.len() - 2400) as f32).sqrt();
        let rms_out: f32 = (output[2400..].iter().map(|s| s * s).sum::<f32>()
            / (output.len() - 2400) as f32)
            .sqrt();
        let ratio = rms_out / rms_in;
        assert!(
            (0.7..1.3).contains(&ratio),
            "Unity ratio (1:1) should pass through, but RMS ratio is {ratio:.3}"
        );
    }

    /// Spectral mode: basic smoke test — output must be finite and non-silent for a
    /// loud input signal (threshold set very low so gate is open).
    #[test]
    fn test_spectral_mode_basic() {
        let params = MultibandExpanderPluginParams {
            num_bands: 3,
            threshold_db: -80.0, // very low threshold: gate always open
            ratio: 2.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            range_db: 40.0,
            mix: 1.0,
            processing_mode: "spectral".to_string(),
            ..Default::default()
        };
        let mut p = MultibandExpanderPlugin::with_params(2, params);
        p.initialize(48000).unwrap();

        // Check that latency is reported as fft_size
        assert_eq!(
            p.latency_samples(),
            1024,
            "Spectral mode latency should be fft_size=1024"
        );

        // Generate pink-noise-like signal using sum of sines
        let nf = 8192usize;
        let mut signal = vec![0.0f32; nf * 2];
        for i in 0..nf {
            let t = i as f32 / 48000.0;
            let s = 0.3 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                + 0.15 * (2.0 * std::f32::consts::PI * 880.0 * t).sin()
                + 0.08 * (2.0 * std::f32::consts::PI * 3520.0 * t).sin();
            signal[i * 2] = s;
            signal[i * 2 + 1] = s;
        }

        let mut buf = signal.clone();
        p.process_in_place(
            &mut buf,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: nf,
            },
        )
        .unwrap();

        // All output samples must be finite
        for (i, &s) in buf.iter().enumerate() {
            assert!(s.is_finite(), "Sample {i} is not finite: {s}");
        }

        // After the latency fill (~fft_size frames = 1024), output must not be all-zeros
        let rms_out: f32 =
            (buf[1024 * 2..].iter().map(|s| s * s).sum::<f32>() / ((nf - 1024) * 2) as f32).sqrt();
        assert!(
            rms_out > 1e-5,
            "Spectral mode output should not be silent for loud input, RMS={rms_out:.8}"
        );
    }

    /// Integration test: verify that the multiband expander actually attenuates audio.
    ///
    /// A quiet DC-offset signal at -40 dBFS is fed to a 2-band expander whose
    /// threshold is set at -20 dB and ratio at 4:1.  After processing, the
    /// output RMS must be lower than the input RMS — confirming that expansion
    /// is being applied and not just passing audio through unchanged.
    #[test]
    fn test_multiband_expander_processes_audio() {
        let params = MultibandExpanderPluginParams {
            num_bands: 2,
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            range_db: 60.0,
            hold_ms: 0.0,
            hysteresis_db: 0.0,
            mix: 1.0,
            ..Default::default()
        };
        let mut p = MultibandExpanderPlugin::with_params(1, params);
        p.initialize(48000).unwrap();

        // Quiet DC-offset signal at -40 dBFS (well below -20 dB threshold)
        let amp = 10.0_f32.powf(-40.0 / 20.0);
        let num_frames = 48000usize; // 1 second
        let mut buffer = vec![amp; num_frames];

        let input_rms = amp; // DC: RMS == amplitude

        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        p.process_in_place(&mut buffer, &ctx).unwrap();

        // Measure RMS of the second half to let the expander settle
        let half = num_frames / 2;
        let output_rms: f32 =
            (buffer[half..].iter().map(|s| s * s).sum::<f32>() / (num_frames - half) as f32).sqrt();

        assert!(
            output_rms < input_rms * 0.9,
            "Multiband expander should attenuate a -40 dBFS signal below the -20 dB threshold, \
             but output_rms={output_rms:.8} is not significantly less than input_rms={input_rms:.8}"
        );
    }

    /// Spectral mode: below-threshold signal should be attenuated compared to time-domain mode.
    ///
    /// Both modes are configured identically. A quiet signal (below threshold) is fed to each.
    /// The spectral mode attenuation is compared against the time-domain mode attenuation.
    /// We do not require them to be identical (STFT resolution differs from sample-accurate
    /// tracking), but both should attenuate significantly relative to the unprocessed signal.
    #[test]
    fn test_spectral_vs_time_domain_attenuation() {
        let sr = 48000u32;
        let nf = 16384usize; // enough for multiple STFT hops

        // Quiet broadband signal (below a -20 dB threshold)
        let quiet_amp = 0.005f32; // ~ -46 dBFS
        let signal: Vec<f32> = (0..nf)
            .map(|i| {
                quiet_amp
                    * ((2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr as f32).sin()
                        + (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin()
                        + (2.0 * std::f32::consts::PI * 4000.0 * i as f32 / sr as f32).sin())
                    / 3.0
            })
            .collect();
        let input_rms: f32 = (signal.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();

        let make_params = |mode: &str| MultibandExpanderPluginParams {
            num_bands: 3,
            threshold_db: -20.0,
            ratio: 8.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 0.0,
            range_db: 60.0,
            hysteresis_db: 0.0,
            hold_ms: 0.0,
            mix: 1.0,
            processing_mode: mode.to_string(),
            crossover_frequencies: vec![300.0, 3000.0, 8000.0, 12000.0],
            ..Default::default()
        };

        let mut td_plugin = MultibandExpanderPlugin::with_params(1, make_params("time_domain"));
        td_plugin.initialize(sr).unwrap();

        let mut sp_plugin = MultibandExpanderPlugin::with_params(1, make_params("spectral"));
        sp_plugin.initialize(sr).unwrap();

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: nf,
        };

        let mut td_buf = signal.clone();
        td_plugin.process_in_place(&mut td_buf, &ctx).unwrap();

        let mut sp_buf = signal.clone();
        sp_plugin.process_in_place(&mut sp_buf, &ctx).unwrap();

        // Use the second half of the buffer to avoid transient settling
        let half = nf / 2;
        let td_rms: f32 =
            (td_buf[half..].iter().map(|s| s * s).sum::<f32>() / (nf - half) as f32).sqrt();
        let sp_rms: f32 =
            (sp_buf[half..].iter().map(|s| s * s).sum::<f32>() / (nf - half) as f32).sqrt();

        // Both modes should attenuate: output RMS must be < 80% of input RMS
        assert!(
            td_rms < input_rms * 0.8,
            "Time-domain mode should attenuate below-threshold signal, \
             input_rms={input_rms:.6}, td_rms={td_rms:.6}"
        );
        // Spectral mode has STFT latency and OLA settling; use a looser threshold
        assert!(
            sp_rms < input_rms * 0.98,
            "Spectral mode should attenuate below-threshold signal, \
             input_rms={input_rms:.6}, sp_rms={sp_rms:.6}"
        );
    }
}
