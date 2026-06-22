use super::allpass_diffuser::AllpassDiffuser;
use super::consts::DIALOGUE_ANALYSIS_WINDOW_MS;
use super::consts::DIALOGUE_ATTACK_MS;
use super::consts::DIALOGUE_HOLD_MS;
use super::consts::DIALOGUE_LEVEL_FAST_MS;
use super::consts::DIALOGUE_LEVEL_SLOW_MS;
use super::consts::DIALOGUE_MIN_CENTER_LEVEL;
use super::consts::DIALOGUE_MIN_MODULATION;
use super::consts::DIALOGUE_MOD_SMOOTH_MS;
use super::consts::DIALOGUE_RELEASE_MS;
use super::consts::FINAL_LIMITER_RELEASE_MS;
use super::consts::FINAL_OUTPUT_CEILING;
use super::consts::LFE_CROSSOVER_HZ;
use super::consts::MAX_PRE_DELAY_MS;
use super::early_reflections;
use super::misc::compute_lp_coeff;
use super::misc::create_auto_gain;
use super::misc::ms_to_samples;
use super::misc::signed_rms;
use super::smoothing::smoothing_coeff;
use super::smoothing::smoothing_coeff_for_samples;
use super::types::AaeData;
use crate::early_reflections::EarlyReflections;
use crate::fdn::{FDN_SIZE, Fdn};
use crate::params::{AaePluginParams, build_parameters};
use sotf_host::auto_gain::{AutoGainLoudnessType, AutoGainParams};
use sotf_host::multichannel_auto_gain::MultichannelAutoGain;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::speaker_config::{
    SourcePosition, SpeakerConfig, compute_vbap_matrix, get_speaker_config, normalize_gains_l2,
};
use std::any::Any;
use std::sync::Arc;

pub struct AaePlugin {
    pub(super) sample_rate: u32,
    pub(super) params: AaePluginParams,
    pub(super) speaker_config: &'static SpeakerConfig,
    pub(super) num_output_channels: usize,

    // DSP components
    pub(super) pre_delay: super::delay_line::DelayLine,
    pub(super) pre_delay_samples: usize,
    pub(super) diffusion_allpass: [AllpassDiffuser; 2],
    pub(super) early_reflections: EarlyReflections,
    pub(super) fdn: Fdn,

    // LFE extraction: one-pole LP filter state
    pub(super) lfe_filter_state: f32,
    pub(super) lfe_filter_coeff: f32,

    // Per-tap VBAP gains for early reflections (flattened row-major):
    // [tap_index * out_ch + speaker_index]
    pub(super) er_gains: Vec<f32>,

    // FDN output → speaker routing (flattened row-major):
    // [fdn_line * out_ch + speaker_index]
    pub(super) fdn_gains: Vec<f32>,

    // Direct signal panning gains (stereo L/R to front speakers)
    pub(super) direct_gains_l: Vec<f32>,
    pub(super) direct_gains_r: Vec<f32>,

    // Pre-allocated scratch buffer for ER tap outputs (avoids hot-path allocation)
    pub(super) er_tap_buffer: Vec<f32>,

    // Content-aware wet ducking state
    pub(super) dialogue_duck_gain: f32,
    pub(super) dialogue_attack_coeff: f32,
    pub(super) dialogue_release_coeff: f32,
    pub(super) dialogue_level_fast: f32,
    pub(super) dialogue_level_slow: f32,
    pub(super) dialogue_modulation: f32,
    pub(super) dialogue_level_fast_coeff: f32,
    pub(super) dialogue_level_slow_coeff: f32,
    pub(super) dialogue_mod_coeff: f32,
    pub(super) dialogue_hold_samples: usize,
    pub(super) dialogue_hold_remaining: usize,
    pub(super) dialogue_window_center_sum: f32,
    pub(super) dialogue_window_side_sum: f32,
    pub(super) dialogue_window_fill: usize,
    pub(super) dialogue_window_samples: usize,
    pub(super) dialogue_voiced_center: bool,

    // Linked emergency limiter state for the rendered multichannel output.
    pub(super) final_limiter_gain: f32,
    pub(super) final_limiter_release_coeff: f32,

    // Parameter smoothers
    pub(super) dry_smoother: Smoother,
    pub(super) er_smoother: Smoother,
    pub(super) late_smoother: Smoother,
    pub(super) lfe_smoother: Smoother,

    // Auto-gain compensation. The meter is stereo: input L/R vs a stereo fold-down
    // of the multichannel render, then the resulting gain is applied to all outputs.
    pub(super) auto_gain: Option<MultichannelAutoGain>,

