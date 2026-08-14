// ============================================================================
// Gain Plugin - Simple gain control with per-channel support
// ============================================================================

pub mod params;

use crate::params::PARAMS as GN;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet, ParametricPlugin};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use sotf_host::simd::{apply_gain_simd, apply_per_channel_gain_simd};
use sotf_host::smoothing::Smoother;

use serde::{Deserialize, Serialize};

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GainProcessPath {
    SettledBlock,
    SmoothedFrames,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GainPluginParams {
    /// Global gain in dB.
    #[serde(default = "default_gain_db")]
    pub gain_db: f32,
    /// Gain-change smoothing time in milliseconds.
    #[serde(default = "default_smoothing_ms")]
    pub smoothing_ms: f32,
    /// Per-channel gains in dB. When empty, `gain_db` is applied to all channels.
    #[serde(default)]
    pub channel_gains: Vec<f32>,
}

fn default_gain_db() -> f32 {
    pk(GN, "gain_db").default_f64() as f32
}

pub use crate::params::default_smoothing_ms;

pub struct GainPlugin {
    channels: usize,
    sample_rate: u32,
    global_gain_db: f32,
    global_gain_smoother: Smoother,
    channel_gains_db: Vec<f32>,
    channel_gains_smoothers: Vec<Smoother>,
    param_gain_db: ParameterId,
    param_smoothing_ms: ParameterId,
    /// Pre-built per-channel parameter IDs and display names so
    /// `parameter_schema` does not re-format them on every call.
    channel_param_keys: Vec<(ParameterId, String)>,
    cached_gains: Vec<f32>,
    smoothing_ms: f32,
    #[cfg(test)]
    last_process_path: GainProcessPath,
}

impl GainPlugin {
    fn validate_gain_db(db: f32) -> Result<(), String> {
        let min = pk(GN, "gain_db").min_f64() as f32;
        let max = pk(GN, "gain_db").max_f64() as f32;
        if db.is_finite() && (min..=max).contains(&db) {
            Ok(())
        } else {
            Err(format!(
                "gain must be finite and in {min}..={max} dB, got {db}"
            ))
        }
    }

    fn validate_smoothing_ms(smoothing_ms: f32) -> Result<(), String> {
        let min = pk(GN, "smoothing_ms").min_f64() as f32;
        let max = pk(GN, "smoothing_ms").max_f64() as f32;
        if smoothing_ms.is_finite() && (min..=max).contains(&smoothing_ms) {
            Ok(())
        } else {
            Err(format!(
                "smoothing must be finite and in {min}..={max} ms, got {smoothing_ms}"
            ))
        }
    }

    /// Create a gain plugin in global-gain mode using the canonical default
    /// smoothing time from `params::PARAMS`.
    pub fn new(channels: usize, gain_db: f32) -> Self {
        Self::with_smoothing(channels, gain_db, default_smoothing_ms())
    }

    pub fn with_smoothing(channels: usize, gain_db: f32, smoothing_ms: f32) -> Self {
        // Placeholder rate; real rate is set in initialize()
        let sr = 48000;
        let gain_linear = Self::db_to_linear(gain_db);
        Self {
            channels,
            sample_rate: sr,
            global_gain_db: gain_db,
            global_gain_smoother: Smoother::new(gain_linear, smoothing_ms, sr),
            channel_gains_db: Vec::with_capacity(channels),
            channel_gains_smoothers: Vec::with_capacity(channels),
            param_gain_db: ParameterId::from("gain_db"),
            param_smoothing_ms: ParameterId::from("smoothing_ms"),
            channel_param_keys: Self::build_channel_param_keys(channels),
            cached_gains: vec![0.0; channels],
            smoothing_ms,
            #[cfg(test)]
            last_process_path: GainProcessPath::SmoothedFrames,
        }
    }

    fn build_channel_param_keys(channels: usize) -> Vec<(ParameterId, String)> {
        (0..channels)
            .map(|ch| {
                let id = format!("gain_db_{}", ch);
                let name = format!("Gain Ch {}", ch + 1);
                (ParameterId::from(id.as_str()), name)
            })
            .collect()
    }

    /// Create a per-channel gain plugin using the canonical default smoothing
    /// time from `params::PARAMS`.
    pub fn new_per_channel(channel_gains: Vec<f32>) -> Result<Self, String> {
        Self::new_per_channel_with_smoothing(channel_gains, default_smoothing_ms())
    }

    pub fn new_per_channel_with_smoothing(
        channel_gains: Vec<f32>,
        smoothing_ms: f32,
    ) -> Result<Self, String> {
        if channel_gains.is_empty() {
            return Err("Empty".into());
        }
        Self::validate_smoothing_ms(smoothing_ms)?;
        for &gain_db in &channel_gains {
            Self::validate_gain_db(gain_db)?;
        }
        let channels = channel_gains.len();
        // Placeholder rate; real rate is set in initialize()
        let sr = 48000;
        let cgs: Vec<Smoother> = channel_gains
            .iter()
            .map(|&db| Smoother::new(Self::db_to_linear(db), smoothing_ms, sr))
            .collect();
        Ok(Self {
            channels,
            sample_rate: sr,
            global_gain_db: 0.0,
            global_gain_smoother: Smoother::new(1.0, smoothing_ms, sr),
            channel_gains_db: channel_gains,
            channel_gains_smoothers: cgs,
            param_gain_db: ParameterId::from("gain_db"),
            param_smoothing_ms: ParameterId::from("smoothing_ms"),
            channel_param_keys: Self::build_channel_param_keys(channels),
            cached_gains: vec![0.0; channels],
            smoothing_ms,
            #[cfg(test)]
            last_process_path: GainProcessPath::SmoothedFrames,
        })
    }

    pub fn from_params(channels: usize, params: GainPluginParams) -> Result<Self, String> {
        if channels == 0 {
            return Err("gain plugin requires at least one channel".into());
        }
        Self::validate_gain_db(params.gain_db)?;
        Self::validate_smoothing_ms(params.smoothing_ms)?;
        if params.channel_gains.is_empty() {
            Ok(Self::with_smoothing(
                channels,
                params.gain_db,
                params.smoothing_ms,
            ))
        } else {
            let actual = params.channel_gains.len();
            if actual != channels {
                return Err(format!(
                    "channel_gains length mismatch: expected {channels}, got {actual}"
                ));
            }
            Self::new_per_channel_with_smoothing(params.channel_gains, params.smoothing_ms)
        }
    }

    pub fn is_per_channel(&self) -> bool {
        !self.channel_gains_db.is_empty()
    }
    /// Set global gain while preserving this legacy infallible API.
    ///
    /// Non-finite and out-of-range values are deliberate atomic no-ops: mode,
    /// smoother state, reported values, and rendered audio remain unchanged.
    /// New callers that need an error should use the validated parametric API.
    pub fn set_gain_db(&mut self, db: f32) {
        if Self::validate_gain_db(db).is_err() {
            return;
        }
        if let Some(channel) = self.channel_gains_smoothers.first() {
            self.global_gain_smoother.reset(channel.current());
        }
        self.global_gain_db = db;
        self.global_gain_smoother.set_target(Self::db_to_linear(db));
        self.channel_gains_db.clear();
        self.channel_gains_smoothers.clear();
    }
    /// Set global linear gain; invalid values follow [`Self::set_gain_db`]'s
    /// compatibility no-op contract.
    pub fn set_gain_linear(&mut self, g: f32) {
        let db = Self::linear_to_db(g);
        if !g.is_finite() || g <= 0.0 || Self::validate_gain_db(db).is_err() {
            return;
        }
        if let Some(channel) = self.channel_gains_smoothers.first() {
            self.global_gain_smoother.reset(channel.current());
        }
        self.global_gain_smoother.set_target(g);
        self.global_gain_db = db;
        self.channel_gains_db.clear();
        self.channel_gains_smoothers.clear();
    }
    pub fn set_channel_gains(&mut self, dbs: Vec<f32>) -> Result<(), String> {
        if dbs.len() != self.channels {
            return Err("Mismatch".into());
        }
        for &db in &dbs {
            Self::validate_gain_db(db)?;
        }
        self.channel_gains_smoothers = dbs
            .iter()
            .map(|&db| Smoother::new(Self::db_to_linear(db), self.smoothing_ms, self.sample_rate))
            .collect();
        self.channel_gains_db = dbs;
        Ok(())
    }

    /// Set a single channel gain. It is safe to call before `initialize()`;
    /// initialize recalculates all smoother coefficients for the real host
    /// sample rate while preserving current and target values.
    pub fn set_channel_gain_db(&mut self, ch: usize, db: f32) -> Result<(), String> {
        if ch >= self.channels {
            return Err("OOB".into());
        }
        Self::validate_gain_db(db)?;
        if self.channel_gains_db.is_empty() {
            self.channel_gains_db = vec![self.global_gain_db; self.channels];
            let mut smoother = Smoother::new(
                self.global_gain_smoother.current(),
                self.smoothing_ms,
                self.sample_rate,
            );
            smoother.set_target(self.global_gain_smoother.target());
            self.channel_gains_smoothers = vec![smoother; self.channels];
        }
        self.channel_gains_db[ch] = db;
        self.channel_gains_smoothers[ch].set_target(Self::db_to_linear(db));
        Ok(())
    }
    pub fn gain_db(&self) -> f32 {
        self.global_gain_db
    }
    pub fn gain_linear(&self) -> f32 {
        self.global_gain_smoother.current()
    }
    pub fn channel_gain_db(&self, ch: usize) -> Option<f32> {
        if self.is_per_channel() {
            self.channel_gains_db.get(ch).copied()
        } else if ch < self.channels {
            Some(self.global_gain_db)
        } else {
            None
        }
    }
    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        sotf_host::db_to_linear(db)
    }
    #[inline]
    fn linear_to_db(l: f32) -> f32 {
        sotf_host::linear_to_db(l)
    }

    #[inline]
    fn apply_frame_gain(frame: &mut [f32], gain: f32) {
        if frame.len() <= 4 {
            for sample in frame {
                *sample *= gain;
            }
        } else {
            apply_gain_simd(frame, gain);
        }
    }

    #[inline]
    fn apply_frame_channel_gains(frame: &mut [f32], gains: &[f32]) {
        if frame.len() <= 4 {
            for (sample, gain) in frame.iter_mut().zip(gains.iter()) {
                *sample *= *gain;
            }
        } else {
            apply_per_channel_gain_simd(frame, frame.len(), gains);
        }
    }

    #[inline]
    fn smoother_is_settled(smoother: &Smoother) -> bool {
        (smoother.current() - smoother.target()).abs() < 1e-5
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let nf = context.num_frames;
        if self.is_per_channel() {
            if self
                .channel_gains_smoothers
                .iter()
                .all(Self::smoother_is_settled)
            {
                for (gain, smoother) in self
                    .cached_gains
                    .iter_mut()
                    .zip(&mut self.channel_gains_smoothers)
                {
                    *gain = smoother.target();
                    smoother.reset(*gain);
                }
                apply_per_channel_gain_simd(buffer, self.channels, &self.cached_gains);
                #[cfg(test)]
                {
                    self.last_process_path = GainProcessPath::SettledBlock;
                }
                return Ok(nf);
            }
            #[cfg(test)]
            {
                self.last_process_path = GainProcessPath::SmoothedFrames;
            }
            for frame in 0..nf {
                for ch in 0..self.channels {
                    self.cached_gains[ch] = self.channel_gains_smoothers[ch].advance();
                }
                let off = frame * self.channels;
                Self::apply_frame_channel_gains(
                    &mut buffer[off..off + self.channels],
                    &self.cached_gains,
                );
            }
        } else {
            if Self::smoother_is_settled(&self.global_gain_smoother) {
                let gain = self.global_gain_smoother.target();
                self.global_gain_smoother.reset(gain);
                apply_gain_simd(buffer, gain);
                #[cfg(test)]
                {
                    self.last_process_path = GainProcessPath::SettledBlock;
                }
                return Ok(nf);
            }
            #[cfg(test)]
            {
                self.last_process_path = GainProcessPath::SmoothedFrames;
            }
            for frame in 0..nf {
                let g = self.global_gain_smoother.advance();
                let off = frame * self.channels;
                Self::apply_frame_gain(&mut buffer[off..off + self.channels], g);
            }
        }
        Ok(nf)
    }
}

