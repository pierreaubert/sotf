// ============================================================================
// Crossfeed Plugin - Headphone crossfeed for speaker-like listening
// ============================================================================

pub mod params;

use sotf_host::lr4_crossover::MultibandLr4Crossover;
use sotf_host::param_specs::find_by_key as pk;

use crate::params::PARAMS as CF;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{deinterleave_stereo, enable_ftz_daz, interleave_stereo};
use sotf_host::smoothing::Smoother;

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

    // ITD delay
    #[serde(default)]
    pub itd_delay_ms: f32,

    /// Head yaw angle in degrees (-90 to +90, 0 = centered).
    /// Dynamically adjusts ITD based on head rotation.
    /// ITD = head_radius * sin(yaw) / speed_of_sound * 1000 ms.
    #[serde(default)]
    pub head_yaw_deg: f32,

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
    pk(CF, "mix").default_f64() as f32
}

fn default_bauer_fcut() -> f32 {
    pk(CF, "bauer_fcut_hz").default_f64() as f32
}

fn default_bauer_feed() -> f32 {
    pk(CF, "bauer_feed_db").default_f64() as f32
}

fn default_meier_level() -> f32 {
    pk(CF, "meier_level").default_f64() as f32
}

fn default_mb_low_freq() -> f32 {
    pk(CF, "mb_low_freq_hz").default_f64() as f32
}

fn default_mb_mid_high_freq() -> f32 {
    pk(CF, "mb_mid_high_freq_hz").default_f64() as f32
}

fn default_mb_low_feed() -> f32 {
    pk(CF, "mb_low_feed_db").default_f64() as f32
}

fn default_mb_mid_feed() -> f32 {
    pk(CF, "mb_mid_feed_db").default_f64() as f32
}

fn default_mb_high_feed() -> f32 {
    pk(CF, "mb_high_feed_db").default_f64() as f32
}

fn default_autogain_target() -> f32 {
    pk(CF, "autogain_target_lufs").default_f64() as f32
}

fn default_autogain_max_gain() -> f32 {
    pk(CF, "autogain_max_gain_db").default_f64() as f32
}

fn default_autogain_smoothing() -> f32 {
    pk(CF, "autogain_smoothing_ms").default_f64() as f32
}

