// ============================================================================
// Multiband Compressor Plugin
// ============================================================================
//
// Dynamic range compressor with configurable frequency bands (2-5 bands).
// Uses Linkwitz-Riley 24dB/oct crossover filters for phase-coherent band splitting.
//
// Architecture:
// Input → [LR Crossover Filters] → [Band Compressors] → Sum → Output
//
// Parameters:
// - num_bands: Number of frequency bands (2-5)
// - crossover_preset: Preset frequencies (0=Custom, 1=200/2k, 2=100/3k, 3=250/4k)
// - crossover_freq_1-4: Custom crossover frequencies
// - Global: threshold, ratio, attack, release, knee (defaults for all bands)
// - Per-band: threshold, ratio, attack, release, knee, makeup_gain, solo, bypass

use super::param_specs::multiband_compressor::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use autoeq_iir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

/// Crossover presets
pub const CROSSOVER_PRESETS: &[(f32, f32, f32, f32)] = &[
    (200.0, 2000.0, 8000.0, 12000.0),   // Preset 1: Classic 200/2k
    (100.0, 3000.0, 8000.0, 12000.0),   // Preset 2: Wide mid 100/3k
    (250.0, 4000.0, 10000.0, 14000.0),  // Preset 3: Hi emphasis 250/4k
];

/// Per-band compressor parameters (optional overrides)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandCompressorParams {
    /// Threshold override (None = use global)
    #[serde(default)]
    pub threshold_db: Option<f32>,
    /// Ratio override (None = use global)
    #[serde(default)]
    pub ratio: Option<f32>,
    /// Attack override (None = use global)
    #[serde(default)]
    pub attack_ms: Option<f32>,
    /// Release override (None = use global)
    #[serde(default)]
    pub release_ms: Option<f32>,
    /// Knee override (None = use global)
    #[serde(default)]
    pub knee_db: Option<f32>,
    /// Per-band makeup gain (always per-band)
    #[serde(default)]
    pub makeup_gain_db: f32,
    /// Solo this band
    #[serde(default)]
    pub solo: bool,
    /// Bypass this band
    #[serde(default)]
    pub bypass: bool,
}

impl Default for BandCompressorParams {
    fn default() -> Self {
        Self {
            threshold_db: None,
            ratio: None,
            attack_ms: None,
            release_ms: None,
            knee_db: None,
            makeup_gain_db: 0.0,
            solo: false,
            bypass: false,
        }
    }
}

/// Configuration parameters for MultibandCompressorPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultibandCompressorPluginParams {
    /// Number of bands (2-5)
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,
    /// Crossover preset (0=Custom, 1-3=presets)
    #[serde(default = "default_crossover_preset")]
    pub crossover_preset: i32,
    /// Custom crossover frequencies
    #[serde(default = "default_crossover_frequencies")]
    pub crossover_frequencies: Vec<f32>,
    /// Global threshold
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    /// Global ratio
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    /// Global attack
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    /// Global release
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    /// Global knee
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    /// Link channels
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    /// Dry/wet mix
    #[serde(default = "default_mix")]
    pub mix: f32,
    /// Per-band parameters
    #[serde(default)]
    pub bands: Vec<BandCompressorParams>,
}

fn default_num_bands() -> usize {
    NUM_BANDS_DEFAULT
}
fn default_crossover_preset() -> i32 {
    CROSSOVER_PRESET_DEFAULT
}
fn default_crossover_frequencies() -> Vec<f32> {
    vec![
        CROSSOVER_FREQ_1_DEFAULT,
        CROSSOVER_FREQ_2_DEFAULT,
        CROSSOVER_FREQ_3_DEFAULT,
        CROSSOVER_FREQ_4_DEFAULT,
    ]
}
fn default_threshold_db() -> f32 {
    THRESHOLD_DEFAULT
}
fn default_ratio() -> f32 {
    RATIO_DEFAULT
}
fn default_attack_ms() -> f32 {
    ATTACK_DEFAULT
}
fn default_release_ms() -> f32 {
    RELEASE_DEFAULT
}
fn default_knee_db() -> f32 {
    KNEE_DEFAULT
}
fn default_link_channels() -> bool {
    LINK_CHANNELS_DEFAULT
}
fn default_mix() -> f32 {
    MIX_DEFAULT
}

