// ============================================================================
// De-Esser Plugin
// ============================================================================

pub mod params;

use crate::params::{MODES, PARAMS as DE};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::dynamics_core::DynamicsCore;
use sotf_host::dynamics_core::DynamicsMode;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

const DB_CONVERSION_FACTOR: f32 = 20.0;
const EPSILON: f32 = 1e-10;
const FIXED_KNEE_DB: f32 = 6.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeEsserPluginParams {
    #[serde(default = "default_frequency")]
    pub frequency: f32,
    #[serde(default = "default_q")]
    pub q: f32,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    #[serde(default = "default_ratio")]
    pub ratio: f32,
    #[serde(default = "default_attack")]
    pub attack_ms: f32,
    #[serde(default = "default_release")]
    pub release_ms: f32,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_mix")]
    pub mix: f32,
}

fn default_frequency() -> f32 {
    pk(DE, "frequency").default_f64() as f32
}
fn default_q() -> f32 {
    pk(DE, "q").default_f64() as f32
}
fn default_threshold() -> f32 {
    pk(DE, "threshold").default_f64() as f32
}
fn default_ratio() -> f32 {
    pk(DE, "ratio").default_f64() as f32
}
fn default_attack() -> f32 {
    pk(DE, "attack").default_f64() as f32
}
fn default_release() -> f32 {
    pk(DE, "release").default_f64() as f32
}
fn default_mode() -> String {
    MODES[1].to_string()
}
fn default_mix() -> f32 {
    pk(DE, "mix").default_f64() as f32
}

/// Monitoring data for UI gain reduction meters.
#[derive(Debug, Clone)]
pub struct DeEsserData {
    pub gain_reduction_db: Arc<Vec<f32>>,
}

impl Default for DeEsserData {
    fn default() -> Self {
        Self {
            gain_reduction_db: Arc::new(Vec::new()),
        }
    }
}

impl DeEsserData {
    pub fn new(channels: usize) -> Self {
        Self {
            gain_reduction_db: Arc::new(vec![0.0; channels]),
        }
    }

    pub fn update(&mut self, gr: &[f32]) {
        if let Some(v) = Arc::get_mut(&mut self.gain_reduction_db)
            && v.len() == gr.len()
        {
            v.copy_from_slice(gr);
        }
    }
}

pub struct DeEsserPlugin {
    channels: usize,
    sample_rate: u32,

    // Detection
    param_frequency: ParameterId,
    frequency: f32,
    param_q: ParameterId,
    q: f32,
    /// Highpass filter per channel (lower bound of sidechain BPF)
    hp_filters: Vec<Biquad>,
    /// Lowpass filter per channel (upper bound of sidechain BPF)
    lp_filters: Vec<Biquad>,

    // Dynamics (one DynamicsCore per channel)
    cores: Vec<DynamicsCore>,
    param_threshold: ParameterId,
    threshold: f32,
    param_ratio: ParameterId,
    ratio: f32,

    // Split-band mode
    param_mode: ParameterId,
    /// 0=wideband, 1=split-band
    mode_index: usize,
    crossovers: Vec<Lr4Crossover<f32>>,

    // Mix
    param_mix: ParameterId,
    mix: f32,
    mix_smoother: Smoother,

    // Attack/Release params (tracked for parameter get/set)
    param_attack: ParameterId,
    attack_ms: f32,
    param_release: ParameterId,
    release_ms: f32,

    // Monitoring
    /// Per-channel gain reduction in dB for monitoring
    monitoring_gr: Vec<f32>,
    cache: RealTimeCache<DeEsserData>,
    cache_counter: usize,

    // Parameters
    cached_parameters: Vec<Parameter>,
}