    // Cached parameter list
    pub(super) cached_parameters: Vec<Parameter>,
}

impl AaePlugin {
    /// Create a new AAE plugin from parameters.
    pub fn from_params(params: AaePluginParams) -> Self {
        let speaker_config = get_speaker_config(&params.speaker_config)
            .unwrap_or_else(|| get_speaker_config("5.1").unwrap());
        let num_output_channels = speaker_config.total_channels;

        // Pre-compute at a default sample rate; re-done in initialize()
        let sr = 48000u32;
        let pre_delay_samples = (params.pre_delay_ms * 0.001 * sr as f32).round() as usize;

        let dialogue_window_samples = ms_to_samples(DIALOGUE_ANALYSIS_WINDOW_MS, sr);

        let mut plugin = Self {
            sample_rate: sr,
            speaker_config,
            num_output_channels,
            pre_delay: super::delay_line::DelayLine::new(
                (MAX_PRE_DELAY_MS * 0.001 * sr as f32) as usize + 16,
            ),
            pre_delay_samples,
            diffusion_allpass: [
                AllpassDiffuser::new(142, params.input_diffusion * 0.7),
                AllpassDiffuser::new(379, params.input_diffusion * 0.6),
            ],
            early_reflections: EarlyReflections::new(
                sr,
                params.room_preset_enum(),
                params.er_mod_depth,
            ),
            fdn: Fdn::new(
                sr,
                params.room_size,
                params.rt60,
                params.bass_ratio,
                params.treble_ratio,
                params.mod_depth,
                params.safety_limit_db,
            ),
            lfe_filter_state: 0.0,
            lfe_filter_coeff: compute_lp_coeff(LFE_CROSSOVER_HZ, sr as f32),
            er_gains: vec![0.0; early_reflections::MAX_TAPS * num_output_channels],
            fdn_gains: vec![0.0; FDN_SIZE * num_output_channels],
            direct_gains_l: vec![0.0; num_output_channels],
            direct_gains_r: vec![0.0; num_output_channels],
            er_tap_buffer: vec![0.0; early_reflections::MAX_TAPS],
            dialogue_duck_gain: 1.0,
            dialogue_attack_coeff: smoothing_coeff(DIALOGUE_ATTACK_MS, sr as f32),
            dialogue_release_coeff: smoothing_coeff(DIALOGUE_RELEASE_MS, sr as f32),
            dialogue_level_fast: 0.0,
            dialogue_level_slow: 0.0,
            dialogue_modulation: 0.0,
            dialogue_level_fast_coeff: smoothing_coeff_for_samples(
                DIALOGUE_LEVEL_FAST_MS,
                sr as f32,
                dialogue_window_samples,
            ),
            dialogue_level_slow_coeff: smoothing_coeff_for_samples(
                DIALOGUE_LEVEL_SLOW_MS,
                sr as f32,
                dialogue_window_samples,
            ),
            dialogue_mod_coeff: smoothing_coeff_for_samples(
                DIALOGUE_MOD_SMOOTH_MS,
                sr as f32,
                dialogue_window_samples,
            ),
            dialogue_hold_samples: ms_to_samples(DIALOGUE_HOLD_MS, sr),
            dialogue_hold_remaining: 0,
            dialogue_window_center_sum: 0.0,
            dialogue_window_side_sum: 0.0,
            dialogue_window_fill: 0,
            dialogue_window_samples,
            dialogue_voiced_center: false,
            final_limiter_gain: 1.0,
            final_limiter_release_coeff: smoothing_coeff(FINAL_LIMITER_RELEASE_MS, sr as f32),
            dry_smoother: Smoother::new(params.dry_level, 5.0, sr),
            er_smoother: Smoother::new(params.er_level, 5.0, sr),
            late_smoother: Smoother::new(params.late_level, 5.0, sr),
            lfe_smoother: Smoother::new(params.lfe_level, 5.0, sr),
            auto_gain: create_auto_gain(&params, sr),
            cached_parameters: Vec::new(),
            params,
        };

        plugin.precompute_gains();
        plugin.cached_parameters = build_parameters(&plugin.params);
        plugin
    }

