use super::allpass_state::AllpassState;
use super::misc::MAX_DELAY_MS;
use super::misc::parse_channel_delay_id;
use super::types::DelayPluginParams;
use sotf_host::param_specs::UpdateMode;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

const ALLPASS_SMOOTH_MS: f32 = 20.0;
const CLEAN_CROSSFADE_MS: f32 = 20.0;
pub(super) const MAX_DELAY_CHANNELS: usize = 64;
const MAX_DELAY_SAMPLE_RATE: u32 = 768_000;

/// Preallocated dual-read-head state for pitch-preserving delay changes.
/// Read positions remain fixed throughout a transition; only their gains move.
struct CleanTransitionState {
    current_delay_samples: Vec<f32>,
    next_delay_samples: Vec<f32>,
    fade_position: usize,
    fade_samples: usize,
    active: bool,
}

impl CleanTransitionState {
    fn new(channels: usize, initial_delay_samples: f32, sample_rate: u32) -> Self {
        Self {
            current_delay_samples: vec![initial_delay_samples; channels],
            next_delay_samples: vec![initial_delay_samples; channels],
            fade_position: 0,
            fade_samples: ((CLEAN_CROSSFADE_MS * sample_rate as f32 / 1000.0).round() as usize)
                .max(1),
            active: false,
        }
    }
}

pub(super) struct ModulationState {
    param_rate_hz: ParameterId,
    pub(super) rate_hz: f32,
    param_depth_ms: ParameterId,
    pub(super) depth_ms: f32,
    param_pitch_preserving: ParameterId,
    pitch_preserving: bool,
    phase: f32,
    clean_transition: CleanTransitionState,
}

pub struct DelayPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    pub(super) param_delay_ms: ParameterId,
    pub(super) delay_ms: f32,
    pub(super) param_feedback: ParameterId,
    pub(super) feedback: f32,
    pub(super) param_mix: ParameterId,
    pub(super) mix: f32,
    pub(super) modulation: ModulationState,
    pub(super) param_allpass_feedback: ParameterId,
    pub(super) allpass_feedback: bool,
    pub(super) param_allpass_coeff: ParameterId,
    pub(super) allpass_coeff: f32,
    pub(super) delay_smoother: Smoother,
    pub(super) feedback_smoother: Smoother,
    pub(super) mix_smoother: Smoother,
    /// When non-empty, plugin runs in per-channel mode: each channel has
    /// its own delay time (in ms) and its own smoother. The scalar
    /// `delay_ms` / `delay_smoother` are unused in per-channel mode.
    pub(super) channel_delays_ms: Vec<f32>,
    pub(super) channel_delay_smoothers: Vec<Smoother>,
    pub(super) buffer: Vec<f32>,
    pub(super) write_pos: usize,
    pub(super) max_samples: usize,
    /// Maximum delay that this instance promises to automate without resizing.
    pub(super) max_delay_ms: f32,
    /// Per-channel allpass filter states for the feedback path
    pub(super) allpass_states: Vec<AllpassState>,
    pub(super) allpass_mix_smoother: Smoother,
    pub(super) allpass_coeff_smoother: Smoother,
    pub(super) cached_parameters: Vec<Parameter>,
    initialized: bool,
}

impl DelayPlugin {
    #[cfg(test)]
    pub(super) fn clean_transition_active(&self) -> bool {
        self.modulation.clean_transition.active
    }

    #[inline]
    fn ring_samples(max_delay_ms: f32, sample_rate: u32) -> usize {
        let required = (max_delay_ms * 0.001 * sample_rate as f32).ceil() as usize;
        required
            .checked_add(4)
            .and_then(usize::checked_next_power_of_two)
            .unwrap_or(usize::MAX)
            .max(8)
    }

    fn validate_float(name: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
        if !value.is_finite() || !(min..=max).contains(&value) {
            return Err(format!(
                "{name} must be finite and in [{min}, {max}], got {value}"
            ));
        }
        Ok(())
    }

    fn validate_params(channels: usize, params: &DelayPluginParams) -> Result<(), String> {
        if !(1..=MAX_DELAY_CHANNELS).contains(&channels) {
            return Err(format!(
                "channels must be in [1, {MAX_DELAY_CHANNELS}], got {channels}"
            ));
        }
        Self::validate_float("delay_ms", params.delay_ms, 0.0, MAX_DELAY_MS)?;
        Self::validate_float("feedback", params.feedback, -0.95, 0.95)?;
        Self::validate_float("mix", params.mix, 0.0, 1.0)?;
        Self::validate_float("lfo_rate_hz", params.lfo_rate_hz, 0.0, 20.0)?;
        Self::validate_float("lfo_depth_ms", params.lfo_depth_ms, 0.0, 10.0)?;
        if params.pitch_preserving && (params.lfo_rate_hz != 0.0 || params.lfo_depth_ms != 0.0) {
            return Err(
                "pitch_preserving mode requires lfo_rate_hz and lfo_depth_ms to be zero".into(),
            );
        }
        Self::validate_float("allpass_coeff", params.allpass_coeff, 0.0, 0.99)?;
        for (channel, &delay_ms) in params.channel_delays_ms.iter().enumerate() {
            Self::validate_float(
                &format!("channel_delays_ms[{channel}]"),
                delay_ms,
                0.0,
                MAX_DELAY_MS,
            )?;
        }
        Ok(())
    }