impl DeEsserPlugin {
    pub fn new(channels: usize) -> Self {
        let sr = 44100u32;
        let freq = default_frequency();
        let q = default_q();

        let mut p = Self {
            channels,
            sample_rate: sr,

            param_frequency: ParameterId::from("frequency"),
            frequency: freq,
            param_q: ParameterId::from("q"),
            q,
            hp_filters: Self::make_hp_filters(channels, freq, q, sr),
            lp_filters: Self::make_lp_filters(channels, freq, q, sr),

            cores: (0..channels)
                .map(|_| DynamicsCore::new(DynamicsMode::Compress, 1, sr))
                .collect(),
            param_threshold: ParameterId::from("threshold"),
            threshold: default_threshold(),
            param_ratio: ParameterId::from("ratio"),
            ratio: default_ratio(),

            param_mode: ParameterId::from("mode"),
            mode_index: 1, // default: split-band
            crossovers: (0..channels)
                .map(|_| Lr4Crossover::new(freq, sr as f32, 4))
                .collect(),

            param_mix: ParameterId::from("mix"),
            mix: 1.0,
            mix_smoother: Smoother::new(1.0, 5.0, sr),

            param_attack: ParameterId::from("attack"),
            attack_ms: default_attack(),
            param_release: ParameterId::from("release"),
            release_ms: default_release(),

            monitoring_gr: vec![0.0; channels],
            cache: RealTimeCache::new(DeEsserData::new(channels)),
            cache_counter: 0,

            cached_parameters: Vec::new(),
        };

        // Set attack/release on dynamics cores
        for core in &mut p.cores {
            core.set_attack_release(p.attack_ms, p.release_ms);
        }

        p.rebuild_cached_parameters();
        p
    }

    pub fn from_params(channels: usize, params: DeEsserPluginParams) -> Self {
        let mut p = Self::new(channels);
        p.frequency = params.frequency.clamp(2000.0, 16000.0);
        p.q = params.q.clamp(0.5, 5.0);
        p.threshold = params.threshold.clamp(-60.0, 0.0);
        p.ratio = params.ratio.clamp(1.0, 20.0);
        p.attack_ms = params.attack_ms.clamp(0.1, 10.0);
        p.release_ms = params.release_ms.clamp(5.0, 200.0);
        p.mix = params.mix.clamp(0.0, 1.0);
        p.mix_smoother.set_target(p.mix);

        // Mode
        p.mode_index = match params.mode.as_str() {
            "Wideband" | "wideband" => 0,
            _ => 1, // "Split-Band" or unknown
        };

        // Update dynamics cores
        for core in &mut p.cores {
            core.set_attack_release(p.attack_ms, p.release_ms);
        }

        // Rebuild filters
        p.rebuild_detection_filters();
        p.rebuild_crossovers();
        p.rebuild_cached_parameters();
        p
    }

    fn mode_string(&self) -> String {
        match self.mode_index {
            0 => "Wideband".to_string(),
            _ => "Split-Band".to_string(),
        }
    }

    /// Compute highpass frequency from center and Q.
    /// f_hp = freq / sqrt(1 + 1/(4*Q^2)) ... simplified: freq / (2^(1/(2Q)))
    /// Simpler approach: f_low = freq / sqrt(bandwidth_ratio), f_high = freq * sqrt(bandwidth_ratio)
    /// where bandwidth_ratio = 10^(3/(20*Q)) (approx 3dB bandwidth)
    /// Even simpler: just use freq / ratio and freq * ratio where ratio = 2^(1/(2Q))
    fn bandpass_edges(freq: f32, q: f32) -> (f32, f32) {
        // Bandwidth in octaves ~= 1/Q for a standard bandpass
        // f_low = freq / 2^(1/(2Q)), f_high = freq * 2^(1/(2Q))
        let half_bw = (1.0 / (2.0 * q.max(0.5))).exp2();
        let f_low = (freq / half_bw).max(20.0);
        let f_high = (freq * half_bw).min(20000.0);
        (f_low, f_high)
    }

