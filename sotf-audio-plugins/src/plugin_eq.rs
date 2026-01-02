// ============================================================================
// Parametric EQ Plugin
// ============================================================================
//
// This plugin applies a chain of IIR biquad filters for parametric equalization.
// Supports two modes:
// 1. Single EQ applied to all channels (default)
// 2. Per-channel EQ with independent filter chains for each channel

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use math_audio_iir_fir::Biquad;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPluginParams {
    /// Single filter chain applied to all channels (default mode)
    #[serde(default)]
    pub filters: Vec<BiquadFilterConfig>,

    /// Per-channel filter chains (optional)
    /// If provided, must have exactly one Vec<BiquadFilterConfig> per channel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_filters: Option<Vec<Vec<BiquadFilterConfig>>>,
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

        Self {
            num_channels,
            filters: channel_filters,
            sample_rate: 48000, // Will be updated in initialize()
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

        Ok(Self {
            num_channels,
            filters: channel_filters,
            sample_rate: 48000, // Will be updated in initialize()
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
            "[EqPlugin] Creating EQ for {} channels, {}Hz",
            num_channels,
            sample_rate
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

            Self::new_per_channel(num_channels, channel_filters)
        }
        // Mode 2: Single filter chain for all channels
        else {
            let filters: Result<Vec<Biquad>, String> =
                params.filters.iter().map(config_to_biquad).collect();

            let filters = filters?;
            Ok(Self::new(num_channels, filters))
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
        // EQ parameters are managed externally via set_filters()
        // Could add per-filter gain controls here if needed
        vec![]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("EQ plugin has no adjustable parameters (use set_filters() instead)".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.set_sample_rate(sample_rate);
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

        // Process each frame
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

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        // IIR filters have minimal latency (essentially zero for practical purposes)
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
