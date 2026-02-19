// ============================================================================
// Crossfeed Plugin - Headphone crossfeed for speaker-like listening
// ============================================================================

use crate::analyzer_loudness_monitor::LoudnessMonitor;
use crate::param_specs::crossfeed::*;
use crate::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use crate::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use crate::simd::{apply_gain_simd, deinterleave_stereo, enable_ftz_daz, interleave_stereo};
use crate::smoothing::{LinearSmoother, Smoother};

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

    // Filter storage (pre-allocated, no runtime alloc)
    // Bauer: 2 HPF filters (one per channel for crossfeed processing)
    bauer_hpf_l: Biquad,
    bauer_hpf_r: Biquad,
    bauer_feed: f32,

    // Meier: 2 LPF + 2 allpass for delay
    meier_lpf_l: Biquad,
    meier_lpf_r: Biquad,
    meier_allpass_l: Biquad,
    meier_allpass_r: Biquad,
    meier_feed: f32,

    // Multiband: 2 channels × 3 bands × 2 (LP/HP) = 12 filters
    mb_lp1_l: Biquad,
    mb_hp1_l: Biquad,
    mb_lp2_l: Biquad,
    mb_hp2_l: Biquad,
    mb_lp1_r: Biquad,
    mb_hp1_r: Biquad,
    mb_lp2_r: Biquad,
    mb_hp2_r: Biquad,
    mb_feed_low: f32,
    mb_feed_mid: f32,
    mb_feed_high: f32,

    // Temp buffers for processing (pre-allocated)
    temp_l: Vec<f32>,
    temp_r: Vec<f32>,
    temp_mb_low_l: Vec<f32>,
    temp_mb_low_r: Vec<f32>,
    temp_mb_mid_l: Vec<f32>,
    temp_mb_mid_r: Vec<f32>,
    temp_mb_high_l: Vec<f32>,
    temp_mb_high_r: Vec<f32>,

    // Auto gain
    autogain_input: LoudnessMonitor,
    autogain_output: LoudnessMonitor,
    autogain_gain_smoother: Smoother,
    autogain_current_gain_db: f32,

    // Smoothing
    mix_smoother: LinearSmoother,

    // Parameter IDs
    param_enabled: ParameterId,
    param_mode: ParameterId,
    param_mix: ParameterId,
    param_bauer_fcut: ParameterId,
    param_bauer_feed: ParameterId,
    param_meier_level: ParameterId,
    param_mb_low_freq: ParameterId,
    param_mb_mid_high_freq: ParameterId,
    param_mb_low_feed: ParameterId,
    param_mb_mid_feed: ParameterId,
    param_mb_high_feed: ParameterId,
    param_autogain_enabled: ParameterId,
    param_autogain_target: ParameterId,
    param_autogain_max_gain: ParameterId,
    param_autogain_smoothing: ParameterId,
}

