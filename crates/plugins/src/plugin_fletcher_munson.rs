// ============================================================================
// Fletcher-Munson Loudness Compensation Plugin
// ============================================================================

use super::auto_gain::{AutoGain, AutoGainLoudnessType, AutoGainParams};
use super::param_specs::fletcher_munson::*;
use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::{enable_ftz_daz, flush_denormals_inplace};
use super::smoothing::Smoother;

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FletcherMunsonBand {
    pub frequency: f64,
    pub q: f64,
    pub max_gain_db: f64,
    pub slope: f64,
}

impl FletcherMunsonBand {
    pub fn new(freq: f64, q: f64, max: f64, slp: f64) -> Self {
        Self {
            frequency: freq,
            q,
            max_gain_db: max,
            slope: slp,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FletcherMunsonPluginParams {
    pub playback_volume_db: f32,
    pub reference_level_db: f32,
    pub band1: Option<FletcherMunsonBand>,
    pub band2: Option<FletcherMunsonBand>,
    pub band3: Option<FletcherMunsonBand>,
    pub band4: Option<FletcherMunsonBand>,
    pub smoothing_ms: f32,
    pub enabled: bool,
    pub auto_gain_enabled: bool,
}

const NUM_BANDS: usize = 4;

pub struct FletcherMunsonPlugin {
    num_channels: usize,
    sample_rate: u32,
    playback_volume_db: f32,
    reference_level_db: f32,
    bands: [FletcherMunsonBand; NUM_BANDS],
    enabled: bool,
    filters: Vec<Vec<Biquad>>,
    gain_smoothers: [Smoother; NUM_BANDS],
    compensation_smoother: Smoother,
    auto_gain: Option<AutoGain>,
}

impl FletcherMunsonPlugin {
    pub fn new(num_channels: usize) -> Self {
        let sr = 48000;
        let bands = [
            FletcherMunsonBand::new(
                BAND1_FREQ_DEFAULT,
                BAND1_Q_DEFAULT,
                BAND1_MAX_GAIN_DEFAULT,
                BAND1_SLOPE_DEFAULT,
            ),
            FletcherMunsonBand::new(
                BAND2_FREQ_DEFAULT,
                BAND2_Q_DEFAULT,
                BAND2_MAX_GAIN_DEFAULT,
                BAND2_SLOPE_DEFAULT,
            ),
            FletcherMunsonBand::new(
                BAND3_FREQ_DEFAULT,
                BAND3_Q_DEFAULT,
                BAND3_MAX_GAIN_DEFAULT,
                BAND3_SLOPE_DEFAULT,
            ),
            FletcherMunsonBand::new(
                BAND4_FREQ_DEFAULT,
                BAND4_Q_DEFAULT,
                BAND4_MAX_GAIN_DEFAULT,
                BAND4_SLOPE_DEFAULT,
            ),
        ];
        let mut p = Self {
            num_channels,
            sample_rate: sr,
            playback_volume_db: PLAYBACK_VOLUME_DB_DEFAULT,
            reference_level_db: REFERENCE_LEVEL_DB_DEFAULT,
            bands,
            enabled: true,
            filters: vec![Vec::new(); num_channels],
            gain_smoothers: [Smoother::new(0.0, 50.0, sr); NUM_BANDS],
            compensation_smoother: Smoother::new(1.0, 50.0, sr),
            auto_gain: None,
        };
        p.rebuild_filters();
        p.update_band_targets();
        p
    }

    fn update_band_targets(&mut self) {
        let delta = self.reference_level_db - self.playback_volume_db;
        let mut max_g = 0.0f32;
        for i in 0..NUM_BANDS {
            let g = if delta <= 0.0 {
                0.0
            } else {
                (self.bands[i].slope as f32 * delta).min(self.bands[i].max_gain_db as f32)
            };
            self.gain_smoothers[i].set_target(g);
            max_g = max_g.max(g);
        }
        self.compensation_smoother
            .set_target(10.0_f32.powf(-max_g / 20.0));
    }

    fn rebuild_filters(&mut self) {
        let sr = self.sample_rate as f64;
        for ch in 0..self.num_channels {
            self.filters[ch] = self
                .bands
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    Biquad::new(
                        BiquadFilterType::Peak,
                        b.frequency,
                        sr,
                        b.q,
                        self.gain_smoothers[i].current() as f64,
                    )
                })
                .collect();
        }
    }

    pub fn from_params(num_channels: usize, params: FletcherMunsonPluginParams) -> Self {
        let mut p = Self::new(num_channels);
        p.playback_volume_db = params.playback_volume_db;
        p.reference_level_db = params.reference_level_db;
        p.enabled = params.enabled;
        p.update_band_targets();
        p
    }
}

impl InPlacePlugin for FletcherMunsonPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Fletcher-Munson", "1.1.0", "Sotf")
    }
    fn channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float(
            "playback_volume_db",
            "Volume",
            0.0,
            -100.0,
            0.0,
        )]
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id.0 == "playback_volume_db" {
            self.playback_volume_db = value.as_float().ok_or("val")?;
            self.update_band_targets();
        }
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.0 == "playback_volume_db" {
            Some(ParameterValue::Float(self.playback_volume_db))
        } else {
            None
        }
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        for s in &mut self.gain_smoothers {
            s.set_time(50.0, sr);
        }
        self.compensation_smoother.set_time(50.0, sr);
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
        if !self.enabled {
            return Ok(context.num_frames);
        }
        enable_ftz_daz();
        let nf = context.num_frames;

        // Update coefficients once per block
        let mut gains = [0.0f32; NUM_BANDS];
        let mut changed = false;
        for i in 0..NUM_BANDS {
            gains[i] = self.gain_smoothers[i].next();
            if (gains[i] - self.filters[0][i].db_gain as f32).abs() > 0.1 {
                changed = true;
            }
        }
        if changed {
            let sr = self.sample_rate as f64;
            for ch in 0..self.num_channels {
                for i in 0..NUM_BANDS {
                    let b = &self.bands[i];
                    self.filters[ch][i] = Biquad::new(
                        BiquadFilterType::Peak,
                        b.frequency,
                        sr,
                        b.q,
                        gains[i] as f64,
                    );
                }
            }
        }

        let comp = self.compensation_smoother.next();
        for frame in 0..nf {
            for ch in 0..self.num_channels {
                let idx = frame * self.num_channels + ch;
                let mut s = buffer[idx] as f64;
                for f in &mut self.filters[ch] {
                    s = f.process(s);
                }
                buffer[idx] = (s as f32) * comp;
            }
        }
        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}

impl crate::plugin::Plugin for FletcherMunsonPlugin {
    fn info(&self) -> PluginInfo {
        InPlacePlugin::info(self)
    }
    fn input_channels(&self) -> usize {
        self.num_channels
    }
    fn output_channels(&self) -> usize {
        self.num_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        InPlacePlugin::parameters(self)
    }
    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> PluginResult<()> {
        InPlacePlugin::set_parameter(self, id, val)
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        InPlacePlugin::get_parameter(self, id)
    }
    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        InPlacePlugin::initialize(self, sr)
    }
    fn reset(&mut self) {
        InPlacePlugin::reset(self)
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output.copy_from_slice(input);
        self.process_in_place(output, context)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fm_basic() {
        let mut p = FletcherMunsonPlugin::new(1);
        p.initialize(48000).unwrap();
        let mut b = vec![0.5; 1000];
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
