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
        if s.eq_ignore_ascii_case("low")
            || s.eq_ignore_ascii_case("lowpass")
            || s.eq_ignore_ascii_case("lp")
        {
            Ok(CrossoverMode::Lowpass)
        } else if s.eq_ignore_ascii_case("high")
            || s.eq_ignore_ascii_case("highpass")
            || s.eq_ignore_ascii_case("hp")
        {
            Ok(CrossoverMode::Highpass)
        } else if s.eq_ignore_ascii_case("both") {
            Ok(CrossoverMode::Both)
        } else {
            Err(format!("Invalid output mode: {}", s))
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
        crossover_type: &str,
        frequency: f64,
        output: &str,
    ) -> Result<Self, String> {
        Self::new_multiway(num_channels, crossover_type, frequency, output, &[])
    }

    pub fn new_multiway(
        num_channels: usize,
        crossover_type: &str,
        frequency: f64,
        output: &str,
        extra_frequencies: &[f64],
    ) -> Result<Self, String> {
        if !crossover_type.eq_ignore_ascii_case("lr24")
            && !crossover_type.eq_ignore_ascii_case("lr4")
        {
            return Err(format!(
                "Unsupported crossover type: '{}'. Only LR24/LR4 is supported.",
                crossover_type
            ));
        }
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
    /// Returns None for indices < 2 to prevent aliasing "frequency_1" onto index 0.
    fn parse_extra_freq_index(s: &str) -> Option<usize> {
        s.strip_prefix("frequency_")
            .and_then(|idx_str| idx_str.parse::<usize>().ok())
            .and_then(|idx| if idx >= 2 { Some(idx - 2) } else { None })
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
                // Update first frequency in multi-way list and re-sort to maintain
                // sorted order. MultibandLr4Crossover requires sorted frequencies.
                if !self.all_frequencies.is_empty() {
                    self.all_frequencies[0] = val;
                    self.all_frequencies
                        .sort_by(|a, b| a.partial_cmp(b).unwrap());
                    self.all_frequencies.dedup();
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
                    self.all_frequencies
                        .sort_by(|a, b| a.partial_cmp(b).unwrap());
                    self.all_frequencies.dedup();
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
        // Clamp all frequencies to just below Nyquist to prevent nonsense biquad
        // coefficients at low sample rates (e.g. 32 kHz with a 20 kHz crossover).
        let nyquist_limit = sample_rate as f32 * 0.5 * 0.99;
        let clamped_primary = self.freq_smoother.target().min(nyquist_limit);
        self.freq_smoother = LogSmoother::new(clamped_primary, 20.0, sample_rate);
        self.crossover_2way.reinit(
            clamped_primary,
            sample_rate as f32,
            self.num_channels,
        );
        self.low_buf.resize(self.num_channels, 0.0);
        self.high_buf.resize(self.num_channels, 0.0);

        if let Some(ref mut mb) = self.multiband {
            // extra_freq_smoothers[i] corresponds to all_frequencies[i+1].
            for (freq, smoother) in self
                .all_frequencies
                .iter_mut()
                .skip(1)
                .zip(self.extra_freq_smoothers.iter_mut())
            {
                let clamped = smoother.target().min(nyquist_limit);
                *freq = clamped;
                *smoother = LogSmoother::new(clamped, 20.0, sample_rate);
            }
            // Clamp all_frequencies[0] (primary, already clamped above).
            if !self.all_frequencies.is_empty() {
                self.all_frequencies[0] = clamped_primary;
            }
            let clamped_freqs: Vec<f32> = self
                .all_frequencies
                .iter()
                .map(|&f| f.min(nyquist))
                .collect();
            mb.reinit(&clamped_freqs, sample_rate as f32, self.num_channels);
        }

        // Resize band flat buffer
        let nb = self.num_bands();
        self.band_flat.resize(nb * self.num_channels, 0.0);

        Ok(())
    }

    fn reset(&mut self) {
        self.crossover_2way.reset();
        // Reset smoothers to their targets so that a mid-transition reset does not
        // cause a click from the remaining interpolation step on the next block.
        self.freq_smoother.reset(self.freq_smoother.target());
        for s in &mut self.extra_freq_smoothers {
            s.reset(s.target());
        }
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

        // Compare RMS of input vs RMS of (low+high) over the settled region.
        // Use at least 5000 samples for settle to ensure the filter has fully settled.
        let settle = 5000;
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
            (ratio - 1.0).abs() < 0.01,
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

    // ── New tests for review fixes ─────────────────────────────────────────

    /// §2.1: Setting 'frequency' to a value larger than the second crossover
    /// point must not leave all_frequencies unsorted.
    #[test]
    fn test_all_frequencies_remain_sorted_after_primary_update() {
        // 3-way: [500, 5000]
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();

        // Move primary frequency above the second point — without the fix this
        // would leave all_frequencies = [10000, 5000] (unsorted).
        p.set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(10000.0),
        )
        .unwrap();

        // Verify the vector is still in ascending order.
        let freqs = p.all_frequencies.clone();
        let mut sorted = freqs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            freqs, sorted,
            "all_frequencies must remain sorted after primary frequency change; got {:?}",
            freqs
        );

        // Plugin must still produce finite output.
        let num_frames = 1000;
        let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
        let num_bands = p.num_bands();
        let mut output = vec![0.0f32; num_frames * num_bands];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames,
            },
        )
        .unwrap();
        assert!(output.iter().all(|s| s.is_finite()));
    }

    /// §2.1: Setting 'frequency_2' to a value smaller than 'frequency' must
    /// also maintain sorted order.
    #[test]
    fn test_all_frequencies_remain_sorted_after_extra_freq_update() {
        // 3-way: [500, 5000]
        let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
        p.initialize(48000).unwrap();

        // Move frequency_2 below the primary — without the fix this would leave
        // all_frequencies = [500, 200] (unsorted).
        p.set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(200.0),
        )
        .unwrap();

        let freqs = p.all_frequencies.clone();
        let mut sorted = freqs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            freqs, sorted,
            "all_frequencies must remain sorted after frequency_2 change; got {:?}",
            freqs
        );
    }

    /// §2.2: "frequency_1" must NOT be parsed as a valid extra-freq parameter.
    #[test]
    fn test_parse_extra_freq_index_rejects_idx_less_than_2() {
        // "frequency_1" should return None — it is not a valid parameter.
        assert_eq!(CrossoverPlugin::parse_extra_freq_index("frequency_1"), None);
        assert_eq!(CrossoverPlugin::parse_extra_freq_index("frequency_0"), None);
        // "frequency_2" must still map to smoother index 0.
        assert_eq!(
            CrossoverPlugin::parse_extra_freq_index("frequency_2"),
            Some(0)
        );
        // "frequency_3" must map to smoother index 1.
        assert_eq!(
            CrossoverPlugin::parse_extra_freq_index("frequency_3"),
            Some(1)
        );
    }

    /// §4.1: Unsupported crossover type strings must return an error.
    #[test]
    fn test_unsupported_crossover_type_returns_error() {
        let result = CrossoverPlugin::new(1, "LR12", 1000.0, "low");
        assert!(
            result.is_err(),
            "LR12 crossover type must be rejected with an error"
        );
        let result2 = CrossoverPlugin::new(1, "BW18", 1000.0, "low");
        assert!(
            result2.is_err(),
            "BW18 crossover type must be rejected with an error"
        );
        // Case-insensitive acceptance of the supported types.
        assert!(CrossoverPlugin::new(1, "lr24", 1000.0, "low").is_ok());
        assert!(CrossoverPlugin::new(1, "LR4", 1000.0, "low").is_ok());
        assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "low").is_ok());
    }

    /// §4.2: CrossoverMode::from_str must be case-insensitive (no allocation path).
    #[test]
    fn test_crossover_mode_from_str_is_case_insensitive() {
        assert_eq!(
            CrossoverMode::from_str("LOW"),
            Ok(CrossoverMode::Lowpass)
        );
        assert_eq!(
            CrossoverMode::from_str("Lowpass"),
            Ok(CrossoverMode::Lowpass)
        );
        assert_eq!(
            CrossoverMode::from_str("HP"),
            Ok(CrossoverMode::Highpass)
        );
        assert_eq!(
            CrossoverMode::from_str("BOTH"),
            Ok(CrossoverMode::Both)
        );
    }

    /// §4.3: reset() must snap smoothers to their targets to avoid a
    /// click on the next block when a parameter was mid-transition.
    #[test]
    fn test_reset_snaps_smoothers_to_target() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();

        // Start a slow parameter transition (20 ms @ 48 kHz = ~960 samples to converge).
        p.set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(5000.0),
        )
        .unwrap();

        // Process only a few samples so the smoother is mid-transition.
        let input = vec![0.0f32; 16];
        let mut output = vec![0.0f32; 16];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 16,
            },
        )
        .unwrap();

        // Reset must snap the smoother current to target.
        p.reset();
        let current = p.freq_smoother.current();
        let target = p.freq_smoother.target();
        assert_eq!(
            current, target,
            "After reset(), smoother current ({}) must equal target ({})",
            current, target
        );
    }

    /// §1.4: initialize() at a low sample rate must not produce NaN/Inf even
    /// when the stored frequency exceeds Nyquist.
    #[test]
    fn test_initialize_clamps_frequency_to_nyquist() {
        // At 32 kHz, Nyquist is 16 kHz. A crossover at 20 kHz exceeds it.
        let mut p = CrossoverPlugin::new(1, "LR24", 20000.0, "low").unwrap();
        p.initialize(32000).unwrap();

        // The effective frequency must be below Nyquist.
        let effective = p.freq_smoother.target();
        assert!(
            effective < 16000.0,
            "Frequency must be clamped below Nyquist (16 kHz) at 32 kHz sample rate, got {}",
            effective
        );

        // Output must be finite.
        let num_frames = 1000;
        let input = vec![1.0f32; num_frames];
        let mut output = vec![0.0f32; num_frames];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 32000,
                num_frames,
            },
        )
        .unwrap();
        assert!(
            output.iter().all(|s| s.is_finite()),
            "Output must be finite after initialize at low sample rate"
        );
    }
}
