use super::misc::CACHE_UPDATE_THROTTLE;
use super::types::LimiterData;
use super::types::LimiterPluginParams;
use crate::params::PARAMS as LM;
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCompiledOp, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::{DualRelease, ParametricInPlacePlugin};
use std::any::Any;
use std::sync::Arc;

const TRUE_PEAK_TAPS: usize = 12;
// Preserve the published BS.1770 Table-2 decimal coefficients verbatim; the
// f32 conversion is intentional for the plugin's f32 realtime signal path.
#[allow(clippy::excessive_precision)]
const TRUE_PEAK_PHASES: [[f32; TRUE_PEAK_TAPS]; 4] = [
    [
        0.001708984375,
        -0.029174804688,
        -0.018920898438,
        0.07763671875,
        0.098388671875,
        -0.189758300781,
        -0.395385742188,
        0.889312744141,
        0.644409179688,
        -0.0517578125,
        -0.024536132813,
        0.001586914063,
    ],
    [
        -0.029174804688,
        0.001708984375,
        0.07763671875,
        -0.018920898438,
        -0.189758300781,
        0.098388671875,
        0.889312744141,
        -0.395385742188,
        -0.0517578125,
        0.644409179688,
        0.001586914063,
        -0.024536132813,
    ],
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [
        -0.024536132813,
        0.001586914063,
        0.644409179688,
        -0.0517578125,
        -0.395385742188,
        0.889312744141,
        0.098388671875,
        -0.189758300781,
        -0.018920898438,
        0.07763671875,
        0.001708984375,
        -0.029174804688,
    ],
];

#[derive(Clone)]
pub(super) struct Bs1770TruePeakDetector {
    pub(super) history: [f32; TRUE_PEAK_TAPS],
}

impl Bs1770TruePeakDetector {
    fn new() -> Self {
        Self {
            history: [0.0; TRUE_PEAK_TAPS],
        }
    }
    fn process_linear(&mut self, sample: f32) -> f32 {
        self.history.copy_within(1.., 0);
        self.history[TRUE_PEAK_TAPS - 1] = sample;
        TRUE_PEAK_PHASES
            .iter()
            .map(|phase| {
                phase
                    .iter()
                    .zip(self.history)
                    .map(|(c, x)| c * x)
                    .sum::<f32>()
                    .abs()
            })
            .fold(sample.abs(), f32::max)
    }
    fn reset(&mut self) {
        self.history.fill(0.0);
    }
}

struct SlidingMaximum {
    values: Vec<f32>,
    indices: Vec<usize>,
    head: usize,
    len: usize,
    next_index: usize,
}

impl SlidingMaximum {
    fn new(capacity: usize) -> Self {
        Self {
            values: vec![0.0; capacity + 1],
            indices: vec![0; capacity + 1],
            head: 0,
            len: 0,
            next_index: 0,
        }
    }
    fn reset(&mut self) {
        self.head = 0;
        self.len = 0;
        self.next_index = 0;
    }
    fn push(&mut self, value: f32, window: usize) -> f32 {
        let capacity = self.values.len();
        while self.len > 0 {
            let tail = (self.head + self.len - 1) % capacity;
            if self.values[tail] > value {
                break;
            }
            self.len -= 1;
        }
        let tail = (self.head + self.len) % capacity;
        self.values[tail] = value;
        self.indices[tail] = self.next_index;
        self.len += 1;
        let oldest = self.next_index.saturating_add(1).saturating_sub(window);
        while self.len > 0 && self.indices[self.head] < oldest {
            self.head = (self.head + 1) % capacity;
            self.len -= 1;
        }
        self.next_index += 1;
        if self.len == 0 {
            value
        } else {
            self.values[self.head]
        }
    }
}

