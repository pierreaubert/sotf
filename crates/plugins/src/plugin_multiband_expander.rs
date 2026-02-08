// ============================================================================
// Multiband Expander Plugin
// ============================================================================
//
// Dynamic range expander with configurable frequency bands (2-5 bands).
// Uses Linkwitz-Riley 24dB/oct crossover filters for phase-coherent band splitting.
//
// Architecture:
// Input → [LR Crossover Filters] → [Band Expanders] → Sum → Output
//
// Parameters:
// - num_bands: Number of frequency bands (2-5)
// - crossover_preset: Preset frequencies (0=Custom, 1=200/2k, 2=100/3k, 3=250/4k)
// - crossover_freq_1-4: Custom crossover frequencies
// - Global: threshold, ratio, attack, release, knee, range, hysteresis, hold
// - Per-band overrides with solo/bypass

use super::param_specs::multiband_expander::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
use super::smoothing::Smoother;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

/// Crossover presets (same as multiband compressor)
pub const CROSSOVER_PRESETS: &[(f32, f32, f32, f32)] = &[
    (200.0, 2000.0, 8000.0, 12000.0),  // Preset 1: Classic 200/2k
    (100.0, 3000.0, 8000.0, 12000.0),  // Preset 2: Wide mid 100/3k
    (250.0, 4000.0, 10000.0, 14000.0), // Preset 3: Hi emphasis 250/4k
];

/// Per-band expander parameters (optional overrides)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct BandExpanderParams {
    #[serde(default)]
    pub threshold_db: Option<f32>,
    #[serde(default)]
    pub ratio: Option<f32>,
    #[serde(default)]
    pub attack_ms: Option<f32>,
    #[serde(default)]
    pub release_ms: Option<f32>,
    #[serde(default)]
    pub knee_db: Option<f32>,
    #[serde(default)]
    pub range_db: Option<f32>,
    #[serde(default)]
    pub hysteresis_db: Option<f32>,
    #[serde(default)]
    pub hold_ms: Option<f32>,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub bypass: bool,
}


/// Configuration parameters for MultibandExpanderPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultibandExpanderPluginParams {
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,
    #[serde(default = "default_crossover_preset")]
    pub crossover_preset: i32,
    #[serde(default = "default_crossover_frequencies")]
    pub crossover_frequencies: Vec<f32>,
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    #[serde(default = "default_range_db")]
    pub range_db: f32,
    #[serde(default = "default_hysteresis_db")]
    pub hysteresis_db: f32,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default)]
    pub bands: Vec<BandExpanderParams>,
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
fn default_range_db() -> f32 {
    RANGE_DEFAULT
}
fn default_hysteresis_db() -> f32 {
    HYSTERESIS_DEFAULT
}
fn default_hold_ms() -> f32 {
    HOLD_DEFAULT
}
fn default_link_channels() -> bool {
    LINK_CHANNELS_DEFAULT
}
fn default_mix() -> f32 {
    MIX_DEFAULT
}

impl Default for MultibandExpanderPluginParams {
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
            range_db: default_range_db(),
            hysteresis_db: default_hysteresis_db(),
            hold_ms: default_hold_ms(),
            link_channels: default_link_channels(),
            mix: default_mix(),
            bands: Vec::new(),
        }
    }
}

/// Data exposed by the multiband expander for monitoring
#[derive(Debug, Clone)]
pub struct MultibandExpanderData {
    /// Attenuation per band per channel [band][channel]
    pub attenuation_db: Vec<Vec<f32>>,
    /// Gate state per band (true = open)
    pub is_open: Vec<bool>,
    /// RMS level per band (dB)
    pub band_levels_db: Vec<f32>,
    /// Crossover frequencies currently in use
    pub crossover_frequencies: Vec<f32>,
}

// ============================================================================
// Crossover Filter Bank
// ============================================================================

struct CrossoverPoint {
    lowpass: Vec<Vec<Biquad>>,
    highpass: Vec<Vec<Biquad>>,
    frequency: f32,
}

