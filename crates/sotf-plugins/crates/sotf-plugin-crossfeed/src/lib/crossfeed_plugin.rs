use super::crossfeed_plugin_params::CrossfeedPluginParams;
use super::delay_line::DelayLine;
use super::misc::compute_differential_itd_ms;
use super::types::CrossfeedMode;
use super::types::CrossfeedPreset;
use crate::params::PARAMS as CF;
use math_audio_dsp::fast_math::fast_pow10;
use math_audio_iir_fir::Biquad;
use sotf_host::lr4_crossover::MultibandLr4Crossover;
use sotf_host::param_bridge;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{deinterleave_stereo, enable_ftz_daz, interleave_stereo};
use sotf_host::smoothing::Smoother;

pub struct CrossfeedPlugin {
    pub(super) sample_rate: u32,
    pub(super) params: CrossfeedPluginParams,

    // Bauer: low-shelf cut on the difference signal (L-R)
    pub(super) bauer_shelf: Biquad,

    pub(super) meier_lpf_l: Biquad,
    pub(super) meier_lpf_r: Biquad,
    pub(super) meier_allpass_l: Biquad,
    pub(super) meier_allpass_r: Biquad,

    // Multiband: true LR4 crossover (3-band: low/mid/high)
    pub(super) mb_crossover_l: MultibandLr4Crossover<f32>,
    pub(super) mb_crossover_r: MultibandLr4Crossover<f32>,

    // ITD delay lines (one per crossfeed path)
    pub(super) itd_delay_l: DelayLine,
    pub(super) itd_delay_r: DelayLine,

    // Pre-allocated flat buffers for deinterleaved processing
    pub(super) dry_l: Vec<f32>,
    pub(super) dry_r: Vec<f32>,
    pub(super) wet_l: Vec<f32>,
    pub(super) wet_r: Vec<f32>,

    // Multiband specific buffers (3 bands per channel)
    pub(super) mb_bands_l: [Vec<f32>; 3],
    pub(super) mb_bands_r: [Vec<f32>; 3],
    pub(super) mb_feed_linear: [f32; 3],
    pub(super) mb_wet_norm: f32,

    // Auto gain helper
    pub(super) auto_gain: Option<sotf_host::auto_gain::AutoGain>,

    // Smoothing
    pub(super) mix_smoother: Smoother,
    pub(super) yaw_smoother: Smoother,
    pub(super) cached_parameters: Vec<Parameter>,
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
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
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

    pub(super) fn update_filters(&mut self) {
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
    pub(super) fn process_bauer(&mut self, nf: usize) {
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
    pub(super) fn process_meier(&mut self, nf: usize) {
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
    pub(super) fn process_mb(&mut self, nf: usize) {
        let [fl, fm, fh] = self.mb_feed_linear;
        let wet_norm = self.mb_wet_norm;
        let has_itd = self.params.itd_delay_ms > 0.0;

        // Band buffers are pre-allocated in initialize() to a safe capacity.
        // process_in_place already rejects blocks that exceed this capacity.

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
        let cap = 16384;
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
            return Err(format!(
                "Block size {} exceeds pre-allocated capacity {}",
                nf,
                self.dry_l.len()
            ));
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
