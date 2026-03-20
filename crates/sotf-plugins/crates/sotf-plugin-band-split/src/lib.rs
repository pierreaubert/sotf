// ============================================================================
// Band Split Plugin
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_host::lr4_crossover::MultibandLr4Crossover;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::LogSmoother;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandSplitPluginParams {
    /// Crossover frequencies. Length determines the number of bands (len + 1).
    /// For backwards compatibility, a single frequency creates 2 bands.
    #[serde(default)]
    pub frequencies: Vec<f64>,

    /// Legacy single-frequency field (used when `frequencies` is empty).
    #[serde(default = "default_frequency")]
    pub frequency: f64,

    /// Number of bands (2-4). Ignored when `frequencies` is provided with > 1 element.
    #[serde(default = "default_num_bands")]
    pub num_bands: usize,

    #[serde(rename = "type", default = "default_crossover_type")]
    pub crossover_type: String,
}

fn default_crossover_type() -> String {
    "LR24".to_string()
}

fn default_frequency() -> f64 {
    1000.0
}

fn default_num_bands() -> usize {
    2
}

/// Maximum number of bands supported.
const MAX_BANDS: usize = 4;

pub struct BandSplitPlugin {
    input_channels: usize,
    sample_rate: u32,
    num_bands: usize,
    crossover: MultibandLr4Crossover,
    freq_smoothers: Vec<LogSmoother>,
    /// Per-band gain in dB (one per band, up to MAX_BANDS). Default 0.0 dB.
    band_gains_db: [f32; MAX_BANDS],
    /// Pre-computed linear multipliers from band_gains_db.
    band_gains_linear: [f32; MAX_BANDS],
    cached_parameters: Vec<Parameter>,
}

impl BandSplitPlugin {
    pub fn new(
        input_channels: usize,
        frequency: f64,
        crossover_type: &str,
    ) -> Result<Self, String> {
        Self::new_multiband(input_channels, &[frequency], crossover_type)
    }

