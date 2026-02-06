// ============================================================================
// Fletcher-Munson Loudness Compensation Plugin
// ============================================================================
//
// This plugin implements volume-dependent loudness compensation based on
// ISO 226 equal-loudness contours (Fletcher-Munson curves).
//
// At low listening volumes, human hearing is less sensitive to bass and treble
// frequencies. This plugin uses 4 peak filters with volume-dependent gains
// to compensate for this effect.
//
// Key features:
// - 4 parametric peak bands targeting key frequency regions
// - Gain curves that increase boost as playback volume decreases
// - Smooth transitions when volume changes (no clicks)
// - Reference level parameter to define "flat" response point
// - Output compensation to prevent clipping

use super::auto_gain::{AutoGain, AutoGainLoudnessType, AutoGainParams};
use super::param_specs::fletcher_munson::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
use super::smoothing::Smoother;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

// ============================================================================
// Band Configuration
// ============================================================================

/// Configuration for a single Fletcher-Munson compensation band
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FletcherMunsonBand {
    /// Center frequency (Hz)
    pub frequency: f64,
    /// Q factor (bandwidth) - lower Q = wider band
    pub q: f64,
    /// Maximum gain at lowest volume (dB)
    pub max_gain_db: f64,
    /// Slope of gain vs volume curve (dB gain per dB volume delta)
    pub slope: f64,
}

impl FletcherMunsonBand {
    /// Create a new band configuration
    pub fn new(frequency: f64, q: f64, max_gain_db: f64, slope: f64) -> Self {
        Self {
            frequency,
            q,
            max_gain_db,
            slope,
        }
    }
}

// ============================================================================
// Plugin Parameters
// ============================================================================

fn default_playback_volume_db() -> f32 {
    PLAYBACK_VOLUME_DB_DEFAULT
}

fn default_reference_level_db() -> f32 {
    REFERENCE_LEVEL_DB_DEFAULT
}

fn default_smoothing_ms() -> f32 {
    SMOOTHING_MS_DEFAULT
}

fn default_enabled() -> bool {
    ENABLED_DEFAULT
}

fn default_auto_gain_enabled() -> bool {
    AUTO_GAIN_ENABLED_DEFAULT
}

fn default_auto_gain_max_db() -> f32 {
    AUTO_GAIN_MAX_DB_DEFAULT
}

fn default_auto_gain_smoothing_ms() -> f32 {
    AUTO_GAIN_SMOOTHING_MS_DEFAULT
}

fn default_auto_gain_loudness_type() -> i32 {
    AUTO_GAIN_LOUDNESS_TYPE_DEFAULT
}

fn default_band1() -> FletcherMunsonBand {
    FletcherMunsonBand::new(
        BAND1_FREQ_DEFAULT,
        BAND1_Q_DEFAULT,
        BAND1_MAX_GAIN_DEFAULT,
        BAND1_SLOPE_DEFAULT,
    )
}

fn default_band2() -> FletcherMunsonBand {
    FletcherMunsonBand::new(
        BAND2_FREQ_DEFAULT,
        BAND2_Q_DEFAULT,
        BAND2_MAX_GAIN_DEFAULT,
        BAND2_SLOPE_DEFAULT,
    )
}

fn default_band3() -> FletcherMunsonBand {
    FletcherMunsonBand::new(
        BAND3_FREQ_DEFAULT,
        BAND3_Q_DEFAULT,
        BAND3_MAX_GAIN_DEFAULT,
        BAND3_SLOPE_DEFAULT,
    )
}

fn default_band4() -> FletcherMunsonBand {
    FletcherMunsonBand::new(
        BAND4_FREQ_DEFAULT,
        BAND4_Q_DEFAULT,
        BAND4_MAX_GAIN_DEFAULT,
        BAND4_SLOPE_DEFAULT,
    )
}

