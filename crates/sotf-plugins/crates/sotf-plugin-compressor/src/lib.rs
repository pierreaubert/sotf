// ============================================================================
// Compressor Plugin
// ============================================================================
//
// Dynamic range compressor that reduces the volume of loud signals.
//
// Parameters:
// - threshold: Level above which compression starts (dB)
// - ratio: Compression ratio (1.0 = no compression, 10.0 = 10:1)
// - attack: Time to reach full compression (ms)
// - release: Time to return to no compression (ms)
// - knee: Soft knee width for smoother compression (dB)
// - makeup_gain: Output gain to compensate for volume reduction (dB)
// - mix: Dry/wet mix between unprocessed and compressed signal (0 = dry, 1 = wet)
// - auto_makeup: Automatically add makeup gain based on threshold/ratio
// - link_channels: Use a shared detector across channels to avoid image shifts
// - sidechain_hpf_hz: High-pass filter cutoff for the detector sidechain (Hz)
// - detection_mode: "peak" or "rms" level detection for sidechain
// - lookahead_ms: Delay audio for anticipatory gain computation (0-20ms)
// - program_dependent_release: Dual-release envelope (fast transients, slow sustain)
// - measured_auto_makeup: Use actual gain reduction average instead of heuristic

use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::{compressor::PARAMS as CP, find_by_key as pk};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use sotf_host::{DetectionMode, DualRelease, LevelDetector, LookaheadBuffer, MeasuredMakeup};
use std::f32::consts::PI;

use std::any::Any;
use std::sync::Arc;

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_SMOOTHING_TIME_MS: f32 = 20.0;
const DEFAULT_SAMPLE_RATE: u32 = 44100;
const CACHE_UPDATE_THROTTLE: usize = 10;
const EPSILON: f32 = 1e-10;
const DB_CONVERSION_FACTOR: f32 = 20.0;
const AUTO_MAKEUP_OVERSHOOT_FACTOR: f32 = 0.5;
const MAX_LOOKAHEAD_MS: f32 = 20.0;
const MEASURED_MAKEUP_SMOOTHING_MS: f32 = 1000.0;
const DUAL_RELEASE_SLOW_MULTIPLIER: f32 = 4.0;

// ============================================================================
// Configuration
// ============================================================================

fn default_threshold_db() -> f32 {
    pk(CP, "threshold").default_f64() as f32
}

fn default_ratio() -> f32 {
    pk(CP, "ratio").default_f64() as f32
}

fn default_attack_ms() -> f32 {
    pk(CP, "attack").default_f64() as f32
}

fn default_release_ms() -> f32 {
    pk(CP, "release").default_f64() as f32
}

fn default_knee_db() -> f32 {
    pk(CP, "knee").default_f64() as f32
}

fn default_makeup_gain_db() -> f32 {
    pk(CP, "makeup_gain").default_f64() as f32
}

fn default_mix() -> f32 {
    pk(CP, "mix").default_f64() as f32
}

pub fn default_auto_makeup() -> bool {
    pk(CP, "auto_makeup").default_bool()
}

pub fn default_link_channels() -> bool {
    pk(CP, "link_channels").default_bool()
}

pub fn default_sidechain_hpf_hz() -> f32 {
    pk(CP, "sidechain_hpf_hz").default_f64() as f32
}

fn default_detection_mode() -> String {
    "peak".to_string()
}

fn default_lookahead_ms() -> f32 {
    pk(CP, "lookahead_ms").default_f64() as f32
}

fn default_program_dependent_release() -> bool {
    pk(CP, "program_dependent_release").default_bool()
}

fn default_measured_auto_makeup() -> bool {
    pk(CP, "measured_auto_makeup").default_bool()
}

fn default_sidechain_external() -> bool {
    pk(CP, "sidechain_external").default_bool()
}

/// Data exposed by the compressor for monitoring
#[derive(Debug, Clone)]
pub struct CompressorData {
    /// Current gain reduction in dB (positive value, e.g., 6.0 means -6dB gain)
    /// One value per channel
    pub gain_reduction_db: Arc<Vec<f32>>,
}

impl Default for CompressorData {
    fn default() -> Self {
        Self {
            gain_reduction_db: Arc::new(Vec::new()),
        }
    }
}

impl CompressorData {
    pub fn new(channels: usize) -> Self {
        Self {
            gain_reduction_db: Arc::new(vec![0.0; channels]),
        }
    }

    pub fn update_gains(&mut self, new_gains: &[f32]) {
        if let Some(mut_gains) = Arc::get_mut(&mut self.gain_reduction_db)
            && mut_gains.len() == new_gains.len()
        {
            mut_gains.copy_from_slice(new_gains);
            return;
        }
        self.gain_reduction_db = Arc::new(new_gains.to_vec());
    }
}

/// Configuration parameters for CompressorPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressorPluginParams {
    #[serde(default = "default_threshold_db")]
    pub threshold_db: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32,
    #[serde(default = "default_release_ms")]
    pub release_ms: f32,
    #[serde(default = "default_knee_db")]
    pub knee_db: f32,
    #[serde(default = "default_makeup_gain_db")]
    pub makeup_gain_db: f32,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_auto_makeup")]
    pub auto_makeup: bool,
    #[serde(default = "default_link_channels")]
    pub link_channels: bool,
    #[serde(default = "default_sidechain_hpf_hz")]
    pub sidechain_hpf_hz: f32,
    #[serde(default = "default_detection_mode")]
    pub detection_mode: String,
    #[serde(default = "default_lookahead_ms")]
    pub lookahead_ms: f32,
    #[serde(default = "default_program_dependent_release")]
    pub program_dependent_release: bool,
    #[serde(default = "default_measured_auto_makeup")]
    pub measured_auto_makeup: bool,
    #[serde(default = "default_sidechain_external")]
    pub sidechain_external: bool,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Dynamic range compressor
pub struct CompressorPlugin {
    channels: usize,
    sample_rate: u32,
    smoothing_time_ms: f32,

    // Parameters
    param_threshold: ParameterId,
    threshold_db: f32,

    param_ratio: ParameterId,
    ratio: f32,

    param_attack: ParameterId,
    attack_ms: f32,

    param_release: ParameterId,
    release_ms: f32,

    param_knee: ParameterId,
    knee_db: f32,

    param_makeup_gain: ParameterId,
    makeup_gain_db: f32,

    param_mix: ParameterId,
    mix: f32,

    param_auto_makeup: ParameterId,
    auto_makeup: bool,

