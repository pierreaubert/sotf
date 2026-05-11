// ============================================================================
// Crossover Plugin
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_host::lr4_crossover::{Lr4Crossover, MultibandLr4Crossover};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::LogSmoother;

/// Crossover output mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrossoverMode {
    Lowpass,
    Highpass,
    Both,
}

impl CrossoverMode {
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "low" | "lowpass" | "lp" => Ok(CrossoverMode::Lowpass),
            "high" | "highpass" | "hp" => Ok(CrossoverMode::Highpass),
            "both" => Ok(CrossoverMode::Both),
            _ => Err(format!("Invalid output mode: {}", s)),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            CrossoverMode::Lowpass => "lowpass",
            CrossoverMode::Highpass => "highpass",
            CrossoverMode::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossoverPluginParams {
    #[serde(rename = "type")]
    pub crossover_type: String,
    pub frequency: f64,
    pub output: String,
    /// Additional crossover frequencies for 3-way or 4-way mode.
    /// When provided, creates a multi-way crossover. The primary `frequency`
    /// becomes the first crossover point.
    #[serde(default)]
    pub extra_frequencies: Vec<f64>,
}

pub struct CrossoverPlugin {
    num_channels: usize,
    sample_rate: u32,
    mode: CrossoverMode,
    cached_parameters: Vec<Parameter>,

    /// Single crossover for 2-way operation
    crossover_2way: Lr4Crossover<f32>,
    freq_smoother: LogSmoother,

    /// Multi-band crossover for 3-way and 4-way operation.
    /// None when in 2-way mode.
    multiband: Option<MultibandLr4Crossover<f32>>,
    extra_freq_smoothers: Vec<LogSmoother>,

    /// Sorted crossover frequencies for multi-way mode (including primary).
    all_frequencies: Vec<f32>,

    /// Pre-allocated scratch buffers
    low_buf: Vec<f32>,
    high_buf: Vec<f32>,
    /// Flat buffer for multi-way band outputs: [band0_ch0..band0_chN, band1_ch0..band1_chN, ...]
    band_flat: Vec<f32>,
}

impl CrossoverPlugin {
    pub fn new(
        num_channels: usize,
        _crossover_type: &str,
        frequency: f64,
        output: &str,
    ) -> Result<Self, String> {
        Self::new_multiway(num_channels, _crossover_type, frequency, output, &[])
    }

    pub fn new_multiway(
        num_channels: usize,
        _crossover_type: &str,
        frequency: f64,
        output: &str,
        extra_frequencies: &[f64],
    ) -> Result<Self, String> {
        let mode = CrossoverMode::from_str(output)?;
        let sr = 48000;

        let mut all_freqs: Vec<f32> = vec![frequency as f32];
        for &f in extra_frequencies {
            all_freqs.push(f as f32);
        }
        all_freqs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_freqs.dedup();

        let num_bands = all_freqs.len() + 1;

        let (multiband, extra_smoothers) = if all_freqs.len() > 1 {
            let mb = MultibandLr4Crossover::new(&all_freqs, sr as f32, num_channels);
            let smoothers: Vec<LogSmoother> = all_freqs
                .iter()
                .skip(1) // first freq uses the primary smoother
                .map(|&f| LogSmoother::new(f, 20.0, sr))
                .collect();
            (Some(mb), smoothers)
        } else {
            (None, Vec::new())
        };

        let mut p = Self {
            num_channels,
            sample_rate: sr,
            mode,
            crossover_2way: Lr4Crossover::new(frequency as f32, sr as f32, num_channels),
            freq_smoother: LogSmoother::new(frequency as f32, 20.0, sr),
            multiband,
            extra_freq_smoothers: extra_smoothers,
            all_frequencies: all_freqs,
            cached_parameters: Vec::new(),
            low_buf: vec![0.0; num_channels],
            high_buf: vec![0.0; num_channels],
            band_flat: vec![0.0; num_bands * num_channels],
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_float(
                "frequency",
                "Frequency",
                self.freq_smoother.target(),
                20.0,
                20000.0,
            ),
            Parameter::new_string("mode", "Mode", self.mode.as_str().to_string()),
        ];

        // Add extra frequency parameters for multi-way
        for (i, smoother) in self.extra_freq_smoothers.iter().enumerate() {
            params.push(Parameter::new_float(
                &format!("frequency_{}", i + 2),
                &format!("Frequency {}", i + 2),
                smoother.target(),
                20.0,
                20000.0,
            ));
        }

        self.cached_parameters = params;
    }