    /// Pre-compute VBAP panning gains for all routing paths.
    pub(super) fn precompute_gains(&mut self) {
        let cfg = self.speaker_config;
        let n_ch = self.num_output_channels;

        // Direct signal: stereo L at +30° az, R at -30° az.
        let direct_sources = [
            SourcePosition::new(30.0, 0.0),
            SourcePosition::new(-30.0, 0.0),
        ];
        let mut direct = compute_vbap_matrix(cfg, &direct_sources, None).into_iter();
        let mut left = direct
            .next()
            .expect("compute_vbap_matrix returns one row per source");
        let mut right = direct
            .next()
            .expect("compute_vbap_matrix returns one row per source");
        normalize_gains_l2(&mut left);
        normalize_gains_l2(&mut right);
        self.direct_gains_l = left;
        self.direct_gains_r = right;

        // Early reflections: per-tap VBAP gains, padded to MAX_TAPS rows.
        let num_taps = self.early_reflections.num_taps();
        let er_sources: Vec<SourcePosition> = (0..num_taps)
            .map(|idx| {
                let tap = self.early_reflections.tap_info(idx).unwrap();
                SourcePosition::new(tap.azimuth, tap.elevation)
            })
            .collect();
        let mut er = compute_vbap_matrix(cfg, &er_sources, Some(0.5));
        for row in &mut er {
            normalize_gains_l2(row);
        }
        er.resize(super::early_reflections::MAX_TAPS, vec![0.0; n_ch]);
        self.er_gains = er.into_iter().flat_map(|row| row.into_iter()).collect();

        // FDN outputs: distributed across speakers with envelopment bias.
        // Lines 0-2: more front, lines 3-7: more surround/rear. Line 7 is overhead.
        let envelopment = self.params.envelopment;
        let height_amount = self.params.height_amount;
        let fdn_sources = [
            SourcePosition::new(30.0, 0.0),
            SourcePosition::new(-30.0, 0.0),
            SourcePosition::new(0.0, 0.0),
            SourcePosition::new(110.0, 0.0),
            SourcePosition::new(-110.0, 0.0),
            SourcePosition::new(150.0, 0.0),
            SourcePosition::new(-150.0, 0.0),
            SourcePosition::new(0.0, 45.0),
        ];
        debug_assert_eq!(fdn_sources.len(), FDN_SIZE);
        let mut fdn = compute_vbap_matrix(cfg, &fdn_sources, Some(0.7));
        for (line_idx, row) in fdn.iter_mut().enumerate() {
            for sp in cfg.speakers {
                if sp.is_lfe || sp.channel >= n_ch {
                    continue;
                }
                let g = &mut row[sp.channel];
                if line_idx >= 3 {
                    let is_rear = sp.azimuth.abs() > 90.0;
                    *g *= if is_rear {
                        0.5 + 0.5 * envelopment
                    } else {
                        1.0 - 0.5 * envelopment
                    };
                }
                if sp.elevation > 10.0 {
                    *g *= height_amount;
                }
            }
            normalize_gains_l2(row);
        }
        self.fdn_gains = fdn.into_iter().flat_map(|row| row.into_iter()).collect();
    }

    pub(super) fn update_cached_parameter(&mut self, id: &ParameterId, value: &ParameterValue) {
        if let Some(param) = self.cached_parameters.iter_mut().find(|p| p.id == *id) {
            param.default_value = value.clone();
        }
    }

    #[inline]
    pub(super) fn dialogue_duck_for_frame(&mut self, l: f32, r: f32) -> f32 {
        if !self.params.content_aware {
            self.dialogue_duck_gain = 1.0;
            return 1.0;
        }

        let center = ((l + r) * 0.5).abs();
        let side = ((l - r) * 0.5).abs();
        self.dialogue_window_center_sum += center;
        self.dialogue_window_side_sum += side;
        self.dialogue_window_fill += 1;

        if self.dialogue_window_fill >= self.dialogue_window_samples {
            let inv_window = 1.0 / self.dialogue_window_fill as f32;
            let window_center = self.dialogue_window_center_sum * inv_window;
            let window_side = self.dialogue_window_side_sum * inv_window;
            let centeredness = window_center / (window_center + window_side + 1e-6);

            self.dialogue_level_fast = window_center
                + self.dialogue_level_fast_coeff * (self.dialogue_level_fast - window_center);
            self.dialogue_level_slow = window_center
                + self.dialogue_level_slow_coeff * (self.dialogue_level_slow - window_center);
            let modulation = (self.dialogue_level_fast - self.dialogue_level_slow).max(0.0)
                / (self.dialogue_level_slow + 1e-4);
            self.dialogue_modulation =
                modulation + self.dialogue_mod_coeff * (self.dialogue_modulation - modulation);

            self.dialogue_voiced_center =
                self.dialogue_level_slow > DIALOGUE_MIN_CENTER_LEVEL && centeredness > 0.75;
            if self.dialogue_voiced_center && self.dialogue_modulation > DIALOGUE_MIN_MODULATION {
                self.dialogue_hold_remaining = self.dialogue_hold_samples;
            }

            self.dialogue_window_center_sum = 0.0;
            self.dialogue_window_side_sum = 0.0;
            self.dialogue_window_fill = 0;
        }

        let speech_like = self.dialogue_voiced_center && self.dialogue_hold_remaining > 0;
        self.dialogue_hold_remaining = self.dialogue_hold_remaining.saturating_sub(1);
        let target = if speech_like {
            10.0_f32.powf(-self.params.dialogue_attenuation_db / 20.0)
        } else {
            1.0
        };
        let coeff = if target < self.dialogue_duck_gain {
            self.dialogue_attack_coeff
        } else {
            self.dialogue_release_coeff
        };
        self.dialogue_duck_gain = target + coeff * (self.dialogue_duck_gain - target);
        self.dialogue_duck_gain
    }

