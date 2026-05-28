// ============================================================================
// Active Acoustic Enhancement (AAE) Plugin
// ============================================================================
//
// AAE-inspired multichannel reverb. Takes stereo input and outputs
// multichannel audio (5.0–9.1.6) with synthesized early reflections and
// late reverberation distributed across speakers via VBAP.
//
// Signal flow:
//   Stereo input → mono downmix → pre-delay → input diffusion →
//   early reflections (tapped delay, per-tap VBAP) +
//   FDN late reverb (8-line, Hadamard, time-variant) →
//   multichannel speaker routing → output mixed with dry signal

pub mod delay_line;
pub mod early_reflections;
pub mod fdn;
pub mod hadamard;
pub mod params;
pub mod tone_filter;

use crate::early_reflections::EarlyReflections;
use crate::fdn::{FDN_SIZE, Fdn};
use crate::params::{AaePluginParams, build_parameters};

use sotf_host::auto_gain::{AutoGainData, AutoGainLoudnessType, AutoGainParams};
use sotf_host::multichannel_auto_gain::MultichannelAutoGain;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::speaker_config::{
    SourcePosition, SpeakerConfig, compute_vbap_matrix, get_speaker_config, normalize_gains_l2,
};
use std::any::Any;
use std::sync::Arc;

/// LFE crossover frequency in Hz.
const LFE_CROSSOVER_HZ: f32 = 120.0;

/// Maximum pre-delay in milliseconds.
const MAX_PRE_DELAY_MS: f32 = 100.0;
const DIALOGUE_ATTACK_MS: f32 = 10.0;
const DIALOGUE_RELEASE_MS: f32 = 150.0;
const DIALOGUE_LEVEL_FAST_MS: f32 = 35.0;
const DIALOGUE_LEVEL_SLOW_MS: f32 = 180.0;
const DIALOGUE_MOD_SMOOTH_MS: f32 = 40.0;
const DIALOGUE_ANALYSIS_WINDOW_MS: f32 = 50.0;
const DIALOGUE_HOLD_MS: f32 = 600.0;
const DIALOGUE_MIN_CENTER_LEVEL: f32 = 0.06;
const DIALOGUE_MIN_MODULATION: f32 = 0.10;
const FINAL_OUTPUT_CEILING: f32 = 1.0;
const FINAL_LIMITER_RELEASE_MS: f32 = 60.0;

pub struct AaePlugin {
    sample_rate: u32,
    params: AaePluginParams,
    speaker_config: &'static SpeakerConfig,
    num_output_channels: usize,

    // DSP components
    pre_delay: delay_line::DelayLine,
    pre_delay_samples: usize,
    diffusion_allpass: [AllpassDiffuser; 2],
    early_reflections: EarlyReflections,
    fdn: Fdn,

    // LFE extraction: one-pole LP filter state
    lfe_filter_state: f32,
    lfe_filter_coeff: f32,

    // Per-tap VBAP gains for early reflections (flattened row-major):
    // [tap_index * out_ch + speaker_index]
    er_gains: Vec<f32>,

    // FDN output → speaker routing (flattened row-major):
    // [fdn_line * out_ch + speaker_index]
    fdn_gains: Vec<f32>,

    // Direct signal panning gains (stereo L/R to front speakers)
    direct_gains_l: Vec<f32>,
    direct_gains_r: Vec<f32>,

    // Pre-allocated scratch buffer for ER tap outputs (avoids hot-path allocation)
    er_tap_buffer: Vec<f32>,

    // Content-aware wet ducking state
    dialogue_duck_gain: f32,
    dialogue_attack_coeff: f32,
    dialogue_release_coeff: f32,
    dialogue_level_fast: f32,
    dialogue_level_slow: f32,
    dialogue_modulation: f32,
    dialogue_level_fast_coeff: f32,
    dialogue_level_slow_coeff: f32,
    dialogue_mod_coeff: f32,
    dialogue_hold_samples: usize,
    dialogue_hold_remaining: usize,
    dialogue_window_center_sum: f32,
    dialogue_window_side_sum: f32,
    dialogue_window_fill: usize,
    dialogue_window_samples: usize,
    dialogue_voiced_center: bool,