impl Default for MultibandCompressorPluginParams {
    fn default() -> Self {
        Self {
            num_bands: default_num_bands(),
            crossover_preset: default_crossover_preset(),
            crossover_frequencies: default_crossover_frequencies(),
            threshold_db: default_threshold_db(),
            ratio: default_ratio(),
            attack_ms: default_attack_ms(),
            release_ms: default_release_ms(),
            knee_db: default_knee_db(),
            link_channels: default_link_channels(),
            mix: default_mix(),
            bands: Vec::new(),
        }
    }
}

/// Data exposed by the multiband compressor for monitoring
#[derive(Debug, Clone)]
pub struct MultibandCompressorData {
    /// Gain reduction per band per channel [band][channel]
    pub gain_reduction_db: Vec<Vec<f32>>,
    /// RMS level per band (dB)
    pub band_levels_db: Vec<f32>,
    /// Crossover frequencies currently in use
    pub crossover_frequencies: Vec<f32>,
}

// ============================================================================
// Crossover Filter Bank
// ============================================================================

/// A single crossover point with LP and HP filters
struct CrossoverPoint {
    /// Lowpass filters per channel [channel][cascade_stage]
    lowpass: Vec<Vec<Biquad>>,
    /// Highpass filters per channel [channel][cascade_stage]
    highpass: Vec<Vec<Biquad>>,
    /// Crossover frequency
    frequency: f32,
}

impl CrossoverPoint {
    fn new(channels: usize, frequency: f32, sample_rate: u32) -> Self {
        let q = 1.0 / std::f64::consts::SQRT_2; // Butterworth Q = 0.707

        let mut lowpass = Vec::with_capacity(channels);
        let mut highpass = Vec::with_capacity(channels);

        for _ in 0..channels {
            // LR24 = 2 cascaded 2nd-order Butterworth filters
            let lp1 = Biquad::new(
                BiquadFilterType::Lowpass,
                frequency as f64,
                sample_rate as f64,
                q,
                0.0,
            );
            let lp2 = Biquad::new(
                BiquadFilterType::Lowpass,
                frequency as f64,
                sample_rate as f64,
                q,
                0.0,
            );
            lowpass.push(vec![lp1, lp2]);

            let hp1 = Biquad::new(
                BiquadFilterType::Highpass,
                frequency as f64,
                sample_rate as f64,
                q,
                0.0,
            );
            let hp2 = Biquad::new(
                BiquadFilterType::Highpass,
                frequency as f64,
                sample_rate as f64,
                q,
                0.0,
            );
            highpass.push(vec![hp1, hp2]);
        }

        Self {
            lowpass,
            highpass,
            frequency,
        }
    }

    fn process_lowpass(&mut self, channel: usize, sample: f32) -> f32 {
        let mut s = sample;
        for filter in &mut self.lowpass[channel] {
            s = filter.process(s as f64) as f32;
        }
        s
    }

    fn process_highpass(&mut self, channel: usize, sample: f32) -> f32 {
        let mut s = sample;
        for filter in &mut self.highpass[channel] {
            s = filter.process(s as f64) as f32;
        }
        s
    }

    fn reset(&mut self, sample_rate: u32) {
        let q = 1.0 / std::f64::consts::SQRT_2;
        for ch_filters in &mut self.lowpass {
            for filter in ch_filters {
                *filter = Biquad::new(
                    BiquadFilterType::Lowpass,
                    self.frequency as f64,
                    sample_rate as f64,
                    q,
                    0.0,
                );
            }
        }
        for ch_filters in &mut self.highpass {
            for filter in ch_filters {
                *filter = Biquad::new(
                    BiquadFilterType::Highpass,
                    self.frequency as f64,
                    sample_rate as f64,
                    q,
                    0.0,
                );
            }
        }
    }
}

// ============================================================================
// Band Compressor State
// ============================================================================

/// Per-band compressor state
struct BandCompressor {
    /// Envelope per channel
    envelope: Vec<f32>,
    /// Attack coefficient
    attack_coeff: f32,
    /// Release coefficient
    release_coeff: f32,
}