/// Configuration parameters for FletcherMunsonPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FletcherMunsonPluginParams {
    /// Current playback volume in dB (set by engine/UI when volume changes)
    #[serde(default = "default_playback_volume_db")]
    pub playback_volume_db: f32,

    /// Reference level where response is flat (dB)
    /// Default: -14 dB corresponds to ~80 dB SPL (loud listening)
    #[serde(default = "default_reference_level_db")]
    pub reference_level_db: f32,

    /// Band 1: Sub-bass compensation (~60 Hz)
    #[serde(default = "default_band1")]
    pub band1: FletcherMunsonBand,

    /// Band 2: Mid-bass compensation (~250 Hz)
    #[serde(default = "default_band2")]
    pub band2: FletcherMunsonBand,

    /// Band 3: Presence compensation (~3.5 kHz)
    #[serde(default = "default_band3")]
    pub band3: FletcherMunsonBand,

    /// Band 4: Air/brilliance compensation (~12 kHz)
    #[serde(default = "default_band4")]
    pub band4: FletcherMunsonBand,

    /// Smoothing time for gain changes (ms)
    #[serde(default = "default_smoothing_ms")]
    pub smoothing_ms: f32,

    /// Plugin enabled/bypass
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Auto-gain compensation enabled
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,

    /// Maximum auto-gain correction in dB
    #[serde(default = "default_auto_gain_max_db")]
    pub auto_gain_max_db: f32,

    /// Auto-gain smoothing time in ms
    #[serde(default = "default_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f32,

    /// Loudness measurement type: 0 = Momentary, 1 = ShortTerm
    #[serde(default = "default_auto_gain_loudness_type")]
    pub auto_gain_loudness_type: i32,
}

impl Default for FletcherMunsonPluginParams {
    fn default() -> Self {
        Self {
            playback_volume_db: default_playback_volume_db(),
            reference_level_db: default_reference_level_db(),
            band1: default_band1(),
            band2: default_band2(),
            band3: default_band3(),
            band4: default_band4(),
            smoothing_ms: default_smoothing_ms(),
            enabled: default_enabled(),
            auto_gain_enabled: default_auto_gain_enabled(),
            auto_gain_max_db: default_auto_gain_max_db(),
            auto_gain_smoothing_ms: default_auto_gain_smoothing_ms(),
            auto_gain_loudness_type: default_auto_gain_loudness_type(),
        }
    }
}

// ============================================================================
// Fletcher-Munson Plugin
// ============================================================================

/// Number of compensation bands
const NUM_BANDS: usize = 4;

/// Threshold for updating filter coefficients (dB)
/// Only rebuild filters when gain changes more than this
const GAIN_UPDATE_THRESHOLD: f32 = 0.1;

/// Fletcher-Munson loudness compensation plugin
///
/// Implements ISO 226-based equal-loudness compensation with 4 parametric
/// peak filters whose gains vary based on the current playback volume.
pub struct FletcherMunsonPlugin {
    /// Number of audio channels
    num_channels: usize,

    /// Sample rate
    sample_rate: u32,

    /// Current playback volume (dB)
    playback_volume_db: f32,

    /// Reference level where response is flat (dB)
    reference_level_db: f32,

    /// Band configurations
    bands: [FletcherMunsonBand; NUM_BANDS],

    /// Smoothing time (ms)
    smoothing_ms: f32,

    /// Plugin enabled state
    enabled: bool,

    /// Filters for each channel: filters[channel][band]
    filters: Vec<Vec<Biquad>>,

    /// Gain smoothers for each band
    gain_smoothers: [Smoother; NUM_BANDS],

    /// Last gains used for filter coefficient updates
    last_applied_gains: [f32; NUM_BANDS],

    /// Output compensation smoother
    compensation_smoother: Smoother,

    /// Auto-gain compensation for loudness matching
    auto_gain: Option<AutoGain>,

    /// Auto-gain enabled state
    auto_gain_enabled: bool,

    /// Auto-gain max dB
    auto_gain_max_db: f32,

    /// Auto-gain smoothing ms
    auto_gain_smoothing_ms: f32,

    /// Auto-gain loudness type (0 = Momentary, 1 = ShortTerm)
    auto_gain_loudness_type: i32,
}

impl FletcherMunsonPlugin {
    /// Create a new Fletcher-Munson plugin with default parameters
    pub fn new(num_channels: usize) -> Self {
        Self::from_params(num_channels, FletcherMunsonPluginParams::default())
    }