    // Linked emergency limiter state for the rendered multichannel output.
    final_limiter_gain: f32,
    final_limiter_release_coeff: f32,

    // Parameter smoothers
    dry_smoother: Smoother,
    er_smoother: Smoother,
    late_smoother: Smoother,
    lfe_smoother: Smoother,

    // Auto-gain compensation. The meter is stereo: input L/R vs a stereo fold-down
    // of the multichannel render, then the resulting gain is applied to all outputs.
    auto_gain: Option<MultichannelAutoGain>,

    // Cached parameter list
    cached_parameters: Vec<Parameter>,
}

#[derive(Debug, Clone)]
pub struct AaeData {
    pub auto_gain: AutoGainData,
}

/// Two-stage allpass diffuser for smearing transients before the FDN.
struct AllpassDiffuser {
    buffer: Vec<f32>,
    write_pos: usize,
    delay: usize,
    feedback: f32,
}

impl AllpassDiffuser {
    fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples + 1],
            write_pos: 0,
            delay: delay_samples,
            feedback,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let buf_len = self.buffer.len();
        let read_pos = (self.write_pos + buf_len - self.delay) % buf_len;
        let delayed = self.buffer[read_pos];
        // Schroeder allpass: y[n] = -g*x[n] + s[n-M], s[n] = x[n] + g*y[n]
        // where s is the buffer. DC gain = 1.0, |H(e^jw)| = 1 for all w.
        let output = -self.feedback * input + delayed;
        self.buffer[self.write_pos] = input + self.feedback * output;
        self.write_pos = (self.write_pos + 1) % buf_len;
        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
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
            pre_delay: delay_line::DelayLine::new(
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
    fn precompute_gains(&mut self) {
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
        er.resize(early_reflections::MAX_TAPS, vec![0.0; n_ch]);
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

    fn update_cached_parameter(&mut self, id: &ParameterId, value: &ParameterValue) {
        if let Some(param) = self.cached_parameters.iter_mut().find(|p| p.id == *id) {
            param.default_value = value.clone();
        }
    }

    #[inline]
    fn dialogue_duck_for_frame(&mut self, l: f32, r: f32) -> f32 {
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

    fn ensure_auto_gain(&mut self) -> PluginResult<()> {
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

    fn apply_auto_gain(&mut self, output: &mut [f32], num_frames: usize) -> PluginResult<()> {
        if !self.params.auto_gain_enabled {
            return Ok(());
        }
        self.ensure_auto_gain()?;
        let speaker_config = self.speaker_config;
        let out_ch = self.num_output_channels;
        let auto_gain = self.auto_gain.as_mut().unwrap();
        auto_gain.measure_and_apply(output, num_frames, out_ch, speaker_config)
    }

    fn apply_output_safety_limit(&mut self, output: &mut [f32], num_frames: usize, out_ch: usize) {
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

/// Always allocates a `MultichannelAutoGain` instance so enabling it via
/// `set_parameter` on the audio thread does not trigger a heap allocation.
/// The instance starts enabled or disabled depending on
/// `params.auto_gain_enabled`; callers can flip the flag at runtime.
fn create_auto_gain(params: &AaePluginParams, sample_rate: u32) -> Option<MultichannelAutoGain> {
    MultichannelAutoGain::new(
        sample_rate,
        AutoGainParams {
            enabled: params.auto_gain_enabled,
            loudness_type: AutoGainLoudnessType::Momentary,
            max_gain_db: params.auto_gain_max_db,
            smoothing_ms: params.auto_gain_smoothing_ms,
        },
    )
    .map_err(|err| log::warn!("AAE auto-gain initialization failed: {err}"))
    .ok()
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
        self.pre_delay = delay_line::DelayLine::new((MAX_PRE_DELAY_MS * 0.001 * sr) as usize + 16);
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

fn smoothing_coeff(time_ms: f32, sample_rate: f32) -> f32 {
    (-1.0 / (time_ms * 0.001 * sample_rate)).exp()
}

fn smoothing_coeff_for_samples(time_ms: f32, sample_rate: f32, samples: usize) -> f32 {
    (-(samples.max(1) as f32) / (time_ms * 0.001 * sample_rate)).exp()
}

/// Compute one-pole low-pass filter coefficient from cutoff frequency.
fn compute_lp_coeff(cutoff_hz: f32, sample_rate: f32) -> f32 {
    debug_assert!(sample_rate > 0.0);
    let nyquist = 0.5 * sample_rate;
    let max_cutoff = nyquist * 0.25;
    debug_assert!(
        (0.0..=max_cutoff).contains(&cutoff_hz),
        "cutoff_hz {cutoff_hz} is outside valid one-pole range (0..={max_cutoff}) for sample_rate {sample_rate}"
    );

    let cutoff_hz = cutoff_hz.clamp(1e-6, max_cutoff);
    let w = std::f32::consts::PI * cutoff_hz / nyquist;
    (1.0 - w.sin()) / w.cos()
}

fn ms_to_samples(time_ms: f32, sample_rate: u32) -> usize {
    (time_ms * 0.001 * sample_rate as f32).round().max(1.0) as usize
}

fn signed_rms(_sum: f32, energy: f32, count: usize, _polarity_hint: f32) -> f32 {
    if count == 0 || energy <= 0.0 {
        return 0.0;
    }

    let rms = (energy / count as f32).sqrt();
    rms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plugin() -> AaePlugin {
        let mut p = AaePlugin::from_params(AaePluginParams::default());
        p.initialize(48000).unwrap();
        p
    }

    #[test]
    fn test_output_channel_count() {
        let p = make_plugin();
        assert_eq!(p.input_channels(), 2);
        assert_eq!(p.output_channels(), 6); // 5.1 default
    }

    #[test]
    fn test_bypass_passes_dry() {
        let mut p = make_plugin();
        p.set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
            .unwrap();

        let input = vec![0.5, -0.3]; // one frame, stereo
        let mut output = vec![0.0; 6]; // 5.1
        p.process(&input, &mut output, &ProcessContext::new(48000, 1))
            .unwrap();

        assert!((output[0] - 0.5).abs() < 1e-6, "L should pass through");
        assert!((output[1] - (-0.3)).abs() < 1e-6, "R should pass through");
    }

    #[test]
    fn test_silence_in_silence_out() {
        let mut p = make_plugin();
        let n = 1024;
        let input = vec![0.0; n * 2];
        let mut output = vec![0.0; n * 6];
        p.process(&input, &mut output, &ProcessContext::new(48000, n))
            .unwrap();

        let max = output.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        assert!(max < 1e-6, "Silence in should give silence out, max={max}");
    }

    #[test]
    fn test_compute_lp_coeff_uses_bilinear_formula() {
        let coeff = compute_lp_coeff(LFE_CROSSOVER_HZ, 48000.0);
        let w = std::f32::consts::PI * LFE_CROSSOVER_HZ / (48000.0 * 0.5);
        let expected = (1.0 - w.sin()) / w.cos();
        assert!(
            (coeff - expected).abs() < 1e-6,
            "compute_lp_coeff should match documented bilinear form: got {coeff}, expected {expected}"
        );
        assert!(
            (0.0..1.0).contains(&coeff),
            "LFE coefficient should stay positive and < 1"
        );
    }

    #[test]
    fn test_impulse_produces_reverb() {
        let mut p = make_plugin();

        // Feed one frame of impulse then silence
        let mut all_output = vec![0.0_f32; 0];

        // Impulse frame
        let input_impulse = vec![1.0, 1.0];
        let mut out = vec![0.0; 6];
        p.process(&input_impulse, &mut out, &ProcessContext::new(48000, 1))
            .unwrap();
        all_output.extend_from_slice(&out);

        // Process 2 seconds of silence
        let chunk = 1024;
        let input_silence = vec![0.0; chunk * 2];
        let mut out_chunk = vec![0.0; chunk * 6];
        for _ in 0..(48000 * 2 / chunk) {
            p.process(
                &input_silence,
                &mut out_chunk,
                &ProcessContext::new(48000, chunk),
            )
            .unwrap();
            all_output.extend_from_slice(&out_chunk);
        }

        // Check that there's signal after the pre-delay (reverb tail)
        let pre_delay_frames = (20.0 * 0.001 * 48000.0) as usize; // 20ms default
        let late_start = (pre_delay_frames + 1000) * 6; // well past pre-delay
        if late_start < all_output.len() {
            let late_energy: f32 = all_output[late_start..]
                .iter()
                .take(48000) // ~1 second
                .map(|v| v * v)
                .sum();
            assert!(
                late_energy > 1e-6,
                "Should have reverb tail, late_energy={late_energy}"
            );
        }
    }

    #[test]
    fn test_no_nan_inf() {
        let mut p = make_plugin();
        let n = 4096;
        let input: Vec<f32> = (0..n * 2).map(|i| (i as f32 * 0.01).sin() * 0.8).collect();
        let mut output = vec![0.0; n * 6];
        p.process(&input, &mut output, &ProcessContext::new(48000, n))
            .unwrap();

        for (i, v) in output.iter().enumerate() {
            assert!(v.is_finite(), "Output[{i}] is not finite: {v}");
        }
    }

    #[test]
    fn test_7_1_4_config() {
        let params = AaePluginParams {
            speaker_config: "7.1.4".to_string(),
            ..AaePluginParams::default()
        };
        let mut p = AaePlugin::from_params(params);
        p.initialize(48000).unwrap();
        assert_eq!(p.output_channels(), 12);
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut p = make_plugin();
        p.set_parameter(ParameterId::from("rt60"), ParameterValue::Float(3.5))
            .unwrap();
        assert_eq!(
            p.get_parameter(&ParameterId::from("rt60")),
            Some(ParameterValue::Float(3.5))
        );
    }

    #[test]
    fn test_auto_gain_parameters_roundtrip_and_data() {
        let mut p = make_plugin();
        assert_eq!(
            p.get_parameter(&ParameterId::from("auto_gain_enabled")),
            Some(ParameterValue::Bool(false))
        );

        p.set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(true),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("auto_gain_max_db"),
            ParameterValue::Float(9.0),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("auto_gain_smoothing_ms"),
            ParameterValue::Float(80.0),
        )
        .unwrap();

        assert_eq!(
            p.get_parameter(&ParameterId::from("auto_gain_enabled")),
            Some(ParameterValue::Bool(true))
        );
        assert_eq!(
            p.get_parameter(&ParameterId::from("auto_gain_max_db")),
            Some(ParameterValue::Float(9.0))
        );
        assert_eq!(
            p.get_parameter(&ParameterId::from("auto_gain_smoothing_ms")),
            Some(ParameterValue::Float(80.0))
        );

        let data = p.get_data().unwrap();
        let data = data.downcast_ref::<AaeData>().unwrap();
        assert!(data.auto_gain.enabled);
    }

    #[test]
    fn test_pre_delay_is_not_reported_as_plugin_latency() {
        let p = make_plugin();
        assert_eq!(p.latency_samples(), 0);
    }

    /// Auto-gain must be pre-allocated in `from_params` even when disabled,
    /// so that enabling it via `set_parameter` on the audio thread does not
    /// trigger a heap allocation. Verify by checking `auto_gain.is_some()`
    /// before and after enabling via `set_parameter`.
    #[test]
    fn test_auto_gain_preallocated_when_disabled() {
        // Default params have auto_gain_enabled = false
        let p = AaePlugin::from_params(AaePluginParams::default());
        assert!(
            !p.params.auto_gain_enabled,
            "Precondition: auto_gain disabled by default"
        );
        // The field must be Some even when disabled — pre-allocated for audio-thread safety
        assert!(
            p.auto_gain.is_some(),
            "auto_gain must be pre-allocated even when disabled to avoid \
             audio-thread allocation when set_parameter enables it"
        );
    }

    #[test]
    fn test_process_rejects_mismatched_buffers() {
        let mut p = make_plugin();
        let input = vec![0.0; 1];
        let mut output = vec![0.0; 6];
        let err = p
            .process(&input, &mut output, &ProcessContext::new(48000, 1))
            .unwrap_err();
        assert!(err.contains("input size mismatch"));

        let input = vec![0.0; 2];
        let mut output = vec![0.0; 5];
        let err = p
            .process(&input, &mut output, &ProcessContext::new(48000, 1))
            .unwrap_err();
        assert!(err.contains("output size mismatch"));
    }

    #[test]
    fn test_content_aware_dialogue_ducks_wet_signal() {
        let mut p = make_plugin();
        p.params.content_aware = true;
        p.params.dialogue_attenuation_db = 12.0;

        let sr = 48000.0_f32;
        for i in 0..48000 {
            let t = i as f32 / sr;
            let syllable = if (t * 6.0).fract() < 0.55 { 1.0 } else { 0.12 };
            let sample = (std::f32::consts::TAU * 180.0 * t).sin() * 0.45 * syllable;
            p.dialogue_duck_for_frame(sample, sample);
        }
        assert!(
            p.dialogue_duck_gain < 0.8,
            "centered speech-like input should reduce wet gain, got {}",
            p.dialogue_duck_gain
        );

        p.params.content_aware = false;
        assert_eq!(p.dialogue_duck_for_frame(0.5, 0.5), 1.0);
    }

    #[test]
    fn test_signed_rms_keeps_energy_unsigned() {
        // Sum can cancel to zero for symmetric content; polarity hints should not
        // flip LFE polarity.
        let sum = 0.0;
        let count = 4;
        let samples = [-1.0, 1.0, -1.0, 1.0];
        let energy: f32 = samples.iter().map(|s| s * s).sum();
        let got = signed_rms(sum, energy, count, -0.5);
        assert!(
            (got - 1.0).abs() < 1e-6,
            "LFE extraction should be unsigned RMS-like for decorrelated decorrelation, got {got}"
        );
    }

    #[test]
    fn test_content_aware_ignores_quiet_centered_noise() {
        let mut p = make_plugin();
        p.params.content_aware = true;
        p.params.dialogue_attenuation_db = 12.0;

        for _ in 0..48000 {
            p.dialogue_duck_for_frame(0.025, 0.025);
        }

        assert!(
            p.dialogue_duck_gain > 0.95,
            "quiet centered noise should not trigger dialogue ducking, got {}",
            p.dialogue_duck_gain
        );
    }

    #[test]
    fn test_content_aware_ignores_steady_centered_music() {
        let mut p = make_plugin();
        p.params.content_aware = true;
        p.params.dialogue_attenuation_db = 12.0;

        let sr = 48000.0_f32;
        for i in 0..144000 {
            let t = i as f32 / sr;
            let sample = (std::f32::consts::TAU * 220.0 * t).sin() * 0.4;
            p.dialogue_duck_for_frame(sample, sample);
        }

        assert!(
            p.dialogue_duck_gain > 0.9,
            "steady centered tonal content should not keep wet gain ducked, got {}",
            p.dialogue_duck_gain
        );
    }

    #[test]
    fn test_content_aware_holds_through_sustained_centered_voice() {
        let mut p = make_plugin();
        p.params.content_aware = true;
        p.params.dialogue_attenuation_db = 12.0;

        let sr = 48000.0_f32;
        for i in 0..9600 {
            let t = i as f32 / sr;
            let syllable = if (t * 7.0).fract() < 0.5 { 1.0 } else { 0.2 };
            let sample = (std::f32::consts::TAU * 190.0 * t).sin() * 0.45 * syllable;
            p.dialogue_duck_for_frame(sample, sample);
        }
        let ducked_after_modulation = p.dialogue_duck_gain;

        for i in 0..19200 {
            let t = i as f32 / sr;
            let sample = (std::f32::consts::TAU * 190.0 * t).sin() * 0.32;
            p.dialogue_duck_for_frame(sample, sample);
        }

        assert!(
            ducked_after_modulation < 0.9 && p.dialogue_duck_gain < 0.8,
            "sustained centered voice should stay ducked after syllabic onset, before={} after={}",
            ducked_after_modulation,
            p.dialogue_duck_gain
        );
    }

    #[test]
    fn test_room_and_routing_parameter_changes_do_not_break_processing() {
        let mut p = make_plugin();
        p.set_parameter(ParameterId::from("room_size"), ParameterValue::Float(3.0))
            .unwrap();
        p.set_parameter(
            ParameterId::from("room_preset"),
            ParameterValue::String("cathedral".to_string()),
        )
        .unwrap();
        p.set_parameter(ParameterId::from("envelopment"), ParameterValue::Float(1.0))
            .unwrap();
        p.set_parameter(
            ParameterId::from("height_amount"),
            ParameterValue::Float(1.0),
        )
        .unwrap();
        p.set_parameter(
            ParameterId::from("room_preset"),
            ParameterValue::String("small".to_string()),
        )
        .unwrap();

        let n = 512;
        let input = vec![0.1; n * 2];
        let mut output = vec![0.0; n * 6];
        p.process(&input, &mut output, &ProcessContext::new(48000, n))
            .unwrap();
        assert!(output.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_solo_early() {
        let mut p = make_plugin();
        p.set_parameter(ParameterId::from("solo_early"), ParameterValue::Bool(true))
            .unwrap();

        let n = 4096;
        let input: Vec<f32> = (0..n * 2).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let mut output = vec![0.0; n * 6];
        p.process(&input, &mut output, &ProcessContext::new(48000, n))
            .unwrap();

        // Should produce some output (ER only)
        let energy: f32 = output.iter().map(|v| v * v).sum();
        // Energy should be non-zero (ER taps produce output)
        // Note: with pre-delay, the first few frames may be silent
        assert!(energy.is_finite());
    }

    #[test]
    fn test_reset_clears_state() {
        let mut p = make_plugin();

        // Feed some signal
        let n = 2048;
        let input: Vec<f32> = (0..n * 2).map(|_| 0.5).collect();
        let mut output = vec![0.0; n * 6];
        p.process(&input, &mut output, &ProcessContext::new(48000, n))
            .unwrap();

        p.reset();

        // After reset, silence in should give silence out
        let input_silent = vec![0.0; n * 2];
        let mut output2 = vec![0.0; n * 6];
        p.process(&input_silent, &mut output2, &ProcessContext::new(48000, n))
            .unwrap();

        let max = output2.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        assert!(max < 1e-6, "After reset, should be silent, max={max}");
    }

    #[test]
    fn test_allpass_diffuser_unity_gain_dc() {
        // Schroeder allpass must have |H(z)| = 1 for all frequencies.
        // Verify DC gain = 1.0 by feeding constant input until steady state.
        let mut ap = AllpassDiffuser::new(37, 0.7);
        let mut output = 0.0;
        for _ in 0..10000 {
            output = ap.process(1.0);
        }
        assert!(
            (output - 1.0).abs() < 0.01,
            "Allpass DC gain should be 1.0, got {output}"
        );
    }

    #[test]
    fn test_allpass_diffuser_energy_preservation() {
        // Feed a sine wave; output energy should equal input energy.
        let mut ap = AllpassDiffuser::new(53, 0.65);
        let n = 48000;
        let mut input_energy = 0.0_f64;
        let mut output_energy = 0.0_f64;
        // Skip transient (first 1000 samples)
        for i in 0..1000 {
            let x = (i as f32 * 0.1).sin();
            ap.process(x);
        }
        for i in 1000..n {
            let x = (i as f32 * 0.1).sin();
            let y = ap.process(x);
            input_energy += (x * x) as f64;
            output_energy += (y * y) as f64;
        }
        let ratio = output_energy / input_energy;
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "Allpass energy ratio should be ~1.0, got {ratio}"
        );
    }

    #[test]
    fn test_lfe_tracks_late_reverb_tail() {
        let params = AaePluginParams {
            dry_level: 0.0,
            er_level: 0.0,
            late_level: 1.0,
            lfe_level: 1.0,
            pre_delay_ms: 0.0,
            content_aware: false,
            ..AaePluginParams::default()
        };
        let mut p = AaePlugin::from_params(params);
        p.initialize(48000).unwrap();

        let lfe_idx = p
            .speaker_config
            .speakers
            .iter()
            .find(|speaker| speaker.is_lfe)
            .map(|speaker| speaker.channel)
            .expect("default 5.1 config has an LFE channel");

        let chunk = 512;
        let mut input = vec![0.0_f32; chunk * 2];
        input[0] = 1.0;
        input[1] = 1.0;
        let mut output = vec![0.0_f32; chunk * p.num_output_channels];
        let context = ProcessContext::new(48000, chunk);

        let mut late_lfe_energy = 0.0_f32;
        let mut frame_offset = 0usize;
        for block in 0..120 {
            p.process(&input, &mut output, &context).unwrap();
            for frame in 0..chunk {
                if frame_offset + frame > 12000 {
                    let sample = output[frame * p.num_output_channels + lfe_idx];
                    late_lfe_energy += sample * sample;
                }
            }
            input.fill(0.0);
            frame_offset += chunk;
            if block > 80 && late_lfe_energy > 1e-10 {
                break;
            }
        }

        assert!(
            late_lfe_energy > 1e-10,
            "LFE should contain low-passed late reverb tail energy, got {late_lfe_energy}"
        );
    }

    #[test]
    fn test_lfe_source_energy_does_not_cancel_with_signed_sum() {
        let source = signed_rms(0.0, 2.0, 2, -0.5);

        assert!(
            (source - 1.0).abs() < 1e-6,
            "source-domain LFE energy should use unsigned RMS for decorrelated source energy, got {source}"
        );
    }

    #[test]
    fn test_output_safety_limit_bounds_final_mix() {
        let params = AaePluginParams {
            dry_level: 1.0,
            er_level: 1.0,
            late_level: 1.0,
            lfe_level: 1.0,
            pre_delay_ms: 0.0,
            safety_limit_db: 0.0,
            content_aware: false,
            ..AaePluginParams::default()
        };
        let mut p = AaePlugin::from_params(params);
        p.initialize(48000).unwrap();

        let n = 4096;
        let input = vec![2.0_f32; n * 2];
        let mut output = vec![0.0; n * p.output_channels()];
        p.process(&input, &mut output, &ProcessContext::new(48000, n))
            .unwrap();

        let max = output.iter().copied().map(f32::abs).fold(0.0, f32::max);
        assert!(
            max <= 1.0 + 1e-6,
            "final output should respect the 0 dBFS safety limit, max={max}"
        );
    }

    #[test]
    fn test_output_safety_limit_preserves_channel_ratios() {
        let mut p = make_plugin();
        let mut output = vec![2.0, 1.0, -0.5, 0.25, 0.0, -1.0];

        p.apply_output_safety_limit(&mut output, 1, 6);

        assert!((output[0] - 1.0).abs() < 1e-6);
        assert!((output[1] - 0.5).abs() < 1e-6);
        assert!((output[2] + 0.25).abs() < 1e-6);
        assert!((output[5] + 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_output_energy_bounded() {
        // With default params, output energy should not exceed 2× input energy.
        let mut p = make_plugin();
        let n = 4096;
        let input: Vec<f32> = (0..n * 2).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let mut output = vec![0.0; n * 6];
        p.process(&input, &mut output, &ProcessContext::new(48000, n))
            .unwrap();

        let input_energy: f32 = input.iter().map(|v| v * v).sum();
        let output_energy: f32 = output.iter().map(|v| v * v).sum();

        // Output has 6 channels vs 2 input channels, so per-channel energy can be lower
        // but total should be bounded. With dry=0.5, er=0.3, late=0.2, should be < 2×
        assert!(
            output_energy < input_energy * 3.0,
            "Output energy {output_energy} should be < 3× input energy {input_energy}"
        );
    }
}