    pub fn new(channels: usize, delay_ms: f32, feedback: f32, mix: f32) -> Self {
        Self::try_new(channels, delay_ms, feedback, mix)
            .expect("invalid DelayPlugin constructor arguments")
    }

    /// Validated constructor for the standard five-second automation range.
    pub fn try_new(
        channels: usize,
        delay_ms: f32,
        feedback: f32,
        mix: f32,
    ) -> Result<Self, String> {
        Self::try_new_with_max_delay(channels, delay_ms, feedback, mix, MAX_DELAY_MS)
    }

    /// Fallible constructor with an explicit automation range.
    ///
    /// The ring is sized for `max_delay_ms`, so short fixed/routing delays do
    /// not reserve five seconds per channel. Runtime delay writes are limited
    /// to this declared range and never resize on the audio thread.
    pub fn try_new_with_max_delay(
        channels: usize,
        delay_ms: f32,
        feedback: f32,
        mix: f32,
        max_delay_ms: f32,
    ) -> Result<Self, String> {
        Self::validate_float("max_delay_ms", max_delay_ms, 0.0, MAX_DELAY_MS)?;
        Self::validate_float("delay_ms", delay_ms, 0.0, max_delay_ms)?;
        Self::validate_float("feedback", feedback, -0.95, 0.95)?;
        Self::validate_float("mix", mix, 0.0, 1.0)?;
        if !(1..=MAX_DELAY_CHANNELS).contains(&channels) {
            return Err(format!(
                "channels must be in [1, {MAX_DELAY_CHANNELS}], got {channels}"
            ));
        }
        let sr = 44100;
        let max_samples = Self::ring_samples(max_delay_ms, sr);
        let buffer_len = max_samples
            .checked_mul(channels)
            .ok_or_else(|| "delay buffer capacity overflow".to_string())?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(buffer_len)
            .map_err(|error| format!("failed to reserve delay buffer: {error}"))?;
        buffer.resize(buffer_len, 0.0);
        let mut p = Self {
            channels,
            sample_rate: sr,
            param_delay_ms: ParameterId::from("delay_ms"),
            delay_ms,
            param_feedback: ParameterId::from("feedback"),
            feedback,
            param_mix: ParameterId::from("mix"),
            mix,
            modulation: ModulationState {
                param_rate_hz: ParameterId::from("lfo_rate_hz"),
                rate_hz: 0.0,
                param_depth_ms: ParameterId::from("lfo_depth_ms"),
                depth_ms: 0.0,
                param_pitch_preserving: ParameterId::from("pitch_preserving"),
                pitch_preserving: false,
                phase: 0.0,
                clean_transition: CleanTransitionState::new(
                    channels,
                    delay_ms * sr as f32 / 1000.0,
                    sr,
                ),
            },
            param_allpass_feedback: ParameterId::from("allpass_feedback"),
            allpass_feedback: false,
            param_allpass_coeff: ParameterId::from("allpass_coeff"),
            allpass_coeff: 0.5,
            delay_smoother: Smoother::new(delay_ms * sr as f32 / 1000.0, 50.0, sr),
            feedback_smoother: Smoother::new(feedback, 5.0, sr),
            mix_smoother: Smoother::new(mix, 5.0, sr),
            channel_delays_ms: Vec::new(),
            channel_delay_smoothers: Vec::new(),
            buffer,
            write_pos: 0,
            max_samples,
            max_delay_ms,
            allpass_states: vec![AllpassState::new(0.5); channels],
            allpass_mix_smoother: Smoother::new(0.0, ALLPASS_SMOOTH_MS, sr),
            allpass_coeff_smoother: Smoother::new(0.5, ALLPASS_SMOOTH_MS, sr),
            cached_parameters: Vec::new(),
            initialized: false,
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Build a delay plugin in per-channel mode: each channel gets its own
    /// independent delay time. Used by the RoomEQ factored graph to encode
    /// all per-channel route delays in a single multichannel node.
    pub fn new_per_channel(channel_delays_ms: Vec<f32>) -> Result<Self, String> {
        let max_delay_ms = channel_delays_ms.iter().copied().fold(0.0_f32, f32::max);
        Self::new_per_channel_with_max_delay(channel_delays_ms, max_delay_ms)
    }

    /// Build a per-channel delay with an explicit allocation/automation range.
    pub fn new_per_channel_with_max_delay(
        channel_delays_ms: Vec<f32>,
        max_delay_ms: f32,
    ) -> Result<Self, String> {
        if channel_delays_ms.is_empty() {
            return Err("channel_delays_ms must not be empty".into());
        }
        if channel_delays_ms.len() > MAX_DELAY_CHANNELS {
            return Err(format!(
                "channel_delays_ms length must not exceed {MAX_DELAY_CHANNELS}, got {}",
                channel_delays_ms.len()
            ));
        }
        Self::validate_float("max_delay_ms", max_delay_ms, 0.0, MAX_DELAY_MS)?;
        for (channel, &delay_ms) in channel_delays_ms.iter().enumerate() {
            Self::validate_float(
                &format!("channel_delays_ms[{channel}]"),
                delay_ms,
                0.0,
                max_delay_ms,
            )?;
        }
        let channels = channel_delays_ms.len();
        let sr = 44100u32;
        let max_samples = Self::ring_samples(max_delay_ms, sr);
        let buffer_len = max_samples
            .checked_mul(channels)
            .ok_or_else(|| "delay buffer capacity overflow".to_string())?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(buffer_len)
            .map_err(|error| format!("failed to reserve delay buffer: {error}"))?;
        buffer.resize(buffer_len, 0.0);
        let smoothers: Vec<Smoother> = channel_delays_ms
            .iter()
            .map(|&ms| Smoother::new(ms * sr as f32 / 1000.0, 50.0, sr))
            .collect();
        let clean_delays: Vec<f32> = channel_delays_ms
            .iter()
            .map(|ms| ms * sr as f32 / 1000.0)
            .collect();
        let mut p = Self {
            channels,
            sample_rate: sr,
            param_delay_ms: ParameterId::from("delay_ms"),
            // Global delay_ms reports the channel-0 value for display.
            delay_ms: channel_delays_ms[0],
            param_feedback: ParameterId::from("feedback"),
            feedback: 0.0,
            param_mix: ParameterId::from("mix"),
            // Per-channel RoomEQ delays are dry: mix=1.0, no feedback.
            mix: 1.0,
            modulation: ModulationState {
                param_rate_hz: ParameterId::from("lfo_rate_hz"),
                rate_hz: 0.0,
                param_depth_ms: ParameterId::from("lfo_depth_ms"),
                depth_ms: 0.0,
                param_pitch_preserving: ParameterId::from("pitch_preserving"),
                pitch_preserving: false,
                phase: 0.0,
                clean_transition: CleanTransitionState {
                    current_delay_samples: clean_delays.clone(),
                    next_delay_samples: clean_delays,
                    fade_position: 0,
                    fade_samples: ((CLEAN_CROSSFADE_MS * sr as f32 / 1000.0).round() as usize)
                        .max(1),
                    active: false,
                },
            },
            param_allpass_feedback: ParameterId::from("allpass_feedback"),
            allpass_feedback: false,
            param_allpass_coeff: ParameterId::from("allpass_coeff"),
            allpass_coeff: 0.5,
            delay_smoother: Smoother::new(channel_delays_ms[0] * sr as f32 / 1000.0, 50.0, sr),
            feedback_smoother: Smoother::new(0.0, 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            channel_delays_ms,
            channel_delay_smoothers: smoothers,
            buffer,
            write_pos: 0,
            max_samples,
            max_delay_ms,
            allpass_states: vec![AllpassState::new(0.5); channels],
            allpass_mix_smoother: Smoother::new(0.0, ALLPASS_SMOOTH_MS, sr),
            allpass_coeff_smoother: Smoother::new(0.5, ALLPASS_SMOOTH_MS, sr),
            cached_parameters: Vec::new(),
            initialized: false,
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// True when the plugin is configured with independent per-channel delays.
    pub fn is_per_channel(&self) -> bool {
        !self.channel_delays_ms.is_empty()
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![
            Parameter::new_float(
                "delay_ms",
                "Delay Time",
                self.delay_ms,
                0.0,
                self.max_delay_ms,
            ),
            Parameter::new_float("feedback", "Feedback", self.feedback, -0.95, 0.95),
            Parameter::new_float("mix", "Mix", self.mix, 0.0, 1.0),
            Parameter::new_float(
                "lfo_rate_hz",
                "LFO Rate",
                self.modulation.rate_hz,
                0.0,
                20.0,
            )
            .with_unit("Hz"),
            Parameter::new_float(
                "lfo_depth_ms",
                "LFO Depth",
                self.modulation.depth_ms,
                0.0,
                10.0,
            )
            .with_unit("ms"),
            Parameter::new_float(
                "allpass_coeff",
                "Allpass Coeff",
                self.allpass_coeff,
                0.0,
                0.99,
            ),
            Parameter::new_bool(
                "allpass_feedback",
                "Allpass Feedback",
                self.allpass_feedback,
            ),
            Parameter::new_bool(
                "pitch_preserving",
                "Pitch Preserving",
                self.modulation.pitch_preserving,
            )
            .with_update_mode(UpdateMode::Structural),
        ];
        if self.is_per_channel() {
            for (ch, &ms) in self.channel_delays_ms.iter().enumerate() {
                let id = format!("delay_ms_{ch}");
                let name = format!("Delay Ch{ch}");
                params.push(
                    Parameter::new_float(&id, &name, ms, 0.0, self.max_delay_ms).with_unit("ms"),
                );
            }
        }
        self.cached_parameters = params;
    }

    pub fn from_params(channels: usize, params: DelayPluginParams) -> Result<Self, String> {
        Self::validate_params(channels, &params)?;
        if !params.channel_delays_ms.is_empty() {
            if params.feedback != 0.0
                || params.mix != 1.0
                || params.lfo_rate_hz != 0.0
                || params.lfo_depth_ms != 0.0
                || params.allpass_feedback
                || params.pitch_preserving
            {
                return Err(
                    "per-channel delay mode is a pure routing delay: feedback and LFO must be zero, mix must be one, and allpass/pitch-preserving modes must be disabled"
                        .into(),
                );
            }
            // Per-channel mode: the channels argument must match the
            // per-channel array length — drift here is a wiring bug that
            // produces silent buffer-size mismatches downstream, so error
            // out instead of silently using the array length.
            let expected = params.channel_delays_ms.len();
            if expected != channels {
                return Err(format!(
                    "DelayPlugin::from_params: channels arg ({channels}) does not match channel_delays_ms.len() ({expected})"
                ));
            }
            let mut p = Self::new_per_channel(params.channel_delays_ms.clone())?;
            p.allpass_coeff = params.allpass_coeff;
            for ap in &mut p.allpass_states {
                ap.set_coeff(p.allpass_coeff);
            }
            p.allpass_coeff_smoother.reset(p.allpass_coeff);
            p.rebuild_cached_parameters();
            Ok(p)
        } else {
            let mut p = Self::try_new(channels, params.delay_ms, params.feedback, params.mix)?;
            p.modulation.rate_hz = params.lfo_rate_hz;
            p.modulation.depth_ms = params.lfo_depth_ms;
            p.modulation.pitch_preserving = params.pitch_preserving;
            p.allpass_feedback = params.allpass_feedback;
            p.allpass_coeff = params.allpass_coeff;
            for ap in &mut p.allpass_states {
                ap.set_coeff(p.allpass_coeff);
            }
            p.allpass_mix_smoother
                .reset(if p.allpass_feedback { 1.0 } else { 0.0 });
            p.allpass_coeff_smoother.reset(p.allpass_coeff);
            p.rebuild_cached_parameters();
            Ok(p)
        }
    }

    /// 4-point Lagrange interpolation for fractional delay.
    ///
    /// Given 4 samples y[-1], y[0], y[1], y[2] around the desired read position,
    /// and a fractional part `frac` in [0, 1), interpolates between y[0] and y[1].
    #[inline]
    pub(super) fn lagrange4(y_m1: f32, y_0: f32, y_1: f32, y_2: f32, frac: f32) -> f32 {
        let d = frac;
        let dm1 = d - 1.0;
        let dm2 = d - 2.0;
        let dp1 = d + 1.0;

        let c0 = -dm1 * dm2 * d / 6.0;
        let c1 = dp1 * dm1 * dm2 / 2.0;
        let c2 = -dp1 * d * dm2 / 2.0;
        let c3 = dp1 * d * dm1 / 6.0;

        c0 * y_m1 + c1 * y_0 + c2 * y_1 + c3 * y_2
    }

    /// Read a sample from the delay buffer at a given position and channel.
    #[inline]
    pub(super) fn read_buffer(&self, pos: usize, ch: usize) -> f32 {
        self.buffer[ch * self.max_samples + pos]
    }

    #[inline]
    fn read_delayed_sample(&self, delay_samples: f32, ch: usize, input: f32) -> f32 {
        if delay_samples <= f32::EPSILON {
            return input;
        }
        let int_delay = delay_samples.floor() as usize;
        let frac = delay_samples - int_delay as f32;
        let mask = self.max_samples - 1;
        let r0 = (self.write_pos + self.max_samples - int_delay) & mask;
        if frac <= f32::EPSILON {
            return self.read_buffer(r0, ch);
        }
        let r_m1 = (r0 + 1) & mask;
        let r1 = (r0 + self.max_samples - 1) & mask;
        let r2 = (r1 + self.max_samples - 1) & mask;
        Self::lagrange4(
            self.read_buffer(r_m1, ch),
            self.read_buffer(r0, ch),
            self.read_buffer(r1, ch),
            self.read_buffer(r2, ch),
            frac,
        )
    }

    /// Compute the effective delay in samples for a given frame, including LFO modulation.
    #[inline]
    pub(super) fn effective_delay_samples(&self, base_delay_samples: f32, lfo_val: f32) -> f32 {
        let lfo_offset_samples =
            lfo_val * self.modulation.depth_ms * self.sample_rate as f32 / 1000.0;

        let min_delay = 0.0_f32;
        let max_delay = (self.max_delay_ms * self.sample_rate as f32 / 1000.0)
            .min((self.max_samples - 3) as f32);

        (base_delay_samples + lfo_offset_samples).clamp(min_delay, max_delay)
    }
}

impl ParametricInPlacePlugin for DelayPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Delay", env!("CARGO_PKG_VERSION"), "SotF")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Iir
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        // Delay is linear, but it is stateful and its transfer behavior can
        // vary within a block (smoothers, LFO, and live feedback/allpass
        // controls). Keep the linear classification for region selection while
        // making the scheduling/fusion contract conservative: a gain cannot be
        // moved across the delay state, and compiled plans must preserve this
        // node as an ordering boundary.
        let mut metadata = PluginCompileMetadata::linear_transform(
            PluginCostClass::Iir,
            None,
            0,
            false,
            true,
            false,
        );
        metadata.time_invariant_for_block = false;
        metadata.can_absorb_input_gain = false;
        metadata.can_absorb_output_gain = false;
        metadata.boundary = true;
        metadata
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        values.insert(
            ParameterId::from("delay_ms"),
            ParameterValue::Float(self.delay_ms),
        );
        values.insert(
            ParameterId::from("feedback"),
            ParameterValue::Float(self.feedback),
        );
        values.insert(ParameterId::from("mix"), ParameterValue::Float(self.mix));
        values.insert(
            ParameterId::from("lfo_rate_hz"),
            ParameterValue::Float(self.modulation.rate_hz),
        );
        values.insert(
            ParameterId::from("lfo_depth_ms"),
            ParameterValue::Float(self.modulation.depth_ms),
        );
        values.insert(
            ParameterId::from("pitch_preserving"),
            ParameterValue::Bool(self.modulation.pitch_preserving),
        );
        values.insert(
            ParameterId::from("allpass_feedback"),
            ParameterValue::Bool(self.allpass_feedback),
        );
        values.insert(
            ParameterId::from("allpass_coeff"),
            ParameterValue::Float(self.allpass_coeff),
        );
        if self.is_per_channel() {
            for (ch, &ms) in self.channel_delays_ms.iter().enumerate() {
                values.insert(
                    ParameterId::from(format!("delay_ms_{ch}")),
                    ParameterValue::Float(ms),
                );
            }
        }
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        let mut delay_ms = self.delay_ms;
        let mut feedback = self.feedback;
        let mut mix = self.mix;
        let mut pitch_preserving = self.modulation.pitch_preserving;
        let mut lfo_rate_hz = self.modulation.rate_hz;
        let mut lfo_depth_ms = self.modulation.depth_ms;
        let mut allpass_feedback = self.allpass_feedback;
        let mut allpass_coeff = self.allpass_coeff;
        let mut channel_delays_ms = self.channel_delays_ms.clone();
        for (id, value) in &values {
            self.parametric_validate_parameter(id, value)?;
            if id == &self.param_delay_ms {
                delay_ms = value
                    .as_float()
                    .ok_or_else(|| "delay_ms must be a float".to_string())?;
            } else if id == &self.param_feedback {
                feedback = value
                    .as_float()
                    .ok_or_else(|| "feedback must be a float".to_string())?;
            } else if id == &self.param_mix {
                mix = value
                    .as_float()
                    .ok_or_else(|| "mix must be a float".to_string())?;
            } else if id == &self.modulation.param_pitch_preserving {
                if self.initialized {
                    return Err(
                        "pitch_preserving is structural; rebuild the initialized plugin".into(),
                    );
                }
                pitch_preserving = value
                    .as_bool()
                    .ok_or_else(|| "pitch_preserving must be a bool".to_string())?;
            } else if id == &self.modulation.param_rate_hz {
                lfo_rate_hz = value
                    .as_float()
                    .ok_or_else(|| "lfo_rate_hz must be a float".to_string())?;
            } else if id == &self.modulation.param_depth_ms {
                lfo_depth_ms = value
                    .as_float()
                    .ok_or_else(|| "lfo_depth_ms must be a float".to_string())?;
            } else if id == &self.param_allpass_feedback {
                allpass_feedback = value
                    .as_bool()
                    .ok_or_else(|| "allpass_feedback must be a bool".to_string())?;
            } else if id == &self.param_allpass_coeff {
                allpass_coeff = value
                    .as_float()
                    .ok_or_else(|| "allpass_coeff must be a float".to_string())?;
            } else if let Some(ch) = parse_channel_delay_id(id.as_str()) {
                channel_delays_ms[ch] = value
                    .as_float()
                    .ok_or_else(|| "channel delay must be a float".to_string())?;
            }
        }
        if pitch_preserving && (lfo_rate_hz != 0.0 || lfo_depth_ms != 0.0) {
            return Err(
                "pitch_preserving mode requires lfo_rate_hz and lfo_depth_ms to be zero".into(),
            );
        }

        self.delay_ms = delay_ms;
        self.delay_smoother
            .set_target(delay_ms * self.sample_rate as f32 / 1000.0);
        self.feedback = feedback;
        self.feedback_smoother.set_target(feedback);
        self.mix = mix;
        self.mix_smoother.set_target(mix);
        self.modulation.pitch_preserving = pitch_preserving;
        self.modulation.rate_hz = lfo_rate_hz;
        self.modulation.depth_ms = lfo_depth_ms;
        self.allpass_feedback = allpass_feedback;
        self.allpass_mix_smoother
            .set_target(if allpass_feedback { 1.0 } else { 0.0 });
        self.allpass_coeff = allpass_coeff;
        self.allpass_coeff_smoother.set_target(allpass_coeff);
        self.channel_delays_ms = channel_delays_ms;
        for (smoother, &delay_ms) in self
            .channel_delay_smoothers
            .iter_mut()
            .zip(&self.channel_delays_ms)
        {
            smoother.set_target(delay_ms * self.sample_rate as f32 / 1000.0);
        }
        Ok(())
    }

    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        self.parametric_validate_parameter(&id, &value)?;
        self.apply_one_value(id, value)
    }

    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        if let Some(ch) = parse_channel_delay_id(id.as_str()) {
            if !self.is_per_channel() {
                return Err(format!("invalid per-channel delay id: {}", id.as_str()));
            }
            if ch >= self.channels {
                return Err(format!("Invalid channel index in {}", id));
            }
        }
        if let Some(param) = self.cached_parameters.iter().find(|p| &p.id == id) {
            param.validate(value).map_err(|e| format!("{}: {}", id, e))
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if !(1..=MAX_DELAY_SAMPLE_RATE).contains(&sample_rate) {
            return Err(format!(
                "sample rate must be in [1, {MAX_DELAY_SAMPLE_RATE}], got {sample_rate}"
            ));
        }
        if !(1..=MAX_DELAY_CHANNELS).contains(&self.channels) {
            return Err(format!(
                "channels must be in [1, {MAX_DELAY_CHANNELS}], got {}",
                self.channels
            ));
        }
        let max_samples = Self::ring_samples(self.max_delay_ms, sample_rate);
        let buffer_len = max_samples
            .checked_mul(self.channels)
            .ok_or_else(|| "delay buffer capacity overflow".to_string())?;
        if buffer_len > self.buffer.capacity() {
            self.buffer
                .try_reserve_exact(buffer_len - self.buffer.len())
                .map_err(|error| format!("failed to reserve delay buffer: {error}"))?;
        }
        self.sample_rate = sample_rate;
        self.max_samples = max_samples;
        self.buffer.resize(buffer_len, 0.0);
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.delay_smoother = Smoother::new(
            self.delay_ms * sample_rate as f32 / 1000.0,
            50.0,
            sample_rate,
        );
        if self.is_per_channel() {
            if self.channel_delay_smoothers.len() != self.channels {
                return Err("per-channel delay smoother state is inconsistent".into());
            }
            for (smoother, &delay_ms) in self
                .channel_delay_smoothers
                .iter_mut()
                .zip(&self.channel_delays_ms)
            {
                *smoother =
                    Smoother::new(delay_ms * sample_rate as f32 / 1000.0, 50.0, sample_rate);
            }
        }
        self.feedback_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        self.allpass_mix_smoother = Smoother::new(
            if self.allpass_feedback { 1.0 } else { 0.0 },
            ALLPASS_SMOOTH_MS,
            sample_rate,
        );
        self.allpass_coeff_smoother =
            Smoother::new(self.allpass_coeff, ALLPASS_SMOOTH_MS, sample_rate);
        if self.modulation.clean_transition.current_delay_samples.len() != self.channels
            || self.modulation.clean_transition.next_delay_samples.len() != self.channels
        {
            return Err("delay transition channel state is inconsistent".into());
        }
        for ch in 0..self.channels {
            let delay_ms = if self.is_per_channel() {
                self.channel_delays_ms[ch]
            } else {
                self.delay_ms
            };
            let delay_samples = delay_ms * sample_rate as f32 / 1000.0;
            self.modulation.clean_transition.current_delay_samples[ch] = delay_samples;
            self.modulation.clean_transition.next_delay_samples[ch] = delay_samples;
        }
        self.modulation.clean_transition.fade_position = 0;
        self.modulation.clean_transition.fade_samples =
            ((CLEAN_CROSSFADE_MS * sample_rate as f32 / 1000.0).round() as usize).max(1);
        self.modulation.clean_transition.active = false;
        self.modulation.phase = 0.0;
        if self.allpass_states.len() != self.channels {
            return Err("delay allpass channel state is inconsistent".into());
        }
        for state in &mut self.allpass_states {
            state.reset();
            state.set_coeff(self.allpass_coeff);
        }
        let feedback_target = self.feedback_smoother.target();
        self.feedback_smoother.reset(feedback_target);
        let mix_target = self.mix_smoother.target();
        self.mix_smoother.reset(mix_target);
        self.initialized = true;
        Ok(())
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.modulation.phase = 0.0;
        let global_target = self.delay_smoother.target();
        self.delay_smoother.reset(global_target);
        let fb_target = self.feedback_smoother.target();
        self.feedback_smoother.reset(fb_target);
        let mix_target = self.mix_smoother.target();
        self.mix_smoother.reset(mix_target);
        let allpass_mix_target = self.allpass_mix_smoother.target();
        self.allpass_mix_smoother.reset(allpass_mix_target);
        let allpass_coeff_target = self.allpass_coeff_smoother.target();
        self.allpass_coeff_smoother.reset(allpass_coeff_target);
        for s in &mut self.channel_delay_smoothers {
            let t = s.target();
            s.reset(t);
        }
        for ap in &mut self.allpass_states {
            ap.reset();
            ap.set_coeff(allpass_coeff_target);
        }
        for ch in 0..self.channels {
            let delay_ms = if self.is_per_channel() {
                self.channel_delays_ms[ch]
            } else {
                self.delay_ms
            };
            let delay_samples = delay_ms * self.sample_rate as f32 / 1000.0;
            self.modulation.clean_transition.current_delay_samples[ch] = delay_samples;
            self.modulation.clean_transition.next_delay_samples[ch] = delay_samples;
        }
        self.modulation.clean_transition.fade_position = 0;
        self.modulation.clean_transition.fade_samples =
            ((CLEAN_CROSSFADE_MS * self.sample_rate as f32 / 1000.0).round() as usize).max(1);
        self.modulation.clean_transition.active = false;
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;

        let expected_len = num_frames
            .checked_mul(self.channels)
            .ok_or_else(|| "delay buffer length overflow".to_string())?;
        if self.channels == 0 {
            return Err("delay requires at least one channel".into());
        }
        if buffer.len() != expected_len {
            return Err(format!(
                "delay expected {expected_len} samples for {num_frames} frames and {} channels, got {}",
                self.channels,
                buffer.len()
            ));
        }
        if self.max_samples == 0
            || !self.max_samples.is_power_of_two()
            || self.buffer.len() != self.max_samples.saturating_mul(self.channels)
            || self.allpass_states.len() != self.channels
        {
            return Err("delay internal channel/buffer state is inconsistent".into());
        }

        if !self.channel_delays_ms.is_empty()
            && (self.channel_delays_ms.len() != self.channels
                || self.channel_delay_smoothers.len() != self.channels)
        {
            return Err("per-channel delay arrays drifted out of sync".into());
        }

        let lfo_active =
            self.modulation.rate_hz > 0.0 && self.modulation.depth_ms > 0.0 && self.sample_rate > 0;
        let lfo_phase_inc = if lfo_active {
            self.modulation.rate_hz / self.sample_rate as f32
        } else {
            0.0
        };

        let per_channel = self.is_per_channel();

        for frame in 0..num_frames {
            let fb = self.feedback_smoother.advance();
            let mix = self.mix_smoother.advance();
            let allpass_mix = self.allpass_mix_smoother.advance();
            let allpass_coeff = self.allpass_coeff_smoother.advance();
            for state in &mut self.allpass_states {
                state.set_coeff(allpass_coeff);
            }

            let lfo_val = if lfo_active {
                let val = (self.modulation.phase * std::f32::consts::TAU).sin();
                self.modulation.phase += lfo_phase_inc;
                self.modulation.phase = self.modulation.phase.fract();
                val
            } else {
                0.0
            };

            let global_base_delay = if self.modulation.pitch_preserving {
                self.delay_smoother.target()
            } else {
                self.delay_smoother.advance()
            };

            if self.modulation.pitch_preserving && !self.modulation.clean_transition.active {
                let mut changed = false;
                for ch in 0..self.channels {
                    let base_delay_samples = if per_channel {
                        self.channel_delay_smoothers[ch].target()
                    } else {
                        global_base_delay
                    };
                    let desired = self.effective_delay_samples(base_delay_samples, 0.0);
                    self.modulation.clean_transition.next_delay_samples[ch] = desired;
                    changed |= (desired
                        - self.modulation.clean_transition.current_delay_samples[ch])
                        .abs()
                        > 1.0e-4;
                }
                if changed {
                    self.modulation.clean_transition.fade_position = 0;
                    self.modulation.clean_transition.active = true;
                    // Two differently delayed taps have an input-dependent
                    // phase relationship. No finite correlation window can
                    // prove that summing them will remain phase-safe, so every
                    // nonidentical transition conservatively switches through
                    // silence while both read heads stay stationary.
                }
            }

            let clean_fade =
                if self.modulation.pitch_preserving && self.modulation.clean_transition.active {
                    (self.modulation.clean_transition.fade_position + 1) as f32
                        / self.modulation.clean_transition.fade_samples as f32
                } else {
                    0.0
                };

            for ch in 0..self.channels {
                let base_delay_samples = if per_channel && !self.modulation.pitch_preserving {
                    self.channel_delay_smoothers[ch].advance()
                } else {
                    global_base_delay
                };
                let idx = frame * self.channels + ch;
                let input = buffer[idx];

                let delayed = if self.modulation.pitch_preserving {
                    let current_delay = self.modulation.clean_transition.current_delay_samples[ch];
                    let current = self.read_delayed_sample(current_delay, ch, input);
                    if self.modulation.clean_transition.active {
                        let next_delay = self.modulation.clean_transition.next_delay_samples[ch];
                        let next = self.read_delayed_sample(next_delay, ch, input);
                        if clean_fade < 0.5 {
                            current * (1.0 - 2.0 * clean_fade)
                        } else {
                            next * (2.0 * clean_fade - 1.0)
                        }
                    } else {
                        current
                    }
                } else {
                    let delay_samples = self.effective_delay_samples(base_delay_samples, lfo_val);
                    self.read_delayed_sample(delay_samples, ch, input)
                };

                let direct_feedback = delayed * fb;
                let allpass_feedback = self.allpass_states[ch].process(direct_feedback);
                let feedback_signal =
                    direct_feedback + allpass_mix * (allpass_feedback - direct_feedback);

                self.buffer[ch * self.max_samples + self.write_pos] = input + feedback_signal;
                buffer[idx] = input * (1.0 - mix) + delayed * mix;
            }
            if self.modulation.pitch_preserving && self.modulation.clean_transition.active {
                self.modulation.clean_transition.fade_position += 1;
                if self.modulation.clean_transition.fade_position
                    >= self.modulation.clean_transition.fade_samples
                {
                    self.modulation
                        .clean_transition
                        .current_delay_samples
                        .copy_from_slice(&self.modulation.clean_transition.next_delay_samples);
                    self.modulation.clean_transition.fade_position = 0;
                    self.modulation.clean_transition.active = false;
                }
            }
            self.write_pos = (self.write_pos + 1) & (self.max_samples - 1);
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }
}

impl DelayPlugin {
    fn apply_one_value(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_delay_ms {
            let v = value
                .as_float()
                .ok_or_else(|| "delay_ms must be a float".to_string())?;
            if v.is_finite() {
                self.delay_ms = v;
                self.delay_smoother
                    .set_target(self.delay_ms * self.sample_rate as f32 / 1000.0);
            }
        } else if id == self.param_feedback {
            let v = value
                .as_float()
                .ok_or_else(|| "feedback must be a float".to_string())?;
            if v.is_finite() {
                self.feedback = v;
                self.feedback_smoother.set_target(self.feedback);
            }
        } else if id == self.param_mix {
            let v = value
                .as_float()
                .ok_or_else(|| "mix must be a float".to_string())?;
            if v.is_finite() {
                self.mix = v;
                self.mix_smoother.set_target(self.mix);
            }
        } else if id == self.modulation.param_rate_hz {
            let v = value
                .as_float()
                .ok_or_else(|| "lfo_rate_hz must be a float".to_string())?;
            if v.is_finite() {
                if self.modulation.pitch_preserving && v != 0.0 {
                    return Err("pitch_preserving mode requires lfo_rate_hz to remain zero".into());
                }
                self.modulation.rate_hz = v;
            }
        } else if id == self.modulation.param_depth_ms {
            let v = value
                .as_float()
                .ok_or_else(|| "lfo_depth_ms must be a float".to_string())?;
            if v.is_finite() {
                if self.modulation.pitch_preserving && v != 0.0 {
                    return Err("pitch_preserving mode requires lfo_depth_ms to remain zero".into());
                }
                self.modulation.depth_ms = v;
            }
        } else if id == self.modulation.param_pitch_preserving {
            if self.initialized {
                return Err(
                    "pitch_preserving is structural; rebuild the initialized plugin".into(),
                );
            }
            let v = value
                .as_bool()
                .ok_or_else(|| "pitch_preserving must be a bool".to_string())?;
            if v && (self.modulation.rate_hz != 0.0 || self.modulation.depth_ms != 0.0) {
                return Err(
                    "pitch_preserving mode requires lfo_rate_hz and lfo_depth_ms to be zero".into(),
                );
            }
            self.modulation.pitch_preserving = v;
        } else if id == self.param_allpass_feedback {
            let v = value
                .as_bool()
                .ok_or_else(|| "allpass_feedback must be a bool".to_string())?;
            self.allpass_feedback = v;
            self.allpass_mix_smoother
                .set_target(if v { 1.0 } else { 0.0 });
        } else if id == self.param_allpass_coeff {
            let v = value
                .as_float()
                .ok_or_else(|| "allpass_coeff must be a float".to_string())?;
            if v.is_finite() {
                self.allpass_coeff = v.clamp(0.0, 0.99);
                self.allpass_coeff_smoother.set_target(self.allpass_coeff);
            }
        } else if let Some(ch) = parse_channel_delay_id(id.as_str()) {
            if !self.is_per_channel() || ch >= self.channels {
                return Err(format!("invalid per-channel delay id: {}", id.as_str()));
            }
            let v = value
                .as_float()
                .ok_or_else(|| "channel delay must be a float".to_string())?;
            if v.is_finite() && v >= 0.0 {
                self.channel_delays_ms[ch] = v;
                let target_samples = v * self.sample_rate as f32 / 1000.0;
                if ch < self.channel_delay_smoothers.len() {
                    self.channel_delay_smoothers[ch].set_target(target_samples);
                }
            }
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }
}