    /// Create a new plugin from configuration parameters
    pub fn from_params(num_channels: usize, params: FletcherMunsonPluginParams) -> Self {
        let sample_rate = 48000; // Will be updated in initialize()
        let smoothing_ms = params.smoothing_ms;

        let bands = [
            params.band1.clone(),
            params.band2.clone(),
            params.band3.clone(),
            params.band4.clone(),
        ];

        // Create auto-gain if enabled
        let loudness_type = if params.auto_gain_loudness_type == 0 {
            AutoGainLoudnessType::Momentary
        } else {
            AutoGainLoudnessType::ShortTerm
        };

        let auto_gain = if params.auto_gain_enabled {
            AutoGain::new(
                num_channels,
                sample_rate,
                AutoGainParams {
                    enabled: true,
                    loudness_type,
                    max_gain_db: params.auto_gain_max_db,
                    smoothing_ms: params.auto_gain_smoothing_ms,
                },
            )
            .ok()
        } else {
            None
        };

        let mut plugin = Self {
            num_channels,
            sample_rate,
            playback_volume_db: params.playback_volume_db,
            reference_level_db: params.reference_level_db,
            bands,
            smoothing_ms,
            enabled: params.enabled,
            filters: Vec::new(),
            gain_smoothers: [
                Smoother::new(0.0, smoothing_ms, sample_rate),
                Smoother::new(0.0, smoothing_ms, sample_rate),
                Smoother::new(0.0, smoothing_ms, sample_rate),
                Smoother::new(0.0, smoothing_ms, sample_rate),
            ],
            last_applied_gains: [0.0; NUM_BANDS],
            compensation_smoother: Smoother::new(1.0, smoothing_ms, sample_rate),
            auto_gain,
            auto_gain_enabled: params.auto_gain_enabled,
            auto_gain_max_db: params.auto_gain_max_db,
            auto_gain_smoothing_ms: params.auto_gain_smoothing_ms,
            auto_gain_loudness_type: params.auto_gain_loudness_type,
        };

        plugin.rebuild_filters();
        plugin.update_band_gains();
        plugin
    }

    /// Calculate the gain for a band based on volume delta
    ///
    /// # Arguments
    /// * `band` - Band configuration
    /// * `volume_delta_db` - (reference_level_db - playback_volume_db)
    ///   Positive when playing quietly (below reference)
    ///
    /// # Returns
    /// Gain in dB to apply to this band
    fn calculate_band_gain(band: &FletcherMunsonBand, volume_delta_db: f32) -> f32 {
        if volume_delta_db <= 0.0 {
            // At or above reference level: no compensation needed
            return 0.0;
        }

        // Linear interpolation: gain = slope * volume_delta, clamped to max_gain
        (band.slope as f32 * volume_delta_db).min(band.max_gain_db as f32)
    }

    /// Update all band gains based on current playback volume
    fn update_band_gains(&mut self) {
        let volume_delta = self.reference_level_db - self.playback_volume_db;

        // Calculate target gains for each band
        for (i, band) in self.bands.iter().enumerate() {
            let target_gain = Self::calculate_band_gain(band, volume_delta);
            self.gain_smoothers[i].set_target(target_gain);
        }

        // Update compensation gain (to prevent clipping)
        // Compensation = -max(band gains)
        let max_gain: f32 = self
            .bands
            .iter()
            .map(|b| Self::calculate_band_gain(b, volume_delta))
            .fold(0.0_f32, f32::max);

        let compensation_linear = 10.0_f32.powf(-max_gain / 20.0);
        self.compensation_smoother.set_target(compensation_linear);
    }

    /// Rebuild all filters from scratch
    fn rebuild_filters(&mut self) {
        self.filters.clear();

        for _ch in 0..self.num_channels {
            let channel_filters: Vec<Biquad> = self
                .bands
                .iter()
                .enumerate()
                .map(|(i, band)| {
                    Biquad::new(
                        BiquadFilterType::Peak,
                        band.frequency,
                        self.sample_rate as f64,
                        band.q,
                        self.last_applied_gains[i] as f64,
                    )
                })
                .collect();
            self.filters.push(channel_filters);
        }
    }

    /// Check if filter coefficients need updating and rebuild if so
    fn maybe_update_filter_coefficients(&mut self, current_gains: &[f32; NUM_BANDS]) {
        let needs_update = current_gains
            .iter()
            .zip(self.last_applied_gains.iter())
            .any(|(current, last)| (current - last).abs() > GAIN_UPDATE_THRESHOLD);

        if needs_update {
            self.update_filter_coefficients(current_gains);
            self.last_applied_gains = *current_gains;
        }
    }

