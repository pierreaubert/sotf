use super::crossfeed_plugin_params::CrossfeedPluginParams;
use super::delay_line::DelayLine;
use super::misc::compute_differential_itd_ms;
use super::types::CrossfeedMode;
use super::types::CrossfeedPreset;
use crate::params::PARAMS as CF;
use math_audio_dsp::fast_math::fast_pow10;
use math_audio_iir_fir::{Biquad, BiquadCoefficients};
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{deinterleave_stereo, enable_ftz_daz, interleave_stereo};
use sotf_host::smoothing::Smoother;

const BAUER_FILTER_TRANSITION_SAMPLES: usize = 128;
// The parameter schema exposes -60 dB as a finite, serializable Off endpoint.
// Treating it as exactly zero avoids a residual crossfeed path while retaining
// a normal float range for hosts and presets.
const MB_FEED_OFF_DB: f32 = -60.0;

pub struct CrossfeedPlugin {
    pub(super) sample_rate: u32,
    initialized: bool,
    pub(super) params: CrossfeedPluginParams,

    // Bauer: low-shelf cut on the difference signal (L-R)
    pub(super) bauer_shelf: Biquad,
    bauer_coefficients: BiquadCoefficients<f64>,
    bauer_transition_target: BiquadCoefficients<f64>,
    bauer_transition_remaining: usize,

    pub(super) meier_lpf_l: Biquad,
    pub(super) meier_lpf_r: Biquad,
    pub(super) meier_allpass_l: Biquad,
    pub(super) meier_allpass_r: Biquad,

    // Multiband: true LR4 crossover (3-band: low/mid/high)
    pub(super) mb_low_l: Lr4Crossover<f32>,
    pub(super) mb_high_l: Lr4Crossover<f32>,
    pub(super) mb_low_r: Lr4Crossover<f32>,
    pub(super) mb_high_r: Lr4Crossover<f32>,

    // ITD delay lines (one per crossfeed path)
    pub(super) itd_delay_l: DelayLine,
    pub(super) itd_delay_r: DelayLine,

    // Pre-allocated flat buffers for deinterleaved processing
    pub(super) dry_l: Vec<f32>,
    pub(super) dry_r: Vec<f32>,
    pub(super) wet_l: Vec<f32>,
    pub(super) wet_r: Vec<f32>,

    pub(super) mb_feed_linear: [f32; 3],
    // Constant-power normalization per band. Keeping this per-band prevents
    // changing one feed control from attenuating unrelated frequency bands.
    pub(super) mb_wet_norm: [f32; 3],

    // Auto gain helper
    pub(super) auto_gain: sotf_host::auto_gain::AutoGain,

    // Smoothing
    pub(super) mix_smoother: Smoother,
    pub(super) yaw_smoother: Smoother,
    pub(super) cached_parameters: Vec<Parameter>,
}

impl CrossfeedPlugin {
    pub fn new(params: CrossfeedPluginParams) -> Result<Self, String> {
        let sr = 44100;
        Self::validate_params(&params, sr)?;
        let cap = params.max_block_frames;
        let bauer_shelf = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowshelf,
            params.bauer_fcut_hz as f64,
            sr as f64,
            0.707,
            -(params.bauer_feed_db as f64),
        );
        let bauer_coefficients = bauer_shelf.coefficients();
        let mut plugin = Self {
            sample_rate: sr,
            initialized: false,
            params: params.clone(),

            // Bauer: low-shelf cut on the difference signal
            bauer_shelf,
            bauer_coefficients,
            bauer_transition_target: bauer_coefficients,
            bauer_transition_remaining: 0,

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
            mb_low_l: Lr4Crossover::new(params.mb_low_freq_hz, sr as f32, 1),
            mb_high_l: Lr4Crossover::new(params.mb_mid_high_freq_hz, sr as f32, 1),
            mb_low_r: Lr4Crossover::new(params.mb_low_freq_hz, sr as f32, 1),
            mb_high_r: Lr4Crossover::new(params.mb_mid_high_freq_hz, sr as f32, 1),

            // ITD delay lines
            itd_delay_l: DelayLine::new(params.itd_delay_ms, sr),
            itd_delay_r: DelayLine::new(params.itd_delay_ms, sr),

            dry_l: vec![0.0; cap],
            dry_r: vec![0.0; cap],
            wet_l: vec![0.0; cap],
            wet_r: vec![0.0; cap],
            mb_feed_linear: [
                fast_pow10(params.mb_low_feed_db / 20.0),
                fast_pow10(params.mb_mid_feed_db / 20.0),
                fast_pow10(params.mb_high_feed_db / 20.0),
            ],
            mb_wet_norm: [1.0; 3],

            auto_gain: sotf_host::auto_gain::AutoGain::new(
                2,
                sr,
                sotf_host::auto_gain::AutoGainParams {
                    enabled: params.autogain_enabled,
                    loudness_type: Default::default(),
                    max_gain_db: params.autogain_max_gain_db,
                    smoothing_ms: params.autogain_smoothing_ms,
                },
            )?,
            mix_smoother: Smoother::new(params.mix, 20.0, sr),
            yaw_smoother: Smoother::new(params.head_yaw_deg, 10.0, sr),
            cached_parameters: Vec::new(),
        };

