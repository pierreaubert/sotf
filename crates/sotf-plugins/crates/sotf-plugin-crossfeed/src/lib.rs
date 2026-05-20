// ============================================================================
// Crossfeed Plugin - Headphone crossfeed for speaker-like listening
// ============================================================================

pub mod params;

use sotf_host::lr4_crossover::MultibandLr4Crossover;
use sotf_host::param_specs::find_by_key as pk;

use crate::params::PARAMS as CF;
use sotf_host::param_bridge;
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
            mode: CrossfeedMode::Mb,
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

/// A mono delay line supporting fractional delays up to 1 ms at the active sample rate.
struct DelayLine {
    buffer: Vec<f32>,
    write_pos: usize,
    delay_samples: f32,
    capacity: usize,
}

impl DelayLine {
    fn new(delay_ms: f32, sample_rate: u32) -> Self {
        let capacity = Self::capacity_for_sample_rate(sample_rate);
        let delay_samples = Self::delay_samples(delay_ms, sample_rate, capacity);
        Self {
            buffer: vec![0.0; capacity],
            write_pos: 0,
            delay_samples,
            capacity,
        }
    }

    fn set_delay(&mut self, delay_ms: f32, sample_rate: u32) {
        let capacity = Self::capacity_for_sample_rate(sample_rate);
        if capacity != self.capacity {
            self.buffer.resize(capacity, 0.0);
            self.capacity = capacity;
            self.write_pos %= self.capacity;
        }
        self.delay_samples = Self::delay_samples(delay_ms, sample_rate, self.capacity);
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }

    fn capacity_for_sample_rate(sample_rate: u32) -> usize {
        (sample_rate as f32 * 0.001).ceil() as usize + 2
    }

    fn delay_samples(delay_ms: f32, sample_rate: u32, capacity: usize) -> f32 {
        ((delay_ms / 1000.0) * sample_rate as f32)
            .max(0.0)
            .min(capacity as f32 - 2.0)
    }

    #[inline]
    fn process(&mut self, sample: f32) -> f32 {
        if self.delay_samples <= f32::EPSILON {
            return sample;
        }
        self.buffer[self.write_pos] = sample;
        let int_delay = self.delay_samples.floor() as usize;
        let fract = self.delay_samples - int_delay as f32;
        let read_pos_base = (self.write_pos + self.capacity - int_delay) % self.capacity;
        let read_pos_next = (read_pos_base + self.capacity - 1) % self.capacity;
        let out = self.buffer[read_pos_base] * (1.0 - fract) + self.buffer[read_pos_next] * fract;
        self.write_pos = (self.write_pos + 1) % self.capacity;
        out
    }
}

pub struct CrossfeedPlugin {
    sample_rate: u32,
    params: CrossfeedPluginParams,

    // Bauer: low-shelf cut on the difference signal (L-R)
    bauer_shelf: Biquad,

    meier_lpf_l: Biquad,
    meier_lpf_r: Biquad,
    meier_allpass_l: Biquad,
    meier_allpass_r: Biquad,

