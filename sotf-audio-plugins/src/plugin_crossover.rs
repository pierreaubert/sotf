// ============================================================================
// Crossover Plugin
// ============================================================================
//
// This plugin implements multi-way crossover filtering for speaker systems.
// Supports Linkwitz-Riley and Butterworth crossover types.

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

/// Crossover configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossoverPluginParams {
    /// Crossover type: "LR24", "LR48", "Butterworth24", etc.
    #[serde(rename = "type")]
    pub crossover_type: String,

    /// Crossover frequency in Hz
    pub frequency: f64,

    /// Which output to keep: "low" or "high"
    pub output: String,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Crossover filter plugin
#[derive(Debug)]
pub struct CrossoverPlugin {
    /// Number of channels
    num_channels: usize,

    /// IIR filters (one chain per channel)
    /// For LR24: 2 cascaded 2nd-order filters
    /// For LR48: 4 cascaded 2nd-order filters
    filters: Vec<Vec<Biquad>>,

    /// Sample rate
    sample_rate: u32,

    /// Crossover frequency
    frequency: f64,

    /// Output type: true = highpass, false = lowpass
    is_highpass: bool,
}

impl CrossoverPlugin {
    /// Create a new crossover plugin
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels
    /// * `_crossover_type` - Type of crossover ("LR24", "LR48", "Butterworth24")
    /// * `frequency` - Crossover frequency in Hz
    /// * `output` - Which output to keep: "low" or "high"
    pub fn new(
        num_channels: usize,
        _crossover_type: &str,
        frequency: f64,
        output: &str,
    ) -> Result<Self, String> {
        let is_highpass = match output.to_lowercase().as_str() {
            "high" | "highpass" | "hp" => true,
            "low" | "lowpass" | "lp" => false,
            _ => return Err(format!("Invalid crossover output: {}", output)),
        };

        let plugin = Self {
            num_channels,
            filters: vec![Vec::new(); num_channels],
            sample_rate: 48000, // Will be updated in initialize()
            frequency,
            is_highpass,
        };

        // Filters will be created in initialize() when we have the sample rate
        Ok(plugin)
    }

    /// Create a new crossover plugin from configuration parameters
    pub fn from_params(
        num_channels: usize,
        params: &CrossoverPluginParams,
    ) -> Result<Self, String> {
        Self::new(
            num_channels,
            &params.crossover_type,
            params.frequency,
            &params.output,
        )
    }

    /// Build filter chain based on crossover type
    fn build_filters(&mut self, crossover_type: &str) -> Result<(), String> {
        let q = 1.0 / std::f64::consts::SQRT_2; // Butterworth Q = 0.707

        let filter_type = if self.is_highpass {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Lowpass
        };

        // Determine number of cascaded 2nd-order sections
        let num_sections = match crossover_type.to_lowercase().as_str() {
            "lr24" | "lr4" | "linkwitzriley24" | "linkwitzriley4" => 2, // 2x 2nd-order = 4th-order = 24dB/oct
            "lr48" | "lr8" | "linkwitzriley48" | "linkwitzriley8" => 4, // 4x 2nd-order = 8th-order = 48dB/oct
            "butterworth24" | "bw24" => 2,
            "butterworth12" | "bw12" => 1,
            _ => return Err(format!("Unknown crossover type: {}", crossover_type)),
        };

        // Build filter chain for each channel
        for ch in 0..self.num_channels {
            let mut filters = Vec::new();

            for _ in 0..num_sections {
                let biquad = Biquad::new(
                    filter_type,
                    self.frequency,
                    self.sample_rate as f64,
                    q,
                    0.0, // No gain for crossover filters
                );
                filters.push(biquad);
            }

            self.filters[ch] = filters;
        }

        Ok(())
    }
}

impl InPlacePlugin for CrossoverPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Crossover".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: format!(
                "Multi-way crossover filter at {} Hz ({})",
                self.frequency,
                if self.is_highpass {
                    "highpass"
                } else {
                    "lowpass"
                }
            ),
        }
    }

    fn channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("Crossover plugin has no adjustable parameters".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Rebuild filters with correct sample rate
        // For now, assume LR24 as default
        self.build_filters("LR24")?;

        Ok(())
    }

    fn reset(&mut self) {
        // Reset all filter states
        for channel_filters in &mut self.filters {
            for filter in channel_filters {
                // Biquad doesn't have a reset method, so we just recreate the filter
                let filter_type = if self.is_highpass {
                    BiquadFilterType::Highpass
                } else {
                    BiquadFilterType::Lowpass
                };
                let q = 1.0 / std::f64::consts::SQRT_2;
                *filter = Biquad::new(filter_type, self.frequency, self.sample_rate as f64, q, 0.0);
            }
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify buffer size matches channel count
        if !buffer.len().is_multiple_of(self.num_channels) {
            return Err(format!(
                "Buffer size {} is not a multiple of channel count {}",
                buffer.len(),
                self.num_channels
            ));
        }

        let num_frames = context.num_frames;

        // Process each channel
        for ch in 0..self.num_channels {
            for frame in 0..num_frames {
                let idx = frame * self.num_channels + ch;
                let mut sample = buffer[idx];

                // Apply cascaded filters
                for filter in &mut self.filters[ch] {
                    sample = filter.process(sample as f64) as f32;
                }

                buffer[idx] = sample;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        0
    }
}
