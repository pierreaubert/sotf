// ============================================================================
// Crossover Plugin
// ============================================================================

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::LogSmoother;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossoverPluginParams {
    #[serde(rename = "type")]
    pub crossover_type: String,
    pub frequency: f64,
    pub output: String,
}

pub struct CrossoverPlugin {
    num_channels: usize,
    filters: Vec<Vec<Biquad>>,
    sample_rate: u32,
    crossover_type: String,
    is_highpass: bool,
    freq_smoother: LogSmoother,
    cached_parameters: Vec<Parameter>,
}

impl CrossoverPlugin {
    pub fn new(
        num_channels: usize,
        crossover_type: &str,
        frequency: f64,
        output: &str,
    ) -> Result<Self, String> {
        let is_highpass = match output.to_lowercase().as_str() {
            "high" | "highpass" | "hp" => true,
            "low" | "lowpass" | "lp" => false,
            _ => return Err(format!("Invalid output: {}", output)),
        };
        let sr = 48000;
        let mut p = Self {
            num_channels,
            filters: vec![Vec::new(); num_channels],
            sample_rate: sr,
            crossover_type: crossover_type.to_string(),
            is_highpass,
            freq_smoother: LogSmoother::new(frequency as f32, 20.0, sr),
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![Parameter::new_float(
            "frequency",
            "Frequency",
            self.freq_smoother.target(),
            20.0,
            20000.0,
        )];
    }

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

    fn build_filters(&mut self, freq: f64) {
        let q = 1.0 / std::f64::consts::SQRT_2;
        let ftype = if self.is_highpass {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Lowpass
        };
        let n_sects = match self.crossover_type.to_lowercase().as_str() {
            "lr24" | "lr4" => 2,
            "lr48" | "lr8" => 4,
            "bw12" | "bw1" => 1,
            _ => 2,
        };
        for ch in 0..self.num_channels {
            let mut sects = Vec::with_capacity(n_sects);
            for _ in 0..n_sects {
                sects.push(Biquad::new(ftype, freq, self.sample_rate as f64, q, 0.0));
            }
            self.filters[ch] = sects;
        }
    }
}

impl InPlacePlugin for CrossoverPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Crossover", "1.1.0", "SotF")
    }
    fn channels(&self) -> usize {
        self.num_channels
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

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;

        // Block-based smoothing
        let new_freq = self.freq_smoother.next_n(num_frames);

        if (new_freq - self.filters[0][0].freq as f32).abs() > 0.1 {
            let f64 = new_freq as f64;
            let q = 1.0 / std::f64::consts::SQRT_2;
            let ftype = self.filters[0][0].filter_type;
            let sr = self.sample_rate as f64;
            for ch in 0..self.num_channels {
                for f in &mut self.filters[ch] {
                    *f = Biquad::new(ftype, f64, sr, q, 0.0);
                }
            }
        }

        for frame in 0..num_frames {
            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let mut s = buffer[idx] as f64;
                for f in &mut self.filters[ch] {
                    s = f.process(s);
                }
                buffer[idx] = s as f32;
            }
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use sotf_host::*;
    #[test]
    fn test_crossover_basic() {
        let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        p.initialize(48000).unwrap();
        let mut b = vec![1.0; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999].is_finite());
    }
}