pub struct LimiterPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,
    initialized: bool,
    pub(super) param_threshold: ParameterId,
    pub(super) threshold_db: f32,
    pub(super) param_release: ParameterId,
    pub(super) release_ms: f32,
    pub(super) param_lookahead: ParameterId,
    pub(super) lookahead_ms: f32,
    pub(super) param_soft: ParameterId,
    pub(super) soft: bool,
    pub(super) param_true_peak: ParameterId,
    pub(super) true_peak: bool,
    pub(super) param_isp_mode: ParameterId,
    pub(super) isp_mode: bool,
    pub(super) param_dual_release: ParameterId,
    pub(super) dual_release: bool,
    pub(super) param_mix: ParameterId,
    pub(super) mix: f32,
    pub(super) param_feed_forward: ParameterId,
    pub(super) feed_forward: bool,
    pub(super) param_link_amount: ParameterId,
    pub(super) link_amount: f32,
    /// Threshold is smoothed in dB so equal time intervals produce equal
    /// perceptual gain changes rather than an asymmetric linear-amplitude ramp.
    pub(super) threshold_db_smoother: Smoother,
    pub(super) mix_smoother: Smoother,
    pub(super) envelope: f32,
    pub(super) release_coeff: f32,
    pub(super) lookahead_buffer: Vec<f32>,
    pub(super) lookahead_pos: usize,
    pub(super) lookahead_len: usize,
    pub(super) true_peak_detectors: Vec<Bs1770TruePeakDetector>,
    /// Output ISP detectors for verifying no inter-sample peaks exceed ceiling
    pub(super) output_isp_detectors: Vec<Bs1770TruePeakDetector>,
    /// Accumulated ISP correction in dB from output ISP violations (feedback loop)
    pub(super) isp_correction_db: f32,
    pub(super) dual_release_env: DualRelease,
    pub(super) channel_dual_release: Vec<DualRelease>,
    pub(super) cached_parameters: Vec<Parameter>,
    pub(super) cache: RealTimeCache<LimiterData>,
    pub(super) cache_update_counter: usize,
    pub(super) monitoring_peak_db: f32,
    pub(super) monitoring_gr_db: f32,
    meter_peak_db: f32,
    meter_gr_db: f32,
    /// Per-channel ISP (inter-sample true peak) in linear, tracked across blocks
    pub(super) monitoring_isp_linear: Vec<f32>,
    /// Per-channel peak scratch for the current frame.
    pub(super) channel_peaks: Vec<f32>,
    /// Per-channel gain-reduction envelopes for independent/partial linking.
    pub(super) channel_envelopes: Vec<f32>,
    sliding_maxima: Vec<SlidingMaximum>,
}

impl LimiterPlugin {
    pub fn new(
        channels: usize,
        threshold_db: f32,
        release_ms: f32,
        lookahead_ms: f32,
        soft: bool,
    ) -> Self {
        let sr = 44100;
        let lookahead_len = (lookahead_ms.max(0.0) * 0.001 * sr as f32) as usize;
        let max_lookahead_len = Self::max_lookahead_len(sr);
        let mut p = Self {
            channels,
            sample_rate: sr,
            initialized: false,
            param_threshold: ParameterId::from("threshold"),
            threshold_db,
            param_release: ParameterId::from("release"),
            release_ms,
            param_lookahead: ParameterId::from("lookahead"),
            lookahead_ms,
            param_soft: ParameterId::from("soft"),
            soft,
            param_true_peak: ParameterId::from("true_peak"),
            true_peak: false,
            param_isp_mode: ParameterId::from("isp_mode"),
            isp_mode: false,
            param_dual_release: ParameterId::from("dual_release"),
            dual_release: false,
            param_mix: ParameterId::from("mix"),
            mix: 1.0,
            param_feed_forward: ParameterId::from("feed_forward"),
            feed_forward: false,
            param_link_amount: ParameterId::from("link_amount"),
            link_amount: pk(LM, "link_amount").default_f64() as f32,
            threshold_db_smoother: Smoother::new(threshold_db, 5.0, sr),
            mix_smoother: Smoother::new(1.0, 5.0, sr),
            envelope: 0.0,
            release_coeff: 0.0,
            lookahead_buffer: vec![0.0; max_lookahead_len * channels],
            lookahead_pos: 0,
            lookahead_len,
            true_peak_detectors: (0..channels)
                .map(|_| Bs1770TruePeakDetector::new())
                .collect(),
            output_isp_detectors: (0..channels)
                .map(|_| Bs1770TruePeakDetector::new())
                .collect(),
            isp_correction_db: 0.0,
            dual_release_env: DualRelease::new(release_ms, release_ms * 5.0, sr),
            channel_dual_release: (0..channels)
                .map(|_| DualRelease::new(release_ms, release_ms * 5.0, sr))
                .collect(),
            cached_parameters: Vec::new(),
            cache: RealTimeCache::new(LimiterData {
                isp_dbtp: vec![-120.0; channels],
                ..LimiterData::default()
            }),
            cache_update_counter: 0,
            monitoring_peak_db: -100.0,
            monitoring_gr_db: 0.0,
            meter_peak_db: -100.0,
            meter_gr_db: 0.0,
            monitoring_isp_linear: vec![0.0; channels],
            channel_peaks: vec![0.0; channels],
            channel_envelopes: vec![0.0; channels],
            sliding_maxima: (0..channels.max(1))
                .map(|_| SlidingMaximum::new(max_lookahead_len))
                .collect(),
        };
        p.rebuild_cached_parameters();
        p
    }

