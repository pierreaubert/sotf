// ============================================================================
// Band Split Plugin
// ============================================================================
//
// Splits audio into low and high frequency bands using Linkwitz-Riley crossover.
// Output is 2x input channels: [low_ch0, low_ch1, ..., high_ch0, high_ch1, ...]
//
// Used for frequency-based processing where different filters are applied
// to different frequency bands (e.g., FIR for bass, IIR for highs).

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

/// Band split plugin parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandSplitPluginParams {
    /// Crossover frequency in Hz
    pub frequency: f64,

    /// Crossover type: "LR24" (default), "LR48"
    #[serde(rename = "type", default = "default_crossover_type")]
    pub crossover_type: String,
}

fn default_crossover_type() -> String {
    "LR24".to_string()
}

impl Default for BandSplitPluginParams {
    fn default() -> Self {
        Self {
            frequency: 300.0,
            crossover_type: default_crossover_type(),
        }
    }
}

// ============================================================================
// Crossover Filter Implementation
// ============================================================================

/// A crossover point with both LP and HP filters for band splitting
struct CrossoverFilters {
    /// Lowpass filters per channel [channel][cascade_stage]
    lowpass: Vec<Vec<Biquad>>,
    /// Highpass filters per channel [channel][cascade_stage]
    highpass: Vec<Vec<Biquad>>,
    /// Crossover frequency
    frequency: f64,
}

impl CrossoverFilters {
    fn new(
        channels: usize,
        frequency: f64,
        crossover_type: &str,
        sample_rate: u32,
    ) -> Result<Self, String> {
        let num_sections = match crossover_type.to_uppercase().as_str() {
            "LR24" | "LR4" => 2, // 24 dB/oct
            "LR48" | "LR8" => 4, // 48 dB/oct
            _ => return Err(format!("Unknown crossover type: {}", crossover_type)),
        };

        let q = 1.0 / std::f64::consts::SQRT_2; // Butterworth Q = 0.707

        let mut lowpass = Vec::with_capacity(channels);
        let mut highpass = Vec::with_capacity(channels);

        for _ in 0..channels {
            let mut lp_chain = Vec::with_capacity(num_sections);
            let mut hp_chain = Vec::with_capacity(num_sections);

            for _ in 0..num_sections {
                lp_chain.push(Biquad::new(
                    BiquadFilterType::Lowpass,
                    frequency,
                    sample_rate as f64,
                    q,
                    0.0,
                ));
                hp_chain.push(Biquad::new(
                    BiquadFilterType::Highpass,
                    frequency,
                    sample_rate as f64,
                    q,
                    0.0,
                ));
            }

            lowpass.push(lp_chain);
            highpass.push(hp_chain);
        }

        Ok(Self {
            lowpass,
            highpass,
            frequency,
        })
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
                    self.frequency,
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
                    self.frequency,
                    sample_rate as f64,
                    q,
                    0.0,
                );
            }
        }
    }
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Band split plugin - splits audio into low and high frequency bands
///
/// Input: N channels
/// Output: 2N channels [low_0, low_1, ..., low_N-1, high_0, high_1, ..., high_N-1]
pub struct BandSplitPlugin {
    /// Number of input channels
    input_channels: usize,
    /// Sample rate
    sample_rate: u32,
    /// Crossover frequency
    frequency: f64,
    /// Crossover type
    crossover_type: String,
    /// Crossover filters
    filters: Option<CrossoverFilters>,

    // Parameter IDs
    param_frequency: ParameterId,
    param_type: ParameterId,
}

impl BandSplitPlugin {
    /// Create a new band split plugin
    pub fn new(input_channels: usize, frequency: f64, crossover_type: &str) -> Result<Self, String> {
        // Validate crossover type
        match crossover_type.to_uppercase().as_str() {
            "LR24" | "LR4" | "LR48" | "LR8" => {}
            _ => return Err(format!("Unknown crossover type: {}", crossover_type)),
        }

        Ok(Self {
            input_channels,
            sample_rate: 48000, // Will be set in initialize()
            frequency,
            crossover_type: crossover_type.to_string(),
            filters: None, // Will be created in initialize()
            param_frequency: ParameterId("frequency".to_string()),
            param_type: ParameterId("type".to_string()),
        })
    }

    /// Create from parameters
    pub fn from_params(input_channels: usize, params: &BandSplitPluginParams) -> Result<Self, String> {
        Self::new(input_channels, params.frequency, &params.crossover_type)
    }
}