    param_link_channels: ParameterId,
    link_channels: bool,

    param_sidechain_hpf_hz: ParameterId,
    sidechain_hpf_hz: f32,

    param_detection_mode: ParameterId,
    /// 0 = Peak, 1 = RMS
    detection_mode_index: usize,

    param_lookahead_ms: ParameterId,
    lookahead_ms: f32,

    param_program_dependent_release: ParameterId,
    program_dependent_release: bool,

    param_measured_auto_makeup: ParameterId,
    measured_auto_makeup: bool,

    param_sidechain_external: ParameterId,
    sidechain_external: bool,

    cached_parameters: Vec<Parameter>,

    /// Gain reduction envelope in dB (positive value)
    envelope: Vec<f32>,
    /// Buffer for monitoring gain reduction without allocations
    monitoring_levels: Vec<f32>,
    sidechain_hpf_prev_input: Vec<f32>,
    sidechain_hpf_prev_output: Vec<f32>,
    attack_coeff: f32,
    release_coeff: f32,
    sidechain_hpf_alpha: f32,

    // Smoothing
    threshold_smoother: Smoother,
    makeup_gain_smoother: Smoother,

    // Shared utilities
    level_detectors: Vec<LevelDetector>,
    lookahead_buffer: LookaheadBuffer,
    dual_release: Vec<DualRelease>,
    measured_makeup: MeasuredMakeup,
    /// Temp buffer for lookahead delayed frames (avoids per-frame allocation)
    lookahead_frame_buf: Vec<f32>,

    cache: RealTimeCache<CompressorData>,
    cache_update_counter: usize,
}