    pub fn from_params(
        num_channels: usize,
        params: &CrossoverPluginParams,
    ) -> Result<Self, String> {
        Self::new_multiway(
            num_channels,
            &params.crossover_type,
            params.frequency,
            &params.output,
            &params.extra_frequencies,
        )
    }

    /// Number of output bands based on current configuration.
    fn num_bands(&self) -> usize {
        self.all_frequencies.len() + 1
    }

    /// Calculate output channels based on mode and band count.
    fn calc_output_channels(&self) -> usize {
        match self.mode {
            CrossoverMode::Lowpass | CrossoverMode::Highpass => self.num_channels,
            CrossoverMode::Both => self.num_channels * self.num_bands(),
        }
    }

    /// Returns true if operating in multi-way (3+ bands) mode.
    fn is_multiway(&self) -> bool {
        self.multiband.is_some()
    }

    /// Parse "frequency_N" into an extra smoother index (0-based).
    /// "frequency_2" -> Some(0), "frequency_3" -> Some(1), etc.
    fn parse_extra_freq_index(s: &str) -> Option<usize> {
        s.strip_prefix("frequency_")
            .and_then(|idx_str| idx_str.parse::<usize>().ok())
            .map(|idx| idx.saturating_sub(2))
    }
}

impl Plugin for CrossoverPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Crossover", "3.0.0", "SotF")
            .with_description("Linkwitz-Riley crossover with multi-way and dual-output support")
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.calc_output_channels()
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id.0 == "frequency" {
            let val = value.as_float().unwrap_or(1000.0);
            if val.is_finite() {
                self.freq_smoother.set_target(val);
                // Update first frequency in multi-way list
                if !self.all_frequencies.is_empty() {
                    self.all_frequencies[0] = val;
                }
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if id.0 == "mode" {
            if let Some(s) = value.as_string() {
                self.mode = CrossoverMode::from_str(s)?;
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else if let Some(smoother_idx) = Self::parse_extra_freq_index(&id.0) {
            let val = value.as_float().unwrap_or(1000.0);
            if val.is_finite() && smoother_idx < self.extra_freq_smoothers.len() {
                self.extra_freq_smoothers[smoother_idx].set_target(val);
                let freq_idx = smoother_idx + 1; // offset: extra smoothers start at freq index 1
                if freq_idx < self.all_frequencies.len() {
                    self.all_frequencies[freq_idx] = val;
                }
                self.rebuild_cached_parameters();
            }
            Ok(())
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "frequency" {
            Some(ParameterValue::Float(self.freq_smoother.target()))
        } else if id.0 == "mode" {
            Some(ParameterValue::String(self.mode.as_str().to_string()))
        } else if let Some(smoother_idx) = Self::parse_extra_freq_index(&id.0) {
            self.extra_freq_smoothers
                .get(smoother_idx)
                .map(|s| ParameterValue::Float(s.target()))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        let nyquist = sample_rate as f32 * 0.5 * 0.99;
        let freq = self.freq_smoother.target().min(nyquist);
        self.freq_smoother = LogSmoother::new(freq, 20.0, sample_rate);
        self.crossover_2way.reinit(freq, sample_rate as f32, self.num_channels);
        self.low_buf.resize(self.num_channels, 0.0);
        self.high_buf.resize(self.num_channels, 0.0);

        if let Some(ref mut mb) = self.multiband {
            for smoother in &mut self.extra_freq_smoothers {
                let f = smoother.target().min(nyquist);
                *smoother = LogSmoother::new(f, 20.0, sample_rate);
            }
            let clamped_freqs: Vec<f32> = self.all_frequencies.iter().map(|&f| f.min(nyquist)).collect();
            mb.reinit(&clamped_freqs, sample_rate as f32, self.num_channels);
        }

        // Resize band flat buffer
        let nb = self.num_bands();
        self.band_flat.resize(nb * self.num_channels, 0.0);

        Ok(())
    }

    fn reset(&mut self) {
        self.crossover_2way.reset();
        if let Some(ref mut mb) = self.multiband {
            mb.reset();
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let in_ch = self.num_channels;
        let out_ch = self.output_channels();

        if self.is_multiway() {
            // Multi-way processing
            let num_bands = self.num_bands();
            let mb = self.multiband.as_mut().unwrap();

            // Sub-block size for frequency updates: every 16 samples to avoid
            // zipper noise while keeping CPU cost reasonable.
            const SUBBLOCK: usize = 16;

            for frame in 0..num_frames {
                if frame % SUBBLOCK == 0 {
                    let new_freq0 = self.freq_smoother.next_n(SUBBLOCK.min(num_frames - frame));
                    mb.set_frequency(0, new_freq0);
                    for (i, smoother) in self.extra_freq_smoothers.iter_mut().enumerate() {
                        let f = smoother.next_n(SUBBLOCK.min(num_frames - frame));
                        mb.set_frequency(i + 1, f);
                    }
                }
                let in_off = frame * in_ch;
                let out_off = frame * out_ch;
                let frame_slice = &input[in_off..in_off + in_ch];

                // Build mutable slice references into band_flat using split_at_mut
                // to satisfy the borrow checker without unsafe.
                {
                    let flat = &mut self.band_flat[..num_bands * in_ch];
                    // Use fixed-size array to avoid per-frame heap allocation.
                    // Crossover supports up to 4 bands (3 crossover points).
                    let mut band_slices: [&mut [f32]; 4] = [&mut [], &mut [], &mut [], &mut []];
                    let mut remaining = flat;
                    for slot in band_slices.iter_mut().take(num_bands) {
                        let (chunk, rest) = remaining.split_at_mut(in_ch);
                        *slot = chunk;
                        remaining = rest;
                    }
                    mb.process_frame(frame_slice, &mut band_slices[..num_bands]);
                }

                match self.mode {
                    CrossoverMode::Lowpass => {
                        output[out_off..out_off + in_ch].copy_from_slice(&self.band_flat[..in_ch]);
                    }
                    CrossoverMode::Highpass => {
                        let hi_off = (num_bands - 1) * in_ch;
                        output[out_off..out_off + in_ch]
                            .copy_from_slice(&self.band_flat[hi_off..hi_off + in_ch]);
                    }
                    CrossoverMode::Both => {
                        output[out_off..out_off + out_ch]
                            .copy_from_slice(&self.band_flat[..out_ch]);
                    }
                }
            }
        } else {
            // 2-way processing
            const SUBBLOCK: usize = 16;

            for frame in 0..num_frames {
                if frame % SUBBLOCK == 0 {
                    let new_freq = self.freq_smoother.next_n(SUBBLOCK.min(num_frames - frame));
                    self.crossover_2way.set_frequency(new_freq);
                }
                let in_off = frame * in_ch;
                let out_off = frame * out_ch;
                let frame_slice = &input[in_off..in_off + in_ch];

                self.crossover_2way.process_frame(
                    frame_slice,
                    &mut self.low_buf,
                    &mut self.high_buf,
                );

                match self.mode {
                    CrossoverMode::Lowpass => {
                        output[out_off..out_off + in_ch].copy_from_slice(&self.low_buf);
                    }
                    CrossoverMode::Highpass => {
                        output[out_off..out_off + in_ch].copy_from_slice(&self.high_buf);
                    }
                    CrossoverMode::Both => {
                        // Low band first, then high band
                        output[out_off..out_off + in_ch].copy_from_slice(&self.low_buf);
                        output[out_off + in_ch..out_off + 2 * in_ch]
                            .copy_from_slice(&self.high_buf);
                    }
                }
            }
        }

        flush_denormals_inplace(output);
        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_crossover_basic() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 1000];
        let mut output = vec![0.0; 1000];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(output[999].is_finite());
    }

    #[test]
    fn test_crossover_highpass() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 1000];
        let mut output = vec![0.0; 1000];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(output[999].is_finite());
    }

    #[test]
    fn test_crossover_stereo() {
        let mut p = CrossoverPlugin::new(2, "LR24", 500.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![0.5; 200];
        let mut output = vec![0.0; 200];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 100,
            },
        )
        .unwrap();
        assert!(output[0].is_finite());
        assert!(output[199].is_finite());
    }

    #[test]
    fn test_crossover_dc_passes_lowpass() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 10000];
        let mut output = vec![0.0; 10000];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 10000,
            },
        )
        .unwrap();
        assert!(
            output[9999] > 0.9,
            "DC through lowpass should be near 1.0, got {}",
            output[9999]
        );
    }

    #[test]
    fn test_crossover_dc_rejected_highpass() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
        p.initialize(48000).unwrap();
        let input = vec![1.0; 10000];
        let mut output = vec![0.0; 10000];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 10000,
            },
        )
        .unwrap();
        assert!(
            output[9999].abs() < 0.1,
            "DC through highpass should be near 0.0, got {}",
            output[9999]
        );
    }

    #[test]
    fn test_crossover_invalid_output() {
        let result = CrossoverPlugin::new(1, "LR24", 1000.0, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_crossover_both_mode_doubles_channels() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 1);
        assert_eq!(p.output_channels(), 2); // 1 channel * 2 bands

        // Process DC: low band should have the signal, high should be ~0
        let num_frames = 10000;
        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames * 2]; // 2 output channels
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();

        // Last frame: output[idx*2] = low, output[idx*2+1] = high
        let last = (num_frames - 1) * 2;
        assert!(
            output[last] > 0.9,
            "DC low band should be near 1.0, got {}",
            output[last]
        );
        assert!(
            output[last + 1].abs() < 0.1,
            "DC high band should be near 0.0, got {}",
            output[last + 1]
        );
    }

    #[test]
    fn test_crossover_both_bands_sum_preserves_energy() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
        p.initialize(48000).unwrap();

        // Feed a signal and verify low + high sum has comparable energy to input.
        // LR4 crossovers sum to flat magnitude but introduce group delay,
        // so per-sample comparison with undelayed input is not valid.
        // Instead, verify RMS energy is preserved.
        let num_frames = 10000;
        let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.05).sin()).collect();
        let mut output = vec![0.0f32; num_frames * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();

        // Compare RMS of input vs RMS of (low+high) over the settled region
        let settle = 2000;
        let input_rms: f32 = (input[settle..].iter().map(|s| s * s).sum::<f32>()
            / (num_frames - settle) as f32)
            .sqrt();

        let sum_rms: f32 = ((settle..num_frames)
            .map(|f| {
                let s = output[f * 2] + output[f * 2 + 1];
                s * s
            })
            .sum::<f32>()
            / (num_frames - settle) as f32)
            .sqrt();

        let ratio = sum_rms / input_rms;
        assert!(
            (ratio - 1.0).abs() < 0.15,
            "RMS ratio should be near 1.0 (flat sum), got {}",
            ratio
        );
    }

    #[test]
    fn test_crossover_stereo_both_mode() {
        let mut p = CrossoverPlugin::new(2, "LR24", 1000.0, "both").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 2);
        assert_eq!(p.output_channels(), 4); // 2 channels * 2 bands

        let num_frames = 100;
        let input = vec![0.5f32; num_frames * 2];
        let mut output = vec![0.0f32; num_frames * 4];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();
        // All outputs should be finite
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_crossover_3way() {
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 1);
        assert_eq!(p.output_channels(), 3); // 1 channel * 3 bands

        let num_frames = 10000;
        let input = vec![1.0f32; num_frames]; // DC
        let mut output = vec![0.0f32; num_frames * 3];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();

        // DC should pass through lowest band only
        let last = (num_frames - 1) * 3;
        assert!(
            output[last] > 0.9,
            "3-way DC band 0 (low) should be near 1.0, got {}",
            output[last]
        );
        assert!(
            output[last + 1].abs() < 0.1,
            "3-way DC band 1 (mid) should be near 0.0, got {}",
            output[last + 1]
        );
        assert!(
            output[last + 2].abs() < 0.1,
            "3-way DC band 2 (high) should be near 0.0, got {}",
            output[last + 2]
        );
    }

    #[test]
    fn test_crossover_4way() {
        let mut p =
            CrossoverPlugin::new_multiway(1, "LR24", 200.0, "both", &[1000.0, 5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 1);
        assert_eq!(p.output_channels(), 4); // 1 channel * 4 bands

        let num_frames = 1000;
        let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut output = vec![0.0f32; num_frames * 4];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();

        // All outputs should be finite
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_crossover_3way_lowpass_mode() {
        // In lowpass mode, 3-way should output only the lowest band
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "low", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 1); // Only lowest band

        let num_frames = 10000;
        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();

        // DC passes through lowpass
        assert!(
            output[9999] > 0.9,
            "3-way lowpass DC should be near 1.0, got {}",
            output[9999]
        );
    }

    #[test]
    fn test_crossover_output_selection_highpass_rejects_dc() {
        // Highpass mode should reject DC (output near zero)
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
        p.initialize(48000).unwrap();
        let num_frames = 10000;
        let input = vec![1.0f32; num_frames]; // DC
        let mut output = vec![0.0; num_frames];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();
        assert!(
            output[num_frames - 1].abs() < 0.05,
            "Highpass should reject DC, got {}",
            output[num_frames - 1]
        );
    }

    #[test]
    fn test_crossover_output_selection_lowpass_passes_dc() {
        // Lowpass mode should pass DC (output near 1.0)
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let num_frames = 10000;
        let input = vec![1.0f32; num_frames]; // DC
        let mut output = vec![0.0; num_frames];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();
        assert!(
            output[num_frames - 1] > 0.95,
            "Lowpass should pass DC, got {}",
            output[num_frames - 1]
        );
    }

    #[test]
    fn test_crossover_mode_parameter() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 1);

        p.set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("both".to_string()),
        )
        .unwrap();
        assert_eq!(p.output_channels(), 2);

        let val = p.get_parameter(&ParameterId::from("mode"));
        assert_eq!(val, Some(ParameterValue::String("both".to_string())));
    }

    /// Changing frequency_2 on a 3-way crossover should not panic and should
    /// continue producing finite output.
    #[test]
    fn test_3way_frequency_update_no_panic() {
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 3); // 3 bands

        let num_frames = 2000;
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process a block before parameter change
        let input: Vec<f32> = (0..num_frames)
            .map(|i| 0.3 * (i as f32 * 0.1).sin())
            .collect();
        let mut output = vec![0.0f32; num_frames * 3];
        p.process(&input, &mut output, &ctx).unwrap();

        // Change frequency_2 (the second crossover point)
        p.set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(8000.0),
        )
        .unwrap();

        // Verify the parameter was accepted
        let val = p.get_parameter(&ParameterId::from("frequency_2"));
        assert_eq!(val, Some(ParameterValue::Float(8000.0)));

        // Process another block after the change -- must not panic
        let input2: Vec<f32> = (0..num_frames)
            .map(|i| 0.3 * ((num_frames + i) as f32 * 0.1).sin())
            .collect();
        let mut output2 = vec![0.0f32; num_frames * 3];
        p.process(&input2, &mut output2, &ctx).unwrap();

        // All output must be finite
        assert!(
            output2.iter().all(|s| s.is_finite()),
            "All output samples must be finite after frequency_2 change"
        );

        // At least some output should be non-zero
        let has_signal = output2.iter().any(|s| s.abs() > 1e-6);
        assert!(
            has_signal,
            "Output should contain non-zero samples after frequency change"
        );
    }
}
