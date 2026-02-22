// ============================================================================
// Crossfeed Plugin - Headphone crossfeed for speaker-like listening
// ============================================================================

use crate::param_specs::crossfeed::*;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use crate::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use crate::simd::{deinterleave_stereo, enable_ftz_daz, interleave_stereo};
use crate::smoothing::Smoother;

use math_audio_dsp::fast_math::fast_pow10;
use math_audio_iir_fir::Biquad;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossfeedMode {
    #[default]
    Off,
    Bauer,
    Meier,
    Mb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossfeedPreset {
    #[default]
    Default,
    Cmoy,
    Meier,
    Mb,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossfeedPluginParams {
    #[serde(default)]
    pub mode: CrossfeedMode,
    #[serde(default)]
    pub preset: CrossfeedPreset,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default = "default_mix")]
    pub mix: f32,

    // Bauer
    #[serde(default = "default_bauer_fcut")]
    pub bauer_fcut_hz: f32,
    #[serde(default = "default_bauer_feed")]
    pub bauer_feed_db: f32,

    // Meier
    #[serde(default = "default_meier_level")]
    pub meier_level: f32,

    // Multiband
    #[serde(default = "default_mb_low_freq")]
    pub mb_low_freq_hz: f32,
    #[serde(default = "default_mb_mid_high_freq")]
    pub mb_mid_high_freq_hz: f32,
    #[serde(default = "default_mb_low_feed")]
    pub mb_low_feed_db: f32,
    #[serde(default = "default_mb_mid_feed")]
    pub mb_mid_feed_db: f32,
    #[serde(default = "default_mb_high_feed")]
    pub mb_high_feed_db: f32,

    // Auto gain
    #[serde(default)]
    pub autogain_enabled: bool,
    #[serde(default = "default_autogain_target")]
    pub autogain_target_lufs: f32,
    #[serde(default = "default_autogain_max_gain")]
    pub autogain_max_gain_db: f32,
    #[serde(default = "default_autogain_smoothing")]
    pub autogain_smoothing_ms: f32,
}

fn default_enabled() -> bool {
    true
}

fn default_mix() -> f32 {
    MIX_DEFAULT
}

fn default_bauer_fcut() -> f32 {
    BAUER_FCUT_DEFAULT
}

fn default_bauer_feed() -> f32 {
    BAUER_FEED_DEFAULT
}

fn default_meier_level() -> f32 {
    MEIER_LEVEL_DEFAULT
}

fn default_mb_low_freq() -> f32 {
    MB_LOW_FREQ_DEFAULT
}

fn default_mb_mid_high_freq() -> f32 {
    MB_MID_HIGH_FREQ_DEFAULT
}

fn default_mb_low_feed() -> f32 {
    MB_LOW_FEED_DEFAULT
}

fn default_mb_mid_feed() -> f32 {
    MB_MID_FEED_DEFAULT
}

fn default_mb_high_feed() -> f32 {
    MB_HIGH_FEED_DEFAULT
}

fn default_autogain_target() -> f32 {
    AUTOGAIN_TARGET_DEFAULT
}

fn default_autogain_max_gain() -> f32 {
    AUTOGAIN_MAX_GAIN_DEFAULT
}

fn default_autogain_smoothing() -> f32 {
    AUTOGAIN_SMOOTHING_DEFAULT
}

impl Default for CrossfeedPluginParams {
    fn default() -> Self {
        Self {
            mode: CrossfeedMode::Bauer,
            preset: CrossfeedPreset::Default,
            enabled: true,
            mix: 1.0,
            bauer_fcut_hz: BAUER_FCUT_DEFAULT,
            bauer_feed_db: BAUER_FEED_DEFAULT,
            meier_level: MEIER_LEVEL_DEFAULT,
            mb_low_freq_hz: MB_LOW_FREQ_DEFAULT,
            mb_mid_high_freq_hz: MB_MID_HIGH_FREQ_DEFAULT,
            mb_low_feed_db: MB_LOW_FEED_DEFAULT,
            mb_mid_feed_db: MB_MID_FEED_DEFAULT,
            mb_high_feed_db: MB_HIGH_FEED_DEFAULT,
            autogain_enabled: false,
            autogain_target_lufs: AUTOGAIN_TARGET_DEFAULT,
            autogain_max_gain_db: AUTOGAIN_MAX_GAIN_DEFAULT,
            autogain_smoothing_ms: AUTOGAIN_SMOOTHING_DEFAULT,
        }
    }
}