    pub(super) fn max_lookahead_len(sample_rate: u32) -> usize {
        let max_ms = pk(LM, "lookahead").max_f64() as f32;
        ((max_ms * 0.001 * sample_rate as f32) as usize).max(1)
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(LM, "threshold").min_f64() as f32,
                pk(LM, "threshold").max_f64() as f32,
            )
            .with_description("Ceiling level (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(LM, "release").min_f64() as f32,
                pk(LM, "release").max_f64() as f32,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "lookahead",
                "Lookahead",
                self.lookahead_ms,
                pk(LM, "lookahead").min_f64() as f32,
                pk(LM, "lookahead").max_f64() as f32,
            )
            .with_description("Structural predictive lookahead / host latency (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("soft", "Soft", self.soft)
                .with_description("Use a one-dB gain-computer knee")
                .with_group("Dynamics")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("true_peak", "True Peak", self.true_peak)
                .with_description("Use ITU-R BS.1770 Table-2 4x true-peak detection")
                .with_group("Detection")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("isp_mode", "ISP Limit", self.isp_mode)
                .with_description("Predictive ISP limit (hard, wet, lookahead required)")
                .with_group("Detection")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("dual_release", "Dual Release", self.dual_release)
                .with_description("Program-dependent fast/slow release")
                .with_group("Timing")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(LM, "mix").min_f64() as f32,
                pk(LM, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = limited)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            // Must match PARAMS order: idx 8=link_amount, idx 9=feed_forward
            Parameter::new_float(
                "link_amount",
                "Link",
                self.link_amount,
                pk(LM, "link_amount").min_f64() as f32,
                pk(LM, "link_amount").max_f64() as f32,
            )
            .with_description("Channel linking (0=independent, 1=linked)")
            .with_group("Detection")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("feed_forward", "Feed Forward", self.feed_forward)
                .with_description("Compatibility flag; lookahead is always predictive")
                .with_group("Detection")
                .with_importance(ParameterImportance::Useful),
        ];
    }

    pub fn from_params(channels: usize, params: LimiterPluginParams) -> Self {
        let finite_or = |value: f32, key: &str| {
            if value.is_finite() {
                value
            } else {
                pk(LM, key).default_f64() as f32
            }
        };
        let threshold = finite_or(params.threshold_db, "threshold").clamp(
            pk(LM, "threshold").min_f64() as f32,
            pk(LM, "threshold").max_f64() as f32,
        );
        let release = finite_or(params.release_ms, "release").clamp(
            pk(LM, "release").min_f64() as f32,
            pk(LM, "release").max_f64() as f32,
        );
        let lookahead = finite_or(params.lookahead_ms, "lookahead").clamp(
            pk(LM, "lookahead").min_f64() as f32,
            pk(LM, "lookahead").max_f64() as f32,
        );
        let mut p = Self::new(channels, threshold, release, lookahead, params.soft);
        p.true_peak = params.true_peak;
        p.isp_mode = params.isp_mode;
        p.dual_release = params.dual_release;
        p.mix = finite_or(params.mix, "mix").clamp(0.0, 1.0);
        p.mix_smoother.reset(p.mix);
        p.feed_forward = params.feed_forward;
        p.link_amount = finite_or(params.link_amount, "link_amount").clamp(0.0, 1.0);
        p.rebuild_cached_parameters();
        p
    }

    pub(super) fn update_coefficients(&mut self) {
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate as f32)).exp();
        let new_len = (self.lookahead_ms.max(0.0) * 0.001 * self.sample_rate as f32) as usize;
        let required_capacity = Self::max_lookahead_len(self.sample_rate);
        let required_buffer_capacity = required_capacity * self.channels;
        if !self.initialized && self.lookahead_buffer.len() < required_buffer_capacity {
            self.lookahead_buffer.resize(required_buffer_capacity, 0.0);
        }
        if new_len != self.lookahead_len {
            self.lookahead_len = new_len;
            self.lookahead_buffer.fill(0.0);
            self.lookahead_pos = 0;
        }
        self.dual_release_env
            .set_times(self.release_ms, self.release_ms * 5.0, self.sample_rate);
        for release in &mut self.channel_dual_release {
            release.set_times(self.release_ms, self.release_ms * 5.0, self.sample_rate);
        }
    }
}

