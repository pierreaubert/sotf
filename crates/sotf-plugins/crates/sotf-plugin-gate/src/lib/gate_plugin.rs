use super::consts::DB_CONVERSION_FACTOR;
use super::consts::EPSILON;
use super::consts::MAX_LOOKAHEAD_MS;
use super::gate_data::GateData;
use super::types::GatePluginParams;
use crate::params::{DETECTION_MODES, HPF_ORDERS, PARAMS as GT, default_range_db};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, peq_butterworth_highpass};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::{DetectionMode, LevelDetector, LookaheadBuffer, ParametricInPlacePlugin};
use std::any::Any;
use std::sync::Arc;

pub struct GatePlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    pub(super) threshold_db: f32,
    pub(super) ratio: f32,
    pub(super) attack_ms: f32,
    pub(super) hold_ms: f32,
    pub(super) hold_samples: usize,
    pub(super) release_ms: f32,
    pub(super) mix: f32,
    pub(super) link_channels: bool,
    pub(super) sidechain_hpf_hz: f32,
    /// 0 = 2nd order (-12dB/oct), 1 = 4th order (-24dB/oct)
    pub(super) sidechain_hpf_order_index: usize,
    /// 0 = Peak, 1 = RMS
    pub(super) detection_mode_index: usize,
    pub(super) sidechain_external: bool,
    pub(super) range_db: f32,
    pub(super) hysteresis_db: f32,
    pub(super) knee_db: f32,
    pub(super) lookahead_ms: f32,
    pub(super) lookahead_buffers: Vec<LookaheadBuffer>,
    /// Gate state per channel for hysteresis
    pub(super) gate_open: Vec<bool>,
    pub(super) hold_counter: Vec<usize>,
    pub(super) attack_coeff: f32,
    pub(super) release_coeff: f32,
    /// Butterworth HPF biquad sections per channel (empty when HPF disabled)
    pub(super) sidechain_hpf_biquads: Vec<Vec<Biquad>>,
    /// Level detectors for peak/RMS detection
    pub(super) level_detectors: Vec<LevelDetector>,
    pub(super) threshold_smoother: Smoother,
    pub(super) mix_smoother: Smoother,
    /// Gain reduction envelope in dB (positive value)
    pub(super) envelope: Vec<f32>,
    /// Instantaneous input levels in dB for monitoring
    pub(super) monitoring_levels: Vec<f32>,
    /// Attenuation levels kept separate from the input-level diagnostic scratch.
    pub(super) attenuation_scratch: Vec<f32>,
    pub(super) cache: RealTimeCache<GateData>,
    pub(super) diagnostic_samples: usize,
    pub(super) diagnostic_interval_samples: usize,
    pub(super) initialized: bool,
    pub(super) cached_parameters: Vec<sotf_host::parameters::Parameter>,
}