        plugin
            .auto_gain
            .set_target_lufs(Some(params.autogain_target_lufs))?;
        plugin.update_mb_feed_cache();
        plugin.rebuild_cached_parameters();

        Ok(plugin)
    }

    fn validate_params(params: &CrossfeedPluginParams, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("sample rate must be greater than zero".to_string());
        }
        if params.max_block_frames == 0 || params.max_block_frames > 1_048_576 {
            return Err("max_block_frames must be within 1..=1048576".to_string());
        }
        let finite_range = |name: &str, value: f32, min: f32, max: f32| {
            if !value.is_finite() || !(min..=max).contains(&value) {
                Err(format!("{name} must be finite and within {min}..={max}"))
            } else {
                Ok(())
            }
        };
        finite_range("mix", params.mix, 0.0, 1.0)?;
        finite_range("bauer_fcut_hz", params.bauer_fcut_hz, 400.0, 1000.0)?;
        finite_range("bauer_feed_db", params.bauer_feed_db, 0.0, 15.0)?;
        finite_range("meier_level", params.meier_level, 0.0, 100.0)?;
        finite_range("mb_low_freq_hz", params.mb_low_freq_hz, 50.0, 500.0)?;
        finite_range(
            "mb_mid_high_freq_hz",
            params.mb_mid_high_freq_hz,
            2000.0,
            15000.0,
        )?;
        finite_range(
            "mb_low_feed_db",
            params.mb_low_feed_db,
            MB_FEED_OFF_DB,
            15.0,
        )?;
        finite_range(
            "mb_mid_feed_db",
            params.mb_mid_feed_db,
            MB_FEED_OFF_DB,
            15.0,
        )?;
        finite_range(
            "mb_high_feed_db",
            params.mb_high_feed_db,
            MB_FEED_OFF_DB,
            15.0,
        )?;
        finite_range("itd_delay_ms", params.itd_delay_ms, 0.0, 1.0)?;
        finite_range("head_yaw_deg", params.head_yaw_deg, -90.0, 90.0)?;
        finite_range(
            "autogain_target_lufs",
            params.autogain_target_lufs,
            -40.0,
            -12.0,
        )?;
        finite_range(
            "autogain_max_gain_db",
            params.autogain_max_gain_db,
            0.0,
            24.0,
        )?;
        finite_range(
            "autogain_smoothing_ms",
            params.autogain_smoothing_ms,
            10.0,
            5000.0,
        )?;
        if params.mb_low_freq_hz >= params.mb_mid_high_freq_hz {
            return Err("multiband crossover frequencies must be strictly ascending".to_string());
        }
        let nyquist = sample_rate as f32 * 0.5;
        for (name, frequency) in [
            ("bauer_fcut_hz", params.bauer_fcut_hz),
            ("mb_low_freq_hz", params.mb_low_freq_hz),
            ("mb_mid_high_freq_hz", params.mb_mid_high_freq_hz),
        ] {
            if frequency >= nyquist {
                return Err(format!("{name} must be below Nyquist ({nyquist} Hz)"));
            }
        }
        Ok(())
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
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
    fn set_param_value_on(params: &mut CrossfeedPluginParams, index: usize, value: f64) {
        match index {
            0 => {
                params.mode = match value as usize {
                    0 => CrossfeedMode::Off,
                    1 => CrossfeedMode::Bauer,
                    2 => CrossfeedMode::Meier,
                    3 => CrossfeedMode::Mb,
                    _ => CrossfeedMode::Off,
                };
            }
            1 => {
                params.preset = match value as usize {
                    0 => CrossfeedPreset::Default,
                    1 => CrossfeedPreset::Cmoy,
                    2 => CrossfeedPreset::Meier,
                    3 => CrossfeedPreset::Mb,
                    4 => CrossfeedPreset::Off,
                    _ => CrossfeedPreset::Default,
                };
            }
            2 => params.enabled = value > 0.5,
            3 => params.mix = value as f32,
            4 => params.bauer_fcut_hz = value as f32,
            5 => params.bauer_feed_db = value as f32,
            6 => params.meier_level = value as f32,
            7 => params.mb_low_freq_hz = value as f32,
            8 => params.mb_mid_high_freq_hz = value as f32,
            9 => params.mb_low_feed_db = value as f32,
            10 => params.mb_mid_feed_db = value as f32,
            11 => params.mb_high_feed_db = value as f32,
            12 => params.itd_delay_ms = value as f32,
            13 => params.autogain_enabled = value > 0.5,
            14 => params.autogain_target_lufs = value as f32,
            15 => params.autogain_max_gain_db = value as f32,
            16 => params.autogain_smoothing_ms = value as f32,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
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

    pub(super) fn update_mb_feed_cache(&mut self) {
        let feed_linear = |db: f32| {
            if db <= MB_FEED_OFF_DB {
                0.0
            } else {
                fast_pow10(db / 20.0)
            }
        };
        self.mb_feed_linear = [
            feed_linear(self.params.mb_low_feed_db),
            feed_linear(self.params.mb_mid_feed_db),
            feed_linear(self.params.mb_high_feed_db),
        ];
        self.mb_wet_norm = self
            .mb_feed_linear
            .map(|feed| 1.0 / (1.0 + feed * feed).sqrt());
    }

    pub(super) fn update_bauer_filter(&mut self) {
        let sr = self.sample_rate as f64;
        self.bauer_shelf.update_params(
            math_audio_iir_fir::BiquadFilterType::Lowshelf,
            self.params.bauer_fcut_hz as f64,
            sr,
            0.707,
            -(self.params.bauer_feed_db as f64),
        );
        self.bauer_coefficients = self.bauer_shelf.coefficients();
        self.bauer_transition_target = self.bauer_coefficients;
        self.bauer_transition_remaining = 0;
    }

    fn transition_bauer_filter(&mut self) {
        self.bauer_transition_target = Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Lowshelf,
            self.params.bauer_fcut_hz as f64,
            self.sample_rate as f64,
            0.707,
            -(self.params.bauer_feed_db as f64),
        )
        .coefficients();
        self.bauer_transition_remaining = BAUER_FILTER_TRANSITION_SAMPLES;
    }

    #[inline(always)]
    fn process_bauer_filter(&mut self, input: f64) -> f64 {
        if self.bauer_transition_remaining == 0 {
            return self.bauer_shelf.process(input);
        }
        let progress = (BAUER_FILTER_TRANSITION_SAMPLES - self.bauer_transition_remaining + 1)
            as f64
            / BAUER_FILTER_TRANSITION_SAMPLES as f64;
        let coefficients = self
            .bauer_coefficients
            .lerp(&self.bauer_transition_target, progress);
        let output = self
            .bauer_shelf
            .process_with_coefficients(input, &coefficients);
        self.bauer_coefficients = coefficients;
        self.bauer_transition_remaining -= 1;
        if self.bauer_transition_remaining == 0 {
            self.bauer_coefficients = self.bauer_transition_target;
            // Keep Biquad metadata and coefficient caches synchronized while
            // preserving the state updated by process_with_coefficients.
            self.bauer_shelf.update_params(
                math_audio_iir_fir::BiquadFilterType::Lowshelf,
                self.params.bauer_fcut_hz as f64,
                self.sample_rate as f64,
                0.707,
                -(self.params.bauer_feed_db as f64),
            );
        }
        output
    }

    pub(super) fn update_meier_filters(&mut self) {
        let sr = self.sample_rate as f64;
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
    }

    pub(super) fn update_mb_filters(&mut self) {
        // Reinitializing the bank discards its delay history and clicks on
        // automation. The crossover API updates coefficients in place.
        self.mb_low_l.set_frequency(self.params.mb_low_freq_hz);
        self.mb_high_l
            .set_frequency(self.params.mb_mid_high_freq_hz);
        self.mb_low_r.set_frequency(self.params.mb_low_freq_hz);
        self.mb_high_r
            .set_frequency(self.params.mb_mid_high_freq_hz);
    }

    pub(super) fn update_filters(&mut self) {
        self.update_bauer_filter();
        self.update_meier_filters();
        self.update_mb_filters();
    }

    /// Advance head-yaw smoothing and update both fractional ITD paths for one
    /// audio sample.  DelayLine::set_delay does not resize when the sample rate
    /// is unchanged (the normal realtime case), so this keeps automation
    /// allocation-free while making the delay trajectory independent of the
    /// host callback partition.
    #[inline(always)]
    fn advance_itd(&mut self) {
        let smoothed_yaw = self.yaw_smoother.advance();
        let (itd_l, itd_r) = compute_differential_itd_ms(smoothed_yaw, self.params.itd_delay_ms);
        self.itd_delay_l.set_delay(itd_l, self.sample_rate);
        self.itd_delay_r.set_delay(itd_r, self.sample_rate);
    }

    #[inline(always)]
    pub(super) fn process_bauer(&mut self, nf: usize) {
        for i in 0..nf {
            let x_l = self.dry_l[i];
            let x_r = self.dry_r[i];
            // Low-shelf cut on the difference signal: reduces stereo width at low frequencies
            let diff = x_l - x_r;
            let diff_f = self.process_bauer_filter(diff as f64) as f32;
            // Crossfeed is derived from the part of the difference signal that was removed
            let mut cross_r = (diff_f - diff) * 0.5;
            let mut cross_l = (diff - diff_f) * 0.5;
            // Apply ITD delay to the crossfeed path
            self.advance_itd();
            cross_r = self.itd_delay_r.process(cross_r);
            cross_l = self.itd_delay_l.process(cross_l);
            self.wet_l[i] = x_l + cross_r;
            self.wet_r[i] = x_r + cross_l;
        }
    }

    #[inline(always)]
    pub(super) fn process_meier(&mut self, nf: usize) {
        let feed = self.params.meier_level / 100.0;
        for i in 0..nf {
            let mut cross_r =
                self.meier_allpass_r
                    .process(self.meier_lpf_r.process(self.dry_r[i] as f64)) as f32;
            let mut cross_l =
                self.meier_allpass_l
                    .process(self.meier_lpf_l.process(self.dry_l[i] as f64)) as f32;
            self.advance_itd();
            cross_r = self.itd_delay_r.process(cross_r);
            cross_l = self.itd_delay_l.process(cross_l);
            self.wet_l[i] = self.dry_l[i] + feed * cross_r;
            self.wet_r[i] = self.dry_r[i] + feed * cross_l;
        }
    }

    #[inline(always)]
    pub(super) fn process_mb(&mut self, nf: usize) {
        let [fl, fm, fh] = self.mb_feed_linear;
        let [norm_l, norm_m, norm_h] = self.mb_wet_norm;

        for i in 0..nf {
            let (low_l, carry_l) = self.mb_low_l.process(self.dry_l[i], 0);
            let (mid_l, high_l) = self.mb_high_l.process(carry_l, 0);
            let (low_r, carry_r) = self.mb_low_r.process(self.dry_r[i], 0);
            let (mid_r, high_r) = self.mb_high_r.process(carry_r, 0);

            // Compute crossfeed signal per band
            let mut cross_l = norm_l * fl * low_l + norm_m * fm * mid_l + norm_h * fh * high_l;
            let mut cross_r = norm_l * fl * low_r + norm_m * fm * mid_r + norm_h * fh * high_r;

            // Apply ITD delay to the crossfeed path
            self.advance_itd();
            cross_l = self.itd_delay_l.process(cross_l);
            cross_r = self.itd_delay_r.process(cross_r);

            // Mix crossfeed from opposite channel with headroom normalization.
            self.wet_l[i] = norm_l * low_l + norm_m * mid_l + norm_h * high_l + cross_r;
            self.wet_r[i] = norm_l * low_r + norm_m * mid_r + norm_h * high_r + cross_l;
        }
    }
}