    /// Update filter coefficients with new gains
    fn update_filter_coefficients(&mut self, gains: &[f32; NUM_BANDS]) {
        for ch in 0..self.num_channels {
            for (i, (band, gain)) in self.bands.iter().zip(gains.iter()).enumerate() {
                self.filters[ch][i] = Biquad::new(
                    BiquadFilterType::Peak,
                    band.frequency,
                    self.sample_rate as f64,
                    band.q,
                    *gain as f64,
                );
            }
        }
    }

    /// Get band reference by index
    fn get_band(&self, band_idx: usize) -> Option<&FletcherMunsonBand> {
        match band_idx {
            0 => Some(&self.bands[0]),
            1 => Some(&self.bands[1]),
            2 => Some(&self.bands[2]),
            3 => Some(&self.bands[3]),
            _ => None,
        }
    }

    /// Get mutable band reference by index
    fn get_band_mut(&mut self, band_idx: usize) -> Option<&mut FletcherMunsonBand> {
        match band_idx {
            0 => Some(&mut self.bands[0]),
            1 => Some(&mut self.bands[1]),
            2 => Some(&mut self.bands[2]),
            3 => Some(&mut self.bands[3]),
            _ => None,
        }
    }
}

impl Plugin for FletcherMunsonPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Fletcher-Munson Compensation", "1.0.0", "SotF").with_description(
            "Volume-dependent loudness compensation based on ISO 226 equal-loudness contours",
        )
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        let mut params = vec![
            // Main control parameters
            Parameter::new_float(
                "playback_volume_db",
                "Playback Volume",
                PLAYBACK_VOLUME_DB_DEFAULT,
                PLAYBACK_VOLUME_DB_MIN,
                PLAYBACK_VOLUME_DB_MAX,
            )
            .with_description("Current playback volume (set by engine/UI)")
            .with_group("Control")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "reference_level_db",
                "Reference Level",
                REFERENCE_LEVEL_DB_DEFAULT,
                REFERENCE_LEVEL_DB_MIN,
                REFERENCE_LEVEL_DB_MAX,
            )
            .with_description("Volume level where response is flat (~80 dB SPL)")
            .with_group("Control")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_bool("enabled", "Enabled", ENABLED_DEFAULT)
                .with_description("Enable/bypass the plugin")
                .with_group("Control")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "smoothing_ms",
                "Smoothing",
                SMOOTHING_MS_DEFAULT,
                SMOOTHING_MS_MIN,
                SMOOTHING_MS_MAX,
            )
            .with_description("Gain transition smoothing time (ms)")
            .with_group("Control")
            .with_importance(ParameterImportance::Useful),
        ];

        // Auto-gain parameters
        params.push(
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", AUTO_GAIN_ENABLED_DEFAULT)
                .with_description("Enable automatic loudness compensation")
                .with_group("Auto Gain")
                .with_importance(ParameterImportance::Useful),
        );
        params.push(
            Parameter::new_float(
                "auto_gain_max_db",
                "Max Gain",
                AUTO_GAIN_MAX_DB_DEFAULT,
                AUTO_GAIN_MAX_DB_MIN,
                AUTO_GAIN_MAX_DB_MAX,
            )
            .with_description("Maximum auto-gain correction (dB)")
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
        );
        params.push(
            Parameter::new_float(
                "auto_gain_smoothing_ms",
                "AG Smoothing",
                AUTO_GAIN_SMOOTHING_MS_DEFAULT,
                AUTO_GAIN_SMOOTHING_MS_MIN,
                AUTO_GAIN_SMOOTHING_MS_MAX,
            )
            .with_description("Auto-gain smoothing time (ms)")
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::FineTuning),
        );
        params.push(
            Parameter::new_int(
                "auto_gain_loudness_type",
                "Loudness Type",
                AUTO_GAIN_LOUDNESS_TYPE_DEFAULT,
                0,
                1,
            )
            .with_description("0=Momentary (400ms), 1=ShortTerm (3s)")
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::FineTuning),
        );

        // Band parameters
        for band_idx in 1..=NUM_BANDS {
            let (freq_default, q_default, max_gain_default, slope_default) = match band_idx {
                1 => (
                    BAND1_FREQ_DEFAULT,
                    BAND1_Q_DEFAULT,
                    BAND1_MAX_GAIN_DEFAULT,
                    BAND1_SLOPE_DEFAULT,
                ),
                2 => (
                    BAND2_FREQ_DEFAULT,
                    BAND2_Q_DEFAULT,
                    BAND2_MAX_GAIN_DEFAULT,
                    BAND2_SLOPE_DEFAULT,
                ),
                3 => (
                    BAND3_FREQ_DEFAULT,
                    BAND3_Q_DEFAULT,
                    BAND3_MAX_GAIN_DEFAULT,
                    BAND3_SLOPE_DEFAULT,
                ),
                4 => (
                    BAND4_FREQ_DEFAULT,
                    BAND4_Q_DEFAULT,
                    BAND4_MAX_GAIN_DEFAULT,
                    BAND4_SLOPE_DEFAULT,
                ),
                _ => unreachable!(),
            };

            params.push(
                Parameter::new_float(
                    &format!("band{}_freq", band_idx),
                    &format!("Band {} Frequency", band_idx),
                    freq_default as f32,
                    BAND_FREQ_MIN as f32,
                    BAND_FREQ_MAX as f32,
                )
                .with_description(&format!("Band {} center frequency (Hz)", band_idx))
                .with_group(&format!("Band {}", band_idx))
                .with_importance(ParameterImportance::FineTuning),
            );
            params.push(
                Parameter::new_float(
                    &format!("band{}_q", band_idx),
                    &format!("Band {} Q", band_idx),
                    q_default as f32,
                    BAND_Q_MIN as f32,
                    BAND_Q_MAX as f32,
                )
                .with_description(&format!("Band {} Q factor (bandwidth)", band_idx))
                .with_group(&format!("Band {}", band_idx))
                .with_importance(ParameterImportance::FineTuning),
            );
            params.push(
                Parameter::new_float(
                    &format!("band{}_max_gain", band_idx),
                    &format!("Band {} Max Gain", band_idx),
                    max_gain_default as f32,
                    BAND_MAX_GAIN_MIN as f32,
                    BAND_MAX_GAIN_MAX as f32,
                )
                .with_description(&format!(
                    "Band {} maximum gain at lowest volume (dB)",
                    band_idx
                ))
                .with_group(&format!("Band {}", band_idx))
                .with_importance(ParameterImportance::FineTuning),
            );
            params.push(
                Parameter::new_float(
                    &format!("band{}_slope", band_idx),
                    &format!("Band {} Slope", band_idx),
                    slope_default as f32,
                    BAND_SLOPE_MIN as f32,
                    BAND_SLOPE_MAX as f32,
                )
                .with_description(&format!(
                    "Band {} gain slope (dB per dB volume delta)",
                    band_idx
                ))
                .with_group(&format!("Band {}", band_idx))
                .with_importance(ParameterImportance::FineTuning),
            );
        }

        params
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let id_str = id.as_str();

        match id_str {
            "playback_volume_db" => {
                if let Some(v) = value.as_float() {
                    self.playback_volume_db = v;
                    self.update_band_gains();
                    return Ok(());
                }
                return Err("playback_volume_db must be a float".to_string());
            }
            "reference_level_db" => {
                if let Some(v) = value.as_float() {
                    self.reference_level_db = v;
                    self.update_band_gains();
                    return Ok(());
                }
                return Err("reference_level_db must be a float".to_string());
            }
            "enabled" => {
                if let Some(v) = value.as_bool() {
                    self.enabled = v;
                    return Ok(());
                }
                return Err("enabled must be a bool".to_string());
            }
            "smoothing_ms" => {
                if let Some(v) = value.as_float() {
                    self.smoothing_ms = v;
                    for smoother in &mut self.gain_smoothers {
                        smoother.set_time(v, self.sample_rate);
                    }
                    self.compensation_smoother.set_time(v, self.sample_rate);
                    return Ok(());
                }
                return Err("smoothing_ms must be a float".to_string());
            }
            "auto_gain_enabled" => {
                if let Some(v) = value.as_bool() {
                    self.auto_gain_enabled = v;
                    if v {
                        // Create auto-gain if not exists
                        if self.auto_gain.is_none() {
                            let loudness_type = if self.auto_gain_loudness_type == 0 {
                                AutoGainLoudnessType::Momentary
                            } else {
                                AutoGainLoudnessType::ShortTerm
                            };
                            self.auto_gain = AutoGain::new(
                                self.num_channels,
                                self.sample_rate,
                                AutoGainParams {
                                    enabled: true,
                                    loudness_type,
                                    max_gain_db: self.auto_gain_max_db,
                                    smoothing_ms: self.auto_gain_smoothing_ms,
                                },
                            )
                            .ok();
                        } else if let Some(ag) = &mut self.auto_gain {
                            ag.set_enabled(true);
                        }
                    } else if let Some(ag) = &mut self.auto_gain {
                        ag.set_enabled(false);
                    }
                    return Ok(());
                }
                return Err("auto_gain_enabled must be a bool".to_string());
            }
            "auto_gain_max_db" => {
                if let Some(v) = value.as_float() {
                    self.auto_gain_max_db = v;
                    if let Some(ag) = &mut self.auto_gain {
                        ag.set_max_gain_db(v);
                    }
                    return Ok(());
                }
                return Err("auto_gain_max_db must be a float".to_string());
            }
            "auto_gain_smoothing_ms" => {
                if let Some(v) = value.as_float() {
                    self.auto_gain_smoothing_ms = v;
                    if let Some(ag) = &mut self.auto_gain {
                        ag.set_smoothing_ms(v);
                    }
                    return Ok(());
                }
                return Err("auto_gain_smoothing_ms must be a float".to_string());
            }
            "auto_gain_loudness_type" => {
                if let Some(v) = value.as_int() {
                    self.auto_gain_loudness_type = v;
                    if let Some(ag) = &mut self.auto_gain {
                        let loudness_type = if v == 0 {
                            AutoGainLoudnessType::Momentary
                        } else {
                            AutoGainLoudnessType::ShortTerm
                        };
                        ag.set_loudness_type(loudness_type);
                    }
                    return Ok(());
                }
                return Err("auto_gain_loudness_type must be an int".to_string());
            }
            _ => {}
        }

        // Handle band parameters: band{1-4}_{freq|q|max_gain|slope}
        for band_num in 1..=NUM_BANDS {
            let band_idx = band_num - 1;
            let prefix = format!("band{}_", band_num);

            if let Some(suffix) = id_str.strip_prefix(&prefix)
                && let Some(v) = value.as_float()
                && let Some(band) = self.get_band_mut(band_idx)
            {
                match suffix {
                    "freq" => {
                        band.frequency = v as f64;
                        self.rebuild_filters();
                        return Ok(());
                    }
                    "q" => {
                        band.q = v as f64;
                        self.rebuild_filters();
                        return Ok(());
                    }
                    "max_gain" => {
                        band.max_gain_db = v as f64;
                        self.update_band_gains();
                        return Ok(());
                    }
                    "slope" => {
                        band.slope = v as f64;
                        self.update_band_gains();
                        return Ok(());
                    }
                    _ => return Err(format!("Invalid band parameter: {}", id_str)),
                }
            }
        }

        Err(format!("Unknown parameter: {}", id_str))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let id_str = id.as_str();

        match id_str {
            "playback_volume_db" => return Some(ParameterValue::Float(self.playback_volume_db)),
            "reference_level_db" => return Some(ParameterValue::Float(self.reference_level_db)),
            "enabled" => return Some(ParameterValue::Bool(self.enabled)),
            "smoothing_ms" => return Some(ParameterValue::Float(self.smoothing_ms)),
            "auto_gain_enabled" => return Some(ParameterValue::Bool(self.auto_gain_enabled)),
            "auto_gain_max_db" => return Some(ParameterValue::Float(self.auto_gain_max_db)),
            "auto_gain_smoothing_ms" => {
                return Some(ParameterValue::Float(self.auto_gain_smoothing_ms));
            }
            "auto_gain_loudness_type" => {
                return Some(ParameterValue::Int(self.auto_gain_loudness_type));
            }
            _ => {}
        }

        // Handle band parameters
        for band_num in 1..=NUM_BANDS {
            let band_idx = band_num - 1;
            let prefix = format!("band{}_", band_num);

            if let Some(suffix) = id_str.strip_prefix(&prefix)
                && let Some(band) = self.get_band(band_idx)
            {
                match suffix {
                    "freq" => return Some(ParameterValue::Float(band.frequency as f32)),
                    "q" => return Some(ParameterValue::Float(band.q as f32)),
                    "max_gain" => return Some(ParameterValue::Float(band.max_gain_db as f32)),
                    "slope" => return Some(ParameterValue::Float(band.slope as f32)),
                    _ => {}
                }
            }
        }

        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Reinitialize smoothers with new sample rate
        for smoother in &mut self.gain_smoothers {
            smoother.set_time(self.smoothing_ms, sample_rate);
        }
        self.compensation_smoother
            .set_time(self.smoothing_ms, sample_rate);

        // Rebuild filters with new sample rate
        self.rebuild_filters();
        self.update_band_gains();

        // Reinitialize auto-gain with new sample rate
        if let Some(ag) = &mut self.auto_gain
            && let Err(e) = ag.set_sample_rate(sample_rate) {
                log::warn!("Failed to set auto-gain sample rate: {}", e);
            }

        Ok(())
    }

    fn reset(&mut self) {
        // Reset filter states
        self.filters.clear();
        self.rebuild_filters();

        // Reset smoothers to current target values
        for smoother in &mut self.gain_smoothers {
            smoother.reset(smoother.target());
        }
        self.compensation_smoother
            .reset(self.compensation_smoother.target());

        self.last_applied_gains = [0.0; NUM_BANDS];

        // Reset auto-gain
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
        // Validate buffer sizes
        let expected_samples = context.num_frames * self.num_channels;
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

        // If disabled, passthrough
        if !self.enabled {
            output.copy_from_slice(input);
            return Ok(context.num_frames);
        }

        // Ensure filters are initialized
        if self.filters.is_empty() {
            self.rebuild_filters();
        }

        // Measure input for auto-gain if enabled
        if let Some(ag) = &mut self.auto_gain
            && ag.is_enabled() {
                let _ = ag.measure_input(input);
            }

        // Process each frame
        for frame_idx in 0..context.num_frames {
            // Advance smoothers and get current gains
            let current_gains: [f32; NUM_BANDS] = [
                self.gain_smoothers[0].next(),
                self.gain_smoothers[1].next(),
                self.gain_smoothers[2].next(),
                self.gain_smoothers[3].next(),
            ];

            // Update filter coefficients if gains changed significantly
            self.maybe_update_filter_coefficients(&current_gains);

            // Get compensation gain for this frame
            let compensation = self.compensation_smoother.next();

            // Process each channel
            for ch in 0..self.num_channels {
                let sample_idx = frame_idx * self.num_channels + ch;
                let mut sample = input[sample_idx] as f64;

                // Apply all 4 peak filters in series
                for filter in &mut self.filters[ch] {
                    sample = filter.process(sample);
                }

                // Apply compensation gain to prevent clipping
                output[sample_idx] = (sample as f32) * compensation;
            }
        }

        // Measure output and apply auto-gain compensation if enabled
        if let Some(ag) = &mut self.auto_gain
            && ag.is_enabled() {
                let _ = ag.measure_output(output);
                ag.apply_compensation(output, context.num_frames);
            }

        // Flush denormals to prevent CPU performance spikes and audio crackle
        // IIR biquad filter calculations can produce denormal numbers
        flush_denormals_inplace(output);

        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        // IIR filters have minimal latency
        0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let plugin = FletcherMunsonPlugin::new(2);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
        assert!(plugin.enabled);
    }

    #[test]
    fn test_band_gain_at_reference_level() {
        // At reference level (volume_delta = 0), gain should be 0
        let band = FletcherMunsonBand::new(60.0, 0.5, 15.0, 0.6);
        let gain = FletcherMunsonPlugin::calculate_band_gain(&band, 0.0);
        assert!((gain - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_band_gain_above_reference() {
        // Above reference level (volume_delta < 0), gain should be 0
        let band = FletcherMunsonBand::new(60.0, 0.5, 15.0, 0.6);
        let gain = FletcherMunsonPlugin::calculate_band_gain(&band, -10.0);
        assert!((gain - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_band_gain_below_reference() {
        // Below reference: gain = slope * volume_delta
        let band = FletcherMunsonBand::new(60.0, 0.5, 15.0, 0.6);
        // 20 dB below reference: 0.6 * 20 = 12 dB gain
        let gain = FletcherMunsonPlugin::calculate_band_gain(&band, 20.0);
        assert!((gain - 12.0).abs() < 0.001);
    }

    #[test]
    fn test_band_gain_clamped_to_max() {
        let band = FletcherMunsonBand::new(60.0, 0.5, 15.0, 0.6);
        // 40 dB below: 0.6 * 40 = 24 dB, but clamped to 15 dB
        let gain = FletcherMunsonPlugin::calculate_band_gain(&band, 40.0);
        assert!((gain - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_parameter_set_get() {
        let mut plugin = FletcherMunsonPlugin::new(2);

        // Set playback volume
        plugin
            .set_parameter(
                ParameterId::from("playback_volume_db"),
                ParameterValue::Float(-30.0),
            )
            .unwrap();

        let value = plugin.get_parameter(&ParameterId::from("playback_volume_db"));
        assert_eq!(value, Some(ParameterValue::Float(-30.0)));

        // Set band parameter
        plugin
            .set_parameter(ParameterId::from("band1_freq"), ParameterValue::Float(80.0))
            .unwrap();

        let value = plugin.get_parameter(&ParameterId::from("band1_freq"));
        assert_eq!(value, Some(ParameterValue::Float(80.0)));
    }

    #[test]
    fn test_bypass_passthrough() {
        let mut plugin = FletcherMunsonPlugin::new(2);
        plugin.initialize(48000).unwrap();

        // Disable plugin
        plugin
            .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
            .unwrap();

        let num_frames = 512;
        let input: Vec<f32> = (0..num_frames * 2)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();
        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Should be exact passthrough
        for (i, o) in input.iter().zip(output.iter()) {
            assert_eq!(*i, *o);
        }
    }

    #[test]
    fn test_processing_at_reference_level() {
        let mut plugin = FletcherMunsonPlugin::new(2);
        plugin.initialize(48000).unwrap();

        // Set volume at reference level (should be flat response)
        plugin
            .set_parameter(
                ParameterId::from("playback_volume_db"),
                ParameterValue::Float(REFERENCE_LEVEL_DB_DEFAULT),
            )
            .unwrap();

        let num_frames = 1024;
        let input: Vec<f32> = (0..num_frames * 2)
            .map(|i| {
                let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0 / 2.0;
                phase.sin() * 0.5
            })
            .collect();
        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // At reference level, output should be close to input (filters have ~0 gain)
        let input_rms = (input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32).sqrt();
        let output_rms = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
        let ratio = output_rms / input_rms;

        // Should be nearly unity (within 1 dB)
        assert!(
            ratio > 0.89 && ratio < 1.12,
            "At reference level, ratio should be ~1.0, got {}",
            ratio
        );
    }

    #[test]
    fn test_bass_boost_at_low_volume() {
        let mut plugin = FletcherMunsonPlugin::new(2);
        plugin.initialize(48000).unwrap();

        // Set very low playback volume
        plugin
            .set_parameter(
                ParameterId::from("playback_volume_db"),
                ParameterValue::Float(-50.0),
            )
            .unwrap();

        // Wait for smoothers to settle
        for _ in 0..10000 {
            for smoother in &mut plugin.gain_smoothers {
                smoother.next();
            }
            plugin.compensation_smoother.next();
        }

        // Generate 60 Hz sine wave (in bass region)
        let num_frames = 4096;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 60.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.3;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let mut output = vec![0.0_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Output should not be silent
        let output_energy: f32 = output.iter().map(|x| x * x).sum();
        assert!(
            output_energy > 0.0,
            "Output should not be silent at low volume"
        );
    }

    #[test]
    fn test_multichannel_processing() {
        let mut plugin = FletcherMunsonPlugin::new(5); // 5.0 surround
        plugin.initialize(48000).unwrap();

        let num_frames = 1024;
        let input = vec![0.5_f32; num_frames * 5];
        let mut output = vec![0.0_f32; num_frames * 5];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Should not panic
        plugin.process(&input, &mut output, &context).unwrap();

        // Output should be non-zero
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0);
    }

    #[test]
    fn test_info() {
        let plugin = FletcherMunsonPlugin::new(2);
        let info = plugin.info();
        assert_eq!(info.name, "Fletcher-Munson Compensation");
        assert_eq!(info.version, "1.0.0");
    }

    #[test]
    fn test_reset() {
        let mut plugin = FletcherMunsonPlugin::new(2);
        plugin.initialize(48000).unwrap();

        // Process some audio
        let input = vec![0.5_f32; 1024 * 2];
        let mut output = vec![0.0_f32; 1024 * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };
        plugin.process(&input, &mut output, &context).unwrap();

        // Reset should not panic
        plugin.reset();

        // Should still be able to process
        plugin.process(&input, &mut output, &context).unwrap();
    }
}