impl ParametricPlugin for GainPlugin {
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo::new("Gain", env!("CARGO_PKG_VERSION"), "Sotf")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Scalar
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameter_schema(&self) -> ParameterSchema {
        let mut params = vec![
            Parameter::new_float(
                "gain_db",
                "Gain",
                self.global_gain_db,
                pk(GN, "gain_db").min_f64() as f32,
                pk(GN, "gain_db").max_f64() as f32,
            ),
            Parameter::new_float(
                "smoothing_ms",
                "Smoothing",
                self.smoothing_ms,
                pk(GN, "smoothing_ms").min_f64() as f32,
                pk(GN, "smoothing_ms").max_f64() as f32,
            ),
        ];

        for (ch, (id, name)) in self.channel_param_keys.iter().enumerate() {
            let db = if self.is_per_channel() {
                self.channel_gains_db[ch]
            } else {
                self.global_gain_db
            };
            params.push(Parameter::new_float(
                &id.0,
                name,
                db,
                pk(GN, "gain_db").min_f64() as f32,
                pk(GN, "gain_db").max_f64() as f32,
            ));
        }

        params
    }

    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        values.insert(
            self.param_gain_db.clone(),
            ParameterValue::Float(self.global_gain_db),
        );
        values.insert(
            self.param_smoothing_ms.clone(),
            ParameterValue::Float(self.smoothing_ms),
        );
        for (ch, (id, _)) in self.channel_param_keys.iter().enumerate() {
            let db = if self.is_per_channel() {
                self.channel_gains_db[ch]
            } else {
                self.global_gain_db
            };
            values.insert(id.clone(), ParameterValue::Float(db));
        }
        values
    }

    fn parametric_validate_parameter(
        &self,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<()> {
        // Give a clearer error for out-of-range per-channel gain indices before
        // falling back to the generic schema lookup.
        if let Some(s) = id.as_str().strip_prefix("gain_db_")
            && let Ok(ch) = s.parse::<usize>()
            && ch >= self.channels
        {
            return Err(format!(
                "Channel gain out of bounds: {} (channels: {})",
                id, self.channels
            ));
        }
        if let Some(param) = self.parameter_schema().iter().find(|p| &p.id == id) {
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
        let Some(value) = value.as_float().filter(|value| value.is_finite()) else {
            return Err(format!("Invalid value for {id}"));
        };
        match id.as_str() {
            "gain_db" => {
                let spec = pk(GN, "gain_db");
                if value < spec.min_f64() as f32 || value > spec.max_f64() as f32 {
                    return Err(format!("Invalid value for {id}"));
                }
                self.set_gain_db(value);
                Ok(())
            }
            "smoothing_ms" => {
                let spec = pk(GN, "smoothing_ms");
                if value < spec.min_f64() as f32 || value > spec.max_f64() as f32 {
                    return Err(format!("Invalid value for {id}"));
                }
                self.smoothing_ms = value;
                self.global_gain_smoother.set_time(value, self.sample_rate);
                for smoother in &mut self.channel_gains_smoothers {
                    smoother.set_time(value, self.sample_rate);
                }
                Ok(())
            }
            key => {
                let channel = key
                    .strip_prefix("gain_db_")
                    .and_then(|suffix| suffix.parse::<usize>().ok())
                    .ok_or_else(|| format!("Unknown parameter: {id}"))?;
                self.set_channel_gain_db(channel, value)
            }
        }
    }

    fn parametric_get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.as_str() {
            "gain_db" => Some(ParameterValue::Float(self.global_gain_db)),
            "smoothing_ms" => Some(ParameterValue::Float(self.smoothing_ms)),
            key => {
                let channel = key.strip_prefix("gain_db_")?.parse::<usize>().ok()?;
                self.channel_gain_db(channel).map(ParameterValue::Float)
            }
        }
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            match id.as_str() {
                "gain_db" => {
                    let Some(v) = value.as_float().filter(|v| v.is_finite()) else {
                        return Err(format!("Invalid value for {}", id));
                    };
                    self.set_gain_db(v);
                }
                "smoothing_ms" => {
                    let Some(v) = value.as_float().filter(|v| v.is_finite()) else {
                        return Err(format!("Invalid value for {}", id));
                    };
                    self.smoothing_ms = v.clamp(
                        pk(GN, "smoothing_ms").min_f64() as f32,
                        pk(GN, "smoothing_ms").max_f64() as f32,
                    );
                    self.global_gain_smoother
                        .set_time(self.smoothing_ms, self.sample_rate);
                    for s in &mut self.channel_gains_smoothers {
                        s.set_time(self.smoothing_ms, self.sample_rate);
                    }
                }
                key => {
                    let Some(s) = key.strip_prefix("gain_db_") else {
                        return Err(format!("Unknown parameter: {}", id));
                    };
                    let Ok(ch) = s.parse::<usize>() else {
                        return Err(format!("Unknown parameter: {}", id));
                    };
                    let Some(v) = value.as_float().filter(|v| v.is_finite()) else {
                        return Err(format!("Invalid value for {}", id));
                    };
                    self.set_channel_gain_db(ch, v)?;
                }
            }
        }
        Ok(())
    }

    fn plugin_initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.global_gain_smoother
            .set_time(self.smoothing_ms, sample_rate);
        for s in &mut self.channel_gains_smoothers {
            s.set_time(self.smoothing_ms, sample_rate);
        }
        Ok(())
    }

    fn plugin_reset(&mut self) {
        self.global_gain_smoother
            .reset(Self::db_to_linear(self.global_gain_db));
        for (smoother, &gain_db) in self
            .channel_gains_smoothers
            .iter_mut()
            .zip(&self.channel_gains_db)
        {
            smoother.reset(Self::db_to_linear(gain_db));
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let sample_len = context
            .num_frames
            .checked_mul(self.channels)
            .ok_or_else(|| "Gain block sample count overflow".to_string())?;
        if input.len() < sample_len {
            return Err(format!(
                "Gain input too small: need {sample_len} samples, got {}",
                input.len()
            ));
        }
        if output.len() < sample_len {
            return Err(format!(
                "Gain output too small: need {sample_len} samples, got {}",
                output.len()
            ));
        }
        output[..sample_len].copy_from_slice(&input[..sample_len]);
        self.process_in_place(&mut output[..sample_len], context)
    }

    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != PluginCompiledOp::ApplyGain {
            return None;
        }
        let sample_len = match context.num_frames.checked_mul(self.channels) {
            Some(sample_len) => sample_len,
            None => return Some(Err("Gain block sample count overflow".to_string())),
        };
        if input.len() < sample_len {
            return Some(Err(format!(
                "Gain compiled input too small: need {sample_len} samples, got {}",
                input.len()
            )));
        }
        if output.len() < sample_len {
            return Some(Err(format!(
                "Gain compiled output too small: need {sample_len} samples, got {}",
                output.len()
            )));
        }
        output[..sample_len].copy_from_slice(&input[..sample_len]);
        Some(self.process_in_place(&mut output[..sample_len], context))
    }

    fn compiled_static_gain(&self) -> Option<f32> {
        if self.is_per_channel() {
            return None;
        }
        let current = self.global_gain_smoother.current();
        let target = self.global_gain_smoother.target();
        if (current - target).abs() <= 1e-6 {
            Some(target)
        } else {
            None
        }
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let static_gain = self.compiled_static_gain();
        PluginCompileMetadata {
            cost_class: PluginCostClass::Scalar,
            compiled_op: Some(PluginCompiledOp::ApplyGain),
            static_gain,
            linear: true,
            time_invariant_for_block: static_gain.is_some(),
            channel_mixing: false,
            stateful: static_gain.is_none(),
            latency_samples: 0,
            can_absorb_input_gain: static_gain.is_some(),
            can_absorb_output_gain: static_gain.is_some(),
            can_merge_with_eq: false,
            boundary: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::parametric_plugin::ParametricPlugin;
    use sotf_host::plugin::PluginCompiledOp;
    #[test]
    fn test_unity_gain() {
        let mut p = GainPlugin::new(2, 0.0);
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut b = vec![0.0; input.len()];
        p.process(&input, &mut b, &ProcessContext::new(44100, 2))
            .unwrap();
        assert!((b[0] - 1.0).abs() < 1e-5);
    }

    /// set_parameter must reject values outside the param-spec range [-60, 20].
    /// Prior to fix, set_parameter validated against hardcoded [-100, 24] while
    /// rebuild_cached_parameters used [-60, 20] from the spec — they disagreed.
    #[test]
    fn test_set_parameter_rejects_out_of_range_gain() {
        let mut p = GainPlugin::new(2, 0.0);
        // +21 dB is above the param spec max of +20 dB — must be rejected.
        let result =
            p.parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(21.0));
        assert!(
            result.is_err(),
            "gain_db=21.0 should be rejected (spec max is 20.0), but got Ok"
        );
        // -61 dB is below the param spec min of -60 dB — must be rejected.
        let result =
            p.parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-61.0));
        assert!(
            result.is_err(),
            "gain_db=-61.0 should be rejected (spec min is -60.0), but got Ok"
        );
        // Values within spec range must be accepted.
        let result =
            p.parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(20.0));
        assert!(
            result.is_ok(),
            "gain_db=20.0 should be accepted (spec max), got Err"
        );
        let result =
            p.parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-60.0));
        assert!(
            result.is_ok(),
            "gain_db=-60.0 should be accepted (spec min), got Err"
        );
    }

    /// from_params error message must include expected and actual channel counts.
    #[test]
    fn test_from_params_mismatch_error_is_descriptive() {
        let params = GainPluginParams {
            gain_db: 0.0,
            smoothing_ms: default_smoothing_ms(),
            channel_gains: vec![0.0, 0.0, 0.0], // 3 channels
        };
        match GainPlugin::from_params(2, params) {
            Ok(_) => panic!("Should fail: 3 channel_gains for 2-channel plugin"),
            Err(err) => {
                // Must contain both the expected (2) and actual (3) lengths.
                assert!(
                    err.contains('2') || err.contains("expected"),
                    "Error should mention expected channel count (2), got: {err:?}"
                );
                assert!(
                    err.contains('3') || err.contains("got") || err.contains("actual"),
                    "Error should mention actual channel count (3), got: {err:?}"
                );
            }
        }
    }

    /// Sample rate deferred initialization: create gain plugin, call
    /// initialize(96000), then verify smoothers respond correctly at the
    /// new rate (a gain change converges within expected time).
    #[test]
    fn test_sample_rate_deferred_initialization() {
        let mut p = GainPlugin::with_smoothing(1, 0.0, default_smoothing_ms());
        // Initialize at 96000 Hz
        p.plugin_initialize(96000).unwrap();

        // Set a new gain target
        p.set_gain_db(-6.0);
        let target_linear = GainPlugin::db_to_linear(-6.0);

        // At 96000 Hz with default smoothing, we need ~5*tau = ~100ms = 9600 samples
        // to converge. Process 200ms worth of samples to be safe.
        let num_frames = 19200; // 200ms at 96kHz
        let input = vec![1.0f32; num_frames];
        let mut buf = vec![0.0f32; input.len()];
        p.process(&input, &mut buf, &ProcessContext::new(96000, num_frames))
            .unwrap();

        // After 200ms, the smoother should have converged to the target gain
        let last_sample = buf[num_frames - 1];
        assert!(
            (last_sample - target_linear).abs() < 0.01,
            "After 200ms at 96kHz, gain should converge to {target_linear:.4}, got {last_sample:.4}"
        );

        // Verify it didn't converge too fast (after only 1ms = 96 samples)
        // by checking the output wasn't already at target near the beginning
        let early_sample = buf[96]; // ~1ms
        let diff_from_target = (early_sample - target_linear).abs();
        assert!(
            diff_from_target > 0.01,
            "After only 1ms, gain should still be transitioning (diff={diff_from_target:.4})"
        );
    }

    #[test]
    fn test_channel_gain_before_initialize_uses_host_sample_rate_after_initialize() {
        let mut p = GainPlugin::with_smoothing(1, 0.0, default_smoothing_ms());
        p.set_channel_gain_db(0, -6.0).unwrap();
        p.plugin_initialize(96000).unwrap();

        let target_linear = GainPlugin::db_to_linear(-6.0);
        let num_frames = 19200; // 200ms at 96kHz
        let input = vec![1.0f32; num_frames];
        let mut buf = vec![0.0f32; input.len()];
        p.process(&input, &mut buf, &ProcessContext::new(96000, num_frames))
            .unwrap();

        let last_sample = buf[num_frames - 1];
        assert!(
            (last_sample - target_linear).abs() < 0.01,
            "pre-initialize channel smoother should converge at host sample rate: target={target_linear:.4}, got={last_sample:.4}"
        );

        let early_sample = buf[96]; // ~1ms at 96kHz
        assert!(
            (early_sample - target_linear).abs() > 0.01,
            "pre-initialize channel smoother should not use stale/too-fast timing"
        );
    }

    #[test]
    fn test_small_frame_gain_helpers_match_expected_scalar_results() {
        let mut stereo = vec![1.0, -2.0];
        GainPlugin::apply_frame_gain(&mut stereo, 0.25);
        assert_eq!(stereo, vec![0.25, -0.5]);

        let mut quad = vec![1.0, 2.0, 3.0, 4.0];
        GainPlugin::apply_frame_channel_gains(&mut quad, &[1.0, 0.5, -1.0, 2.0]);
        assert_eq!(quad, vec![1.0, 1.0, -3.0, 8.0]);
    }

    /// process_in_place smoke test with a known scalar global gain.
    #[test]
    fn test_process_in_place_global_gain_known_output() {
        let mut p = GainPlugin::with_smoothing(2, 6.0, 0.0); // no smoothing
        p.plugin_initialize(48000).unwrap();

        let input = vec![0.1f32, 0.2, 0.3, 0.4];
        let mut buffer = vec![0.0f32; input.len()];
        let context = ProcessContext::new(48000, 2);

        p.process(&input, &mut buffer, &context).unwrap();

        let expected_linear = 10.0_f32.powf(6.0 / 20.0);
        for (i, (&out, &inp)) in buffer.iter().zip(input.iter()).enumerate() {
            assert!(
                (out - inp * expected_linear).abs() < 1e-5,
                "sample {i}: expected {}, got {}",
                inp * expected_linear,
                out
            );
        }
    }

    /// process_in_place smoke test with known per-channel gains.
    #[test]
    fn test_process_in_place_per_channel_known_output() {
        let mut p = GainPlugin::new_per_channel(vec![0.0f32, -6.0]).unwrap();
        p.plugin_initialize(48000).unwrap();

        // interleaved stereo: [L0, R0, L1, R1]
        let input = vec![1.0f32, 1.0, 1.0, 1.0];
        let mut buffer = vec![0.0f32; input.len()];
        let context = ProcessContext::new(48000, 2);

        p.process(&input, &mut buffer, &context).unwrap();

        let ch0_gain = 1.0; // 0 dB -> linear 1.0
        let ch1_gain = 10.0_f32.powf(-6.0 / 20.0);
        assert!((buffer[0] - ch0_gain).abs() < 1e-4);
        assert!((buffer[1] - ch1_gain).abs() < 1e-4);
        assert!((buffer[2] - ch0_gain).abs() < 1e-4);
        assert!((buffer[3] - ch1_gain).abs() < 1e-4);
    }

    #[test]
    fn test_compiled_apply_gain_matches_regular_process() {
        let input: Vec<f32> = (0..128)
            .map(|i| (((i * 17) % 31) as f32 - 15.0) / 16.0)
            .collect();
        let context = ProcessContext::new(48000, input.len() / 2);
        let mut regular = GainPlugin::with_smoothing(2, -6.0, 0.0);
        let mut compiled = GainPlugin::with_smoothing(2, -6.0, 0.0);
        regular.plugin_initialize(48000).unwrap();
        compiled.plugin_initialize(48000).unwrap();
        let mut regular_output = vec![0.0; input.len()];
        let mut compiled_output = vec![0.0; input.len()];

        let regular_frames = regular
            .process(&input, &mut regular_output, &context)
            .unwrap();
        let compiled_frames = compiled
            .process_compiled_f32(
                PluginCompiledOp::ApplyGain,
                &input,
                &mut compiled_output,
                &context,
            )
            .expect("gain should accept compiled apply-gain op")
            .unwrap();

        assert_eq!(regular_frames, context.num_frames);
        assert_eq!(compiled_frames, context.num_frames);
        assert_eq!(compiled_output, regular_output);
    }

    #[test]
    fn test_compiled_static_gain_reports_only_settled_global_gain() {
        let mut p = GainPlugin::new(2, -6.0);
        let initial = GainPlugin::db_to_linear(-6.0);
        assert!(
            (p.compiled_static_gain().unwrap() - initial).abs() < 1e-6,
            "constructor starts global gain smoother at its target"
        );

        p.set_gain_db(-12.0);
        assert!(
            p.compiled_static_gain().is_none(),
            "transitioning smoother must not be fused as a static gain"
        );

        p.set_channel_gain_db(0, -3.0).unwrap();
        assert!(
            p.compiled_static_gain().is_none(),
            "per-channel gain must not be fused as a scalar gain"
        );
    }

    #[test]
    fn test_compile_metadata_tracks_static_gain_legality() {
        let mut p = GainPlugin::new(2, -6.0);
        let metadata = p.compile_metadata();
        assert_eq!(metadata.compiled_op, Some(PluginCompiledOp::ApplyGain));
        assert!(metadata.static_gain.is_some());
        assert!(metadata.linear);
        assert!(metadata.time_invariant_for_block);
        assert!(!metadata.boundary);
        assert!(metadata.can_absorb_input_gain);
        assert!(metadata.can_absorb_output_gain);

        p.set_gain_db(-12.0);
        let metadata = p.compile_metadata();
        assert_eq!(metadata.compiled_op, Some(PluginCompiledOp::ApplyGain));
        assert!(metadata.static_gain.is_none());
        assert!(metadata.linear);
        assert!(!metadata.time_invariant_for_block);
        assert!(metadata.stateful);
        assert!(!metadata.can_absorb_input_gain);
    }

    /// set_parameter smoke tests for gain, smoothing_ms, and per-channel gains.
    #[test]
    fn test_set_parameter_smoke_known_values() {
        let mut p = GainPlugin::new(2, 0.0);

        // gain_db
        p.parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-12.0))
            .unwrap();
        assert!((p.gain_db() - (-12.0)).abs() < 1e-5);

        // smoothing_ms
        p.parametric_set_parameter(
            ParameterId::from("smoothing_ms"),
            ParameterValue::Float(50.0),
        )
        .unwrap();
        assert!((p.smoothing_ms - 50.0).abs() < 1e-5);

        // per-channel gain
        p.parametric_set_parameter(ParameterId::from("gain_db_0"), ParameterValue::Float(3.0))
            .unwrap();
        assert!((p.channel_gain_db(0).unwrap() - 3.0).abs() < 1e-5);

        p.parametric_set_parameter(ParameterId::from("gain_db_1"), ParameterValue::Float(-3.0))
            .unwrap();
        assert!((p.channel_gain_db(1).unwrap() - (-3.0)).abs() < 1e-5);
    }

    /// set_parameter must reject NaN and infinite values.
    #[test]
    fn test_set_parameter_rejects_non_finite() {
        let mut p = GainPlugin::new(2, 0.0);
        assert!(
            p.parametric_set_parameter(
                ParameterId::from("gain_db"),
                ParameterValue::Float(f32::NAN)
            )
            .is_err()
        );
        assert!(
            p.parametric_set_parameter(
                ParameterId::from("gain_db"),
                ParameterValue::Float(f32::INFINITY)
            )
            .is_err()
        );
    }

    /// Zero-frame processing must return 0 and leave the output untouched.
    #[test]
    fn test_process_in_place_zero_frames() {
        let mut p = GainPlugin::new(2, 6.0);
        p.plugin_initialize(48000).unwrap();
        let input = vec![0.5, 0.6, 0.7, 0.8];
        let mut buffer = vec![0.0; input.len()];
        let processed = p
            .process(&input, &mut buffer, &ProcessContext::new(48000, 0))
            .unwrap();
        assert_eq!(processed, 0);
        assert_eq!(buffer, vec![0.0; input.len()]);
    }

    /// get_parameter must round-trip the values set by set_parameter.
    #[test]
    fn test_get_parameter_round_trip() {
        let mut p = GainPlugin::new(2, 0.0);
        p.parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-10.0))
            .unwrap();
        assert_eq!(
            p.parametric_get_parameter(&ParameterId::from("gain_db")),
            Some(ParameterValue::Float(-10.0))
        );

        p.parametric_set_parameter(
            ParameterId::from("smoothing_ms"),
            ParameterValue::Float(42.0),
        )
        .unwrap();
        assert_eq!(
            p.parametric_get_parameter(&ParameterId::from("smoothing_ms")),
            Some(ParameterValue::Float(42.0))
        );

        p.parametric_set_parameter(ParameterId::from("gain_db_0"), ParameterValue::Float(5.0))
            .unwrap();
        assert_eq!(
            p.parametric_get_parameter(&ParameterId::from("gain_db_0")),
            Some(ParameterValue::Float(5.0))
        );
    }

    /// Per-channel parameter IDs and display names must be cached at
    /// construction and reused by `parameter_schema`, avoiding a per-call
    /// `format!` for every channel.
    #[test]
    fn test_per_channel_param_keys_are_cached_and_reused() {
        let mut p = GainPlugin::new_per_channel(vec![1.0f32, 2.0, 3.0]).unwrap();
        assert_eq!(p.channel_param_keys.len(), 3);
        assert_eq!(p.channel_param_keys[0].0, ParameterId::from("gain_db_0"));
        assert_eq!(p.channel_param_keys[0].1, "Gain Ch 1");
        assert_eq!(p.channel_param_keys[2].0, ParameterId::from("gain_db_2"));
        assert_eq!(p.channel_param_keys[2].1, "Gain Ch 3");

        // Mutate a channel gain; the cached keys must stay the same.
        p.set_channel_gain_db(1, -6.0).unwrap();
        let keys_before = p.channel_param_keys.clone();
        let params = p.parametric_parameters();
        assert_eq!(p.channel_param_keys, keys_before);

        let ch_params: Vec<_> = params
            .iter()
            .filter(|param| param.id.as_str().starts_with("gain_db_"))
            .collect();
        assert_eq!(ch_params.len(), 3);
        assert_eq!(ch_params[0].id, ParameterId::from("gain_db_0"));
        assert_eq!(ch_params[0].name, "Gain Ch 1");
    }

    /// Per-channel gain parameters must round-trip through set_parameter /
    /// get_parameter when the plugin is created in per-channel mode.
    #[test]
    fn test_per_channel_gain_round_trip() {
        let mut p = GainPlugin::new_per_channel(vec![0.0f32, 0.0]).unwrap();
        p.plugin_initialize(48000).unwrap();

        p.parametric_set_parameter(ParameterId::from("gain_db_0"), ParameterValue::Float(3.0))
            .unwrap();
        p.parametric_set_parameter(ParameterId::from("gain_db_1"), ParameterValue::Float(-3.0))
            .unwrap();

        assert_eq!(
            p.parametric_get_parameter(&ParameterId::from("gain_db_0")),
            Some(ParameterValue::Float(3.0))
        );
        assert_eq!(
            p.parametric_get_parameter(&ParameterId::from("gain_db_1")),
            Some(ParameterValue::Float(-3.0))
        );
    }

    /// Default values for gain and smoothing must come from the canonical
    /// PARAMS spec, not from hard-coded literals.
    #[test]
    fn test_default_values_come_from_params_spec() {
        let p = GainPlugin::new(2, 0.0);
        assert!(
            (p.gain_db() - pk(GN, "gain_db").default_f64() as f32).abs() < 1e-6,
            "default gain_db should come from PARAMS"
        );
        assert!(
            (p.smoothing_ms - pk(GN, "smoothing_ms").default_f64() as f32).abs() < 1e-6,
            "default smoothing_ms should come from PARAMS"
        );

        let params: GainPluginParams = serde_json::from_str("{}").unwrap();
        assert!(
            (params.gain_db - pk(GN, "gain_db").default_f64() as f32).abs() < 1e-6,
            "deserialized default gain_db should come from PARAMS"
        );
        assert!(
            (params.smoothing_ms - pk(GN, "smoothing_ms").default_f64() as f32).abs() < 1e-6,
            "deserialized default smoothing_ms should come from PARAMS"
        );
    }

    #[test]
    fn from_params_rejects_invalid_numeric_values_and_zero_channels() {
        let params = |gain_db, smoothing_ms, channel_gains| GainPluginParams {
            gain_db,
            smoothing_ms,
            channel_gains,
        };
        assert!(GainPlugin::from_params(0, params(0.0, 10.0, vec![])).is_err());
        for gain in [f32::NAN, f32::INFINITY, -61.0, 21.0] {
            assert!(GainPlugin::from_params(2, params(gain, 10.0, vec![])).is_err());
            assert!(GainPlugin::from_params(2, params(0.0, 10.0, vec![gain, 0.0])).is_err());
        }
        for smoothing in [f32::NAN, f32::INFINITY, -0.1, 100.1] {
            assert!(GainPlugin::from_params(2, params(0.0, smoothing, vec![])).is_err());
        }
        assert!(GainPlugin::from_params(2, params(-60.0, 0.0, vec![])).is_ok());
        assert!(GainPlugin::from_params(2, params(20.0, 100.0, vec![])).is_ok());
    }

    #[test]
    fn invalid_infallible_global_setters_are_atomic_no_ops() {
        let invalid_db = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -60.1, 20.1];
        let invalid_linear = [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            0.0,
            GainPlugin::db_to_linear(-60.1),
            GainPlugin::db_to_linear(20.1),
        ];

        for starts_per_channel in [false, true] {
            let mut plugin = if starts_per_channel {
                GainPlugin::new_per_channel_with_smoothing(vec![-3.0, -12.0], 100.0).unwrap()
            } else {
                GainPlugin::with_smoothing(2, -3.0, 100.0)
            };
            plugin.plugin_initialize(1_000).unwrap();
            if starts_per_channel {
                plugin.set_channel_gain_db(0, -18.0).unwrap();
            } else {
                plugin.set_gain_db(-18.0);
            }

            let mode_before = plugin.is_per_channel();
            let db_before = plugin.global_gain_db;
            let current_before = plugin.global_gain_smoother.current();
            let target_before = plugin.global_gain_smoother.target();
            let channel_db_before = plugin.channel_gains_db.clone();
            let channel_smoothers_before: Vec<(f32, f32)> = plugin
                .channel_gains_smoothers
                .iter()
                .map(|smoother| (smoother.current(), smoother.target()))
                .collect();
            let values_before = plugin.current_values();

            for &db in &invalid_db {
                plugin.set_gain_db(db);
                assert_eq!(plugin.is_per_channel(), mode_before);
                assert_eq!(plugin.global_gain_db.to_bits(), db_before.to_bits());
                assert_eq!(
                    plugin.global_gain_smoother.current().to_bits(),
                    current_before.to_bits()
                );
                assert_eq!(
                    plugin.global_gain_smoother.target().to_bits(),
                    target_before.to_bits()
                );
                assert_eq!(plugin.channel_gains_db, channel_db_before);
                assert_eq!(
                    plugin
                        .channel_gains_smoothers
                        .iter()
                        .map(|smoother| (smoother.current(), smoother.target()))
                        .collect::<Vec<_>>(),
                    channel_smoothers_before
                );
                assert_eq!(plugin.current_values(), values_before);
            }
            for &gain in &invalid_linear {
                plugin.set_gain_linear(gain);
                assert_eq!(plugin.is_per_channel(), mode_before);
                assert_eq!(plugin.global_gain_db.to_bits(), db_before.to_bits());
                assert_eq!(
                    plugin.global_gain_smoother.current().to_bits(),
                    current_before.to_bits()
                );
                assert_eq!(
                    plugin.global_gain_smoother.target().to_bits(),
                    target_before.to_bits()
                );
                assert_eq!(plugin.channel_gains_db, channel_db_before);
                assert_eq!(
                    plugin
                        .channel_gains_smoothers
                        .iter()
                        .map(|smoother| (smoother.current(), smoother.target()))
                        .collect::<Vec<_>>(),
                    channel_smoothers_before
                );
                assert_eq!(plugin.current_values(), values_before);
            }

            let mut actual = [0.25_f32, -0.5];
            plugin
                .process_in_place(&mut actual, &ProcessContext::new(1_000, 1))
                .unwrap();

            let mut reference = if starts_per_channel {
                GainPlugin::new_per_channel_with_smoothing(vec![-3.0, -12.0], 100.0).unwrap()
            } else {
                GainPlugin::with_smoothing(2, -3.0, 100.0)
            };
            reference.plugin_initialize(1_000).unwrap();
            if starts_per_channel {
                reference.set_channel_gain_db(0, -18.0).unwrap();
            } else {
                reference.set_gain_db(-18.0);
            }
            let mut expected = [0.25_f32, -0.5];
            reference
                .process_in_place(&mut expected, &ProcessContext::new(1_000, 1))
                .unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn entering_per_channel_mode_preserves_global_ramp_for_untouched_channels() {
        let mut plugin = GainPlugin::with_smoothing(2, 0.0, 100.0);
        plugin.plugin_initialize(1_000).unwrap();
        plugin.set_gain_db(-20.0);
        let input = vec![1.0; 20];
        let mut output = vec![0.0; 20];
        plugin
            .process(&input, &mut output, &ProcessContext::new(1_000, 10))
            .unwrap();
        let transition_gain = output[19];

        plugin.set_channel_gain_db(0, -6.0).unwrap();
        let mut output = vec![0.0; 20];
        plugin
            .process(&input, &mut output, &ProcessContext::new(1_000, 10))
            .unwrap();

        assert!(
            output[1] < transition_gain,
            "untouched channel must keep ramping"
        );
        assert_eq!(plugin.channel_gain_db(1), Some(-20.0));
    }

    #[test]
    fn returning_to_global_mode_starts_from_current_channel_gain() {
        let mut plugin =
            GainPlugin::new_per_channel_with_smoothing(vec![-20.0, 0.0], 100.0).unwrap();
        plugin.plugin_initialize(1_000).unwrap();
        plugin.set_channel_gain_db(0, 0.0).unwrap();
        let input = vec![1.0; 20];
        let mut output = vec![0.0; 20];
        plugin
            .process(&input, &mut output, &ProcessContext::new(1_000, 10))
            .unwrap();
        let channel_zero_gain = output[18];

        plugin.set_gain_db(-6.0);
        let mut next = vec![0.0; 2];
        plugin
            .process(&[1.0, 1.0], &mut next, &ProcessContext::new(1_000, 1))
            .unwrap();
        assert!((next[0] - channel_zero_gain).abs() < 0.02);
    }

    #[test]
    fn process_checks_active_prefix_and_preserves_output_tail() {
        let mut plugin = GainPlugin::with_smoothing(2, 0.0, 0.0);
        let input = [1.0, 2.0, 3.0, 4.0, 99.0];
        let mut output = [0.0, 0.0, 0.0, 0.0, 77.0, 88.0];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 2))
            .unwrap();
        assert_eq!(output, [1.0, 2.0, 3.0, 4.0, 77.0, 88.0]);
        assert!(
            plugin
                .process(&input[..3], &mut output, &ProcessContext::new(48_000, 2))
                .is_err()
        );
        assert!(
            plugin
                .process(&input, &mut output[..3], &ProcessContext::new(48_000, 2))
                .is_err()
        );

        let mut zero_frame_output = [7.0, 8.0];
        plugin
            .process(
                &input,
                &mut zero_frame_output,
                &ProcessContext::new(48_000, 0),
            )
            .unwrap();
        assert_eq!(zero_frame_output, [7.0, 8.0]);
    }

    #[test]
    fn reset_snaps_global_and_channel_ramps_to_targets() {
        let mut global = GainPlugin::with_smoothing(1, 0.0, 100.0);
        global.plugin_initialize(1_000).unwrap();
        global.set_gain_db(-20.0);
        global.plugin_reset();
        let mut output = [0.0];
        global
            .process(&[1.0], &mut output, &ProcessContext::new(1_000, 1))
            .unwrap();
        assert!((output[0] - GainPlugin::db_to_linear(-20.0)).abs() < 1e-6);

        let mut per_channel =
            GainPlugin::new_per_channel_with_smoothing(vec![0.0, 0.0], 100.0).unwrap();
        per_channel.set_channel_gain_db(0, -20.0).unwrap();
        per_channel.plugin_reset();
        let mut output = [0.0; 2];
        per_channel
            .process(&[1.0; 2], &mut output, &ProcessContext::new(1_000, 1))
            .unwrap();
        assert!((output[0] - GainPlugin::db_to_linear(-20.0)).abs() < 1e-6);
        assert!((output[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn settled_global_gain_uses_one_block_kernel() {
        let mut plugin = GainPlugin::with_smoothing(2, -6.0, 10.0);
        plugin.plugin_initialize(48_000).unwrap();
        let input = vec![0.25; 4096 * 2];
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 4096))
            .unwrap();
        assert_eq!(plugin.last_process_path, GainProcessPath::SettledBlock);
    }

    #[test]
    fn settled_per_channel_gain_uses_one_block_kernel() {
        let mut plugin = GainPlugin::new_per_channel_with_smoothing(vec![0.0, -6.0], 10.0).unwrap();
        plugin.plugin_initialize(48_000).unwrap();
        let input = vec![0.25; 4096 * 2];
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 4096))
            .unwrap();
        assert_eq!(plugin.last_process_path, GainProcessPath::SettledBlock);
    }

    #[test]
    fn moving_gain_keeps_sample_accurate_smoothed_path() {
        let mut plugin = GainPlugin::with_smoothing(2, 0.0, 100.0);
        plugin.plugin_initialize(48_000).unwrap();
        plugin.set_gain_db(-12.0);
        let input = vec![0.25; 64 * 2];
        let mut output = vec![0.0; input.len()];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 64))
            .unwrap();
        assert_eq!(plugin.last_process_path, GainProcessPath::SmoothedFrames);
        assert_ne!(output[0], output[output.len() - 2]);
    }

    #[test]
    fn settled_fast_path_preserves_ramp_partition_invariance() {
        fn render(partitions: &[usize]) -> Vec<f32> {
            let mut plugin = GainPlugin::with_smoothing(2, 0.0, 10.0);
            plugin.plugin_initialize(1_000).unwrap();
            plugin.set_gain_db(-12.0);
            let mut rendered = Vec::new();
            for &frames in partitions {
                let input = vec![0.25; frames * 2];
                let mut output = vec![0.0; input.len()];
                plugin
                    .process(&input, &mut output, &ProcessContext::new(1_000, frames))
                    .unwrap();
                rendered.extend(output);
            }
            rendered
        }

        let contiguous = render(&[256]);
        let partitioned = render(&[16; 16]);
        assert_eq!(contiguous, partitioned);
    }

    #[test]
    fn runtime_plugin_version_matches_manifest_version() {
        assert_eq!(
            GainPlugin::new(1, 0.0).plugin_info().version,
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn global_mode_reports_each_advertised_channel_parameter() {
        let plugin = GainPlugin::new(2, -3.0);
        for channel in 0..2 {
            let id = ParameterId::from(format!("gain_db_{channel}").as_str());
            assert_eq!(
                plugin.parametric_get_parameter(&id),
                Some(ParameterValue::Float(-3.0))
            );
        }
    }
}
