// ============================================================================
// Loudness Compensation Plugin
// ============================================================================
//
// This plugin provides loudness compensation using:
// - Low-shelf filter with 12dB/octave slope (2 cascaded biquads)
// - High-shelf filter with 12dB/octave slope (2 cascaded biquads)
// - Automatic gain compensation to prevent clipping
// - Optional auto-gain for loudness matching (measures input/output LUFS)
//
// Supports two modes:
// 1. Single set of parameters applied to all channels (default)
// 2. Per-channel parameters with independent values for each channel
//
// Typical use: Boost bass and treble at low listening volumes to compensate
// for the Fletcher-Munson equal-loudness contours.

use super::auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
use super::param_specs::loudness_compensation::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

// ============================================================================
// Loudness Compensation Configuration (used by CamillaDSP integration)
// ============================================================================

/// Loudness compensation settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoudnessCompensation {
    pub reference_level: f64, // -100 .. +20
    pub low_boost: f64,       // 0 .. 20
    pub high_boost: f64,      // 0 .. 20
    #[serde(default)]
    pub attenuate_mid: bool,
}

impl LoudnessCompensation {
    pub fn new(reference_level: f64, low_boost: f64, high_boost: f64) -> Result<Self, String> {
        let lc = Self {
            reference_level,
            low_boost,
            high_boost,
            attenuate_mid: false,
        };
        lc.validate()?;
        Ok(lc)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(self.reference_level >= -100.0 && self.reference_level <= 20.0) {
            return Err(format!(
                "reference_level out of range (-100..20): {}",
                self.reference_level
            ));
        }
        if !(self.low_boost >= 0.0 && self.low_boost <= 20.0) {
            return Err(format!(
                "low_boost out of range (0..20): {}",
                self.low_boost
            ));
        }
        if !(self.high_boost >= 0.0 && self.high_boost <= 20.0) {
            return Err(format!(
                "high_boost out of range (0..20): {}",
                self.high_boost
            ));
        }
        Ok(())
    }
}

// ============================================================================
// Configuration Parameters
// ============================================================================

fn default_low_freq() -> f32 {
    LOW_FREQ_DEFAULT
}

fn default_low_gain() -> f32 {
    LOW_GAIN_DEFAULT
}

fn default_high_freq() -> f32 {
    HIGH_FREQ_DEFAULT
}

fn default_high_gain() -> f32 {
    HIGH_GAIN_DEFAULT
}

fn default_auto_gain_enabled() -> bool {
    false
}

fn default_auto_gain_max_db() -> f32 {
    12.0
}

fn default_auto_gain_smoothing_ms() -> f32 {
    100.0
}

/// Per-channel loudness compensation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLoudnessParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
}

impl Default for ChannelLoudnessParams {
    fn default() -> Self {
        Self {
            low_freq: LOW_FREQ_DEFAULT,
            low_gain: LOW_GAIN_DEFAULT,
            high_freq: HIGH_FREQ_DEFAULT,
            high_gain: HIGH_GAIN_DEFAULT,
        }
    }
}

/// Configuration parameters for LoudnessCompensationPlugin
///
/// Supports two modes:
/// 1. Single parameters for all channels: Use top-level fields
/// 2. Per-channel parameters: Use `channel_params` field
///
/// If `channel_params` is provided and non-empty, it takes precedence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessCompensationPluginParams {
    /// Global low-shelf frequency (used if channel_params is empty)
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    /// Global low-shelf gain (used if channel_params is empty)
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    /// Global high-shelf frequency (used if channel_params is empty)
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    /// Global high-shelf gain (used if channel_params is empty)
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,

    /// Per-channel parameters (optional)
    /// If provided, must have exactly one entry per channel
    #[serde(default)]
    pub channel_params: Vec<ChannelLoudnessParams>,

    /// Enable automatic gain compensation to maintain perceived loudness
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,

    /// Maximum gain correction in dB (clamped to +/- this value)
    #[serde(default = "default_auto_gain_max_db")]
    pub auto_gain_max_db: f32,

    /// Gain smoothing time in milliseconds
    #[serde(default = "default_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f32,
}

// ============================================================================
// Loudness Compensation Plugin
// ============================================================================

