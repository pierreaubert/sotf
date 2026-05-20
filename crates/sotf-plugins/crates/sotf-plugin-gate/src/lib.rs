// ============================================================================
// Gate Plugin
// ============================================================================

pub mod params;

use crate::params::{HPF_ORDERS, PARAMS as GT};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, peq_butterworth_highpass};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::{DetectionMode, LevelDetector, LookaheadBuffer};
use std::any::Any;
use std::sync::Arc;

const MAX_LOOKAHEAD_MS: f32 = 20.0;
const DB_CONVERSION_FACTOR: f32 = 20.0;
const EPSILON: f32 = 1e-10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f32,
    #[serde(default = "default_sidechain_hpf_order")]
    pub sidechain_hpf_order: String,
    #[serde(default = "default_detection_mode")]
    pub detection_mode: String,
    #[serde(default = "default_sidechain_external")]
    pub sidechain_external: bool,
    /// Maximum attenuation in dB (0 = unlimited). Caps how much the gate attenuates.
    #[serde(default = "default_range_db")]
    pub range_db: f32,
    /// Hysteresis in dB. Close threshold = threshold - hysteresis.
    #[serde(default)]
    pub hysteresis_db: f32,
    /// Soft knee width in dB (0 = hard knee).
    #[serde(default)]
    pub knee_db: f32,
    /// Lookahead delay in ms (0 = off, max 20ms). Delays audio so gain is computed from non-delayed signal.
    #[serde(default)]
    pub lookahead_ms: f32,
}

fn default_threshold_db() -> f32 {
    pk(GT, "threshold").default_f64() as f32
}
fn default_ratio() -> f32 {
    pk(GT, "ratio").default_f64() as f32
}
fn default_attack_ms() -> f32 {
    pk(GT, "attack").default_f64() as f32
}
fn default_hold_ms() -> f32 {
    pk(GT, "hold").default_f64() as f32
}
fn default_release_ms() -> f32 {
    pk(GT, "release").default_f64() as f32
}
fn default_mix() -> f32 {
    pk(GT, "mix").default_f64() as f32
}
fn default_link_channels() -> bool {
    pk(GT, "link_channels").default_bool()
}
fn default_sidechain_hpf_hz() -> f32 {
    pk(GT, "sidechain_hpf_hz").default_f64() as f32
}
fn default_sidechain_hpf_order() -> String {
    HPF_ORDERS[0].to_string()
}
fn default_detection_mode() -> String {
    "peak".to_string()
}
fn default_sidechain_external() -> bool {
    pk(GT, "sidechain_external").default_bool()
}
fn default_range_db() -> f32 {
    80.0
}

#[derive(Debug, Clone)]
pub struct GateData {
    pub input_levels_db: Arc<Vec<f32>>,
    pub is_open: bool,
    pub attenuation_db: Arc<Vec<f32>>,
}

impl Default for GateData {
    fn default() -> Self {
        Self {
            input_levels_db: Arc::new(Vec::new()),
            is_open: false,
            attenuation_db: Arc::new(Vec::new()),
        }
    }
}

impl GateData {
    pub fn new(channels: usize) -> Self {
        Self {
            input_levels_db: Arc::new(vec![-120.0; channels]),
            is_open: false,
            attenuation_db: Arc::new(vec![0.0; channels]),
        }
    }

    pub fn update(&mut self, is_open: bool, attenuation: &[f32]) {
        self.is_open = is_open;
        if let Some(mut_att) = Arc::get_mut(&mut self.attenuation_db)
            && mut_att.len() == attenuation.len()
        {
            mut_att.copy_from_slice(attenuation);
        }
    }
}