impl CrossoverPoint {
    fn new(channels: usize, frequency: f32, sample_rate: u32) -> Self {
        let q = 1.0 / std::f64::consts::SQRT_2;

        let mut lowpass = Vec::with_capacity(channels);
        let mut highpass = Vec::with_capacity(channels);

        for _ in 0..channels {
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
// Band Expander State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    Open,
    Hold,
    Closing,
}

struct BandExpander {
    envelope: Vec<f32>,
    gate_state: Vec<GateState>,
    hold_counter: Vec<usize>,
    attack_coeff: f32,
    release_coeff: f32,
}

impl BandExpander {
    fn new(channels: usize) -> Self {
        Self {
            envelope: vec![0.0; channels],
            gate_state: vec![GateState::Open; channels],
            hold_counter: vec![0; channels],
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
        self.gate_state.fill(GateState::Open);
        self.hold_counter.fill(0);
    }

    fn is_open(&self) -> bool {
        self.gate_state
            .iter()
            .any(|&s| s == GateState::Open || s == GateState::Hold)
    }
}

// ============================================================================
// Plugin Implementation
// ============================================================================

pub struct MultibandExpanderPlugin {
    channels: usize,
    sample_rate: u32,

    num_bands: usize,
    crossover_preset: i32,
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

    band_params: Vec<BandExpanderParams>,
    crossover_points: Vec<CrossoverPoint>,
    band_expanders: Vec<BandExpander>,

    // Flat buffer for better cache locality [band * stride + index]
    band_buffers: Vec<f32>,
    band_levels_db: Vec<f32>,

    // Pre-allocated dry signal buffer (avoids allocation in process_in_place hot path)
    dry_buffer: Vec<f32>,

    // Smoothing
    threshold_smoother: Smoother,

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
    param_range: ParameterId,
    param_hysteresis: ParameterId,
    param_hold: ParameterId,
    param_link_channels: ParameterId,
    param_mix: ParameterId,
}

impl MultibandExpanderPlugin {
    pub fn new(channels: usize) -> Self {
        Self::with_params(channels, MultibandExpanderPluginParams::default())
    }

    pub fn with_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
        let num_bands = params.num_bands.clamp(NUM_BANDS_MIN, NUM_BANDS_MAX);
        let sample_rate = 44100;

        let mut band_params = params.bands.clone();
        while band_params.len() < num_bands {
            band_params.push(BandExpanderParams::default());
        }

        let band_expanders: Vec<_> = (0..num_bands)
            .map(|_| BandExpander::new(channels))
            .collect();

        let crossover_frequencies =
            Self::get_crossover_frequencies(params.crossover_preset, &params.crossover_frequencies);

        // Initialize smoother (50ms smoothing time)
        let threshold_smoother = Smoother::new(params.threshold_db, 50.0, sample_rate);

        Self {
            channels,
            sample_rate,
            num_bands,
            crossover_preset: params.crossover_preset,
            crossover_frequencies,
            threshold_db: params.threshold_db,
            ratio: params.ratio,
            attack_ms: params.attack_ms,
            release_ms: params.release_ms,
            knee_db: params.knee_db,
            range_db: params.range_db,
            hysteresis_db: params.hysteresis_db,
            hold_ms: params.hold_ms,
            link_channels: params.link_channels,
            mix: params.mix.clamp(0.0, 1.0),
            band_params,
            crossover_points: Vec::new(),
            band_expanders,
            band_buffers: Vec::new(),
            band_levels_db: vec![0.0; num_bands],
            dry_buffer: Vec::new(), // Sized lazily in process_in_place()
            threshold_smoother,
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
            param_range: ParameterId::from("range"),
            param_hysteresis: ParameterId::from("hysteresis"),
            param_hold: ParameterId::from("hold"),
            param_link_channels: ParameterId::from("link_channels"),
            param_mix: ParameterId::from("mix"),
        }
    }

    pub fn from_params(channels: usize, params: MultibandExpanderPluginParams) -> Self {
        Self::with_params(channels, params)
    }

    fn get_crossover_frequencies(preset: i32, custom: &[f32]) -> Vec<f32> {
        if preset > 0 && preset <= CROSSOVER_PRESETS.len() as i32 {
            let (f1, f2, f3, f4) = CROSSOVER_PRESETS[(preset - 1) as usize];
            vec![f1, f2, f3, f4]
        } else {
            let mut freqs = custom.to_vec();
            while freqs.len() < 4 {
                freqs.push(default_crossover_frequencies()[freqs.len()]);
            }
            freqs
        }
    }

    fn build_crossovers(&mut self) {
        self.crossover_points.clear();
        let num_crossovers = self.num_bands - 1;
        for i in 0..num_crossovers {
            let freq = self.crossover_frequencies.get(i).copied().unwrap_or(1000.0);
            self.crossover_points
                .push(CrossoverPoint::new(self.channels, freq, self.sample_rate));
        }
    }

    fn update_coefficients(&mut self) {
        for (i, band) in self.band_expanders.iter_mut().enumerate() {
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

    fn get_band_ratio(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.ratio)
            .unwrap_or(self.ratio)
    }

    fn get_band_knee(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.knee_db)
            .unwrap_or(self.knee_db)
    }

    fn get_band_range(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.range_db)
            .unwrap_or(self.range_db)
    }

    fn get_band_hysteresis(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.hysteresis_db)
            .unwrap_or(self.hysteresis_db)
    }

