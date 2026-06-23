use super::consts::DB_CONVERSION_FACTOR;
use super::consts::EPSILON;
use super::consts::MAX_LOOKAHEAD_MS;
use super::gate_data::GateData;
use super::types::GatePluginParams;
use crate::params::{default_range_db, PARAMS as GT};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, peq_butterworth_highpass};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_bridge;
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
    pub(super) cache: RealTimeCache<GateData>,
    pub(super) cache_update_counter: usize,
    pub(super) cached_parameters: Vec<sotf_host::parameters::Parameter>,
}

impl GatePlugin {
    pub fn new(
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
            cache: RealTimeCache::new(GateData::new(channels)),
            cache_update_counter: 0,
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

    pub fn from_params(channels: usize, params: GatePluginParams) -> Self {
        let mut p = Self::new(
            channels,
            params.threshold_db,
            params.ratio,
            params.attack_ms,
            params.hold_ms,
            params.release_ms,
        );
        p.mix = params.mix.clamp(0.0, 1.0);
        p.link_channels = params.link_channels;
        p.sidechain_hpf_hz = params.sidechain_hpf_hz.max(0.0);

        // HPF order
        p.sidechain_hpf_order_index = match params.sidechain_hpf_order.as_str() {
            "4th" => 1,
            _ => 0, // "2nd" or any unknown
        };

        // Detection mode
        p.detection_mode_index = match params.detection_mode.as_str() {
            "rms" => 1,
            _ => 0, // "peak" or any unknown
        };
        if p.detection_mode_index == 1 {
            let mode = DetectionMode::Rms { window_ms: 10.0 };
            for det in &mut p.level_detectors {
                det.set_mode(mode);
            }
        }

        // External sidechain
        p.sidechain_external = params.sidechain_external;

        p.range_db = params.range_db.max(0.0);
        p.hysteresis_db = params.hysteresis_db.max(0.0);
        p.knee_db = params.knee_db.max(0.0);
        p.lookahead_ms = params.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS);
        p.update_hold_samples();
        p.update_lookahead_delay();
        p.rebuild_cached_parameters();
        p
    }

    pub(super) fn update_hold_samples(&mut self) {
        self.hold_samples = (self.hold_ms * 0.001 * self.sample_rate as f32).round() as usize;
    }

    pub(super) fn update_lookahead_delay(&mut self) {
        for buf in &mut self.lookahead_buffers {
            buf.set_delay_ms(self.lookahead_ms, self.sample_rate);
        }
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

        // Cap attenuation at range_db
        atten.min(self.range_db.max(0.0))
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
}

impl ParametricInPlacePlugin for GatePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gate", "1.3.0", "SotF")
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
            self.channels * 2
        } else {
            self.channels
        }
    }
    fn parameter_schema(&self) -> ParameterSchema {
        self.cached_parameters.clone()
    }
    fn current_values(&self) -> ParameterSet {
        self.cached_parameters
            .iter()
            .map(|p| (p.id.clone(), p.default_value.clone()))
            .collect()
    }
    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            let idx = param_bridge::set_parameter(GT, &id, &value, |i, v| {
                self.set_param_value(i, v);
            })?;
            // Side effects
            match idx {
                0 => self.threshold_smoother.set_target(self.threshold_db), // threshold
                2 | 4 => self.update_coefficients(),                        // attack or release
                3 => self.update_hold_samples(),                            // hold
                5 => self.mix_smoother.set_target(self.mix),                // mix
                7 | 8 => self.rebuild_sidechain_hpf(), // sidechain_hpf_hz or order
                9 => {
                    // detection_mode
                    let mode = if self.detection_mode_index == 1 {
                        DetectionMode::Rms { window_ms: 10.0 }
                    } else {
                        DetectionMode::Peak
                    };
                    for det in &mut self.level_detectors {
                        det.set_mode(mode);
                    }
                }
                14 => self.update_lookahead_delay(), // lookahead_ms
                _ => {}
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn parametric_set_parameter(
        &mut self,
        id: ParameterId,
        value: ParameterValue,
    ) -> PluginResult<()> {
        let mut values = ParameterSet::new();
        values.insert(id, value);
        self.apply_values(values)
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.update_hold_samples();
        self.rebuild_sidechain_hpf();
        self.threshold_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);

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
        Ok(())
    }
    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.hold_counter.fill(0);
        self.gate_open.fill(false);
        // Rebuild biquads to reset their internal state
        self.rebuild_sidechain_hpf();
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
        let num_frames = context.num_frames;
        let hs = self.hold_samples;
        let use_lookahead = self.lookahead_ms > 0.0;
        let use_ext_sc = self.sidechain_external;
        // When external sidechain is active, the buffer stride is channels*2
        // (audio channels followed by sidechain channels per frame).
        let stride = if use_ext_sc {
            self.channels * 2
        } else {
            self.channels
        };

        // Block-based smoothing: advance once per block
        let thresh = self.threshold_smoother.next_n(num_frames);
        let mix = self.mix_smoother.next_n(num_frames);

        // Pre-compute linear thresholds to avoid fast_log10 on the hot audio path
        let threshold_linear = fast_pow10(thresh / DB_CONVERSION_FACTOR);
        let close_threshold_linear = if self.hysteresis_db > 0.0 {
            fast_pow10((thresh - self.hysteresis_db) / DB_CONVERSION_FACTOR)
        } else {
            threshold_linear
        };

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                let frame_start = frame * stride;
                let sc_offset = if use_ext_sc { self.channels } else { 0 };

                let mut det = 0.0f32;
                for ch in 0..self.channels {
                    let sc_idx = frame_start + sc_offset + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[sc_idx]);
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
                    if use_lookahead {
                        let delayed = self.lookahead_buffers[ch].push(buffer[idx]);
                        buffer[idx] = delayed * gain;
                    } else {
                        buffer[idx] *= gain;
                    }
                }
            }
        } else {
            for frame in 0..num_frames {
                let frame_start = frame * stride;
                let sc_offset = if use_ext_sc { self.channels } else { 0 };

                for ch in 0..self.channels {
                    let idx = frame_start + ch;
                    let sc_idx = frame_start + sc_offset + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[sc_idx]);
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
                    if use_lookahead {
                        let delayed = self.lookahead_buffers[ch].push(buffer[idx]);
                        buffer[idx] = delayed * gain;
                    } else {
                        buffer[idx] *= gain;
                    }
                }
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            // In linked mode only envelope[0] is updated; envelope[1..] stay at 0.0
            // (their init value), so using any() would always return true even when
            // the gate is fully closed.  Use envelope[0] as the sole authority.
            let is_open = if self.link_channels {
                self.envelope[0] < 0.1
            } else {
                self.envelope.iter().any(|&a| a < 0.1)
            };
            if self.link_channels {
                self.monitoring_levels.fill(self.envelope[0]);
            } else {
                self.monitoring_levels.copy_from_slice(&self.envelope);
            }
            self.cache.update(|d| {
                d.update(is_open, &self.monitoring_levels);
            });
        }

        // Only flush denormals in the audio output region.  When external sidechain
        // is active the buffer is wider (stride = channels * 2): writing to the
        // sidechain half is harmless but inconsistent with read-only sidechain usage.
        let audio_len = num_frames * self.channels;
        flush_denormals_inplace(&mut buffer[..audio_len]);
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