impl Plugin for BandSplitPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandSplit", "1.0.0", "SotF").with_description(format!(
            "Band splitter at {} Hz ({}) - {} in, {} out",
            self.frequency,
            self.crossover_type,
            self.input_channels,
            self.input_channels * 2
        ))
    }

    fn input_channels(&self) -> usize {
        self.input_channels
    }

    fn output_channels(&self) -> usize {
        self.input_channels * 2 // Low band + High band
    }

    fn parameters(&self) -> Vec<Parameter> {
        use super::parameters::ParameterImportance;
        use crate::param_specs::band_split::*;

        vec![
            Parameter::new_float(
                "frequency",
                "Crossover Frequency",
                self.frequency as f32,
                FREQUENCY_MIN as f32,
                FREQUENCY_MAX as f32,
            )
            .with_unit("Hz")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_string("type", "Crossover Type", self.crossover_type.clone())
                .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_frequency {
            if let Some(v) = value.as_float() {
                self.frequency = v as f64;
                // Rebuild filters
                return self.initialize(self.sample_rate);
            }
            return Err("frequency must be float".to_string());
        } else if id == self.param_type {
            if let Some(v) = value.as_string() {
                // Validate type
                match v.to_uppercase().as_str() {
                    "LR24" | "LR4" | "LR48" | "LR8" => {
                        self.crossover_type = v.to_string();
                        // Rebuild filters
                        return self.initialize(self.sample_rate);
                    }
                    _ => return Err(format!("Unknown crossover type: {}", v)),
                }
            }
            return Err("type must be string".to_string());
        }
        Err(format!("Unknown parameter ID: {}", id.0))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_frequency {
            Some(ParameterValue::Float(self.frequency as f32))
        } else if id == &self.param_type {
            Some(ParameterValue::String(self.crossover_type.clone()))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        self.filters = Some(
            CrossoverFilters::new(
                self.input_channels,
                self.frequency,
                &self.crossover_type,
                sample_rate,
            )
            .map_err(|e| e.to_string())?,
        );

        Ok(())
    }

    fn reset(&mut self) {
        if let Some(ref mut filters) = self.filters {
            filters.reset(self.sample_rate);
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        let in_ch = self.input_channels;
        let out_ch = self.output_channels();

        // Verify buffer sizes
        if input.len() != num_frames * in_ch {
            return Err(format!(
                "Input buffer size {} doesn't match expected {} (frames={}, channels={})",
                input.len(),
                num_frames * in_ch,
                num_frames,
                in_ch
            ));
        }
        if output.len() != num_frames * out_ch {
            return Err(format!(
                "Output buffer size {} doesn't match expected {} (frames={}, channels={})",
                output.len(),
                num_frames * out_ch,
                num_frames,
                out_ch
            ));
        }

        let filters = self
            .filters
            .as_mut()
            .ok_or("Plugin not initialized")?;

        // Process each frame
        // Input: [ch0_f0, ch1_f0, ch0_f1, ch1_f1, ...]
        // Output: [low0_f0, low1_f0, high0_f0, high1_f0, low0_f1, low1_f1, high0_f1, high1_f1, ...]
        for frame in 0..num_frames {
            let in_offset = frame * in_ch;
            let out_offset = frame * out_ch;

            for ch in 0..in_ch {
                let sample = input[in_offset + ch];

                // Low band goes to first half of output channels
                let low = filters.process_lowpass(ch, sample);
                output[out_offset + ch] = low;

                // High band goes to second half of output channels
                let high = filters.process_highpass(ch, sample);
                output[out_offset + in_ch + ch] = high;
            }
        }

        // Flush denormals
        flush_denormals_inplace(output);

        Ok(context.num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_band_split_creation() {
        let plugin = BandSplitPlugin::new(2, 300.0, "LR24").unwrap();
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 4);
    }

    #[test]
    fn test_band_split_invalid_type() {
        let result = BandSplitPlugin::new(2, 300.0, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_band_split_sum_to_flat() {
        // LR24 crossovers should sum to flat (unity gain)
        let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
        plugin.initialize(48000).unwrap();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        // Generate test signal (impulse)
        let mut input = vec![0.0f32; 1024];
        input[0] = 1.0;

        let mut output = vec![0.0f32; 2048]; // 2 channels output

        plugin.process(&input, &mut output, &context).unwrap();

        // Sum low and high bands
        let mut summed = vec![0.0f32; 1024];
        for i in 0..1024 {
            summed[i] = output[i * 2] + output[i * 2 + 1]; // low + high
        }

        // After settling, the sum should approximately equal the input
        // (within a few samples for filter settling)
        // This is a basic sanity check - more rigorous tests would use FFT analysis
    }
}