    fn get_band_hold(&self, band: usize) -> f32 {
        self.band_params
            .get(band)
            .and_then(|p| p.hold_ms)
            .unwrap_or(self.hold_ms)
    }

    fn is_band_solo(&self, band: usize) -> bool {
        self.band_params.get(band).map(|p| p.solo).unwrap_or(false)
    }

    fn is_band_bypass(&self, band: usize) -> bool {
        self.band_params
            .get(band)
            .map(|p| p.bypass)
            .unwrap_or(false)
    }

    fn hold_samples(&self, hold_ms: f32) -> usize {
        (hold_ms * 0.001 * self.sample_rate as f32) as usize
    }

    /// Calculate expansion attenuation with soft knee and range limit
    fn calculate_expansion_attenuation(
        input_db: f32,
        threshold: f32,
        ratio: f32,
        knee: f32,
        range: f32,
    ) -> f32 {
        let slope = 1.0 - 1.0 / ratio.max(1.0);

        let attenuation = if knee < 0.1 {
            if input_db >= threshold {
                0.0
            } else {
                (threshold - input_db) * slope
            }
        } else if input_db > threshold + knee / 2.0 {
            0.0
        } else if input_db < threshold - knee / 2.0 {
            (threshold - input_db) * slope
        } else {
            let below = threshold + knee / 2.0 - input_db;
            let knee_factor = below / knee;
            knee_factor * knee_factor * knee / 2.0 * slope
        };

        attenuation.min(range)
    }

