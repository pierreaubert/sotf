// ============================================================================
// Band Merge Plugin
// ============================================================================
//
// Merges split frequency bands back together by summing.
// Input is 2x output channels: [low_ch0, low_ch1, ..., high_ch0, high_ch1, ...]
// Output is: [ch0, ch1, ...] where each channel is sum of low and high bands.
//
// Used with BandSplitPlugin for frequency-based processing.

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

/// Band merge plugin parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandMergePluginParams {
    /// Number of bands to merge (default: 2)
    #[serde(default = "default_num_bands")]
    pub bands: usize,
}

fn default_num_bands() -> usize {
    2
}

impl Default for BandMergePluginParams {
    fn default() -> Self {
        Self {
            bands: default_num_bands(),
        }
    }
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Band merge plugin - merges split frequency bands back together
///
/// Input: N * bands channels [band0_ch0, band0_ch1, ..., band1_ch0, band1_ch1, ...]
/// Output: N channels [ch0, ch1, ...] where each is the sum of all bands
pub struct BandMergePlugin {
    /// Number of output channels
    output_channels: usize,
    /// Number of bands to merge
    num_bands: usize,

    // Parameter ID
    param_bands: ParameterId,
}

impl BandMergePlugin {
    /// Create a new band merge plugin
    ///
    /// # Arguments
    /// * `output_channels` - Number of output channels (input will be output_channels * bands)
    /// * `bands` - Number of bands to merge (default: 2)
    pub fn new(output_channels: usize, bands: usize) -> Result<Self, String> {
        if bands < 2 {
            return Err("Band merge requires at least 2 bands".to_string());
        }
        if output_channels == 0 {
            return Err("Output channels must be > 0".to_string());
        }

        Ok(Self {
            output_channels,
            num_bands: bands,
            param_bands: ParameterId("bands".to_string()),
        })
    }

    /// Create from parameters
    pub fn from_params(output_channels: usize, params: &BandMergePluginParams) -> Result<Self, String> {
        Self::new(output_channels, params.bands)
    }
}

impl Plugin for BandMergePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandMerge", "1.0.0", "SotF").with_description(format!(
            "Band merger - {} bands × {} channels = {} in, {} out",
            self.num_bands,
            self.output_channels,
            self.input_channels(),
            self.output_channels
        ))
    }

    fn input_channels(&self) -> usize {
        self.output_channels * self.num_bands
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        use super::parameters::ParameterImportance;
        use crate::param_specs::band_merge::*;

        vec![Parameter::new_int(
            "bands",
            "Number of Bands",
            self.num_bands as i32,
            BANDS_MIN as i32,
            BANDS_MAX as i32,
        )
        .with_importance(ParameterImportance::Critical)]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_bands {
            if let Some(v) = value.as_int() {
                if v < 2 {
                    return Err("bands must be at least 2".to_string());
                }
                self.num_bands = v as usize;
                return Ok(());
            }
            return Err("bands must be int".to_string());
        }
        Err(format!("Unknown parameter ID: {}", id.0))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_bands {
            Some(ParameterValue::Int(self.num_bands as i32))
        } else {
            None
        }
    }

    fn initialize(&mut self, _sample_rate: u32) -> PluginResult<()> {
        Ok(())
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        let in_ch = self.input_channels();
        let out_ch = self.output_channels;

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

        // Process each frame
        // Input layout: [band0_ch0, band0_ch1, ..., band1_ch0, band1_ch1, ...]
        //               where each frame has: [b0_c0, b0_c1, ..., b1_c0, b1_c1, ...]
        // Output layout: [ch0, ch1, ...] (sum of all bands per channel)
        for frame in 0..num_frames {
            let in_offset = frame * in_ch;
            let out_offset = frame * out_ch;

            for ch in 0..out_ch {
                let mut sum = 0.0f32;

                // Sum all bands for this channel
                for band in 0..self.num_bands {
                    let band_offset = band * out_ch;
                    sum += input[in_offset + band_offset + ch];
                }

                output[out_offset + ch] = sum;
            }
        }

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
    fn test_band_merge_creation() {
        let plugin = BandMergePlugin::new(2, 2).unwrap();
        assert_eq!(plugin.input_channels(), 4); // 2 bands × 2 channels
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_band_merge_invalid_bands() {
        let result = BandMergePlugin::new(2, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_band_merge_process() {
        let mut plugin = BandMergePlugin::new(2, 2).unwrap();
        plugin.initialize(48000).unwrap();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 2,
        };

        // Input: 2 frames, 4 channels (2 bands × 2 channels)
        // Frame 0: [low_L=1.0, low_R=2.0, high_L=3.0, high_R=4.0]
        // Frame 1: [low_L=0.5, low_R=1.0, high_L=1.5, high_R=2.0]
        let input = [1.0, 2.0, 3.0, 4.0, 0.5, 1.0, 1.5, 2.0];
        let mut output = [0.0f32; 4]; // 2 frames × 2 channels

        plugin.process(&input, &mut output, &context).unwrap();

        // Expected output:
        // Frame 0: L = 1.0 + 3.0 = 4.0, R = 2.0 + 4.0 = 6.0
        // Frame 1: L = 0.5 + 1.5 = 2.0, R = 1.0 + 2.0 = 3.0
        assert!((output[0] - 4.0).abs() < 1e-6);
        assert!((output[1] - 6.0).abs() < 1e-6);
        assert!((output[2] - 2.0).abs() < 1e-6);
        assert!((output[3] - 3.0).abs() < 1e-6);
    }
}