impl BandCompressor {
    fn new(channels: usize) -> Self {
        Self {
            envelope: vec![0.0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
        }
    }

    fn update_coefficients(&mut self, attack_ms: f32, release_ms: f32, sample_rate: u32) {
        self.attack_coeff = if attack_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (attack_ms * 0.001 * sample_rate as f32)).exp()
        };
        self.release_coeff = if release_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (release_ms * 0.001 * sample_rate as f32)).exp()
        };
    }

    fn reset(&mut self) {
        self.envelope.fill(0.0);
    }
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Multiband compressor plugin
pub struct MultibandCompressorPlugin {
    channels: usize,
    sample_rate: u32,

    // Configuration
    num_bands: usize,
    crossover_preset: i32,
    crossover_frequencies: Vec<f32>,

    // Global parameters
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    link_channels: bool,
    mix: f32,

    // Per-band parameters
    band_params: Vec<BandCompressorParams>,

    // Processing state
    crossover_points: Vec<CrossoverPoint>,
    band_compressors: Vec<BandCompressor>,

    // Temporary buffers for band processing
    band_buffers: Vec<Vec<f32>>, // [band][samples]
    band_levels_db: Vec<f32>,    // RMS per band

    // Parameter IDs
    param_num_bands: ParameterId,
    param_crossover_preset: ParameterId,
    param_crossover_freq_1: ParameterId,
    param_crossover_freq_2: ParameterId,
    param_crossover_freq_3: ParameterId,
    param_crossover_freq_4: ParameterId,
    param_threshold: ParameterId,
    param_ratio: ParameterId,
    param_attack: ParameterId,
    param_release: ParameterId,
    param_knee: ParameterId,
    param_link_channels: ParameterId,
    param_mix: ParameterId,
}