    fn split_bands(&mut self, input: &[f32], num_frames: usize) {
        let required_len = self.num_bands * num_frames * self.channels;

        // Ensure band buffers are allocated
        if self.band_buffers.len() < required_len {
            self.band_buffers.resize(required_len, 0.0);
        }

        // Reset buffers
        self.band_buffers[0..required_len].fill(0.0);

        let stride = num_frames * self.channels;

        for frame in 0..num_frames {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut remaining = input[idx];

                for (xover_idx, crossover) in self.crossover_points.iter_mut().enumerate() {
                    let low = crossover.process_lowpass(ch, remaining);

                    let band_idx = xover_idx * stride + idx;
                    self.band_buffers[band_idx] += low;

                    remaining = crossover.process_highpass(ch, remaining);
                }

                let high_band_idx = (self.num_bands - 1) * stride + idx;
                self.band_buffers[high_band_idx] = remaining;
            }
        }
    }

    fn process_bands(&mut self, num_frames: usize) {
        let any_solo = (0..self.num_bands).any(|b| self.is_band_solo(b));
        let stride = num_frames * self.channels;

        let smoothed_threshold = self.threshold_smoother.next();

        for band in 0..self.num_bands {
            let bypass = self.is_band_bypass(band);
            let solo = self.is_band_solo(band);
            let muted = any_solo && !solo;

            let band_offset = band * stride;

            if muted {
                let buf_slice = &mut self.band_buffers[band_offset..band_offset + stride];
                buf_slice.fill(0.0);
                continue;
            }

            if bypass {
                continue;
            }

            let threshold = if let Some(p) = self.band_params.get(band).and_then(|b| b.threshold_db)
            {
                p
            } else {
                smoothed_threshold
            };

            let ratio = self.get_band_ratio(band);
            let knee = self.get_band_knee(band);
            let range = self.get_band_range(band);
            let hysteresis = self.get_band_hysteresis(band);
            let hold_samples = self.hold_samples(self.get_band_hold(band));

            let open_threshold = threshold;
            let close_threshold = threshold - hysteresis;

            let band_exp = &mut self.band_expanders[band];

            if self.link_channels && self.channels > 1 {
                for frame in 0..num_frames {
                    let mut max_level = 0.0_f32;
                    for ch in 0..self.channels {
                        let idx = frame * self.channels + ch;
                        let sample = self.band_buffers[band_offset + idx];
                        max_level = max_level.max(sample.abs());
                    }

                    let input_db = 20.0 * max_level.max(1e-10).log10();

                    // Process hysteresis for channel 0
                    let target_atten = Self::process_hysteresis(
                        band_exp,
                        0,
                        input_db,
                        open_threshold,
                        close_threshold,
                        hold_samples,
                        threshold,
                        ratio,
                        knee,
                        range,
                    );

                    // Copy state to all channels
                    for ch in 1..self.channels {
                        band_exp.envelope[ch] = band_exp.envelope[0];
                        band_exp.gate_state[ch] = band_exp.gate_state[0];
                        band_exp.hold_counter[ch] = band_exp.hold_counter[0];
                    }

                    let gain = 10.0_f32.powf(-target_atten / 20.0);
                    for ch in 0..self.channels {
                        let idx = frame * self.channels + ch;
                        self.band_buffers[band_offset + idx] *= gain;
                    }
                }
            } else {
                for frame in 0..num_frames {
                    for ch in 0..self.channels {
                        let idx = frame * self.channels + ch;
                        let sample = self.band_buffers[band_offset + idx];
                        let input_db = 20.0 * sample.abs().max(1e-10).log10();

                        let atten = Self::process_hysteresis(
                            band_exp,
                            ch,
                            input_db,
                            open_threshold,
                            close_threshold,
                            hold_samples,
                            threshold,
                            ratio,
                            knee,
                            range,
                        );

                        let gain = 10.0_f32.powf(-atten / 20.0);
                        self.band_buffers[band_offset + idx] = sample * gain;
                    }
                }
            }

            // Calculate RMS for monitoring
            let mut sum_sq = 0.0_f32;
            let buf_slice = &self.band_buffers[band_offset..band_offset + stride];
            for &s in buf_slice {
                sum_sq += s * s;
            }
            let rms = (sum_sq / buf_slice.len() as f32).sqrt();
            self.band_levels_db[band] = 20.0 * rms.max(1e-10).log10();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_hysteresis(
        band_exp: &mut BandExpander,
        ch: usize,
        input_db: f32,
        open_threshold: f32,
        close_threshold: f32,
        hold_samples: usize,
        threshold: f32,
        ratio: f32,
        knee: f32,
        range: f32,
    ) -> f32 {
        let target_attenuation = match band_exp.gate_state[ch] {
            GateState::Open => {
                if input_db < open_threshold {
                    band_exp.gate_state[ch] = GateState::Hold;
                    band_exp.hold_counter[ch] = hold_samples;
                    0.0
                } else {
                    0.0
                }
            }
            GateState::Hold => {
                if input_db >= open_threshold {
                    band_exp.gate_state[ch] = GateState::Open;
                    band_exp.hold_counter[ch] = 0;
                    0.0
                } else if band_exp.hold_counter[ch] > 0 {
                    band_exp.hold_counter[ch] -= 1;
                    0.0
                } else if input_db < close_threshold {
                    band_exp.gate_state[ch] = GateState::Closing;
                    Self::calculate_expansion_attenuation(
                        input_db, threshold, ratio, knee, range,
                    )
                } else {
                    0.0
                }
            }
            GateState::Closing => {
                if input_db >= open_threshold {
                    band_exp.gate_state[ch] = GateState::Open;
                    band_exp.hold_counter[ch] = 0;
                    0.0
                } else {
                    Self::calculate_expansion_attenuation(input_db, threshold, ratio, knee, range)
                }
            }
        };

        let coeff = if target_attenuation > band_exp.envelope[ch] {
            band_exp.release_coeff
        } else {
            band_exp.attack_coeff
        };

        band_exp.envelope[ch] =
            target_attenuation + coeff * (band_exp.envelope[ch] - target_attenuation);

        band_exp.envelope[ch]
    }

    fn sum_bands(&self, output: &mut [f32], num_frames: usize) {
        let stride = num_frames * self.channels;

        for frame in 0..num_frames {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let mut sum = 0.0_f32;
                for band in 0..self.num_bands {
                    let band_offset = band * stride;
                    sum += self.band_buffers[band_offset + idx];
                }
                output[idx] = sum;
            }
        }
    }
}

