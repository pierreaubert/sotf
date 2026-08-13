use super::types::{ChannelMuteSoloParams, ChannelState, default_dim_gain_db, default_fade_ms};
use crate::params::PARAMS;
use sotf_host::param_specs::find_by_key as param_by_key;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use sotf_host::simd::apply_per_channel_gain_simd;
use sotf_host::smoothing::Smoother;

/// Channel mute/solo plugin
///
/// Allows muting or soloing individual channels in a multi-channel stream.
pub struct ChannelMuteSoloPlugin {
    /// Number of channels
    pub(super) channels: usize,
    /// Whether the plugin is enabled (if false, audio passes through unchanged)
    ///
    /// When false, all channels are faded toward unity gain (bypass),
    /// ignoring per-channel mute/solo/dim state until enabled again.
    pub(super) enabled: bool,
    /// Per-channel mute/solo state
    pub(super) channel_states: Vec<ChannelState>,
    /// Per-channel gain smoothers for click-free mute/solo/dim transitions
    pub(super) channel_smoothers: Vec<Smoother>,
    /// Sample rate for smoother initialization
    pub(super) sample_rate: u32,
    /// Dim gain in dB (e.g. -20.0 means dimmed channels are attenuated by 20dB)
    pub(super) dim_gain_db: f32,
    /// Dim gain as linear multiplier (cached from dim_gain_db)
    pub(super) dim_gain_linear: f32,
    /// One-pole time constant in ms for mute/solo/dim transitions
    pub(super) fade_ms: f32,
    /// Parameter ID for enabled flag
    pub(super) param_enabled: ParameterId,
    /// Parameter ID for channel states (JSON)
    pub(super) param_channel_states: ParameterId,
    /// Parameter ID for dim gain in dB
    pub(super) param_dim_gain_db: ParameterId,
    /// Parameter ID for fade time in ms
    pub(super) param_fade_ms: ParameterId,
    /// Cached parameter descriptors — rebuilt lazily when `params_dirty` is true.
    /// `parameters()` takes `&self`, so we use `std::cell::Cell` + `std::cell::RefCell`
    /// for interior mutability to avoid rebuilding on every individual toggle.
    pub(super) cached_parameters: std::cell::RefCell<Vec<Parameter>>,
    /// Dirty flag — set when any state change could affect cached_parameters.
    pub(super) params_dirty: std::cell::Cell<bool>,
    /// Test-only count of channel-state JSON serializations performed for schema refreshes.
    #[cfg(test)]
    pub(super) schema_state_serializations: std::cell::Cell<usize>,
    /// Cache for SIMD optimization
    pub(super) cached_gains: Vec<f32>,
    /// Test-only proof that a converged block used the static-gain kernel.
    #[cfg(test)]
    pub(super) static_path_blocks: usize,
}

impl ChannelMuteSoloPlugin {
    /// Create a new channel mute/solo plugin
    pub fn new(channels: usize, enabled: bool) -> Self {
        let channel_states = vec![ChannelState::default(); channels];
        let sample_rate = 48000;
        let dim_gain_db = default_dim_gain_db();
        let fade_ms = default_fade_ms();
        let channel_smoothers = vec![Smoother::new(1.0, fade_ms, sample_rate); channels];
        let p = Self {
            channels,
            enabled,
            channel_states,
            channel_smoothers,
            sample_rate,
            dim_gain_db,
            dim_gain_linear: Self::db_to_linear(dim_gain_db),
            fade_ms,
            param_enabled: ParameterId::from("enabled"),
            param_channel_states: ParameterId::from("channel_states"),
            param_dim_gain_db: ParameterId::from("dim_gain_db"),
            param_fade_ms: ParameterId::from("fade_ms"),
            cached_parameters: std::cell::RefCell::new(Vec::new()),
            params_dirty: std::cell::Cell::new(true),
            #[cfg(test)]
            schema_state_serializations: std::cell::Cell::new(0),
            cached_gains: vec![1.0; channels],
            #[cfg(test)]
            static_path_blocks: 0,
        };
        // Build initial cache so validate_parameter works before any set_parameter call.
        p.rebuild_cached_parameters_if_dirty();
        p
    }