    pub(super) fn ensure_auto_gain(&mut self) -> PluginResult<()> {
        if self.auto_gain.is_none() {
            self.auto_gain = Some(MultichannelAutoGain::new(
                self.sample_rate,
                AutoGainParams {
                    enabled: self.params.auto_gain_enabled,
                    loudness_type: AutoGainLoudnessType::Momentary,
                    max_gain_db: self.params.auto_gain_max_db,
                    smoothing_ms: self.params.auto_gain_smoothing_ms,
                },
            )?);
        }
        Ok(())
    }

    pub(super) fn apply_auto_gain(
        &mut self,
        output: &mut [f32],
        num_frames: usize,
    ) -> PluginResult<()> {
        if !self.params.auto_gain_enabled {
            return Ok(());
        }
        self.ensure_auto_gain()?;
        let speaker_config = self.speaker_config;
        let out_ch = self.num_output_channels;
        let auto_gain = self.auto_gain.as_mut().unwrap();
        auto_gain.measure_and_apply(output, num_frames, out_ch, speaker_config)
    }

    pub(super) fn apply_output_safety_limit(
        &mut self,
        output: &mut [f32],
        num_frames: usize,
        out_ch: usize,
    ) {
        for frame in 0..num_frames {
            let start = frame * out_ch;
            let frame_samples = &mut output[start..start + out_ch];
            let peak = frame_samples
                .iter()
                .map(|v| v.abs())
                .fold(0.0_f32, f32::max);

            let target_gain = if peak > FINAL_OUTPUT_CEILING {
                FINAL_OUTPUT_CEILING / peak
            } else {
                1.0
            };

            if target_gain < self.final_limiter_gain {
                self.final_limiter_gain = target_gain;
            } else {
                self.final_limiter_gain = target_gain
                    + self.final_limiter_release_coeff * (self.final_limiter_gain - target_gain);
            }

            // Do not let release smoothing permit an overshoot.
            if peak * self.final_limiter_gain > FINAL_OUTPUT_CEILING {
                self.final_limiter_gain = FINAL_OUTPUT_CEILING / peak;
            }

            if self.final_limiter_gain < 0.999_999 {
                for sample in frame_samples {
                    *sample *= self.final_limiter_gain;
                }
            }
        }
    }
}

