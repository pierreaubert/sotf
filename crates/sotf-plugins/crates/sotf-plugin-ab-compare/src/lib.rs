//! ============================================================================
//! A/B Comparison Plugin
//! ============================================================================
//!
//! This plugin allows fair comparison between two audio processing chains
//! with automatic loudness matching. Each path (A or B) can be:
//! - A single plugin
//! - A rack (linear chain of plugins)
//! - A graph (full DAG topology)

mod config;
mod factory;
#[cfg(test)]
mod tests;

pub use config::*;
use factory::build_path_from_config;

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainLoudnessType, AutoGainParams};
use sotf_host::host::DawHost;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Delay Line for Latency Compensation
// ============================================================================

/// Minimal fixed-delay ring buffer for aligning two processing paths.
struct DelayLine {
    buffer: Vec<f32>,
    pos: usize,
    len: usize,
}

impl DelayLine {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            pos: 0,
            len: 0,
        }
    }

    /// Set delay in frames (interleaved samples = frames * channels).
    /// Allocates only when size changes.
    fn set_delay(&mut self, frames: usize, channels: usize) {
        let new_len = frames * channels;
        if new_len != self.len {
            self.buffer.resize(new_len, 0.0);
            self.buffer.fill(0.0);
            self.pos = 0;
            self.len = new_len;
        }
    }

    /// Swap each sample in `data` with the delayed version. No-op when len == 0.
    #[inline]
    fn process(&mut self, data: &mut [f32]) {
        if self.len == 0 {
            return;
        }
        for sample in data.iter_mut() {
            std::mem::swap(&mut self.buffer[self.pos], sample);
            self.pos += 1;
            if self.pos >= self.len {
                self.pos = 0;
            }
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }
}

// ============================================================================
// Exposed Data Structure
// ============================================================================

/// Data exposed by the A/B comparison plugin for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ABCompareData {
    pub loudness_a_lufs: f64,
    pub loudness_b_lufs: f64,
    pub auto_gain_db: f32,
    pub peak_a: f64,
    pub peak_b: f64,
    pub current_mix: f32,
    pub bypass_active: bool,
}

// ============================================================================
// Main Plugin Struct
// ============================================================================

/// A/B Comparison Plugin
///
/// Allows fair comparison between two audio processing chains with automatic
/// loudness matching. Each path (A or B) can be a single plugin, a rack
/// (linear chain), or a full graph.
pub struct ABComparePlugin {
    // Configuration
    num_channels: usize,
    sample_rate: u32,

    // Processing paths - use DawHost for flexibility
    host_a: DawHost,
    host_b: DawHost,

    // Path configurations (stored for runtime changes)
    path_a_config: PathConfig,
    path_b_config: PathConfig,

    // Auto-gain for matching B to A's loudness
    // Uses A's output as "input reference" and B's output as "output to compensate"
    // Also provides loudness and peak data for both paths
    auto_gain: AutoGain,

    // State
    mix_mode: MixMode,
    mix: f32,
    mix_smoother: Smoother,
    selected_path: i32,
    bypass: bool,
    mix_transition_ms: f32,

    // Phase inversion
    phase_invert_a: bool,
    phase_invert_b: bool,

    // Difference mode (A - B)
    difference_mode: bool,

    // Latency compensation delay lines
    delay_a: DelayLine,
    delay_b: DelayLine,

    // Internal buffers
    buffer_a: Vec<f32>,
    buffer_b: Vec<f32>,

    // Band mask (bandpass filter for isolating frequency range in comparison)
    band_mask_low_hz: f32,
    band_mask_high_hz: f32,
    /// Per-channel highpass filters (one per channel) for band mask low cutoff
    band_mask_hp: Vec<Biquad>,
    /// Per-channel lowpass filters (one per channel) for band mask high cutoff
    band_mask_lp: Vec<Biquad>,

    // Cached peak values
    last_peak_a: f64,
    last_peak_b: f64,

    cache: RealTimeCache<ABCompareData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
}