impl MultibandCompressorPlugin {
    /// Create a new multiband compressor with default parameters
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, MultibandCompressorPluginParams::default())
    }

    /// Create a new multiband compressor with custom parameters
    pub fn with_params(channels: usize, params: MultibandCompressorPluginParams) -> Self {
        let num_bands = params.num_bands.clamp(NUM_BANDS_MIN, NUM_BANDS_MAX);

        // Initialize per-band parameters
        let mut band_params = params.bands.clone();
        while band_params.len() < num_bands {
            band_params.push(BandCompressorParams::default());
        }

        // Initialize band compressors
        let band_compressors: Vec<_> = (0..num_bands)
            .map(|_| BandCompressor::new(channels))
            .collect();

        // Get crossover frequencies from preset or custom
        let crossover_frequencies = Self::get_crossover_frequencies(
            params.crossover_preset,
            &params.crossover_frequencies,
        );

        Self {
            channels,
            sample_rate: 44100, // Updated in initialize()
            num_bands,
            crossover_preset: params.crossover_preset,
            crossover_frequencies,
            threshold_db: params.threshold_db,
            ratio: params.ratio,
            attack_ms: params.attack_ms,
            release_ms: params.release_ms,
            knee_db: params.knee_db,
            link_channels: params.link_channels,
            mix: params.mix.clamp(0.0, 1.0),
            band_params,
            crossover_points: Vec::new(), // Built in initialize()
            band_compressors,
            band_buffers: Vec::new(), // Allocated in initialize()
            band_levels_db: vec![0.0; num_bands],
            param_num_bands: ParameterId::from("num_bands"),
            param_crossover_preset: ParameterId::from("crossover_preset"),
            param_crossover_freq_1: ParameterId::from("crossover_freq_1"),
            param_crossover_freq_2: ParameterId::from("crossover_freq_2"),
            param_crossover_freq_3: ParameterId::from("crossover_freq_3"),
            param_crossover_freq_4: ParameterId::from("crossover_freq_4"),
            param_threshold: ParameterId::from("threshold"),
            param_ratio: ParameterId::from("ratio"),
            param_attack: ParameterId::from("attack"),
            param_release: ParameterId::from("release"),
            param_knee: ParameterId::from("knee"),
            param_link_channels: ParameterId::from("link_channels"),
            param_mix: ParameterId::from("mix"),
        }
    }

    /// Create from params (for compatibility)
    pub fn from_params(channels: usize, params: MultibandCompressorPluginParams) -> Self {
        Self::with_params(channels, params)
    }

    /// Get crossover frequencies from preset or custom values
    fn get_crossover_frequencies(preset: i32, custom: &[f32]) -> Vec<f32> {
        if preset > 0 && preset <= CROSSOVER_PRESETS.len() as i32 {
            let (f1, f2, f3, f4) = CROSSOVER_PRESETS[(preset - 1) as usize];
            vec![f1, f2, f3, f4]
        } else {
            // Custom or preset 0
            let mut freqs = custom.to_vec();
            while freqs.len() < 4 {
                freqs.push(default_crossover_frequencies()[freqs.len()]);
            }
            freqs
        }
    }

    /// Build crossover filter bank
    fn build_crossovers(&mut self) {
        self.crossover_points.clear();

        // For N bands, we need N-1 crossover points
        let num_crossovers = self.num_bands - 1;
        for i in 0..num_crossovers {
            let freq = self.crossover_frequencies.get(i).copied().unwrap_or(1000.0);
            self.crossover_points.push(CrossoverPoint::new(
                self.channels,
                freq,
                self.sample_rate,
            ));
        }
    }

    /// Update compressor coefficients
    fn update_coefficients(&mut self) {
        for (i, band) in self.band_compressors.iter_mut().enumerate() {
            let attack = self
                .band_params
                .get(i)
                .and_then(|p| p.attack_ms)
                .unwrap_or(self.attack_ms);
            let release = self
                .band_params
                .get(i)
                .and_then(|p| p.release_ms)
                .unwrap_or(self.release_ms);
            band.update_coefficients(attack, release, self.sample_rate);
        }
    }

    /// Get effective threshold for a band
    fn get_band_threshold(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.threshold_db)
            .unwrap_or(self.threshold_db)
    }

    /// Get effective ratio for a band
    fn get_band_ratio(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.ratio)
            .unwrap_or(self.ratio)
    }

    /// Get effective knee for a band
    fn get_band_knee(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.knee_db)
            .unwrap_or(self.knee_db)
    }

    /// Get makeup gain for a band
    fn get_band_makeup(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .map(|p| p.makeup_gain_db)
            .unwrap_or(0.0)
    }

    /// Check if band is soloed
    fn is_band_solo(&self, band: usize) -> bool {
        self.band_params.get(band).map(|p| p.solo).unwrap_or(false)
    }

    /// Check if band is bypassed
    fn is_band_bypass(&self, band: usize) -> bool {
        self.band_params
            .get(band)
            .map(|p| p.bypass)
            .unwrap_or(false)
    }

    /// Calculate gain reduction with soft knee
    fn calculate_gain_reduction(input_db: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
        let slope = 1.0 - 1.0 / ratio.max(1.0);

        if knee < 0.1 {
            // Hard knee
            if input_db <= threshold {
                0.0
            } else {
                (input_db - threshold) * slope
            }
        } else {
            // Soft knee
            if input_db < threshold - knee / 2.0 {
                0.0
            } else if input_db > threshold + knee / 2.0 {
                (input_db - threshold) * slope
            } else {
                let overshoot = input_db - threshold + knee / 2.0;
                let knee_factor = overshoot / knee;
                knee_factor * knee_factor * knee / 2.0 * slope
            }
        }
    }

    /// Split input into frequency bands
    fn split_bands(&mut self, input: &[f32], num_frames: usize) {
        // Ensure band buffers are allocated
        if self.band_buffers.len() != self.num_bands {
            self.band_buffers = vec![vec![0.0; num_frames * self.channels]; self.num_bands];
        }
        for buf in &mut self.band_buffers {
            if buf.len() < num_frames * self.channels {
                buf.resize(num_frames * self.channels, 0.0);
            }
            buf.fill(0.0);
        }

        // Process each sample through the crossover network
        for frame in 0..num_frames {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut remaining = input[idx];

                // Split through each crossover point
                for (xover_idx, crossover) in self.crossover_points.iter_mut().enumerate() {
                    // Low band gets the lowpass output
                    let low = crossover.process_lowpass(ch, remaining);
                    self.band_buffers[xover_idx][idx] += low;

                    // High portion continues to next crossover
                    remaining = crossover.process_highpass(ch, remaining);
                }

                // Highest band gets whatever remains
                self.band_buffers[self.num_bands - 1][idx] = remaining;
            }
        }
    }

    /// Process compression for each band
    fn process_bands(&mut self, num_frames: usize) {
        // Check if any band is soloed
        let any_solo = (0..self.num_bands).any(|b| self.is_band_solo(b));

        for band in 0..self.num_bands {
            let bypass = self.is_band_bypass(band);
            let solo = self.is_band_solo(band);
            let muted = any_solo && !solo;

            if muted {
                // Mute this band (some band is soloed and it's not this one)
                self.band_buffers[band].fill(0.0);
                continue;
            }

            if bypass {
                // No processing, just pass through
                continue;
            }

            let threshold = self.get_band_threshold(band);
            let ratio = self.get_band_ratio(band);
            let knee = self.get_band_knee(band);
            let makeup_linear = 10.0_f32.powf(self.get_band_makeup(band) / 20.0);

            let band_comp = &mut self.band_compressors[band];

            if self.link_channels && self.channels > 1 {
                // Linked processing
                for frame in 0..num_frames {
                    // Find max level across channels
                    let mut max_level = 0.0_f32;
                    for ch in 0..self.channels {
                        let idx = frame * self.channels + ch;
                        max_level = max_level.max(self.band_buffers[band][idx].abs());
                    }

                    let input_db = 20.0 * max_level.max(1e-10).log10();
                    let target_gr = Self::calculate_gain_reduction(input_db, threshold, ratio, knee);

                    // Apply same gain reduction to all channels
                    for ch in 0..self.channels {
                        let idx = frame * self.channels + ch;

                        let coeff = if target_gr > band_comp.envelope[ch] {
                            band_comp.attack_coeff
                        } else {
                            band_comp.release_coeff
                        };
                        band_comp.envelope[ch] =
                            target_gr + coeff * (band_comp.envelope[ch] - target_gr);

                        let gain = 10.0_f32.powf(-band_comp.envelope[ch] / 20.0) * makeup_linear;
                        self.band_buffers[band][idx] *= gain;
                    }
                }
            } else {
                // Unlinked processing
                for frame in 0..num_frames {
                    for ch in 0..self.channels {
                        let idx = frame * self.channels + ch;
                        let sample = self.band_buffers[band][idx];

                        let input_db = 20.0 * sample.abs().max(1e-10).log10();
                        let target_gr =
                            Self::calculate_gain_reduction(input_db, threshold, ratio, knee);

                        let coeff = if target_gr > band_comp.envelope[ch] {
                            band_comp.attack_coeff
                        } else {
                            band_comp.release_coeff
                        };
                        band_comp.envelope[ch] =
                            target_gr + coeff * (band_comp.envelope[ch] - target_gr);

                        let gain = 10.0_f32.powf(-band_comp.envelope[ch] / 20.0) * makeup_linear;
                        self.band_buffers[band][idx] = sample * gain;
                    }
                }
            }

            // Calculate RMS for monitoring
            let mut sum_sq = 0.0_f32;
            let count = self.band_buffers[band].len();
            for &s in &self.band_buffers[band] {
                sum_sq += s * s;
            }
            let rms = (sum_sq / count as f32).sqrt();
            self.band_levels_db[band] = 20.0 * rms.max(1e-10).log10();
        }
    }

    /// Sum bands back together
    fn sum_bands(&self, output: &mut [f32], num_frames: usize) {
        for frame in 0..num_frames {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut sum = 0.0_f32;
                for band in 0..self.num_bands {
                    sum += self.band_buffers[band][idx];
                }
                output[idx] = sum;
            }
        }
    }
}