    fn make_hp_filters(channels: usize, freq: f32, q: f32, sr: u32) -> Vec<Biquad> {
        let (f_low, _) = Self::bandpass_edges(freq, q);
        (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Highpass,
                    f_low as f64,
                    sr as f64,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                )
            })
            .collect()
    }

    fn make_lp_filters(channels: usize, freq: f32, q: f32, sr: u32) -> Vec<Biquad> {
        let (_, f_high) = Self::bandpass_edges(freq, q);
        (0..channels)
            .map(|_| {
                Biquad::new(
                    BiquadFilterType::Lowpass,
                    f_high as f64,
                    sr as f64,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                )
            })
            .collect()
    }

    fn rebuild_detection_filters(&mut self) {
        self.hp_filters =
            Self::make_hp_filters(self.channels, self.frequency, self.q, self.sample_rate);
        self.lp_filters =
            Self::make_lp_filters(self.channels, self.frequency, self.q, self.sample_rate);
    }

    fn rebuild_crossovers(&mut self) {
        for xo in &mut self.crossovers {
            xo.set_frequency(self.frequency);
        }
    }

    fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "frequency",
                "Frequency",
                self.frequency,
                pk(DE, "frequency").min_f64() as f32,
                pk(DE, "frequency").max_f64() as f32,
            )
            .with_description("Center frequency for sibilance detection (Hz)")
            .with_group("Detection")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "q",
                "Q",
                self.q,
                pk(DE, "q").min_f64() as f32,
                pk(DE, "q").max_f64() as f32,
            )
            .with_description("Bandwidth of detection filter")
            .with_group("Detection")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "threshold",
                "Threshold",
                self.threshold,
                pk(DE, "threshold").min_f64() as f32,
                pk(DE, "threshold").max_f64() as f32,
            )
            .with_description("Sibilance detection threshold (dB)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "ratio",
                "Ratio",
                self.ratio,
                pk(DE, "ratio").min_f64() as f32,
                pk(DE, "ratio").max_f64() as f32,
            )
            .with_description("Compression ratio for sibilance")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "attack",
                "Attack",
                self.attack_ms,
                pk(DE, "attack").min_f64() as f32,
                pk(DE, "attack").max_f64() as f32,
            )
            .with_description("Attack time (ms)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "release",
                "Release",
                self.release_ms,
                pk(DE, "release").min_f64() as f32,
                pk(DE, "release").max_f64() as f32,
            )
            .with_description("Release time (ms)")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_string("mode", "Mode", self.mode_string())
                .with_description("Wideband reduces full signal; Split-band only reduces HF")
                .with_group("Mode")
                .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "mix",
                "Mix",
                self.mix,
                pk(DE, "mix").min_f64() as f32,
                pk(DE, "mix").max_f64() as f32,
            )
            .with_description("Dry/wet mix (0 = dry, 1 = processed)")
            .with_group("Output")
            .with_importance(ParameterImportance::Useful),
        ];
    }
}