    /// Create a new channel mute/solo plugin from configuration parameters
    pub fn from_params(channels: usize, params: ChannelMuteSoloParams) -> Self {
        let mut plugin = Self::new(channels, params.enabled);

        plugin.set_dim_gain_db(params.dim_gain_db);
        plugin.set_fade_ms(params.fade_ms);

        // Accept mismatched lengths: truncate if too many states, pad with defaults if too few.
        let mut states = params.channel_states;
        states.truncate(channels);
        states.resize(channels, ChannelState::default());
        plugin.channel_states = states;

        plugin.reset_smoothers_to_current();
        plugin.mark_params_dirty();
        plugin
    }

    pub fn try_from_params(channels: usize, params: ChannelMuteSoloParams) -> PluginResult<Self> {
        if channels == 0 {
            return Err("Channel Mute/Solo requires at least one channel".to_string());
        }
        if !params.dim_gain_db.is_finite() || !(-60.0..=0.0).contains(&params.dim_gain_db) {
            return Err(format!("Invalid dim_gain_db: {}", params.dim_gain_db));
        }
        if !params.fade_ms.is_finite() || !(0.0..=100.0).contains(&params.fade_ms) {
            return Err(format!("Invalid fade_ms: {}", params.fade_ms));
        }
        Ok(Self::from_params(channels, params))
    }

