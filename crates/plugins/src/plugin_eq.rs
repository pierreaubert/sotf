// ============================================================================
// Parametric EQ Plugin
// ============================================================================

use super::auto_gain::{AutoGain, AutoGainParams};
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use math_audio_iir_fir::Biquad;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiquadFilterConfig {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    #[serde(default)]
    pub db_gain: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EqPluginParams {
    #[serde(default)]
    pub filters: Vec<BiquadFilterConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_filters: Option<Vec<Vec<BiquadFilterConfig>>>,
    #[serde(default)]
    pub auto_gain: AutoGainParams,
}

pub struct EqPlugin {
    num_channels: usize,
    filters: Vec<Vec<Biquad>>,
    sample_rate: u32,
    auto_gain: AutoGain,
}

impl EqPlugin {
    pub fn new(num_channels: usize, filters: Vec<Biquad>) -> Self {
        let mut channel_filters = Vec::with_capacity(num_channels);
        for _ in 0..num_channels {
            channel_filters.push(filters.clone());
        }
        let sample_rate = 48000;
        let auto_gain = AutoGain::new_default(num_channels, sample_rate).expect("ag");
        Self {
            num_channels,
            filters: channel_filters,
            sample_rate,
            auto_gain,
        }
    }

    pub fn new_per_channel(
        num_channels: usize,
        channel_filters: Vec<Vec<Biquad>>,
    ) -> Result<Self, String> {
        if channel_filters.len() != num_channels {
            return Err("Count mismatch".into());
        }
        let sample_rate = 48000;
        let auto_gain = AutoGain::new_default(num_channels, sample_rate)?;
        Ok(Self {
            num_channels,
            filters: channel_filters,
            sample_rate,
            auto_gain,
        })
    }

    pub fn from_params(
        num_channels: usize,
        sample_rate: u32,
        params: EqPluginParams,
    ) -> Result<Self, String> {
        use math_audio_iir_fir::BiquadFilterType;
        let config_to_biquad = |f: &BiquadFilterConfig| -> Result<Biquad, String> {
            let filter_type = match f.filter_type.as_str() {
                "peak" => BiquadFilterType::Peak,
                "lowshelf" => BiquadFilterType::Lowshelf,
                "highshelf" => BiquadFilterType::Highshelf,
                "lowpass" => BiquadFilterType::Lowpass,
                "highpass" => BiquadFilterType::Highpass,
                "notch" => BiquadFilterType::Notch,
                "bandpass" => BiquadFilterType::Bandpass,
                other => return Err(format!("Type: {}", other)),
            };
            Biquad::try_new(filter_type, f.freq, sample_rate as f64, f.q, f.db_gain)
                .map_err(|e| e.to_string())
        };
        let auto_gain = AutoGain::new(num_channels, sample_rate, params.auto_gain)?;
        if let Some(cfgs) = params.channel_filters {
            if cfgs.len() != num_channels {
                return Err("Mismatched chains".into());
            }
            let mut channel_filters = Vec::with_capacity(num_channels);
            for c in cfgs {
                channel_filters.push(
                    c.iter()
                        .map(config_to_biquad)
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            Ok(Self {
                num_channels,
                filters: channel_filters,
                sample_rate,
                auto_gain,
            })
        } else {
            let filters = params
                .filters
                .iter()
                .map(config_to_biquad)
                .collect::<Result<Vec<_>, _>>()?;
            let mut channel_filters = Vec::with_capacity(num_channels);
            for _ in 0..num_channels {
                channel_filters.push(filters.clone());
            }
            Ok(Self {
                num_channels,
                filters: channel_filters,
                sample_rate,
                auto_gain,
            })
        }
    }

    pub fn set_filters(&mut self, filters: Vec<Biquad>) {
        self.filters.clear();
        for _ in 0..self.num_channels {
            self.filters.push(filters.clone());
        }
    }

    pub fn set_channel_filters(&mut self, channel_filters: Vec<Vec<Biquad>>) -> Result<(), String> {
        if channel_filters.len() != self.num_channels {
            return Err("mismatch".into());
        }
        self.filters = channel_filters;
        Ok(())
    }
}

impl Plugin for EqPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Parametric EQ", "1.1.0", "SotF")
    }
    fn input_channels(&self) -> usize {
        self.num_channels
    }
    fn output_channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_bool(
            "auto_gain_enabled",
            "Auto Gain",
            self.auto_gain.is_enabled(),
        )]
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id.0 == "auto_gain_enabled" {
            if let Some(v) = value.as_bool() {
                self.auto_gain.set_enabled(v);
            }
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "auto_gain_enabled" {
            Some(ParameterValue::Bool(self.auto_gain.is_enabled()))
        } else {
            None
        }
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        for chain in &mut self.filters {
            for f in chain {
                *f = Biquad::new(f.filter_type, f.freq, sample_rate as f64, f.q, f.db_gain);
            }
        }
        self.auto_gain
            .set_sample_rate(sample_rate)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn reset(&mut self) {
        for chain in &mut self.filters {
            for f in chain {
                *f = Biquad::new(f.filter_type, f.freq, f.srate, f.q, f.db_gain);
            }
        }
        self.auto_gain.reset();
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        output.copy_from_slice(input);
        self.auto_gain
            .measure_input(output)
            .map_err(|e| e.to_string())?;

        for frame in 0..num_frames {
            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let mut s = output[idx] as f64;
                for f in &mut self.filters[ch] {
                    s = f.process(s);
                }
                output[idx] = s as f32;
            }
        }

        self.auto_gain
            .measure_output(output)
            .map_err(|e| e.to_string())?;
        self.auto_gain.apply_compensation(output, num_frames);
        flush_denormals_inplace(output);
        Ok(num_frames)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(Arc::new(self.auto_gain.get_data()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    #[test]
    fn test_eq_passthrough() {
        let mut p = EqPlugin::new(2, vec![]);
        p.initialize(48000).unwrap();
        let i = vec![0.5; 2048];
        let mut o = vec![0.0; 2048];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1024,
            },
        )
        .unwrap();
        assert_eq!(i, o);
    }

    #[test]
    fn test_eq_boost() {
        let f = vec![Biquad::new(
            BiquadFilterType::Highshelf,
            1000.0,
            48000.0,
            0.707,
            6.0,
        )];
        let mut p = EqPlugin::new(1, f);
        p.initialize(48000).unwrap();
        p.set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
        let mut i = vec![0.0; 1024];
        for k in 0..1024 {
            i[k] = (k as f32 * 0.1).sin();
        }
        let mut o = vec![0.0; 1024];
        p.process(
            &i,
            &mut o,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1024,
            },
        )
        .unwrap();
        // Check a sample after some settling
        assert!(o[100].abs() > i[100].abs());
    }
}