impl CrossfeedPluginParams {
    pub fn from_preset(preset: CrossfeedPreset) -> Self {
        match preset {
            CrossfeedPreset::Off => Self {
                mode: CrossfeedMode::Off,
                ..Default::default()
            },
            CrossfeedPreset::Default => Self {
                mode: CrossfeedMode::Bauer,
                bauer_fcut_hz: 700.0,
                bauer_feed_db: 4.5,
                ..Default::default()
            },
            CrossfeedPreset::Cmoy => Self {
                mode: CrossfeedMode::Bauer,
                bauer_fcut_hz: 700.0,
                bauer_feed_db: 6.0,
                ..Default::default()
            },
            CrossfeedPreset::Meier => Self {
                mode: CrossfeedMode::Meier,
                meier_level: 30.0,
                ..Default::default()
            },
            CrossfeedPreset::Mb => Self {
                mode: CrossfeedMode::Mb,
                mb_low_freq_hz: 150.0,
                mb_mid_high_freq_hz: 5700.0,
                mb_low_feed_db: 0.0,
                mb_mid_feed_db: 6.0,
                mb_high_feed_db: 3.0,
                ..Default::default()
            },
        }
    }
}

pub struct CrossfeedPlugin {
    sample_rate: u32,
    params: CrossfeedPluginParams,

    // Filter storage
    bauer_hpf_l: Biquad,
    bauer_hpf_r: Biquad,
    
    meier_lpf_l: Biquad,
    meier_lpf_r: Biquad,
    meier_allpass_l: Biquad,
    meier_allpass_r: Biquad,

    mb_lp1_l: Biquad,
    mb_hp1_l: Biquad,
    mb_lp2_l: Biquad,
    mb_hp2_l: Biquad,
    mb_lp1_r: Biquad,
    mb_hp1_r: Biquad,
    mb_lp2_r: Biquad,
    mb_hp2_r: Biquad,

    // Pre-allocated flat buffers for deinterleaved processing
    dry_l: Vec<f32>,
    dry_r: Vec<f32>,
    wet_l: Vec<f32>,
    wet_r: Vec<f32>,
    
    // Multiband specific buffers
    mb_low_l: Vec<f32>,
    mb_low_r: Vec<f32>,
    mb_mid_l: Vec<f32>,
    mb_mid_r: Vec<f32>,
    mb_high_l: Vec<f32>,
    mb_high_r: Vec<f32>,

    // Auto gain helper
    auto_gain: Option<crate::auto_gain::AutoGain>,

    // Smoothing
    mix_smoother: Smoother,
}

impl CrossfeedPlugin {
    pub fn new(params: CrossfeedPluginParams) -> Result<Self, String> {
        let sr = 44100;
        let mut plugin = Self {
            sample_rate: sr,
            params: params.clone(),

            bauer_hpf_l: Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, params.bauer_fcut_hz as f64, sr as f64, 0.707, 0.0),
            bauer_hpf_r: Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, params.bauer_fcut_hz as f64, sr as f64, 0.707, 0.0),