    pub fn new_multiband(
        input_channels: usize,
        frequencies: &[f64],
        _crossover_type: &str,
    ) -> Result<Self, String> {
        if frequencies.is_empty() {
            return Err("At least one crossover frequency is required".to_string());
        }
        if frequencies.len() + 1 > MAX_BANDS {
            return Err(format!(
                "Too many bands: {} (max {})",
                frequencies.len() + 1,
                MAX_BANDS
            ));
        }
        let sr = 48000;
        let freq_f32: Vec<f32> = frequencies.iter().map(|&f| f as f32).collect();

        let smoothers = frequencies
            .iter()
            .map(|&f| LogSmoother::new(f as f32, 20.0, sr))
            .collect();

        let num_bands = frequencies.len() + 1;

        let mut p = Self {
            input_channels,
            sample_rate: sr,
            num_bands,
            crossover: MultibandLr4Crossover::new(&freq_f32, sr, input_channels),
            freq_smoothers: smoothers,
            band_gains_db: [0.0; MAX_BANDS],
            band_gains_linear: [1.0; MAX_BANDS],
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub fn from_params(
        input_channels: usize,
        params: &BandSplitPluginParams,
    ) -> Result<Self, String> {
        let freqs = if !params.frequencies.is_empty() {
            params.frequencies.clone()
        } else {
            // Use num_bands to determine the number of crossover frequencies.
            match params.num_bands {
                2 => vec![params.frequency],
                3 => {
                    // Default 3-band: split at frequency and frequency * 8
                    let f1 = params.frequency;
                    let f2 = (f1 * 8.0).min(20000.0);
                    vec![f1, f2]
                }
                4 => {
                    // Default 4-band: geometric spread
                    let f1 = params.frequency;
                    let f2 = (f1 * 4.0).min(18000.0);
                    let f3 = (f2 * 3.0).min(20000.0);
                    vec![f1, f2, f3]
                }
                n => return Err(format!("Unsupported num_bands: {} (must be 2-4)", n)),
            }
        };
        Self::new_multiband(input_channels, &freqs, &params.crossover_type)
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = Vec::new();
        for (i, smoother) in self.freq_smoothers.iter().enumerate() {
            let label = if self.freq_smoothers.len() == 1 {
                "Frequency".to_string()
            } else {
                format!("Frequency {}", i + 1)
            };
            let key = if i == 0 {
                "frequency".to_string()
            } else {
                format!("frequency_{}", i + 1)
            };
            params.push(Parameter::new_float(&key, &label, smoother.target(), 20.0, 20000.0));
        }
        for i in 0..self.num_bands {
            let key = format!("band_{}_gain_db", i);
            let label = format!("Band {} Gain (dB)", i + 1);
            params.push(
                Parameter::new_float(&key, &label, self.band_gains_db[i], -24.0, 24.0)
                    .with_group("Band Gains"),
            );
        }
        self.cached_parameters = params;
    }
}

impl Plugin for BandSplitPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandSplit", "2.0.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.input_channels
    }
    fn output_channels(&self) -> usize {
        self.input_channels * self.num_bands
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        let name = &id.0;

        // Match "band_N_gain_db"
        if let Some(rest) = name.strip_prefix("band_")
            && let Some(idx_str) = rest.strip_suffix("_gain_db")
            && let Ok(band_idx) = idx_str.parse::<usize>()
            && band_idx < self.num_bands
        {
            let v = value
                .as_float()
                .ok_or_else(|| "band gain must be a float".to_string())?;
            if v.is_finite() {
                self.band_gains_db[band_idx] = v.clamp(-24.0, 24.0);
                self.band_gains_linear[band_idx] =
                    10.0f32.powf(self.band_gains_db[band_idx] / 20.0);
                self.rebuild_cached_parameters();
            }
            return Ok(());
        }

        // Match "frequency" (index 0) or "frequency_N" (index N-1)
        let idx = if name == "frequency" {
            Some(0)
        } else if let Some(suffix) = name.strip_prefix("frequency_") {
            suffix.parse::<usize>().ok().map(|n| n - 1)
        } else {
            None
        };

        match idx {
            Some(i) if i < self.freq_smoothers.len() => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "frequency must be a float".to_string())?;
                if v.is_finite() {
                    self.freq_smoothers[i].set_target(v);
                    self.rebuild_cached_parameters();
                }
                Ok(())
            }
            _ => Err(format!("Unknown parameter: {}", id)),
        }
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let name = &id.0;

        // Match "band_N_gain_db"
        if let Some(rest) = name.strip_prefix("band_")
            && let Some(idx_str) = rest.strip_suffix("_gain_db")
            && let Ok(band_idx) = idx_str.parse::<usize>()
            && band_idx < self.num_bands
        {
            return Some(ParameterValue::Float(self.band_gains_db[band_idx]));
        }

        let idx = if name == "frequency" {
            Some(0)
        } else if let Some(suffix) = name.strip_prefix("frequency_") {
            suffix.parse::<usize>().ok().map(|n| n - 1)
        } else {
            None
        };

        match idx {
            Some(i) if i < self.freq_smoothers.len() => {
                Some(ParameterValue::Float(self.freq_smoothers[i].target()))
            }
            _ => None,
        }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        let freqs: Vec<f32> = self.freq_smoothers.iter().map(|s| s.target()).collect();
        for s in &mut self.freq_smoothers {
            *s = LogSmoother::new(s.target(), 20.0, sample_rate);
        }
        self.crossover
            .reinit(&freqs, sample_rate, self.input_channels);
        Ok(())
    }
    fn reset(&mut self) {
        self.crossover.reset();
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let in_ch = self.input_channels;
        let out_ch = in_ch * self.num_bands;

        // Block-based smoothing — update all crossover frequencies
        for (i, smoother) in self.freq_smoothers.iter_mut().enumerate() {
            let new_freq = smoother.next_n(num_frames);
            self.crossover.set_frequency(i, new_freq);
        }

        // Allocate band output slices for process_frame
        let mut band_bufs: Vec<Vec<f32>> = (0..self.num_bands).map(|_| vec![0.0; in_ch]).collect();

        for frame in 0..num_frames {
            let in_off = frame * in_ch;
            let out_off = frame * out_ch;
            let frame_input = &input[in_off..in_off + in_ch];

            // Build mutable slice refs for process_frame
            let mut band_slices: Vec<&mut [f32]> =
                band_bufs.iter_mut().map(|b| b.as_mut_slice()).collect();
            self.crossover.process_frame(frame_input, &mut band_slices);

            // Interleave bands into output: [band0_ch0, band0_ch1, band1_ch0, band1_ch1, ...]
            // Apply per-band gain as linear multiplier
            for (band_idx, band) in band_bufs.iter().enumerate() {
                let gain = self.band_gains_linear[band_idx];
                for ch in 0..in_ch {
                    output[out_off + band_idx * in_ch + ch] = band[ch] * gain;
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
    fn test_band_split_basic() {
        let mut p = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
        p.initialize(48000).unwrap();
        let i = vec![1.0; 1000];
        let mut o = vec![0.0; 2000];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(o[0].is_finite());
    }

    #[test]
    fn test_band_split_three_bands() {
        let mut p =
            BandSplitPlugin::new_multiband(1, &[500.0, 5000.0], "LR24").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 3); // 1 channel * 3 bands
        let i = vec![1.0; 1000];
        let mut o = vec![0.0; 3000]; // 1000 frames * 3 output channels
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(o[0].is_finite());
        assert!(o[2999].is_finite());
    }

    #[test]
    fn test_band_split_four_bands() {
        let mut p =
            BandSplitPlugin::new_multiband(1, &[200.0, 2000.0, 10000.0], "LR24").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 4);
        let i = vec![1.0; 500];
        let mut o = vec![0.0; 2000]; // 500 frames * 4 output channels
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 500,
            },
        )
        .unwrap();
        assert!(o[0].is_finite());
        assert!(o[1999].is_finite());
    }

    #[test]
    fn test_band_split_stereo_three_bands() {
        let mut p =
            BandSplitPlugin::new_multiband(2, &[500.0, 5000.0], "LR24").unwrap();
        p.initialize(48000).unwrap();
        assert_eq!(p.input_channels(), 2);
        assert_eq!(p.output_channels(), 6); // 2 channels * 3 bands
        let i = vec![0.5; 200]; // 100 frames * 2 channels
        let mut o = vec![0.0; 600]; // 100 frames * 6 output channels
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 100,
            },
        )
        .unwrap();
        assert!(o[0].is_finite());
        assert!(o[599].is_finite());
    }

    #[test]
    fn test_band_split_from_params_3_bands() {
        let params = BandSplitPluginParams {
            frequencies: vec![],
            frequency: 500.0,
            num_bands: 3,
            crossover_type: "LR24".to_string(),
        };
        let p = BandSplitPlugin::from_params(1, &params).unwrap();
        assert_eq!(p.output_channels(), 3);
    }

    #[test]
    fn test_band_split_from_params_4_bands() {
        let params = BandSplitPluginParams {
            frequencies: vec![],
            frequency: 200.0,
            num_bands: 4,
            crossover_type: "LR24".to_string(),
        };
        let p = BandSplitPlugin::from_params(1, &params).unwrap();
        assert_eq!(p.output_channels(), 4);
    }

    #[test]
    fn test_band_split_dc_sums_to_unity() {
        // DC signal through 2-band split: low + high should sum ~1.0
        let mut p = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
        p.initialize(48000).unwrap();
        let n = 10000;
        let input = vec![1.0; n];
        let mut output = vec![0.0; n * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: n,
            },
        )
        .unwrap();
        // Last frame: low (idx n*2 - 2) + high (idx n*2 - 1) should sum near 1.0
        let low = output[n * 2 - 2];
        let high = output[n * 2 - 1];
        let sum = low + high;
        assert!(
            (sum - 1.0).abs() < 0.05,
            "DC sum should be near 1.0, got {} (low={}, high={})",
            sum,
            low,
            high
        );
    }

    #[test]
    fn test_band_split_too_many_bands() {
        // 5 bands (4 crossovers) should fail
        let result =
            BandSplitPlugin::new_multiband(1, &[200.0, 500.0, 2000.0, 8000.0], "LR24");
        assert!(result.is_err());
    }

    #[test]
    fn test_band_split_frequency_parameter() {
        let mut p =
            BandSplitPlugin::new_multiband(1, &[500.0, 5000.0], "LR24").unwrap();
        p.initialize(48000).unwrap();

        // Check frequency_2 parameter
        let val = p.get_parameter(&ParameterId("frequency_2".to_string()));
        assert!(val.is_some());
        if let Some(ParameterValue::Float(f)) = val {
            assert!((f - 5000.0).abs() < 1.0);
        }
    }
}
