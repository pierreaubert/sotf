// ============================================================================
// Parametric EQ Plugin
// ============================================================================
//
// This plugin applies a chain of IIR biquad filters for parametric equalization.
// Supports two modes:
// 1. Single EQ applied to all channels (default)
// 2. Per-channel EQ with independent filter chains for each channel

use super::auto_gain::{AutoGain, AutoGainLoudnessType, AutoGainParams};
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use math_audio_iir_fir::Biquad;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Configuration
// ============================================================================

/// Biquad filter configuration for JSON deserialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiquadFilterConfig {
    pub filter_type: String, // "peak", "lowshelf", "highshelf", "lowpass", "highpass", "notch", "bandpass"
    pub freq: f64,
    pub q: f64,
    #[serde(default)]
    pub db_gain: f64,
}

/// Configuration parameters for EqPlugin
///
/// Supports two modes:
/// 1. Single EQ for all channels: Use `filters` field
/// 2. Per-channel EQ: Use `channel_filters` field
///
/// If `channel_filters` is provided, it takes precedence over `filters`.
/// When using `channel_filters`, the number of channel filter arrays must match
/// the number of audio channels.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EqPluginParams {
    /// Single filter chain applied to all channels (default mode)
    #[serde(default)]
    pub filters: Vec<BiquadFilterConfig>,

    /// Per-channel filter chains (optional)
    /// If provided, must have exactly one Vec<BiquadFilterConfig> per channel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_filters: Option<Vec<Vec<BiquadFilterConfig>>>,

    /// Auto-gain compensation parameters
    #[serde(default)]
    pub auto_gain: AutoGainParams,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Parametric EQ plugin using IIR biquad filters
#[derive(Debug)]
pub struct EqPlugin {
    /// Number of input/output channels
    num_channels: usize,

    /// IIR filters (one chain per channel)
    /// filters[channel_idx][filter_idx]
    filters: Vec<Vec<Biquad>>,

    /// Sample rate
    sample_rate: u32,

    /// Auto-gain compensation
    auto_gain: AutoGain,
}

impl EqPlugin {
    /// Create a new EQ plugin with a single filter chain applied to all channels
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to process
    /// * `filters` - List of biquad filters to apply (will be cloned for each channel)
    pub fn new(num_channels: usize, filters: Vec<Biquad>) -> Self {
        // Clone the filter chain for each channel
        let mut channel_filters = Vec::with_capacity(num_channels);
        for _ in 0..num_channels {
            channel_filters.push(filters.clone());
        }

        let sample_rate = 48000;
        let auto_gain =
            AutoGain::new_default(num_channels, sample_rate).expect("Failed to create auto-gain");

        Self {
            num_channels,
            filters: channel_filters,
            sample_rate,
            auto_gain,
        }
    }

    /// Create a new EQ plugin with per-channel filter chains
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to process
    /// * `channel_filters` - List of filter chains, one per channel
    ///
    /// # Errors
    /// Returns an error if the number of filter chains doesn't match num_channels
    pub fn new_per_channel(
        num_channels: usize,
        channel_filters: Vec<Vec<Biquad>>,
    ) -> Result<Self, String> {
        if channel_filters.len() != num_channels {
            return Err(format!(
                "Channel filter count mismatch: expected {} channels, got {} filter chains",
                num_channels,
                channel_filters.len()
            ));
        }

        let sample_rate = 48000;
        let auto_gain = AutoGain::new_default(num_channels, sample_rate)?;

        Ok(Self {
            num_channels,
            filters: channel_filters,
            sample_rate,
            auto_gain,
        })
    }