impl Plugin for AaePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("AAE", "0.5.1", "SotF")
    }

    fn input_channels(&self) -> usize {
        2
    }

    fn output_channels(&self) -> usize {
        self.num_output_channels
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(
            PluginCostClass::Dynamics,
            None,
            self.latency_samples(),
            true,
        )
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        let id_str = id.as_str();
        match id_str {
            "room_size" => {
                if let Some(v) = value.as_float() {
                    self.params.room_size = v;
                    self.fdn.set_room_size(
                        v,
                        self.params.rt60,
                        self.params.bass_ratio,
                        self.params.treble_ratio,
                    );
                }
            }
            "rt60" => {
                if let Some(v) = value.as_float() {
                    self.params.rt60 = v;
                    self.fdn
                        .set_rt60(v, self.params.bass_ratio, self.params.treble_ratio);
                }
            }
            "bass_ratio" => {
                if let Some(v) = value.as_float() {
                    self.params.bass_ratio = v;
                    self.fdn
                        .set_rt60(self.params.rt60, v, self.params.treble_ratio);
                }
            }
            "treble_ratio" => {
                if let Some(v) = value.as_float() {
                    self.params.treble_ratio = v;
                    self.fdn
                        .set_rt60(self.params.rt60, self.params.bass_ratio, v);
                }
            }
            "pre_delay_ms" => {
                if let Some(v) = value.as_float() {
                    self.params.pre_delay_ms = v;
                    self.pre_delay_samples = (v * 0.001 * self.sample_rate as f32).round() as usize;
                }
            }
            "room_preset" => {
                if let Some(v) = value.as_string() {
                    self.params.room_preset = v.to_string();
                    self.early_reflections
                        .set_preset(self.params.room_preset_enum());
                    self.precompute_gains();
                }
            }
            "dry_level" => {
                if let Some(v) = value.as_float() {
                    self.params.dry_level = v;
                    self.dry_smoother.set_target(v);
                }
            }
            "er_level" => {
                if let Some(v) = value.as_float() {
                    self.params.er_level = v;
                    self.er_smoother.set_target(v);
                }
            }
            "late_level" => {
                if let Some(v) = value.as_float() {
                    self.params.late_level = v;
                    self.late_smoother.set_target(v);
                }
            }
            "lfe_level" => {
                if let Some(v) = value.as_float() {
                    self.params.lfe_level = v;
                    self.lfe_smoother.set_target(v);
                }
            }
            "mod_depth" => {
                if let Some(v) = value.as_float() {
                    self.params.mod_depth = v;
                    self.fdn.set_mod_depth(v);
                }
            }
            "er_mod_depth" => {
                if let Some(v) = value.as_float() {
                    self.params.er_mod_depth = v;
                    self.early_reflections.set_mod_depth(v);
                }
            }
            "input_diffusion" => {
                if let Some(v) = value.as_float() {
                    self.params.input_diffusion = v;
                    self.diffusion_allpass[0].feedback = v * 0.7;
                    self.diffusion_allpass[1].feedback = v * 0.6;
                }
            }
            "speaker_config" => {
                if let Some(v) = value.as_string()
                    && let Some(cfg) = get_speaker_config(v)
                {
                    self.params.speaker_config = v.to_string();
                    self.speaker_config = cfg;
                    self.num_output_channels = cfg.total_channels;
                    self.precompute_gains();
                }
            }
            "envelopment" => {
                if let Some(v) = value.as_float() {
                    self.params.envelopment = v;
                    self.precompute_gains();
                }
            }
            "height_amount" => {
                if let Some(v) = value.as_float() {
                    self.params.height_amount = v;
                    self.precompute_gains();
                }
            }
            "content_aware" => {
                if let Some(v) = value.as_bool() {
                    self.params.content_aware = v;
                }
            }
            "dialogue_attenuation_db" => {
                if let Some(v) = value.as_float() {
                    self.params.dialogue_attenuation_db = v;
                }
            }
            "safety_limit_db" => {
                if let Some(v) = value.as_float() {
                    self.params.safety_limit_db = v;
                    self.fdn.set_safety_limit_db(v);
                }
            }
            "auto_gain_enabled" => {
                if let Some(v) = value.as_bool() {
                    self.params.auto_gain_enabled = v;
                    if v {
                        self.ensure_auto_gain()?;
                        if let Some(auto_gain) = &mut self.auto_gain {
                            auto_gain.set_enabled(true);
                        }
                    } else if let Some(auto_gain) = &mut self.auto_gain {
                        auto_gain.set_enabled(false);
                    }
                }
            }
            "auto_gain_max_db" => {
                if let Some(v) = value.as_float() {
                    self.params.auto_gain_max_db = v;
                    if let Some(auto_gain) = &mut self.auto_gain {
                        auto_gain.set_max_gain_db(v);
                    }
                }
            }
            "auto_gain_smoothing_ms" => {
                if let Some(v) = value.as_float() {
                    self.params.auto_gain_smoothing_ms = v;
                    if let Some(auto_gain) = &mut self.auto_gain {
                        auto_gain.set_smoothing_ms(v);
                    }
                }
            }
            "bypass" => {
                if let Some(v) = value.as_bool() {
                    self.params.bypass = v;
                }
            }
            "solo_early" => {
                if let Some(v) = value.as_bool() {
                    self.params.solo_early = v;
                }
            }
            "solo_late" => {
                if let Some(v) = value.as_bool() {
                    self.params.solo_late = v;
                }
            }
            _ => {}
        }

        self.update_cached_parameter(&id, &value);
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.as_str() {
            "room_size" => Some(ParameterValue::Float(self.params.room_size)),
            "rt60" => Some(ParameterValue::Float(self.params.rt60)),
            "bass_ratio" => Some(ParameterValue::Float(self.params.bass_ratio)),
            "treble_ratio" => Some(ParameterValue::Float(self.params.treble_ratio)),
            "pre_delay_ms" => Some(ParameterValue::Float(self.params.pre_delay_ms)),
            "room_preset" => Some(ParameterValue::String(self.params.room_preset.clone())),
            "dry_level" => Some(ParameterValue::Float(self.params.dry_level)),
            "er_level" => Some(ParameterValue::Float(self.params.er_level)),
            "late_level" => Some(ParameterValue::Float(self.params.late_level)),
            "lfe_level" => Some(ParameterValue::Float(self.params.lfe_level)),
            "mod_depth" => Some(ParameterValue::Float(self.params.mod_depth)),
            "er_mod_depth" => Some(ParameterValue::Float(self.params.er_mod_depth)),
            "input_diffusion" => Some(ParameterValue::Float(self.params.input_diffusion)),
            "speaker_config" => Some(ParameterValue::String(self.params.speaker_config.clone())),
            "envelopment" => Some(ParameterValue::Float(self.params.envelopment)),
            "height_amount" => Some(ParameterValue::Float(self.params.height_amount)),
            "content_aware" => Some(ParameterValue::Bool(self.params.content_aware)),
            "dialogue_attenuation_db" => {
                Some(ParameterValue::Float(self.params.dialogue_attenuation_db))
            }
            "safety_limit_db" => Some(ParameterValue::Float(self.params.safety_limit_db)),
            "auto_gain_enabled" => Some(ParameterValue::Bool(self.params.auto_gain_enabled)),
            "auto_gain_max_db" => Some(ParameterValue::Float(self.params.auto_gain_max_db)),
            "auto_gain_smoothing_ms" => {
                Some(ParameterValue::Float(self.params.auto_gain_smoothing_ms))
            }
            "bypass" => Some(ParameterValue::Bool(self.params.bypass)),
            "solo_early" => Some(ParameterValue::Bool(self.params.solo_early)),
            "solo_late" => Some(ParameterValue::Bool(self.params.solo_late)),
            _ => None,
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        let sr = sample_rate as f32;

        // Rebuild pre-delay
        self.pre_delay =
            super::delay_line::DelayLine::new((MAX_PRE_DELAY_MS * 0.001 * sr) as usize + 16);
        self.pre_delay_samples = (self.params.pre_delay_ms * 0.001 * sr).round() as usize;

        // Rebuild diffusion allpasses (scale delay times by sample rate)
        let diff_delay_1 = (142.0 * sr / 48000.0).round() as usize;
        let diff_delay_2 = (379.0 * sr / 48000.0).round() as usize;
        self.diffusion_allpass = [
            AllpassDiffuser::new(diff_delay_1, self.params.input_diffusion * 0.7),
            AllpassDiffuser::new(diff_delay_2, self.params.input_diffusion * 0.6),
        ];

        // Rebuild early reflections
        self.early_reflections = EarlyReflections::new(
            sample_rate,
            self.params.room_preset_enum(),
            self.params.er_mod_depth,
        );

        // Rebuild FDN
        self.fdn = Fdn::new(
            sample_rate,
            self.params.room_size,
            self.params.rt60,
            self.params.bass_ratio,
            self.params.treble_ratio,
            self.params.mod_depth,
            self.params.safety_limit_db,
        );

        // LFE filter
        self.lfe_filter_coeff = compute_lp_coeff(LFE_CROSSOVER_HZ, sr);
        self.lfe_filter_state = 0.0;
        self.dialogue_duck_gain = 1.0;
        self.dialogue_attack_coeff = smoothing_coeff(DIALOGUE_ATTACK_MS, sr);
        self.dialogue_release_coeff = smoothing_coeff(DIALOGUE_RELEASE_MS, sr);
        self.dialogue_level_fast = 0.0;
        self.dialogue_level_slow = 0.0;
        self.dialogue_modulation = 0.0;
        self.dialogue_window_center_sum = 0.0;
        self.dialogue_window_side_sum = 0.0;
        self.dialogue_window_fill = 0;
        self.dialogue_window_samples = ms_to_samples(DIALOGUE_ANALYSIS_WINDOW_MS, sample_rate);
        self.dialogue_level_fast_coeff =
            smoothing_coeff_for_samples(DIALOGUE_LEVEL_FAST_MS, sr, self.dialogue_window_samples);
        self.dialogue_level_slow_coeff =
            smoothing_coeff_for_samples(DIALOGUE_LEVEL_SLOW_MS, sr, self.dialogue_window_samples);
        self.dialogue_mod_coeff =
            smoothing_coeff_for_samples(DIALOGUE_MOD_SMOOTH_MS, sr, self.dialogue_window_samples);
        self.dialogue_hold_samples = ms_to_samples(DIALOGUE_HOLD_MS, sample_rate);
        self.dialogue_hold_remaining = 0;
        self.dialogue_voiced_center = false;
        self.final_limiter_gain = 1.0;
        self.final_limiter_release_coeff = smoothing_coeff(FINAL_LIMITER_RELEASE_MS, sr);

        // Smoothers
        self.dry_smoother = Smoother::new(self.params.dry_level, 5.0, sample_rate);
        self.er_smoother = Smoother::new(self.params.er_level, 5.0, sample_rate);
        self.late_smoother = Smoother::new(self.params.late_level, 5.0, sample_rate);
        self.lfe_smoother = Smoother::new(self.params.lfe_level, 5.0, sample_rate);
        if let Some(auto_gain) = &mut self.auto_gain {
            auto_gain.set_sample_rate(sample_rate)?;
        } else if self.params.auto_gain_enabled {
            self.ensure_auto_gain()?;
        }

        // Recompute VBAP gains (ER taps may have changed)
        self.precompute_gains();

        Ok(())
    }

    fn reset(&mut self) {
        self.pre_delay.reset();
        for ap in &mut self.diffusion_allpass {
            ap.reset();
        }
        self.early_reflections.reset();
        self.fdn.reset();
        self.lfe_filter_state = 0.0;
        self.dialogue_duck_gain = 1.0;
        self.dialogue_level_fast = 0.0;
        self.dialogue_level_slow = 0.0;
        self.dialogue_modulation = 0.0;
        self.dialogue_hold_remaining = 0;
        self.dialogue_window_center_sum = 0.0;
        self.dialogue_window_side_sum = 0.0;
        self.dialogue_window_fill = 0;
        self.dialogue_voiced_center = false;
        self.final_limiter_gain = 1.0;
        if let Some(auto_gain) = &mut self.auto_gain {
            auto_gain.reset();
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        enable_ftz_daz();

        let num_frames = context.num_frames;
        let in_ch = 2;
        let out_ch = self.num_output_channels;
        let expected_input = num_frames * in_ch;
        let expected_output = num_frames * out_ch;

        if input.len() != expected_input {
            return Err(format!(
                "AAE input size mismatch: expected {}, got {}",
                expected_input,
                input.len()
            ));
        }
        if output.len() != expected_output {
            return Err(format!(
                "AAE output size mismatch: expected {}, got {}",
                expected_output,
                output.len()
            ));
        }

        // Zero output
        output.fill(0.0);

        // Bypass: copy L/R to front L/R, silence rest
        if self.params.bypass {
            for frame in 0..num_frames {
                let in_base = frame * in_ch;
                let out_base = frame * out_ch;
                let l = input[in_base];
                let r = input[in_base + 1];
                // Find front left and front right channels
                if out_ch >= 2 {
                    output[out_base] = l;
                    output[out_base + 1] = r;
                }
            }
            return Ok(num_frames);
        }

        if self.params.auto_gain_enabled {
            self.ensure_auto_gain()?;
            if let Some(auto_gain) = &mut self.auto_gain {
                auto_gain.measure_input(input)?;
            }
        }

        // Smooth parameters
        let dry_gain = self.dry_smoother.next_n(num_frames);
        let er_gain = self.er_smoother.next_n(num_frames);
        let late_gain = self.late_smoother.next_n(num_frames);
        let lfe_gain = self.lfe_smoother.next_n(num_frames);

        let num_er_taps = self.early_reflections.num_taps();
        self.er_tap_buffer.fill(0.0);

        // Find LFE channel index
        let lfe_ch = self
            .speaker_config
            .speakers
            .iter()
            .find(|s| s.is_lfe)
            .map(|s| s.channel);
        for frame in 0..num_frames {
            let in_base = frame * in_ch;
            let out_base = frame * out_ch;
            let l = input[in_base];
            let r = input[in_base + 1];
            let wet_duck = self.dialogue_duck_for_frame(l, r);

            // ── Direct path: pan stereo to front speakers ────────────
            if !self.params.solo_early && !self.params.solo_late {
                for (ch_idx, (gl, gr)) in self
                    .direct_gains_l
                    .iter()
                    .zip(self.direct_gains_r.iter())
                    .enumerate()
                {
                    output[out_base + ch_idx] += dry_gain * (l * gl + r * gr);
                }
            }

            // ── Reverb feed: mono downmix ────────────────────────────
            let mono = (l + r) * 0.5;

            // Pre-delay
            self.pre_delay.push(mono);
            let delayed = if self.pre_delay_samples > 0 {
                self.pre_delay.read(self.pre_delay_samples)
            } else {
                mono
            };

            // Input diffusion (allpass chain)
            let mut diffused = delayed;
            for ap in &mut self.diffusion_allpass {
                diffused = ap.process(diffused);
            }
            let mut lfe_wet_sum = 0.0_f32;
            let mut lfe_wet_energy = 0.0_f32;
            let mut lfe_wet_sources = 0usize;

            // ── Early reflections ────────────────────────────────────
            if !self.params.solo_late {
                self.early_reflections
                    .process(diffused, &mut self.er_tap_buffer);

                // `er_gains` has MAX_TAPS * out_ch entries and `tap_idx` is bounded
                // by `num_er_taps ≤ MAX_TAPS`.
                debug_assert!(
                    self.er_gains.len() == early_reflections::MAX_TAPS * out_ch,
                    "er_gains size mismatch: {}",
                    self.er_gains.len()
                );
                for (tap_idx, &tap_val) in self.er_tap_buffer[..num_er_taps].iter().enumerate() {
                    if tap_val.abs() < 1e-10 {
                        continue;
                    }
                    let row_start = tap_idx * out_ch;
                    let gains = &self.er_gains[row_start..row_start + out_ch];
                    let source_scaled = tap_val * er_gain;
                    let scaled = source_scaled * wet_duck;
                    lfe_wet_sum += source_scaled;
                    lfe_wet_energy += source_scaled * source_scaled;
                    lfe_wet_sources += 1;
                    for (ch_idx, &g) in gains.iter().enumerate() {
                        output[out_base + ch_idx] += scaled * g;
                    }
                }
            }

            // ── Late reverberation (FDN) ─────────────────────────────
            if !self.params.solo_early {
                // Feed the FDN with the sum of diffused input + ER output
                // (ER feeds into late reverb, creating a natural buildup)
                let er_sum: f32 = self.er_tap_buffer[..num_er_taps].iter().sum::<f32>()
                    / (num_er_taps.max(1) as f32);
                let fdn_input = diffused + er_sum * 0.3;

                let fdn_outputs = self.fdn.process(fdn_input);

                // `fdn_gains` has FDN_SIZE rows, `fdn_outputs` has FDN_SIZE elements,
                // so `line_idx >= fdn_gains.len()` is impossible at runtime.
                debug_assert_eq!(
                    self.fdn_gains.len(),
                    FDN_SIZE * out_ch,
                    "fdn_gains must have FDN_SIZE rows"
                );
                for (line_idx, &line_val) in fdn_outputs.iter().enumerate() {
                    if line_val.abs() < 1e-10 {
                        continue;
                    }
                    let row_start = line_idx * out_ch;
                    let gains = &self.fdn_gains[row_start..row_start + out_ch];
                    let source_scaled = line_val * late_gain;
                    let scaled = source_scaled * wet_duck;
                    lfe_wet_sum += source_scaled;
                    lfe_wet_energy += source_scaled * source_scaled;
                    lfe_wet_sources += 1;
                    for (ch_idx, &g) in gains.iter().enumerate() {
                        output[out_base + ch_idx] += scaled * g;
                    }
                }
            }

            // ── LFE extraction ───────────────────────────────────────
            if let Some(lfe_idx) = lfe_ch {
                // One-pole LP on source-domain wet energy. This keeps the LFE
                // independent of speaker layout and avoids cancellation from
                // summing decorrelated routed speaker feeds.
                let lfe_source = signed_rms(lfe_wet_sum, lfe_wet_energy, lfe_wet_sources, diffused);
                let lp_coeff = self.lfe_filter_coeff;
                self.lfe_filter_state =
                    lp_coeff * self.lfe_filter_state + (1.0 - lp_coeff) * lfe_source;
                output[out_base + lfe_idx] += self.lfe_filter_state * lfe_gain * wet_duck;
            }
        }

        flush_denormals_inplace(output);
        self.apply_auto_gain(output, num_frames)?;
        self.apply_output_safety_limit(output, num_frames, out_ch);
        flush_denormals_inplace(output);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(Arc::new(AaeData {
            auto_gain: self
                .auto_gain
                .as_ref()
                .map(MultichannelAutoGain::data)
                .unwrap_or_default(),
        }))
    }
}