impl CompressorPlugin {
    /// Create a new compressor plugin
    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        knee_db: f32,
        makeup_gain_db: f32,
    ) -> Self {
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let max_lookahead_samples =
            (MAX_LOOKAHEAD_MS * 0.001 * sample_rate as f32).round() as usize;
        let mut plugin = Self {
            channels,
            sample_rate,
            smoothing_time_ms: DEFAULT_SMOOTHING_TIME_MS,

            param_threshold: ParameterId::from("threshold"),
            threshold_db,

            param_ratio: ParameterId::from("ratio"),
            ratio,

            param_attack: ParameterId::from("attack"),
            attack_ms,

            param_release: ParameterId::from("release"),
            release_ms,

            param_knee: ParameterId::from("knee"),
            knee_db,

            param_makeup_gain: ParameterId::from("makeup_gain"),
            makeup_gain_db,

            param_mix: ParameterId::from("mix"),
            mix: 1.0,

            param_auto_makeup: ParameterId::from("auto_makeup"),
            auto_makeup: false,

            param_link_channels: ParameterId::from("link_channels"),
            link_channels: true,

            param_sidechain_hpf_hz: ParameterId::from("sidechain_hpf_hz"),
            sidechain_hpf_hz: 80.0,

            param_detection_mode: ParameterId::from("detection_mode"),
            detection_mode_index: 0, // Peak

            param_lookahead_ms: ParameterId::from("lookahead_ms"),
            lookahead_ms: 0.0,

            param_program_dependent_release: ParameterId::from("program_dependent_release"),
            program_dependent_release: false,

            param_measured_auto_makeup: ParameterId::from("measured_auto_makeup"),
            measured_auto_makeup: false,

            param_sidechain_external: ParameterId::from("sidechain_external"),
            sidechain_external: false,

            cached_parameters: Vec::new(),

            envelope: vec![0.0; channels],
            monitoring_levels: vec![0.0; channels],
            sidechain_hpf_prev_input: vec![0.0; channels],
            sidechain_hpf_prev_output: vec![0.0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sidechain_hpf_alpha: 0.0,

            threshold_smoother: Smoother::new(threshold_db, DEFAULT_SMOOTHING_TIME_MS, sample_rate),
            makeup_gain_smoother: Smoother::new(
                makeup_gain_db,
                DEFAULT_SMOOTHING_TIME_MS,
                sample_rate,
            ),

            level_detectors: (0..channels)
                .map(|_| LevelDetector::new(DetectionMode::Peak, sample_rate))
                .collect(),
            lookahead_buffer: LookaheadBuffer::new(max_lookahead_samples.max(1), channels),
            dual_release: (0..channels)
                .map(|_| {
                    DualRelease::new(
                        release_ms,
                        release_ms * DUAL_RELEASE_SLOW_MULTIPLIER,
                        sample_rate,
                    )
                })
                .collect(),
            measured_makeup: MeasuredMakeup::new(MEASURED_MAKEUP_SMOOTHING_MS, sample_rate),
            lookahead_frame_buf: vec![0.0; channels],

            cache: RealTimeCache::new(CompressorData::new(channels)),
            cache_update_counter: 0,
        };
        // Lookahead disabled by default (0ms), set delay to minimum so push works
        plugin.lookahead_buffer.set_delay(1);
        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Set smoothing time for parameters (useful for testing)
    pub fn with_smoothing_time(mut self, time_ms: f32) -> Self {
        self.smoothing_time_ms = time_ms;
        self.threshold_smoother.set_time(time_ms, self.sample_rate);
        self.makeup_gain_smoother
            .set_time(time_ms, self.sample_rate);
        self
    }

    fn detection_mode_string(&self) -> String {
        match self.detection_mode_index {
            0 => "peak".to_string(),
            _ => "rms".to_string(),
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold_db,
                pk(CP, "threshold").min_f64() as f32,
                pk(CP, "threshold").max_f64() as f32,
            )
            .with_description("Level above which compression starts (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(CP, "ratio").min_f64() as f32,
                pk(CP, "ratio").max_f64() as f32,
            )
            .with_description("Compression ratio (1:1 to 20:1)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(CP, "attack").min_f64() as f32,
                pk(CP, "attack").max_f64() as f32,
            )
            .with_description("Attack time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(CP, "release").min_f64() as f32,
                pk(CP, "release").max_f64() as f32,
            )
            .with_description("Release time (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "knee",
                "Knee",
                self.knee_db,
                pk(CP, "knee").min_f64() as f32,
                pk(CP, "knee").max_f64() as f32,
            )
            .with_description("Soft knee width (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "makeup_gain",
                "Makeup Gain",
                self.makeup_gain_db,
                pk(CP, "makeup_gain").min_f64() as f32,
                pk(CP, "makeup_gain").max_f64() as f32,
            )
            .with_description("Output gain compensation (dB)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(CP, "mix").min_f64() as f32,
                pk(CP, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = compressed)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("auto_makeup", "Auto Makeup", self.auto_makeup)
                .with_description("Automatically compensate for gain reduction")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_bool("link_channels", "Link Channels", self.link_channels)
                .with_description("Use linked sidechain for all channels")
                .with_group("Channels")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "sidechain_hpf_hz",
                "Sidechain HPF",
                self.sidechain_hpf_hz,
                pk(CP, "sidechain_hpf_hz").min_f64() as f32,
                pk(CP, "sidechain_hpf_hz").max_f64() as f32,
            )
            .with_description("High-pass filter frequency for sidechain (Hz)")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_string(
                "detection_mode",
                "Detection Mode",
                self.detection_mode_string(),
            )
            .with_description("Level detection mode: peak or rms")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "lookahead_ms",
                "Lookahead",
                self.lookahead_ms,
                pk(CP, "lookahead_ms").min_f64() as f32,
                pk(CP, "lookahead_ms").max_f64() as f32,
            )
            .with_description("Lookahead delay for anticipatory compression (ms)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "program_dependent_release",
                "Program Dep. Release",
                self.program_dependent_release,
            )
            .with_description("Use dual-release envelope (fast transients, slow sustain)")
            .with_group("Timing")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "measured_auto_makeup",
                "Measured Auto Makeup",
                self.measured_auto_makeup,
            )
            .with_description("Use measured gain reduction for auto makeup (requires auto_makeup)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "sidechain_external",
                "External Sidechain",
                self.sidechain_external,
            )
            .with_description("Use external sidechain signal from extra input channels")
            .with_group("Sidechain")
            .with_importance(ParameterImportance::Useful),
        ];
    }

    /// Create a new compressor plugin from configuration parameters
    pub fn from_params(channels: usize, params: CompressorPluginParams) -> Self {
        let mut plugin = Self::new(
            channels,
            params.threshold_db,
            params.ratio,
            params.attack_ms,
            params.release_ms,
            params.knee_db,
            params.makeup_gain_db,
        );

        plugin.mix = params.mix.clamp(0.0, 1.0);
        plugin.auto_makeup = params.auto_makeup;
        plugin.link_channels = params.link_channels;
        plugin.sidechain_hpf_hz = params.sidechain_hpf_hz.max(0.0);

        // Detection mode
        plugin.detection_mode_index = match params.detection_mode.as_str() {
            "rms" => 1,
            _ => 0, // "peak" or any unknown
        };
        if plugin.detection_mode_index == 1 {
            let mode = DetectionMode::Rms { window_ms: 10.0 };
            for det in &mut plugin.level_detectors {
                det.set_mode(mode);
            }
        }

        // Lookahead
        plugin.lookahead_ms = params.lookahead_ms.clamp(0.0, MAX_LOOKAHEAD_MS);
        if plugin.lookahead_ms > 0.0 {
            plugin
                .lookahead_buffer
                .set_delay_ms(plugin.lookahead_ms, plugin.sample_rate);
        }

        // Program-dependent release
        plugin.program_dependent_release = params.program_dependent_release;

        // Measured auto makeup
        plugin.measured_auto_makeup = params.measured_auto_makeup;

        // External sidechain
        plugin.sidechain_external = params.sidechain_external;

        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Calculate time coefficient for envelope follower
    fn time_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
        if time_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
        }
    }

    /// Calculate gain reduction for a given input level using fast math
    #[inline]
    fn calculate_gain_reduction(&self, input_db: f32, threshold: f32) -> f32 {
        let knee = self.knee_db.max(0.0);
        let ratio = self.ratio.max(1.0);
        let slope = 1.0 - 1.0 / ratio;

        if knee < 0.1 {
            if input_db <= threshold {
                0.0
            } else {
                let overshoot = input_db - threshold;
                overshoot * slope
            }
        } else if input_db < threshold - knee / 2.0 {
            0.0
        } else if input_db > threshold + knee / 2.0 {
            let overshoot = input_db - threshold;
            overshoot * slope
        } else {
            let overshoot = input_db - threshold + knee / 2.0;
            let knee_factor = overshoot / knee;
            knee_factor * knee_factor * (knee / 2.0) * slope
        }
    }

    /// Update coefficients when parameters change
    fn update_coefficients(&mut self) {
        self.attack_coeff = Self::time_to_coeff(self.attack_ms, self.sample_rate);
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate);

        let fc = self.sidechain_hpf_hz.max(0.0);
        if fc > 0.0 && self.sample_rate > 0 {
            let dt = 1.0 / self.sample_rate as f32;
            let rc = 1.0 / (2.0 * PI * fc);
            self.sidechain_hpf_alpha = rc / (rc + dt);
        } else {
            self.sidechain_hpf_alpha = 0.0;
        }

        // Update dual release times
        for dr in &mut self.dual_release {
            dr.set_times(
                self.release_ms,
                self.release_ms * DUAL_RELEASE_SLOW_MULTIPLIER,
                self.sample_rate,
            );
        }
    }

    /// Detect level for one sample on a channel, using either peak or RMS mode.
    #[inline]
    fn detect_level(&mut self, channel: usize, filtered: f32) -> f32 {
        if self.detection_mode_index == 0 {
            // Peak mode: use abs() directly, convert to dB
            filtered.abs()
        } else {
            // RMS mode: use LevelDetector
            self.level_detectors[channel].process_linear(filtered)
        }
    }

    #[inline]
    fn apply_sidechain_filter(&mut self, channel: usize, sample: f32) -> f32 {
        if self.sidechain_hpf_alpha <= 0.0 {
            return sample;
        }

        let prev_in = self.sidechain_hpf_prev_input[channel];
        let prev_out = self.sidechain_hpf_prev_output[channel];
        let alpha = self.sidechain_hpf_alpha;

        let y = alpha * (prev_out + sample - prev_in);
        self.sidechain_hpf_prev_input[channel] = sample;
        self.sidechain_hpf_prev_output[channel] = y;
        y
    }

    #[inline]
    fn apply_gain_for_channel(
        &mut self,
        channel: usize,
        target_gr: f32,
        makeup_gain_linear: f32,
        input_sample: f32,
        dry_mix: f32,
        wet_mix: f32,
    ) -> f32 {
        let coeff = if target_gr > self.envelope[channel] {
            self.attack_coeff
        } else if self.program_dependent_release {
            self.dual_release[channel].process(target_gr)
        } else {
            self.release_coeff
        };

        // One-pole smoothing for the envelope
        self.envelope[channel] = target_gr + coeff * (self.envelope[channel] - target_gr);

        // Feed measured makeup with current gain reduction
        if self.measured_auto_makeup && self.auto_makeup {
            self.measured_makeup.update(self.envelope[channel]);
        }

        let wet_gain_linear =
            fast_pow10(-self.envelope[channel] / DB_CONVERSION_FACTOR) * makeup_gain_linear;

        let wet = input_sample * wet_gain_linear;
        dry_mix * input_sample + wet_mix * wet
    }

    /// Compute the lookahead delay in samples for the current settings.
    fn lookahead_delay_samples(&self) -> usize {
        if self.lookahead_ms <= 0.0 {
            return 0;
        }
        (self.lookahead_ms * 0.001 * self.sample_rate as f32).round() as usize
    }
}