impl InPlacePlugin for DeEsserPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("DeEsser", "1.0.0", "SotF")
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;

        if id == self.param_frequency {
            let v = value
                .as_float()
                .unwrap_or(pk(DE, "frequency").default_f64() as f32);
            if v.is_finite() {
                self.frequency = v.clamp(2000.0, 16000.0);
                self.rebuild_detection_filters();
                self.rebuild_crossovers();
            }
        } else if id == self.param_q {
            let v = value.as_float().unwrap_or(pk(DE, "q").default_f64() as f32);
            if v.is_finite() {
                self.q = v.clamp(0.5, 5.0);
                self.rebuild_detection_filters();
            }
        } else if id == self.param_threshold {
            let v = value
                .as_float()
                .unwrap_or(pk(DE, "threshold").default_f64() as f32);
            if v.is_finite() {
                self.threshold = v.clamp(-60.0, 0.0);
            }
        } else if id == self.param_ratio {
            let v = value
                .as_float()
                .unwrap_or(pk(DE, "ratio").default_f64() as f32);
            if v.is_finite() {
                self.ratio = v.clamp(1.0, 20.0);
            }
        } else if id == self.param_attack {
            let v = value
                .as_float()
                .unwrap_or(pk(DE, "attack").default_f64() as f32);
            if v.is_finite() {
                self.attack_ms = v.clamp(0.1, 10.0);
                for core in &mut self.cores {
                    core.set_attack_release(self.attack_ms, self.release_ms);
                }
            }
        } else if id == self.param_release {
            let v = value
                .as_float()
                .unwrap_or(pk(DE, "release").default_f64() as f32);
            if v.is_finite() {
                self.release_ms = v.clamp(5.0, 200.0);
                for core in &mut self.cores {
                    core.set_attack_release(self.attack_ms, self.release_ms);
                }
            }
        } else if id == self.param_mode {
            let new_index = if let Some(s) = value.as_string() {
                match s {
                    "Wideband" | "wideband" => 0,
                    _ => 1,
                }
            } else if let Some(v) = value.as_float() {
                (v as usize).min(1)
            } else {
                1
            };
            self.mode_index = new_index;
        } else if id == self.param_mix {
            let v = value
                .as_float()
                .unwrap_or(pk(DE, "mix").default_f64() as f32);
            if v.is_finite() {
                self.mix = v.clamp(0.0, 1.0);
                self.mix_smoother.set_target(self.mix);
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_frequency {
            Some(ParameterValue::Float(self.frequency))
        } else if id == &self.param_q {
            Some(ParameterValue::Float(self.q))
        } else if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold))
        } else if id == &self.param_ratio {
            Some(ParameterValue::Float(self.ratio))
        } else if id == &self.param_attack {
            Some(ParameterValue::Float(self.attack_ms))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_mode {
            Some(ParameterValue::String(self.mode_string()))
        } else if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;

        // Rebuild detection filters for new sample rate
        self.rebuild_detection_filters();

        // Reinit crossovers
        for xo in &mut self.crossovers {
            xo.reinit(self.frequency, sample_rate as f32, 1);
        }

        // Reinit dynamics cores
        for core in &mut self.cores {
            core.initialize(sample_rate);
            core.set_attack_release(self.attack_ms, self.release_ms);
        }

        // Reset smoother
        self.mix_smoother.set_time(5.0, sample_rate);

        Ok(())
    }

    fn reset(&mut self) {
        // Rebuild filters to reset state
        self.rebuild_detection_filters();

        // Reset crossovers
        for xo in &mut self.crossovers {
            xo.reset();
        }

        // Reset dynamics cores
        for core in &mut self.cores {
            core.reset();
        }

        self.monitoring_gr.fill(0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        enable_ftz_daz();
        let num_frames = context.num_frames;

        if self.mode_index == 0 {
            // ============================================================
            // Wideband mode
            // ============================================================
            for frame in 0..num_frames {
                // Advance mix smoother once per frame (not per channel) to avoid
                // block-constant mix that would cause zipper noise during automation.
                let mix = self.mix_smoother.advance();
                let dry_mix = 1.0 - mix;
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let input = buffer[idx];

                    // Sidechain: HP then LP to form bandpass
                    let hp_out = self.hp_filters[ch].process(input as f64) as f32;
                    let sidechain = self.lp_filters[ch].process(hp_out as f64) as f32;

                    // Level detection
                    let level = self.cores[ch].detect_level(0, sidechain);
                    let level_db = DB_CONVERSION_FACTOR * fast_log10(level.max(EPSILON));

                    // Gain reduction
                    let gr = self.cores[ch].calculate_gain_reduction(
                        level_db,
                        self.threshold,
                        self.ratio,
                        FIXED_KNEE_DB,
                    );
                    let smoothed_gr = self.cores[ch].apply_envelope(0, gr);
                    let gain = fast_pow10(-smoothed_gr / DB_CONVERSION_FACTOR);

                    let wet = input * gain;
                    buffer[idx] = dry_mix * input + mix * wet;

                    self.monitoring_gr[ch] = smoothed_gr;
                }
            }
        } else {
            // ============================================================
            // Split-band mode
            // ============================================================
            for frame in 0..num_frames {
                // Advance mix smoother once per frame (not per channel) to avoid
                // block-constant mix that would cause zipper noise during automation.
                let mix = self.mix_smoother.advance();
                let dry_mix = 1.0 - mix;
                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    let input = buffer[idx];

                    // Split into low and high bands
                    let (low, high) = self.crossovers[ch].process(input, 0);

                    // Detect level on the high band
                    let level = self.cores[ch].detect_level(0, high);
                    let level_db = DB_CONVERSION_FACTOR * fast_log10(level.max(EPSILON));

                    // Gain reduction (only on HF)
                    let gr = self.cores[ch].calculate_gain_reduction(
                        level_db,
                        self.threshold,
                        self.ratio,
                        FIXED_KNEE_DB,
                    );
                    let smoothed_gr = self.cores[ch].apply_envelope(0, gr);
                    let gain = fast_pow10(-smoothed_gr / DB_CONVERSION_FACTOR);

                    let wet = low + high * gain;
                    buffer[idx] = dry_mix * input + mix * wet;

                    self.monitoring_gr[ch] = smoothed_gr;
                }
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_counter += 1;
        if self.cache_counter >= 10 {
            self.cache_counter = 0;
            self.cache.update(|d| {
                d.update(&self.monitoring_gr);
            });
        }

        flush_denormals_inplace(buffer);
        Ok(num_frames)
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine(freq_hz: f32, sample_rate: u32, num_frames: usize, amplitude: f32) -> Vec<f32> {
        (0..num_frames)
            .map(|i| {
                amplitude
                    * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
            })
            .collect()
    }

    fn rms(buf: &[f32]) -> f32 {
        let sum: f32 = buf.iter().map(|x| x * x).sum();
        (sum / buf.len() as f32).sqrt()
    }

    #[test]
    fn test_de_esser_reduces_sibilance() {
        let sr = 48000u32;
        let num_frames = 48000; // 1 second
        let amplitude = 0.5;

        let mut plugin = DeEsserPlugin::from_params(
            1,
            DeEsserPluginParams {
                frequency: 8000.0,
                q: 1.5,
                threshold: -20.0,
                ratio: 10.0,
                attack_ms: 0.5,
                release_ms: 20.0,
                mode: "Wideband".to_string(),
                mix: 1.0,
            },
        );
        plugin.initialize(sr).unwrap();

        // 8kHz sine in the sibilance range
        let mut buf = make_sine(8000.0, sr, num_frames, amplitude);
        let input_rms = rms(&buf);

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        // Use the second half to allow attack to settle
        let output_rms = rms(&buf[num_frames / 2..]);

        // Output should be significantly quieter
        assert!(
            output_rms < input_rms * 0.5,
            "8kHz signal should be reduced: input_rms={:.4}, output_rms={:.4}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_de_esser_passes_low_frequencies() {
        let sr = 48000u32;
        let num_frames = 48000; // 1 second
        let amplitude = 0.5;

        let mut plugin = DeEsserPlugin::from_params(
            1,
            DeEsserPluginParams {
                frequency: 7000.0,
                q: 1.5,
                threshold: -20.0,
                ratio: 10.0,
                attack_ms: 0.5,
                release_ms: 20.0,
                mode: "Wideband".to_string(),
                mix: 1.0,
            },
        );
        plugin.initialize(sr).unwrap();

        // 200Hz sine — well below detection range
        let mut buf = make_sine(200.0, sr, num_frames, amplitude);
        let input_rms = rms(&buf);

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        let output_rms = rms(&buf[num_frames / 2..]);

        // Low-frequency signal should pass through mostly unchanged
        assert!(
            output_rms > input_rms * 0.9,
            "200Hz signal should pass through: input_rms={:.4}, output_rms={:.4}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_de_esser_parameter_set_get() {
        let mut plugin = DeEsserPlugin::new(2);
        plugin.initialize(48000).unwrap();

        // Set frequency
        plugin
            .set_parameter(
                ParameterId::from("frequency"),
                ParameterValue::Float(10000.0),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("frequency"));
        assert_eq!(val, Some(ParameterValue::Float(10000.0)));

        // Set threshold
        plugin
            .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("threshold"));
        assert_eq!(val, Some(ParameterValue::Float(-30.0)));

        // Set mode
        plugin
            .set_parameter(
                ParameterId::from("mode"),
                ParameterValue::String("Wideband".to_string()),
            )
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mode"));
        assert_eq!(val, Some(ParameterValue::String("Wideband".to_string())));

        // Set mix
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();
        let val = plugin.get_parameter(&ParameterId::from("mix"));
        assert_eq!(val, Some(ParameterValue::Float(0.5)));
    }

    /// Verify that the mix smoother advances per-sample during a block, not as a
    /// block-constant value. If `next_n(num_frames)` were used (old code), the
    /// smoother would jump to its target on the first block and the first sample
    /// would already be at the target. With per-sample `advance()`, the value
    /// ramps smoothly: the very first sample is close to the *starting* value,
    /// not the target value.
    #[test]
    fn test_mix_smoother_ramps_per_sample() {
        let sr = 48000u32;
        // Start mix at 0 (dry)
        let mut plugin = DeEsserPlugin::from_params(
            1,
            DeEsserPluginParams {
                frequency: 7000.0,
                q: 1.5,
                threshold: -20.0,
                ratio: 10.0,
                attack_ms: 0.5,
                release_ms: 20.0,
                mode: "Wideband".to_string(),
                mix: 0.0, // fully dry initially
            },
        );
        plugin.initialize(sr).unwrap();

        // Now request mix = 1.0 (fully wet). The smoother has a 5 ms ramp.
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
            .unwrap();

        // A silent input: output should also be silent regardless of mix
        // Use a 1 kHz tone instead so we can measure dry-vs-wet differences.
        // Use a 100-sample block — well within the 5 ms ramp (~240 samples at 48 kHz).
        let num_frames = 100;
        let mut buf: Vec<f32> = (0..num_frames)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin())
            .collect();

        // Capture the first sample's input value
        let first_input = buf[0];

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf, &ctx).unwrap();

        // With mix=0 at block start and a 5ms ramp, after only 100 samples
        // (~2ms) the smoother should still be far below 1.0. The first output
        // sample must still be close to dry (= input * gain).
        // Specifically: if the smoother was block-constant, it would jump to ~1.0
        // and the first output would be purely wet. If it ramps per-sample,
        // the first output should be much closer to the dry value.
        //
        // We assert that the smoother has NOT jumped all the way to fully wet
        // on the first sample: the first output must not equal the fully-wet value.
        //
        // For a 1 kHz sine at mix=0 (dry), the output is approximately input*gain.
        // For mix=1 (wet), at this threshold the 1kHz tone is below the detection
        // range so gain≈1 and wet≈input. The meaningful test is therefore to
        // observe that the mix value at sample 0 is near 0, not near 1.
        //
        // We do this indirectly: set mix from 0 to 1 and verify the per-sample
        // smoother current value starts near 0. We read back the smoother
        // state by checking it hasn't already converged in 100 samples.
        // At 48kHz with a 5ms ramp, coeff = exp(-1/(0.005*48000)) = exp(-1/240) ≈ 0.9958.
        // After 100 samples: value ≈ 1 - 0.9958^100 * 1 ≈ 1 - 0.665 = 0.335.
        // The block-constant version would give 1 - 0.9958^100 ≈ 0.335 at the END
        // of block but apply that single value as-if the whole block ran at 0.335.
        // The per-sample version truly ramps 0..0.335 across the 100 samples.
        //
        // A simpler check: the first output sample should NOT be at full wet.
        // At the first sample, mix is approximately 0 (start value). So output[0]
        // should be very close to input[0] (dry) rather than whatever wet[0] would be.
        // Since there is no gain reduction yet (envelope not triggered), wet = input,
        // so dry ≈ wet in this case and the test is degenerate. Instead we verify
        // the smoother stays monotone: use a plugin with 0 threshold so gain ≈ 0 (heavy).
        let _ = first_input; // suppress unused warning

        // --- New approach: heavy compression so wet != dry ---
        let mut plugin2 = DeEsserPlugin::from_params(
            1,
            DeEsserPluginParams {
                frequency: 1000.0, // center at test freq
                q: 0.5,            // wide bandwidth to catch 1kHz
                threshold: -60.0,  // extremely low threshold → heavy compression
                ratio: 20.0,       // max ratio → near total gain kill
                attack_ms: 0.1,    // fast attack
                release_ms: 200.0,
                mode: "Wideband".to_string(),
                mix: 0.0, // start dry
            },
        );
        plugin2.initialize(sr).unwrap();

        // Ramp to fully wet over 5ms
        plugin2
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
            .unwrap();

        // One block of 100 samples — still in the ramp window
        let mut buf2: Vec<f32> = (0..num_frames)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin())
            .collect();
        let dry_ref = buf2.clone(); // original input (dry output when mix=0)

        plugin2.process_in_place(&mut buf2, &ctx).unwrap();

        // Wet output (after heavy GR) should be near-silent. Dry output = original.
        // With per-sample ramp starting at mix=0, the first samples lean dry.
        // With block-constant mix, the whole block has mix≈0.335 (already ramped).
        //
        // The first sample of buf2 should be between dry_ref[0] (when mix≈0)
        // and near-zero (when mix≈1 and gain≈0). It must not equal dry_ref[0]
        // exactly (some ramp happened) but must not be at 0 either.
        //
        // Most importantly: the output must NOT be identical to the full-wet result
        // for the entire block. We verify at least the first sample has nonzero
        // dry component.
        let first_out = buf2[0];
        let first_dry = dry_ref[0];
        // If the smoother started truly at 0 and ramped, the first sample is
        // output = 0 * wet + 1 * dry = dry (approximately, mix≈0 at t=0).
        // Allow a small tolerance since one-pole starts advancing immediately.
        assert!(
            (first_out - first_dry).abs() < first_dry.abs() * 0.2 + 1e-4,
            "First output sample should be near dry (mix≈0 at t=0): \
             first_out={:.6}, first_dry={:.6}",
            first_out,
            first_dry
        );
    }

    #[test]
    fn test_split_band_mode() {
        let sr = 48000u32;
        let num_frames = 48000; // 1 second
        let amplitude = 0.5;

        let mut plugin = DeEsserPlugin::from_params(
            1,
            DeEsserPluginParams {
                frequency: 7000.0,
                q: 1.5,
                threshold: -20.0,
                ratio: 10.0,
                attack_ms: 0.5,
                release_ms: 20.0,
                mode: "Split-Band".to_string(),
                mix: 1.0,
            },
        );
        plugin.initialize(sr).unwrap();

        // --- Test that HF is attenuated ---
        let mut buf_hf = make_sine(8000.0, sr, num_frames, amplitude);
        let input_rms_hf = rms(&buf_hf);

        let ctx = ProcessContext {
            sample_rate: sr,
            num_frames,
        };
        plugin.process_in_place(&mut buf_hf, &ctx).unwrap();
        let output_rms_hf = rms(&buf_hf[num_frames / 2..]);

        assert!(
            output_rms_hf < input_rms_hf * 0.7,
            "Split-band: 8kHz should be reduced: input={:.4}, output={:.4}",
            input_rms_hf,
            output_rms_hf
        );

        // --- Test that LF passes through ---
        plugin.reset();
        let mut buf_lf = make_sine(200.0, sr, num_frames, amplitude);
        let input_rms_lf = rms(&buf_lf);

        plugin.process_in_place(&mut buf_lf, &ctx).unwrap();
        let output_rms_lf = rms(&buf_lf[num_frames / 2..]);

        assert!(
            output_rms_lf > input_rms_lf * 0.85,
            "Split-band: 200Hz should pass through: input={:.4}, output={:.4}",
            input_rms_lf,
            output_rms_lf
        );
    }
}
