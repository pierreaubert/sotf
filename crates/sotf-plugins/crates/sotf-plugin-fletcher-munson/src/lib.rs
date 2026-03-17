// ============================================================================
// Fletcher-Munson Loudness Compensation Plugin
// ============================================================================

use sotf_host::auto_gain::{AutoGain, AutoGainLoudnessType, AutoGainParams};
use sotf_host::param_specs::{find_by_key as pk, fletcher_munson::PARAMS as FM};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

use math_audio_dsp::fast_math::fast_pow10;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

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
    auto_gain_input_snapshot: Vec<f32>,
}

impl FletcherMunsonPlugin {
    pub fn new(num_channels: usize) -> Self {
        let sr = 44100;
        let bands = [
            FletcherMunsonBand::new(
                pk(FM, "band1_freq").default_f64(),
                pk(FM, "band1_q").default_f64(),
                pk(FM, "band1_max_gain").default_f64(),
                pk(FM, "band1_slope").default_f64(),
            ),
            FletcherMunsonBand::new(
                pk(FM, "band2_freq").default_f64(),
                pk(FM, "band2_q").default_f64(),
                pk(FM, "band2_max_gain").default_f64(),
                pk(FM, "band2_slope").default_f64(),
            ),
            FletcherMunsonBand::new(
                pk(FM, "band3_freq").default_f64(),
                pk(FM, "band3_q").default_f64(),
                pk(FM, "band3_max_gain").default_f64(),
                pk(FM, "band3_slope").default_f64(),
            ),
            FletcherMunsonBand::new(
                pk(FM, "band4_freq").default_f64(),
                pk(FM, "band4_q").default_f64(),
                pk(FM, "band4_max_gain").default_f64(),
                pk(FM, "band4_slope").default_f64(),
            ),
        ];

        let mut p = Self {
            num_channels,
            sample_rate: sr,
            playback_volume_db: pk(FM, "playback_volume_db").default_f64() as f32,
            reference_level_db: pk(FM, "reference_level_db").default_f64() as f32,
            bands,
            enabled: true,
            filters: vec![Vec::with_capacity(NUM_BANDS); num_channels],
            gain_smoothers: [Smoother::new(0.0, 50.0, sr); NUM_BANDS],
            compensation_smoother: Smoother::new(1.0, 50.0, sr),
            auto_gain: None,
            auto_gain_input_snapshot: vec![0.0; 4096 * num_channels],
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
            .set_target(fast_pow10(-max_g / 20.0));
    }

    fn rebuild_filters(&mut self) {
        let sr = self.sample_rate as f64;
        for ch in 0..self.num_channels {
            self.filters[ch].clear();
            for i in 0..NUM_BANDS {
                let b = &self.bands[i];
                self.filters[ch].push(Biquad::new(
                    BiquadFilterType::Peak,
                    b.frequency,
                    sr,
                    b.q,
                    self.gain_smoothers[i].current() as f64,
                ));
            }
        }
    }

    pub fn from_params(
        num_channels: usize,
        params: FletcherMunsonPluginParams,
    ) -> Result<Self, String> {
        let mut p = Self::new(num_channels);
        p.playback_volume_db = params.playback_volume_db;
        p.reference_level_db = params.reference_level_db;
        p.enabled = params.enabled;

        if params.auto_gain_enabled {
            p.auto_gain = Some(AutoGain::new(
                num_channels,
                p.sample_rate,
                AutoGainParams {
                    enabled: true,
                    loudness_type: AutoGainLoudnessType::Momentary,
                    max_gain_db: pk(FM, "auto_gain_max_db").default_f64() as f32,
                    smoothing_ms: pk(FM, "auto_gain_smoothing_ms").default_f64() as f32,
                },
            )?);
        }
        p.update_band_targets();
        Ok(p)
    }
}

impl InPlacePlugin for FletcherMunsonPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Fletcher-Munson", "2.0.0", "Sotf")
            .with_description("Loudness-dependent frequency compensation")
    }

    fn channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        let mut params = vec![
            Parameter::new_float(
                "playback_volume_db",
                "Playback Volume",
                self.playback_volume_db,
                pk(FM, "playback_volume_db").min_f64() as f32,
                pk(FM, "playback_volume_db").max_f64() as f32,
            )
            .with_group("Levels")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "reference_level_db",
                "Reference Level",
                self.reference_level_db,
                pk(FM, "reference_level_db").min_f64() as f32,
                pk(FM, "reference_level_db").max_f64() as f32,
            )
            .with_group("Levels"),
            Parameter::new_bool("enabled", "Enabled", self.enabled).with_group("Control"),
            Parameter::new_bool("auto_gain_enabled", "Auto Gain", self.auto_gain.is_some())
                .with_group("Auto Gain"),
            Parameter::new_float(
                "auto_gain_max_db",
                "AG Max",
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.max_gain_db())
                    .unwrap_or(pk(FM, "auto_gain_max_db").default_f32()),
                pk(FM, "auto_gain_max_db").min_f64() as f32,
                pk(FM, "auto_gain_max_db").max_f64() as f32,
            )
            .with_group("Auto Gain"),
            Parameter::new_float(
                "auto_gain_smoothing_ms",
                "AG Smoothing",
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.smoothing_ms())
                    .unwrap_or(pk(FM, "auto_gain_smoothing_ms").default_f32()),
                pk(FM, "auto_gain_smoothing_ms").min_f64() as f32,
                pk(FM, "auto_gain_smoothing_ms").max_f64() as f32,
            )
            .with_group("Auto Gain"),
        ];
        for (i, band) in self.bands.iter().enumerate() {
            let group = format!("Band {}", i + 1);
            let keys = [
                ("freq", "Freq", band.frequency as f32),
                ("q", "Q", band.q as f32),
                ("max_gain", "Max Gain", band.max_gain_db as f32),
                ("slope", "Slope", band.slope as f32),
            ];
            for (suffix, label, val) in keys {
                let key = format!("band{}_{}", i + 1, suffix);
                params.push(
                    Parameter::new_float(
                        &key,
                        label,
                        val,
                        pk(FM, &key).min_f64() as f32,
                        pk(FM, &key).max_f64() as f32,
                    )
                    .with_group(&group),
                );
            }
        }
        params
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        let name = id.0.as_str();
        if name == "playback_volume_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "playback_volume_db must be a float".to_string())?;
            if v.is_finite() {
                self.playback_volume_db = v;
                self.update_band_targets();
            }
        } else if name == "reference_level_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "reference_level_db must be a float".to_string())?;
            if v.is_finite() {
                self.reference_level_db = v;
                self.update_band_targets();
            }
        } else if name == "enabled" {
            self.enabled = value
                .as_bool()
                .ok_or_else(|| "enabled must be a boolean".to_string())?;
        } else if name == "auto_gain_enabled" {
            let v = value
                .as_bool()
                .ok_or_else(|| "auto_gain_enabled must be a boolean".to_string())?;
            if v && self.auto_gain.is_none() {
                self.auto_gain = Some(AutoGain::new(
                    self.num_channels,
                    self.sample_rate,
                    AutoGainParams {
                        enabled: true,
                        loudness_type: AutoGainLoudnessType::Momentary,
                        max_gain_db: pk(FM, "auto_gain_max_db").default_f32(),
                        smoothing_ms: pk(FM, "auto_gain_smoothing_ms").default_f32(),
                    },
                )?);
            } else if !v {
                self.auto_gain = None;
            }
        } else if name == "auto_gain_max_db" {
            let v = value
                .as_float()
                .ok_or_else(|| "auto_gain_max_db must be a float".to_string())?;
            if v.is_finite()
                && let Some(ag) = &mut self.auto_gain
            {
                ag.set_max_gain_db(v);
            }
        } else if name == "auto_gain_smoothing_ms" {
            let v = value
                .as_float()
                .ok_or_else(|| "auto_gain_smoothing_ms must be a float".to_string())?;
            if v.is_finite()
                && let Some(ag) = &mut self.auto_gain
            {
                ag.set_smoothing_ms(v);
            }
        } else if name.starts_with("band") && name.len() > 5 {
            // Parse band parameter: band1_freq, band2_q, etc.
            let v = value
                .as_float()
                .ok_or_else(|| format!("{} must be a float", name))?;
            if v.is_finite() {
                let band_idx = name.as_bytes()[4] - b'1';
                if band_idx < NUM_BANDS as u8 {
                    let field = &name[6..]; // skip "bandN_"
                    let band = &mut self.bands[band_idx as usize];
                    match field {
                        "freq" => {
                            band.frequency = v as f64;
                            self.rebuild_filters();
                        }
                        "q" => {
                            band.q = v as f64;
                            self.rebuild_filters();
                        }
                        "max_gain" => {
                            band.max_gain_db = v as f64;
                            self.update_band_targets();
                        }
                        "slope" => {
                            band.slope = v as f64;
                            self.update_band_targets();
                        }
                        _ => return Err(format!("Unknown band field: {}", field)),
                    }
                } else {
                    return Err(format!("Band index out of range: {}", name));
                }
            }
        } else {
            return Err(format!("Unknown: {}", id));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let name = id.0.as_str();
        if name == "playback_volume_db" {
            Some(ParameterValue::Float(self.playback_volume_db))
        } else if name == "reference_level_db" {
            Some(ParameterValue::Float(self.reference_level_db))
        } else if name == "enabled" {
            Some(ParameterValue::Bool(self.enabled))
        } else if name == "auto_gain_enabled" {
            Some(ParameterValue::Bool(self.auto_gain.is_some()))
        } else if name == "auto_gain_max_db" {
            Some(ParameterValue::Float(
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.max_gain_db())
                    .unwrap_or(pk(FM, "auto_gain_max_db").default_f32()),
            ))
        } else if name == "auto_gain_smoothing_ms" {
            Some(ParameterValue::Float(
                self.auto_gain
                    .as_ref()
                    .map(|ag| ag.smoothing_ms())
                    .unwrap_or(pk(FM, "auto_gain_smoothing_ms").default_f32()),
            ))
        } else if name.starts_with("band") && name.len() > 5 {
            let band_idx = name.as_bytes()[4].wrapping_sub(b'1');
            if band_idx < NUM_BANDS as u8 {
                let field = &name[6..];
                let band = &self.bands[band_idx as usize];
                match field {
                    "freq" => Some(ParameterValue::Float(band.frequency as f32)),
                    "q" => Some(ParameterValue::Float(band.q as f32)),
                    "max_gain" => Some(ParameterValue::Float(band.max_gain_db as f32)),
                    "slope" => Some(ParameterValue::Float(band.slope as f32)),
                    _ => None,
                }
            } else {
                None
            }
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
        if let Some(ag) = &mut self.auto_gain {
            ag.set_sample_rate(sr).map_err(|e| e.to_string())?;
        }
        self.auto_gain_input_snapshot
            .resize(4096 * self.num_channels, 0.0);
        Ok(())
    }

    fn reset(&mut self) {
        self.rebuild_filters();
        if let Some(ag) = &mut self.auto_gain {
            ag.reset();
        }
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

        if let Some(ag) = &mut self.auto_gain {
            let len = nf * self.num_channels;
            if self.auto_gain_input_snapshot.len() < len {
                self.auto_gain_input_snapshot.resize(len, 0.0);
            }
            self.auto_gain_input_snapshot[..len].copy_from_slice(&buffer[..len]);
            let _ = ag.measure_input(&self.auto_gain_input_snapshot[..len]);
        }

        // Update filters if gain changed significantly
        let mut gains = [0.0f32; NUM_BANDS];
        let mut changed = false;
        for (i, smoother) in self.gain_smoothers.iter_mut().enumerate().take(NUM_BANDS) {
            gains[i] = smoother.advance();
            if (gains[i] - self.filters[0][i].db_gain as f32).abs() > 0.05 {
                changed = true;
            }
        }
        if changed {
            let sr = self.sample_rate as f64;
            for ch in 0..self.num_channels {
                for (filter, (band, &gain)) in self.filters[ch]
                    .iter_mut()
                    .zip(self.bands.iter().zip(gains.iter()))
                    .take(NUM_BANDS)
                {
                    *filter = Biquad::new(
                        BiquadFilterType::Peak,
                        band.frequency,
                        sr,
                        band.q,
                        gain as f64,
                    );
                }
            }
        }

        let comp = self.compensation_smoother.next_n(nf);
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

        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_output(buffer);
            ag.apply_compensation(buffer, nf);
        }

        self.compensation_smoother.next_n(nf); // Sync smoother
        for s in &mut self.gain_smoothers {
            s.next_n(nf);
        }

        flush_denormals_inplace(buffer);
        Ok(nf)
    }
}

impl sotf_host::plugin::Plugin for FletcherMunsonPlugin {
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
    use sotf_host::plugin::{InPlacePlugin, ProcessContext};
    #[test]
    fn test_fm_basic() {
        let mut p = FletcherMunsonPlugin::new(1);
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
        assert!(b[999].is_finite());
    }
}