    /// Set whether the plugin is enabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.update_smoother_targets();
        self.mark_params_dirty();
    }

    /// Get whether the plugin is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the state for a specific channel
    pub fn set_channel_state(
        &mut self,
        channel: usize,
        muted: bool,
        soloed: bool,
        dimmed: bool,
    ) -> Result<(), String> {
        if channel >= self.channels {
            return Err(format!(
                "channel {} out of bounds ({})",
                channel, self.channels
            ));
        }
        self.channel_states[channel] = ChannelState {
            muted,
            soloed,
            dimmed,
        };
        self.update_smoother_targets();
        self.mark_params_dirty();
        Ok(())
    }

    /// Set all channel states at once
    pub fn set_channel_states(&mut self, states: &[ChannelState]) -> PluginResult<()> {
        if states.len() != self.channels {
            return Err(format!(
                "channel state count mismatch: expected {}, got {}",
                self.channels,
                states.len()
            ));
        }
        self.channel_states.clone_from_slice(states);
        self.update_smoother_targets();
        self.mark_params_dirty();
        Ok(())
    }

    /// Get the state for a specific channel
    pub fn get_channel_state(&self, channel: usize) -> Option<&ChannelState> {
        self.channel_states.get(channel)
    }

    /// Set the dim gain in dB
    pub fn set_dim_gain_db(&mut self, db: f32) {
        self.dim_gain_db = db;
        self.dim_gain_linear = Self::db_to_linear(db);
        self.update_smoother_targets();
        self.mark_params_dirty();
    }

    /// Get the dim gain in dB
    pub fn dim_gain_db(&self) -> f32 {
        self.dim_gain_db
    }

    /// Set the fade time in ms
    pub fn set_fade_ms(&mut self, ms: f32) {
        self.fade_ms = ms;
        for smoother in &mut self.channel_smoothers {
            smoother.set_time(ms, self.sample_rate);
        }
        self.mark_params_dirty();
    }

    /// Get the fade time in ms
    pub fn fade_ms(&self) -> f32 {
        self.fade_ms
    }

    #[inline]
    pub(super) fn db_to_linear(db: f32) -> f32 {
        sotf_host::db_to_linear(db)
    }

    /// Mark cached parameters as stale; they will be rebuilt lazily in `parameters()`.
    #[inline]
    pub(super) fn mark_params_dirty(&self) {
        self.params_dirty.set(true);
    }

    /// Refresh cached parameter values if the dirty flag is set.
    ///
    /// Descriptor IDs, names, groups, and ranges are constructed once. Later
    /// schema requests update only the value-bearing fields, so a routing
    /// toggle never reformats every per-channel descriptor.
    pub(super) fn rebuild_cached_parameters_if_dirty(&self) {
        if !self.params_dirty.get() {
            return;
        }
        let mut cached = self.cached_parameters.borrow_mut();
        #[cfg(test)]
        self.schema_state_serializations
            .set(self.schema_state_serializations.get() + 1);
        let channel_states_json = serde_json::to_string(&self.channel_states).unwrap_or_default();
        if cached.is_empty() {
            let dim_spec = param_by_key(PARAMS, "dim_gain_db");
            let fade_spec = param_by_key(PARAMS, "fade_ms");
            *cached = vec![
                Parameter::new_bool("enabled", "Enabled", self.enabled)
                    .with_description("Enable/disable the plugin")
                    .with_group("General")
                    .with_importance(ParameterImportance::Critical),
                Parameter::new_string("channel_states", "Channel States", channel_states_json)
                    .with_description("Per-channel mute/solo/dim states (JSON)")
                    .with_group("General"),
                Parameter::new_float(
                    "dim_gain_db",
                    dim_spec.name,
                    self.dim_gain_db,
                    dim_spec.min_f64() as f32,
                    dim_spec.max_f64() as f32,
                )
                .with_description(dim_spec.doc)
                .with_group(dim_spec.group),
                Parameter::new_float(
                    "fade_ms",
                    fade_spec.name,
                    self.fade_ms,
                    fade_spec.min_f64() as f32,
                    fade_spec.max_f64() as f32,
                )
                .with_description(fade_spec.doc)
                .with_group(fade_spec.group),
            ];
            cached.reserve(self.channels.saturating_mul(3));
            for ch in 0..self.channels {
                cached.push(
                    Parameter::new_bool(
                        &format!("mute_{ch}"),
                        &format!("Mute Ch{ch}"),
                        self.channel_states[ch].muted,
                    )
                    .with_group("Per-Channel"),
                );
                cached.push(
                    Parameter::new_bool(
                        &format!("solo_{ch}"),
                        &format!("Solo Ch{ch}"),
                        self.channel_states[ch].soloed,
                    )
                    .with_group("Per-Channel"),
                );
                cached.push(
                    Parameter::new_bool(
                        &format!("dim_{ch}"),
                        &format!("Dim Ch{ch}"),
                        self.channel_states[ch].dimmed,
                    )
                    .with_group("Per-Channel"),
                );
            }
        } else {
            debug_assert_eq!(cached.len(), 4 + self.channels * 3);
            cached[0].default_value = ParameterValue::Bool(self.enabled);
            cached[1].default_value = ParameterValue::String(channel_states_json);
            cached[2].default_value = ParameterValue::Float(self.dim_gain_db);
            cached[3].default_value = ParameterValue::Float(self.fade_ms);
            for (channel, state) in self.channel_states.iter().enumerate() {
                let base = 4 + channel * 3;
                cached[base].default_value = ParameterValue::Bool(state.muted);
                cached[base + 1].default_value = ParameterValue::Bool(state.soloed);
                cached[base + 2].default_value = ParameterValue::Bool(state.dimmed);
            }
        }
        self.params_dirty.set(false);
    }

    #[inline]
    fn smoothers_are_settled(&self) -> bool {
        self.channel_smoothers
            .iter()
            .all(|smoother| (smoother.current() - smoother.target()).abs() < 1.0e-5)
    }

    /// Recompute smoother targets based on current channel states
    pub(super) fn update_smoother_targets(&mut self) {
        let has_solo = self.channel_states.iter().any(|s| s.soloed);
        for (ch, state) in self.channel_states.iter().enumerate() {
            let target = if !self.enabled {
                1.0
            } else {
                self.compute_channel_gain(state, has_solo)
            };
            self.channel_smoothers[ch].set_target(target);
        }
    }

    /// Reset smoothers to current state immediately
    pub(super) fn reset_smoothers_to_current(&mut self) {
        let has_solo = self.channel_states.iter().any(|s| s.soloed);
        for (ch, state) in self.channel_states.iter().enumerate() {
            let target = if !self.enabled {
                1.0
            } else {
                self.compute_channel_gain(state, has_solo)
            };
            self.channel_smoothers[ch].reset(target);
        }
    }

    /// Compute the target gain for a channel given its state
    pub(super) fn compute_channel_gain(&self, state: &ChannelState, has_solo: bool) -> f32 {
        if has_solo {
            if state.soloed { 1.0 } else { 0.0 }
        } else if state.muted {
            0.0
        } else if state.dimmed {
            self.dim_gain_linear
        } else {
            1.0
        }
    }
}