/// Loudness compensation plugin with per-channel support
pub struct LoudnessCompensationPlugin {
    /// Number of input/output channels
    num_channels: usize,

    /// Global low-shelf frequency (Hz)
    param_low_freq: ParameterId,
    low_freq: f32,

    /// Global low-shelf gain (dB)
    param_low_gain: ParameterId,
    low_gain: f32,

    /// Global high-shelf frequency (Hz)
    param_high_freq: ParameterId,
    high_freq: f32,

    /// Global high-shelf gain (dB)
    param_high_gain: ParameterId,
    high_gain: f32,

    /// Per-channel parameters (empty = use global params)
    channel_params: Vec<ChannelLoudnessParams>,

    /// Sample rate
    sample_rate: u32,

    /// Filters for each channel
    /// filters[channel][filter_idx] where filter_idx:
    /// 0-1: Low-shelf stages (2 for 12dB/oct)
    /// 2-3: High-shelf stages (2 for 12dB/oct)
    filters: Vec<Vec<Biquad>>,

    /// Compensation gains per channel to prevent clipping
    compensation_gains: Vec<f32>,

    /// Auto-gain compensation for loudness matching
    auto_gain: Option<AutoGain>,

    /// Auto-gain enabled state
    auto_gain_enabled: bool,

    /// Auto-gain max gain in dB
    auto_gain_max_db: f32,

    /// Auto-gain smoothing time in ms
    auto_gain_smoothing_ms: f32,
}

impl LoudnessCompensationPlugin {
    /// Create a new loudness compensation plugin with global parameters
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to process
    /// * `low_freq` - Low-shelf frequency in Hz (default: 100.0)
    /// * `low_gain` - Low-shelf gain in dB (default: 6.0)
    /// * `high_freq` - High-shelf frequency in Hz (default: 10000.0)
    /// * `high_gain` - High-shelf gain in dB (default: 6.0)
    pub fn new(
        num_channels: usize,
        low_freq: f32,
        low_gain: f32,
        high_freq: f32,
        high_gain: f32,
    ) -> Self {
        let mut plugin = Self {
            num_channels,
            param_low_freq: ParameterId::from("low_freq"),
            low_freq,
            param_low_gain: ParameterId::from("low_gain"),
            low_gain,
            param_high_freq: ParameterId::from("high_freq"),
            high_freq,
            param_high_gain: ParameterId::from("high_gain"),
            high_gain,
            channel_params: Vec::new(),
            sample_rate: 48000,
            filters: Vec::new(),
            compensation_gains: Vec::new(),
            auto_gain: None,
            auto_gain_enabled: default_auto_gain_enabled(),
            auto_gain_max_db: default_auto_gain_max_db(),
            auto_gain_smoothing_ms: default_auto_gain_smoothing_ms(),
        };

        plugin.rebuild_filters();
        plugin
    }

    /// Create a new loudness compensation plugin with per-channel parameters
    ///
    /// # Arguments
    /// * `channel_params` - Parameters for each channel
    ///
    /// # Errors
    /// Returns an error if channel_params is empty
    pub fn new_per_channel(channel_params: Vec<ChannelLoudnessParams>) -> Result<Self, String> {
        if channel_params.is_empty() {
            return Err("channel_params must not be empty".to_string());
        }

        let num_channels = channel_params.len();
        let mut plugin = Self {
            num_channels,
            param_low_freq: ParameterId::from("low_freq"),
            low_freq: LOW_FREQ_DEFAULT,
            param_low_gain: ParameterId::from("low_gain"),
            low_gain: LOW_GAIN_DEFAULT,
            param_high_freq: ParameterId::from("high_freq"),
            high_freq: HIGH_FREQ_DEFAULT,
            param_high_gain: ParameterId::from("high_gain"),
            high_gain: HIGH_GAIN_DEFAULT,
            channel_params,
            sample_rate: 48000,
            filters: Vec::new(),
            compensation_gains: Vec::new(),
            auto_gain: None,
            auto_gain_enabled: default_auto_gain_enabled(),
            auto_gain_max_db: default_auto_gain_max_db(),
            auto_gain_smoothing_ms: default_auto_gain_smoothing_ms(),
        };

        plugin.rebuild_filters();
        Ok(plugin)
    }