impl ParametricInPlacePlugin for CrossfeedPlugin {
    fn info(&self) -> PluginInfo {
        let mode_str = match self.params.mode {
            CrossfeedMode::Off => "Off",
            CrossfeedMode::Bauer => "Bauer",
            CrossfeedMode::Meier => "Meier",
            CrossfeedMode::Mb => "Multiband",
        };
        PluginInfo::new("Crossfeed", env!("CARGO_PKG_VERSION"), "SotF")
            .with_description(format!("Headphone crossfeed ({})", mode_str))
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Iir
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        if self.params.autogain_enabled {
            return PluginCompileMetadata::boundary(PluginCostClass::Iir, 0);
        }
        PluginCompileMetadata::linear_transform(PluginCostClass::Iir, None, 0, true, true, false)
    }

    fn channels(&self) -> usize {
        2
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        for param in &self.cached_parameters {
            if param.id.as_str() == "head_yaw_deg" {
                values.insert(
                    ParameterId::from("head_yaw_deg"),
                    ParameterValue::Float(self.params.head_yaw_deg),
                );
            } else if let Some(v) =
                param_bridge::get_parameter(CF, &param.id, |i| self.param_value(i))
            {
                values.insert(param.id.clone(), v);
            }
        }
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        let mut candidate = self.params.clone();
        let mut bauer_filter_dirty = false;
        let mut mb_filters_dirty = false;
        let mut mb_feed_dirty = false;
        let mut mix_dirty = false;
        let mut autogain_dirty = false;
        let mut mode_dirty = false;
        let mut yaw_dirty = false;

        // Presets are complete states, not display-only selectors. Apply one first so any
        // additional values in the same host update intentionally override the preset.
        if let Some((_, value)) = values
            .iter()
            .find(|(id, _)| id.as_str() == "crossfeed_preset")
        {
            let index = value
                .as_int()
                .ok_or_else(|| "crossfeed_preset must be an integer".to_string())?;
            let preset = match index {
                0 => CrossfeedPreset::Default,
                1 => CrossfeedPreset::Cmoy,
                2 => CrossfeedPreset::Meier,
                3 => CrossfeedPreset::Mb,
                4 => CrossfeedPreset::Off,
                _ => return Err(format!("invalid crossfeed preset index: {index}")),
            };
            let max_block_frames = candidate.max_block_frames;
            candidate = CrossfeedPluginParams::from_preset(preset);
            candidate.max_block_frames = max_block_frames;
            mode_dirty = true;
            bauer_filter_dirty = true;
            mb_filters_dirty = true;
            mb_feed_dirty = true;
            mix_dirty = true;
            autogain_dirty = true;
            yaw_dirty = true;
        }

        for (id, value) in values {
            if id.as_str() == "crossfeed_preset" {
                continue;
            }
            if id.as_str() == "head_yaw_deg" {
                let v = value
                    .as_float()
                    .ok_or_else(|| "head_yaw_deg must be a float".to_string())?;
                if !v.is_finite() {
                    return Err("head_yaw_deg must be finite".to_string());
                }
                candidate.head_yaw_deg = v.clamp(-90.0, 90.0);
                yaw_dirty = true;
                // Do NOT update delay lines here — process_in_place owns delay line updates
                // via the yaw smoother, preventing the double-discontinuity bug.
            } else {
                let idx = param_bridge::set_parameter(CF, &id, &value, |i, v| {
                    Self::set_param_value_on(&mut candidate, i, v)
                })?;
                match idx {
                    0 => mode_dirty = true,
                    3 => mix_dirty = true,
                    4 | 5 => bauer_filter_dirty = true,
                    7 | 8 => mb_filters_dirty = true,
                    9..=11 => mb_feed_dirty = true,
                    13..=16 => autogain_dirty = true,
                    _ => {}
                }
            }
        }

        Self::validate_params(&candidate, self.sample_rate)?;
        self.params = candidate;
        if yaw_dirty {
            self.yaw_smoother.set_target(self.params.head_yaw_deg);
        }
        if mode_dirty {
            self.reset();
        }
        if mix_dirty {
            self.mix_smoother.set_target(self.params.mix);
        }
        if bauer_filter_dirty {
            self.transition_bauer_filter();
        }
        if mb_filters_dirty {
            self.update_mb_filters();
        }
        if mb_feed_dirty {
            self.update_mb_feed_cache();
        }
        if autogain_dirty {
            self.auto_gain.set_enabled(self.params.autogain_enabled);
            self.auto_gain
                .set_max_gain_db(self.params.autogain_max_gain_db);
            self.auto_gain
                .set_smoothing_ms(self.params.autogain_smoothing_ms);
            self.auto_gain
                .set_target_lufs(Some(self.params.autogain_target_lufs))?;
        }

        Ok(())
    }

    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        if id.as_str() == "head_yaw_deg" {
            let yaw = value
                .as_float()
                .ok_or_else(|| "head_yaw_deg must be a float".to_string())?;
            if !yaw.is_finite() {
                return Err("head_yaw_deg must be finite".to_string());
            }
            return Ok(());
        }
        if let Some(param) = self.cached_parameters.iter().find(|p| &p.id == id) {
            param.validate(value).map_err(|e| format!("{}: {}", id, e))
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        self.parametric_validate_parameter(&id, &value)?;
        let mut candidate = self.params.clone();
        let mut bauer_filter_dirty = false;
        let mut mb_filters_dirty = false;
        let mut mb_feed_dirty = false;
        let mut mix_dirty = false;
        let mut autogain_dirty = false;
        let mut mode_dirty = false;
        let mut yaw_dirty = false;

        if id.as_str() == "crossfeed_preset" {
            let index = value
                .as_int()
                .ok_or_else(|| "crossfeed_preset must be an integer".to_string())?;
            let preset = match index {
                0 => CrossfeedPreset::Default,
                1 => CrossfeedPreset::Cmoy,
                2 => CrossfeedPreset::Meier,
                3 => CrossfeedPreset::Mb,
                4 => CrossfeedPreset::Off,
                _ => return Err(format!("invalid crossfeed preset index: {index}")),
            };
            let max_block_frames = candidate.max_block_frames;
            candidate = CrossfeedPluginParams::from_preset(preset);
            candidate.max_block_frames = max_block_frames;
            bauer_filter_dirty = true;
            mb_filters_dirty = true;
            mb_feed_dirty = true;
            mix_dirty = true;
            autogain_dirty = true;
            mode_dirty = true;
            yaw_dirty = true;
        } else if id.as_str() == "head_yaw_deg" {
            let yaw = value
                .as_float()
                .ok_or_else(|| "head_yaw_deg must be a float".to_string())?;
            candidate.head_yaw_deg = yaw.clamp(-90.0, 90.0);
            yaw_dirty = true;
        } else {
            let index = param_bridge::set_parameter(CF, &id, &value, |i, v| {
                Self::set_param_value_on(&mut candidate, i, v)
            })?;
            match index {
                0 => mode_dirty = true,
                3 => mix_dirty = true,
                4 | 5 => bauer_filter_dirty = true,
                7 | 8 => mb_filters_dirty = true,
                9..=11 => mb_feed_dirty = true,
                13..=16 => autogain_dirty = true,
                _ => {}
            }
        }

        Self::validate_params(&candidate, self.sample_rate)?;
        self.params = candidate;
        if yaw_dirty {
            self.yaw_smoother.set_target(self.params.head_yaw_deg);
        }
        if mode_dirty {
            self.reset();
        }
        if mix_dirty {
            self.mix_smoother.set_target(self.params.mix);
        }
        if bauer_filter_dirty {
            self.transition_bauer_filter();
        }
        if mb_filters_dirty {
            self.update_mb_filters();
        }
        if mb_feed_dirty {
            self.update_mb_feed_cache();
        }
        if autogain_dirty {
            self.auto_gain.set_enabled(self.params.autogain_enabled);
            self.auto_gain
                .set_max_gain_db(self.params.autogain_max_gain_db);
            self.auto_gain
                .set_smoothing_ms(self.params.autogain_smoothing_ms);
            self.auto_gain
                .set_target_lufs(Some(self.params.autogain_target_lufs))?;
        }
        Ok(())
    }