            meier_lpf_l: Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, 650.0, sr as f64, 0.707, 0.0),
            meier_lpf_r: Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, 650.0, sr as f64, 0.707, 0.0),
            meier_allpass_l: Biquad::new(math_audio_iir_fir::BiquadFilterType::AllPass, 1000.0, sr as f64, 0.5, 0.0),
            meier_allpass_r: Biquad::new(math_audio_iir_fir::BiquadFilterType::AllPass, 1000.0, sr as f64, 0.5, 0.0),

            mb_lp1_l: Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, params.mb_low_freq_hz as f64, sr as f64, 0.707, 0.0),
            mb_hp1_l: Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, params.mb_low_freq_hz as f64, sr as f64, 0.707, 0.0),
            mb_lp2_l: Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, params.mb_mid_high_freq_hz as f64, sr as f64, 0.707, 0.0),
            mb_hp2_l: Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, params.mb_mid_high_freq_hz as f64, sr as f64, 0.707, 0.0),
            mb_lp1_r: Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, params.mb_low_freq_hz as f64, sr as f64, 0.707, 0.0),
            mb_hp1_r: Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, params.mb_low_freq_hz as f64, sr as f64, 0.707, 0.0),
            mb_lp2_r: Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, params.mb_mid_high_freq_hz as f64, sr as f64, 0.707, 0.0),
            mb_hp2_r: Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, params.mb_mid_high_freq_hz as f64, sr as f64, 0.707, 0.0),

            dry_l: vec![0.0; 4096],
            dry_r: vec![0.0; 4096],
            wet_l: vec![0.0; 4096],
            wet_r: vec![0.0; 4096],
            mb_low_l: vec![0.0; 4096],
            mb_low_r: vec![0.0; 4096],
            mb_mid_l: vec![0.0; 4096],
            mb_mid_r: vec![0.0; 4096],
            mb_high_l: vec![0.0; 4096],
            mb_high_r: vec![0.0; 4096],

            auto_gain: None,
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
        };

        if params.autogain_enabled {
            plugin.auto_gain = Some(crate::auto_gain::AutoGain::new(2, sr, crate::auto_gain::AutoGainParams {
                enabled: true,
                loudness_type: Default::default(),
                max_gain_db: params.autogain_max_gain_db,
                smoothing_ms: params.autogain_smoothing_ms,
            })?);
        }

        Ok(plugin)
    }

    fn update_filters(&mut self) {
        let sr = self.sample_rate as f64;
        self.bauer_hpf_l = Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, self.params.bauer_fcut_hz as f64, sr, 0.707, 0.0);
        self.bauer_hpf_r = Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, self.params.bauer_fcut_hz as f64, sr, 0.707, 0.0);
        
        self.mb_lp1_l = Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, self.params.mb_low_freq_hz as f64, sr, 0.707, 0.0);
        self.mb_hp1_l = Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, self.params.mb_low_freq_hz as f64, sr, 0.707, 0.0);
        self.mb_lp2_l = Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, self.params.mb_mid_high_freq_hz as f64, sr, 0.707, 0.0);
        self.mb_hp2_l = Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, self.params.mb_mid_high_freq_hz as f64, sr, 0.707, 0.0);
        self.mb_lp1_r = Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, self.params.mb_low_freq_hz as f64, sr, 0.707, 0.0);
        self.mb_hp1_r = Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, self.params.mb_low_freq_hz as f64, sr, 0.707, 0.0);
        self.mb_lp2_r = Biquad::new(math_audio_iir_fir::BiquadFilterType::Lowpass, self.params.mb_mid_high_freq_hz as f64, sr, 0.707, 0.0);
        self.mb_hp2_r = Biquad::new(math_audio_iir_fir::BiquadFilterType::Highpass, self.params.mb_mid_high_freq_hz as f64, sr, 0.707, 0.0);
    }

    #[inline(always)]
    fn process_bauer(&mut self, nf: usize) {
        let feed = fast_pow10(self.params.bauer_feed_db / 20.0);
        for i in 0..nf {
            let x_l = self.dry_l[i];
            let x_r = self.dry_r[i];
            let cross_r = self.bauer_hpf_r.process(x_r as f64) as f32;
            let cross_l = self.bauer_hpf_l.process(x_l as f64) as f32;
            self.wet_l[i] = x_l + feed * cross_r;
            self.wet_r[i] = x_r + feed * cross_l;
        }
    }

    #[inline(always)]
    fn process_meier(&mut self, nf: usize) {
        let feed = self.params.meier_level / 100.0;
        for i in 0..nf {
            let cross_r = self.meier_allpass_r.process(self.meier_lpf_r.process(self.dry_r[i] as f64)) as f32;
            let cross_l = self.meier_allpass_l.process(self.meier_lpf_l.process(self.dry_l[i] as f64)) as f32;
            self.wet_l[i] = self.dry_l[i] + feed * cross_r;
            self.wet_r[i] = self.dry_r[i] + feed * cross_l;
        }
    }

    #[inline(always)]
    fn process_mb(&mut self, nf: usize) {
        let fl = fast_pow10(self.params.mb_low_feed_db / 20.0);
        let fm = fast_pow10(self.params.mb_mid_feed_db / 20.0);
        let fh = fast_pow10(self.params.mb_high_feed_db / 20.0);

        for i in 0..nf {
            let xl = self.dry_l[i] as f64;
            let xr = self.dry_r[i] as f64;
            
            let low_l = self.mb_lp1_l.process(self.mb_lp2_l.process(xl)) as f32;
            let low_r = self.mb_lp1_r.process(self.mb_lp2_r.process(xr)) as f32;
            let mid_l = self.mb_hp1_l.process(self.mb_lp2_l.process(xl)) as f32;
            let mid_r = self.mb_hp1_r.process(self.mb_lp2_r.process(xr)) as f32;
            let high_l = self.mb_hp1_l.process(self.mb_hp2_l.process(xl)) as f32;
            let high_r = self.mb_hp1_r.process(self.mb_hp2_r.process(xr)) as f32;

            self.wet_l[i] = (low_l + fl * low_r) + (mid_l + fm * mid_r) + (high_l + fh * high_r);
            self.wet_r[i] = (low_r + fl * low_l) + (mid_r + fm * mid_l) + (high_r + fh * high_l);
        }
    }
}