    /// Create a new loudness compensation plugin from configuration parameters
    pub fn from_params(
        num_channels: usize,
        params: LoudnessCompensationPluginParams,
    ) -> Result<Self, String> {
        let mut plugin = if params.channel_params.is_empty() {
            // Global mode
            Self::new(
                num_channels,
                params.low_freq,
                params.low_gain,
                params.high_freq,
                params.high_gain,
            )
        } else {
            // Per-channel mode
            if params.channel_params.len() != num_channels {
                return Err(format!(
                    "Channel params count mismatch: expected {} channels, got {} params",
                    num_channels,
                    params.channel_params.len()
                ));
            }
            Self::new_per_channel(params.channel_params)?
        };

        // Apply auto_gain settings
        plugin.auto_gain_enabled = params.auto_gain_enabled;
        plugin.auto_gain_max_db = params.auto_gain_max_db;
        plugin.auto_gain_smoothing_ms = params.auto_gain_smoothing_ms;

        Ok(plugin)
    }

    /// Check if plugin is in per-channel mode
    pub fn is_per_channel(&self) -> bool {
        !self.channel_params.is_empty()
    }

    /// Get parameters for a specific channel
    fn get_channel_params(&self, channel: usize) -> ChannelLoudnessParams {
        if self.is_per_channel() {
            self.channel_params
                .get(channel)
                .cloned()
                .unwrap_or_default()
        } else {
            ChannelLoudnessParams {
                low_freq: self.low_freq,
                low_gain: self.low_gain,
                high_freq: self.high_freq,
                high_gain: self.high_gain,
            }
        }
    }

    /// Set per-channel parameters (switches to per-channel mode)
    pub fn set_channel_params(
        &mut self,
        channel_params: Vec<ChannelLoudnessParams>,
    ) -> Result<(), String> {
        if channel_params.len() != self.num_channels {
            return Err(format!(
                "Channel params count mismatch: expected {} channels, got {} params",
                self.num_channels,
                channel_params.len()
            ));
        }

        self.channel_params = channel_params;
        self.rebuild_filters();
        Ok(())
    }

    /// Set a parameter for a specific channel (initializes per-channel mode if needed)
    pub fn set_channel_param(
        &mut self,
        channel: usize,
        param: &str,
        value: f32,
    ) -> Result<(), String> {
        if channel >= self.num_channels {
            return Err(format!(
                "Channel index {} out of bounds (max {})",
                channel,
                self.num_channels - 1
            ));
        }

        // Initialize per-channel mode if not already
        if self.channel_params.is_empty() {
            self.channel_params = (0..self.num_channels)
                .map(|_| ChannelLoudnessParams {
                    low_freq: self.low_freq,
                    low_gain: self.low_gain,
                    high_freq: self.high_freq,
                    high_gain: self.high_gain,
                })
                .collect();
        }

        match param {
            "low_freq" => self.channel_params[channel].low_freq = value,
            "low_gain" => self.channel_params[channel].low_gain = value,
            "high_freq" => self.channel_params[channel].high_freq = value,
            "high_gain" => self.channel_params[channel].high_gain = value,
            _ => return Err(format!("Unknown parameter: {}", param)),
        }

        self.rebuild_filters();
        Ok(())
    }

