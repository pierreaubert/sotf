// ============================================================================
// Band Split Plugin
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use super::smoothing::LogSmoother;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandSplitPluginParams {
    pub frequency: f64,
    #[serde(rename = "type", default = "default_crossover_type")]
    pub crossover_type: String,
}

fn default_crossover_type() -> String {
    "LR24".to_string()
}

pub struct BandSplitPlugin {
    input_channels: usize,
    sample_rate: u32,
    crossover_type: String,
    lowpass: Vec<Vec<Biquad>>,
    highpass: Vec<Vec<Biquad>>,
    freq_smoother: LogSmoother,
}

impl BandSplitPlugin {
    pub fn new(
        input_channels: usize,
        frequency: f64,
        crossover_type: &str,
    ) -> Result<Self, String> {
        let sr = 48000;
        let mut p = Self {
            input_channels,
            sample_rate: sr,
            crossover_type: crossover_type.to_string(),
            lowpass: vec![Vec::new(); input_channels],
            highpass: vec![Vec::new(); input_channels],
            freq_smoother: LogSmoother::new(frequency as f32, 20.0, sr),
        };
        p.build_filters(frequency);
        Ok(p)
    }

    pub fn from_params(
        input_channels: usize,
        params: &BandSplitPluginParams,
    ) -> Result<Self, String> {
        Self::new(input_channels, params.frequency, &params.crossover_type)
    }

    fn build_filters(&mut self, freq: f64) {
        let n_sects = match self.crossover_type.to_uppercase().as_str() {
            "LR24" | "LR4" => 2,
            "LR48" | "LR8" => 4,
            _ => 2,
        };
        let q = 1.0 / std::f64::consts::SQRT_2;
        let sr = self.sample_rate as f64;
        for ch in 0..self.input_channels {
            let mut lp = Vec::with_capacity(n_sects);
            let mut hp = Vec::with_capacity(n_sects);
            for _ in 0..n_sects {
                lp.push(Biquad::new(BiquadFilterType::Lowpass, freq, sr, q, 0.0));
                hp.push(Biquad::new(BiquadFilterType::Highpass, freq, sr, q, 0.0));
            }
            self.lowpass[ch] = lp;
            self.highpass[ch] = hp;
        }
    }
}

impl Plugin for BandSplitPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandSplit", "1.1.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.input_channels
    }
    fn output_channels(&self) -> usize {
        self.input_channels * 2
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float(
            "frequency",
            "Frequency",
            1000.0,
            20.0,
            20000.0,
        )]
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id.0 == "frequency" {
            self.freq_smoother
                .set_target(value.as_float().ok_or("val")?);
            Ok(())
        } else {
            Err("unknown".into())
        }
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "frequency" {
            Some(ParameterValue::Float(self.freq_smoother.target()))
        } else {
            None
        }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.freq_smoother = LogSmoother::new(self.freq_smoother.target(), 20.0, sample_rate);
        self.build_filters(self.freq_smoother.target() as f64);
        Ok(())
    }
    fn reset(&mut self) {
        self.build_filters(self.freq_smoother.target() as f64);
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
        let out_ch = in_ch * 2;
        
        // Block-based smoothing
        let new_freq = self.freq_smoother.next_n(num_frames);

        if (new_freq - self.lowpass[0][0].freq as f32).abs() > 0.1 {
            let f64 = new_freq as f64;
            let q = 1.0 / std::f64::consts::SQRT_2;
            let sr = self.sample_rate as f64;
            for ch in 0..in_ch {
                for f in &mut self.lowpass[ch] {
                    *f = Biquad::new(BiquadFilterType::Lowpass, f64, sr, q, 0.0);
                }
                for f in &mut self.highpass[ch] {
                    *f = Biquad::new(BiquadFilterType::Highpass, f64, sr, q, 0.0);
                }
            }
        }

        for frame in 0..num_frames {
            let in_off = frame * in_ch;
            let out_off = frame * out_ch;
            for ch in 0..in_ch {
                let s = input[in_off + ch];
                let mut lp_s = s as f64;
                for f in &mut self.lowpass[ch] {
                    lp_s = f.process(lp_s);
                }
                output[out_off + ch] = lp_s as f32;

                let mut hp_s = s as f64;
                for f in &mut self.highpass[ch] {
                    hp_s = f.process(hp_s);
                }
                output[out_off + in_ch + ch] = hp_s as f32;
            }
        }
        
        flush_denormals_inplace(output);
        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