impl InPlacePlugin for CrossfeedPlugin {
    fn info(&self) -> PluginInfo {
        let mode_str = match self.params.mode {
            CrossfeedMode::Off => "Off",
            CrossfeedMode::Bauer => "Bauer",
            CrossfeedMode::Meier => "Meier",
            CrossfeedMode::Mb => "Multiband",
        };
        PluginInfo::new("Crossfeed", "2.0.0", "SotF")
            .with_description(format!("Headphone crossfeed ({})", mode_str))
    }

    fn channels(&self) -> usize { 2 }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool("enabled", "Enabled", self.params.enabled).with_group("General"),
            Parameter::new_float("mix", "Mix", self.params.mix, 0.0, 1.0).with_group("General"),
            Parameter::new_float("bauer_fcut_hz", "Bauer Cutoff", self.params.bauer_fcut_hz, BAUER_FCUT_MIN, BAUER_FCUT_MAX).with_group("Bauer"),
            Parameter::new_float("bauer_feed_db", "Bauer Feed", self.params.bauer_feed_db, BAUER_FEED_MIN, BAUER_FEED_MAX).with_group("Bauer"),
            Parameter::new_float("meier_level", "Meier Level", self.params.meier_level, MEIER_LEVEL_MIN, MEIER_LEVEL_MAX).with_group("Meier"),
            Parameter::new_float("mb_low_freq_hz", "MB Low Freq", self.params.mb_low_freq_hz, MB_LOW_FREQ_MIN, MB_LOW_FREQ_MAX).with_group("Multiband"),
            Parameter::new_float("mb_mid_high_freq_hz", "MB High Freq", self.params.mb_mid_high_freq_hz, MB_MID_HIGH_FREQ_MIN, MB_MID_HIGH_FREQ_MAX).with_group("Multiband"),
            Parameter::new_float("mb_low_feed_db", "MB Low Feed", self.params.mb_low_feed_db, MB_LOW_FEED_MIN, MB_LOW_FEED_MAX).with_group("Multiband"),
            Parameter::new_float("mb_mid_feed_db", "MB Mid Feed", self.params.mb_mid_feed_db, MB_MID_FEED_MIN, MB_MID_FEED_MAX).with_group("Multiband"),
            Parameter::new_float("mb_high_feed_db", "MB High Feed", self.params.mb_high_feed_db, MB_HIGH_FEED_MIN, MB_HIGH_FEED_MAX).with_group("Multiband"),
            Parameter::new_bool("autogain_enabled", "Auto Gain", self.params.autogain_enabled).with_group("Auto Gain"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let name = id.0.as_str();
        match name {
            "enabled" => self.params.enabled = value.as_bool().unwrap_or(true),
            "mix" => { let v = value.as_float().unwrap_or(1.0); self.params.mix = v; self.mix_smoother.set_target(v); }
            "bauer_fcut_hz" => { self.params.bauer_fcut_hz = value.as_float().unwrap_or(700.0); self.update_filters(); }
            "bauer_feed_db" => { self.params.bauer_feed_db = value.as_float().unwrap_or(4.5); self.update_filters(); }
            "meier_level" => { self.params.meier_level = value.as_float().unwrap_or(15.0); }
            "mb_low_freq_hz" => { self.params.mb_low_freq_hz = value.as_float().unwrap_or(150.0); self.update_filters(); }
            "mb_mid_high_freq_hz" => { self.params.mb_mid_high_freq_hz = value.as_float().unwrap_or(5700.0); self.update_filters(); }
            "mb_low_feed_db" => { self.params.mb_low_feed_db = value.as_float().unwrap_or(0.0); }
            "mb_mid_feed_db" => { self.params.mb_mid_feed_db = value.as_float().unwrap_or(6.0); }
            "mb_high_feed_db" => { self.params.mb_high_feed_db = value.as_float().unwrap_or(3.0); }
            "autogain_enabled" => {
                let v = value.as_bool().unwrap_or(false);
                self.params.autogain_enabled = v;
                if v && self.auto_gain.is_none() {
                    self.auto_gain = crate::auto_gain::AutoGain::new(2, self.sample_rate, crate::auto_gain::AutoGainParams::default()).ok();
                }
            }
            _ => return Err(format!("Unknown: {}", name)),
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.0.as_str() {
            "enabled" => Some(ParameterValue::Bool(self.params.enabled)),
            "mix" => Some(ParameterValue::Float(self.params.mix)),
            "bauer_fcut_hz" => Some(ParameterValue::Float(self.params.bauer_fcut_hz)),
            "bauer_feed_db" => Some(ParameterValue::Float(self.params.bauer_feed_db)),
            "meier_level" => Some(ParameterValue::Float(self.params.meier_level)),
            "mb_low_freq_hz" => Some(ParameterValue::Float(self.params.mb_low_freq_hz)),
            "mb_mid_high_freq_hz" => Some(ParameterValue::Float(self.params.mb_mid_high_freq_hz)),
            "mb_low_feed_db" => Some(ParameterValue::Float(self.params.mb_low_feed_db)),
            "mb_mid_feed_db" => Some(ParameterValue::Float(self.params.mb_mid_feed_db)),
            "mb_high_feed_db" => Some(ParameterValue::Float(self.params.mb_high_feed_db)),
            "autogain_enabled" => Some(ParameterValue::Bool(self.params.autogain_enabled)),
            _ => None,
        }
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.update_filters();
        self.mix_smoother = Smoother::new(self.params.mix, 20.0, sr);
        if let Some(ag) = &mut self.auto_gain { ag.set_sample_rate(sr).ok(); }
        let cap = 4096;
        self.dry_l.resize(cap, 0.0); self.dry_r.resize(cap, 0.0);
        self.wet_l.resize(cap, 0.0); self.wet_r.resize(cap, 0.0);
        self.mb_low_l.resize(cap, 0.0); self.mb_low_r.resize(cap, 0.0);
        self.mb_mid_l.resize(cap, 0.0); self.mb_mid_r.resize(cap, 0.0);
        self.mb_high_l.resize(cap, 0.0); self.mb_high_r.resize(cap, 0.0);
        Ok(())
    }

    fn reset(&mut self) {
        self.mix_smoother.reset(self.params.mix);
        if let Some(ag) = &mut self.auto_gain { ag.reset(); }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        if !self.params.enabled || self.params.mode == CrossfeedMode::Off { return Ok(context.num_frames); }
        enable_ftz_daz();
        let nf = context.num_frames;
        if nf > self.dry_l.len() {
            self.dry_l.resize(nf, 0.0); self.dry_r.resize(nf, 0.0);
            self.wet_l.resize(nf, 0.0); self.wet_r.resize(nf, 0.0);
        }

        if let Some(ag) = &mut self.auto_gain { let _ = ag.measure_input(buffer); }

        deinterleave_stereo(buffer, &mut self.dry_l[..nf], &mut self.dry_r[..nf]);

        match self.params.mode {
            CrossfeedMode::Bauer => self.process_bauer(nf),
            CrossfeedMode::Meier => self.process_meier(nf),
            CrossfeedMode::Mb => self.process_mb(nf),
            _ => { self.wet_l[..nf].copy_from_slice(&self.dry_l[..nf]); self.wet_r[..nf].copy_from_slice(&self.dry_r[..nf]); }
        }

        let mix = self.mix_smoother.next_n(nf);
        for i in 0..nf {
            self.dry_l[i] = self.dry_l[i] * (1.0 - mix) + self.wet_l[i] * mix;
            self.dry_r[i] = self.dry_r[i] * (1.0 - mix) + self.wet_r[i] * mix;
        }

        interleave_stereo(&self.dry_l[..nf], &self.dry_r[..nf], buffer);

        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_output(buffer);
            ag.apply_compensation(buffer, nf);
        }

        Ok(nf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::ProcessContext;

    #[test]
    fn test_crossfeed_basic() {
        let mut p = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
        p.initialize(48000).unwrap();
        let mut b = vec![1.0, 0.0, 1.0, 0.0];
        p.process_in_place(&mut b, &ProcessContext { sample_rate: 48000, num_frames: 2 }).unwrap();
        assert!(b[1].abs() > 0.0);
    }

    #[test]
    fn test_bauer_basic() {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
        params.mode = CrossfeedMode::Bauer;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        let mut buffer = vec![1.0, 0.0, 0.0, 1.0];
        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 2,
            },
        )
        .unwrap();

        assert!(buffer[1].abs() > 0.0);
    }
}