impl InPlacePlugin for MultibandCompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Multiband Compressor".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: format!(
                "{}-band compressor with LR24 crossovers",
                self.num_bands
            ),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_int(
                "num_bands",
                "Bands",
                NUM_BANDS_DEFAULT as i32,
                NUM_BANDS_MIN as i32,
                NUM_BANDS_MAX as i32,
            )
            .with_description("Number of frequency bands"),
            Parameter::new_int(
                "crossover_preset",
                "Preset",
                CROSSOVER_PRESET_DEFAULT,
                CROSSOVER_PRESET_MIN,
                CROSSOVER_PRESET_MAX,
            )
            .with_description("Crossover preset (0=Custom, 1=200/2k, 2=100/3k, 3=250/4k)"),
            Parameter::new_float(
                "crossover_freq_1",
                "Xover 1",
                CROSSOVER_FREQ_1_DEFAULT,
                CROSSOVER_FREQ_1_MIN,
                CROSSOVER_FREQ_1_MAX,
            )
            .with_description("Low/Mid crossover frequency (Hz)"),
            Parameter::new_float(
                "crossover_freq_2",
                "Xover 2",
                CROSSOVER_FREQ_2_DEFAULT,
                CROSSOVER_FREQ_2_MIN,
                CROSSOVER_FREQ_2_MAX,
            )
            .with_description("Mid/High crossover frequency (Hz)"),
            Parameter::new_float(
                "crossover_freq_3",
                "Xover 3",
                CROSSOVER_FREQ_3_DEFAULT,
                CROSSOVER_FREQ_3_MIN,
                CROSSOVER_FREQ_3_MAX,
            )
            .with_description("High/Ultra crossover frequency (Hz)"),
            Parameter::new_float(
                "crossover_freq_4",
                "Xover 4",
                CROSSOVER_FREQ_4_DEFAULT,
                CROSSOVER_FREQ_4_MIN,
                CROSSOVER_FREQ_4_MAX,
            )
            .with_description("Ultra/Air crossover frequency (Hz)"),
            Parameter::new_float(
                "threshold",
                "Threshold",
                THRESHOLD_DEFAULT,
                THRESHOLD_MIN,
                THRESHOLD_MAX,
            )
            .with_description("Global compression threshold (dB)"),
            Parameter::new_float("ratio", "Ratio", RATIO_DEFAULT, RATIO_MIN, RATIO_MAX)
                .with_description("Global compression ratio"),
            Parameter::new_float("attack", "Attack", ATTACK_DEFAULT, ATTACK_MIN, ATTACK_MAX)
                .with_description("Global attack time (ms)"),
            Parameter::new_float(
                "release",
                "Release",
                RELEASE_DEFAULT,
                RELEASE_MIN,
                RELEASE_MAX,
            )
            .with_description("Global release time (ms)"),
            Parameter::new_float("knee", "Knee", KNEE_DEFAULT, KNEE_MIN, KNEE_MAX)
                .with_description("Global soft knee width (dB)"),
            Parameter::new_bool("link_channels", "Link Channels", LINK_CHANNELS_DEFAULT)
                .with_description("Link stereo detection"),
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_num_bands {
            let new_bands = value
                .as_int()
                .ok_or("Invalid num_bands value")?
                .clamp(NUM_BANDS_MIN as i32, NUM_BANDS_MAX as i32) as usize;
            if new_bands != self.num_bands {
                self.num_bands = new_bands;
                // Resize band structures
                while self.band_params.len() < new_bands {
                    self.band_params.push(BandCompressorParams::default());
                }
                while self.band_compressors.len() < new_bands {
                    self.band_compressors.push(BandCompressor::new(self.channels));
                }
                self.band_levels_db.resize(new_bands, 0.0);
                self.build_crossovers();
                self.update_coefficients();
            }
        } else if id == self.param_crossover_preset {
            self.crossover_preset = value.as_int().ok_or("Invalid preset value")?;
            self.crossover_frequencies =
                Self::get_crossover_frequencies(self.crossover_preset, &self.crossover_frequencies);
            self.build_crossovers();
        } else if id == self.param_crossover_freq_1 {
            self.crossover_frequencies[0] = value.as_float().ok_or("Invalid frequency")?;
            self.crossover_preset = 0; // Switch to custom
            self.build_crossovers();
        } else if id == self.param_crossover_freq_2 {
            self.crossover_frequencies[1] = value.as_float().ok_or("Invalid frequency")?;
            self.crossover_preset = 0;
            self.build_crossovers();
        } else if id == self.param_crossover_freq_3 {
            self.crossover_frequencies[2] = value.as_float().ok_or("Invalid frequency")?;
            self.crossover_preset = 0;
            self.build_crossovers();
        } else if id == self.param_crossover_freq_4 {
            self.crossover_frequencies[3] = value.as_float().ok_or("Invalid frequency")?;
            self.crossover_preset = 0;
            self.build_crossovers();
        } else if id == self.param_threshold {
            self.threshold_db = value.as_float().ok_or("Invalid threshold")?;
        } else if id == self.param_ratio {
            self.ratio = value.as_float().ok_or("Invalid ratio")?.max(1.0);
        } else if id == self.param_attack {
            self.attack_ms = value.as_float().ok_or("Invalid attack")?;
            self.update_coefficients();
        } else if id == self.param_release {
            self.release_ms = value.as_float().ok_or("Invalid release")?;
            self.update_coefficients();
        } else if id == self.param_knee {
            self.knee_db = value.as_float().ok_or("Invalid knee")?.max(0.0);
        } else if id == self.param_link_channels {
            self.link_channels = value.as_bool().ok_or("Invalid link_channels")?;
        } else if id == self.param_mix {
            self.mix = value.as_float().ok_or("Invalid mix")?.clamp(0.0, 1.0);
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_num_bands {
            Some(ParameterValue::Int(self.num_bands as i32))
        } else if id == &self.param_crossover_preset {
            Some(ParameterValue::Int(self.crossover_preset))
        } else if id == &self.param_crossover_freq_1 {
            Some(ParameterValue::Float(self.crossover_frequencies[0]))
        } else if id == &self.param_crossover_freq_2 {
            Some(ParameterValue::Float(self.crossover_frequencies[1]))
        } else if id == &self.param_crossover_freq_3 {
            Some(ParameterValue::Float(self.crossover_frequencies[2]))
        } else if id == &self.param_crossover_freq_4 {
            Some(ParameterValue::Float(self.crossover_frequencies[3]))
        } else if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_ratio {
            Some(ParameterValue::Float(self.ratio))
        } else if id == &self.param_attack {
            Some(ParameterValue::Float(self.attack_ms))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_knee {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_link_channels {
            Some(ParameterValue::Bool(self.link_channels))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.build_crossovers();
        self.update_coefficients();
        Ok(())
    }

    fn reset(&mut self) {
        for crossover in &mut self.crossover_points {
            crossover.reset(self.sample_rate);
        }
        for band in &mut self.band_compressors {
            band.reset();
        }
        for buf in &mut self.band_buffers {
            buf.fill(0.0);
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;
        let dry_mix = 1.0 - self.mix;
        let wet_mix = self.mix;

        // Keep a copy of dry signal if needed
        let dry_signal: Vec<f32> = if dry_mix > 0.0 {
            buffer.to_vec()
        } else {
            Vec::new()
        };

        // Split into bands
        self.split_bands(buffer, num_frames);

        // Process each band
        self.process_bands(num_frames);

        // Sum bands back
        self.sum_bands(buffer, num_frames);

        // Apply dry/wet mix
        if dry_mix > 0.0 {
            for (i, sample) in buffer.iter_mut().enumerate() {
                *sample = dry_mix * dry_signal[i] + wet_mix * *sample;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        0
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let gain_reduction: Vec<Vec<f32>> = self
            .band_compressors
            .iter()
            .map(|b| b.envelope.clone())
            .collect();

        Some(Arc::new(MultibandCompressorData {
            gain_reduction_db: gain_reduction,
            band_levels_db: self.band_levels_db.clone(),
            crossover_frequencies: self.crossover_frequencies.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiband_compressor_creation() {
        let comp = MultibandCompressorPlugin::new(2);
        assert_eq!(comp.channels(), 2);
        assert_eq!(comp.num_bands, NUM_BANDS_DEFAULT);
    }

    #[test]
    fn test_crossover_presets() {
        let freqs = MultibandCompressorPlugin::get_crossover_frequencies(1, &[]);
        assert_eq!(freqs[0], 200.0);
        assert_eq!(freqs[1], 2000.0);

        let freqs = MultibandCompressorPlugin::get_crossover_frequencies(2, &[]);
        assert_eq!(freqs[0], 100.0);
        assert_eq!(freqs[1], 3000.0);
    }

    #[test]
    fn test_gain_reduction_calculation() {
        // Hard knee, 4:1 ratio, -20dB threshold
        let gr = MultibandCompressorPlugin::calculate_gain_reduction(-10.0, -20.0, 4.0, 0.0);
        // 10dB over threshold, slope = 1 - 1/4 = 0.75, GR = 10 * 0.75 = 7.5dB
        assert!((gr - 7.5).abs() < 0.01);

        // Below threshold
        let gr = MultibandCompressorPlugin::calculate_gain_reduction(-30.0, -20.0, 4.0, 0.0);
        assert_eq!(gr, 0.0);
    }

    #[test]
    fn test_multiband_compressor_initialization() {
        let mut comp = MultibandCompressorPlugin::new(2);
        comp.initialize(48000).unwrap();

        assert_eq!(comp.crossover_points.len(), comp.num_bands - 1);
        assert_eq!(comp.band_compressors.len(), comp.num_bands);
    }
}