impl Default for CrossfeedPluginParams {
    fn default() -> Self {
        Self {
            mode: CrossfeedMode::Bauer,
            preset: CrossfeedPreset::Default,
            enabled: true,
            mix: 1.0,
            bauer_fcut_hz: pk(CF, "bauer_fcut_hz").default_f64() as f32,
            bauer_feed_db: pk(CF, "bauer_feed_db").default_f64() as f32,
            meier_level: pk(CF, "meier_level").default_f64() as f32,
            mb_low_freq_hz: pk(CF, "mb_low_freq_hz").default_f64() as f32,
            mb_mid_high_freq_hz: pk(CF, "mb_mid_high_freq_hz").default_f64() as f32,
            mb_low_feed_db: pk(CF, "mb_low_feed_db").default_f64() as f32,
            mb_mid_feed_db: pk(CF, "mb_mid_feed_db").default_f64() as f32,
            mb_high_feed_db: pk(CF, "mb_high_feed_db").default_f64() as f32,
            itd_delay_ms: 0.0,
            head_yaw_deg: 0.0,
            autogain_enabled: pk(CF, "autogain_enabled").default_bool(),
            autogain_target_lufs: pk(CF, "autogain_target_lufs").default_f64() as f32,
            autogain_max_gain_db: pk(CF, "autogain_max_gain_db").default_f64() as f32,
            autogain_smoothing_ms: pk(CF, "autogain_smoothing_ms").default_f64() as f32,
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

// ============================================================================
// ITD Delay Line — simple fractional-sample delay for interaural time difference
// ============================================================================

/// A mono delay line supporting up to ~1ms of delay at any common sample rate.
/// Max capacity: 48 samples (1ms at 48kHz).
struct DelayLine {
    buffer: [f32; 96], // 2ms at 48kHz — headroom for high sample rates
    write_pos: usize,
    delay_samples: usize,
    capacity: usize,
}

impl DelayLine {
    fn new(delay_ms: f32, sample_rate: u32) -> Self {
        let capacity = 96;
        let delay_samples = ((delay_ms / 1000.0) * sample_rate as f32)
            .round()
            .max(0.0)
            .min(capacity as f32 - 1.0) as usize;
        Self {
            buffer: [0.0; 96],
            write_pos: 0,
            delay_samples,
            capacity,
        }
    }

    fn set_delay(&mut self, delay_ms: f32, sample_rate: u32) {
        self.delay_samples = ((delay_ms / 1000.0) * sample_rate as f32)
            .round()
            .max(0.0)
            .min(self.capacity as f32 - 1.0) as usize;
    }

    fn reset(&mut self) {
        self.buffer = [0.0; 96];
        self.write_pos = 0;
    }

    #[inline]
    fn process(&mut self, sample: f32) -> f32 {
        if self.delay_samples == 0 {
            return sample;
        }
        self.buffer[self.write_pos] = sample;
        let read_pos =
            (self.write_pos + self.capacity - self.delay_samples) % self.capacity;
        let out = self.buffer[read_pos];
        self.write_pos = (self.write_pos + 1) % self.capacity;
        out
    }
}

pub struct CrossfeedPlugin {
    sample_rate: u32,
    params: CrossfeedPluginParams,

    // Bauer: LPF filters (crossfeed low frequencies per bs2b spec)
    bauer_lpf_l: Biquad,
    bauer_lpf_r: Biquad,

    meier_lpf_l: Biquad,
    meier_lpf_r: Biquad,
    meier_allpass_l: Biquad,
    meier_allpass_r: Biquad,

    // Multiband: true LR4 crossover (3-band: low/mid/high)
    mb_crossover_l: MultibandLr4Crossover,
    mb_crossover_r: MultibandLr4Crossover,

    // ITD delay lines (one per crossfeed path)
    itd_delay_l: DelayLine,
    itd_delay_r: DelayLine,

    // Pre-allocated flat buffers for deinterleaved processing
    dry_l: Vec<f32>,
    dry_r: Vec<f32>,
    wet_l: Vec<f32>,
    wet_r: Vec<f32>,

    // Multiband specific buffers (3 bands per channel)
    mb_bands_l: [Vec<f32>; 3],
    mb_bands_r: [Vec<f32>; 3],

    // Auto gain helper
    auto_gain: Option<sotf_host::auto_gain::AutoGain>,

    // Smoothing
    mix_smoother: Smoother,
    yaw_smoother: Smoother,
    cached_parameters: Vec<Parameter>,
}

/// Head radius in meters (typical adult)
const HEAD_RADIUS_M: f32 = 0.0875;
/// Speed of sound in m/s
const SPEED_OF_SOUND: f32 = 343.0;

/// Compute ITD in ms from yaw angle (degrees) and static offset.
/// Positive yaw = turned right = right ear closer to source = shorter right path.
fn compute_dynamic_itd_ms(head_yaw_deg: f32, static_itd_ms: f32) -> f32 {
    let yaw_rad = head_yaw_deg * std::f32::consts::PI / 180.0;
    let dynamic_itd_ms = HEAD_RADIUS_M * yaw_rad.sin() / SPEED_OF_SOUND * 1000.0;
    (static_itd_ms + dynamic_itd_ms).clamp(0.0, 1.0)
}

impl CrossfeedPlugin {
    pub fn new(params: CrossfeedPluginParams) -> Result<Self, String> {
        let sr = 44100;
        let mut plugin = Self {
            sample_rate: sr,
            params: params.clone(),

            // Bauer: LPF (lowpass) per bs2b specification
            bauer_lpf_l: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                params.bauer_fcut_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),
            bauer_lpf_r: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowpass,
                params.bauer_fcut_hz as f64,
                sr as f64,
                0.707,
                0.0,
            ),

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

            // Multiband: true LR4 crossover with 2 crossover points → 3 bands
            mb_crossover_l: MultibandLr4Crossover::new(
                &[params.mb_low_freq_hz, params.mb_mid_high_freq_hz],
                sr,
                1,
            ),
            mb_crossover_r: MultibandLr4Crossover::new(
                &[params.mb_low_freq_hz, params.mb_mid_high_freq_hz],
                sr,
                1,
            ),

            // ITD delay lines
            itd_delay_l: DelayLine::new(params.itd_delay_ms, sr),
            itd_delay_r: DelayLine::new(params.itd_delay_ms, sr),

            dry_l: vec![0.0; 4096],
            dry_r: vec![0.0; 4096],
            wet_l: vec![0.0; 4096],
            wet_r: vec![0.0; 4096],
            mb_bands_l: [vec![0.0; 4096], vec![0.0; 4096], vec![0.0; 4096]],
            mb_bands_r: [vec![0.0; 4096], vec![0.0; 4096], vec![0.0; 4096]],

            auto_gain: None,
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            yaw_smoother: Smoother::new(params.head_yaw_deg, 10.0, sr),
            cached_parameters: Vec::new(),
        };

        if params.autogain_enabled {
            plugin.auto_gain = Some(sotf_host::auto_gain::AutoGain::new(
                2,
                sr,
                sotf_host::auto_gain::AutoGainParams {
                    enabled: true,
                    loudness_type: Default::default(),
                    max_gain_db: params.autogain_max_gain_db,
                    smoothing_ms: params.autogain_smoothing_ms,
                },
            )?);
        }
        plugin.rebuild_cached_parameters();

        Ok(plugin)
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_bool("enabled", "Enabled", self.params.enabled).with_group("General"),
            Parameter::new_float(
                "mix",
                "Mix",
                self.params.mix,
                pk(CF, "mix").min_f64() as f32,
                pk(CF, "mix").max_f64() as f32,
            )
            .with_group("General"),
            Parameter::new_float(
                "bauer_fcut_hz",
                "Bauer Cutoff",
                self.params.bauer_fcut_hz,
                pk(CF, "bauer_fcut_hz").min_f64() as f32,
                pk(CF, "bauer_fcut_hz").max_f64() as f32,
            )
            .with_group("Bauer"),
            Parameter::new_float(
                "bauer_feed_db",
                "Bauer Feed",
                self.params.bauer_feed_db,
                pk(CF, "bauer_feed_db").min_f64() as f32,
                pk(CF, "bauer_feed_db").max_f64() as f32,
            )
            .with_group("Bauer"),
            Parameter::new_float(
                "meier_level",
                "Meier Level",
                self.params.meier_level,
                pk(CF, "meier_level").min_f64() as f32,
                pk(CF, "meier_level").max_f64() as f32,
            )
            .with_group("Meier"),
            Parameter::new_float(
                "mb_low_freq_hz",
                "MB Low Freq",
                self.params.mb_low_freq_hz,
                pk(CF, "mb_low_freq_hz").min_f64() as f32,
                pk(CF, "mb_low_freq_hz").max_f64() as f32,
            )
            .with_group("Multiband"),
            Parameter::new_float(
                "mb_mid_high_freq_hz",
                "MB High Freq",
                self.params.mb_mid_high_freq_hz,
                pk(CF, "mb_mid_high_freq_hz").min_f64() as f32,
                pk(CF, "mb_mid_high_freq_hz").max_f64() as f32,
            )
            .with_group("Multiband"),
            Parameter::new_float(
                "mb_low_feed_db",
                "MB Low Feed",
                self.params.mb_low_feed_db,
                pk(CF, "mb_low_feed_db").min_f64() as f32,
                pk(CF, "mb_low_feed_db").max_f64() as f32,
            )
            .with_group("Multiband"),
            Parameter::new_float(
                "mb_mid_feed_db",
                "MB Mid Feed",
                self.params.mb_mid_feed_db,
                pk(CF, "mb_mid_feed_db").min_f64() as f32,
                pk(CF, "mb_mid_feed_db").max_f64() as f32,
            )
            .with_group("Multiband"),
            Parameter::new_float(
                "mb_high_feed_db",
                "MB High Feed",
                self.params.mb_high_feed_db,
                pk(CF, "mb_high_feed_db").min_f64() as f32,
                pk(CF, "mb_high_feed_db").max_f64() as f32,
            )
            .with_group("Multiband"),
            Parameter::new_float(
                "itd_delay_ms",
                "ITD Delay",
                self.params.itd_delay_ms,
                0.0,
                1.0,
            )
            .with_group("General"),
            Parameter::new_float(
                "head_yaw_deg",
                "Head Yaw",
                self.params.head_yaw_deg,
                -90.0,
                90.0,
            )
            .with_description("Head rotation in degrees, dynamically adjusts ITD")
            .with_group("Head Tracking"),
            Parameter::new_bool(
                "autogain_enabled",
                "Auto Gain",
                self.params.autogain_enabled,
            )
            .with_group("Auto Gain"),
            Parameter::new_float(
                "autogain_target_lufs",
                "Target LUFS",
                self.params.autogain_target_lufs,
                pk(CF, "autogain_target_lufs").min_f64() as f32,
                pk(CF, "autogain_target_lufs").max_f64() as f32,
            )
            .with_group("Auto Gain"),
            Parameter::new_float(
                "autogain_max_gain_db",
                "Max Gain",
                self.params.autogain_max_gain_db,
                pk(CF, "autogain_max_gain_db").min_f64() as f32,
                pk(CF, "autogain_max_gain_db").max_f64() as f32,
            )
            .with_group("Auto Gain"),
            Parameter::new_float(
                "autogain_smoothing_ms",
                "Smoothing",
                self.params.autogain_smoothing_ms,
                pk(CF, "autogain_smoothing_ms").min_f64() as f32,
                pk(CF, "autogain_smoothing_ms").max_f64() as f32,
            )
            .with_group("Auto Gain"),
        ];
    }