impl ABComparePlugin {
    /// Create a new A/B Compare plugin with default settings
    pub fn new(num_channels: usize) -> Result<Self, String> {
        Self::from_params(num_channels, ABComparePluginParams::default())
    }

    /// Create from parameters
    pub fn from_params(num_channels: usize, params: ABComparePluginParams) -> Result<Self, String> {
        let sample_rate = 48000; // Will be updated in initialize()

        let host_a = build_path_from_config(&params.path_a, num_channels, sample_rate)?;
        let host_b = build_path_from_config(&params.path_b, num_channels, sample_rate)?;

        // Create AutoGain for matching B's loudness to A's loudness
        // A's output is the "input reference", B's output is "what to compensate"
        let auto_gain_params = AutoGainParams {
            enabled: params.auto_gain_enabled,
            loudness_type: params.loudness_type,
            max_gain_db: params.max_auto_gain_db,
            smoothing_ms: params.gain_smoothing_ms,
        };
        let auto_gain = AutoGain::new(num_channels, sample_rate, auto_gain_params)?;

        let mix_smoother = Smoother::new(params.mix, params.mix_transition_ms, sample_rate);

        let band_mask_low_hz = params.band_mask_low_hz.clamp(20.0, 20000.0);
        let band_mask_high_hz = params.band_mask_high_hz.clamp(20.0, 20000.0);
        let q = 1.0 / std::f64::consts::SQRT_2;
        let band_mask_hp: Vec<Biquad> = (0..num_channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Highpass,
                    band_mask_low_hz as f64,
                    sample_rate as f64,
                    q,
                    0.0,
                )
            })
            .collect();
        let band_mask_lp: Vec<Biquad> = (0..num_channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Lowpass,
                    band_mask_high_hz as f64,
                    sample_rate as f64,
                    q,
                    0.0,
                )
            })
            .collect();

        let mut p = Self {
            num_channels,
            sample_rate,
            host_a,
            host_b,
            path_a_config: params.path_a,
            path_b_config: params.path_b,
            auto_gain,
            mix_mode: params.mix_mode,
            mix: params.mix,
            mix_smoother,
            selected_path: params.selected_path,
            bypass: params.bypass,
            mix_transition_ms: params.mix_transition_ms,
            phase_invert_a: params.phase_invert_a,
            phase_invert_b: params.phase_invert_b,
            difference_mode: params.difference_mode,
            band_mask_low_hz,
            band_mask_high_hz,
            band_mask_hp,
            band_mask_lp,
            delay_a: DelayLine::new(),
            delay_b: DelayLine::new(),
            buffer_a: Vec::new(),
            buffer_b: Vec::new(),
            last_peak_a: 0.0,
            last_peak_b: 0.0,
            cache: RealTimeCache::new(ABCompareData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float("mix", "A/B Mix", self.mix, -1.0, 1.0)
                .with_description("Mix between A and B: -1.0 = A, 0.0 = 50/50, +1.0 = B")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_int(
                "mix_mode",
                "Mix Mode",
                match self.mix_mode {
                    MixMode::Potentiometer => 0,
                    MixMode::Binary => 1,
                },
                0,
                1,
            )
            .with_description("0 = Potentiometer (continuous), 1 = Binary (A/B switch)")
            .with_group("Mix Control")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_int("selected_path", "Selected Path", self.selected_path, 0, 1)
                .with_description("0 = A, 1 = B (only used in binary mode)")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("bypass", "Bypass", self.bypass)
                .with_description("Bypass A/B processing, output original input")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_bool(
                "auto_gain_enabled",
                "Auto Gain",
                self.auto_gain.is_enabled(),
            )
            .with_description("Automatically match loudness between A and B")
            .with_group("Loudness Matching")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_int(
                "loudness_type",
                "Loudness Type",
                match self.auto_gain.loudness_type() {
                    AutoGainLoudnessType::Momentary => 0,
                    AutoGainLoudnessType::ShortTerm => 1,
                },
                0,
                1,
            )
            .with_description("0 = Momentary (400ms), 1 = Short-term (3s)")
            .with_group("Loudness Matching")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "max_auto_gain_db",
                "Max Auto Gain",
                self.auto_gain.max_gain_db(),
                0.0,
                24.0,
            )
            .with_description("Maximum loudness correction in dB")
            .with_group("Loudness Matching")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "gain_smoothing_ms",
                "Gain Smoothing",
                self.auto_gain.smoothing_ms(),
                10.0,
                500.0,
            )
            .with_description("Auto-gain smoothing time in milliseconds")
            .with_group("Loudness Matching")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "mix_transition_ms",
                "Mix Transition",
                self.mix_transition_ms,
                5.0,
                500.0,
            )
            .with_description("A/B transition smoothing time in milliseconds")
            .with_group("Timing")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_bool("phase_invert_a", "Phase Invert A", self.phase_invert_a)
                .with_description("Invert phase of path A output (multiply by -1.0)")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("phase_invert_b", "Phase Invert B", self.phase_invert_b)
                .with_description("Invert phase of path B output (multiply by -1.0)")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("difference_mode", "Difference Mode", self.difference_mode)
                .with_description("Output A - B instead of crossfade mix")
                .with_group("Mix Control")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "band_mask_low_hz",
                "Band Mask Low",
                self.band_mask_low_hz,
                20.0,
                20000.0,
            )
            .with_description("Highpass cutoff for band-masking the comparison output (Hz)")
            .with_group("Band Mask")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "band_mask_high_hz",
                "Band Mask High",
                self.band_mask_high_hz,
                20.0,
                20000.0,
            )
            .with_description("Lowpass cutoff for band-masking the comparison output (Hz)")
            .with_group("Band Mask")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_string(
                "path_a_config",
                "Path A Config",
                serde_json::to_string(&self.path_a_config)
                    .unwrap_or_else(|_| r#"{"type":"None"}"#.to_string()),
            )
            .with_description("JSON configuration for path A")
            .with_group("Configuration")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_string(
                "path_b_config",
                "Path B Config",
                serde_json::to_string(&self.path_b_config)
                    .unwrap_or_else(|_| r#"{"type":"None"}"#.to_string()),
            )
            .with_description("JSON configuration for path B")
            .with_group("Configuration")
            .with_importance(ParameterImportance::Critical),
        ];
    }

    /// Rebuild path A from config
    fn rebuild_path_a(&mut self) -> Result<(), String> {
        self.host_a =
            build_path_from_config(&self.path_a_config, self.num_channels, self.sample_rate)?;
        self.update_latency_compensation();
        Ok(())
    }

    /// Rebuild path B from config
    fn rebuild_path_b(&mut self) -> Result<(), String> {
        self.host_b =
            build_path_from_config(&self.path_b_config, self.num_channels, self.sample_rate)?;
        self.update_latency_compensation();
        Ok(())
    }

    /// Returns true if the band mask range is narrower than full spectrum.
    fn band_mask_active(&self) -> bool {
        self.band_mask_low_hz > 20.5 || self.band_mask_high_hz < 19999.5
    }

    /// Rebuild the bandpass filter pair for the current band mask settings.
    fn rebuild_band_mask_filters(&mut self) {
        let q = 1.0 / std::f64::consts::SQRT_2;
        let sr = self.sample_rate as f64;
        self.band_mask_hp = (0..self.num_channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Highpass,
                    self.band_mask_low_hz as f64,
                    sr,
                    q,
                    0.0,
                )
            })
            .collect();
        self.band_mask_lp = (0..self.num_channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Lowpass,
                    self.band_mask_high_hz as f64,
                    sr,
                    q,
                    0.0,
                )
            })
            .collect();
    }

    /// Align both paths by delaying the shorter one.
    fn update_latency_compensation(&mut self) {
        // Ensure hosts are built so latency queries work
        let _ = self.host_a.build();
        let _ = self.host_b.build();
        let lat_a = self.host_a.total_latency_samples();
        let lat_b = self.host_b.total_latency_samples();
        if lat_a > lat_b {
            self.delay_a.set_delay(0, self.num_channels);
            self.delay_b.set_delay(lat_a - lat_b, self.num_channels);
        } else {
            self.delay_a.set_delay(lat_b - lat_a, self.num_channels);
            self.delay_b.set_delay(0, self.num_channels);
        }
    }
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