impl GatePlugin {
    pub fn try_new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
    ) -> Result<Self, String> {
        Self::try_from_params(
            channels,
            GatePluginParams {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                ..GatePluginParams::default()
            },
        )
    }

    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
    ) -> Self {
        Self::try_new(
            channels,
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
        )
        .expect("invalid Gate parameters")
    }

    fn new_validated(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
    ) -> Self {
        let sr = 44100;
        let mut p = Self {
            channels,
            sample_rate: sr,
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            hold_samples: (hold_ms * 0.001 * sr as f32).round() as usize,
            release_ms,
            mix: 1.0,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order_index: 0,
            detection_mode_index: 0,
            sidechain_external: false,
            range_db: default_range_db(),
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
            lookahead_buffers: (0..channels)
                .map(|_| LookaheadBuffer::from_ms(MAX_LOOKAHEAD_MS, sr, 1))
                .collect(),
            gate_open: vec![false; channels],
            envelope: vec![0.0; channels],
            monitoring_levels: vec![-120.0; channels],
            hold_counter: vec![0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sidechain_hpf_biquads: Vec::new(),
            level_detectors: (0..channels)
                .map(|_| LevelDetector::new(DetectionMode::Peak, sr))
                .collect(),
            threshold_smoother: Smoother::new(threshold_db, 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            attenuation_scratch: vec![0.0; channels],
            cache: RealTimeCache::new_pair(GateData::new(channels), GateData::new(channels)),
            diagnostic_samples: 0,
            diagnostic_interval_samples: sr as usize / 30,
            initialized: false,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    /// Get the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.threshold_db as f64),
            1 => Some(self.ratio as f64),
            2 => Some(self.attack_ms as f64),
            3 => Some(self.hold_ms as f64),
            4 => Some(self.release_ms as f64),
            5 => Some(self.mix as f64),
            6 => Some(if self.link_channels { 1.0 } else { 0.0 }),
            7 => Some(self.sidechain_hpf_hz as f64),
            8 => Some(self.sidechain_hpf_order_index as f64),
            9 => Some(self.detection_mode_index as f64),
            10 => Some(if self.sidechain_external { 1.0 } else { 0.0 }),
            11 => Some(self.range_db as f64),
            12 => Some(self.hysteresis_db as f64),
            13 => Some(self.knee_db as f64),
            14 => Some(self.lookahead_ms as f64),
            _ => None,
        }
    }

    /// Set the f64 value of parameter at PARAMS index.
    /// Order must match params::PARAMS exactly.
    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.threshold_db = value as f32,
            1 => self.ratio = value as f32,
            2 => self.attack_ms = value as f32,
            3 => self.hold_ms = value as f32,
            4 => self.release_ms = value as f32,
            5 => self.mix = value as f32,
            6 => self.link_channels = value > 0.5,
            7 => self.sidechain_hpf_hz = value as f32,
            8 => self.sidechain_hpf_order_index = value as usize,
            9 => self.detection_mode_index = value as usize,
            10 => self.sidechain_external = value > 0.5,
            11 => self.range_db = value as f32,
            12 => self.hysteresis_db = value as f32,
            13 => self.knee_db = value as f32,
            14 => self.lookahead_ms = value as f32,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(GT, |i| self.param_value(i));
    }

    /// Construct from serialized parameters after validating the complete
    /// parameter contract.  Factory callers should use this fallible entry
    /// point so malformed JSON cannot create NaNs or invalid timing constants.
    pub fn try_from_params(channels: usize, params: GatePluginParams) -> Result<Self, String> {
        if channels == 0 {
            return Err("Gate requires at least one channel".into());
        }
        if params.sidechain_external && channels.checked_mul(2).is_none() {
            return Err("Gate external-sidechain channel count overflows usize".into());
        }
        fn finite_spec(name: &str, key: &str, value: f32) -> Result<(), String> {
            let spec = pk(GT, key);
            let min = spec.min_f64() as f32;
            let max = spec.max_f64() as f32;
            if value.is_finite() && (min..=max).contains(&value) {
                Ok(())
            } else {
                Err(format!(
                    "Gate parameter {name} must be finite and in [{min}, {max}], got {value}"
                ))
            }
        }
        finite_spec("threshold_db", "threshold", params.threshold_db)?;
        finite_spec("ratio", "ratio", params.ratio)?;
        finite_spec("attack_ms", "attack", params.attack_ms)?;
        finite_spec("hold_ms", "hold", params.hold_ms)?;
        finite_spec("release_ms", "release", params.release_ms)?;
        finite_spec("mix", "mix", params.mix)?;
        finite_spec(
            "sidechain_hpf_hz",
            "sidechain_hpf_hz",
            params.sidechain_hpf_hz,
        )?;
        finite_spec("range_db", "range_db", params.range_db)?;
        finite_spec("hysteresis_db", "hysteresis_db", params.hysteresis_db)?;
        finite_spec("knee_db", "knee_db", params.knee_db)?;
        finite_spec("lookahead_ms", "lookahead_ms", params.lookahead_ms)?;
        if !HPF_ORDERS
            .iter()
            .any(|v| v.eq_ignore_ascii_case(&params.sidechain_hpf_order))
        {
            return Err(format!(
                "Unknown Gate HPF order: {}",
                params.sidechain_hpf_order
            ));
        }
        if !DETECTION_MODES
            .iter()
            .any(|v| v.eq_ignore_ascii_case(&params.detection_mode))
        {
            return Err(format!(
                "Unknown Gate detection mode: {}",
                params.detection_mode
            ));
        }
        let mut p = Self::new_validated(
            channels,
            params.threshold_db,
            params.ratio,
            params.attack_ms,
            params.hold_ms,
            params.release_ms,
        );
        p.mix = params.mix;
        p.mix_smoother.reset(p.mix);
        p.link_channels = params.link_channels;
        p.sidechain_hpf_hz = params.sidechain_hpf_hz;

        // HPF order
        p.sidechain_hpf_order_index =
            usize::from(params.sidechain_hpf_order.eq_ignore_ascii_case("4th"));

        // Detection mode
        p.detection_mode_index = usize::from(params.detection_mode.eq_ignore_ascii_case("rms"));
        if p.detection_mode_index == 1 {
            let mode = DetectionMode::Rms { window_ms: 10.0 };
            for det in &mut p.level_detectors {
                det.set_mode(mode);
            }
        }

        // External sidechain
        p.sidechain_external = params.sidechain_external;

        p.range_db = params.range_db;
        p.hysteresis_db = params.hysteresis_db;
        p.knee_db = params.knee_db;
        p.lookahead_ms = params.lookahead_ms;
        p.update_hold_samples();
        p.update_lookahead_delay();
        p.rebuild_cached_parameters();
        Ok(p)
    }

    /// Compatibility constructor for existing in-process callers.  New
    /// factory/configuration paths must use [`try_from_params`].
    pub fn from_params(channels: usize, params: GatePluginParams) -> Self {
        Self::try_from_params(channels, params).expect("invalid GatePluginParams")
    }

    pub(super) fn update_hold_samples(&mut self) {
        self.hold_samples = (self.hold_ms * 0.001 * self.sample_rate as f32).round() as usize;
        for counter in &mut self.hold_counter {
            *counter = (*counter).min(self.hold_samples);
        }
    }

    pub(super) fn update_lookahead_delay(&mut self) {
        for buf in &mut self.lookahead_buffers {
            buf.set_delay_ms(self.lookahead_ms, self.sample_rate);
        }
    }

    #[inline]
    pub(super) fn advance_threshold(&mut self) -> (f32, f32) {
        let threshold_db = self.threshold_smoother.advance();
        (
            threshold_db,
            fast_pow10(threshold_db / DB_CONVERSION_FACTOR),
        )
    }

    pub(super) fn calculate_gate_attenuation(&self, input_db: f32, threshold: f32) -> f32 {
        let knee = self.knee_db.max(0.0);
        let slope = 1.0 - 1.0 / self.ratio.max(1.0);

        let atten = if knee < 0.1 {
            // Hard knee
            if input_db >= threshold {
                0.0
            } else {
                (threshold - input_db) * slope
            }
        } else if input_db > threshold + knee / 2.0 {
            // Above knee zone -- no attenuation
            0.0
        } else if input_db < threshold - knee / 2.0 {
            // Below knee zone -- full gate
            (threshold - input_db) * slope
        } else {
            // Within knee zone: quadratic easing from 0 dB attenuation at
            // threshold + knee/2 to the full below-threshold slope at
            // threshold - knee/2. The curve is continuous at both boundaries
            // and intentionally softer near the opening point.
            let below = threshold + knee / 2.0 - input_db;
            let kf = below / knee;
            kf * kf * (knee / 2.0) * slope
        };

        // A zero range is documented as unlimited. Keep a finite ceiling to
        // avoid inf/NaN propagation when processing denormal/invalid input.
        if self.range_db > 0.0 {
            atten.min(self.range_db)
        } else {
            atten.min(240.0)
        }
    }

    pub(super) fn update_coefficients(&mut self) {
        self.attack_coeff = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate as f32)).exp();
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    /// Rebuild the Butterworth HPF biquad chain from current freq/order/sample_rate.
    pub(super) fn rebuild_sidechain_hpf(&mut self) {
        let fc = self.sidechain_hpf_hz.max(0.0);
        if fc > 0.0 && self.sample_rate > 0 {
            let order = match self.sidechain_hpf_order_index {
                1 => 4,
                _ => 2,
            };
            let peq = peq_butterworth_highpass(order, fc as f64, self.sample_rate as f64);
            // One set of biquad sections per channel (each needs independent state)
            let sections: Vec<Biquad> = peq.into_iter().map(|(_, bq)| bq).collect();
            self.sidechain_hpf_biquads = (0..self.channels).map(|_| sections.clone()).collect();
        } else {
            self.sidechain_hpf_biquads.clear();
        }
    }

    /// Detect level for one sample on a channel, using either peak or RMS mode.
    #[inline]
    pub(super) fn detect_level(&mut self, channel: usize, filtered: f32) -> f32 {
        if self.detection_mode_index == 0 {
            // Peak mode: use abs() directly
            filtered.abs()
        } else {
            // RMS mode: use LevelDetector
            self.level_detectors[channel].process_linear(filtered)
        }
    }

    #[inline]
    pub(super) fn apply_sidechain_filter(&mut self, channel: usize, sample: f32) -> f32 {
        if channel >= self.sidechain_hpf_biquads.len() {
            return sample;
        }
        let biquads: &mut [Biquad] = &mut self.sidechain_hpf_biquads[channel];
        let mut x = sample as f64;
        for bq in biquads.iter_mut() {
            x = bq.process(x);
        }
        x as f32
    }

    fn migrate_link_state(&mut self, was_linked: bool) {
        if self.channels == 0 {
            return;
        }
        if self.link_channels && !was_linked {
            let envelope = self.envelope.iter().copied().fold(0.0_f32, f32::max);
            let gate_open = self.gate_open.iter().copied().any(|open| open);
            let hold = self.hold_counter.iter().copied().max().unwrap_or(0);
            self.envelope.fill(envelope);
            self.gate_open.fill(gate_open);
            self.hold_counter.fill(hold);
        } else if !self.link_channels && was_linked {
            let envelope = self.envelope[0];
            let gate_open = self.gate_open[0];
            let hold = self.hold_counter[0];
            self.envelope.fill(envelope);
            self.gate_open.fill(gate_open);
            self.hold_counter.fill(hold);
        }
    }

    fn params_snapshot(&self) -> GatePluginParams {
        GatePluginParams {
            threshold_db: self.threshold_db,
            ratio: self.ratio,
            attack_ms: self.attack_ms,
            hold_ms: self.hold_ms,
            release_ms: self.release_ms,
            mix: self.mix,
            link_channels: self.link_channels,
            sidechain_hpf_hz: self.sidechain_hpf_hz,
            sidechain_hpf_order: HPF_ORDERS[self.sidechain_hpf_order_index].to_string(),
            detection_mode: DETECTION_MODES[self.detection_mode_index].to_string(),
            sidechain_external: self.sidechain_external,
            range_db: self.range_db,
            hysteresis_db: self.hysteresis_db,
            knee_db: self.knee_db,
            lookahead_ms: self.lookahead_ms,
        }
    }

    fn stage_parameter(
        params: &mut GatePluginParams,
        id: &ParameterId,
        value: &ParameterValue,
    ) -> PluginResult<usize> {
        param_bridge::set_parameter(GT, id, value, |index, value| match index {
            0 => params.threshold_db = value as f32,
            1 => params.ratio = value as f32,
            2 => params.attack_ms = value as f32,
            3 => params.hold_ms = value as f32,
            4 => params.release_ms = value as f32,
            5 => params.mix = value as f32,
            6 => params.link_channels = value > 0.5,
            7 => params.sidechain_hpf_hz = value as f32,
            8 => params.sidechain_hpf_order = HPF_ORDERS[value as usize].to_string(),
            9 => params.detection_mode = DETECTION_MODES[value as usize].to_string(),
            10 => params.sidechain_external = value > 0.5,
            11 => params.range_db = value as f32,
            12 => params.hysteresis_db = value as f32,
            13 => params.knee_db = value as f32,
            14 => params.lookahead_ms = value as f32,
            _ => {}
        })
    }

    fn structural_differs(&self, params: &GatePluginParams) -> bool {
        self.link_channels != params.link_channels
            || self.sidechain_hpf_hz != params.sidechain_hpf_hz
            || !HPF_ORDERS[self.sidechain_hpf_order_index]
                .eq_ignore_ascii_case(&params.sidechain_hpf_order)
            || !DETECTION_MODES[self.detection_mode_index]
                .eq_ignore_ascii_case(&params.detection_mode)
            || self.sidechain_external != params.sidechain_external
            || self.lookahead_ms != params.lookahead_ms
    }

    fn apply_staged_params(&mut self, params: GatePluginParams) {
        let threshold_changed = self.threshold_db != params.threshold_db;
        let timing_changed =
            self.attack_ms != params.attack_ms || self.release_ms != params.release_ms;
        let hold_changed = self.hold_ms != params.hold_ms;
        let mix_changed = self.mix != params.mix;
        let hpf_changed = self.sidechain_hpf_hz != params.sidechain_hpf_hz
            || !HPF_ORDERS[self.sidechain_hpf_order_index]
                .eq_ignore_ascii_case(&params.sidechain_hpf_order);
        let detection_changed = !DETECTION_MODES[self.detection_mode_index]
            .eq_ignore_ascii_case(&params.detection_mode);
        let lookahead_changed = self.lookahead_ms != params.lookahead_ms;
        let was_linked = self.link_channels;

        self.threshold_db = params.threshold_db;
        self.ratio = params.ratio;
        self.attack_ms = params.attack_ms;
        self.hold_ms = params.hold_ms;
        self.release_ms = params.release_ms;
        self.mix = params.mix;
        self.link_channels = params.link_channels;
        self.sidechain_hpf_hz = params.sidechain_hpf_hz;
        self.sidechain_hpf_order_index = usize::from(
            params
                .sidechain_hpf_order
                .eq_ignore_ascii_case(HPF_ORDERS[1]),
        );
        self.detection_mode_index = usize::from(
            params
                .detection_mode
                .eq_ignore_ascii_case(DETECTION_MODES[1]),
        );
        self.sidechain_external = params.sidechain_external;
        self.range_db = params.range_db;
        self.hysteresis_db = params.hysteresis_db;
        self.knee_db = params.knee_db;
        self.lookahead_ms = params.lookahead_ms;

        if threshold_changed {
            self.threshold_smoother.set_target(self.threshold_db);
        }
        if timing_changed {
            self.update_coefficients();
        }
        if hold_changed {
            self.update_hold_samples();
        }
        if mix_changed {
            self.mix_smoother.set_target(self.mix);
        }
        if hpf_changed {
            self.rebuild_sidechain_hpf();
        }
        if detection_changed {
            let mode = if self.detection_mode_index == 1 {
                DetectionMode::Rms { window_ms: 10.0 }
            } else {
                DetectionMode::Peak
            };
            for detector in &mut self.level_detectors {
                detector.set_mode(mode);
            }
        }
        if lookahead_changed {
            self.update_lookahead_delay();
        }
        if was_linked != self.link_channels {
            self.migrate_link_state(was_linked);
        }
    }

    fn structural_value_matches(&self, id: &ParameterId, value: &ParameterValue) -> bool {
        match id.as_str() {
            "link_channels" => value.as_bool() == Some(self.link_channels),
            "sidechain_hpf_hz" => value.as_float() == Some(self.sidechain_hpf_hz),
            "sidechain_hpf_order" => value.as_int() == Some(self.sidechain_hpf_order_index as i32),
            "detection_mode" => value.as_int() == Some(self.detection_mode_index as i32),
            "sidechain_external" => value.as_bool() == Some(self.sidechain_external),
            "lookahead_ms" => value.as_float() == Some(self.lookahead_ms),
            _ => false,
        }
    }

    fn apply_single_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        let structural = matches!(
            id.as_str(),
            "link_channels"
                | "sidechain_hpf_hz"
                | "sidechain_hpf_order"
                | "detection_mode"
                | "sidechain_external"
                | "lookahead_ms"
        );
        if self.initialized && structural {
            self.cached_parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .ok_or_else(|| format!("Unknown parameter: {id}"))?
                .validate(&value)
                .map_err(|error| format!("{id}: {error}"))?;
            if self.structural_value_matches(&id, &value) {
                return Ok(());
            }
            return Err("Gate structural parameter change requires graph rebuild".into());
        }

        let was_linked = self.link_channels;
        let index = param_bridge::set_parameter(GT, &id, &value, |index, value| {
            self.set_param_value(index, value);
        })?;
        match index {
            0 => self.threshold_smoother.set_target(self.threshold_db),
            2 | 4 => self.update_coefficients(),
            3 => self.update_hold_samples(),
            5 => self.mix_smoother.set_target(self.mix),
            7 | 8 => self.rebuild_sidechain_hpf(),
            9 => {
                let mode = if self.detection_mode_index == 1 {
                    DetectionMode::Rms { window_ms: 10.0 }
                } else {
                    DetectionMode::Peak
                };
                for detector in &mut self.level_detectors {
                    detector.set_mode(mode);
                }
            }
            14 => self.update_lookahead_delay(),
            _ => {}
        }
        if was_linked != self.link_channels {
            self.migrate_link_state(was_linked);
        }
        Ok(())
    }
}