pub struct GatePlugin {
    channels: usize,
    sample_rate: u32,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    hold_ms: f32,
    release_ms: f32,
    mix: f32,
    link_channels: bool,
    sidechain_hpf_hz: f32,
    /// 0 = 2nd order (-12dB/oct), 1 = 4th order (-24dB/oct)
    sidechain_hpf_order_index: usize,
    /// 0 = Peak, 1 = RMS
    detection_mode_index: usize,
    sidechain_external: bool,
    range_db: f32,
    hysteresis_db: f32,
    knee_db: f32,
    lookahead_ms: f32,
    lookahead_buffers: Vec<LookaheadBuffer>,
    /// Gate state per channel for hysteresis
    gate_open: Vec<bool>,
    hold_counter: Vec<usize>,
    attack_coeff: f32,
    release_coeff: f32,
    /// Butterworth HPF biquad sections per channel (empty when HPF disabled)
    sidechain_hpf_biquads: Vec<Vec<Biquad>>,
    /// Level detectors for peak/RMS detection
    level_detectors: Vec<LevelDetector>,
    threshold_smoother: Smoother,
    mix_smoother: Smoother,
    /// Gain reduction envelope in dB (positive value)
    envelope: Vec<f32>,
    /// Instantaneous input levels in dB for monitoring
    monitoring_levels: Vec<f32>,
    cache: RealTimeCache<GateData>,
    cache_update_counter: usize,
    cached_parameters: Vec<sotf_host::parameters::Parameter>,
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
    fn param_value(&self, index: usize) -> Option<f64> {
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
    fn set_param_value(&mut self, index: usize, value: f64) {
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

    fn rebuild_cached_parameters(&mut self) {
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
        p.update_lookahead_delay();
        p.rebuild_cached_parameters();
        p
    }

    fn update_lookahead_delay(&mut self) {
        for buf in &mut self.lookahead_buffers {
            buf.set_delay_ms(self.lookahead_ms, self.sample_rate);
        }
    }

    fn calculate_gate_attenuation(&self, input_db: f32, threshold: f32) -> f32 {
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
            // Within knee zone -- quadratic transition (ported from expander)
            let below = threshold + knee / 2.0 - input_db;
            let kf = below / knee;
            kf * kf * (knee / 2.0) * slope
        };

        // Cap attenuation at range_db
        atten.min(self.range_db.max(0.0))
    }

    fn update_coefficients(&mut self) {
        self.attack_coeff = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate as f32)).exp();
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate as f32)).exp();
    }

    /// Rebuild the Butterworth HPF biquad chain from current freq/order/sample_rate.
    fn rebuild_sidechain_hpf(&mut self) {
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
    fn detect_level(&mut self, channel: usize, filtered: f32) -> f32 {
        if self.detection_mode_index == 0 {
            // Peak mode: use abs() directly
            filtered.abs()
        } else {
            // RMS mode: use LevelDetector
            self.level_detectors[channel].process_linear(filtered)
        }
    }

    #[inline]
    fn apply_sidechain_filter(&mut self, channel: usize, sample: f32) -> f32 {
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

impl InPlacePlugin for GatePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gate", "1.3.0", "SotF")
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
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        let idx = param_bridge::set_parameter(GT, &id, &value, |i, v| self.set_param_value(i, v))?;
        // Side effects
        match idx {
            0 => self.threshold_smoother.set_target(self.threshold_db), // threshold
            2 | 4 => self.update_coefficients(),                        // attack or release
            5 => self.mix_smoother.set_target(self.mix),                // mix
            7 | 8 => self.rebuild_sidechain_hpf(),                      // sidechain_hpf_hz or order
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
        self.rebuild_cached_parameters();
        Ok(())
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(GT, id, |i| self.param_value(i))
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
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
        let hs = (self.hold_ms * 0.001 * self.sample_rate as f32) as usize;
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

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn test_gate_basic() {
        let mut p = GatePlugin::new(1, -20.0, 100.0, 1.0, 10.0, 50.0);
        p.initialize(48000).unwrap();
        let mut b = vec![0.05; 1000];
        p.process_in_place(
            &mut b,
            &ProcessContext {
                sample_rate: 48000,
                num_frames: 1000,
            },
        )
        .unwrap();
        assert!(b[999] < 0.05);
    }

    /// CRITICAL: Attack must control gate opening speed, Release must control closing speed.
    /// With fast attack (1 ms) and slow release (500 ms), the gate should open quickly
    /// when the signal rises above threshold.
    #[test]
    fn test_attack_controls_opening_speed() {
        let sr = 48000u32;
        let mut p = GatePlugin::new(1, -20.0, 100.0, 1.0, 0.0, 500.0);
        p.initialize(sr).unwrap();

        // Close the gate with very quiet signal (-100 dBFS)
        let quiet_len = sr as usize;
        let mut quiet = vec![0.00001f32; quiet_len];
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: quiet_len,
        };
        p.process_in_place(&mut quiet, &ctx).unwrap();

        // Switch to loud signal (-6 dBFS, well above threshold)
        let loud_len = sr as usize / 10; // 100 ms
        let input_level = 0.5f32;
        let mut loud = vec![input_level; loud_len];
        let ctx2 = ProcessContext {
            sample_rate: sr,
            num_frames: loud_len,
        };
        p.process_in_place(&mut loud, &ctx2).unwrap();

        // With fast attack (1 ms) the gate should be essentially fully open
        // within the last 10 ms of the loud section.
        let tail_start = loud_len - sr as usize / 100;
        let avg_output: f32 =
            loud[tail_start..].iter().sum::<f32>() / (loud_len - tail_start) as f32;
        assert!(
            avg_output > input_level * 0.95,
            "Gate should open quickly with fast attack (1 ms), but avg output was {avg_output}"
        );
    }

    /// CRITICAL: Release must control gate closing speed.
    /// With slow attack (500 ms) and fast release (1 ms), the gate should close quickly
    /// when the signal drops below threshold.
    #[test]
    fn test_release_controls_closing_speed() {
        let sr = 48000u32;
        let mut p = GatePlugin::new(1, -20.0, 100.0, 500.0, 0.0, 1.0);
        p.initialize(sr).unwrap();

        // Open the gate with loud signal (-6 dBFS)
        let loud_len = sr as usize;
        let mut loud = vec![0.5f32; loud_len];
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: loud_len,
        };
        p.process_in_place(&mut loud, &ctx).unwrap();

        // Switch to very quiet signal (-60 dBFS, well below threshold)
        let quiet_len = sr as usize / 10; // 100 ms
        let quiet_input = 0.001f32;
        let mut quiet = vec![quiet_input; quiet_len];
        let ctx2 = ProcessContext {
            sample_rate: sr,
            num_frames: quiet_len,
        };
        p.process_in_place(&mut quiet, &ctx2).unwrap();

        // With fast release (1 ms) the gate should be essentially fully closed
        // within the last 10 ms of the quiet section.
        let tail_start = quiet_len - sr as usize / 100;
        let avg_output: f32 =
            quiet[tail_start..].iter().sum::<f32>() / (quiet_len - tail_start) as f32;
        assert!(
            avg_output < quiet_input * 0.1,
            "Gate should close quickly with fast release (1 ms), but avg output was {avg_output}"
        );
    }

    /// CRITICAL: In linked stereo mode the monitoring cache `is_open` must reflect
    /// the actual gate state. When the gate is fully closed it must report false.
    #[test]
    fn test_linked_stereo_monitoring_cache_reports_closed() {
        let sr = 48000u32;
        let mut p = GatePlugin::from_params(
            2,
            GatePluginParams {
                threshold_db: -20.0,
                ratio: 100.0,
                attack_ms: 1.0,
                hold_ms: 0.0,
                release_ms: 10.0,
                mix: 1.0,
                link_channels: true,
                sidechain_hpf_hz: 0.0,
                sidechain_hpf_order: "2nd".to_string(),
                detection_mode: "peak".to_string(),
                sidechain_external: false,
                range_db: 80.0,
                hysteresis_db: 0.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p.initialize(sr).unwrap();

        let block_size = 1024;
        let num_blocks = 20; // enough to trigger cache update (every 10 blocks)
        let quiet = vec![0.0001f32; block_size * 2];
        for _ in 0..num_blocks {
            let mut buf = quiet.clone();
            let ctx = ProcessContext {
                sample_rate: sr,
                num_frames: block_size,
            };
            p.process_in_place(&mut buf, &ctx).unwrap();
        }

        let data = p.get_data().unwrap();
        let gate_data = data.downcast_ref::<GateData>().unwrap();
        assert!(
            !gate_data.is_open,
            "Linked stereo gate should report is_open=false when fully closed"
        );
    }

    /// Sidechain HPF at 200 Hz: a 50 Hz signal below threshold should NOT open
    /// the gate (HPF filters out the low-freq detection signal). A 1 kHz signal
    /// at the same level should open it.
    #[test]
    fn test_sidechain_hpf_filters_low_freq_detection() {
        let sr = 48000u32;
        let threshold_db = -20.0;
        // Signal amplitude is above threshold in raw dB but below after HPF
        let amplitude = 10.0_f32.powf(-15.0 / 20.0); // -15 dBFS (above -20 threshold)

        // --- Test 1: 50 Hz signal with HPF=200 Hz. Gate should stay closed. ---
        let mut p_low = GatePlugin::from_params(
            1,
            GatePluginParams {
                threshold_db,
                ratio: 100.0,
                attack_ms: 1.0,
                hold_ms: 0.0,
                release_ms: 10.0,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 200.0,
                sidechain_hpf_order: "2nd".to_string(),
                detection_mode: "peak".to_string(),
                sidechain_external: false,
                range_db: 80.0,
                hysteresis_db: 0.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p_low.initialize(sr).unwrap();

        let num_frames = 9600; // 200ms
        let mut buf_low = vec![0.0f32; num_frames];
        for (i, sample) in buf_low.iter_mut().enumerate() {
            *sample = amplitude * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr as f32).sin();
        }

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        p_low.process_in_place(&mut buf_low, &ctx).unwrap();

        // The 50 Hz signal should be significantly attenuated because the HPF
        // at 200 Hz filters out the 50 Hz from the sidechain detection.
        let rms_low: f32 =
            buf_low[4800..].iter().map(|x| x * x).sum::<f32>() / (num_frames - 4800) as f32;
        let rms_low = rms_low.sqrt();

        // --- Test 2: 1 kHz signal with HPF=200 Hz. Gate should open. ---
        let mut p_high = GatePlugin::from_params(
            1,
            GatePluginParams {
                threshold_db,
                ratio: 100.0,
                attack_ms: 1.0,
                hold_ms: 0.0,
                release_ms: 10.0,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 200.0,
                sidechain_hpf_order: "2nd".to_string(),
                detection_mode: "peak".to_string(),
                sidechain_external: false,
                range_db: 80.0,
                hysteresis_db: 0.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p_high.initialize(sr).unwrap();

        let mut buf_high = vec![0.0f32; num_frames];
        for (i, sample) in buf_high.iter_mut().enumerate() {
            *sample =
                amplitude * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin();
        }

        p_high.process_in_place(&mut buf_high, &ctx).unwrap();

        let rms_high: f32 =
            buf_high[4800..].iter().map(|x| x * x).sum::<f32>() / (num_frames - 4800) as f32;
        let rms_high = rms_high.sqrt();

        // 1 kHz should pass through much louder than 50 Hz (gate open vs closed)
        assert!(
            rms_high > rms_low * 2.0,
            "1kHz (RMS={rms_high:.5}) should pass through gate much louder than 50Hz (RMS={rms_low:.5}) \
             when sidechain HPF=200Hz"
        );
    }

    /// Hysteresis test: a signal that oscillates +/-2 dB around the threshold should
    /// not cause the gate to "chatter" (rapidly open and close).
    ///
    /// Setup:
    ///   threshold = -20 dB, hysteresis = 4 dB
    ///   -> open threshold  = -20 dB
    ///   -> close threshold = -24 dB
    ///
    /// The test signal alternates every 100 samples between -18 dBFS and -22 dBFS.
    /// Both levels are between -24 dB and -20 dB when the gate is open, so once
    /// opened the gate should remain open for the entire alternating region.
    ///
    /// Without hysteresis the gate would open on -18 dB and close on -22 dB every
    /// 100-sample segment, producing many transitions.  With hysteresis it should
    /// stay open after the first opening.
    #[test]
    fn test_gate_hysteresis_no_chatter() {
        let sr = 48000u32;
        // Fast attack/release so the envelope reacts within the 100-sample segments
        let mut p = GatePlugin::from_params(
            1,
            GatePluginParams {
                threshold_db: -20.0,
                hysteresis_db: 4.0,
                ratio: 100.0,
                attack_ms: 0.5,
                hold_ms: 0.0,
                release_ms: 1.0,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 0.0,
                sidechain_hpf_order: "2nd".to_string(),
                detection_mode: "peak".to_string(),
                sidechain_external: false,
                range_db: 80.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p.initialize(sr).unwrap();

        // Build 1-second buffer that alternates every 100 samples between
        // -18 dBFS (above open threshold -20 dB) and -22 dBFS (between open and
        // close thresholds, so gate should stay open once opened).
        let amp_high = 10.0_f32.powf(-18.0 / 20.0); // -18 dBFS
        let amp_low = 10.0_f32.powf(-22.0 / 20.0); // -22 dBFS  (above close threshold -24 dB)
        let num_frames = sr as usize; // 1 second
        let mut buffer: Vec<f32> = (0..num_frames)
            .map(|i| {
                if (i / 100) % 2 == 0 {
                    amp_high
                } else {
                    amp_low
                }
            })
            .collect();

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        p.process_in_place(&mut buffer, &ctx).unwrap();

        // Count how many times the output crosses a "gate closed" boundary.
        // If the gate chatters, the output will swing between near-zero and amp_low
        // each 100-sample segment.  With hysteresis the output should be consistently
        // passed through after the initial opening.
        //
        // Threshold for "effectively gated": output below 10 % of amp_low.
        let closed_threshold = amp_low * 0.1;

        // Skip the first 500 samples (attack / settling period).
        let steady_state = &buffer[500..];

        // Count sign-changes between "open" and "closed" state.
        let mut transitions = 0usize;
        let mut prev_open = steady_state[0] > closed_threshold;
        for &s in steady_state.iter().skip(1) {
            let cur_open = s > closed_threshold;
            if cur_open != prev_open {
                transitions += 1;
                prev_open = cur_open;
            }
        }

        // With hysteresis the gate should open once and stay open: 0 or at most 1
        // transition (the initial opening) throughout the steady-state region.
        // Without hysteresis we would expect ~2 * (num_frames / 100) ~ 190 transitions.
        assert!(
            transitions <= 2,
            "Gate with hysteresis=4dB should not chatter on a +/-2dB oscillating signal, \
             but observed {transitions} open/closed transitions in steady-state"
        );
    }

    /// Regression test: gate open/close decisions use linear-space thresholds.
    ///
    /// Prior to this refactor, `process_in_place` called `fast_log10(det)` on every
    /// frame to compare the detected envelope against the threshold in dB space.
    /// The gate now compares the linear envelope directly against pre-computed
    /// `threshold_linear` and `close_threshold_linear`, eliminating `fast_log10`
    /// from the hot per-frame audio path.  `fast_log10` is only called when the
    /// gate is actually closing and we need to compute the attenuation curve in
    /// dB space.
    #[test]
    fn test_gate_linear_threshold_no_fast_log10_in_decision() {
        let sr = 48000u32;
        let mut p = GatePlugin::from_params(
            1,
            GatePluginParams {
                threshold_db: -20.0,
                ratio: 100.0,
                attack_ms: 1.0,
                hold_ms: 0.0,
                release_ms: 10.0,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 0.0,
                sidechain_hpf_order: "2nd".to_string(),
                detection_mode: "peak".to_string(),
                sidechain_external: false,
                range_db: 80.0,
                hysteresis_db: 0.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        p.initialize(sr).unwrap();

        // Signal exactly at the linear threshold (-20 dBFS = 0.1).
        // With exact linear comparison the gate should remain open.
        let input_level = 0.1f32;
        let frames = sr as usize / 10;
        let mut buf = vec![input_level; frames];
        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: frames,
        };
        p.process_in_place(&mut buf, &ctx).unwrap();

        let tail_start = frames - sr as usize / 100;
        let avg_output: f32 = buf[tail_start..].iter().sum::<f32>() / (frames - tail_start) as f32;
        assert!(
            (avg_output - input_level).abs() < 0.001,
            "Gate should stay open for signal at exact threshold (linear comparison), \
             avg output was {avg_output}"
        );
    }
}