impl CrossfeedPlugin {
    pub fn new(params: CrossfeedPluginParams) -> Result<Self, String> {
        let sr = 48000;
        let mut plugin = Self {
            sample_rate: sr,
            params: params.clone(),

            // Bauer filters (initialized in initialize)
            bauer_hpf_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Highpass,
                params.bauer_fcut_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            bauer_hpf_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Highpass,
                params.bauer_fcut_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            bauer_feed: fast_pow10(params.bauer_feed_db / 20.0),

            // Meier filters
            meier_lpf_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                650.0,
                sr as f64,
                0.707,
                0.0,
            ),
            meier_lpf_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                650.0,
                sr as f64,
                0.707,
                0.0,
            ),
            meier_allpass_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::AllPass,
                1000.0,
                sr as f64,
                0.5,
                0.0,
            ),
            meier_allpass_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::AllPass,
                1000.0,
                sr as f64,
                0.5,
                0.0,
            ),
            meier_feed: params.meier_level / 100.0,

            // Multiband filters
            mb_lp1_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                params.mb_low_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_hp1_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Highpass,
                params.mb_low_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_lp2_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                params.mb_mid_high_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_hp2_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Highpass,
                params.mb_mid_high_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_lp1_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                params.mb_low_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_hp1_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Highpass,
                params.mb_low_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_lp2_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                params.mb_mid_high_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_hp2_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Highpass,
                params.mb_mid_high_freq_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            mb_feed_low: fast_pow10(params.mb_low_feed_db / 20.0),
            mb_feed_mid: fast_pow10(params.mb_mid_feed_db / 20.0),
            mb_feed_high: fast_pow10(params.mb_high_feed_db / 20.0),

            // Temp buffers (allocated once)
            temp_l: Vec::with_capacity(4096),
            temp_r: Vec::with_capacity(4096),
            temp_mb_low_l: Vec::with_capacity(4096),
            temp_mb_low_r: Vec::with_capacity(4096),
            temp_mb_mid_l: Vec::with_capacity(4096),
            temp_mb_mid_r: Vec::with_capacity(4096),
            temp_mb_high_l: Vec::with_capacity(4096),
            temp_mb_high_r: Vec::with_capacity(4096),

            // Auto gain
            autogain_input: LoudnessMonitor::new(2, sr)?,
            autogain_output: LoudnessMonitor::new(2, sr)?,
            autogain_gain_smoother: Smoother::new(1.0, params.autogain_smoothing_ms, sr),
            autogain_current_gain_db: 0.0,

            // Smoothing
            mix_smoother: LinearSmoother::new(params.mix, 20.0, sr),

            // Parameter IDs
            param_enabled: ParameterId::from("enabled"),
            param_mode: ParameterId::from("mode"),
            param_mix: ParameterId::from("mix"),
            param_bauer_fcut: ParameterId::from("bauer_fcut_hz"),
            param_bauer_feed: ParameterId::from("bauer_feed_db"),
            param_meier_level: ParameterId::from("meier_level"),
            param_mb_low_freq: ParameterId::from("mb_low_freq_hz"),
            param_mb_mid_high_freq: ParameterId::from("mb_mid_high_freq_hz"),
            param_mb_low_feed: ParameterId::from("mb_low_feed_db"),
            param_mb_mid_feed: ParameterId::from("mb_mid_feed_db"),
            param_mb_high_feed: ParameterId::from("mb_high_feed_db"),
            param_autogain_enabled: ParameterId::from("autogain_enabled"),
            param_autogain_target: ParameterId::from("autogain_target_lufs"),
            param_autogain_max_gain: ParameterId::from("autogain_max_gain_db"),
            param_autogain_smoothing: ParameterId::from("autogain_smoothing_ms"),
        };

        Ok(plugin)
    }

    pub fn from_params(params: CrossfeedPluginParams) -> Result<Self, String> {
        Self::new(params)
    }

    #[inline(always)]
    fn process_bauer(&mut self, buffer: &mut [f32], num_frames: usize) {
        // Ensure temp buffers are large enough
        if self.temp_l.len() < num_frames {
            self.temp_l.resize(num_frames, 0.0);
            self.temp_r.resize(num_frames, 0.0);
        }

        // Deinterleave
        deinterleave_stereo(
            buffer,
            &mut self.temp_l[..num_frames],
            &mut self.temp_r[..num_frames],
        );

        // Process crossfeed: L += feed * HPF(R), R += feed * HPF(L)
        let feed = self.bauer_feed;

        // Apply HPF to opposite channel and mix
        for i in 0..num_frames {
            let x_l = self.temp_l[i];
            let x_r = self.temp_r[i];

            let cross_r = self.bauer_hpf_r.process(x_r as f64) as f32;
            let cross_l = self.bauer_hpf_l.process(x_l as f64) as f32;

            self.temp_l[i] = x_l + feed * cross_r;
            self.temp_r[i] = x_r + feed * cross_l;
        }
        // Interleave back
        interleave_stereo(
            &self.temp_l[..num_frames],
            &self.temp_r[..num_frames],
            buffer,
        );
    }

    #[inline(always)]
    fn process_meier(&mut self, buffer: &mut [f32], num_frames: usize) {
        if self.temp_l.len() < num_frames {
            self.temp_l.resize(num_frames, 0.0);
            self.temp_r.resize(num_frames, 0.0);
        }

        deinterleave_stereo(
            buffer,
            &mut self.temp_l[..num_frames],
            &mut self.temp_r[..num_frames],
        );

        let feed = self.meier_feed;

        // Meier: LPF + allpass delay on opposite channel
        for i in 0..num_frames {
            let cross_r = self
                .meier_allpass_r
                .process(self.meier_lpf_r.process(self.temp_r[i] as f64) as f64)
                as f32;
            let cross_l = self
                .meier_allpass_l
                .process(self.meier_lpf_l.process(self.temp_l[i] as f64) as f64)
                as f32;

            self.temp_l[i] = self.temp_l[i] + feed * cross_r;
            self.temp_r[i] = self.temp_r[i] + feed * cross_l;
        }

        interleave_stereo(
            &self.temp_l[..num_frames],
            &self.temp_r[..num_frames],
            buffer,
        );
    }

    #[inline(always)]
    fn process_mb(&mut self, buffer: &mut [f32], num_frames: usize) {
        let nf = num_frames;

        // Ensure temp buffers
        if self.temp_l.len() < nf {
            self.temp_l.resize(nf, 0.0);
            self.temp_r.resize(nf, 0.0);
            self.temp_mb_low_l.resize(nf, 0.0);
            self.temp_mb_low_r.resize(nf, 0.0);
            self.temp_mb_mid_l.resize(nf, 0.0);
            self.temp_mb_mid_r.resize(nf, 0.0);
            self.temp_mb_high_l.resize(nf, 0.0);
            self.temp_mb_high_r.resize(nf, 0.0);
        }

        deinterleave_stereo(buffer, &mut self.temp_l[..nf], &mut self.temp_r[..nf]);

        // Split into bands for left channel
        for i in 0..nf {
            let x_l = self.temp_l[i] as f64;
            let x_r = self.temp_r[i] as f64;

            // Low band
            self.temp_mb_low_l[i] = self.mb_lp1_l.process(self.mb_lp2_l.process(x_l)) as f32;
            self.temp_mb_low_r[i] = self.mb_lp1_r.process(self.mb_lp2_r.process(x_r)) as f32;

            // Mid band
            self.temp_mb_mid_l[i] = self.mb_hp1_l.process(self.mb_lp2_l.process(x_l)) as f32;
            self.temp_mb_mid_r[i] = self.mb_hp1_r.process(self.mb_lp2_r.process(x_r)) as f32;

            // High band
            self.temp_mb_high_l[i] = self.mb_hp1_l.process(self.mb_hp2_l.process(x_l)) as f32;
            self.temp_mb_high_r[i] = self.mb_hp1_r.process(self.mb_hp2_r.process(x_r)) as f32;
        }

        // Apply crossfeed per band
        let feed_low = self.mb_feed_low;
        let feed_mid = self.mb_feed_mid;
        let feed_high = self.mb_feed_high;

        for i in 0..nf {
            let low_l = self.temp_mb_low_l[i];
            let low_r = self.temp_mb_low_r[i];
            let mid_l = self.temp_mb_mid_l[i];
            let mid_r = self.temp_mb_mid_r[i];
            let high_l = self.temp_mb_high_l[i];
            let high_r = self.temp_mb_high_r[i];

            // Low band: less crossfeed
            self.temp_mb_low_l[i] = low_l + feed_low * low_r;
            self.temp_mb_low_r[i] = low_r + feed_low * low_l;

            // Mid band
            self.temp_mb_mid_l[i] = mid_l + feed_mid * mid_r;
            self.temp_mb_mid_r[i] = mid_r + feed_mid * mid_l;

            // High band
            self.temp_mb_high_l[i] = high_l + feed_high * high_r;
            self.temp_mb_high_r[i] = high_r + feed_high * high_l;
        }

        // Merge bands back
        for i in 0..nf {
            self.temp_l[i] = self.temp_mb_low_l[i] + self.temp_mb_mid_l[i] + self.temp_mb_high_l[i];
            self.temp_r[i] = self.temp_mb_low_r[i] + self.temp_mb_mid_r[i] + self.temp_mb_high_r[i];
        }

        interleave_stereo(&self.temp_l[..nf], &self.temp_r[..nf], buffer);
    }

    fn update_filters(&mut self) {
        let sr = self.sample_rate as f64;

        // Update Bauer filters
        self.bauer_hpf_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Highpass,
            self.params.bauer_fcut_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.bauer_hpf_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Highpass,
            self.params.bauer_fcut_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.bauer_feed = fast_pow10(self.params.bauer_feed_db / 20.0);

        // Update Meier filters
        self.meier_feed = self.params.meier_level / 100.0;

        // Update Multiband filters
        self.mb_lp1_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            self.params.mb_low_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.mb_hp1_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Highpass,
            self.params.mb_low_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.mb_lp2_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            self.params.mb_mid_high_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.mb_hp2_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Highpass,
            self.params.mb_mid_high_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );

        self.mb_lp1_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            self.params.mb_low_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.mb_hp1_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Highpass,
            self.params.mb_low_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.mb_lp2_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            self.params.mb_mid_high_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.mb_hp2_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Highpass,
            self.params.mb_mid_high_freq_hz as f64,
            sr,
            0.707,
            0.0,
        );

        self.mb_feed_low = fast_pow10(self.params.mb_low_feed_db / 20.0);
        self.mb_feed_mid = fast_pow10(self.params.mb_mid_feed_db / 20.0);
        self.mb_feed_high = fast_pow10(self.params.mb_high_feed_db / 20.0);
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
        PluginInfo::new("Crossfeed", "1.0.0", "SotF")
            .with_description(format!("Headphone crossfeed ({})", mode_str))
    }

    fn channels(&self) -> usize {
        2
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_bool("enabled", "Enabled", self.params.enabled)
                .with_group("General")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float("mix", "Mix", self.params.mix, MIX_MIN, MIX_MAX)
                .with_group("General")
                .with_importance(ParameterImportance::Useful),
            // Bauer
            Parameter::new_float(
                "bauer_fcut_hz",
                "Bauer Cutoff (Hz)",
                self.params.bauer_fcut_hz,
                BAUER_FCUT_MIN,
                BAUER_FCUT_MAX,
            )
            .with_group("Bauer")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "bauer_feed_db",
                "Bauer Feed (dB)",
                self.params.bauer_feed_db,
                BAUER_FEED_MIN,
                BAUER_FEED_MAX,
            )
            .with_group("Bauer")
            .with_importance(ParameterImportance::Useful),
            // Meier
            Parameter::new_float(
                "meier_level",
                "Meier Level (%)",
                self.params.meier_level,
                MEIER_LEVEL_MIN,
                MEIER_LEVEL_MAX,
            )
            .with_group("Meier")
            .with_importance(ParameterImportance::Useful),
            // Multiband
            Parameter::new_float(
                "mb_low_freq_hz",
                "MB Low Freq (Hz)",
                self.params.mb_low_freq_hz,
                MB_LOW_FREQ_MIN,
                MB_LOW_FREQ_MAX,
            )
            .with_group("Multiband")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mb_mid_high_freq_hz",
                "MB Mid/High Freq (Hz)",
                self.params.mb_mid_high_freq_hz,
                MB_MID_HIGH_FREQ_MIN,
                MB_MID_HIGH_FREQ_MAX,
            )
            .with_group("Multiband")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mb_low_feed_db",
                "MB Low Feed (dB)",
                self.params.mb_low_feed_db,
                MB_LOW_FEED_MIN,
                MB_LOW_FEED_MAX,
            )
            .with_group("Multiband")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mb_mid_feed_db",
                "MB Mid Feed (dB)",
                self.params.mb_mid_feed_db,
                MB_MID_FEED_MIN,
                MB_MID_FEED_MAX,
            )
            .with_group("Multiband")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mb_high_feed_db",
                "MB High Feed (dB)",
                self.params.mb_high_feed_db,
                MB_HIGH_FEED_MIN,
                MB_HIGH_FEED_MAX,
            )
            .with_group("Multiband")
            .with_importance(ParameterImportance::Useful),
            // Auto gain
            Parameter::new_bool(
                "autogain_enabled",
                "Auto Gain",
                self.params.autogain_enabled,
            )
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "autogain_target_lufs",
                "Target LUFS",
                self.params.autogain_target_lufs,
                AUTOGAIN_TARGET_MIN,
                AUTOGAIN_TARGET_MAX,
            )
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "autogain_max_gain_db",
                "Max Gain (dB)",
                self.params.autogain_max_gain_db,
                AUTOGAIN_MAX_GAIN_MIN,
                AUTOGAIN_MAX_GAIN_MAX,
            )
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "autogain_smoothing_ms",
                "Smoothing (ms)",
                self.params.autogain_smoothing_ms,
                AUTOGAIN_SMOOTHING_MIN,
                AUTOGAIN_SMOOTHING_MAX,
            )
            .with_group("Auto Gain")
            .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let mut needs_filter_update = false;

        if id == self.param_enabled {
            if let ParameterValue::Bool(v) = value {
                self.params.enabled = v;
            }
        } else if id == self.param_mix {
            if let Some(v) = value.as_float() {
                self.params.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.params.mix);
            }
        } else if id == self.param_bauer_fcut {
            if let Some(v) = value.as_float() {
                self.params.bauer_fcut_hz = v.clamp(BAUER_FCUT_MIN, BAUER_FCUT_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_bauer_feed {
            if let Some(v) = value.as_float() {
                self.params.bauer_feed_db = v.clamp(BAUER_FEED_MIN, BAUER_FEED_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_meier_level {
            if let Some(v) = value.as_float() {
                self.params.meier_level = v.clamp(MEIER_LEVEL_MIN, MEIER_LEVEL_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_mb_low_freq {
            if let Some(v) = value.as_float() {
                self.params.mb_low_freq_hz = v.clamp(MB_LOW_FREQ_MIN, MB_LOW_FREQ_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_mb_mid_high_freq {
            if let Some(v) = value.as_float() {
                self.params.mb_mid_high_freq_hz =
                    v.clamp(MB_MID_HIGH_FREQ_MIN, MB_MID_HIGH_FREQ_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_mb_low_feed {
            if let Some(v) = value.as_float() {
                self.params.mb_low_feed_db = v.clamp(MB_LOW_FEED_MIN, MB_LOW_FEED_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_mb_mid_feed {
            if let Some(v) = value.as_float() {
                self.params.mb_mid_feed_db = v.clamp(MB_MID_FEED_MIN, MB_MID_FEED_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_mb_high_feed {
            if let Some(v) = value.as_float() {
                self.params.mb_high_feed_db = v.clamp(MB_HIGH_FEED_MIN, MB_HIGH_FEED_MAX);
                needs_filter_update = true;
            }
        } else if id == self.param_autogain_enabled {
            if let ParameterValue::Bool(v) = value {
                self.params.autogain_enabled = v;
            }
        } else if id == self.param_autogain_target {
            if let Some(v) = value.as_float() {
                self.params.autogain_target_lufs =
                    v.clamp(AUTOGAIN_TARGET_MIN, AUTOGAIN_TARGET_MAX);
            }
        } else if id == self.param_autogain_max_gain {
            if let Some(v) = value.as_float() {
                self.params.autogain_max_gain_db =
                    v.clamp(AUTOGAIN_MAX_GAIN_MIN, AUTOGAIN_MAX_GAIN_MAX);
            }
        } else if id == self.param_autogain_smoothing {
            if let Some(v) = value.as_float() {
                self.params.autogain_smoothing_ms =
                    v.clamp(AUTOGAIN_SMOOTHING_MIN, AUTOGAIN_SMOOTHING_MAX);
                self.autogain_gain_smoother
                    .set_time(self.params.autogain_smoothing_ms, self.sample_rate);
            }
        }

        if needs_filter_update {
            self.update_filters();
        }

        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_enabled {
            Some(ParameterValue::Bool(self.params.enabled))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.params.mix))
        } else if id == &self.param_bauer_fcut {
            Some(ParameterValue::Float(self.params.bauer_fcut_hz))
        } else if id == &self.param_bauer_feed {
            Some(ParameterValue::Float(self.params.bauer_feed_db))
        } else if id == &self.param_meier_level {
            Some(ParameterValue::Float(self.params.meier_level))
        } else if id == &self.param_mb_low_freq {
            Some(ParameterValue::Float(self.params.mb_low_freq_hz))
        } else if id == &self.param_mb_mid_high_freq {
            Some(ParameterValue::Float(self.params.mb_mid_high_freq_hz))
        } else if id == &self.param_mb_low_feed {
            Some(ParameterValue::Float(self.params.mb_low_feed_db))
        } else if id == &self.param_mb_mid_feed {
            Some(ParameterValue::Float(self.params.mb_mid_feed_db))
        } else if id == &self.param_mb_high_feed {
            Some(ParameterValue::Float(self.params.mb_high_feed_db))
        } else if id == &self.param_autogain_enabled {
            Some(ParameterValue::Bool(self.params.autogain_enabled))
        } else if id == &self.param_autogain_target {
            Some(ParameterValue::Float(self.params.autogain_target_lufs))
        } else if id == &self.param_autogain_max_gain {
            Some(ParameterValue::Float(self.params.autogain_max_gain_db))
        } else if id == &self.param_autogain_smoothing {
            Some(ParameterValue::Float(self.params.autogain_smoothing_ms))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_filters();
        self.mix_smoother = LinearSmoother::new(self.params.mix, 20.0, sample_rate);

        // Re-init auto gain with new sample rate
        self.autogain_input = LoudnessMonitor::new(2, sample_rate)?;
        self.autogain_output = LoudnessMonitor::new(2, sample_rate)?;
        self.autogain_gain_smoother =
            Smoother::new(1.0, self.params.autogain_smoothing_ms, sample_rate);

        // Pre-allocate temp buffers
        self.temp_l.resize(4096, 0.0);
        self.temp_r.resize(4096, 0.0);
        self.temp_mb_low_l.resize(4096, 0.0);
        self.temp_mb_low_r.resize(4096, 0.0);
        self.temp_mb_mid_l.resize(4096, 0.0);
        self.temp_mb_mid_r.resize(4096, 0.0);
        self.temp_mb_high_l.resize(4096, 0.0);
        self.temp_mb_high_r.resize(4096, 0.0);

        Ok(())
    }

    fn reset(&mut self) {
        self.update_filters();
        self.mix_smoother.reset(self.params.mix);
        self.autogain_gain_smoother.reset(1.0);
        self.autogain_current_gain_db = 0.0;
        let _ = self.autogain_input.reset();
        let _ = self.autogain_output.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();

        let num_frames = context.num_frames;

        // Bypass if disabled or mode is off
        if !self.params.enabled || self.params.mode == CrossfeedMode::Off {
            return Ok(num_frames);
        }

        // Measure input loudness for autogain
        if self.params.autogain_enabled {
            self.autogain_input.add_frames(buffer).ok();
        }

        // Process based on mode
        match self.params.mode {
            CrossfeedMode::Bauer => self.process_bauer(buffer, num_frames),
            CrossfeedMode::Meier => self.process_meier(buffer, num_frames),
            CrossfeedMode::Mb => self.process_mb(buffer, num_frames),
            CrossfeedMode::Off => {}
        }

        // Apply autogain
        if self.params.autogain_enabled {
            // Measure output loudness
            self.autogain_output.add_frames(buffer).ok();

            // Compute gain
            let input_lufs = self.autogain_input.get_loudness().momentary_lufs as f32;
            let output_lufs = self.autogain_output.get_loudness().momentary_lufs as f32;

            if input_lufs.is_finite() && output_lufs.is_finite() {
                let target = input_lufs - output_lufs + self.params.autogain_target_lufs;
                let clamped = target.clamp(
                    -self.params.autogain_max_gain_db,
                    self.params.autogain_max_gain_db,
                );
                self.autogain_gain_smoother
                    .set_target(fast_pow10(clamped / 20.0));
            }

            let gain = self.autogain_gain_smoother.next();
            apply_gain_simd(buffer, gain);
        }

        // Apply mix (dry/wet)
        // Note: For simplicity, we apply mix as output gain when < 1.0
        // A proper implementation would keep dry signal separately
        let mix = self.mix_smoother.next();
        if (mix - 1.0).abs() > 0.001 {
            apply_gain_simd(buffer, mix);
        }

        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::ProcessContext;

    #[test]
    fn test_bypass_off() {
        let mut params = CrossfeedPluginParams {
            mode: CrossfeedMode::Off,
            enabled: true,
            ..Default::default()
        };
        let mut p = CrossfeedPlugin::new(params).unwrap();

        let mut buffer = vec![1.0, 2.0, 3.0, 4.0];
        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 2,
            },
        )
        .unwrap();

        assert!((buffer[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_bauer_basic() {
        let mut params = CrossfeedPluginParams {
            mode: CrossfeedMode::Bauer,
            bauer_fcut_hz: 700.0,
            bauer_feed_db: 6.0,
            enabled: true,
            ..Default::default()
        };
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

        // Should have crossfeed now
        assert!(buffer[1].abs() > 0.0);
    }

    #[test]
    fn test_meier() {
        let mut params = CrossfeedPluginParams {
            mode: CrossfeedMode::Meier,
            meier_level: 30.0,
            enabled: true,
            ..Default::default()
        };
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

    #[test]
    fn test_multiband() {
        let mut params = CrossfeedPluginParams {
            mode: CrossfeedMode::Mb,
            mb_low_freq_hz: 150.0,
            mb_mid_high_freq_hz: 5700.0,
            mb_low_feed_db: 0.0,
            mb_mid_feed_db: 6.0,
            mb_high_feed_db: 3.0,
            enabled: true,
            ..Default::default()
        };
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