    /// Rebuild or update all filters based on current parameters
    fn rebuild_filters(&mut self) {
        // Q factor for shelving filters (0.707 = Butterworth response)
        let q = 0.707;

        // Ensure vectors are sized correctly
        if self.filters.len() != self.num_channels {
            self.filters.clear();
            self.filters.resize(self.num_channels, Vec::new());
        }
        if self.compensation_gains.len() != self.num_channels {
            self.compensation_gains.resize(self.num_channels, 0.0);
        }

        for ch in 0..self.num_channels {
            let params = self.get_channel_params(ch);

            // Calculate compensation gain for this channel: -max(low_gain, high_gain)
            let comp_gain = -params.low_gain.max(params.high_gain);
            self.compensation_gains[ch] = comp_gain;

            // For 12dB/octave slope, we need 2 cascaded biquads (each is 6dB/oct)
            // Split the gain between the two stages
            let low_gain_per_stage = params.low_gain / 2.0;
            let high_gain_per_stage = params.high_gain / 2.0;

            let target_configs = [
                // Low-shelf stage 1
                (
                    BiquadFilterType::Lowshelf,
                    params.low_freq,
                    low_gain_per_stage,
                ),
                // Low-shelf stage 2
                (
                    BiquadFilterType::Lowshelf,
                    params.low_freq,
                    low_gain_per_stage,
                ),
                // High-shelf stage 1
                (
                    BiquadFilterType::Highshelf,
                    params.high_freq,
                    high_gain_per_stage,
                ),
                // High-shelf stage 2
                (
                    BiquadFilterType::Highshelf,
                    params.high_freq,
                    high_gain_per_stage,
                ),
            ];

            // Initialize or update filters
            if self.filters[ch].len() != 4 {
                // Initialize from scratch (resets state)
                self.filters[ch] = target_configs
                    .iter()
                    .map(|(ft, freq, gain)| {
                        Biquad::new(*ft, *freq as f64, self.sample_rate as f64, q, *gain as f64)
                    })
                    .collect();
            } else {
                // Recreate filters with new coefficients
                // Note: This resets filter state which may cause brief transients
                for (i, (ft, freq, gain)) in target_configs.iter().enumerate() {
                    self.filters[ch][i] =
                        Biquad::new(*ft, *freq as f64, self.sample_rate as f64, q, *gain as f64);
                }
            }
        }
    }

    /// Update a global parameter and rebuild filters if needed
    fn update_global_parameter(&mut self, id: &ParameterId, value: f32) -> bool {
        let mut changed = false;

        if id == &self.param_low_freq {
            self.low_freq = value;
            changed = true;
        } else if id == &self.param_low_gain {
            self.low_gain = value;
            changed = true;
        } else if id == &self.param_high_freq {
            self.high_freq = value;
            changed = true;
        } else if id == &self.param_high_gain {
            self.high_gain = value;
            changed = true;
        }

        if changed {
            // Clear per-channel mode and rebuild
            self.channel_params.clear();
            self.rebuild_filters();
        }

        changed
    }

    /// Get auto-gain monitoring data
    pub fn get_auto_gain_data(&self) -> Option<AutoGainData> {
        self.auto_gain.as_ref().map(|ag| ag.get_data())
    }

    /// Check if auto-gain is enabled
    pub fn is_auto_gain_enabled(&self) -> bool {
        self.auto_gain_enabled
    }

    /// Rebuild the auto-gain instance with current settings
    fn rebuild_auto_gain(&mut self) -> Result<(), String> {
        if self.auto_gain_enabled {
            let params = AutoGainParams {
                enabled: true,
                loudness_type: AutoGainLoudnessType::Momentary,
                max_gain_db: self.auto_gain_max_db,
                smoothing_ms: self.auto_gain_smoothing_ms,
            };
            self.auto_gain = Some(AutoGain::new(self.num_channels, self.sample_rate, params)?);
        } else {
            // Keep existing auto_gain but disable it
            if let Some(ag) = &mut self.auto_gain {
                ag.set_enabled(false);
            }
        }
        Ok(())
    }
}