impl InPlacePlugin for CompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Compressor", "1.3.0", "SotF").with_description(
            "Dynamic range compressor with RMS detection, lookahead, program-dependent release, and measured auto-makeup",
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

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_threshold {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "threshold").default_f64() as f32);
            if val.is_finite() {
                self.threshold_db = val;
                self.threshold_smoother.set_target(val);
            }
        } else if id == self.param_ratio {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "ratio").default_f64() as f32);
            if val.is_finite() {
                self.ratio = val.max(1.0);
            }
        } else if id == self.param_attack {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "attack").default_f64() as f32);
            if val.is_finite() {
                self.attack_ms = val;
                self.update_coefficients();
            }
        } else if id == self.param_release {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "release").default_f64() as f32);
            if val.is_finite() {
                self.release_ms = val;
                self.update_coefficients();
            }
        } else if id == self.param_knee {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "knee").default_f64() as f32);
            if val.is_finite() {
                self.knee_db = val.max(0.0);
            }
        } else if id == self.param_makeup_gain {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "makeup_gain").default_f64() as f32);
            if val.is_finite() {
                self.makeup_gain_db = val;
                self.makeup_gain_smoother.set_target(val);
            }
        } else if id == self.param_mix {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "mix").default_f64() as f32);
            if val.is_finite() {
                self.mix = val.clamp(0.0, 1.0);
            }
        } else if id == self.param_auto_makeup {
            self.auto_makeup = value
                .as_bool()
                .unwrap_or(pk(CP, "auto_makeup").default_bool());
        } else if id == self.param_link_channels {
            self.link_channels = value
                .as_bool()
                .unwrap_or(pk(CP, "link_channels").default_bool());
        } else if id == self.param_sidechain_hpf_hz {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "sidechain_hpf_hz").default_f64() as f32);
            if val.is_finite() {
                self.sidechain_hpf_hz = val.max(0.0);
                self.update_coefficients();
            }
        } else if id == self.param_detection_mode {
            // Accept either String("peak"/"rms") or Float(0.0/1.0) for choice param
            let new_index = if let Some(s) = value.as_string() {
                match s {
                    "rms" | "RMS" => 1,
                    _ => 0,
                }
            } else if let Some(v) = value.as_float() {
                (v as usize).min(1)
            } else {
                0
            };
            self.detection_mode_index = new_index;
            let mode = if new_index == 1 {
                DetectionMode::Rms { window_ms: 10.0 }
            } else {
                DetectionMode::Peak
            };
            for det in &mut self.level_detectors {
                det.set_mode(mode);
            }
        } else if id == self.param_lookahead_ms {
            let val = value
                .as_float()
                .unwrap_or(pk(CP, "lookahead_ms").default_f64() as f32);
            if val.is_finite() {
                self.lookahead_ms = val.clamp(0.0, MAX_LOOKAHEAD_MS);
                if self.lookahead_ms > 0.0 {
                    self.lookahead_buffer
                        .set_delay_ms(self.lookahead_ms, self.sample_rate);
                } else {
                    self.lookahead_buffer.set_delay(1);
                }
            }
        } else if id == self.param_program_dependent_release {
            self.program_dependent_release = value
                .as_bool()
                .unwrap_or(pk(CP, "program_dependent_release").default_bool());
        } else if id == self.param_measured_auto_makeup {
            self.measured_auto_makeup = value
                .as_bool()
                .unwrap_or(pk(CP, "measured_auto_makeup").default_bool());
        } else if id == self.param_sidechain_external {
            self.sidechain_external = value
                .as_bool()
                .unwrap_or(pk(CP, "sidechain_external").default_bool());
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_ratio {
            Some(ParameterValue::Float(self.ratio))
        } else if id == &self.param_attack {
            Some(ParameterValue::Float(self.attack_ms))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_knee {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_makeup_gain {
            Some(ParameterValue::Float(self.makeup_gain_db))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else if id == &self.param_auto_makeup {
            Some(ParameterValue::Bool(self.auto_makeup))
        } else if id == &self.param_link_channels {
            Some(ParameterValue::Bool(self.link_channels))
        } else if id == &self.param_sidechain_hpf_hz {
            Some(ParameterValue::Float(self.sidechain_hpf_hz))
        } else if id == &self.param_detection_mode {
            Some(ParameterValue::String(self.detection_mode_string()))
        } else if id == &self.param_lookahead_ms {
            Some(ParameterValue::Float(self.lookahead_ms))
        } else if id == &self.param_program_dependent_release {
            Some(ParameterValue::Bool(self.program_dependent_release))
        } else if id == &self.param_measured_auto_makeup {
            Some(ParameterValue::Bool(self.measured_auto_makeup))
        } else if id == &self.param_sidechain_external {
            Some(ParameterValue::Bool(self.sidechain_external))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.threshold_smoother
            .set_time(self.smoothing_time_ms, sample_rate);
        self.makeup_gain_smoother
            .set_time(self.smoothing_time_ms, sample_rate);
        self.update_coefficients();

        // Reinitialize level detectors with new sample rate
        let mode = if self.detection_mode_index == 1 {
            DetectionMode::Rms { window_ms: 10.0 }
        } else {
            DetectionMode::Peak
        };
        self.level_detectors = (0..self.channels)
            .map(|_| LevelDetector::new(mode, sample_rate))
            .collect();

        // Reinitialize lookahead buffer
        let max_lookahead_samples =
            (MAX_LOOKAHEAD_MS * 0.001 * sample_rate as f32).round() as usize;
        self.lookahead_buffer
            .resize(max_lookahead_samples.max(1), self.channels);
        if self.lookahead_ms > 0.0 {
            self.lookahead_buffer
                .set_delay_ms(self.lookahead_ms, sample_rate);
        } else {
            self.lookahead_buffer.set_delay(1);
        }

        // Reinitialize dual release
        self.dual_release = (0..self.channels)
            .map(|_| {
                DualRelease::new(
                    self.release_ms,
                    self.release_ms * DUAL_RELEASE_SLOW_MULTIPLIER,
                    sample_rate,
                )
            })
            .collect();

        // Reinitialize measured makeup
        self.measured_makeup = MeasuredMakeup::new(MEASURED_MAKEUP_SMOOTHING_MS, sample_rate);

        Ok(())
    }

    fn reset(&mut self) {
        self.envelope.fill(0.0);
        self.sidechain_hpf_prev_input.fill(0.0);
        self.sidechain_hpf_prev_output.fill(0.0);
        for det in &mut self.level_detectors {
            det.reset();
        }
        self.lookahead_buffer.reset();
        for dr in &mut self.dual_release {
            dr.reset();
        }
        self.measured_makeup.reset();
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();

        let num_frames = context.num_frames;
        let use_lookahead = self.lookahead_ms > 0.0;
        let use_ext_sc = self.sidechain_external;
        // When external sidechain is active, the buffer stride is channels*2
        // (audio channels followed by sidechain channels per frame).
        let stride = if use_ext_sc {
            self.channels * 2
        } else {
            self.channels
        };

        let thresh = self.threshold_smoother.next_n(num_frames);
        let makeup_gain = self.makeup_gain_smoother.next_n(num_frames);

        let dry_mix = 1.0 - self.mix;
        let wet_mix = self.mix;

        // Compute auto makeup gain
        let auto_makeup_db = if self.auto_makeup {
            if self.measured_auto_makeup {
                // Use measured gain reduction average
                self.measured_makeup.makeup_db()
            } else {
                // Heuristic formula
                let ratio = self.ratio.max(1.0);
                let compression_slope = 1.0 - 1.0 / ratio;
                let avg_overshoot = (-thresh).max(0.0) * AUTO_MAKEUP_OVERSHOOT_FACTOR;
                avg_overshoot * compression_slope
            }
        } else {
            0.0
        };
        let makeup_gain_linear = fast_pow10((makeup_gain + auto_makeup_db) / DB_CONVERSION_FACTOR);

        if self.link_channels && self.channels > 1 {
            for frame in 0..num_frames {
                let frame_start = frame * stride;
                // Sidechain detection offset: use sidechain channels if external, else audio channels
                let sc_offset = if use_ext_sc { self.channels } else { 0 };

                // Detect level from sidechain signal (non-delayed)
                let mut detection_level = 0.0_f32;
                for ch in 0..self.channels {
                    let sample_idx = frame_start + sc_offset + ch;
                    let filtered = self.apply_sidechain_filter(ch, buffer[sample_idx]);
                    let level = self.detect_level(ch, filtered);
                    detection_level = detection_level.max(level);
                }

                let input_db = DB_CONVERSION_FACTOR * fast_log10(detection_level.max(EPSILON));
                let target_gr = self.calculate_gain_reduction(input_db, thresh);

                if use_lookahead {
                    // Push current audio frame into lookahead, get delayed frame
                    self.lookahead_buffer.process_frame(
                        &buffer[frame_start..frame_start + self.channels],
                        &mut self.lookahead_frame_buf,
                    );
                    // Apply gain to the delayed audio
                    for ch in 0..self.channels {
                        let sample_idx = frame_start + ch;
                        buffer[sample_idx] = self.apply_gain_for_channel(
                            ch,
                            target_gr,
                            makeup_gain_linear,
                            self.lookahead_frame_buf[ch],
                            dry_mix,
                            wet_mix,
                        );
                    }
                } else {
                    for ch in 0..self.channels {
                        let sample_idx = frame_start + ch;
                        let input_sample = buffer[sample_idx];
                        buffer[sample_idx] = self.apply_gain_for_channel(
                            ch,
                            target_gr,
                            makeup_gain_linear,
                            input_sample,
                            dry_mix,
                            wet_mix,
                        );
                    }
                }
            }
        } else {
            for frame in 0..num_frames {
                let frame_start = frame * stride;
                let sc_offset = if use_ext_sc { self.channels } else { 0 };

                if use_lookahead {
                    // First detect levels from sidechain signal per channel
                    #[allow(clippy::needless_range_loop)]
                    let target_grs = {
                        let mut grs = [0.0_f32; 32];
                        for ch in 0..self.channels {
                            let sample_idx = frame_start + sc_offset + ch;
                            let filtered = self.apply_sidechain_filter(ch, buffer[sample_idx]);
                            let level = self.detect_level(ch, filtered);
                            let input_db =
                                DB_CONVERSION_FACTOR * fast_log10(level.max(EPSILON));
                            grs[ch] = self.calculate_gain_reduction(input_db, thresh);
                        }
                        grs
                    };

                    // Push audio frame through lookahead
                    self.lookahead_buffer.process_frame(
                        &buffer[frame_start..frame_start + self.channels],
                        &mut self.lookahead_frame_buf,
                    );

                    // Apply per-channel gain to delayed audio
                    #[allow(clippy::needless_range_loop)]
                    for ch in 0..self.channels {
                        let sample_idx = frame_start + ch;
                        buffer[sample_idx] = self.apply_gain_for_channel(
                            ch,
                            target_grs[ch],
                            makeup_gain_linear,
                            self.lookahead_frame_buf[ch],
                            dry_mix,
                            wet_mix,
                        );
                    }
                } else {
                    for ch in 0..self.channels {
                        let sample_idx = frame_start + ch;
                        let input_sample = buffer[sample_idx];
                        let sc_sample = buffer[frame_start + sc_offset + ch];
                        let filtered = self.apply_sidechain_filter(ch, sc_sample);
                        let level = self.detect_level(ch, filtered);
                        let input_db = DB_CONVERSION_FACTOR * fast_log10(level.max(EPSILON));
                        let target_gr = self.calculate_gain_reduction(input_db, thresh);

                        buffer[sample_idx] = self.apply_gain_for_channel(
                            ch,
                            target_gr,
                            makeup_gain_linear,
                            input_sample,
                            dry_mix,
                            wet_mix,
                        );
                    }
                }
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= CACHE_UPDATE_THROTTLE {
            self.cache_update_counter = 0;

            if self.link_channels {
                self.monitoring_levels.fill(self.envelope[0]);
            } else {
                self.monitoring_levels.copy_from_slice(&self.envelope);
            }

            self.cache.update(|d| {
                d.update_gains(&self.monitoring_levels);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_delay_samples()
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use sotf_host::*;

    #[test]
    fn test_compressor_creation() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        assert_eq!(compressor.channels(), 2);
        assert_eq!(compressor.threshold_db, -20.0);
        assert_eq!(compressor.ratio, 4.0);
    }

    #[test]
    fn test_compressor_gain_reduction() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0); // No knee for simple test

        // Below threshold - no compression
        let gr = compressor.calculate_gain_reduction(-30.0, -20.0);
        assert_eq!(gr, 0.0);

        // At threshold - no compression
        let gr = compressor.calculate_gain_reduction(-20.0, -20.0);
        assert_eq!(gr, 0.0);

        // 12 dB above threshold with 4:1 ratio
        // Gain reduction = 12 * (1 - 1/4) = 9 dB
        let gr = compressor.calculate_gain_reduction(-8.0, -20.0);
        assert!((gr - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_compressor_additional_parameters_defaults() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);

        assert_eq!(compressor.mix, 1.0);
        assert!(!compressor.auto_makeup);
        assert!(compressor.link_channels);
        assert_eq!(compressor.sidechain_hpf_hz, 80.0);
        assert_eq!(compressor.detection_mode_index, 0); // Peak
        assert_eq!(compressor.lookahead_ms, 0.0);
        assert!(!compressor.program_dependent_release);
        assert!(!compressor.measured_auto_makeup);
    }

    #[test]
    fn test_compressor_mix_and_sidechain_parameters_set_get() {
        let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        compressor.initialize(48000).unwrap();

        compressor
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();
        compressor
            .set_parameter(
                ParameterId::from("sidechain_hpf_hz"),
                ParameterValue::Float(120.0),
            )
            .unwrap();

        let mix = compressor.get_parameter(&ParameterId::from("mix"));
        let sidechain = compressor.get_parameter(&ParameterId::from("sidechain_hpf_hz"));

        assert_eq!(mix, Some(ParameterValue::Float(0.5)));
        assert_eq!(sidechain, Some(ParameterValue::Float(120.0)));
    }

    #[test]
    fn test_compressor_processing_varied_buffers() {
        use sotf_host::{InPlacePluginAdapter, Plugin, test_varied_buffer_sizes};
        let sample_rate = 48000.0;
        let channels = 2;
        let mut inner = CompressorPlugin::new(channels, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0)
            .with_smoothing_time(0.0);
        inner.initialize(sample_rate as u32).unwrap();
        let mut plugin = InPlacePluginAdapter::new(inner);

        // Generate a 1 second sine wave at -10dB (above threshold)
        let mut signal_gen = SignalGen::new_sine(sample_rate, 1000.0, 0.316); // -10dB approx
        let input = signal_gen.generate(4800 * channels); // 100ms is enough for CI

        // Generate reference output with standard block size
        let mut expected_output = vec![0.0; input.len()];
        let ctx = ProcessContext {
            sample_rate: sample_rate as u32,
            num_frames: 4800,
        };
        plugin.process(&input, &mut expected_output, &ctx).unwrap();

        // Reset and test varied buffer sizes
        plugin.reset();
        test_varied_buffer_sizes(&mut plugin, sample_rate, &input, &expected_output);
    }

    #[test]
    fn test_compressor_rt_safety() {
        use sotf_host::{InPlacePluginAdapter, Plugin, assert_no_allocs};
        let sample_rate = 48000;
        let channels = 2;
        let mut inner = CompressorPlugin::new(channels, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0);
        inner.initialize(sample_rate).unwrap();
        let mut plugin = InPlacePluginAdapter::new(inner);

        let input = vec![0.1; 512 * channels];
        let mut output = vec![0.0; 512 * channels];
        let ctx = ProcessContext {
            sample_rate,
            num_frames: 512,
        };

        // Warm up
        for _ in 0..10 {
            plugin.process(&input, &mut output, &ctx).unwrap();
        }

        assert_no_allocs("CompressorPlugin::process", || {
            plugin.process(&input, &mut output, &ctx).unwrap();
        });
    }

    // ========================================================================
    // New feature tests
    // ========================================================================

    #[test]
    fn test_detection_mode_parameter() {
        let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0);
        compressor.initialize(48000).unwrap();

        // Default is peak
        assert_eq!(
            compressor.get_parameter(&ParameterId::from("detection_mode")),
            Some(ParameterValue::String("peak".to_string()))
        );

        // Set to RMS
        compressor
            .set_parameter(
                ParameterId::from("detection_mode"),
                ParameterValue::String("rms".to_string()),
            )
            .unwrap();
        assert_eq!(compressor.detection_mode_index, 1);
        assert_eq!(
            compressor.get_parameter(&ParameterId::from("detection_mode")),
            Some(ParameterValue::String("rms".to_string()))
        );

        // Set back to peak via string
        compressor
            .set_parameter(
                ParameterId::from("detection_mode"),
                ParameterValue::String("peak".to_string()),
            )
            .unwrap();
        assert_eq!(compressor.detection_mode_index, 0);
    }

    #[test]
    fn test_rms_detection_processes() {
        let mut compressor = CompressorPlugin::new(1, -20.0, 4.0, 0.1, 50.0, 0.0, 0.0)
            .with_smoothing_time(0.0);
        compressor.initialize(48000).unwrap();
        compressor
            .set_parameter(
                ParameterId::from("detection_mode"),
                ParameterValue::String("rms".to_string()),
            )
            .unwrap();
        compressor.link_channels = false;

        // Verify the detection mode was actually applied
        assert_eq!(compressor.detection_mode_index, 1, "Should be RMS mode");

        // Process a loud signal — RMS should still cause compression
        // Use a longer signal to ensure the RMS window is fully populated
        let num_frames = 48000; // 1 second
        let mut buffer: Vec<f32> = (0..num_frames).map(|i| {
            0.5 * (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin()
        }).collect();
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        compressor.process_in_place(&mut buffer, &ctx).unwrap();

        // Check the peak of the last 1000 frames (RMS window is definitely filled)
        let tail_peak: f32 = buffer[num_frames - 1000..]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        assert!(
            tail_peak < 0.49,
            "RMS mode should compress signal (tail peak={tail_peak})"
        );
    }

    #[test]
    fn test_lookahead_parameter() {
        let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0);
        compressor.initialize(48000).unwrap();

        // Default is 0
        assert_eq!(
            compressor.get_parameter(&ParameterId::from("lookahead_ms")),
            Some(ParameterValue::Float(0.0))
        );
        assert_eq!(compressor.latency_samples(), 0);

        // Set to 5ms
        compressor
            .set_parameter(
                ParameterId::from("lookahead_ms"),
                ParameterValue::Float(5.0),
            )
            .unwrap();
        assert_eq!(compressor.lookahead_ms, 5.0);
        // 5ms at 48000 = 240 samples
        assert_eq!(compressor.latency_samples(), 240);
    }

    #[test]
    fn test_lookahead_delays_audio() {
        let channels = 1;
        let mut compressor = CompressorPlugin::new(channels, 0.0, 1.0, 100.0, 100.0, 0.0, 0.0)
            .with_smoothing_time(0.0);
        compressor.initialize(48000).unwrap();
        compressor
            .set_parameter(
                ParameterId::from("lookahead_ms"),
                ParameterValue::Float(5.0),
            )
            .unwrap();
        compressor.link_channels = false;

        // ratio 1.0 = no compression, so output should be delayed version of input
        let delay_samples = 240; // 5ms * 48000
        let num_frames = delay_samples + 100;
        let mut buffer = vec![0.0_f32; num_frames];
        // Put an impulse at frame 0
        buffer[0] = 1.0;

        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        compressor.process_in_place(&mut buffer, &ctx).unwrap();

        // The impulse should appear at frame `delay_samples`
        let peak_idx = buffer
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .unwrap()
            .0;
        assert_eq!(
            peak_idx, delay_samples,
            "Impulse should be delayed by {delay_samples} samples, found at {peak_idx}"
        );
    }

    #[test]
    fn test_program_dependent_release_parameter() {
        let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0);
        compressor.initialize(48000).unwrap();

        assert_eq!(
            compressor.get_parameter(&ParameterId::from("program_dependent_release")),
            Some(ParameterValue::Bool(false))
        );

        compressor
            .set_parameter(
                ParameterId::from("program_dependent_release"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        assert!(compressor.program_dependent_release);
    }

    #[test]
    fn test_program_dependent_release_processes() {
        let mut compressor = CompressorPlugin::new(1, -20.0, 4.0, 0.1, 50.0, 0.0, 0.0)
            .with_smoothing_time(0.0);
        compressor.initialize(48000).unwrap();
        compressor
            .set_parameter(
                ParameterId::from("program_dependent_release"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        compressor.link_channels = false;

        // Process a loud burst followed by silence
        let num_frames = 9600; // 200ms
        let mut buffer: Vec<f32> = (0..num_frames).map(|i| {
            if i < 2400 {
                // 50ms of loud signal
                0.5 * (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin()
            } else {
                0.0
            }
        }).collect();

        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        compressor.process_in_place(&mut buffer, &ctx).unwrap();
        // Just verify it runs without panicking
    }

    #[test]
    fn test_measured_auto_makeup_parameter() {
        let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0);
        compressor.initialize(48000).unwrap();

        assert_eq!(
            compressor.get_parameter(&ParameterId::from("measured_auto_makeup")),
            Some(ParameterValue::Bool(false))
        );

        compressor
            .set_parameter(
                ParameterId::from("measured_auto_makeup"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        assert!(compressor.measured_auto_makeup);
    }

    #[test]
    fn test_measured_auto_makeup_compensates() {
        let channels = 1;
        let mut compressor = CompressorPlugin::new(channels, -20.0, 4.0, 0.1, 50.0, 0.0, 0.0)
            .with_smoothing_time(0.0);
        compressor.initialize(48000).unwrap();
        compressor.auto_makeup = true;
        compressor.measured_auto_makeup = true;
        compressor.link_channels = false;
        compressor.rebuild_cached_parameters();

        // Process a sustained loud signal to let measured makeup converge
        let num_frames = 48000; // 1 second
        let mut buffer: Vec<f32> = (0..num_frames).map(|i| {
            0.3 * (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin()
        }).collect();

        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        compressor.process_in_place(&mut buffer, &ctx).unwrap();

        // The measured makeup should have tracked the average gain reduction
        let makeup = compressor.measured_makeup.makeup_db();
        assert!(
            makeup > 0.0,
            "Measured makeup should be positive when compressing, got {makeup}"
        );
    }

    #[test]
    fn test_from_params_new_fields() {
        let params = CompressorPluginParams {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 6.0,
            makeup_gain_db: 0.0,
            mix: 1.0,
            auto_makeup: false,
            link_channels: true,
            sidechain_hpf_hz: 80.0,
            detection_mode: "rms".to_string(),
            lookahead_ms: 5.0,
            program_dependent_release: true,
            measured_auto_makeup: true,
            sidechain_external: false,
        };
        let mut plugin = CompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();

        assert_eq!(plugin.detection_mode_index, 1);
        assert_eq!(plugin.lookahead_ms, 5.0);
        assert!(plugin.program_dependent_release);
        assert!(plugin.measured_auto_makeup);
        assert_eq!(plugin.latency_samples(), 240);
    }

    #[test]
    fn test_serde_defaults_new_fields() {
        // Deserialize with no new fields — defaults should apply
        let json = r#"{"threshold_db": -20.0}"#;
        let params: CompressorPluginParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.detection_mode, "peak");
        assert_eq!(params.lookahead_ms, 0.0);
        assert!(!params.program_dependent_release);
        assert!(!params.measured_auto_makeup);
    }

    #[test]
    fn test_lookahead_linked_channels() {
        // Ensure lookahead works with linked channels (>1 channel)
        let channels = 2;
        let mut compressor =
            CompressorPlugin::new(channels, 0.0, 1.0, 100.0, 100.0, 0.0, 0.0)
                .with_smoothing_time(0.0);
        compressor.initialize(48000).unwrap();
        compressor
            .set_parameter(
                ParameterId::from("lookahead_ms"),
                ParameterValue::Float(5.0),
            )
            .unwrap();

        let delay_samples = 240;
        let num_frames = delay_samples + 100;
        let mut buffer = vec![0.0_f32; num_frames * channels];
        // Put impulse at frame 0, channel 0
        buffer[0] = 1.0;

        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };
        compressor.process_in_place(&mut buffer, &ctx).unwrap();

        // Check channel 0: impulse should be at frame delay_samples
        let ch0_peak_frame = (0..num_frames)
            .max_by(|&a, &b| {
                buffer[a * channels]
                    .abs()
                    .partial_cmp(&buffer[b * channels].abs())
                    .unwrap()
            })
            .unwrap();
        assert_eq!(ch0_peak_frame, delay_samples);
    }

    #[test]
    fn test_program_dependent_release_slower_than_fixed() {
        // With program_dependent_release, a sustained loud signal should
        // cause a slower release than without it. We compare the envelope
        // state after a loud burst followed by silence.
        let sr = 48000;
        let channels = 1;
        let release_ms = 30.0; // short release for measurable difference

        // Generate: 50ms loud burst + 100ms silence
        let burst_frames = (0.05 * sr as f32) as usize;
        let silence_frames = (0.1 * sr as f32) as usize;
        let total = burst_frames + silence_frames;

        let make_signal = || -> Vec<f32> {
            (0..total)
                .map(|i| {
                    if i < burst_frames {
                        0.8 * (2.0 * PI * 1000.0 * i as f32 / sr as f32).sin()
                    } else {
                        0.0
                    }
                })
                .collect()
        };

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames: total,
        };

        // Fixed release
        let mut comp_fixed =
            CompressorPlugin::new(channels, -20.0, 8.0, 0.1, release_ms, 0.0, 0.0)
                .with_smoothing_time(0.0);
        comp_fixed.initialize(sr).unwrap();
        comp_fixed.program_dependent_release = false;
        comp_fixed.link_channels = false;
        let mut buf_fixed = make_signal();
        comp_fixed
            .process_in_place(&mut buf_fixed, &ctx)
            .unwrap();

        // Program-dependent release
        let mut comp_pdr =
            CompressorPlugin::new(channels, -20.0, 8.0, 0.1, release_ms, 0.0, 0.0)
                .with_smoothing_time(0.0);
        comp_pdr.initialize(sr).unwrap();
        comp_pdr.program_dependent_release = true;
        comp_pdr.link_channels = false;
        let mut buf_pdr = make_signal();
        comp_pdr
            .process_in_place(&mut buf_pdr, &ctx)
            .unwrap();

        // After the burst, the envelope still has gain reduction.
        // With program-dependent release (dual release with slow multiplier 4x),
        // the envelope should release slower. Check that the envelope at the end
        // is larger (more gain reduction remaining) with PDR.
        assert!(
            comp_pdr.envelope[0] > comp_fixed.envelope[0],
            "Program-dependent release should have more residual gain reduction: \
             PDR envelope={}, fixed envelope={}",
            comp_pdr.envelope[0],
            comp_fixed.envelope[0]
        );
    }

    /// RMS vs peak detection: a compressor in RMS mode should respond more slowly
    /// to a short transient than one in peak mode, so its output peak amplitude
    /// should be higher (less compression applied to the transient).
    ///
    /// Signal: 5 ms loud burst (0 dBFS sine) followed by silence.  Total duration
    /// is 1 second so the RMS window (10 ms) is well-defined in both modes.
    ///
    /// Peak mode detects the instantaneous amplitude → compresses the burst hard.
    /// RMS mode integrates over ~10 ms → the 5 ms burst barely raises the RMS
    /// level, so less compression is applied and the transient passes louder.
    #[test]
    fn test_compressor_rms_vs_peak_detection() {
        let sr = 48000u32;
        // Very aggressive settings: high ratio, fast attack, so peak mode hammers
        // the transient visibly.
        let make_comp = |detection: &str| -> CompressorPlugin {
            let mut c = CompressorPlugin::new(1, -20.0, 20.0, 0.1, 200.0, 0.0, 0.0)
                .with_smoothing_time(0.0);
            c.initialize(sr).unwrap();
            c.link_channels = false;
            c.set_parameter(
                ParameterId::from("detection_mode"),
                ParameterValue::String(detection.to_string()),
            )
            .unwrap();
            c
        };

        // 5 ms burst at 0 dBFS, then silence for the rest of 1 second
        let burst_frames = (0.005 * sr as f32) as usize; // 240 samples @ 48 kHz
        let num_frames = sr as usize;
        let signal: Vec<f32> = (0..num_frames)
            .map(|i| {
                if i < burst_frames {
                    (2.0 * PI * 1000.0 * i as f32 / sr as f32).sin()
                } else {
                    0.0
                }
            })
            .collect();

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };

        let mut peak_comp = make_comp("peak");
        let mut peak_buf = signal.clone();
        peak_comp
            .process_in_place(&mut peak_buf, &ctx)
            .unwrap();

        let mut rms_comp = make_comp("rms");
        let mut rms_buf = signal.clone();
        rms_comp
            .process_in_place(&mut rms_buf, &ctx)
            .unwrap();

        // Measure peak amplitude during the burst window in each output
        let peak_mode_peak: f32 = peak_buf[..burst_frames]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        let rms_mode_peak: f32 = rms_buf[..burst_frames]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);

        // RMS mode is slower to react → transient passes through with higher amplitude
        assert!(
            rms_mode_peak > peak_mode_peak,
            "RMS detection should compress transients less than peak detection: \
             rms_mode_peak={rms_mode_peak:.5}, peak_mode_peak={peak_mode_peak:.5}"
        );
    }

    #[test]
    fn test_rt_safety_with_features_enabled() {
        use sotf_host::{InPlacePluginAdapter, Plugin, assert_no_allocs};
        let sample_rate = 48000;
        let channels = 2;
        let mut inner = CompressorPlugin::new(channels, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0);
        inner.initialize(sample_rate).unwrap();
        // Enable all new features
        inner
            .set_parameter(
                ParameterId::from("detection_mode"),
                ParameterValue::String("rms".to_string()),
            )
            .unwrap();
        inner
            .set_parameter(
                ParameterId::from("lookahead_ms"),
                ParameterValue::Float(5.0),
            )
            .unwrap();
        inner
            .set_parameter(
                ParameterId::from("program_dependent_release"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        inner.auto_makeup = true;
        inner
            .set_parameter(
                ParameterId::from("measured_auto_makeup"),
                ParameterValue::Bool(true),
            )
            .unwrap();

        let mut plugin = InPlacePluginAdapter::new(inner);

        let input = vec![0.1; 512 * channels];
        let mut output = vec![0.0; 512 * channels];
        let ctx = ProcessContext {
            sample_rate,
            num_frames: 512,
        };

        // Warm up
        for _ in 0..10 {
            plugin.process(&input, &mut output, &ctx).unwrap();
        }

        assert_no_allocs("CompressorPlugin::process with all features", || {
            plugin.process(&input, &mut output, &ctx).unwrap();
        });
    }
}