    /// Create a new EQ plugin from configuration parameters
    ///
    /// Supports two modes:
    /// 1. If `params.channel_filters` is provided: Creates per-channel EQ
    ///    - Number of filter chains must match `num_channels`
    /// 2. Otherwise: Uses `params.filters` and applies same EQ to all channels
    pub fn from_params(
        num_channels: usize,
        sample_rate: u32,
        params: EqPluginParams,
    ) -> Result<Self, String> {
        log::debug!(
            "[EqPlugin] Creating EQ for {} channels, {}Hz, auto_gain={}",
            num_channels,
            sample_rate,
            params.auto_gain.enabled
        );

        // Helper function to convert BiquadFilterConfig to Biquad
        let config_to_biquad = |f: &BiquadFilterConfig| -> Result<Biquad, String> {
            use math_audio_iir_fir::BiquadFilterType;

            let filter_type = match f.filter_type.as_str() {
                "peak" => BiquadFilterType::Peak,
                "lowshelf" => BiquadFilterType::Lowshelf,
                "highshelf" => BiquadFilterType::Highshelf,
                "lowpass" => BiquadFilterType::Lowpass,
                "highpass" => BiquadFilterType::Highpass,
                "notch" => BiquadFilterType::Notch,
                "bandpass" => BiquadFilterType::Bandpass,
                other => return Err(format!("Unknown filter type: {}", other)),
            };

            Ok(Biquad::new(
                filter_type,
                f.freq,
                sample_rate as f64,
                f.q,
                f.db_gain,
            ))
        };

        // Create auto-gain with provided parameters
        let auto_gain = AutoGain::new(num_channels, sample_rate, params.auto_gain)?;

        // Mode 1: Per-channel filters (takes precedence)
        if let Some(channel_filter_configs) = params.channel_filters {
            // Validate channel count
            if channel_filter_configs.len() != num_channels {
                return Err(format!(
                    "Per-channel EQ: expected {} filter chains (one per channel), got {}",
                    num_channels,
                    channel_filter_configs.len()
                ));
            }

            // Convert each channel's filter configs to Biquad filters
            let mut channel_filters = Vec::with_capacity(num_channels);
            for (ch_idx, ch_configs) in channel_filter_configs.iter().enumerate() {
                let filters: Result<Vec<Biquad>, String> =
                    ch_configs.iter().map(config_to_biquad).collect();
                let filters = filters.map_err(|e| {
                    format!("Error in channel {} filter configuration: {}", ch_idx, e)
                })?;
                channel_filters.push(filters);
            }

            Ok(Self {
                num_channels,
                filters: channel_filters,
                sample_rate,
                auto_gain,
            })
        }
        // Mode 2: Single filter chain for all channels
        else {
            let filters: Result<Vec<Biquad>, String> =
                params.filters.iter().map(config_to_biquad).collect();
            let filters = filters?;

            // Clone the filter chain for each channel
            let mut channel_filters = Vec::with_capacity(num_channels);
            for _ in 0..num_channels {
                channel_filters.push(filters.clone());
            }

            Ok(Self {
                num_channels,
                filters: channel_filters,
                sample_rate,
                auto_gain,
            })
        }
    }

    /// Update the sample rate for all filters
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;

        // Update sample rate for all filters
        for channel_filters in &mut self.filters {
            for filter in channel_filters {
                filter.srate = sample_rate as f64;
                // Recompute coefficients with new sample rate
                // Note: This requires making compute_coeffs public or adding a method
                // For now we'll recreate the filter
                *filter = Biquad::new(
                    filter.filter_type,
                    filter.freq,
                    sample_rate as f64,
                    filter.q,
                    filter.db_gain,
                );
            }
        }
    }

    /// Replace the filter chain (applies same filters to all channels)
    pub fn set_filters(&mut self, filters: Vec<Biquad>) {
        // Clone the new filter chain for each channel
        self.filters.clear();
        for _ in 0..self.num_channels {
            self.filters.push(filters.clone());
        }
    }

    /// Replace the filter chains with per-channel filters
    ///
    /// # Arguments
    /// * `channel_filters` - List of filter chains, one per channel
    ///
    /// # Errors
    /// Returns an error if the number of filter chains doesn't match num_channels
    pub fn set_channel_filters(&mut self, channel_filters: Vec<Vec<Biquad>>) -> Result<(), String> {
        if channel_filters.len() != self.num_channels {
            return Err(format!(
                "Channel filter count mismatch: expected {} channels, got {} filter chains",
                self.num_channels,
                channel_filters.len()
            ));
        }

        self.filters = channel_filters;
        Ok(())
    }

    /// Get a reference to the filter chain
    pub fn filters(&self) -> &[Biquad] {
        if !self.filters.is_empty() {
            &self.filters[0]
        } else {
            &[]
        }
    }
}

