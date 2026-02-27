// ============================================================================
// Loudness Compensation Plugin
// ============================================================================

use sotf_host::analyzer::RealTimeCache;
use sotf_host::auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
use sotf_host::param_specs::{find_by_key as pk, loudness_compensation::PARAMS as LC};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoudnessCompensation {
    pub reference_level: f64,
    pub low_boost: f64,
    pub high_boost: f64,
    #[serde(default)]
    pub attenuate_mid: bool,
}

impl LoudnessCompensation {
    pub fn new(reference_level: f64, low_boost: f64, high_boost: f64) -> Result<Self, String> {
        Ok(Self {
            reference_level,
            low_boost,
            high_boost,
            attenuate_mid: false,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLoudnessParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
}

fn default_low_freq() -> f32 {
    pk(LC, "low_freq").default_f32()
}
fn default_low_gain() -> f32 {
    pk(LC, "low_gain").default_f32()
}
fn default_high_freq() -> f32 {
    pk(LC, "high_freq").default_f32()
}
fn default_high_gain() -> f32 {
    pk(LC, "high_gain").default_f32()
}

impl Default for ChannelLoudnessParams {
    fn default() -> Self {
        Self {
            low_freq: default_low_freq(),
            low_gain: default_low_gain(),
            high_freq: default_high_freq(),
            high_gain: default_high_gain(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessCompensationPluginParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
    #[serde(default)]
    pub channel_params: Vec<ChannelLoudnessParams>,
    #[serde(default)]
    pub auto_gain_enabled: bool,
    #[serde(default)]
    pub auto_gain_max_db: f32,
    #[serde(default)]
    pub auto_gain_smoothing_ms: f32,
}

pub struct LoudnessCompensationPlugin {
    num_channels: usize,
    sample_rate: u32,
    low_freq: f32,
    low_gain: f32,
    high_freq: f32,
    high_gain: f32,
    filters: Vec<Vec<Biquad>>,
    auto_gain: Option<AutoGain>,
    comp_gain_smoother: Vec<Smoother>,
    cache: RealTimeCache<AutoGainData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
}

impl LoudnessCompensationPlugin {
    pub fn new(
        num_channels: usize,
        low_freq: f32,
        low_gain: f32,
        high_freq: f32,
        high_gain: f32,
    ) -> Self {
        let sr = 48000;
        let mut p = Self {
            num_channels,
            sample_rate: sr,
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            filters: vec![Vec::new(); num_channels],
            auto_gain: None,
            comp_gain_smoother: (0..num_channels)
                .map(|_| Smoother::new(1.0, 20.0, sr))
                .collect(),
            cache: RealTimeCache::new(AutoGainData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_filters();
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "low_gain",
                "Bass Boost",
                self.low_gain,
                pk(LC, "low_gain").min_f64() as f32,
                pk(LC, "low_gain").max_f64() as f32,
            )
            .with_description("Low-frequency shelf gain (dB)")
            .with_group("Gain")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "high_gain",
                "Treble Boost",
                self.high_gain,
                pk(LC, "high_gain").min_f64() as f32,
                pk(LC, "high_gain").max_f64() as f32,
            )
            .with_description("High-frequency shelf gain (dB)")
            .with_group("Gain")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "low_freq",
                "Low Frequency",
                self.low_freq,
                pk(LC, "low_freq").min_f64() as f32,
                pk(LC, "low_freq").max_f64() as f32,
            )
            .with_description("Low shelf center frequency (Hz)")
            .with_group("Frequency")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "high_freq",
                "High Frequency",
                self.high_freq,
                pk(LC, "high_freq").min_f64() as f32,
                pk(LC, "high_freq").max_f64() as f32,
            )
            .with_description("High shelf center frequency (Hz)")
            .with_group("Frequency")
            .with_importance(ParameterImportance::Useful),
        ];
    }

    fn rebuild_filters(&mut self) {
        let q = 0.707;
        let sr = self.sample_rate as f64;
        for ch in 0..self.num_channels {
            let lg = self.low_gain / 2.0;
            let hg = self.high_gain / 2.0;
            self.filters[ch] = vec![
                Biquad::new(
                    BiquadFilterType::Lowshelf,
                    self.low_freq as f64,
                    sr,
                    q,
                    lg as f64,
                ),
                Biquad::new(
                    BiquadFilterType::Lowshelf,
                    self.low_freq as f64,
                    sr,
                    q,
                    lg as f64,
                ),
                Biquad::new(
                    BiquadFilterType::Highshelf,
                    self.high_freq as f64,
                    sr,
                    q,
                    hg as f64,
                ),
                Biquad::new(
                    BiquadFilterType::Highshelf,
                    self.high_freq as f64,
                    sr,
                    q,
                    hg as f64,
                ),
            ];
            let target = 10.0_f32.powf(-self.low_gain.max(self.high_gain) / 20.0);
            self.comp_gain_smoother[ch].set_target(target);
        }
    }

    pub fn from_params(
        num_channels: usize,
        params: LoudnessCompensationPluginParams,
    ) -> Result<Self, String> {
        let mut p = Self::new(
            num_channels,
            params.low_freq,
            params.low_gain,
            params.high_freq,
            params.high_gain,
        );
        if params.auto_gain_enabled {
            p.auto_gain = Some(AutoGain::new(
                num_channels,
                p.sample_rate,
                AutoGainParams {
                    enabled: true,
                    loudness_type: AutoGainLoudnessType::Momentary,
                    max_gain_db: params.auto_gain_max_db,
                    smoothing_ms: params.auto_gain_smoothing_ms,
                },
            )?);
        }
        p.rebuild_cached_parameters();
        Ok(p)
    }
}

impl InPlacePlugin for LoudnessCompensationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Loudness Compensation", "2.0.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id.0 == "low_gain" {
            let v = value.as_float().unwrap_or(pk(LC, "low_gain").default_f32());
            if v.is_finite() {
                self.low_gain = v;
                self.rebuild_filters();
            }
        } else if id.0 == "high_gain" {
            let v = value.as_float().unwrap_or(pk(LC, "high_gain").default_f32());
            if v.is_finite() {
                self.high_gain = v;
                self.rebuild_filters();
            }
        } else if id.0 == "low_freq" {
            let v = value.as_float().unwrap_or(pk(LC, "low_freq").default_f32());
            if v.is_finite() {
                self.low_freq = v;
                self.rebuild_filters();
            }
        } else if id.0 == "high_freq" {
            let v = value.as_float().unwrap_or(pk(LC, "high_freq").default_f32());
            if v.is_finite() {
                self.high_freq = v;
                self.rebuild_filters();
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "low_gain" {
            Some(ParameterValue::Float(self.low_gain))
        } else if id.0 == "high_gain" {
            Some(ParameterValue::Float(self.high_gain))
        } else if id.0 == "low_freq" {
            Some(ParameterValue::Float(self.low_freq))
        } else if id.0 == "high_freq" {
            Some(ParameterValue::Float(self.high_freq))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        for s in &mut self.comp_gain_smoother {
            s.set_time(20.0, sr);
        }
        self.rebuild_filters();
        Ok(())
    }
    fn reset(&mut self) {
        self.rebuild_filters();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let nf = context.num_frames;

        // Throttled measurement
        self.cache_update_counter += 1;
        let mut do_measure = false;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            do_measure = true;
        }

        if let Some(ag) = &mut self.auto_gain {
            if do_measure {
                let _ = ag.measure_input(buffer);
            }
        }

        for frame in 0..nf {
            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let mut s = buffer[idx] as f64;
                for f in &mut self.filters[ch] {
                    s = f.process(s);
                }
                buffer[idx] = (s as f32) * self.comp_gain_smoother[ch].advance();
            }
        }

        if let Some(ag) = &mut self.auto_gain {
            if do_measure {
                let _ = ag.measure_output(buffer);
                // Update diagnostic cache
                let data = ag.get_data();
                self.cache.update(|d| {
                    *d = data;
                });
            }
            ag.apply_compensation(buffer, nf);
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        if self.auto_gain.is_some() {
            Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use sotf_host::*;
    use crate::*;
    #[test]
    fn test_loudness_basic() {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        InPlacePlugin::initialize(&mut p, 48000).unwrap();
        let mut b = vec![0.5; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999] > 0.0);
    }
}