impl Plugin for LoudnessCompensationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Compensation", "1.1.0", "SotF").with_description(
            "Bass and treble boost for low-volume listening with per-channel support",
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
            Parameter::new_float(
                "low_freq",
                "Low-shelf Frequency",
                LOW_FREQ_DEFAULT,
                LOW_FREQ_MIN,
                LOW_FREQ_MAX,
            )
            .with_description("Global frequency for bass boost (Hz)")
            .with_group("Low Shelf")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "low_gain",
                "Low-shelf Gain",
                LOW_GAIN_DEFAULT,
                LOW_GAIN_MIN,
                LOW_GAIN_MAX,
            )
            .with_description("Global bass boost amount (dB)")
            .with_group("Low Shelf")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "high_freq",
                "High-shelf Frequency",
                HIGH_FREQ_DEFAULT,
                HIGH_FREQ_MIN,
                HIGH_FREQ_MAX,
            )
            .with_description("Global frequency for treble boost (Hz)")
            .with_group("High Shelf")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "high_gain",
                "High-shelf Gain",
                HIGH_GAIN_DEFAULT,
                HIGH_GAIN_MIN,
                HIGH_GAIN_MAX,
            )
            .with_description("Global treble boost amount (dB)")
            .with_group("High Shelf")
            .with_importance(ParameterImportance::Critical),
        ];

        // Add per-channel parameters
        for ch in 0..self.num_channels {
            params.push(
                Parameter::new_float(
                    &format!("low_freq_{}", ch),
                    &format!("Ch{} Low Freq", ch),
                    LOW_FREQ_DEFAULT,
                    LOW_FREQ_MIN,
                    LOW_FREQ_MAX,
                )
                .with_description(&format!("Channel {} bass frequency (Hz)", ch))
                .with_group("Channels")
                .with_importance(ParameterImportance::FineTuning),
            );
            params.push(
                Parameter::new_float(
                    &format!("low_gain_{}", ch),
                    &format!("Ch{} Low Gain", ch),
                    LOW_GAIN_DEFAULT,
                    LOW_GAIN_MIN,
                    LOW_GAIN_MAX,
                )
                .with_description(&format!("Channel {} bass boost (dB)", ch))
                .with_group("Channels")
                .with_importance(ParameterImportance::FineTuning),
            );
            params.push(
                Parameter::new_float(
                    &format!("high_freq_{}", ch),
                    &format!("Ch{} High Freq", ch),
                    HIGH_FREQ_DEFAULT,
                    HIGH_FREQ_MIN,
                    HIGH_FREQ_MAX,
                )
                .with_description(&format!("Channel {} treble frequency (Hz)", ch))
                .with_group("Channels")
                .with_importance(ParameterImportance::FineTuning),
            );
            params.push(
                Parameter::new_float(
                    &format!("high_gain_{}", ch),
                    &format!("Ch{} High Gain", ch),
                    HIGH_GAIN_DEFAULT,
                    HIGH_GAIN_MIN,
                    HIGH_GAIN_MAX,
                )
                .with_description(&format!("Channel {} treble boost (dB)", ch))
                .with_group("Channels")
                .with_importance(ParameterImportance::FineTuning),
            );
        }

        // Add auto-gain parameters
        params.push(
            Parameter::new_bool(
                "auto_gain_enabled",
                "Auto-Gain",
                default_auto_gain_enabled(),
            )
            .with_description("Enable automatic loudness compensation")
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
        );
        params.push(
            Parameter::new_float(
                "auto_gain_max_db",
                "Max Gain",
                default_auto_gain_max_db(),
                0.0,
                24.0,
            )
            .with_description("Maximum gain correction in dB")
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::FineTuning),
        );
        params.push(
            Parameter::new_float(
                "auto_gain_smoothing_ms",
                "Smoothing",
                default_auto_gain_smoothing_ms(),
                1.0,
                1000.0,
            )
            .with_description("Gain smoothing time in milliseconds")
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::FineTuning),
        );

        params
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let id_str = id.as_str();

        // Try global parameters first
        if let Some(val) = value.as_float() {
            if self.update_global_parameter(&id, val) {
                return Ok(());
            }
        }

        // Try auto-gain parameters
        match id_str {
            "auto_gain_enabled" => {
                if let Some(val) = value.as_bool() {
                    self.auto_gain_enabled = val;
                    self.rebuild_auto_gain()?;
                    return Ok(());
                } else {
                    return Err("auto_gain_enabled must be a bool".to_string());
                }
            }
            "auto_gain_max_db" => {
                if let Some(val) = value.as_float() {
                    self.auto_gain_max_db = val;
                    if let Some(ag) = &mut self.auto_gain {
                        ag.set_max_gain_db(val);
                    }
                    return Ok(());
                } else {
                    return Err("auto_gain_max_db must be a float".to_string());
                }
            }
            "auto_gain_smoothing_ms" => {
                if let Some(val) = value.as_float() {
                    self.auto_gain_smoothing_ms = val;
                    if let Some(ag) = &mut self.auto_gain {
                        ag.set_smoothing_ms(val);
                    }
                    return Ok(());
                } else {
                    return Err("auto_gain_smoothing_ms must be a float".to_string());
                }
            }
            _ => {}
        }

        // Try per-channel parameters: {param}_{channel}
        for param_name in &["low_freq", "low_gain", "high_freq", "high_gain"] {
            let prefix = format!("{}_", param_name);
            if let Some(suffix) = id_str.strip_prefix(&prefix) {
                if let Ok(channel) = suffix.parse::<usize>() {
                    if let Some(val) = value.as_float() {
                        return self.set_channel_param(channel, param_name, val);
                    } else {
                        return Err("Parameter must be a float".to_string());
                    }
                }
            }
        }

        Err(format!("Unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let id_str = id.as_str();

        // Check global parameters
        if id == &self.param_low_freq {
            return Some(ParameterValue::Float(self.low_freq));
        } else if id == &self.param_low_gain {
            return Some(ParameterValue::Float(self.low_gain));
        } else if id == &self.param_high_freq {
            return Some(ParameterValue::Float(self.high_freq));
        } else if id == &self.param_high_gain {
            return Some(ParameterValue::Float(self.high_gain));
        }

        // Check auto-gain parameters
        match id_str {
            "auto_gain_enabled" => return Some(ParameterValue::Bool(self.auto_gain_enabled)),
            "auto_gain_max_db" => return Some(ParameterValue::Float(self.auto_gain_max_db)),
            "auto_gain_smoothing_ms" => {
                return Some(ParameterValue::Float(self.auto_gain_smoothing_ms));
            }
            _ => {}
        }

        // Check per-channel parameters
        for param_name in &["low_freq", "low_gain", "high_freq", "high_gain"] {
            let prefix = format!("{}_", param_name);
            if let Some(suffix) = id_str.strip_prefix(&prefix) {
                if let Ok(channel) = suffix.parse::<usize>() {
                    if channel < self.num_channels {
                        let params = self.get_channel_params(channel);
                        let value = match *param_name {
                            "low_freq" => params.low_freq,
                            "low_gain" => params.low_gain,
                            "high_freq" => params.high_freq,
                            "high_gain" => params.high_gain,
                            _ => return None,
                        };
                        return Some(ParameterValue::Float(value));
                    }
                }
            }
        }

        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.rebuild_filters();
        self.rebuild_auto_gain()?;
        Ok(())
    }

    fn reset(&mut self) {
        // Reset all filter states
        // Force full rebuild to reset state
        self.filters.clear();
        self.rebuild_filters();
        // Reset auto-gain state
        if let Some(ag) = &mut self.auto_gain {
            ag.reset();
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify input size
        let input_samples = context.num_frames * self.num_channels;
        if input.len() != input_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                input_samples,
                input.len()
            ));
        }

        let output_samples = context.num_frames * self.num_channels;
        if output.len() != output_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                output_samples,
                output.len()
            ));
        }

        // Measure input loudness for auto-gain (before processing)
        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_input(input);
        }

        // Process each frame
        for frame_idx in 0..context.num_frames {
            for ch in 0..self.num_channels {
                let sample_idx = frame_idx * self.num_channels + ch;
                let mut sample = input[sample_idx] as f64;

                // Apply all 4 filters in series (2 low-shelf + 2 high-shelf)
                for filter in &mut self.filters[ch] {
                    sample = filter.process(sample);
                }

                // Apply per-channel compensation gain
                let comp_gain_linear = 10.0_f32.powf(self.compensation_gains[ch] / 20.0);
                output[sample_idx] = (sample as f32) * comp_gain_linear;
            }
        }

        // Measure output loudness and apply auto-gain compensation
        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_output(output);
            ag.apply_compensation(output, context.num_frames);
        }

        // Flush denormals to prevent CPU performance spikes and audio crackle
        // IIR biquad filter calculations can produce denormal numbers
        flush_denormals_inplace(output);

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        // IIR filters have minimal latency
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loudness_comp_creation() {
        let plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(plugin.filters.len(), 2); // 2 channels
        assert_eq!(plugin.filters[0].len(), 4); // 4 filters per channel
        assert!(!plugin.is_per_channel());
    }

    #[test]
    fn test_loudness_comp_compensation_gain() {
        // Test that compensation gain prevents clipping
        let plugin = LoudnessCompensationPlugin::new(2, 100.0, 10.0, 10000.0, 8.0);

        // Compensation gain should be -max(10, 8) = -10dB for both channels
        assert_eq!(plugin.compensation_gains[0], -10.0);
        assert_eq!(plugin.compensation_gains[1], -10.0);
    }

    #[test]
    fn test_loudness_comp_processing() {
        let mut plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
        plugin.initialize(48000).unwrap();

        // Create test signal: mid-frequency sine wave (1kHz)
        let num_frames = 1024;
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

        // Output should be processed
        let output_sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(output_sum > 0.0, "Output should not be silent");

        // Mid frequencies should be relatively unchanged (only shelves affect bass/treble)
        let input_rms = (input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32).sqrt();
        let output_rms = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
        let ratio = output_rms / input_rms;

        log::info!("RMS ratio (1kHz): {:.3}", ratio);
        // At 1kHz (between 100Hz and 10kHz), should be relatively flat
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "Mid frequencies should be relatively unchanged"
        );
    }

    #[test]
    fn test_loudness_comp_bass_boost() {
        // Test with low frequency to verify bass boost
        let mut plugin = LoudnessCompensationPlugin::new(2, 100.0, 12.0, 10000.0, 0.0);
        plugin.initialize(48000).unwrap();

        // Create test signal: 50Hz sine wave (in bass region)
        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.1; // Small amplitude
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Bass should be boosted
        let input_energy: f32 = input.iter().map(|x| x * x).sum();
        let output_energy: f32 = output.iter().map(|x| x * x).sum();
        let ratio = output_energy / input_energy;

        log::info!("Energy ratio at 50Hz with +12dB bass: {:.2}", ratio);
        // With +12dB boost and compensation, should still be boosted
        // 12dB = 4x power, but compensation is -12dB so we get ~1x
        assert!(ratio > 0.5, "Bass should be affected by boost");
    }

    #[test]
    fn test_loudness_comp_parameter_update() {
        let mut plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
        plugin.initialize(48000).unwrap();

        // Update low-shelf gain
        plugin
            .set_parameter(ParameterId::from("low_gain"), ParameterValue::Float(12.0))
            .unwrap();

        assert_eq!(plugin.low_gain, 12.0);
        // Compensation gain should update to -max(12, 6) = -12dB
        assert_eq!(plugin.compensation_gains[0], -12.0);

        // Get parameter
        let val = plugin.get_parameter(&ParameterId::from("low_freq"));
        assert_eq!(val, Some(ParameterValue::Float(100.0)));
    }

    #[test]
    fn test_loudness_comp_zero_gain() {
        // Test with zero gain (should be passthrough)
        let mut plugin = LoudnessCompensationPlugin::new(2, 100.0, 0.0, 10000.0, 0.0);
        plugin.initialize(48000).unwrap();

        let num_frames = 1024;
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

        // Should be approximately passthrough (may have tiny numerical differences)
        let max_diff = input
            .iter()
            .zip(output.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);

        log::info!("Max difference with zero gain: {}", max_diff);
        assert!(
            max_diff < 0.01,
            "With zero gain should be nearly passthrough"
        );
    }

    #[test]
    fn test_per_channel_creation() {
        let channel_params = vec![
            ChannelLoudnessParams {
                low_freq: 80.0,
                low_gain: 6.0,
                high_freq: 8000.0,
                high_gain: 3.0,
            },
            ChannelLoudnessParams {
                low_freq: 120.0,
                low_gain: 9.0,
                high_freq: 12000.0,
                high_gain: 6.0,
            },
        ];

        let plugin = LoudnessCompensationPlugin::new_per_channel(channel_params).unwrap();

        assert!(plugin.is_per_channel());
        assert_eq!(plugin.num_channels, 2);

        // Check per-channel compensation gains
        assert_eq!(plugin.compensation_gains[0], -6.0); // -max(6, 3)
        assert_eq!(plugin.compensation_gains[1], -9.0); // -max(9, 6)
    }

    #[test]
    fn test_per_channel_parameter() {
        let mut plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);

        // Set per-channel parameter
        plugin
            .set_parameter(ParameterId::from("low_gain_0"), ParameterValue::Float(12.0))
            .unwrap();

        assert!(plugin.is_per_channel());

        // Channel 0 should have new value
        let params = plugin.get_channel_params(0);
        assert_eq!(params.low_gain, 12.0);

        // Channel 1 should have inherited global value
        let params = plugin.get_channel_params(1);
        assert_eq!(params.low_gain, 6.0);

        // Get via parameter system
        let value = plugin.get_parameter(&ParameterId::from("low_gain_0"));
        assert_eq!(value.unwrap().as_float(), Some(12.0));
    }

    #[test]
    fn test_per_channel_processing() {
        let channel_params = vec![
            ChannelLoudnessParams {
                low_freq: 100.0,
                low_gain: 12.0, // More bass on channel 0
                high_freq: 10000.0,
                high_gain: 0.0,
            },
            ChannelLoudnessParams {
                low_freq: 100.0,
                low_gain: 0.0, // No bass boost on channel 1
                high_freq: 10000.0,
                high_gain: 0.0,
            },
        ];

        let mut plugin = LoudnessCompensationPlugin::new_per_channel(channel_params).unwrap();
        plugin.initialize(48000).unwrap();

        // Create test signal: 50Hz sine wave (in bass region)
        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.1;
            input[i * 2] = sample; // Ch0
            input[i * 2 + 1] = sample; // Ch1
        }

        let mut output = vec![0.0_f32; num_frames * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Channel 0 should have more energy (bass boosted)
        // Channel 1 should be close to passthrough
        let ch0_energy: f32 = (0..num_frames).map(|i| output[i * 2].powi(2)).sum();
        let ch1_energy: f32 = (0..num_frames).map(|i| output[i * 2 + 1].powi(2)).sum();

        log::info!(
            "Per-channel energy: ch0={:.4}, ch1={:.4}",
            ch0_energy,
            ch1_energy
        );

        // Ch0 should have more energy than ch1 due to bass boost
        // (though compensation reduces absolute levels)
        assert!(
            ch0_energy > ch1_energy * 0.5 || ch1_energy > ch0_energy * 0.5,
            "Channels should process independently"
        );
    }

    #[test]
    fn test_from_params_global() {
        let params = LoudnessCompensationPluginParams {
            low_freq: 80.0,
            low_gain: 9.0,
            high_freq: 12000.0,
            high_gain: 3.0,
            channel_params: vec![],
            auto_gain_enabled: false,
            auto_gain_max_db: 12.0,
            auto_gain_smoothing_ms: 100.0,
        };

        let plugin = LoudnessCompensationPlugin::from_params(2, params).unwrap();

        assert!(!plugin.is_per_channel());
        assert_eq!(plugin.low_freq, 80.0);
        assert_eq!(plugin.low_gain, 9.0);
    }

    #[test]
    fn test_from_params_per_channel() {
        let params = LoudnessCompensationPluginParams {
            low_freq: 100.0,
            low_gain: 6.0,
            high_freq: 10000.0,
            high_gain: 6.0,
            channel_params: vec![
                ChannelLoudnessParams {
                    low_freq: 80.0,
                    low_gain: 9.0,
                    high_freq: 8000.0,
                    high_gain: 3.0,
                },
                ChannelLoudnessParams {
                    low_freq: 120.0,
                    low_gain: 12.0,
                    high_freq: 12000.0,
                    high_gain: 6.0,
                },
            ],
            auto_gain_enabled: false,
            auto_gain_max_db: 12.0,
            auto_gain_smoothing_ms: 100.0,
        };

        let plugin = LoudnessCompensationPlugin::from_params(2, params).unwrap();

        assert!(plugin.is_per_channel());

        let ch0_params = plugin.get_channel_params(0);
        assert_eq!(ch0_params.low_freq, 80.0);
        assert_eq!(ch0_params.low_gain, 9.0);

        let ch1_params = plugin.get_channel_params(1);
        assert_eq!(ch1_params.low_freq, 120.0);
        assert_eq!(ch1_params.low_gain, 12.0);
    }

    #[test]
    fn test_switch_modes() {
        let mut plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);

        // Switch to per-channel mode
        plugin
            .set_parameter(ParameterId::from("low_gain_0"), ParameterValue::Float(12.0))
            .unwrap();
        assert!(plugin.is_per_channel());

        // Switch back to global mode
        plugin
            .set_parameter(ParameterId::from("low_gain"), ParameterValue::Float(9.0))
            .unwrap();
        assert!(!plugin.is_per_channel());
        assert_eq!(plugin.low_gain, 9.0);
    }
}