    // Multiband: true LR4 crossover (3-band: low/mid/high)
    mb_crossover_l: MultibandLr4Crossover<f32>,
    mb_crossover_r: MultibandLr4Crossover<f32>,

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
    mb_feed_linear: [f32; 3],
    mb_wet_norm: f32,

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

/// Compute per-ear ITD delays (ms) from yaw angle (degrees) and static offset.
///
/// Returns `(delay_l, delay_r)` where `delay_l` is the delay on the L→R crossfeed path
/// and `delay_r` is the delay on the R→L crossfeed path.
///
/// Acoustic model: the crossfeed path for the ear *farther* from the source gets the
/// longer delay.  With positive yaw (head turned right) the left ear is farther, so
/// the L→R path (carrying left-channel signal to the right ear) is longer.
///
/// `base = static_itd_ms / 2` so that when yaw = 0 both paths carry equal delay
/// summing to `static_itd_ms`.
fn compute_differential_itd_ms(head_yaw_deg: f32, static_itd_ms: f32) -> (f32, f32) {
    let yaw_rad = head_yaw_deg * std::f32::consts::PI / 180.0;
    let dynamic_ms = HEAD_RADIUS_M * yaw_rad.sin() / SPEED_OF_SOUND * 1000.0;
    let base = static_itd_ms * 0.5;
    // Positive yaw → left ear farther → longer L→R crossfeed delay
    let delay_l = (base + dynamic_ms).clamp(0.0, 1.0);
    let delay_r = (base - dynamic_ms).clamp(0.0, 1.0);
    (delay_l, delay_r)
}

impl CrossfeedPlugin {
    pub fn new(params: CrossfeedPluginParams) -> Result<Self, String> {
        let sr = 44100;
        let mut plugin = Self {
            sample_rate: sr,
            params: params.clone(),

            // Bauer: low-shelf cut on the difference signal
            bauer_shelf: Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowshelf,
                params.bauer_fcut_hz as f64,
                sr as f64,
                0.707,
                -(params.bauer_feed_db as f64),
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
                sr as f32,
                1,
            ),
            mb_crossover_r: MultibandLr4Crossover::new(
                &[params.mb_low_freq_hz, params.mb_mid_high_freq_hz],
                sr as f32,
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
            mb_feed_linear: [
                fast_pow10(params.mb_low_feed_db / 20.0),
                fast_pow10(params.mb_mid_feed_db / 20.0),
                fast_pow10(params.mb_high_feed_db / 20.0),
            ],
            mb_wet_norm: 1.0,

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
        plugin.update_mb_feed_cache();
        plugin.rebuild_cached_parameters();

        Ok(plugin)
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.params.mode as usize as f64),
            1 => Some(self.params.preset as usize as f64),
            2 => Some(if self.params.enabled { 1.0 } else { 0.0 }),
            3 => Some(self.params.mix as f64),
            4 => Some(self.params.bauer_fcut_hz as f64),
            5 => Some(self.params.bauer_feed_db as f64),
            6 => Some(self.params.meier_level as f64),
            7 => Some(self.params.mb_low_freq_hz as f64),
            8 => Some(self.params.mb_mid_high_freq_hz as f64),
            9 => Some(self.params.mb_low_feed_db as f64),
            10 => Some(self.params.mb_mid_feed_db as f64),
            11 => Some(self.params.mb_high_feed_db as f64),
            12 => Some(self.params.itd_delay_ms as f64),
            13 => Some(if self.params.autogain_enabled {
                1.0
            } else {
                0.0
            }),
            14 => Some(self.params.autogain_target_lufs as f64),
            15 => Some(self.params.autogain_max_gain_db as f64),
            16 => Some(self.params.autogain_smoothing_ms as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {
                self.params.mode = match value as usize {
                    0 => CrossfeedMode::Off,
                    1 => CrossfeedMode::Bauer,
                    2 => CrossfeedMode::Meier,
                    3 => CrossfeedMode::Mb,
                    _ => CrossfeedMode::Off,
                };
            }
            1 => {
                self.params.preset = match value as usize {
                    0 => CrossfeedPreset::Default,
                    1 => CrossfeedPreset::Cmoy,
                    2 => CrossfeedPreset::Meier,
                    3 => CrossfeedPreset::Mb,
                    4 => CrossfeedPreset::Off,
                    _ => CrossfeedPreset::Default,
                };
            }
            2 => self.params.enabled = value > 0.5,
            3 => self.params.mix = value as f32,
            4 => self.params.bauer_fcut_hz = value as f32,
            5 => self.params.bauer_feed_db = value as f32,
            6 => self.params.meier_level = value as f32,
            7 => self.params.mb_low_freq_hz = value as f32,
            8 => self.params.mb_mid_high_freq_hz = value as f32,
            9 => self.params.mb_low_feed_db = value as f32,
            10 => self.params.mb_mid_feed_db = value as f32,
            11 => self.params.mb_high_feed_db = value as f32,
            12 => self.params.itd_delay_ms = value as f32,
            13 => self.params.autogain_enabled = value > 0.5,
            14 => self.params.autogain_target_lufs = value as f32,
            15 => self.params.autogain_max_gain_db = value as f32,
            16 => self.params.autogain_smoothing_ms = value as f32,
            _ => {}
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(CF, |i| self.param_value(i));
        // Append parameters not in PARAMS
        self.cached_parameters.push(
            Parameter::new_float(
                "head_yaw_deg",
                "Head Yaw",
                self.params.head_yaw_deg,
                -90.0,
                90.0,
            )
            .with_group("Head Tracking"),
        );
    }

    fn update_mb_feed_cache(&mut self) {
        self.mb_feed_linear = [
            fast_pow10(self.params.mb_low_feed_db / 20.0),
            fast_pow10(self.params.mb_mid_feed_db / 20.0),
            fast_pow10(self.params.mb_high_feed_db / 20.0),
        ];
        self.mb_wet_norm = 1.0
            / (1.0
                + self.mb_feed_linear[0]
                    .max(self.mb_feed_linear[1])
                    .max(self.mb_feed_linear[2]));
    }

    fn update_filters(&mut self) {
        let sr = self.sample_rate as f64;

        // Bauer: low-shelf cut on the difference signal
        self.bauer_shelf = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowshelf,
            self.params.bauer_fcut_hz as f64,
            sr,
            0.707,
            -(self.params.bauer_feed_db as f64),
        );

        // Meier: LPF + allpass — must be recomputed for every sample rate change
        self.meier_lpf_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            650.0,
            sr,
            0.707,
            0.0,
        );
        self.meier_lpf_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowpass,
            650.0,
            sr,
            0.707,
            0.0,
        );
        self.meier_allpass_l = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::AllPass,
            1000.0,
            sr,
            0.5,
            0.0,
        );
        self.meier_allpass_r = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::AllPass,
            1000.0,
            sr,
            0.5,
            0.0,
        );

        // Multiband: true LR4 crossover
        self.mb_crossover_l.reinit(
            &[self.params.mb_low_freq_hz, self.params.mb_mid_high_freq_hz],
            self.sample_rate as f32,
            1,
        );
        self.mb_crossover_r.reinit(
            &[self.params.mb_low_freq_hz, self.params.mb_mid_high_freq_hz],
            self.sample_rate as f32,
            1,
        );
    }

    #[inline(always)]
    fn process_bauer(&mut self, nf: usize) {
        let has_itd = self.params.itd_delay_ms > 0.0;
        for i in 0..nf {
            let x_l = self.dry_l[i];
            let x_r = self.dry_r[i];
            // Low-shelf cut on the difference signal: reduces stereo width at low frequencies
            let diff = x_l - x_r;
            let diff_f = self.bauer_shelf.process(diff as f64) as f32;
            // Crossfeed is derived from the part of the difference signal that was removed
            let mut cross_r = (diff_f - diff) * 0.5;
            let mut cross_l = (diff - diff_f) * 0.5;
            // Apply ITD delay to the crossfeed path
            if has_itd {
                cross_r = self.itd_delay_r.process(cross_r);
                cross_l = self.itd_delay_l.process(cross_l);
            }
            self.wet_l[i] = x_l + cross_r;
            self.wet_r[i] = x_r + cross_l;
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
        let [fl, fm, fh] = self.mb_feed_linear;
        let wet_norm = self.mb_wet_norm;
        let has_itd = self.params.itd_delay_ms > 0.0;

        // Resize band buffers if needed (normally pre-allocated in initialize())
        for b in &mut self.mb_bands_l {
            if b.len() < nf {
                b.resize(nf, 0.0);
            }
        }
        for b in &mut self.mb_bands_r {
            if b.len() < nf {
                b.resize(nf, 0.0);
            }
        }

        // Process each sample through the crossover using the pre-allocated band buffers.
        // We call process_frame one sample at a time but write into pre-allocated slices,
        // avoiding 8 per-sample stack array allocations.
        // Use split_at_mut to convince the borrow checker that the three band slices are
        // disjoint, since indexing `[Vec; 3]` multiple times mutably in one expression
        // violates the alias rules at the array level.
        for i in 0..nf {
            let input_l = [self.dry_l[i]];
            let input_r = [self.dry_r[i]];

            let (bl01, bl2) = self.mb_bands_l.split_at_mut(2);
            let (bl0, bl1) = bl01.split_at_mut(1);
            self.mb_crossover_l.process_frame(
                &input_l,
                &mut [
                    &mut bl0[0][i..i + 1],
                    &mut bl1[0][i..i + 1],
                    &mut bl2[0][i..i + 1],
                ],
            );

            let (br01, br2) = self.mb_bands_r.split_at_mut(2);
            let (br0, br1) = br01.split_at_mut(1);
            self.mb_crossover_r.process_frame(
                &input_r,
                &mut [
                    &mut br0[0][i..i + 1],
                    &mut br1[0][i..i + 1],
                    &mut br2[0][i..i + 1],
                ],
            );
        }

        for i in 0..nf {
            let low_l = self.mb_bands_l[0][i];
            let mid_l = self.mb_bands_l[1][i];
            let high_l = self.mb_bands_l[2][i];
            let low_r = self.mb_bands_r[0][i];
            let mid_r = self.mb_bands_r[1][i];
            let high_r = self.mb_bands_r[2][i];

            // Compute crossfeed signal per band
            let mut cross_l = fl * low_l + fm * mid_l + fh * high_l;
            let mut cross_r = fl * low_r + fm * mid_r + fh * high_r;

            // Apply ITD delay to the crossfeed path
            if has_itd {
                cross_l = self.itd_delay_l.process(cross_l);
                cross_r = self.itd_delay_r.process(cross_r);
            }

            // Mix crossfeed from opposite channel with headroom normalization.
            self.wet_l[i] = ((low_l + mid_l + high_l) + cross_r) * wet_norm;
            self.wet_r[i] = ((low_r + mid_r + high_r) + cross_l) * wet_norm;
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
        // head_yaw_deg is not in PARAMS — handle separately
        if id.0 == "head_yaw_deg" {
            let v = value
                .as_float()
                .ok_or_else(|| "head_yaw_deg must be a float".to_string())?;
            if v.is_finite() {
                self.params.head_yaw_deg = v.clamp(-90.0, 90.0);
                self.yaw_smoother.set_target(self.params.head_yaw_deg);
                // Do NOT update delay lines here — process_in_place owns delay line updates
                // via the yaw smoother, preventing the double-discontinuity bug.
            }
            self.rebuild_cached_parameters();
            return Ok(());
        }

        let idx = param_bridge::set_parameter(CF, &id, &value, |i, v| self.set_param_value(i, v))?;

        // Side effects based on parameter index
        match idx {
            3 => self.mix_smoother.set_target(self.params.mix), // mix
            4 | 5 => self.update_filters(),                     // bauer_fcut_hz, bauer_feed_db
            7 | 8 => self.update_filters(), // mb_low_freq_hz, mb_mid_high_freq_hz
            9..=11 => self.update_mb_feed_cache(),
            12 => {
                // itd_delay_ms — delay lines are updated in process_in_place, not here.
            }
            13 => {
                // autogain_enabled
                if self.params.autogain_enabled && self.auto_gain.is_none() {
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
                } else if !self.params.autogain_enabled {
                    self.auto_gain = None;
                }
            }
            15 => {
                // autogain_max_gain_db
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_max_gain_db(self.params.autogain_max_gain_db);
                }
            }
            16 => {
                // autogain_smoothing_ms
                if let Some(ag) = &mut self.auto_gain {
                    ag.set_smoothing_ms(self.params.autogain_smoothing_ms);
                }
            }
            _ => {}
        }

        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // head_yaw_deg is not in PARAMS — handle separately
        if id.0 == "head_yaw_deg" {
            return Some(ParameterValue::Float(self.params.head_yaw_deg));
        }
        param_bridge::get_parameter(CF, id, |i| self.param_value(i))
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        self.sample_rate = sr;
        self.update_filters();
        self.mix_smoother = Smoother::new(self.params.mix, 20.0, sr);
        self.yaw_smoother = Smoother::new(self.params.head_yaw_deg, 10.0, sr);
        let (itd_l, itd_r) =
            compute_differential_itd_ms(self.params.head_yaw_deg, self.params.itd_delay_ms);
        self.itd_delay_l = DelayLine::new(itd_l, sr);
        self.itd_delay_r = DelayLine::new(itd_r, sr);
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
        self.yaw_smoother.reset(self.params.head_yaw_deg);
        self.itd_delay_l.reset();
        self.itd_delay_r.reset();
        self.bauer_shelf.reset();
        self.meier_lpf_l.reset();
        self.meier_lpf_r.reset();
        self.meier_allpass_l.reset();
        self.meier_allpass_r.reset();
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

        // Advance yaw smoother by the full block size (not just 1 sample).
        // This gives the correct smoothing rate: a 10ms time-constant at 48kHz means
        // the yaw settles in ~480 samples, regardless of block size.
        let smoothed_yaw = self.yaw_smoother.next_n(nf);
        if smoothed_yaw.abs() > 0.01 || self.params.itd_delay_ms > 0.0 {
            let (itd_l, itd_r) =
                compute_differential_itd_ms(smoothed_yaw, self.params.itd_delay_ms);
            self.itd_delay_l.set_delay(itd_l, self.sample_rate);
            self.itd_delay_r.set_delay(itd_r, self.sample_rate);
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

        // Apply mix with a linear ramp across the block to avoid zipper noise.
        // `current()` is the mix value at the start of this block; `next_n(nf)` advances
        // it to the end-of-block value.
        let mix_start = self.mix_smoother.current();
        let mix_end = self.mix_smoother.next_n(nf);
        let mix_step = if nf > 1 {
            (mix_end - mix_start) / nf as f32
        } else {
            0.0
        };
        for i in 0..nf {
            let mix = mix_start + mix_step * i as f32;
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
    fn test_mb_feed_linear_cache_updates_on_parameter_change() {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
        params.mode = CrossfeedMode::Mb;
        params.mb_low_feed_db = 0.0;
        params.mb_mid_feed_db = 6.0;
        params.mb_high_feed_db = 3.0;
        let mut p = CrossfeedPlugin::new(params).unwrap();

        let before = p.mb_feed_linear;
        p.set_parameter(
            ParameterId::from("mb_mid_feed_db"),
            ParameterValue::Float(0.0),
        )
        .unwrap();

        assert_ne!(
            before, p.mb_feed_linear,
            "linear feed cache should change when feed dB changes"
        );
        assert!(
            (p.mb_feed_linear[1] - 1.0).abs() < 1e-4,
            "0 dB mid feed should cache as unity gain"
        );
    }

    /// Regression: Bauer mode used a plain lowpass on the per-channel crossfeed path,
    /// which caused a bass boost on mono signals and a steep roll-off. A proper Bauer
    /// crossfeed applies a low-shelf cut to the difference signal (L-R), preserving
    /// mono energy and gently attenuating low-frequency stereo width.
    #[test]
    fn test_bauer_mono_preserved() {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
        params.mode = CrossfeedMode::Bauer;
        params.bauer_feed_db = 6.0;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        let n = 4000;
        let mut buf: Vec<f32> = (0..n).flat_map(|_| [0.5f32, 0.5]).collect();
        p.process_in_place(
            &mut buf,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: n,
            },
        )
        .unwrap();

        let last_l = buf[(n - 1) * 2];
        let last_r = buf[(n - 1) * 2 + 1];
        // Old lowpass code boosted mono by feed*lowpass(mono) ≈ 3.0.
        // Proper low-shelf on difference leaves mono unchanged.
        assert!(
            (last_l - 0.5).abs() < 0.01 && (last_r - 0.5).abs() < 0.01,
            "Mono signal should be preserved, got L={last_l}, R={last_r}"
        );
    }

    /// Regression: Bauer mode should apply a low-shelf cut to the difference signal,
    /// attenuating low-frequency stereo width while preserving high-frequency width.
    #[test]
    fn test_bauer_difference_shelved() {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
        params.mode = CrossfeedMode::Bauer;
        params.bauer_feed_db = 6.0;
        let sr = 48000u32;
        let n = 8000;

        // Low-frequency stereo difference
        let mut buf_lf: Vec<f32> = (0..n)
            .flat_map(|i| {
                let t = i as f32 / sr as f32;
                let s = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.5;
                [s, -s]
            })
            .collect();
        let mut p = CrossfeedPlugin::new(params.clone()).unwrap();
        p.initialize(sr).unwrap();
        p.process_in_place(
            &mut buf_lf,
            &ProcessContext {
                sample_rate: sr,
                num_frames: n,
            },
        )
        .unwrap();

        let tail_start = (n - 2000) * 2;
        let diff_rms_lf: f32 = buf_lf[tail_start..]
            .chunks(2)
            .map(|c| {
                let d = c[0] - c[1];
                d * d
            })
            .sum::<f32>()
            .sqrt()
            / (2000.0f32).sqrt();

        // With a -6 dB shelf, low-frequency difference should be attenuated
        assert!(
            diff_rms_lf < 0.5,
            "Low-frequency difference should be attenuated by shelf, got {diff_rms_lf}"
        );

        // High-frequency stereo difference
        let mut buf_hf: Vec<f32> = (0..n)
            .flat_map(|i| {
                let t = i as f32 / sr as f32;
                let s = (2.0 * std::f32::consts::PI * 10000.0 * t).sin() * 0.5;
                [s, -s]
            })
            .collect();
        let mut p2 = CrossfeedPlugin::new(params).unwrap();
        p2.initialize(sr).unwrap();
        p2.process_in_place(
            &mut buf_hf,
            &ProcessContext {
                sample_rate: sr,
                num_frames: n,
            },
        )
        .unwrap();

        let diff_rms_hf: f32 = buf_hf[tail_start..]
            .chunks(2)
            .map(|c| {
                let d = c[0] - c[1];
                d * d
            })
            .sum::<f32>()
            .sqrt()
            / (2000.0f32).sqrt();

        // High-frequency difference should be nearly unchanged
        assert!(
            diff_rms_hf > 0.6,
            "High-frequency difference should be preserved, got {diff_rms_hf}"
        );
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
        // Bauer mode should crossfeed low frequencies (low-shelf on difference, per bs2b spec).
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
        assert!(
            last_r.abs() > 0.0,
            "MB crossfeed should bleed, got {}",
            last_r
        );
    }

    #[test]
    fn test_mb_mono_signal_is_headroom_normalized() {
        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb);
        params.mode = CrossfeedMode::Mb;
        params.mix = 1.0;
        params.autogain_enabled = false;

        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(48000).unwrap();

        let n = 4096;
        let mut buffer: Vec<f32> = (0..n).flat_map(|_| [0.5f32, 0.5f32]).collect();
        p.process_in_place(
            &mut buffer,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: n,
            },
        )
        .unwrap();

        let tail_peak = buffer[(n / 2) * 2..]
            .iter()
            .copied()
            .map(f32::abs)
            .fold(0.0, f32::max);
        assert!(
            tail_peak <= 0.75,
            "default multiband mono output should stay headroom-normalized, got peak {tail_peak}"
        );
    }

    #[test]
    fn test_itd_delay() {
        let params = CrossfeedPluginParams {
            mode: CrossfeedMode::Bauer,
            itd_delay_ms: 0.5, // 0.5ms = 24 samples at 48kHz
            ..CrossfeedPluginParams::default()
        };
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
        let params = CrossfeedPluginParams {
            mode: CrossfeedMode::Bauer,
            itd_delay_ms: 0.0,
            ..CrossfeedPluginParams::default()
        };
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
    fn test_delay_line_supports_fractional_and_high_sample_rate_delay() {
        let mut delay = DelayLine::new(1.0, 192000);
        assert!(
            delay.capacity >= 194,
            "1ms at 192kHz needs at least 192 samples plus interpolation headroom"
        );

        delay.set_delay(0.5, 48000);
        assert!(
            (delay.delay_samples - 24.0).abs() < 1e-5,
            "0.5ms at 48kHz should be represented as 24 samples"
        );

        delay.set_delay(0.25, 44100);
        assert!(
            delay.delay_samples.fract() > 0.0,
            "0.25ms at 44.1kHz should preserve a fractional delay"
        );
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
            let params = CrossfeedPluginParams {
                mode: CrossfeedMode::Bauer,
                bauer_feed_db: 6.0,
                itd_delay_ms: itd_ms,
                mix: 1.0,
                ..CrossfeedPluginParams::default()
            };
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

        // With the differential-ITD model, itd_delay_ms is split equally across the two
        // crossfeed paths (base = itd_ms / 2 per path when yaw = 0).
        // So 0.5ms → each path gets 0.25ms = 12 samples at 48kHz.
        // The delayed version should arrive later.
        assert!(
            onset_with_delay > onset_no_delay,
            "ITD 0.5ms should delay R onset: no_delay_onset={}, delayed_onset={}",
            onset_no_delay,
            onset_with_delay
        );

        // The difference should be approximately 12 samples (0.25ms at 48kHz, half of 0.5ms ITD)
        let diff = onset_with_delay - onset_no_delay;
        assert!(
            (diff as i32 - 12).unsigned_abs() <= 3,
            "ITD difference should be ~12 samples (0.25ms per path at 48kHz), got {} \
             (onset_no={}, onset_with={})",
            diff,
            onset_no_delay,
            onset_with_delay
        );
    }

    #[test]
    fn test_disabled_passthrough() {
        let params = CrossfeedPluginParams {
            enabled: false,
            ..CrossfeedPluginParams::default()
        };
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
        assert_eq!(
            buffer, original,
            "Disabled crossfeed should pass through unchanged"
        );
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

    // -------------------------------------------------------------------------
    // Regression tests for bugs fixed from code review
    // -------------------------------------------------------------------------

    /// Bug: Meier filters were not updated when sample rate changed from 44100 to 48000.
    /// Verify that both sample rates produce consistent crossfeed RMS for a tone well
    /// below the 650 Hz LPF cutoff (should pass freely at both rates).
    #[test]
    fn test_meier_filter_coefficients_correct_after_sample_rate_change() {
        let n = 8000usize;
        let freq = 200.0f32;

        let measure_rms = |sr: u32| -> f32 {
            let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier);
            params.mode = CrossfeedMode::Meier;
            params.mix = 1.0;
            let mut p = CrossfeedPlugin::new(params).unwrap();
            p.initialize(sr).unwrap();

            let mut buf: Vec<f32> = (0..n)
                .flat_map(|i| {
                    let t = i as f32 / sr as f32;
                    let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
                    [s, 0.0f32]
                })
                .collect();
            p.process_in_place(
                &mut buf,
                &ProcessContext {
                    sample_rate: sr,
                    num_frames: n,
                },
            )
            .unwrap();

            let tail_start = (n * 3 / 4) * 2;
            let rms: f32 = buf[tail_start..]
                .chunks(2)
                .map(|c| c[1] * c[1])
                .sum::<f32>();
            (rms / (n / 4) as f32).sqrt()
        };

        let rms_44 = measure_rms(44100);
        let rms_48 = measure_rms(48000);

        assert!(
            rms_44 > 0.001,
            "Meier crossfeed should produce output at 44100: rms={rms_44}"
        );
        assert!(
            rms_48 > 0.001,
            "Meier crossfeed should produce output at 48000: rms={rms_48}"
        );
        // At 200 Hz (well below cutoff) both rates should produce similar gain. 20% tolerance.
        let ratio = if rms_44 > rms_48 {
            rms_44 / rms_48
        } else {
            rms_48 / rms_44
        };
        assert!(
            ratio < 1.2,
            "Meier crossfeed at 200 Hz should be consistent across sample rates \
             (44100={rms_44:.4}, 48000={rms_48:.4}, ratio={ratio:.3})"
        );
    }

    /// Bug: ITD was modeled symmetrically — both crossfeed paths got the same delay.
    /// With positive yaw, the L→R path should be longer than the R→L path.
    #[test]
    fn test_itd_yaw_asymmetry() {
        let sr = 48000u32;
        let n = 300usize;

        let find_onset = |impulse_on_left: bool, yaw_deg: f32| -> usize {
            let params = CrossfeedPluginParams {
                mode: CrossfeedMode::Bauer,
                bauer_feed_db: 6.0,
                itd_delay_ms: 0.5,
                head_yaw_deg: yaw_deg,
                mix: 1.0,
                ..CrossfeedPluginParams::default()
            };
            let mut p = CrossfeedPlugin::new(params).unwrap();
            p.initialize(sr).unwrap();

            let mut buffer = vec![0.0f32; n * 2];
            if impulse_on_left {
                buffer[0] = 1.0;
            } else {
                buffer[1] = 1.0;
            }

            p.process_in_place(
                &mut buffer,
                &ProcessContext {
                    sample_rate: sr,
                    num_frames: n,
                },
            )
            .unwrap();

            let threshold = 0.001;
            for f in 0..n {
                let idx = if impulse_on_left { f * 2 + 1 } else { f * 2 };
                if buffer[idx].abs() > threshold {
                    return f;
                }
            }
            n
        };

        // At yaw=0: symmetric — both paths carry equal delay (base = 0.25 ms each)
        let onset_l_to_r_yaw0 = find_onset(true, 0.0);
        let onset_r_to_l_yaw0 = find_onset(false, 0.0);
        assert!(
            (onset_l_to_r_yaw0 as i32 - onset_r_to_l_yaw0 as i32).unsigned_abs() <= 2,
            "At yaw=0 both paths should have equal delay: L→R={onset_l_to_r_yaw0}, R→L={onset_r_to_l_yaw0}"
        );

        // At positive yaw: L→R path should be longer (larger onset index)
        let onset_l_to_r_pos = find_onset(true, 45.0);
        let onset_r_to_l_pos = find_onset(false, 45.0);
        assert!(
            onset_l_to_r_pos >= onset_r_to_l_pos,
            "Positive yaw: L→R delay ({onset_l_to_r_pos}) should be >= R→L ({onset_r_to_l_pos})"
        );
    }

    /// Bug: mix smoother advanced to end-of-block value and applied it uniformly,
    /// causing a step discontinuity instead of a ramp.
    /// Verify that after a mix change the right channel output increases across the block.
    #[test]
    fn test_mix_ramp_no_step_discontinuity() {
        let sr = 48000u32;
        let n = 512usize;

        let mut params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
        params.mode = CrossfeedMode::Bauer;
        params.mix = 0.0;
        let mut p = CrossfeedPlugin::new(params).unwrap();
        p.initialize(sr).unwrap();

        // Warm-up block to settle smoother at mix=0
        let mut warmup = vec![0.5f32; n * 2];
        p.process_in_place(
            &mut warmup,
            &ProcessContext {
                sample_rate: sr,
                num_frames: n,
            },
        )
        .unwrap();

        // Jump mix to 1.0
        p.set_parameter(
            sotf_host::parameters::ParameterId("mix".to_string()),
            sotf_host::parameters::ParameterValue::Float(1.0),
        )
        .unwrap();

        // Process DC on L only
        let mut buf: Vec<f32> = (0..n).flat_map(|_| [1.0f32, 0.0f32]).collect();
        p.process_in_place(
            &mut buf,
            &ProcessContext {
                sample_rate: sr,
                num_frames: n,
            },
        )
        .unwrap();

        // Right channel: dry_r=0, wet_r>0 (crossfeed).  With a ramp, early < late.
        let first_r = buf[1].abs();
        let last_r = buf[(n - 1) * 2 + 1].abs();
        assert!(
            last_r > first_r,
            "Mix ramp: last right sample ({last_r:.6}) should exceed first ({first_r:.6})"
        );
    }

    /// Regression: reset() must clear all filter state so that a second
    /// playback pass starts from the same deterministic state as a fresh
    /// plugin. Previously bauer_shelf, meier LPF/allpass, and yaw_smoother
    /// were not reset, causing stale filter tails and wrong yaw values.
    #[test]
    fn test_reset_clears_all_filter_state() {
        let sr = 48000;
        let n = 512;

        // Create two identical plugins
        let params = CrossfeedPluginParams {
            mode: CrossfeedMode::Meier,
            meier_level: 0.5,
            head_yaw_deg: 30.0,
            itd_delay_ms: 0.3,
            mix: 1.0,
            ..CrossfeedPluginParams::default()
        };
        let mut p1 = CrossfeedPlugin::new(params.clone()).unwrap();
        p1.initialize(sr).unwrap();
        let mut p2 = CrossfeedPlugin::new(params.clone()).unwrap();
        p2.initialize(sr).unwrap();

        // Run p1 for one block to warm up filter state
        let mut block1: Vec<f32> = (0..n)
            .flat_map(|i| [(i as f32 * 0.01).sin(), 0.0f32])
            .collect();
        p1.process_in_place(
            &mut block1,
            &ProcessContext {
                sample_rate: sr,
                num_frames: n,
            },
        )
        .unwrap();

        // Reset p1 — after this it should behave like a fresh p2
        p1.reset();

        // Process the same impulse on both
        let mut impulse1 = vec![0.0f32; n * 2];
        impulse1[0] = 1.0;
        let mut impulse2 = impulse1.clone();

        p1.process_in_place(
            &mut impulse1,
            &ProcessContext {
                sample_rate: sr,
                num_frames: n,
            },
        )
        .unwrap();
        p2.process_in_place(
            &mut impulse2,
            &ProcessContext {
                sample_rate: sr,
                num_frames: n,
            },
        )
        .unwrap();

        // Outputs should match exactly (or very closely)
        for i in 0..(n * 2) {
            assert!(
                (impulse1[i] - impulse2[i]).abs() < 1e-5,
                "reset() did not fully clear state: sample {} differs by {}",
                i,
                (impulse1[i] - impulse2[i]).abs()
            );
        }
    }
}
