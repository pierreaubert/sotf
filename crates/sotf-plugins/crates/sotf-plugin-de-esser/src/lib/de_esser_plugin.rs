use super::consts::DB_CONVERSION_FACTOR;
use super::consts::EPSILON;
use super::consts::FIXED_KNEE_DB;
use super::de_esser_data::DeEsserData;
use super::types::DeEsserPluginParams;
use crate::params::{
    default_attack_ms, default_frequency, default_mix, default_q, default_ratio,
    default_release_ms, default_threshold, PARAMS as DE,
};
use math_audio_dsp::fast_math::{fast_log10, fast_pow10};
use math_audio_iir_fir::{Biquad, BiquadBank, BiquadFilterType};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::dynamics_core::DynamicsCore;
use sotf_host::dynamics_core::DynamicsMode;
use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::{ParameterSchema, ParameterSet};
use sotf_host::plugin::{
    PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{apply_per_channel_gain_simd, enable_ftz_daz, flush_denormals_inplace};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

pub struct DeEsserPlugin {
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Detection
    pub(super) param_frequency: ParameterId,
    pub(super) frequency: f32,
    pub(super) param_q: ParameterId,
    pub(super) q: f32,
    /// Highpass filter bank per channel (lower bound of sidechain BPF)
    pub(super) hp_filters: BiquadBank<f32>,
    /// Lowpass filter bank per channel (upper bound of sidechain BPF)
    pub(super) lp_filters: BiquadBank<f32>,

    /// Reusable per-frame sidechain scratch buffer.
    pub(super) sidechain_frame: Vec<f32>,

    /// Reusable per-frame gain multipliers for SIMD path.
    pub(super) frame_gains: Vec<f32>,

    // Dynamics (one DynamicsCore per channel)
    pub(super) cores: Vec<DynamicsCore>,
    pub(super) param_threshold: ParameterId,
    pub(super) threshold: f32,
    pub(super) param_ratio: ParameterId,
    pub(super) ratio: f32,

    // Split-band mode
    pub(super) param_mode: ParameterId,
    /// 0=wideband, 1=split-band
    pub(super) mode_index: usize,
    pub(super) crossovers: Vec<Lr4Crossover<f32>>,

    // Mix
    pub(super) param_mix: ParameterId,
    pub(super) mix: f32,
    pub(super) mix_smoother: Smoother,

    // Attack/Release params (tracked for parameter get/set)
    pub(super) param_attack: ParameterId,
    pub(super) attack_ms: f32,
    pub(super) param_release: ParameterId,
    pub(super) release_ms: f32,

    // Monitoring
    /// Per-channel gain reduction in dB for monitoring
    pub(super) monitoring_gr: Vec<f32>,
    pub(super) cache: RealTimeCache<DeEsserData>,
    pub(super) cache_counter: usize,

    // Parameters
    pub(super) cached_parameters: Vec<Parameter>,
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
            sidechain_frame: vec![0.0; channels],
            frame_gains: vec![0.0; channels],

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
                .map(|_| Lr4Crossover::new(freq, sr as f32, 1))
                .collect(),

            param_mix: ParameterId::from("mix"),
            mix: default_mix(),
            mix_smoother: Smoother::new(1.0, 5.0, sr),

            param_attack: ParameterId::from("attack"),
            attack_ms: default_attack_ms(),
            param_release: ParameterId::from("release"),
            release_ms: default_release_ms(),

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

    pub(super) fn mode_string(&self) -> String {
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
    pub(super) fn bandpass_edges(freq: f32, q: f32) -> (f32, f32) {
        // Bandwidth in octaves ~= 1/Q for a standard bandpass
        // f_low = freq / 2^(1/(2Q)), f_high = freq * 2^(1/(2Q))
        let half_bw = (1.0 / (2.0 * q.max(0.5))).exp2();
        let f_low = (freq / half_bw).max(20.0);
        let f_high = (freq * half_bw).min(20000.0);
        (f_low, f_high)
    }

    pub(super) fn make_hp_filters(channels: usize, freq: f32, q: f32, sr: u32) -> BiquadBank<f32> {
        let (f_low, _) = Self::bandpass_edges(freq, q);
        let template = Biquad::new(BiquadFilterType::Highpass, f_low, sr as f32, q, 0.0);
        BiquadBank::new(&template, channels)
    }

    pub(super) fn make_lp_filters(channels: usize, freq: f32, q: f32, sr: u32) -> BiquadBank<f32> {
        let (_, f_high) = Self::bandpass_edges(freq, q);
        let template = Biquad::new(BiquadFilterType::Lowpass, f_high, sr as f32, q, 0.0);
        BiquadBank::new(&template, channels)
    }

    pub(super) fn rebuild_detection_filters(&mut self) {
        let (f_low, f_high) = Self::bandpass_edges(self.frequency, self.q);
        self.hp_filters
            .update_params(f_low, self.sample_rate as f32, self.q, 0.0);
        self.lp_filters
            .update_params(f_high, self.sample_rate as f32, self.q, 0.0);
    }

    pub(super) fn rebuild_crossovers(&mut self) {
        for xo in &mut self.crossovers {
            xo.set_frequency(self.frequency);
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
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

impl ParametricInPlacePlugin for DeEsserPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("DeEsser", "1.0.0", "SotF")
    }

    fn cost_class(&self) -> PluginCostClass {
        PluginCostClass::Dynamics
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(PluginCostClass::Dynamics, None, 0, false)
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
            self.param_frequency.clone(),
            ParameterValue::Float(self.frequency),
        );
        values.insert(self.param_q.clone(), ParameterValue::Float(self.q));
        values.insert(
            self.param_threshold.clone(),
            ParameterValue::Float(self.threshold),
        );
        values.insert(self.param_ratio.clone(), ParameterValue::Float(self.ratio));
        values.insert(
            self.param_attack.clone(),
            ParameterValue::Float(self.attack_ms),
        );
        values.insert(
            self.param_release.clone(),
            ParameterValue::Float(self.release_ms),
        );
        values.insert(
            self.param_mode.clone(),
            ParameterValue::String(self.mode_string()),
        );
        values.insert(self.param_mix.clone(), ParameterValue::Float(self.mix));
        values
    }

    fn apply_values(&mut self, values: ParameterSet) -> PluginResult<()> {
        for (id, value) in values {
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
            } else {
                return Err(format!("Unknown parameter: {id}"));
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
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
        if self.channels == 0 {
            return Ok(num_frames);
        }

        if self.mode_index == 0 {
            // ============================================================
            // Wideband mode
            // ============================================================
            for frame in 0..num_frames {
                let frame_offset = frame * self.channels;
                // Process sidechain in a scratch frame to keep the main buffer as output.
                let frame_samples = &mut self.sidechain_frame[..self.channels];
                frame_samples.copy_from_slice(&buffer[frame_offset..frame_offset + self.channels]);
                self.hp_filters.process_interleaved_frame(frame_samples);
                self.lp_filters.process_interleaved_frame(frame_samples);

                // Advance mix smoother once per frame (not per channel) to avoid
                // block-constant mix that would cause zipper noise during automation.
                let mix = self.mix_smoother.advance();
                let dry_mix = 1.0 - mix;
                for (ch, &sidechain) in frame_samples.iter().enumerate().take(self.channels) {
                    // Sidechain: HP then LP to form bandpass
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
                    self.frame_gains[ch] = dry_mix + mix * gain;

                    if frame + 1 == num_frames {
                        self.monitoring_gr[ch] = smoothed_gr;
                    }
                }
                apply_per_channel_gain_simd(
                    &mut buffer[frame_offset..frame_offset + self.channels],
                    self.channels,
                    &self.frame_gains,
                );
            }
        } else {
            // ============================================================
            // Split-band mode
            // ============================================================
            for frame in 0..num_frames {
                let frame_offset = frame * self.channels;
                // Advance mix smoother once per frame (not per channel) to avoid
                // block-constant mix that would cause zipper noise during automation.
                let mix = self.mix_smoother.advance();
                let dry_mix = 1.0 - mix;
                for ch in 0..self.channels {
                    let idx = frame_offset + ch;
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

                    if frame + 1 == num_frames {
                        self.monitoring_gr[ch] = smoothed_gr;
                    }
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