impl ParametricInPlacePlugin for GatePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gate", env!("CARGO_PKG_VERSION"), "SotF")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Dynamics
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(
            PluginCostClass::Dynamics,
            None,
            self.latency_samples(),
            self.link_channels || self.sidechain_external,
        )
    }

    fn channels(&self) -> usize {
        self.channels
    }
    fn input_channels(&self) -> usize {
        if self.sidechain_external {
            self.channels
                .checked_mul(2)
                .expect("validated external-sidechain channel count")
        } else {
            self.channels
        }
    }
    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }
    fn current_values(&self) -> ParameterSet {
        let mut values = ParameterSet::new();
        for parameter in &self.cached_parameters {
            let value = match parameter.id.as_str() {
                "threshold" => ParameterValue::Float(self.threshold_db),
                "ratio" => ParameterValue::Float(self.ratio),
                "attack" => ParameterValue::Float(self.attack_ms),
                "hold" => ParameterValue::Float(self.hold_ms),
                "release" => ParameterValue::Float(self.release_ms),
                "mix" => ParameterValue::Float(self.mix),
                "link_channels" => ParameterValue::Bool(self.link_channels),
                "sidechain_hpf_hz" => ParameterValue::Float(self.sidechain_hpf_hz),
                "sidechain_hpf_order" => ParameterValue::Int(self.sidechain_hpf_order_index as i32),
                "detection_mode" => ParameterValue::Int(self.detection_mode_index as i32),
                "sidechain_external" => ParameterValue::Bool(self.sidechain_external),
                "range_db" => ParameterValue::Float(self.range_db),
                "hysteresis_db" => ParameterValue::Float(self.hysteresis_db),
                "knee_db" => ParameterValue::Float(self.knee_db),
                "lookahead_ms" => ParameterValue::Float(self.lookahead_ms),
                _ => continue,
            };
            values.insert(parameter.id.clone(), value);
        }
        values
    }
    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        let mut staged = self.params_snapshot();
        for (id, value) in &values {
            Self::stage_parameter(&mut staged, id, value)?;
        }
        if self.initialized && self.structural_differs(&staged) {
            return Err("Gate structural parameter change requires graph rebuild".into());
        }
        self.apply_staged_params(staged);
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        self.apply_single_parameter(id, value)
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("Gate requires a non-zero sample rate".into());
        }
        self.sample_rate = sample_rate;
        self.diagnostic_interval_samples = (sample_rate as usize / 30).max(1);
        self.diagnostic_samples = 0;
        self.update_coefficients();
        self.update_hold_samples();
        self.rebuild_sidechain_hpf();
        self.threshold_smoother.set_time(5.0, sample_rate);
        self.threshold_smoother.reset(self.threshold_db);
        self.mix_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.reset(self.mix);

        // Reinitialize level detectors with new sample rate
        let mode = if self.detection_mode_index == 1 {
            DetectionMode::Rms { window_ms: 10.0 }
        } else {
            DetectionMode::Peak
        };
        self.level_detectors = (0..self.channels)
            .map(|_| LevelDetector::new(mode, sample_rate))
            .collect();

        let max_samples = (MAX_LOOKAHEAD_MS * 0.001 * sample_rate as f32).round() as usize;
        for buf in &mut self.lookahead_buffers {
            buf.resize(max_samples, 1);
        }
        self.update_lookahead_delay();
        self.initialized = true;
        Ok(())
    }
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.hold_counter.fill(0);
        self.gate_open.fill(false);
        self.monitoring_levels.fill(-120.0);
        self.attenuation_scratch.fill(0.0);
        self.diagnostic_samples = 0;
        self.threshold_smoother.reset(self.threshold_db);
        self.mix_smoother.reset(self.mix);
        // Reset existing filter state in place. Rebuilding the topology here
        // allocates and frees vectors on a lifecycle callback that may run on
        // the realtime thread.
        for channel in &mut self.sidechain_hpf_biquads {
            for biquad in channel {
                biquad.reset();
            }
        }
        for det in &mut self.level_detectors {
            det.reset();
        }
        for buf in &mut self.lookahead_buffers {
            buf.reset();
        }
    }
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        if !self.initialized {
            return Err("Gate must be initialized before processing".into());
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "Gate process sample rate {} does not match initialized sample rate {}",
                context.sample_rate, self.sample_rate
            ));
        }
        if self.channels == 0 {
            return Err("Gate cannot process with zero channels".into());
        }
        let num_frames = context.num_frames;
        let hs = self.hold_samples;
        let use_lookahead = self.lookahead_ms > 0.0;
        let use_ext_sc = self.sidechain_external;
        let close_threshold_ratio = if self.hysteresis_db > 0.0 {
            fast_pow10(-self.hysteresis_db / DB_CONVERSION_FACTOR)
        } else {
            1.0
        };
        // When external sidechain is active, the buffer stride is channels*2
        // (audio channels followed by sidechain channels per frame).
        let stride = if use_ext_sc {
            self.channels
                .checked_mul(2)
                .ok_or_else(|| "Gate channel stride overflow".to_string())?
        } else {
            self.channels
        };

        // Guard against buffer/channel-count mismatch when external sidechain is
        // toggled without rebuilding the plugin with the doubled input width.
        let expected_len = num_frames
            .checked_mul(stride)
            .ok_or_else(|| "Gate buffer size overflow".to_string())?;
        if buffer.len() != expected_len {
            return Err(format!(
                "gate process buffer length {} does not match expected {} for {} channels (external_sidechain={})",
                buffer.len(),
                expected_len,
                self.channels,
                use_ext_sc
            ));
        }

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                let (thresh, threshold_linear) = self.advance_threshold();
                let mix = self.mix_smoother.advance();
                let close_threshold_linear = threshold_linear * close_threshold_ratio;
                let frame_start = frame * stride;
                let sc_offset = if use_ext_sc { self.channels } else { 0 };

                let mut det = 0.0f32;
                for ch in 0..self.channels {
                    let sc_idx = frame_start + sc_offset + ch;
                    let sidechain = if buffer[sc_idx].is_finite() {
                        buffer[sc_idx]
                    } else {
                        0.0
                    };
                    let filtered = self.apply_sidechain_filter(ch, sidechain);
                    let level = self.detect_level(ch, filtered);
                    det = det.max(level);
                    // Update monitoring
                    self.monitoring_levels[ch] =
                        DB_CONVERSION_FACTOR * fast_log10(level.max(EPSILON));
                }

                // Linear-space gate decision (no fast_log10 on hot path)
                let is_open = if self.hysteresis_db <= 0.0 {
                    det >= threshold_linear
                } else if self.gate_open[0] {
                    det >= close_threshold_linear
                } else {
                    det >= threshold_linear
                };
                self.gate_open[0] = is_open;
                let target = if is_open {
                    self.hold_counter[0] = hs;
                    0.0
                } else if self.hold_counter[0] > 0 {
                    self.hold_counter[0] -= 1;
                    0.0
                } else {
                    let idb = DB_CONVERSION_FACTOR * fast_log10(det.max(EPSILON));
                    self.calculate_gate_attenuation(idb, thresh)
                };

                // target > envelope means attenuation is increasing (gate closing) → release.
                // target < envelope means attenuation is decreasing (gate opening) → attack.
                let coeff = if target > self.envelope[0] {
                    self.release_coeff // closing
                } else {
                    self.attack_coeff // opening
                };
                self.envelope[0] = target + coeff * (self.envelope[0] - target);
                let gain = (1.0 - mix) + mix * fast_pow10(-self.envelope[0] / DB_CONVERSION_FACTOR);

                for ch in 0..self.channels {
                    let idx = frame_start + ch;
                    let input = if buffer[idx].is_finite() {
                        buffer[idx]
                    } else {
                        0.0
                    };
                    if use_lookahead {
                        let delayed = self.lookahead_buffers[ch].push(input);
                        buffer[idx] = delayed * gain;
                    } else {
                        buffer[idx] = input * gain;
                    }
                }
            }
        } else {
            for frame in 0..num_frames {
                let (thresh, threshold_linear) = self.advance_threshold();
                let mix = self.mix_smoother.advance();
                let close_threshold_linear = threshold_linear * close_threshold_ratio;
                let frame_start = frame * stride;
                let sc_offset = if use_ext_sc { self.channels } else { 0 };

                for ch in 0..self.channels {
                    let idx = frame_start + ch;
                    let sc_idx = frame_start + sc_offset + ch;
                    let sidechain = if buffer[sc_idx].is_finite() {
                        buffer[sc_idx]
                    } else {
                        0.0
                    };
                    let filtered = self.apply_sidechain_filter(ch, sidechain);
                    let level_abs = self.detect_level(ch, filtered);
                    self.monitoring_levels[ch] =
                        DB_CONVERSION_FACTOR * fast_log10(level_abs.max(EPSILON));

                    // Linear-space gate decision (no fast_log10 on hot path)
                    let is_open = if self.hysteresis_db <= 0.0 {
                        level_abs >= threshold_linear
                    } else if self.gate_open[ch] {
                        level_abs >= close_threshold_linear
                    } else {
                        level_abs >= threshold_linear
                    };
                    self.gate_open[ch] = is_open;
                    let target = if is_open {
                        self.hold_counter[ch] = hs;
                        0.0
                    } else if self.hold_counter[ch] > 0 {
                        self.hold_counter[ch] -= 1;
                        0.0
                    } else {
                        let idb = self.monitoring_levels[ch];
                        self.calculate_gate_attenuation(idb, thresh)
                    };

                    // target > envelope means attenuation is increasing (gate closing) → release.
                    // target < envelope means attenuation is decreasing (gate opening) → attack.
                    let coeff = if target > self.envelope[ch] {
                        self.release_coeff // closing
                    } else {
                        self.attack_coeff // opening
                    };
                    self.envelope[ch] = target + coeff * (self.envelope[ch] - target);
                    let gain =
                        (1.0 - mix) + mix * fast_pow10(-self.envelope[ch] / DB_CONVERSION_FACTOR);
                    let input = if buffer[idx].is_finite() {
                        buffer[idx]
                    } else {
                        0.0
                    };
                    if use_lookahead {
                        let delayed = self.lookahead_buffers[ch].push(input);
                        buffer[idx] = delayed * gain;
                    } else {
                        buffer[idx] = input * gain;
                    }
                }
            }
        }

        // Update diagnostic cache (throttled)
        self.diagnostic_samples = self.diagnostic_samples.saturating_add(num_frames);
        if self.diagnostic_samples >= self.diagnostic_interval_samples {
            self.diagnostic_samples %= self.diagnostic_interval_samples;
            // In linked mode only envelope[0] is updated; envelope[1..] stay at 0.0
            // (their init value), so using any() would always return true even when
            // the gate is fully closed.  Use envelope[0] as the sole authority.
            let is_open = if self.link_channels {
                self.envelope[0] < 0.1
            } else {
                self.envelope.iter().any(|&a| a < 0.1)
            };
            if self.link_channels {
                // Linked processing applies the channel-0 envelope to every
                // audio channel. Mirror that applied gain in diagnostics;
                // exposing the unused per-channel envelope entries would
                // incorrectly report zero attenuation on channels > 0.
                self.attenuation_scratch.fill(self.envelope[0]);
            } else {
                self.attenuation_scratch.copy_from_slice(&self.envelope);
            }
            self.cache.update(|d| {
                d.update(is_open, &self.monitoring_levels, &self.attenuation_scratch);
            });
        }

        // Only flush denormals in the audio output region.  When external sidechain
        // is active the buffer is wider (stride = channels * 2): writing to the
        // sidechain half is harmless but inconsistent with read-only sidechain usage.
        if use_ext_sc {
            for frame in 0..num_frames {
                let audio_start = frame * stride;
                flush_denormals_inplace(&mut buffer[audio_start..audio_start + self.channels]);
            }
        } else {
            flush_denormals_inplace(buffer);
        }
        Ok(num_frames)
    }
    fn latency_samples(&self) -> usize {
        if self.lookahead_ms > 0.0 {
            (self.lookahead_ms * 0.001 * self.sample_rate as f32).round() as usize
        } else {
            0
        }
    }
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}
