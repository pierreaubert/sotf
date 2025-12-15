// ============================================================================
// Loudness Compensation Plugin
// ============================================================================
//
// This plugin provides loudness compensation using:
// - Low-shelf filter with 12dB/octave slope (2 cascaded biquads)
// - High-shelf filter with 12dB/octave slope (2 cascaded biquads)
// - Automatic gain compensation to prevent clipping
//
// Typical use: Boost bass and treble at low listening volumes to compensate
// for the Fletcher-Munson equal-loudness contours.

use super::param_specs::loudness_compensation::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use autoeq_iir::{Biquad, BiquadFilterType};
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

/// Configuration parameters for LoudnessCompensationPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessCompensationPluginParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
}

// ============================================================================
// Loudness Compensation Plugin
// ============================================================================

/// Loudness compensation plugin
pub struct LoudnessCompensationPlugin {
    /// Number of input/output channels
    num_channels: usize,

    /// Low-shelf frequency (Hz)
    param_low_freq: ParameterId,
    low_freq: f32,

    /// Low-shelf gain (dB)
    param_low_gain: ParameterId,
    low_gain: f32,

    /// High-shelf frequency (Hz)
    param_high_freq: ParameterId,
    high_freq: f32,

    /// High-shelf gain (dB)
    param_high_gain: ParameterId,
    high_gain: f32,

    /// Sample rate
    sample_rate: u32,

    /// Filters for each channel
    /// filters[channel][filter_idx] where filter_idx:
    /// 0-1: Low-shelf stages (2 for 12dB/oct)
    /// 2-3: High-shelf stages (2 for 12dB/oct)
    filters: Vec<Vec<Biquad>>,

    /// Compensation gain to prevent clipping
    compensation_gain: f32,
}

impl LoudnessCompensationPlugin {
    /// Create a new loudness compensation plugin
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
            sample_rate: 48000,
            filters: Vec::new(),
            compensation_gain: 0.0,
        };

        plugin.rebuild_filters();
        plugin
    }

    /// Create a new loudness compensation plugin from configuration parameters
    pub fn from_params(num_channels: usize, params: LoudnessCompensationPluginParams) -> Self {
        Self::new(
            num_channels,
            params.low_freq,
            params.low_gain,
            params.high_freq,
            params.high_gain,
        )
    }

    /// Rebuild all filters based on current parameters
    fn rebuild_filters(&mut self) {
        // Calculate compensation gain: -max(low_gain, high_gain)
        // This prevents clipping when both shelves boost
        self.compensation_gain = -self.low_gain.max(self.high_gain);

        // For 12dB/octave slope, we need 2 cascaded biquads (each is 6dB/oct)
        // Split the gain between the two stages
        let low_gain_per_stage = self.low_gain / 2.0;
        let high_gain_per_stage = self.high_gain / 2.0;

        // Q factor for shelving filters (0.707 = Butterworth response)
        let q = 0.707;

        self.filters.clear();
        for _ in 0..self.num_channels {
            let channel_filters = vec![
                // Low-shelf stage 1
                Biquad::new(
                    BiquadFilterType::Lowshelf,
                    self.low_freq as f64,
                    self.sample_rate as f64,
                    q,
                    low_gain_per_stage as f64,
                ),
                // Low-shelf stage 2
                Biquad::new(
                    BiquadFilterType::Lowshelf,
                    self.low_freq as f64,
                    self.sample_rate as f64,
                    q,
                    low_gain_per_stage as f64,
                ),
                // High-shelf stage 1
                Biquad::new(
                    BiquadFilterType::Highshelf,
                    self.high_freq as f64,
                    self.sample_rate as f64,
                    q,
                    high_gain_per_stage as f64,
                ),
                // High-shelf stage 2
                Biquad::new(
                    BiquadFilterType::Highshelf,
                    self.high_freq as f64,
                    self.sample_rate as f64,
                    q,
                    high_gain_per_stage as f64,
                ),
            ];

            self.filters.push(channel_filters);
        }
    }

    /// Update a parameter and rebuild filters if needed
    fn update_parameter(&mut self, id: &ParameterId, value: f32) -> bool {
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
            self.rebuild_filters();
        }

        changed
    }
}

impl Plugin for LoudnessCompensationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Loudness Compensation".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description:
                "Bass and treble boost for low-volume listening (Fletcher-Munson compensation)"
                    .to_string(),
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
            Parameter::new_float(
                "low_freq",
                "Low-shelf Frequency",
                self.low_freq,
                LOW_FREQ_MIN,
                LOW_FREQ_MAX,
            )
            .with_description("Frequency for bass boost (Hz)"),
            Parameter::new_float(
                "low_gain",
                "Low-shelf Gain",
                self.low_gain,
                LOW_GAIN_MIN,
                LOW_GAIN_MAX,
            )
            .with_description("Bass boost amount (dB)"),
            Parameter::new_float(
                "high_freq",
                "High-shelf Frequency",
                self.high_freq,
                HIGH_FREQ_MIN,
                HIGH_FREQ_MAX,
            )
            .with_description("Frequency for treble boost (Hz)"),
            Parameter::new_float(
                "high_gain",
                "High-shelf Gain",
                self.high_gain,
                HIGH_GAIN_MIN,
                HIGH_GAIN_MAX,
            )
            .with_description("Treble boost amount (dB)"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if let Some(val) = value.as_float()
            && self.update_parameter(&id, val)
        {
            return Ok(());
        }
        Err(format!("Unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_low_freq {
            Some(ParameterValue::Float(self.low_freq))
        } else if id == &self.param_low_gain {
            Some(ParameterValue::Float(self.low_gain))
        } else if id == &self.param_high_freq {
            Some(ParameterValue::Float(self.high_freq))
        } else if id == &self.param_high_gain {
            Some(ParameterValue::Float(self.high_gain))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.rebuild_filters();
        Ok(())
    }

    fn reset(&mut self) {
        // Reset all filter states
        self.rebuild_filters();
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

        // Calculate linear compensation gain
        let comp_gain_linear = 10.0_f32.powf(self.compensation_gain / 20.0);

        // Process each frame
        for frame_idx in 0..context.num_frames {
            for ch in 0..self.num_channels {
                let sample_idx = frame_idx * self.num_channels + ch;
                let mut sample = input[sample_idx] as f64;

                // Apply all 4 filters in series (2 low-shelf + 2 high-shelf)
                for filter in &mut self.filters[ch] {
                    sample = filter.process(sample);
                }

                // Apply compensation gain
                output[sample_idx] = (sample as f32) * comp_gain_linear;
            }
        }

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
    }

    #[test]
    fn test_loudness_comp_compensation_gain() {
        // Test that compensation gain prevents clipping
        let plugin = LoudnessCompensationPlugin::new(2, 100.0, 10.0, 10000.0, 8.0);

        // Compensation gain should be -max(10, 8) = -10dB
        assert_eq!(plugin.compensation_gain, -10.0);
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
        assert_eq!(plugin.compensation_gain, -12.0);

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
}