impl InPlacePlugin for MultibandExpanderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Multiband Expander", "1.0.0", "SotF").with_description(format!(
            "{}-band expander with LR24 crossovers and hysteresis",
            self.num_bands
        ))
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
            .with_description("Number of frequency bands")
            .with_group("Configuration")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_int(
                "crossover_preset",
                "Preset",
                CROSSOVER_PRESET_DEFAULT,
                CROSSOVER_PRESET_MIN,
                CROSSOVER_PRESET_MAX,
            )
            .with_description("Crossover preset")
            .with_group("Crossover")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "crossover_freq_1",
                "Xover 1",
                CROSSOVER_FREQ_1_DEFAULT,
                CROSSOVER_FREQ_1_MIN,
                CROSSOVER_FREQ_1_MAX,
            )
            .with_description("Low/Mid crossover (Hz)")
            .with_group("Crossover")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "crossover_freq_2",
                "Xover 2",
                CROSSOVER_FREQ_2_DEFAULT,
                CROSSOVER_FREQ_2_MIN,
                CROSSOVER_FREQ_2_MAX,
            )
            .with_description("Mid/High crossover (Hz)")
            .with_group("Crossover")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "crossover_freq_3",
                "Xover 3",
                CROSSOVER_FREQ_3_DEFAULT,
                CROSSOVER_FREQ_3_MIN,
                CROSSOVER_FREQ_3_MAX,
            )
            .with_description("High/Ultra crossover (Hz)")
            .with_group("Crossover")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "crossover_freq_4",
                "Xover 4",
                CROSSOVER_FREQ_4_DEFAULT,
                CROSSOVER_FREQ_4_MIN,
                CROSSOVER_FREQ_4_MAX,
            )
            .with_description("Ultra/Air crossover (Hz)")
            .with_group("Crossover")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "threshold",
                "Threshold",
                THRESHOLD_DEFAULT,
                THRESHOLD_MIN,
                THRESHOLD_MAX,
            )
            .with_description("Global expansion threshold (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("ratio", "Ratio", RATIO_DEFAULT, RATIO_MIN, RATIO_MAX)
                .with_description("Global expansion ratio")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("attack", "Attack", ATTACK_DEFAULT, ATTACK_MIN, ATTACK_MAX)
                .with_description("Global attack time (ms)")
                .with_group("Timing")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                RELEASE_DEFAULT,
                RELEASE_MIN,
                RELEASE_MAX,
            )
            .with_description("Global release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float("knee", "Knee", KNEE_DEFAULT, KNEE_MIN, KNEE_MAX)
                .with_description("Global soft knee (dB)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("range", "Range", RANGE_DEFAULT, RANGE_MIN, RANGE_MAX)
                .with_description("Maximum attenuation (dB)")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "hysteresis",
                "Hysteresis",
                HYSTERESIS_DEFAULT,
                HYSTERESIS_MIN,
                HYSTERESIS_MAX,
            )
            .with_description("Hysteresis range (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("hold", "Hold", HOLD_DEFAULT, HOLD_MIN, HOLD_MAX)
                .with_description("Hold time (ms)")
                .with_group("Timing")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("link_channels", "Link Channels", LINK_CHANNELS_DEFAULT)
                .with_description("Link stereo detection")
                .with_group("Channels")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_num_bands {
            let new_bands = value
                .as_int()
                .ok_or("Invalid num_bands")?
                .clamp(NUM_BANDS_MIN as i32, NUM_BANDS_MAX as i32)
                as usize;
            if new_bands != self.num_bands {
                self.num_bands = new_bands;
                while self.band_params.len() < new_bands {
                    self.band_params.push(BandExpanderParams::default());
                }
                while self.band_expanders.len() < new_bands {
                    self.band_expanders.push(BandExpander::new(self.channels));
                }
                self.band_levels_db.resize(new_bands, 0.0);
                self.build_crossovers();
                self.update_coefficients();
            }
        } else if id == self.param_crossover_preset {
            self.crossover_preset = value.as_int().ok_or("Invalid preset")?;
            self.crossover_frequencies =
                Self::get_crossover_frequencies(self.crossover_preset, &self.crossover_frequencies);
            self.build_crossovers();
        } else if id == self.param_crossover_freq_1 {
            self.crossover_frequencies[0] = value.as_float().ok_or("Invalid frequency")?;
            self.crossover_preset = 0;
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
        } else if id.as_str().starts_with("band_") {
            let parts: Vec<&str> = id.as_str().split('_').collect();
            if parts.len() >= 3
                && let Ok(band_idx) = parts[1].parse::<usize>()
                    && band_idx < self.band_params.len() {
                        let param_name = parts[2..].join("_");
                        let band = &mut self.band_params[band_idx];

                        match param_name.as_str() {
                            "threshold" => {
                                band.threshold_db =
                                    Some(value.as_float().ok_or("Invalid threshold")?);
                            }
                            "ratio" => {
                                band.ratio =
                                    Some(value.as_float().ok_or("Invalid ratio")?.max(1.0));
                            }
                            "attack" => {
                                band.attack_ms = Some(value.as_float().ok_or("Invalid attack")?);
                                self.update_coefficients();
                            }
                            "release" => {
                                band.release_ms = Some(value.as_float().ok_or("Invalid release")?);
                                self.update_coefficients();
                            }
                            "knee" => {
                                band.knee_db =
                                    Some(value.as_float().ok_or("Invalid knee")?.max(0.0));
                            }
                            "range" => {
                                band.range_db =
                                    Some(value.as_float().ok_or("Invalid range")?.max(0.0));
                            }
                            "hysteresis" => {
                                band.hysteresis_db =
                                    Some(value.as_float().ok_or("Invalid hysteresis")?.max(0.0));
                            }
                            "hold" => {
                                band.hold_ms =
                                    Some(value.as_float().ok_or("Invalid hold")?.max(0.0));
                            }
                            "bypass" => {
                                band.bypass = value.as_bool().ok_or("Invalid bypass")?;
                            }
                            "solo" => {
                                band.solo = value.as_bool().ok_or("Invalid solo")?;
                            }
                            _ => return Err(format!("Unknown band parameter: {}", param_name)),
                        }
                        return Ok(());
                    }
            return Err(format!("Invalid band parameter ID: {}", id));
        } else if id == self.param_threshold {
            let val = value.as_float().ok_or("Invalid threshold")?;
            self.threshold_db = val;
            self.threshold_smoother.set_target(val);
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
        } else if id == self.param_range {
            self.range_db = value.as_float().ok_or("Invalid range")?.max(0.0);
        } else if id == self.param_hysteresis {
            self.hysteresis_db = value.as_float().ok_or("Invalid hysteresis")?.max(0.0);
        } else if id == self.param_hold {
            self.hold_ms = value.as_float().ok_or("Invalid hold")?.max(0.0);
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
        if id.as_str().starts_with("band_") {
            let parts: Vec<&str> = id.as_str().split('_').collect();
            if parts.len() >= 3
                && let Ok(band_idx) = parts[1].parse::<usize>()
                    && band_idx < self.band_params.len() {
                        let param_name = parts[2..].join("_");
                        let band = &self.band_params[band_idx];

                        return match param_name.as_str() {
                            "threshold" => Some(ParameterValue::Float(
                                band.threshold_db.unwrap_or(self.threshold_db),
                            )),
                            "ratio" => {
                                Some(ParameterValue::Float(band.ratio.unwrap_or(self.ratio)))
                            }
                            "attack" => Some(ParameterValue::Float(
                                band.attack_ms.unwrap_or(self.attack_ms),
                            )),
                            "release" => Some(ParameterValue::Float(
                                band.release_ms.unwrap_or(self.release_ms),
                            )),
                            "knee" => {
                                Some(ParameterValue::Float(band.knee_db.unwrap_or(self.knee_db)))
                            }
                            "range" => Some(ParameterValue::Float(
                                band.range_db.unwrap_or(self.range_db),
                            )),
                            "hysteresis" => Some(ParameterValue::Float(
                                band.hysteresis_db.unwrap_or(self.hysteresis_db),
                            )),
                            "hold" => {
                                Some(ParameterValue::Float(band.hold_ms.unwrap_or(self.hold_ms)))
                            }
                            "bypass" => Some(ParameterValue::Bool(band.bypass)),
                            "solo" => Some(ParameterValue::Bool(band.solo)),
                            _ => None,
                        };
                    }
            return None;
        }

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
        } else if id == &self.param_range {
            Some(ParameterValue::Float(self.range_db))
        } else if id == &self.param_hysteresis {
            Some(ParameterValue::Float(self.hysteresis_db))
        } else if id == &self.param_hold {
            Some(ParameterValue::Float(self.hold_ms))
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
        self.threshold_smoother.set_time(50.0, sample_rate);
        self.build_crossovers();
        self.update_coefficients();
        Ok(())
    }

    fn reset(&mut self) {
        for crossover in &mut self.crossover_points {
            crossover.reset(self.sample_rate);
        }
        for band in &mut self.band_expanders {
            band.reset();
        }
        self.band_buffers.fill(0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let num_frames = context.num_frames;
        let dry_mix = 1.0 - self.mix;
        let wet_mix = self.mix;

        // Keep a copy of dry signal if needed (pre-allocated buffer, no allocation)
        if dry_mix > 0.0 {
            if self.dry_buffer.len() < buffer.len() {
                self.dry_buffer.resize(buffer.len(), 0.0);
            }
            self.dry_buffer[..buffer.len()].copy_from_slice(buffer);
        }

        self.split_bands(buffer, num_frames);
        self.process_bands(num_frames);
        self.sum_bands(buffer, num_frames);

        if dry_mix > 0.0 {
            for (i, sample) in buffer.iter_mut().enumerate() {
                *sample = dry_mix * self.dry_buffer[i] + wet_mix * *sample;
            }
        }

        // Flush denormals to prevent CPU performance spikes and audio crackle
        // Multiband expander envelope calculations can produce denormal numbers
        flush_denormals_inplace(buffer);

        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let attenuation: Vec<Vec<f32>> = self
            .band_expanders
            .iter()
            .map(|b| b.envelope.clone())
            .collect();

        let is_open: Vec<bool> = self.band_expanders.iter().map(|b| b.is_open()).collect();

        Some(Arc::new(MultibandExpanderData {
            attenuation_db: attenuation,
            is_open,
            band_levels_db: self.band_levels_db.clone(),
            crossover_frequencies: self.crossover_frequencies.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiband_expander_creation() {
        let exp = MultibandExpanderPlugin::new(2);
        assert_eq!(exp.channels(), 2);
        assert_eq!(exp.num_bands, NUM_BANDS_DEFAULT);
    }

    #[test]
    fn test_expansion_attenuation_calculation() {
        // Hard knee, 2:1 ratio, -40dB threshold, 60dB range
        let atten =
            MultibandExpanderPlugin::calculate_expansion_attenuation(-50.0, -40.0, 2.0, 0.0, 60.0);
        // 10dB below threshold, slope = 1 - 1/2 = 0.5, atten = 10 * 0.5 = 5dB
        assert!((atten - 5.0).abs() < 0.01);

        // Above threshold
        let atten =
            MultibandExpanderPlugin::calculate_expansion_attenuation(-30.0, -40.0, 2.0, 0.0, 60.0);
        assert_eq!(atten, 0.0);

        // Range limited
        let atten = MultibandExpanderPlugin::calculate_expansion_attenuation(
            -100.0, -40.0, 10.0, 0.0, 20.0,
        );
        // Would be 60 * 0.9 = 54dB but capped at 20dB
        assert!((atten - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_multiband_expander_initialization() {
        let mut exp = MultibandExpanderPlugin::new(2);
        exp.initialize(48000).unwrap();

        assert_eq!(exp.crossover_points.len(), exp.num_bands - 1);
        assert_eq!(exp.band_expanders.len(), exp.num_bands);
    }
}
