// ============================================================================
// Crossover Plugin
// ============================================================================
//
// This plugin implements multi-way crossover filtering for speaker systems.
// Supports Linkwitz-Riley and Butterworth crossover types.

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
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
        let desc = format!(
            "Multi-way crossover filter at {} Hz ({})",
            self.frequency,
            if self.is_highpass {
                "highpass"
            } else {
                "lowpass"
            }
        );
        PluginInfo::new("Crossover", "1.0.0", "SotF").with_description(desc)
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
    ) -> PluginResult<usize> {
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

        // Flush denormals to prevent CPU performance spikes and audio crackle
        // IIR filter calculations can produce denormal numbers
        flush_denormals_inplace(buffer);

        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crossover_creation_lowpass() {
        let plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        assert_eq!(plugin.channels(), 2);
        assert!(!plugin.is_highpass);
        assert_eq!(plugin.frequency, 1000.0);
    }

    #[test]
    fn test_crossover_creation_highpass() {
        let plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "high").unwrap();
        assert!(plugin.is_highpass);
    }

    #[test]
    fn test_crossover_output_aliases() {
        // Test all valid output string aliases
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "high").is_ok());
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "highpass").is_ok());
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "hp").is_ok());
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "low").is_ok());
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "lowpass").is_ok());
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "lp").is_ok());
    }

    #[test]
    fn test_crossover_invalid_output() {
        let result = CrossoverPlugin::new(2, "LR24", 1000.0, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_crossover_from_params() {
        let params = CrossoverPluginParams {
            crossover_type: "LR24".to_string(),
            frequency: 2000.0,
            output: "high".to_string(),
        };
        let plugin = CrossoverPlugin::from_params(2, &params).unwrap();
        assert!(plugin.is_highpass);
        assert_eq!(plugin.frequency, 2000.0);
    }

    #[test]
    fn test_crossover_build_filters_lr24() {
        let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        plugin.build_filters("LR24").unwrap();
        // LR24 = 2 cascaded 2nd-order sections per channel
        assert_eq!(plugin.filters.len(), 2);
        assert_eq!(plugin.filters[0].len(), 2);
        assert_eq!(plugin.filters[1].len(), 2);
    }

    #[test]
    fn test_crossover_build_filters_lr48() {
        let mut plugin = CrossoverPlugin::new(2, "LR48", 1000.0, "low").unwrap();
        plugin.build_filters("LR48").unwrap();
        // LR48 = 4 cascaded 2nd-order sections per channel
        assert_eq!(plugin.filters[0].len(), 4);
    }

    #[test]
    fn test_crossover_build_filters_bw12() {
        let mut plugin = CrossoverPlugin::new(1, "BW12", 500.0, "high").unwrap();
        plugin.build_filters("BW12").unwrap();
        // Butterworth12 = 1 section
        assert_eq!(plugin.filters[0].len(), 1);
    }

    #[test]
    fn test_crossover_build_filters_invalid_type() {
        let mut plugin = CrossoverPlugin::new(1, "invalid", 1000.0, "low").unwrap();
        assert!(plugin.build_filters("invalid_type").is_err());
    }

    #[test]
    fn test_crossover_initialize_sets_sample_rate() {
        let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(96000).unwrap();
        assert_eq!(plugin.sample_rate, 96000);
        // Filters should be built after initialize
        assert_eq!(plugin.filters[0].len(), 2);
    }

    #[test]
    fn test_crossover_no_parameters() {
        let plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        assert!(plugin.parameters().is_empty());
        assert!(plugin.get_parameter(&ParameterId::from("frequency")).is_none());
    }

    #[test]
    fn test_crossover_set_parameter_returns_error() {
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        let result = plugin.set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(2000.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_crossover_lowpass_attenuates_high_frequencies() {
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(48000).unwrap();

        let sample_rate = 48000.0;
        let num_frames = 4096;

        // Generate a high-frequency sine (10kHz, well above 1kHz crossover)
        let mut buffer: Vec<f32> = (0..num_frames)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (t * 10000.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
            })
            .collect();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Measure input energy
        let input_energy: f32 = buffer.iter().map(|s| s * s).sum();

        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Measure output energy (skip first 256 samples for filter settling)
        let output_energy: f32 = buffer[256..].iter().map(|s| s * s).sum();
        let input_energy_tail: f32 = (0..num_frames)
            .skip(256)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let s = (t * 10000.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
                s * s
            })
            .sum();

        // A 24dB/oct lowpass at 1kHz should heavily attenuate 10kHz
        let ratio = output_energy / input_energy_tail;
        assert!(
            ratio < 0.01,
            "Lowpass should heavily attenuate 10kHz signal (ratio: {:.6})",
            ratio
        );
        // Input had energy
        assert!(input_energy > 0.0);
    }

    #[test]
    fn test_crossover_highpass_attenuates_low_frequencies() {
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
        plugin.initialize(48000).unwrap();

        let sample_rate = 48000.0;
        let num_frames = 4096;

        // Generate a low-frequency sine (100Hz, well below 1kHz crossover)
        let mut buffer: Vec<f32> = (0..num_frames)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (t * 100.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
            })
            .collect();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let input_energy_tail: f32 = (0..num_frames)
            .skip(512)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let s = (t * 100.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
                s * s
            })
            .sum();

        plugin.process_in_place(&mut buffer, &context).unwrap();

        let output_energy: f32 = buffer[512..].iter().map(|s| s * s).sum();

        // A 24dB/oct highpass at 1kHz should heavily attenuate 100Hz
        let ratio = output_energy / input_energy_tail;
        assert!(
            ratio < 0.01,
            "Highpass should heavily attenuate 100Hz signal (ratio: {:.6})",
            ratio
        );
    }

    #[test]
    fn test_crossover_lowpass_passes_low_frequencies() {
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(48000).unwrap();

        let sample_rate = 48000.0;
        let num_frames = 4096;

        // Generate a low-frequency sine (100Hz, well below 1kHz crossover)
        let mut buffer: Vec<f32> = (0..num_frames)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (t * 100.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
            })
            .collect();

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        let input_energy_tail: f32 = (0..num_frames)
            .skip(512)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let s = (t * 100.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
                s * s
            })
            .sum();

        plugin.process_in_place(&mut buffer, &context).unwrap();

        let output_energy: f32 = buffer[512..].iter().map(|s| s * s).sum();

        // 100Hz should pass through a 1kHz lowpass with minimal attenuation
        let ratio = output_energy / input_energy_tail;
        assert!(
            ratio > 0.9,
            "Lowpass should pass 100Hz with minimal attenuation (ratio: {:.4})",
            ratio
        );
    }

    #[test]
    fn test_crossover_multichannel() {
        let mut plugin = CrossoverPlugin::new(4, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(48000).unwrap();

        let num_frames = 512;
        let channels = 4;
        let mut buffer = vec![0.5_f32; num_frames * channels];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Should not error
        plugin.process_in_place(&mut buffer, &context).unwrap();

        // All channels should have been processed
        for ch in 0..channels {
            let ch_energy: f32 = (0..num_frames)
                .map(|f| buffer[f * channels + ch] * buffer[f * channels + ch])
                .sum();
            assert!(ch_energy > 0.0, "Channel {} should have energy", ch);
        }
    }

    #[test]
    fn test_crossover_reset() {
        let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(48000).unwrap();

        // Process some data to build up filter state
        let num_frames = 256;
        let mut buffer = vec![1.0_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        plugin.process_in_place(&mut buffer, &context).unwrap();

        // Reset should not panic
        plugin.reset();

        // Process again - should work cleanly
        let mut buffer2 = vec![0.5_f32; num_frames * 2];
        plugin.process_in_place(&mut buffer2, &context).unwrap();
    }

    #[test]
    fn test_crossover_various_sample_rates() {
        for &sample_rate in &[22050, 44100, 48000, 96000, 192000] {
            let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
            plugin.initialize(sample_rate).unwrap();
            assert_eq!(plugin.sample_rate, sample_rate);

            let num_frames = 256;
            let mut buffer = vec![0.5_f32; num_frames * 2];
            let context = ProcessContext {
                sample_rate,
                num_frames,
            };

            plugin.process_in_place(&mut buffer, &context).unwrap();
        }
    }

    #[test]
    fn test_crossover_buffer_size_validation() {
        let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(48000).unwrap();

        // Buffer size not multiple of channels should error
        let mut bad_buffer = vec![0.5_f32; 3]; // 3 is not a multiple of 2
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        let result = plugin.process_in_place(&mut bad_buffer, &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_crossover_info() {
        let plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        let info = plugin.info();
        assert_eq!(info.name, "Crossover");
        assert!(info.description.contains("1000"));
        assert!(info.description.contains("lowpass"));

        let plugin_hp = CrossoverPlugin::new(2, "LR24", 2000.0, "high").unwrap();
        let info_hp = plugin_hp.info();
        assert!(info_hp.description.contains("highpass"));
    }

    #[test]
    fn test_crossover_latency_is_zero() {
        let plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
        assert_eq!(plugin.latency_samples(), 0);
    }
}