impl ParametricInPlacePlugin for LimiterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Limiter", env!("CARGO_PKG_VERSION"), "SotF")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Dynamics
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
            self.param_threshold.clone(),
            ParameterValue::Float(self.threshold_db),
        );
        values.insert(
            self.param_release.clone(),
            ParameterValue::Float(self.release_ms),
        );
        values.insert(
            self.param_lookahead.clone(),
            ParameterValue::Float(self.lookahead_ms),
        );
        values.insert(self.param_soft.clone(), ParameterValue::Bool(self.soft));
        values.insert(
            self.param_true_peak.clone(),
            ParameterValue::Bool(self.true_peak),
        );
        values.insert(
            self.param_isp_mode.clone(),
            ParameterValue::Bool(self.isp_mode),
        );
        values.insert(
            self.param_dual_release.clone(),
            ParameterValue::Bool(self.dual_release),
        );
        values.insert(self.param_mix.clone(), ParameterValue::Float(self.mix));
        values.insert(
            self.param_feed_forward.clone(),
            ParameterValue::Bool(self.feed_forward),
        );
        values.insert(
            self.param_link_amount.clone(),
            ParameterValue::Float(self.link_amount),
        );
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
            if id == self.param_threshold {
                let val = value
                    .as_float()
                    .unwrap_or(pk(LM, "threshold").default_f64() as f32);
                if val.is_finite() {
                    self.threshold_db = val;
                    self.threshold_db_smoother.set_target(self.threshold_db);
                }
            } else if id == self.param_release {
                let val = value
                    .as_float()
                    .unwrap_or(pk(LM, "release").default_f64() as f32);
                if val.is_finite() {
                    self.release_ms = val.max(1.0);
                    self.update_coefficients();
                }
            } else if id == self.param_lookahead {
                if self.initialized {
                    return Err("lookahead changes latency and requires a graph rebuild".into());
                }
                let val = value
                    .as_float()
                    .unwrap_or(pk(LM, "lookahead").default_f64() as f32);
                if val.is_finite() {
                    self.lookahead_ms = val.max(0.0);
                    self.update_coefficients();
                }
            } else if id == self.param_soft {
                let soft = value.as_bool().unwrap_or(pk(LM, "soft").default_bool());
                if soft && self.isp_mode {
                    return Err("soft knee is unavailable in guaranteed ISP mode".into());
                }
                self.soft = soft;
            } else if id == self.param_true_peak {
                self.true_peak = value
                    .as_bool()
                    .unwrap_or(pk(LM, "true_peak").default_bool());
            } else if id == self.param_isp_mode {
                let enabled = value.as_bool().unwrap_or(pk(LM, "isp_mode").default_bool());
                if enabled && (self.mix < 1.0 || self.soft || self.lookahead_len < 6) {
                    return Err("ISP mode requires 100% wet, hard limiting, and at least six lookahead samples".into());
                }
                self.isp_mode = enabled;
            } else if id == self.param_dual_release {
                self.dual_release = value
                    .as_bool()
                    .unwrap_or(pk(LM, "dual_release").default_bool());
            } else if id == self.param_mix {
                let val = value
                    .as_float()
                    .unwrap_or(pk(LM, "mix").default_f64() as f32);
                if val.is_finite() {
                    if self.isp_mode && val < 1.0 {
                        return Err("ISP mode requires 100% wet mix".into());
                    }
                    self.mix = val.clamp(0.0, 1.0);
                    self.mix_smoother.set_target(self.mix);
                }
            } else if id == self.param_feed_forward {
                self.feed_forward = value.as_bool().unwrap_or(false);
            } else if id == self.param_link_amount {
                let val = value.as_float().unwrap_or(1.0);
                if val.is_finite() {
                    self.link_amount = val.clamp(0.0, 1.0);
                }
            } else {
                return Err(format!("Unknown parameter: {}", id));
            }
        }
        Ok(())
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 || self.channels == 0 {
            return Err("limiter requires nonzero sample rate and channels".into());
        }
        self.sample_rate = sample_rate;
        self.initialized = false;
        self.update_coefficients();
        if self.isp_mode && (self.mix < 1.0 || self.soft || self.lookahead_len < 6) {
            return Err(
                "ISP mode requires 100% wet, hard limiting, and at least six lookahead samples"
                    .into(),
            );
        }
        self.threshold_db_smoother.set_time(5.0, sample_rate);
        self.mix_smoother.set_time(5.0, sample_rate);
        // Resize true peak detectors if channel count changed
        self.true_peak_detectors
            .resize_with(self.channels, Bs1770TruePeakDetector::new);
        self.output_isp_detectors
            .resize_with(self.channels, Bs1770TruePeakDetector::new);
        self.channel_peaks.resize(self.channels, 0.0);
        self.channel_envelopes.resize(self.channels, 0.0);
        self.monitoring_isp_linear.resize(self.channels, 0.0);
        self.isp_correction_db = 0.0;
        self.dual_release_env =
            DualRelease::new(self.release_ms, self.release_ms * 5.0, sample_rate);
        self.channel_dual_release = (0..self.channels)
            .map(|_| DualRelease::new(self.release_ms, self.release_ms * 5.0, sample_rate))
            .collect();
        let capacity = Self::max_lookahead_len(sample_rate);
        self.sliding_maxima = (0..self.channels.max(1))
            .map(|_| SlidingMaximum::new(capacity))
            .collect();
        self.initialized = true;
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.channel_envelopes.fill(0.0);
        self.lookahead_buffer.fill(0.0);
        self.lookahead_pos = 0;
        for det in &mut self.true_peak_detectors {
            det.reset();
        }
        for det in &mut self.output_isp_detectors {
            det.reset();
        }
        self.isp_correction_db = 0.0;
        self.dual_release_env.reset();
        for release in &mut self.channel_dual_release {
            release.reset();
        }
        self.cache_update_counter = 0;
        self.monitoring_peak_db = -100.0;
        self.monitoring_gr_db = 0.0;
        self.meter_peak_db = -100.0;
        self.meter_gr_db = 0.0;
        self.monitoring_isp_linear.fill(0.0);
        for maximum in &mut self.sliding_maxima {
            maximum.reset();
        }
        let threshold = self.threshold_db_smoother.target();
        self.threshold_db_smoother.reset(threshold);
        let mix = self.mix_smoother.target();
        self.mix_smoother.reset(mix);
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
            .ok_or_else(|| "limiter buffer length overflow".to_string())?;
        if self.channels == 0 {
            return Err("limiter requires at least one channel".into());
        }
        if buffer.len() != expected_len {
            return Err(format!(
                "limiter expected {expected_len} samples, got {}",
                buffer.len()
            ));
        }
        let use_true_peak = self.true_peak || self.isp_mode;
        let use_dual_release = self.dual_release;
        // Any non-zero pre-delay must use the upcoming window; otherwise gain
        // releases before the delayed transient reaches the output.
        let use_feed_forward = self.lookahead_len > 0;
        let use_isp_mode = self.isp_mode;

        let link = self.link_amount;
        let meter_interval = (self.sample_rate as usize / CACHE_UPDATE_THROTTLE.max(1)).max(1);

        #[allow(clippy::needless_range_loop)]
        for frame in 0..num_frames {
            let thresh = fast_pow10(self.threshold_db_smoother.advance() / 20.0);
            let mix = self.mix_smoother.advance();

            // Detect per-channel peaks using pre-allocated scratch.
            let nc = self.channels;
            self.channel_peaks[..nc].fill(0.0);
            if use_true_peak {
                for ch in 0..nc {
                    let idx = frame * self.channels + ch;
                    let sample = if buffer[idx].is_finite() {
                        buffer[idx]
                    } else {
                        0.0
                    };
                    buffer[idx] = sample;
                    let tp = self.true_peak_detectors[ch].process_linear(sample);
                    self.channel_peaks[ch] = tp;
                    // Track per-channel ISP
                    if tp > self.monitoring_isp_linear[ch] {
                        self.monitoring_isp_linear[ch] = tp;
                    }
                }
            } else {
                for ch in 0..nc {
                    let idx = frame * self.channels + ch;
                    if !buffer[idx].is_finite() {
                        buffer[idx] = 0.0;
                    }
                    self.channel_peaks[ch] = buffer[idx].abs();
                }
            }

            // Apply channel linking: blend each channel's detector toward the
            // strict linked maximum. At link=0, each channel retains its own
            // detector and therefore its own gain-reduction history.
            let max_peak_ch = self.channel_peaks[..nc]
                .iter()
                .copied()
                .fold(0.0f32, f32::max);
            let fully_linked = link >= 1.0 || nc <= 1;
            let linked_peak = if fully_linked {
                max_peak_ch
            } else {
                let avg_peak = self.channel_peaks[..nc].iter().copied().sum::<f32>() / nc as f32;
                avg_peak * (1.0 - link) + max_peak_ch * link
            };
            for ch in 0..nc {
                if fully_linked {
                    self.channel_peaks[ch] = max_peak_ch;
                } else {
                    self.channel_peaks[ch] =
                        self.channel_peaks[ch] * (1.0 - link) + linked_peak * link;
                }
            }

            let linked_window_peak = (use_feed_forward && fully_linked)
                .then(|| self.sliding_maxima[0].push(self.channel_peaks[0], self.lookahead_len));

            // Update the shared envelope for full linking, or one envelope per
            // channel for partial/independent linking.
            for ch in 0..nc {
                let effective_peak = if use_feed_forward {
                    linked_window_peak.unwrap_or_else(|| {
                        self.sliding_maxima[ch].push(self.channel_peaks[ch], self.lookahead_len)
                    })
                } else {
                    self.channel_peaks[ch]
                };
                let over_db = 20.0 * fast_log10(effective_peak.max(1.0e-20) / thresh);
                let target_gr = if self.soft {
                    const KNEE_DB: f32 = 1.0;
                    if over_db <= -KNEE_DB * 0.5 {
                        0.0
                    } else if over_db >= KNEE_DB * 0.5 {
                        over_db
                    } else {
                        (over_db + KNEE_DB * 0.5).powi(2) / (2.0 * KNEE_DB)
                    }
                } else {
                    over_db.max(0.0)
                } + self.isp_correction_db;
                let envelope = &mut self.channel_envelopes[ch];
                if target_gr > *envelope {
                    *envelope = target_gr;
                } else {
                    let rc = if use_dual_release {
                        if fully_linked {
                            self.dual_release_env.process(*envelope)
                        } else {
                            self.channel_dual_release[ch].process(*envelope)
                        }
                    } else {
                        self.release_coeff
                    };
                    *envelope = target_gr + rc * (*envelope - target_gr);
                }
            }
            self.envelope = self.channel_envelopes[..nc]
                .iter()
                .copied()
                .fold(0.0, f32::max);

            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let input_sample = buffer[idx];

                let delayed = if self.lookahead_len == 0 {
                    input_sample
                } else {
                    let buf_idx = self.lookahead_pos * self.channels + ch;
                    let delayed = self.lookahead_buffer[buf_idx];
                    self.lookahead_buffer[buf_idx] = input_sample;
                    delayed
                };

                let gain = fast_pow10(-self.channel_envelopes[ch] / 20.0);
                let wet = (delayed * gain).clamp(-thresh, thresh);

                buffer[idx] = (1.0 - mix) * delayed + mix * wet;
            }
            // ISP output verification: check output for inter-sample peaks
            // and feed back correction to the next frame's gain computation
            if use_isp_mode {
                let mut frame_output_isp = 0.0f32;
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let output_tp = self.output_isp_detectors[ch].process_linear(buffer[idx]);
                    frame_output_isp = frame_output_isp.max(output_tp);
                }
                if frame_output_isp > thresh {
                    let overshoot = 20.0 * fast_log10(frame_output_isp / thresh);
                    // Accumulate — take max of current correction and new overshoot, capped at 12dB
                    self.isp_correction_db = self.isp_correction_db.max(overshoot).min(12.0);
                } else {
                    // Decay correction in linear gain space — release_coeff is
                    // exp(-1/(release_ms * sr)), designed for linear-domain interpolation.
                    // Applying it multiplicatively to a dB value causes double-exponential
                    // decay (too fast). Convert to linear first.
                    let correction_lin = fast_pow10(self.isp_correction_db / 20.0);
                    let decayed_lin = 1.0 + self.release_coeff * (correction_lin - 1.0);
                    self.isp_correction_db = if decayed_lin <= fast_pow10(0.01 / 20.0) {
                        0.0
                    } else {
                        20.0 * fast_log10(decayed_lin.max(1.0))
                    };
                }
            }

            if self.lookahead_len > 0 {
                self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_len;
            }

            let frame_peak = self.channel_peaks[..nc]
                .iter()
                .copied()
                .fold(0.0_f32, f32::max);
            self.monitoring_peak_db = 20.0 * fast_log10(frame_peak.max(1.0e-10));
            self.monitoring_gr_db = self.envelope;
            self.meter_peak_db = self.meter_peak_db.max(self.monitoring_peak_db);
            self.meter_gr_db = self.meter_gr_db.max(self.monitoring_gr_db);
            self.cache_update_counter += 1;
            if self.cache_update_counter >= meter_interval {
                self.cache_update_counter = 0;
                self.cache.update(|d| {
                    d.gain_reduction_db = self.meter_gr_db;
                    d.peak_db = self.meter_peak_db;
                    d.is_limiting = self.meter_gr_db > 0.01;
                    if d.isp_dbtp.len() == self.channels {
                        if use_true_peak {
                            for (ch, &lin) in self.monitoring_isp_linear.iter().enumerate() {
                                d.isp_dbtp[ch] = if lin < 1e-12 {
                                    -120.0
                                } else {
                                    20.0 * lin.log10()
                                };
                            }
                        } else {
                            d.isp_dbtp.fill(-120.0);
                        }
                    }
                });
                self.meter_peak_db = -100.0;
                self.meter_gr_db = 0.0;
                self.monitoring_isp_linear.fill(0.0);
            }
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn process_compiled_f32(
        &mut self,
        op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<Result<usize, String>> {
        if op != PluginCompiledOp::Limiter {
            return None;
        }
        let sample_len = context.num_frames.checked_mul(self.channels)?;
        if input.len() < sample_len || output.len() < sample_len {
            return Some(Err(format!(
                "limiter compiled buffer too small: need {sample_len} samples, input={}, output={}",
                input.len(),
                output.len()
            )));
        }
        output[..sample_len].copy_from_slice(&input[..sample_len]);
        Some(self.process_in_place(&mut output[..sample_len], context))
    }

    fn latency_samples(&self) -> usize {
        if self.lookahead_ms > 0.0 {
            self.lookahead_len
        } else {
            0
        }
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let latency_samples = self.latency_samples();
        PluginCompileMetadata {
            cost_class: PluginCostClass::Dynamics,
            compiled_op: (latency_samples == 0).then_some(PluginCompiledOp::Limiter),
            static_gain: None,
            linear: false,
            time_invariant_for_block: false,
            channel_mixing: self.link_amount > 0.0 && self.channels > 1,
            stateful: true,
            latency_samples,
            can_absorb_input_gain: false,
            can_absorb_output_gain: false,
            can_merge_with_eq: false,
            boundary: true,
        }
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod remediation_tests {
    use super::*;

    #[test]
    fn monotonic_window_matches_naive_maximum() {
        let values = [0.2, 0.8, 0.4, 0.1, 0.9, 0.3, 0.7, 0.6, 0.05];
        for window in 1..=values.len() {
            let mut maximum = SlidingMaximum::new(values.len());
            for (index, &value) in values.iter().enumerate() {
                let got = maximum.push(value, window);
                let begin = (index + 1).saturating_sub(window);
                let expected = values[begin..=index].iter().copied().fold(0.0, f32::max);
                assert_eq!(got, expected, "window {window}, index {index}");
            }
        }
    }

    #[test]
    fn bs1770_detector_matches_reference_polyphase_convolution() {
        let signal = [0.0, 0.7, -0.9, 0.6, -0.2, 0.1, 0.0, -0.4, 0.8];
        let mut detector = Bs1770TruePeakDetector::new();
        let mut history = [0.0_f32; TRUE_PEAK_TAPS];
        for sample in signal {
            history.copy_within(1.., 0);
            history[TRUE_PEAK_TAPS - 1] = sample;
            let expected = TRUE_PEAK_PHASES
                .iter()
                .map(|phase| {
                    phase
                        .iter()
                        .zip(history)
                        .map(|(coefficient, value)| coefficient * value)
                        .sum::<f32>()
                        .abs()
                })
                .fold(sample.abs(), f32::max);
            assert!((detector.process_linear(sample) - expected).abs() < 1.0e-7);
        }
    }
}