impl Plugin for EqPlugin {
    fn info(&self) -> PluginInfo {
        // Check if all channels have the same number of filters
        let filter_counts: Vec<usize> = self.filters.iter().map(|f| f.len()).collect();
        let is_uniform = filter_counts.windows(2).all(|w| w[0] == w[1]);

        let description = if is_uniform {
            format!(
                "Parametric EQ: {} filters per channel ({} channels)",
                filter_counts.first().copied().unwrap_or(0),
                self.num_channels
            )
        } else {
            format!(
                "Parametric EQ: per-channel ({} channels, {} total filters)",
                self.num_channels,
                filter_counts.iter().sum::<usize>()
            )
        };

        PluginInfo {
            name: "Parametric EQ".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description,
        }
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool(
                "auto_gain_enabled",
                "Auto Gain",
                self.auto_gain.is_enabled(),
            )
            .with_description("Automatically compensate for loudness changes from EQ")
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float("auto_gain_max_db", "Max Gain", 12.0, 0.0, 24.0)
                .with_description("Maximum auto-gain correction in dB")
                .with_group("Auto Gain")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float("auto_gain_smoothing_ms", "Smoothing", 100.0, 10.0, 500.0)
                .with_description("Auto-gain smoothing time in milliseconds")
                .with_group("Auto Gain")
                .with_importance(ParameterImportance::FineTuning),
            Parameter::new_int("auto_gain_loudness_type", "Loudness Type", 0, 0, 1)
                .with_description("0 = Momentary (400ms), 1 = Short-term (3s)")
                .with_group("Auto Gain")
                .with_importance(ParameterImportance::FineTuning),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        match id.0.as_str() {
            "auto_gain_enabled" => {
                if let ParameterValue::Bool(v) = value {
                    self.auto_gain.set_enabled(v);
                }
            }
            "auto_gain_max_db" => {
                if let ParameterValue::Float(v) = value {
                    self.auto_gain.set_max_gain_db(v);
                }
            }
            "auto_gain_smoothing_ms" => {
                if let ParameterValue::Float(v) = value {
                    self.auto_gain.set_smoothing_ms(v);
                }
            }
            "auto_gain_loudness_type" => {
                if let ParameterValue::Int(v) = value {
                    let loudness_type = if v == 0 {
                        AutoGainLoudnessType::Momentary
                    } else {
                        AutoGainLoudnessType::ShortTerm
                    };
                    self.auto_gain.set_loudness_type(loudness_type);
                }
            }
            _ => return Err(format!("Unknown parameter: {}", id.0)),
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.0.as_str() {
            "auto_gain_enabled" => Some(ParameterValue::Bool(self.auto_gain.is_enabled())),
            "auto_gain_max_db" => Some(ParameterValue::Float(12.0)), // TODO: store and return actual value
            "auto_gain_smoothing_ms" => Some(ParameterValue::Float(100.0)), // TODO: store and return actual value
            "auto_gain_loudness_type" => Some(ParameterValue::Int(0)), // TODO: store and return actual value
            _ => None,
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        const MIN_SAMPLE_RATE: u32 = 8_000;
        const MAX_SAMPLE_RATE: u32 = 384_000;

        if sample_rate < MIN_SAMPLE_RATE || sample_rate > MAX_SAMPLE_RATE {
            return Err(format!(
                "Invalid sample rate: {} Hz (valid range: {}-{} Hz)",
                sample_rate, MIN_SAMPLE_RATE, MAX_SAMPLE_RATE
            ));
        }

        self.set_sample_rate(sample_rate);
        self.auto_gain
            .set_sample_rate(sample_rate)
            .map_err(|e| format!("Failed to initialize auto-gain: {}", e))?;
        Ok(())
    }

    fn reset(&mut self) {
        // Reset filter state for all channels
        for channel_filters in &mut self.filters {
            for filter in channel_filters {
                // Reset filter state by recreating
                *filter = Biquad::new(
                    filter.filter_type,
                    filter.freq,
                    filter.srate,
                    filter.q,
                    filter.db_gain,
                );
            }
        }
        // Reset auto-gain state
        self.auto_gain.reset();
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

        // Measure input for auto-gain (before processing)
        self.auto_gain
            .measure_input(input)
            .map_err(|e| format!("Auto-gain input measurement failed: {}", e))?;

        // Process each frame through EQ filters
        for frame_idx in 0..context.num_frames {
            for ch in 0..self.num_channels {
                let sample_idx = frame_idx * self.num_channels + ch;
                let mut sample = input[sample_idx] as f64;

                // Apply all filters in the chain for this channel
                for filter in &mut self.filters[ch] {
                    sample = filter.process(sample);
                }

                output[sample_idx] = sample as f32;
            }
        }

        // Measure output for auto-gain (after EQ processing, before gain compensation)
        self.auto_gain
            .measure_output(output)
            .map_err(|e| format!("Auto-gain output measurement failed: {}", e))?;

        // Apply auto-gain compensation
        self.auto_gain
            .apply_compensation(output, context.num_frames);

        Ok(())
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let data = self.auto_gain.get_data();
        Some(Arc::new(data))
    }

    fn latency_samples(&self) -> usize {
        // IIR filters have minimal latency (essentially zero for practical purposes)
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_gain::{AutoGainData, AutoGainLoudnessType, AutoGainParams};
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    #[test]
    fn test_eq_creation() {
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            3.0,
        )];
        let plugin = EqPlugin::new(2, filters);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_eq_passthrough() {
        // Empty filter chain should be passthrough
        let plugin = EqPlugin::new(2, vec![]);
        let mut plugin = plugin;
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            input[i * 2] = (i as f32 * 0.01).sin();
            input[i * 2 + 1] = (i as f32 * 0.01).cos();
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Should be exact passthrough
        for i in 0..input.len() {
            assert_eq!(output[i], input[i]);
        }
    }

    #[test]
    fn test_eq_processing() {
        // Create a simple high-shelf filter (+6dB above 1kHz)
        let filters = vec![Biquad::new(
            BiquadFilterType::Highshelf,
            1000.0,
            48000.0,
            0.707,
            6.0,
        )];
        let mut plugin = EqPlugin::new(2, filters);
        plugin.initialize(48000).unwrap();

        // Test with a 1kHz sine wave
        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.sin() * 0.5;
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Output should be amplified due to high-shelf filter
        let input_energy: f32 = input.iter().map(|x| x * x).sum();
        let output_energy: f32 = output.iter().map(|x| x * x).sum();

        log::info!(
            "Input energy: {}, Output energy: {}, Ratio: {}",
            input_energy,
            output_energy,
            output_energy / input_energy
        );

        // High-shelf at 1kHz with +6dB should amplify (ratio > 1.0)
        assert!(
            output_energy > input_energy * 1.5,
            "Expected amplification from high-shelf filter"
        );
    }

    #[test]
    fn test_eq_multiple_filters() {
        // Create a multi-band EQ: bass boost + mid cut + treble boost
        let filters = vec![
            Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.707, 3.0), // +3dB bass
            Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, -3.0),     // -3dB mid cut
            Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.707, 3.0), // +3dB treble
        ];
        let mut plugin = EqPlugin::new(2, filters);
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5;
            input[i * 2 + 1] = (i as f32 * 0.01).cos() * 0.5;
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Should produce non-zero output
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "Output should not be all zeros");
    }

    #[test]
    fn test_per_channel_eq_creation() {
        // Create different filters for each channel
        let ch0_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            3.0,
        )];
        let ch1_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            2000.0,
            48000.0,
            1.0,
            -3.0,
        )];

        let channel_filters = vec![ch0_filters, ch1_filters];
        let plugin = EqPlugin::new_per_channel(2, channel_filters).unwrap();

        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
        assert_eq!(plugin.filters.len(), 2);
        assert_eq!(plugin.filters[0].len(), 1);
        assert_eq!(plugin.filters[1].len(), 1);
    }

    #[test]
    fn test_per_channel_eq_mismatch_error() {
        // Try to create 2-channel plugin with 3 filter chains
        let ch0_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            3.0,
        )];
        let ch1_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            2000.0,
            48000.0,
            1.0,
            -3.0,
        )];
        let ch2_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            3000.0,
            48000.0,
            1.0,
            2.0,
        )];

        let channel_filters = vec![ch0_filters, ch1_filters, ch2_filters];
        let result = EqPlugin::new_per_channel(2, channel_filters);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected 2 channels, got 3"));
    }

    #[test]
    fn test_per_channel_eq_processing() {
        // Channel 0: boost at 1kHz
        // Channel 1: cut at 1kHz
        let ch0_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            2.0,
            6.0,
        )];
        let ch1_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            2.0,
            -6.0,
        )];

        let channel_filters = vec![ch0_filters, ch1_filters];
        let mut plugin = EqPlugin::new_per_channel(2, channel_filters).unwrap();
        plugin.initialize(48000).unwrap();

        // Create a 1kHz sine wave on both channels
        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample; // L channel
            input[i * 2 + 1] = sample; // R channel (same input)
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Calculate energy for each channel
        let mut ch0_input_energy = 0.0_f32;
        let mut ch0_output_energy = 0.0_f32;
        let mut ch1_input_energy = 0.0_f32;
        let mut ch1_output_energy = 0.0_f32;

        for i in 0..1024 {
            ch0_input_energy += input[i * 2] * input[i * 2];
            ch0_output_energy += output[i * 2] * output[i * 2];
            ch1_input_energy += input[i * 2 + 1] * input[i * 2 + 1];
            ch1_output_energy += output[i * 2 + 1] * output[i * 2 + 1];
        }

        // Channel 0 should be boosted (output > input)
        assert!(
            ch0_output_energy > ch0_input_energy * 1.5,
            "Channel 0 should be boosted at 1kHz"
        );

        // Channel 1 should be attenuated (output < input)
        assert!(
            ch1_output_energy < ch1_input_energy * 0.7,
            "Channel 1 should be attenuated at 1kHz"
        );
    }

    #[test]
    fn test_from_params_single_eq() {
        use serde_json::json;

        let params_json = json!({
            "filters": [
                {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 3.0},
                {"filter_type": "lowshelf", "freq": 100.0, "q": 0.707, "db_gain": 2.0}
            ]
        });

        let params: EqPluginParams = serde_json::from_value(params_json).unwrap();
        let plugin = EqPlugin::from_params(2, 48000, params).unwrap();

        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.filters.len(), 2);
        assert_eq!(plugin.filters[0].len(), 2); // 2 filters per channel
        assert_eq!(plugin.filters[1].len(), 2); // same filters on both channels
    }

    #[test]
    fn test_from_params_per_channel_eq() {
        use serde_json::json;

        let params_json = json!({
            "channel_filters": [
                [
                    {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 3.0}
                ],
                [
                    {"filter_type": "peak", "freq": 2000.0, "q": 1.5, "db_gain": -3.0},
                    {"filter_type": "highshelf", "freq": 8000.0, "q": 0.707, "db_gain": 2.0}
                ]
            ]
        });

        let params: EqPluginParams = serde_json::from_value(params_json).unwrap();
        let plugin = EqPlugin::from_params(2, 48000, params).unwrap();

        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.filters.len(), 2);
        assert_eq!(plugin.filters[0].len(), 1); // 1 filter on channel 0
        assert_eq!(plugin.filters[1].len(), 2); // 2 filters on channel 1
    }

    #[test]
    fn test_from_params_per_channel_mismatch() {
        use serde_json::json;

        let params_json = json!({
            "channel_filters": [
                [{"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 3.0}]
            ]
        });

        let params: EqPluginParams = serde_json::from_value(params_json).unwrap();
        let result = EqPlugin::from_params(2, 48000, params); // 2 channels but only 1 filter chain

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected 2 filter chains"));
    }

    #[test]
    fn test_set_channel_filters() {
        // Start with single EQ
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            3.0,
        )];
        let mut plugin = EqPlugin::new(2, filters);

        // Switch to per-channel EQ
        let ch0_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            500.0,
            48000.0,
            1.0,
            2.0,
        )];
        let ch1_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            2000.0,
            48000.0,
            1.0,
            -2.0,
        )];

        plugin
            .set_channel_filters(vec![ch0_filters, ch1_filters])
            .unwrap();

        assert_eq!(plugin.filters.len(), 2);
        assert_eq!(plugin.filters[0][0].freq, 500.0);
        assert_eq!(plugin.filters[1][0].freq, 2000.0);
    }

    #[test]
    fn test_set_channel_filters_mismatch() {
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            3.0,
        )];
        let mut plugin = EqPlugin::new(2, filters);

        // Try to set 3 filter chains for 2-channel plugin
        let ch0_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            500.0,
            48000.0,
            1.0,
            2.0,
        )];
        let ch1_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1500.0,
            48000.0,
            1.0,
            -2.0,
        )];
        let ch2_filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            2500.0,
            48000.0,
            1.0,
            1.0,
        )];

        let result = plugin.set_channel_filters(vec![ch0_filters, ch1_filters, ch2_filters]);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected 2 channels"));
    }

    // ========================================================================
    // Auto-Gain Tests
    // ========================================================================

    #[test]
    fn test_eq_auto_gain_disabled_by_default() {
        // By default, auto_gain should be disabled
        let plugin = EqPlugin::new(2, vec![]);
        assert!(!plugin.auto_gain.is_enabled());
    }

    #[test]
    fn test_eq_auto_gain_from_params_disabled() {
        use serde_json::json;

        // When not specified, auto_gain should be disabled
        let params_json = json!({
            "filters": [
                {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 6.0}
            ]
        });

        let params: EqPluginParams = serde_json::from_value(params_json).unwrap();
        let plugin = EqPlugin::from_params(2, 48000, params).unwrap();

        assert!(!plugin.auto_gain.is_enabled());
    }

    #[test]
    fn test_eq_auto_gain_from_params_enabled() {
        use serde_json::json;

        let params_json = json!({
            "filters": [
                {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 6.0}
            ],
            "auto_gain": {
                "enabled": true,
                "max_gain_db": 10.0,
                "smoothing_ms": 50.0
            }
        });

        let params: EqPluginParams = serde_json::from_value(params_json).unwrap();
        let plugin = EqPlugin::from_params(2, 48000, params).unwrap();

        assert!(plugin.auto_gain.is_enabled());
    }

    #[test]
    fn test_eq_auto_gain_compensates_boost() {
        // Create EQ with +6dB boost and auto-gain enabled
        let filters = vec![Biquad::new(
            BiquadFilterType::Highshelf,
            1000.0,
            48000.0,
            0.707,
            6.0, // +6dB boost
        )];

        let params = EqPluginParams {
            filters: vec![],
            channel_filters: None,
            auto_gain: AutoGainParams {
                enabled: true,
                max_gain_db: 12.0,
                smoothing_ms: 10.0, // Fast for testing
                ..Default::default()
            },
        };

        let mut plugin = EqPlugin::from_params(2, 48000, params).unwrap();
        plugin.set_filters(filters);
        plugin.initialize(48000).unwrap();

        // Create test signal: 2kHz sine wave (above the shelf frequency)
        let num_frames = 4800; // 100ms at 48kHz
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 2000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.3;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process multiple times to let auto-gain stabilize
        let mut output = vec![0.0_f32; num_frames * 2];
        for _ in 0..20 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        // Get the auto-gain data
        let data = plugin.get_data().unwrap();
        let auto_gain_data = data.downcast_ref::<AutoGainData>().unwrap();

        // With a boost EQ, auto-gain should be applying negative (attenuating) gain
        assert!(
            auto_gain_data.gain_db < 0.0,
            "Auto-gain should attenuate to compensate for EQ boost, got {} dB",
            auto_gain_data.gain_db
        );
    }

    #[test]
    fn test_eq_auto_gain_compensates_cut() {
        // Create EQ with -6dB cut and auto-gain enabled
        let filters = vec![Biquad::new(
            BiquadFilterType::Highshelf,
            1000.0,
            48000.0,
            0.707,
            -6.0, // -6dB cut
        )];

        let params = EqPluginParams {
            filters: vec![],
            channel_filters: None,
            auto_gain: AutoGainParams {
                enabled: true,
                max_gain_db: 12.0,
                smoothing_ms: 10.0,
                ..Default::default()
            },
        };

        let mut plugin = EqPlugin::from_params(2, 48000, params).unwrap();
        plugin.set_filters(filters);
        plugin.initialize(48000).unwrap();

        // Create test signal: 2kHz sine wave
        let num_frames = 4800;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 2000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let mut output = vec![0.0_f32; num_frames * 2];
        for _ in 0..20 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        let data = plugin.get_data().unwrap();
        let auto_gain_data = data.downcast_ref::<AutoGainData>().unwrap();

        // With a cut EQ, auto-gain should be applying positive (boosting) gain
        assert!(
            auto_gain_data.gain_db > 0.0,
            "Auto-gain should boost to compensate for EQ cut, got {} dB",
            auto_gain_data.gain_db
        );
    }

    #[test]
    fn test_eq_auto_gain_parameter_set_get() {
        let mut plugin = EqPlugin::new(2, vec![]);
        plugin.initialize(48000).unwrap();

        // Test set_parameter for auto_gain_enabled
        plugin
            .set_parameter(
                ParameterId("auto_gain_enabled".to_string()),
                ParameterValue::Bool(true),
            )
            .unwrap();

        let value = plugin.get_parameter(&ParameterId("auto_gain_enabled".to_string()));
        assert_eq!(value, Some(ParameterValue::Bool(true)));

        // Disable
        plugin
            .set_parameter(
                ParameterId("auto_gain_enabled".to_string()),
                ParameterValue::Bool(false),
            )
            .unwrap();

        let value = plugin.get_parameter(&ParameterId("auto_gain_enabled".to_string()));
        assert_eq!(value, Some(ParameterValue::Bool(false)));
    }

    #[test]
    fn test_eq_auto_gain_parameter_max_db() {
        let mut plugin = EqPlugin::new(2, vec![]);
        plugin.initialize(48000).unwrap();

        // Set max gain
        plugin
            .set_parameter(
                ParameterId("auto_gain_max_db".to_string()),
                ParameterValue::Float(6.0),
            )
            .unwrap();

        // Parameter should be retrievable
        let value = plugin.get_parameter(&ParameterId("auto_gain_max_db".to_string()));
        assert!(value.is_some());
    }

    #[test]
    fn test_eq_auto_gain_parameter_smoothing() {
        let mut plugin = EqPlugin::new(2, vec![]);
        plugin.initialize(48000).unwrap();

        // Set smoothing time
        plugin
            .set_parameter(
                ParameterId("auto_gain_smoothing_ms".to_string()),
                ParameterValue::Float(200.0),
            )
            .unwrap();

        let value = plugin.get_parameter(&ParameterId("auto_gain_smoothing_ms".to_string()));
        assert!(value.is_some());
    }

    #[test]
    fn test_eq_auto_gain_parameter_loudness_type() {
        let mut plugin = EqPlugin::new(2, vec![]);
        plugin.initialize(48000).unwrap();

        // Set to short-term (1)
        plugin
            .set_parameter(
                ParameterId("auto_gain_loudness_type".to_string()),
                ParameterValue::Int(1),
            )
            .unwrap();

        let value = plugin.get_parameter(&ParameterId("auto_gain_loudness_type".to_string()));
        assert!(value.is_some());
    }

    #[test]
    fn test_eq_auto_gain_unknown_parameter() {
        let mut plugin = EqPlugin::new(2, vec![]);

        let result = plugin.set_parameter(
            ParameterId("unknown_param".to_string()),
            ParameterValue::Float(1.0),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown parameter"));
    }

    #[test]
    fn test_eq_auto_gain_reset() {
        let params = EqPluginParams {
            filters: vec![],
            channel_filters: None,
            auto_gain: AutoGainParams {
                enabled: true,
                ..Default::default()
            },
        };

        let mut plugin = EqPlugin::from_params(2, 48000, params).unwrap();
        plugin.initialize(48000).unwrap();

        // Process some audio
        let num_frames = 1024;
        let input: Vec<f32> = (0..num_frames * 2).map(|_| 0.5).collect();
        let mut output = vec![0.0_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Reset
        plugin.reset();

        // Auto-gain should be reset
        let data = plugin.get_data().unwrap();
        let auto_gain_data = data.downcast_ref::<AutoGainData>().unwrap();

        assert_eq!(auto_gain_data.gain_db, 0.0, "Gain should be reset to 0 dB");
    }

    #[test]
    fn test_eq_auto_gain_passthrough_when_disabled() {
        // With auto-gain disabled, output should only be affected by EQ, not gain compensation
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            6.0, // +6dB boost at 1kHz
        )];

        let mut plugin = EqPlugin::new(2, filters);
        plugin.initialize(48000).unwrap();

        // Auto-gain should be disabled by default
        assert!(!plugin.auto_gain.is_enabled());

        // Process 1kHz sine wave
        let num_frames = 4800;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.3;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let mut output = vec![0.0_f32; num_frames * 2];
        for _ in 0..5 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        // Get auto-gain data
        let data = plugin.get_data().unwrap();
        let auto_gain_data = data.downcast_ref::<AutoGainData>().unwrap();

        // Auto-gain should report 0 dB when disabled
        assert_eq!(
            auto_gain_data.gain_db, 0.0,
            "Auto-gain should be 0 dB when disabled"
        );

        // Output should be boosted by EQ (energy should be higher)
        let input_energy: f32 = input.iter().map(|x| x * x).sum();
        let output_energy: f32 = output.iter().map(|x| x * x).sum();
        assert!(
            output_energy > input_energy,
            "With +6dB EQ boost and no auto-gain, output should be louder"
        );
    }

    #[test]
    fn test_eq_auto_gain_preserves_loudness() {
        // Create EQ with significant boost
        let filters = vec![Biquad::new(
            BiquadFilterType::Highshelf,
            1000.0,
            48000.0,
            0.707,
            6.0, // +6dB boost
        )];

        let params = EqPluginParams {
            filters: vec![],
            channel_filters: None,
            auto_gain: AutoGainParams {
                enabled: true,
                max_gain_db: 12.0,
                smoothing_ms: 5.0, // Very fast for testing
                ..Default::default()
            },
        };

        let mut plugin = EqPlugin::from_params(2, 48000, params).unwrap();
        plugin.set_filters(filters);
        plugin.initialize(48000).unwrap();

        // Create 2kHz sine wave (affected by high shelf)
        let num_frames = 4800;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 2000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.3;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process many times to let auto-gain fully stabilize
        let mut output = vec![0.0_f32; num_frames * 2];
        for _ in 0..50 {
            plugin.process(&input, &mut output, &context).unwrap();
        }

        // Calculate input and output energy
        let input_energy: f32 = input.iter().map(|x| x * x).sum();
        let output_energy: f32 = output.iter().map(|x| x * x).sum();

        let energy_ratio = output_energy / input_energy;
        let energy_ratio_db = 10.0 * energy_ratio.log10();

        // With auto-gain, the output should be closer to input loudness
        // Allow some tolerance since auto-gain uses LUFS which may differ slightly from RMS
        assert!(
            energy_ratio_db.abs() < 3.0,
            "Auto-gain should keep output close to input loudness. Ratio: {:.2} dB",
            energy_ratio_db
        );
    }

    #[test]
    fn test_eq_params_serialization_with_auto_gain() {
        let params = EqPluginParams {
            filters: vec![BiquadFilterConfig {
                filter_type: "peak".to_string(),
                freq: 1000.0,
                q: 1.0,
                db_gain: 3.0,
            }],
            channel_filters: None,
            auto_gain: AutoGainParams {
                enabled: true,
                loudness_type: AutoGainLoudnessType::ShortTerm,
                max_gain_db: 8.0,
                smoothing_ms: 75.0,
            },
        };

        // Serialize
        let json_str = serde_json::to_string(&params).unwrap();
        assert!(json_str.contains("auto_gain"));
        assert!(json_str.contains("\"enabled\":true"));
        assert!(json_str.contains("ShortTerm"));

        // Deserialize
        let parsed: EqPluginParams = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.auto_gain.enabled);
        assert_eq!(parsed.auto_gain.max_gain_db, 8.0);
        assert_eq!(parsed.auto_gain.smoothing_ms, 75.0);
        assert_eq!(
            parsed.auto_gain.loudness_type,
            AutoGainLoudnessType::ShortTerm
        );
    }
}