impl Plugin for ABComparePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("A/B Compare", "1.0.0", "SotF")
            .with_description("A/B comparison with automatic loudness matching")
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        match id.0.as_str() {
            "mix" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mix must be a float".to_string())?;
                if v.is_finite() {
                    self.mix = v.clamp(-1.0, 1.0);
                    self.mix_smoother.set_target(self.mix);
                }
            }
            "mix_mode" => {
                let v = value
                    .as_int()
                    .ok_or_else(|| "mix_mode must be an integer".to_string())?;
                self.mix_mode = if v == 0 {
                    MixMode::Potentiometer
                } else {
                    MixMode::Binary
                };
            }
            "selected_path" => {
                let v = value
                    .as_int()
                    .ok_or_else(|| "selected_path must be an integer".to_string())?;
                self.selected_path = v.clamp(0, 1);
                // Update mix target for binary mode
                if self.mix_mode == MixMode::Binary {
                    let target = if self.selected_path == 0 { -1.0 } else { 1.0 };
                    self.mix_smoother.set_target(target);
                }
            }
            "bypass" => {
                self.bypass = value
                    .as_bool()
                    .ok_or_else(|| "bypass must be a boolean".to_string())?;
            }
            "auto_gain_enabled" => {
                self.auto_gain.set_enabled(
                    value
                        .as_bool()
                        .ok_or_else(|| "auto_gain_enabled must be a boolean".to_string())?,
                );
            }
            "loudness_type" => {
                let v = value
                    .as_int()
                    .ok_or_else(|| "loudness_type must be an integer".to_string())?;
                let loudness_type = if v == 0 {
                    AutoGainLoudnessType::Momentary
                } else {
                    AutoGainLoudnessType::ShortTerm
                };
                self.auto_gain.set_loudness_type(loudness_type);
            }
            "max_auto_gain_db" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "max_auto_gain_db must be a float".to_string())?;
                if v.is_finite() {
                    self.auto_gain.set_max_gain_db(v.clamp(0.0, 24.0));
                }
            }
            "gain_smoothing_ms" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "gain_smoothing_ms must be a float".to_string())?;
                if v.is_finite() {
                    self.auto_gain.set_smoothing_ms(v.clamp(10.0, 500.0));
                }
            }
            "mix_transition_ms" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mix_transition_ms must be a float".to_string())?;
                if v.is_finite() {
                    self.mix_transition_ms = v.clamp(5.0, 500.0);
                    self.mix_smoother
                        .set_time(self.mix_transition_ms, self.sample_rate);
                }
            }
            "phase_invert_a" => {
                self.phase_invert_a = value
                    .as_bool()
                    .ok_or_else(|| "phase_invert_a must be a boolean".to_string())?;
            }
            "phase_invert_b" => {
                self.phase_invert_b = value
                    .as_bool()
                    .ok_or_else(|| "phase_invert_b must be a boolean".to_string())?;
            }
            "difference_mode" => {
                self.difference_mode = value
                    .as_bool()
                    .ok_or_else(|| "difference_mode must be a boolean".to_string())?;
            }
            "band_mask_low_hz" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "band_mask_low_hz must be a float".to_string())?;
                if v.is_finite() {
                    self.band_mask_low_hz = v.clamp(20.0, 20000.0);
                    self.rebuild_band_mask_filters();
                }
            }
            "band_mask_high_hz" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "band_mask_high_hz must be a float".to_string())?;
                if v.is_finite() {
                    self.band_mask_high_hz = v.clamp(20.0, 20000.0);
                    self.rebuild_band_mask_filters();
                }
            }
            "path_a_config" => {
                if let ParameterValue::String(json) = value {
                    let config: PathConfig = serde_json::from_str(&json)
                        .map_err(|e| format!("Invalid path A config JSON: {}", e))?;
                    self.path_a_config = config;
                    self.rebuild_path_a()?;
                }
            }
            "path_b_config" => {
                if let ParameterValue::String(json) = value {
                    let config: PathConfig = serde_json::from_str(&json)
                        .map_err(|e| format!("Invalid path B config JSON: {}", e))?;
                    self.path_b_config = config;
                    self.rebuild_path_b()?;
                }
            }
            _ => return Err(format!("Unknown parameter: {}", id.0)),
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.0.as_str() {
            "mix" => Some(ParameterValue::Float(self.mix)),
            "mix_mode" => Some(ParameterValue::Int(match self.mix_mode {
                MixMode::Potentiometer => 0,
                MixMode::Binary => 1,
            })),
            "selected_path" => Some(ParameterValue::Int(self.selected_path)),
            "bypass" => Some(ParameterValue::Bool(self.bypass)),
            "auto_gain_enabled" => Some(ParameterValue::Bool(self.auto_gain.is_enabled())),
            "loudness_type" => Some(ParameterValue::Int(match self.auto_gain.loudness_type() {
                AutoGainLoudnessType::Momentary => 0,
                AutoGainLoudnessType::ShortTerm => 1,
            })),
            "max_auto_gain_db" => Some(ParameterValue::Float(self.auto_gain.max_gain_db())),
            "gain_smoothing_ms" => Some(ParameterValue::Float(self.auto_gain.smoothing_ms())),
            "mix_transition_ms" => Some(ParameterValue::Float(self.mix_transition_ms)),
            "phase_invert_a" => Some(ParameterValue::Bool(self.phase_invert_a)),
            "phase_invert_b" => Some(ParameterValue::Bool(self.phase_invert_b)),
            "difference_mode" => Some(ParameterValue::Bool(self.difference_mode)),
            "band_mask_low_hz" => Some(ParameterValue::Float(self.band_mask_low_hz)),
            "band_mask_high_hz" => Some(ParameterValue::Float(self.band_mask_high_hz)),
            "path_a_config" => serde_json::to_string(&self.path_a_config)
                .ok()
                .map(ParameterValue::String),
            "path_b_config" => serde_json::to_string(&self.path_b_config)
                .ok()
                .map(ParameterValue::String),
            _ => None,
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Rebuild paths with new sample rate
        self.rebuild_path_a()?;
        self.rebuild_path_b()?;

        // Update auto-gain sample rate
        self.auto_gain
            .set_sample_rate(sample_rate)
            .map_err(|e| format!("Failed to update auto-gain sample rate: {}", e))?;

        // Reset mix smoother with new sample rate
        self.mix_smoother = Smoother::new(self.mix, self.mix_transition_ms, sample_rate);

        // Rebuild band mask filters for new sample rate
        self.rebuild_band_mask_filters();

        Ok(())
    }

    fn reset(&mut self) {
        // Reset hosts
        self.host_a.reset();
        self.host_b.reset();

        // Reset auto-gain (also resets loudness monitors)
        self.auto_gain.reset();

        // Reset delay lines
        self.delay_a.reset();
        self.delay_b.reset();

        // Reset mix smoother
        self.mix_smoother.reset(self.mix);

        // Reset peak values
        self.last_peak_a = 0.0;
        self.last_peak_b = 0.0;

        // Reset band mask filters
        self.rebuild_band_mask_filters();

        // Clear buffers
        self.buffer_a.clear();
        self.buffer_b.clear();

        // Update diagnostic cache immediately with reset values
        let data = ABCompareData {
            loudness_a_lufs: f64::NEG_INFINITY,
            loudness_b_lufs: f64::NEG_INFINITY,
            auto_gain_db: 0.0,
            peak_a: 0.0,
            peak_b: 0.0,
            current_mix: self.mix_smoother.current(),
            bypass_active: self.bypass,
        };
        self.cache.update(|d| {
            *d = data;
        });
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let expected_samples = context.num_frames * self.num_channels;

        // Verify input/output size
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }
        if output.len() != expected_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                expected_samples,
                output.len()
            ));
        }

        // Handle bypass
        if self.bypass {
            output.copy_from_slice(input);
            return Ok(context.num_frames);
        }

        // Resize buffers if needed
        if self.buffer_a.len() != expected_samples {
            self.buffer_a.resize(expected_samples, 0.0);
            self.buffer_b.resize(expected_samples, 0.0);
        }

        // Process path A
        self.host_a.process(input, &mut self.buffer_a)?;

        // Process path B
        self.host_b.process(input, &mut self.buffer_b)?;

        // Apply latency compensation (delays the shorter path)
        self.delay_a.process(&mut self.buffer_a[..expected_samples]);
        self.delay_b.process(&mut self.buffer_b[..expected_samples]);

        // Measure loudness and peaks using AutoGain (throttled)
        // A's output is the "input reference" (what we want B to match)
        // B's output is the "output to compensate"
        self.cache_update_counter += 1;
        let mut do_measure = false;
        // Measure on the first block and then every 10 blocks
        if self.cache_update_counter >= 10 || self.cache_update_counter == 1 {
            if self.cache_update_counter >= 10 {
                self.cache_update_counter = 0;
            }
            do_measure = true;
        }

        if do_measure {
            self.auto_gain.measure_input(&self.buffer_a)?;
            self.auto_gain.measure_output(&self.buffer_b)?;

            // Cache peak values for get_data()
            self.last_peak_a = self.auto_gain.last_input_peak();
            self.last_peak_b = self.auto_gain.last_output_peak();
        }

        // Determine target mix value
        let target_mix = match self.mix_mode {
            MixMode::Potentiometer => self.mix,
            MixMode::Binary => {
                if self.selected_path == 0 {
                    -1.0
                } else {
                    1.0
                }
            }
        };
        self.mix_smoother.set_target(target_mix);

        // Phase inversion signs
        let sign_a: f32 = if self.phase_invert_a { -1.0 } else { 1.0 };
        let sign_b: f32 = if self.phase_invert_b { -1.0 } else { 1.0 };

        // Process sample-by-sample
        for frame in 0..context.num_frames {
            // Tick smoothers into loop
            let gain_linear = self.auto_gain.next_gain_linear();
            let current_mix = self.mix_smoother.advance();

            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let sample_a = self.buffer_a[idx] * sign_a;
                let sample_b = self.buffer_b[idx] * gain_linear * sign_b;

                if self.difference_mode {
                    // Difference mode: output A - B
                    output[idx] = sample_a - sample_b;
                } else {
                    // Equal-power crossfade
                    // mix: -1 = pure A, +1 = pure B
                    let mix_01 = (current_mix + 1.0) / 2.0; // 0 = A, 1 = B
                    let angle = mix_01 * std::f32::consts::FRAC_PI_2; // 0 to PI/2
                    let gain_a = angle.cos();
                    let gain_b = angle.sin();
                    output[idx] = sample_a * gain_a + sample_b * gain_b;
                }
            }
        }

        // Apply band mask filter if the range is narrower than full spectrum
        if self.band_mask_active() {
            for frame in 0..context.num_frames {
                for ch in 0..self.num_channels {
                    let idx = frame * self.num_channels + ch;
                    let mut s = output[idx] as f64;
                    s = self.band_mask_hp[ch].process(s);
                    s = self.band_mask_lp[ch].process(s);
                    output[idx] = s as f32;
                }
            }
        }

        // Update diagnostic cache (throttled)
        if do_measure {
            let data = ABCompareData {
                loudness_a_lufs: self.auto_gain.last_input_lufs(),
                loudness_b_lufs: self.auto_gain.last_output_lufs(),
                auto_gain_db: self.auto_gain.current_gain_db(),
                peak_a: self.last_peak_a,
                peak_b: self.last_peak_b,
                current_mix: self.mix_smoother.current(),
                bypass_active: self.bypass,
            };
            self.cache.update(|d| {
                *d = data;
            });
        }

        Ok(context.num_frames)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }

    fn latency_samples(&self) -> usize {
        // Total latency is the max of both paths
        let latency_a = self.host_a.total_latency_samples();
        let latency_b = self.host_b.total_latency_samples();
        latency_a.max(latency_b)
    }
}