    fn initialize(&mut self, sr: u32) -> PluginResult<()> {
        Self::validate_params(&self.params, sr)?;
        self.sample_rate = sr;
        self.initialized = true;
        self.update_filters();
        self.mix_smoother = Smoother::new(self.params.mix, 20.0, sr);
        self.yaw_smoother = Smoother::new(self.params.head_yaw_deg, 10.0, sr);
        let (itd_l, itd_r) =
            compute_differential_itd_ms(self.params.head_yaw_deg, self.params.itd_delay_ms);
        self.itd_delay_l = DelayLine::new(itd_l, sr);
        self.itd_delay_r = DelayLine::new(itd_r, sr);
        self.auto_gain
            .set_sample_rate(sr)
            .map_err(|e| e.to_string())?;
        self.auto_gain.set_enabled(self.params.autogain_enabled);
        Ok(())
    }

    fn reset(&mut self) {
        self.mix_smoother.reset(self.params.mix);
        self.yaw_smoother.reset(self.params.head_yaw_deg);
        self.itd_delay_l.reset();
        self.itd_delay_r.reset();
        self.bauer_shelf.reset();
        self.bauer_coefficients = self.bauer_shelf.coefficients();
        self.bauer_transition_target = self.bauer_coefficients;
        self.bauer_transition_remaining = 0;
        self.meier_lpf_l.reset();
        self.meier_lpf_r.reset();
        self.meier_allpass_l.reset();
        self.meier_allpass_r.reset();
        self.mb_low_l.reset();
        self.mb_high_l.reset();
        self.mb_low_r.reset();
        self.mb_high_r.reset();
        self.auto_gain.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        if !self.initialized {
            return Err("crossfeed plugin must be initialized before processing".to_string());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "callback sample rate {} does not match initialized sample rate {}",
                context.sample_rate, self.sample_rate
            ));
        }
        let expected_samples = context
            .num_frames
            .checked_mul(2)
            .ok_or_else(|| "stereo buffer length overflow".to_string())?;
        if buffer.len() != expected_samples {
            return Err(format!(
                "expected exactly {expected_samples} stereo samples for {} frames, got {}",
                context.num_frames,
                buffer.len()
            ));
        }
        if !self.params.enabled || self.params.mode == CrossfeedMode::Off {
            // Disabled/Off is an explicit reset-on-bypass contract.  Clear all
            // delay/filter/auto-gain history so re-entry cannot replay stale
            // state captured before the transition.
            self.reset();
            return Ok(context.num_frames);
        }
        enable_ftz_daz();
        let nf = context.num_frames;
        if nf > self.dry_l.len() {
            return Err(format!(
                "Block size {} exceeds pre-allocated capacity {}",
                nf,
                self.dry_l.len()
            ));
        }

        for sample in buffer.iter_mut() {
            if !sample.is_finite() {
                *sample = 0.0;
            }
        }

        if self.params.autogain_enabled {
            self.auto_gain.measure_input(buffer)?;
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

        if self.params.autogain_enabled {
            self.auto_gain.measure_output(buffer)?;
            self.auto_gain.apply_compensation(buffer, nf);
        }

        Ok(nf)
    }
}