impl ParametricInPlacePlugin for ChannelMuteSoloPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Channel Mute/Solo", "1.1.0", "SotF")
            .with_description("Mute or solo individual channels (Optimized & Smoothed)")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Scalar
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata = PluginCompileMetadata::routing(
            PluginCostClass::Scalar,
            Some(PluginCompiledOp::ChannelMuteSolo),
            false,
        );
        metadata.stateful = !self.smoothers_are_settled();
        metadata.time_invariant_for_block = !metadata.stateful;
        metadata
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        // IDs, types, and bounds are immutable after construction. Validate
        // against the cached descriptors directly so consecutive adapter
        // updates do not refresh/serialize dirty current values merely to
        // inspect an unchanged schema.
        if let Some(parameter) = self
            .cached_parameters
            .borrow()
            .iter()
            .find(|parameter| &parameter.id == id)
        {
            parameter
                .validate(value)
                .map_err(|error| format!("{id}: {error}"))
        } else {
            Err(format!("Unknown parameter: {id}"))
        }
    }

    fn parameter_schema(&self) -> ParameterSchema {
        self.rebuild_cached_parameters_if_dirty();
        self.cached_parameters.borrow().clone()
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        for param in self.cached_parameters.borrow().iter() {
            let value = if param.id == self.param_enabled {
                ParameterValue::Bool(self.enabled)
            } else if param.id == self.param_channel_states {
                serde_json::to_string(&self.channel_states)
                    .ok()
                    .map(ParameterValue::String)
                    .unwrap_or_else(|| ParameterValue::String(String::new()))
            } else if param.id == self.param_dim_gain_db {
                ParameterValue::Float(self.dim_gain_db)
            } else if param.id == self.param_fade_ms {
                ParameterValue::Float(self.fade_ms)
            } else if let Some(rest) = param.id.0.strip_prefix("mute_") {
                rest.parse::<usize>()
                    .ok()
                    .filter(|&ch| ch < self.channels)
                    .map(|ch| ParameterValue::Bool(self.channel_states[ch].muted))
                    .unwrap_or(ParameterValue::Bool(false))
            } else if let Some(rest) = param.id.0.strip_prefix("solo_") {
                rest.parse::<usize>()
                    .ok()
                    .filter(|&ch| ch < self.channels)
                    .map(|ch| ParameterValue::Bool(self.channel_states[ch].soloed))
                    .unwrap_or(ParameterValue::Bool(false))
            } else if let Some(rest) = param.id.0.strip_prefix("dim_") {
                rest.parse::<usize>()
                    .ok()
                    .filter(|&ch| ch < self.channels)
                    .map(|ch| ParameterValue::Bool(self.channel_states[ch].dimmed))
                    .unwrap_or(ParameterValue::Bool(false))
            } else {
                ParameterValue::Bool(false)
            };
            values.insert(param.id.clone(), value);
        }
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            // Per-channel dynamic parameters (mute_N / solo_N / dim_N).
            // Only treat as per-channel if the suffix is a valid decimal index (starts with digit),
            // to avoid false matches like "dim_gain_db" -> "dim_" prefix with "gain_db" suffix.
            if let Some(rest) = id.0.strip_prefix("mute_")
                && rest.starts_with(|c: char| c.is_ascii_digit())
            {
                if let Ok(ch) = rest.parse::<usize>()
                    && ch < self.channels
                {
                    self.channel_states[ch].muted = value.as_bool().unwrap_or(false);
                    self.update_smoother_targets();
                    self.mark_params_dirty();
                } else {
                    return Err(format!("Invalid channel index in {}", id));
                }
            } else if let Some(rest) = id.0.strip_prefix("solo_")
                && rest.starts_with(|c: char| c.is_ascii_digit())
            {
                if let Ok(ch) = rest.parse::<usize>()
                    && ch < self.channels
                {
                    self.channel_states[ch].soloed = value.as_bool().unwrap_or(false);
                    self.update_smoother_targets();
                    self.mark_params_dirty();
                } else {
                    return Err(format!("Invalid channel index in {}", id));
                }
            } else if let Some(rest) = id.0.strip_prefix("dim_")
                && rest.starts_with(|c: char| c.is_ascii_digit())
            {
                if let Ok(ch) = rest.parse::<usize>()
                    && ch < self.channels
                {
                    self.channel_states[ch].dimmed = value.as_bool().unwrap_or(false);
                    self.update_smoother_targets();
                    self.mark_params_dirty();
                } else {
                    return Err(format!("Invalid channel index in {}", id));
                }
            } else if id == self.param_enabled {
                self.set_enabled(value.as_bool().unwrap_or(true));
                self.mark_params_dirty();
            } else if id == self.param_channel_states {
                if let Some(json_str) = value.as_string() {
                    let states: Vec<ChannelState> =
                        serde_json::from_str(json_str).map_err(|e| e.to_string())?;
                    self.set_channel_states(&states)?;
                    self.mark_params_dirty();
                } else {
                    return Err("channel_states must be string".to_string());
                }
            } else if id == self.param_dim_gain_db {
                if let Some(v) = value.as_float() {
                    self.set_dim_gain_db(v);
                    self.mark_params_dirty();
                } else {
                    return Err("dim_gain_db must be float".to_string());
                }
            } else if id == self.param_fade_ms {
                if let Some(v) = value.as_float() {
                    self.set_fade_ms(v);
                    self.mark_params_dirty();
                } else {
                    return Err("fade_ms must be float".to_string());
                }
            } else {
                return Err(format!("Unknown parameter: {}", id));
            }
        }
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("Channel Mute/Solo sample rate must be greater than zero".to_string());
        }
        self.sample_rate = sample_rate;
        for smoother in &mut self.channel_smoothers {
            smoother.set_time(self.fade_ms, sample_rate);
        }
        self.update_smoother_targets();
        Ok(())
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let num_frames = context.num_frames;
        let sample_len = num_frames
            .checked_mul(self.channels)
            .ok_or_else(|| "Channel mute/solo block sample count overflow".to_string())?;
        if buffer.len() < sample_len {
            return Err(format!(
                "Channel mute/solo buffer too small: need {sample_len} samples, got {}",
                buffer.len()
            ));
        }

        if self.smoothers_are_settled() {
            for (gain, smoother) in self.cached_gains.iter_mut().zip(&self.channel_smoothers) {
                // Static blocks use the exact target without advancing or
                // otherwise mutating settled smoother state.
                *gain = smoother.target();
            }
            #[cfg(test)]
            {
                self.static_path_blocks += 1;
            }
            if self.cached_gains.iter().all(|gain| *gain == 1.0) {
                return Ok(num_frames);
            }
            apply_per_channel_gain_simd(
                &mut buffer[..sample_len],
                self.channels,
                &self.cached_gains,
            );
            return Ok(num_frames);
        }

        let channels = self.channels;
        for frame in 0..num_frames {
            for (gain, smoother) in self
                .cached_gains
                .iter_mut()
                .zip(&mut self.channel_smoothers)
            {
                *gain = smoother.advance();
            }
            let offset = frame * channels;
            apply_per_channel_gain_simd(
                &mut buffer[offset..offset + channels],
                channels,
                &self.cached_gains,
            );
        }

        Ok(num_frames)
    }

    fn reset(&mut self) {
        // A transport reset/seek does not alter routing state. Preserve both
        // the current and target gains so an in-flight click-free transition
        // continues from the same sample value after transport resumes.
    }

    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != PluginCompiledOp::ChannelMuteSolo {
            return None;
        }
        let sample_len = match context.num_frames.checked_mul(self.channels) {
            Some(sample_len) => sample_len,
            None => {
                return Some(Err(
                    "Channel mute/solo block sample count overflow".to_string()
                ));
            }
        };
        if input.len() < sample_len {
            return Some(Err(format!(
                "Channel mute/solo compiled input too small: need {sample_len} samples, got {}",
                input.len()
            )));
        }
        if output.len() < sample_len {
            return Some(Err(format!(
                "Channel mute/solo compiled output too small: need {sample_len} samples, got {}",
                output.len()
            )));
        }
        output[..sample_len].copy_from_slice(&input[..sample_len]);
        Some(self.process_in_place(&mut output[..sample_len], context))
    }
}