    fn update_filters(&mut self) {
        let sr = self.sample_rate as f64;

        // Bauer: LPF (lowpass) per bs2b specification
        self.bauer_lpf_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            self.params.bauer_fcut_hz as f64,
            sr,
            0.707,
            0.0,
        );
        self.bauer_lpf_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            self.params.bauer_fcut_hz as f64,
            sr,
            0.707,
            0.0,
        );

        // Multiband: true LR4 crossover
        self.mb_crossover_l.reinit(
            &[self.params.mb_low_freq_hz, self.params.mb_mid_high_freq_hz],
            self.sample_rate,
            1,
        );
        self.mb_crossover_r.reinit(
            &[self.params.mb_low_freq_hz, self.params.mb_mid_high_freq_hz],
            self.sample_rate,
            1,
        );
    }

    #[inline(always)]
    fn process_bauer(&mut self, nf: usize) {
        let feed = fast_pow10(self.params.bauer_feed_db / 20.0);
        let has_itd = self.params.itd_delay_ms > 0.0;
        for i in 0..nf {
            let x_l = self.dry_l[i];
            let x_r = self.dry_r[i];
            // LPF crossfeed: extract low frequencies from opposite channel (bs2b spec)
            let mut cross_r = self.bauer_lpf_r.process(x_r as f64) as f32;
            let mut cross_l = self.bauer_lpf_l.process(x_l as f64) as f32;
            // Apply ITD delay to the crossfeed path
            if has_itd {
                cross_r = self.itd_delay_r.process(cross_r);
                cross_l = self.itd_delay_l.process(cross_l);
            }
            self.wet_l[i] = x_l + feed * cross_r;
            self.wet_r[i] = x_r + feed * cross_l;
        }
    }

    #[inline(always)]
    fn process_meier(&mut self, nf: usize) {
        let feed = self.params.meier_level / 100.0;
        let has_itd = self.params.itd_delay_ms > 0.0;
        for i in 0..nf {
            let mut cross_r =
                self.meier_allpass_r
                    .process(self.meier_lpf_r.process(self.dry_r[i] as f64)) as f32;
            let mut cross_l =
                self.meier_allpass_l
                    .process(self.meier_lpf_l.process(self.dry_l[i] as f64)) as f32;
            if has_itd {
                cross_r = self.itd_delay_r.process(cross_r);
                cross_l = self.itd_delay_l.process(cross_l);
            }
            self.wet_l[i] = self.dry_l[i] + feed * cross_r;
            self.wet_r[i] = self.dry_r[i] + feed * cross_l;
        }
    }

    #[inline(always)]
    fn process_mb(&mut self, nf: usize) {
        let fl = fast_pow10(self.params.mb_low_feed_db / 20.0);
        let fm = fast_pow10(self.params.mb_mid_feed_db / 20.0);
        let fh = fast_pow10(self.params.mb_high_feed_db / 20.0);
        let has_itd = self.params.itd_delay_ms > 0.0;

        for i in 0..nf {
            let xl = self.dry_l[i];
            let xr = self.dry_r[i];

            // Split each channel into 3 bands using true LR4 crossover
            let input_l = [xl];
            let input_r = [xr];

            let mut band0_l = [0.0f32];
            let mut band1_l = [0.0f32];
            let mut band2_l = [0.0f32];
            let mut band0_r = [0.0f32];
            let mut band1_r = [0.0f32];
            let mut band2_r = [0.0f32];

            self.mb_crossover_l.process_frame(
                &input_l,
                &mut [&mut band0_l[..], &mut band1_l[..], &mut band2_l[..]],
            );
            self.mb_crossover_r.process_frame(
                &input_r,
                &mut [&mut band0_r[..], &mut band1_r[..], &mut band2_r[..]],
            );

            let low_l = band0_l[0];
            let mid_l = band1_l[0];
            let high_l = band2_l[0];
            let low_r = band0_r[0];
            let mid_r = band1_r[0];
            let high_r = band2_r[0];

            // Compute crossfeed signal per band
            let mut cross_l = fl * low_l + fm * mid_l + fh * high_l;
            let mut cross_r = fl * low_r + fm * mid_r + fh * high_r;

            // Apply ITD delay to the crossfeed path
            if has_itd {
                cross_l = self.itd_delay_l.process(cross_l);
                cross_r = self.itd_delay_r.process(cross_r);
            }

            // Mix crossfeed from opposite channel
            self.wet_l[i] = (low_l + mid_l + high_l) + cross_r;
            self.wet_r[i] = (low_r + mid_r + high_r) + cross_l;
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
        PluginInfo::new("Crossfeed", "3.0.0", "SotF")
            .with_description(format!("Headphone crossfeed ({})", mode_str))
    }

    fn channels(&self) -> usize {
        2
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        let name = id.0.as_str();
        match name {
            "enabled" => {
                self.params.enabled = value
                    .as_bool()
                    .ok_or_else(|| "enabled must be a boolean".to_string())?
            }
            "mix" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mix must be a float".to_string())?;
                if v.is_finite() {
                    self.params.mix = v;
                    self.mix_smoother.set_target(v);
                }
            }
            "bauer_fcut_hz" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "bauer_fcut_hz must be a float".to_string())?;
                if v.is_finite() {
                    self.params.bauer_fcut_hz = v;
                    self.update_filters();
                }
            }
            "bauer_feed_db" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "bauer_feed_db must be a float".to_string())?;
                if v.is_finite() {
                    self.params.bauer_feed_db = v;
                    self.update_filters();
                }
            }
            "meier_level" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "meier_level must be a float".to_string())?;
                if v.is_finite() {
                    self.params.meier_level = v;
                }
            }
            "mb_low_freq_hz" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mb_low_freq_hz must be a float".to_string())?;
                if v.is_finite() {
                    self.params.mb_low_freq_hz = v;
                    self.update_filters();
                }
            }
            "mb_mid_high_freq_hz" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mb_mid_high_freq_hz must be a float".to_string())?;
                if v.is_finite() {
                    self.params.mb_mid_high_freq_hz = v;
                    self.update_filters();
                }
            }
            "mb_low_feed_db" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mb_low_feed_db must be a float".to_string())?;
                if v.is_finite() {
                    self.params.mb_low_feed_db = v;
                }
            }
            "mb_mid_feed_db" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mb_mid_feed_db must be a float".to_string())?;
                if v.is_finite() {
                    self.params.mb_mid_feed_db = v;
                }
            }
            "mb_high_feed_db" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "mb_high_feed_db must be a float".to_string())?;
                if v.is_finite() {
                    self.params.mb_high_feed_db = v;
                }
            }
            "itd_delay_ms" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "itd_delay_ms must be a float".to_string())?;
                if v.is_finite() {
                    self.params.itd_delay_ms = v.clamp(0.0, 1.0);
                    let effective = compute_dynamic_itd_ms(
                        self.params.head_yaw_deg,
                        self.params.itd_delay_ms,
                    );
                    self.itd_delay_l.set_delay(effective, self.sample_rate);
                    self.itd_delay_r.set_delay(effective, self.sample_rate);
                }
            }
            "head_yaw_deg" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "head_yaw_deg must be a float".to_string())?;
                if v.is_finite() {
                    self.params.head_yaw_deg = v.clamp(-90.0, 90.0);
                    self.yaw_smoother.set_target(self.params.head_yaw_deg);
                    let effective = compute_dynamic_itd_ms(
                        self.params.head_yaw_deg,
                        self.params.itd_delay_ms,
                    );
                    self.itd_delay_l.set_delay(effective, self.sample_rate);
                    self.itd_delay_r.set_delay(effective, self.sample_rate);
                }
            }
            "autogain_enabled" => {
                let v = value
                    .as_bool()
                    .ok_or_else(|| "autogain_enabled must be a boolean".to_string())?;
                self.params.autogain_enabled = v;
                if v && self.auto_gain.is_none() {
                    self.auto_gain = Some(sotf_host::auto_gain::AutoGain::new(
                        2,
                        self.sample_rate,
                        sotf_host::auto_gain::AutoGainParams {
                            enabled: true,
                            loudness_type: Default::default(),
                            max_gain_db: self.params.autogain_max_gain_db,
                            smoothing_ms: self.params.autogain_smoothing_ms,
                        },
                    )?);
                } else if !v {
                    self.auto_gain = None;
                }
            }
            "autogain_target_lufs" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "autogain_target_lufs must be a float".to_string())?;
                if v.is_finite() {
                    self.params.autogain_target_lufs = v;
                }
            }
            "autogain_max_gain_db" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "autogain_max_gain_db must be a float".to_string())?;
                if v.is_finite() {
                    self.params.autogain_max_gain_db = v;
                    if let Some(ag) = &mut self.auto_gain {
                        ag.set_max_gain_db(v);
                    }
                }
            }
            "autogain_smoothing_ms" => {
                let v = value
                    .as_float()
                    .ok_or_else(|| "autogain_smoothing_ms must be a float".to_string())?;
                if v.is_finite() {
                    self.params.autogain_smoothing_ms = v;
                    if let Some(ag) = &mut self.auto_gain {
                        ag.set_smoothing_ms(v);
                    }
                }
            }
            _ => return Err(format!("Unknown: {}", name)),
        }
        self.rebuild_cached_parameters();
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
            "itd_delay_ms" => Some(ParameterValue::Float(self.params.itd_delay_ms)),
            "head_yaw_deg" => Some(ParameterValue::Float(self.params.head_yaw_deg)),
            "autogain_enabled" => Some(ParameterValue::Bool(self.params.autogain_enabled)),
            "autogain_target_lufs" => Some(ParameterValue::Float(self.params.autogain_target_lufs)),
            "autogain_max_gain_db" => Some(ParameterValue::Float(self.params.autogain_max_gain_db)),
            "autogain_smoothing_ms" => {
                Some(ParameterValue::Float(self.params.autogain_smoothing_ms))
            }
            _ => None,
        }
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.update_filters();
        self.mix_smoother = Smoother::new(self.params.mix, 20.0, sr);
        self.yaw_smoother = Smoother::new(self.params.head_yaw_deg, 10.0, sr);
        let effective_itd = compute_dynamic_itd_ms(self.params.head_yaw_deg, self.params.itd_delay_ms);
        self.itd_delay_l = DelayLine::new(effective_itd, sr);
        self.itd_delay_r = DelayLine::new(effective_itd, sr);
        if let Some(ag) = &mut self.auto_gain {
            ag.set_sample_rate(sr).map_err(|e| e.to_string())?;
        }
        let cap = 4096;
        self.dry_l.resize(cap, 0.0);
        self.dry_r.resize(cap, 0.0);
        self.wet_l.resize(cap, 0.0);
        self.wet_r.resize(cap, 0.0);
        for b in &mut self.mb_bands_l {
            b.resize(cap, 0.0);
        }
        for b in &mut self.mb_bands_r {
            b.resize(cap, 0.0);
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.mix_smoother.reset(self.params.mix);
        self.itd_delay_l.reset();
        self.itd_delay_r.reset();
        self.mb_crossover_l.reset();
        self.mb_crossover_r.reset();
        if let Some(ag) = &mut self.auto_gain {
            ag.reset();
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        if !self.params.enabled || self.params.mode == CrossfeedMode::Off {
            return Ok(context.num_frames);
        }
        enable_ftz_daz();
        let nf = context.num_frames;
        if nf > self.dry_l.len() {
            self.dry_l.resize(nf, 0.0);
            self.dry_r.resize(nf, 0.0);
            self.wet_l.resize(nf, 0.0);
            self.wet_r.resize(nf, 0.0);
        }

        if let Some(ag) = &mut self.auto_gain {
            let _ = ag.measure_input(buffer);
        }

        // Advance yaw smoother and update ITD delay per block
        let smoothed_yaw = self.yaw_smoother.advance();
        if smoothed_yaw.abs() > 0.01 || self.params.itd_delay_ms > 0.0 {
            let effective = compute_dynamic_itd_ms(smoothed_yaw, self.params.itd_delay_ms);
            self.itd_delay_l.set_delay(effective, self.sample_rate);
            self.itd_delay_r.set_delay(effective, self.sample_rate);
        }

        deinterleave_stereo(buffer, &mut self.dry_l[..nf], &mut self.dry_r[..nf]);

        match self.params.mode {
            CrossfeedMode::Bauer => self.process_bauer(nf),
            CrossfeedMode::Meier => self.process_meier(nf),
            CrossfeedMode::Mb => self.process_mb(nf),
            _ => {
                self.wet_l[..nf].copy_from_slice(&self.dry_l[..nf]);
                self.wet_r[..nf].copy_from_slice(&self.dry_r[..nf]);
            }
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
    use crate::*;
    use sotf_host::plugin::ProcessContext;

    #[test]
    fn test_crossfeed_basic() {
        let mut p = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
        p.initialize(48000).unwrap();
        let mut b = vec![1.0, 0.0, 1.0, 0.0];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 2,
            },
        )
        .unwrap();
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

    #[test]
    fn test_bauer_uses_lowpass() {
        // Bauer mode should crossfeed low frequencies (LPF, per bs2b spec).
        // A DC signal should produce significant crossfeed;
        // a high-frequency signal should produce minimal crossfeed.
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
        params.mode = CrossfeedMode::Bauer;
        params.bauer_feed_db = 6.0;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        // DC signal: all energy in left channel
        let n = 4000;
        let mut dc_buf: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0]).collect();
        p.process_in_place(
            &mut dc_buf,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: n,
            },
        )
        .unwrap();

        // After settling, DC should bleed significantly into right channel via LPF crossfeed
        let last_r = dc_buf[(n - 1) * 2 + 1];
        assert!(
            last_r.abs() > 0.1,
            "Bauer LPF crossfeed: DC should bleed to right channel, got {}",
            last_r
        );
    }

    #[test]
    fn test_meier_basic() {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier);
        params.mode = CrossfeedMode::Meier;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        let mut buffer = vec![1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 4,
            },
        )
        .unwrap();
        assert!(buffer[1].abs() > 0.0);
    }

    #[test]
    fn test_mb_basic() {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
        params.mode = CrossfeedMode::Mb;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        let n = 100;
        let mut buffer: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0]).collect();
        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: n,
            },
        )
        .unwrap();
        // Right channel should get some crossfeed
        let last_r = buffer[(n - 1) * 2 + 1];
        assert!(last_r.abs() > 0.0, "MB crossfeed should bleed, got {}", last_r);
    }

    #[test]
    fn test_itd_delay() {
        let mut params = CrossfeedPluginParams::default();
        params.mode = CrossfeedMode::Bauer;
        params.itd_delay_ms = 0.5; // 0.5ms = 24 samples at 48kHz
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        // Impulse in left channel only
        let n = 100;
        let mut buffer = vec![0.0f32; n * 2];
        buffer[0] = 1.0; // impulse at frame 0, left channel

        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: n,
            },
        )
        .unwrap();

        // The crossfeed to right channel should be delayed by ~24 samples
        // Check that right channel has near-zero for the first few frames
        // and nonzero later
        let early_r: f32 = (0..10).map(|f| buffer[f * 2 + 1].abs()).sum();
        let late_r: f32 = (25..50).map(|f| buffer[f * 2 + 1].abs()).sum();
        assert!(
            late_r > early_r,
            "ITD delay: later right channel samples should exceed early ones. early={}, late={}",
            early_r,
            late_r
        );
    }

    #[test]
    fn test_itd_delay_zero() {
        // With itd_delay_ms = 0, delay line should be transparent
        let mut params = CrossfeedPluginParams::default();
        params.mode = CrossfeedMode::Bauer;
        params.itd_delay_ms = 0.0;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        let n = 100;
        let mut buffer: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0]).collect();
        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: n,
            },
        )
        .unwrap();
        // Should still work and produce crossfeed
        assert!(buffer[1].is_finite());
    }

    #[test]
    fn test_itd_parameter() {
        let mut p = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
        p.initialize(48000).unwrap();

        // Set ITD delay
        p.set_parameter(
            ParameterId("itd_delay_ms".to_string()),
            ParameterValue::Float(0.3),
        )
        .unwrap();

        let val = p.get_parameter(&ParameterId("itd_delay_ms".to_string()));
        assert_eq!(val, Some(ParameterValue::Float(0.3)));
    }

    #[test]
    fn test_itd_delay_accuracy() {
        // Set itd_delay_ms=0.5. Process an impulse on L only.
        // Verify the crossfeed to R arrives later than with itd_delay_ms=0.
        let n = 200;
        let sr = 48000;

        // Helper: process an L-only impulse and find the frame where R channel
        // first exceeds a threshold.
        let find_r_onset = |itd_ms: f32| -> usize {
            let mut params = CrossfeedPluginParams::default();
            params.mode = CrossfeedMode::Bauer;
            params.bauer_feed_db = 6.0;
            params.itd_delay_ms = itd_ms;
            params.mix = 1.0;
            let mut p = CrossfeedPlugin::new(params).unwrap();
            p.initialize(sr).unwrap();

            let mut buffer = vec![0.0f32; n * 2];
            buffer[0] = 1.0; // impulse at frame 0, L channel

            p.process_in_place(
                &mut buffer,
                &ProcessContext {
                    sample_rate: sr,
                    num_frames: n,
                },
            )
            .unwrap();

            // Find the first frame where |R| > threshold
            let threshold = 0.001;
            for f in 0..n {
                if buffer[f * 2 + 1].abs() > threshold {
                    return f;
                }
            }
            n // never found
        };

        let onset_no_delay = find_r_onset(0.0);
        let onset_with_delay = find_r_onset(0.5);

        // 0.5ms at 48kHz = 24 samples. The delayed version should arrive later.
        assert!(
            onset_with_delay > onset_no_delay,
            "ITD 0.5ms should delay R onset: no_delay_onset={}, delayed_onset={}",
            onset_no_delay,
            onset_with_delay
        );

        // The difference should be approximately 24 samples
        let diff = onset_with_delay - onset_no_delay;
        assert!(
            (diff as i32 - 24).unsigned_abs() <= 3,
            "ITD difference should be ~24 samples (0.5ms@48kHz), got {} (onset_no={}, onset_with={})",
            diff,
            onset_no_delay,
            onset_with_delay
        );
    }

    #[test]
    fn test_disabled_passthrough() {
        let mut params = CrossfeedPluginParams::default();
        params.enabled = false;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        let mut buffer = vec![1.0, 0.5, 0.3, 0.7];
        let original = buffer.clone();
        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 2,
            },
        )
        .unwrap();
        assert_eq!(buffer, original, "Disabled crossfeed should pass through unchanged");
    }

    #[test]
    fn test_crossfeed_frequency_response_low_vs_high() {
        // Crossfeed should affect low frequencies more than high frequencies.
        // Generate pure left-channel tones at 200Hz and 8kHz, measure how much
        // crossfeed leaks into the right channel at each frequency.
        let sr = 48000u32;
        let n = 10000; // frames
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: n,
        };

        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
        params.mode = CrossfeedMode::Bauer;
        params.bauer_feed_db = 6.0;

        // Helper: generate left-only sine, process, measure right channel energy in tail
        let measure_crossfeed = |freq: f32| -> f32 {
            let mut p = CrossfeedPlugin::new(params.clone()).unwrap();
            p.initialize(sr).unwrap();

            let mut buf: Vec<f32> = (0..n)
                .flat_map(|i| {
                    let t = i as f32 / sr as f32;
                    let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
                    [s, 0.0] // left only
                })
                .collect();
            p.process_in_place(&mut buf, &ctx).unwrap();

            // Measure right channel RMS in the last 2000 frames (skip transient)
            let tail_start = (n - 2000) * 2;
            let right_energy: f32 = buf[tail_start..]
                .chunks(2)
                .map(|c| c[1] * c[1])
                .sum::<f32>();
            (right_energy / 2000.0).sqrt()
        };

        let low_crossfeed = measure_crossfeed(200.0);
        let high_crossfeed = measure_crossfeed(8000.0);

        assert!(
            low_crossfeed > 0.001,
            "200Hz should produce measurable crossfeed: {low_crossfeed}"
        );
        assert!(
            low_crossfeed > high_crossfeed * 1.5,
            "Low-frequency crossfeed ({low_crossfeed:.4}) should be significantly more than high-frequency ({high_crossfeed:.4})"
        );
    }
}
