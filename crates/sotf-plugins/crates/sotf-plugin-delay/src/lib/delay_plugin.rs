use super::allpass_state::AllpassState;
use super::misc::MAX_DELAY_MS;
use super::misc::parse_channel_delay_id;
use super::types::DelayPluginParams;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;

pub struct DelayPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    pub(super) param_delay_ms: ParameterId,
    pub(super) delay_ms: f32,
    pub(super) param_feedback: ParameterId,
    pub(super) feedback: f32,
    pub(super) param_mix: ParameterId,
    pub(super) mix: f32,
    pub(super) param_lfo_rate_hz: ParameterId,
    pub(super) lfo_rate_hz: f32,
    pub(super) param_lfo_depth_ms: ParameterId,
    pub(super) lfo_depth_ms: f32,
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
    /// LFO phase accumulator (0..1)
    pub(super) lfo_phase: f32,
    /// Per-channel allpass filter states for the feedback path
    pub(super) allpass_states: Vec<AllpassState>,
    pub(super) cached_parameters: Vec<Parameter>,
}

impl DelayPlugin {
    pub fn new(channels: usize, delay_ms: f32, feedback: f32, mix: f32) -> Self {
        let sr = 44100;
        // Round to next power-of-two so modulo in read positions can be replaced
        // with a bitmask by the compiler (the buffer is used with % max_samples).
        let max_samples = ((MAX_DELAY_MS * 0.001 * sr as f32) as usize + 4).next_power_of_two();
        let mut p = Self {
            channels,
            sample_rate: sr,
            param_delay_ms: ParameterId::from("delay_ms"),
            delay_ms,
            param_feedback: ParameterId::from("feedback"),
            feedback,
            param_mix: ParameterId::from("mix"),
            mix,
            param_lfo_rate_hz: ParameterId::from("lfo_rate_hz"),
            lfo_rate_hz: 0.0,
            param_lfo_depth_ms: ParameterId::from("lfo_depth_ms"),
            lfo_depth_ms: 0.0,
            param_allpass_feedback: ParameterId::from("allpass_feedback"),
            allpass_feedback: false,
            param_allpass_coeff: ParameterId::from("allpass_coeff"),
            allpass_coeff: 0.5,
            delay_smoother: Smoother::new(delay_ms * sr as f32 / 1000.0, 50.0, sr),
            feedback_smoother: Smoother::new(feedback, 5.0, sr),
            mix_smoother: Smoother::new(mix, 5.0, sr),
            channel_delays_ms: Vec::new(),
            channel_delay_smoothers: Vec::new(),
            buffer: vec![0.0; max_samples * channels],
            write_pos: 0,
            max_samples,
            lfo_phase: 0.0,
            allpass_states: vec![AllpassState::new(0.5); channels],
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Build a delay plugin in per-channel mode: each channel gets its own
    /// independent delay time. Used by the RoomEQ factored graph to encode
    /// all per-channel route delays in a single multichannel node.
    pub fn new_per_channel(channel_delays_ms: Vec<f32>) -> Result<Self, String> {
        if channel_delays_ms.is_empty() {
            return Err("channel_delays_ms must not be empty".into());
        }
        let channels = channel_delays_ms.len();
        let sr = 44100u32;
        let max_samples = ((MAX_DELAY_MS * 0.001 * sr as f32) as usize + 4).next_power_of_two();
        let smoothers: Vec<Smoother> = channel_delays_ms
            .iter()
            .map(|&ms| Smoother::new(ms * sr as f32 / 1000.0, 50.0, sr))
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
            param_lfo_rate_hz: ParameterId::from("lfo_rate_hz"),
            lfo_rate_hz: 0.0,
            param_lfo_depth_ms: ParameterId::from("lfo_depth_ms"),
            lfo_depth_ms: 0.0,
            param_allpass_feedback: ParameterId::from("allpass_feedback"),
            allpass_feedback: false,
            param_allpass_coeff: ParameterId::from("allpass_coeff"),
            allpass_coeff: 0.5,
            delay_smoother: Smoother::new(channel_delays_ms[0] * sr as f32 / 1000.0, 50.0, sr),
            feedback_smoother: Smoother::new(0.0, 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            channel_delays_ms,
            channel_delay_smoothers: smoothers,
            buffer: vec![0.0; max_samples * channels],
            write_pos: 0,
            max_samples,
            lfo_phase: 0.0,
            allpass_states: vec![AllpassState::new(0.5); channels],
            cached_parameters: Vec::new(),
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
            Parameter::new_float("delay_ms", "Delay Time", self.delay_ms, 0.1, MAX_DELAY_MS),
            Parameter::new_float("feedback", "Feedback", self.feedback, 0.0, 0.95),
            Parameter::new_float("mix", "Mix", self.mix, 0.0, 1.0),
            Parameter::new_float("lfo_rate_hz", "LFO Rate", self.lfo_rate_hz, 0.0, 10.0)
                .with_unit("Hz"),
            Parameter::new_float("lfo_depth_ms", "LFO Depth", self.lfo_depth_ms, 0.0, 5.0)
                .with_unit("ms"),
            Parameter::new_bool(
                "allpass_feedback",
                "Allpass Feedback",
                self.allpass_feedback,
            ),
            Parameter::new_float(
                "allpass_coeff",
                "Allpass Coeff",
                self.allpass_coeff,
                0.0,
                0.99,
            ),
        ];
        if self.is_per_channel() {
            for (ch, &ms) in self.channel_delays_ms.iter().enumerate() {
                let id = format!("delay_ms_{ch}");
                let name = format!("Delay Ch{ch}");
                params
                    .push(Parameter::new_float(&id, &name, ms, 0.0, MAX_DELAY_MS).with_unit("ms"));
            }
        }
        self.cached_parameters = params;
    }

    pub fn from_params(channels: usize, params: DelayPluginParams) -> Result<Self, String> {
        if !params.channel_delays_ms.is_empty() {
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
            p.feedback = params.feedback;
            p.mix = params.mix;
            p.lfo_rate_hz = params.lfo_rate_hz;
            p.lfo_depth_ms = params.lfo_depth_ms;
            p.allpass_feedback = params.allpass_feedback;
            p.allpass_coeff = params.allpass_coeff.clamp(0.0, 0.99);
            for ap in &mut p.allpass_states {
                ap.set_coeff(p.allpass_coeff);
            }
            p.feedback_smoother.set_target(p.feedback);
            p.mix_smoother.set_target(p.mix);
            p.rebuild_cached_parameters();
            Ok(p)
        } else {
            let mut p = Self::new(channels, params.delay_ms, params.feedback, params.mix);
            p.lfo_rate_hz = params.lfo_rate_hz;
            p.lfo_depth_ms = params.lfo_depth_ms;
            p.allpass_feedback = params.allpass_feedback;
            p.allpass_coeff = params.allpass_coeff.clamp(0.0, 0.99);
            for ap in &mut p.allpass_states {
                ap.set_coeff(p.allpass_coeff);
            }
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

    /// Compute the effective delay in samples for a given frame, including LFO modulation.
    #[inline]
    pub(super) fn effective_delay_samples(&self, base_delay_samples: f32, lfo_val: f32) -> f32 {
        let mut lfo_offset_samples = lfo_val * self.lfo_depth_ms * self.sample_rate as f32 / 1000.0;

        let min_delay = 1.0_f32;
        let max_delay = (self.max_samples - 3) as f32;

        let headroom_down = (base_delay_samples - min_delay).max(0.0);
        let headroom_up = (max_delay - base_delay_samples).max(0.0);
        let max_lfo_depth = headroom_down.min(headroom_up);
        if max_lfo_depth.is_finite() && max_lfo_depth < lfo_offset_samples.abs() {
            let sign = if lfo_offset_samples.is_sign_negative() {
                -1.0
            } else {
                1.0
            };
            lfo_offset_samples = sign * max_lfo_depth;
        }

        (base_delay_samples + lfo_offset_samples).clamp(min_delay, max_delay)
    }
}

impl ParametricInPlacePlugin for DelayPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Delay", "2.0.0", "SotF")
    }
    fn channels(&self) -> usize {
        self.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        values.insert(ParameterId::from("delay_ms"), ParameterValue::Float(self.delay_ms));
        values.insert(ParameterId::from("feedback"), ParameterValue::Float(self.feedback));
        values.insert(ParameterId::from("mix"), ParameterValue::Float(self.mix));
        values.insert(ParameterId::from("lfo_rate_hz"), ParameterValue::Float(self.lfo_rate_hz));
        values.insert(
            ParameterId::from("lfo_depth_ms"),
            ParameterValue::Float(self.lfo_depth_ms),
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
                values.insert(ParameterId(format!("delay_ms_{ch}")), ParameterValue::Float(ms));
            }
        }
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
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
            } else if id == self.param_lfo_rate_hz {
                let v = value
                    .as_float()
                    .ok_or_else(|| "lfo_rate_hz must be a float".to_string())?;
                if v.is_finite() {
                    self.lfo_rate_hz = v;
                }
            } else if id == self.param_lfo_depth_ms {
                let v = value
                    .as_float()
                    .ok_or_else(|| "lfo_depth_ms must be a float".to_string())?;
                if v.is_finite() {
                    self.lfo_depth_ms = v;
                }
            } else if id == self.param_allpass_feedback {
                let v = value
                    .as_bool()
                    .ok_or_else(|| "allpass_feedback must be a bool".to_string())?;
                self.allpass_feedback = v;
                if !v {
                    // Reset allpass states when disabling
                    for ap in &mut self.allpass_states {
                        ap.reset();
                    }
                }
            } else if id == self.param_allpass_coeff {
                let v = value
                    .as_float()
                    .ok_or_else(|| "allpass_coeff must be a float".to_string())?;
                if v.is_finite() {
                    self.allpass_coeff = v.clamp(0.0, 0.99);
                    for ap in &mut self.allpass_states {
                        ap.set_coeff(self.allpass_coeff);
                    }
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
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        // Per-channel delay IDs are dynamic: validate mode/channel bounds first, then
        // validate against the matching schema entry (which includes NaN/finite checks).
        if let Some(ch) = parse_channel_delay_id(id.as_str()) {
            if !self.is_per_channel() {
                return Err(format!("invalid per-channel delay id: {}", id.as_str()));
            }
            if ch >= self.channels {
                return Err(format!("Invalid channel index in {}", id));
            }
        }
        if let Some(param) = self.parameter_schema().iter().find(|p| &p.id == id) {
            param.validate(value).map_err(|e| format!("{}: {}", id, e))
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        // Round to next power-of-two (see new() for rationale).
        self.max_samples =
            ((MAX_DELAY_MS * 0.001 * sample_rate as f32) as usize + 4).next_power_of_two();
        self.buffer.resize(self.max_samples * self.channels, 0.0);
        self.delay_smoother = Smoother::new(
            self.delay_ms * sample_rate as f32 / 1000.0,
            50.0,
            sample_rate,
        );
        if self.is_per_channel() {
            self.channel_delay_smoothers = self
                .channel_delays_ms
                .iter()
                .map(|&ms| Smoother::new(ms * sample_rate as f32 / 1000.0, 50.0, sample_rate))
                .collect();
        }
        self.feedback_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        self.lfo_phase = 0.0;
        self.allpass_states = vec![AllpassState::new(self.allpass_coeff); self.channels];
        Ok(())
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.lfo_phase = 0.0;
        // Snap smoothers to their targets so a reset followed by process
        // starts at steady state instead of ramping from the smoother's
        // previous mid-flight value (which causes ~50 ms of pitch glitch
        // and makes per-channel delay testing flaky).
        let global_target = self.delay_smoother.target();
        self.delay_smoother.reset(global_target);
        let fb_target = self.feedback_smoother.target();
        self.feedback_smoother.reset(fb_target);
        let mix_target = self.mix_smoother.target();
        self.mix_smoother.reset(mix_target);
        for s in &mut self.channel_delay_smoothers {
            let t = s.target();
            s.reset(t);
        }
        for ap in &mut self.allpass_states {
            ap.reset();
        }
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;

        // Invariant: when in per-channel mode, the smoother and dB arrays
        // are parallel-sized. A drift here means a bug elsewhere — surface
        // it in debug builds before the indexing in the inner loop ends up
        // panicking with a less helpful message.
        debug_assert_eq!(
            self.channel_delays_ms.len(),
            self.channel_delay_smoothers.len(),
            "per-channel delay arrays drifted out of sync"
        );

        let lfo_active = self.lfo_rate_hz > 0.0 && self.lfo_depth_ms > 0.0 && self.sample_rate > 0;
        let lfo_phase_inc = if lfo_active {
            self.lfo_rate_hz / self.sample_rate as f32
        } else {
            0.0
        };

        let per_channel = self.is_per_channel();

        for frame in 0..num_frames {
            // Advance smoothers once per sample so parameter changes ramp smoothly
            // instead of jumping at block boundaries (prevents zipper noise / pitch glitch).
            let fb = self.feedback_smoother.advance();
            let mix = self.mix_smoother.advance();

            // Compute per-sample LFO value (sine, range -1..+1)
            let lfo_val = if lfo_active {
                let val = (self.lfo_phase * std::f32::consts::TAU).sin();
                self.lfo_phase += lfo_phase_inc;
                // Use fract() to correctly handle phase wrap even if increment > 1.0
                self.lfo_phase = self.lfo_phase.fract();
                val
            } else {
                0.0
            };

            // Keep the global smoother moving once per frame. In per-channel
            // mode the value is unused, but it still tracks its target so its
            // time constant stays consistent if callers switch back.
            let global_base_delay = self.delay_smoother.advance();

            for ch in 0..self.channels {
                let base_delay_samples = if per_channel {
                    // Advance per-channel smoother once per frame.
                    self.channel_delay_smoothers[ch].advance()
                } else {
                    global_base_delay
                };
                let delay_samples = self.effective_delay_samples(base_delay_samples, lfo_val);
                let int_delay = delay_samples.floor() as usize;
                let frac = delay_samples - int_delay as f32;

                let idx = frame * self.channels + ch;
                let input = buffer[idx];

                // 4-point Lagrange interpolation read positions:
                // We want samples at positions: int_delay-1, int_delay, int_delay+1, int_delay+2
                // relative to write_pos (going backwards in time)
                let mask = self.max_samples - 1;
                let r0 = (self.write_pos + self.max_samples - int_delay) & mask;
                let r_m1 = (r0 + 1) & mask;
                let r1 = (r0 + self.max_samples - 1) & mask;
                let r2 = (r1 + self.max_samples - 1) & mask;

                let y_m1 = self.read_buffer(r_m1, ch);
                let y_0 = self.read_buffer(r0, ch);
                let y_1 = self.read_buffer(r1, ch);
                let y_2 = self.read_buffer(r2, ch);

                let delayed = Self::lagrange4(y_m1, y_0, y_1, y_2, frac);

                // Feedback signal, optionally through allpass filter
                let feedback_signal = if self.allpass_feedback {
                    self.allpass_states[ch].process(delayed * fb)
                } else {
                    delayed * fb
                };

                self.buffer[ch * self.max_samples + self.write_pos] = input + feedback_signal;
                buffer[idx] = input * (1.0 - mix) + delayed * mix;
            }
            self.write_pos = (self.write_pos + 1) & (self.max_samples - 1);
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }
}
