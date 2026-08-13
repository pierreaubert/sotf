use super::analysis::PndAnalyzer;
pub use super::config::PndPluginParams;
use super::consts::CORRECTION_STRENGTH_SMOOTH_MS;
use super::consts::PV_FFT_SIZE;
use super::consts::PV_HOP_SIZE;
use super::consts::PV_LATENCY_FRAMES;
use super::phase_vocoder::PhaseVocoder;
use super::types::PndData;
use crate::params::PARAMS as PD;
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_bridge::apply_spec_update_modes;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use std::any::Any;
use std::sync::Arc;

pub struct PndPlugin {
    // Configuration
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Components — one analyzer per channel for multi-channel analysis
    pub(super) analyzers: Vec<PndAnalyzer>,
    // State
    pub(super) current_ratio: f64,
    pub(super) last_drift_ratio: f64,
    pub(super) last_analysis_generation: u64,
    pub(super) reference_transition_pending: bool,

    // Scratch buffer for confidence-weighted channel consensus
    pub(super) channel_consensus_scratch: Vec<(f32, f32)>,

    // Parameters
    pub(super) param_correction_strength: ParameterId,
    pub(super) correction_strength: f32,
    pub(super) correction_strength_current: f32,
    pub(super) correction_strength_alpha: f32,

    pub(super) param_analysis_window_ms: ParameterId,
    pub(super) analysis_window_ms: f32,

    pub(super) param_drift_smoothing: ParameterId,
    pub(super) drift_smoothing: f32,

    pub(super) param_multi_channel_analysis: ParameterId,
    pub(super) multi_channel_analysis: bool,

    pub(super) param_confidence_threshold: ParameterId,
    pub(super) confidence_threshold: f32,

    pub(super) param_reference_frequency_hz: ParameterId,
    pub(super) reference_frequency_hz: f32,

    pub(super) vocoder: Option<PhaseVocoder>,

    pub(super) cache: RealTimeCache<PndData>,
    pub(super) cache_update_counter: usize,
    pub(super) cached_parameters: Vec<Parameter>,
    pub(super) initialized: bool,
}

impl PndPlugin {
    pub fn new(channels: usize) -> Self {
        let mut p = Self {
            channels,
            sample_rate: 44100, // Default, updated in initialize
            analyzers: Vec::new(),
            current_ratio: 1.0,
            last_drift_ratio: 1.0,
            last_analysis_generation: 0,
            reference_transition_pending: false,
            channel_consensus_scratch: vec![(1.0, 0.0); channels],

            param_correction_strength: ParameterId::from("correction_strength"),
            correction_strength: pk(PD, "correction_strength").default_f64() as f32,
            correction_strength_current: pk(PD, "correction_strength").default_f64() as f32,
            correction_strength_alpha: 0.0,

            param_analysis_window_ms: ParameterId::from("analysis_window_ms"),
            analysis_window_ms: pk(PD, "analysis_window_ms").default_f64() as f32,

            param_drift_smoothing: ParameterId::from("drift_smoothing"),
            drift_smoothing: pk(PD, "drift_smoothing").default_f64() as f32,

            param_multi_channel_analysis: ParameterId::from("multi_channel_analysis"),
            multi_channel_analysis: pk(PD, "multi_channel_analysis").default_bool(),

            param_confidence_threshold: ParameterId::from("confidence_threshold"),
            confidence_threshold: pk(PD, "confidence_threshold").default_f64() as f32,

            param_reference_frequency_hz: ParameterId::from("reference_frequency_hz"),
            reference_frequency_hz: pk(PD, "reference_frequency_hz").default_f64() as f32,

            vocoder: None,

            cache: RealTimeCache::new_triplet(
                PndData::default(),
                PndData::default(),
                PndData::default(),
            ),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
            initialized: false,
        };
        p.rebuild_cached_parameters();
        p
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = vec![
            Parameter::new_float(
                "correction_strength",
                "Correction Strength",
                self.correction_strength,
                pk(PD, "correction_strength").min_f64() as f32,
                pk(PD, "correction_strength").max_f64() as f32,
            )
            .with_group("Correction")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "analysis_window_ms",
                "Analysis Window (ms)",
                self.analysis_window_ms,
                pk(PD, "analysis_window_ms").min_f64() as f32,
                pk(PD, "analysis_window_ms").max_f64() as f32,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "drift_smoothing",
                "Drift Smoothing",
                self.drift_smoothing,
                pk(PD, "drift_smoothing").min_f64() as f32,
                pk(PD, "drift_smoothing").max_f64() as f32,
            )
            .with_group("Correction")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_bool(
                "multi_channel_analysis",
                "Multi-Channel Analysis",
                self.multi_channel_analysis,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "confidence_threshold",
                "Confidence Threshold",
                self.confidence_threshold,
                pk(PD, "confidence_threshold").min_f64() as f32,
                pk(PD, "confidence_threshold").max_f64() as f32,
            )
            .with_group("Correction")
            .with_importance(ParameterImportance::Useful),
            Parameter::new_float(
                "reference_frequency_hz",
                "Reference Pitch",
                self.reference_frequency_hz,
                pk(PD, "reference_frequency_hz").min_f64() as f32,
                pk(PD, "reference_frequency_hz").max_f64() as f32,
            )
            .with_group("Correction")
            .with_importance(ParameterImportance::Useful),
        ];
        apply_spec_update_modes(&mut self.cached_parameters, PD);
    }

    /// Construct a plugin from serialized parameters after applying the same
    /// finite/range checks used by the live parameter schema.
    pub fn try_from_params(channels: usize, params: PndPluginParams) -> PluginResult<Self> {
        if channels == 0 {
            return Err("PND requires at least one channel".to_string());
        }
        for (name, value) in [
            ("correction_strength", params.correction_strength),
            ("analysis_window_ms", params.analysis_window_ms),
            ("drift_smoothing", params.drift_smoothing),
            ("confidence_threshold", params.confidence_threshold),
            ("reference_frequency_hz", params.reference_frequency_hz),
        ] {
            let spec = pk(PD, name);
            if !value.is_finite()
                || f64::from(value) < spec.min_f64()
                || f64::from(value) > spec.max_f64()
            {
                return Err(format!(
                    "Invalid PND parameter {name}={value}; expected a finite value in [{}, {}]",
                    spec.min_f64(),
                    spec.max_f64()
                ));
            }
        }

        let mut plugin = Self::new(channels);
        plugin.correction_strength = params.correction_strength;
        plugin.analysis_window_ms = params.analysis_window_ms;
        plugin.drift_smoothing = params.drift_smoothing;
        plugin.multi_channel_analysis = params.multi_channel_analysis;
        plugin.confidence_threshold = params.confidence_threshold;
        plugin.reference_frequency_hz = params.reference_frequency_hz;
        // `phase_vocoder` is a legacy serialized compatibility field. Both
        // historical values now migrate explicitly to the sole fixed-frame,
        // duration-preserving correction engine.
        let _legacy_phase_vocoder = params.phase_vocoder;
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    /// Backwards-compatible infallible constructor. New factory code should
    /// use [`Self::try_from_params`] so malformed serialized state is rejected
    /// instead of reaching DSP initialization.
    pub fn from_params(channels: usize, params: PndPluginParams) -> Self {
        Self::try_from_params(channels, params).unwrap_or_else(|_| {
            Self::try_from_params(channels.max(1), PndPluginParams::default())
                .expect("default PND parameters are valid")
        })
    }

    /// Phase vocoder processing path: uses STFT analysis/synthesis to shift pitch
    /// without changing duration. No separate formant-envelope preservation is applied.
    pub(super) fn process_phase_vocoder(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        let nf = context.num_frames;

        let vocoder = self
            .vocoder
            .as_mut()
            .ok_or("Phase vocoder not initialized")?;

        if input.len() != num_frames * self.channels {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                num_frames * self.channels,
                input.len()
            ));
        }
        if output.len() != num_frames * self.channels {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                num_frames * self.channels,
                output.len()
            ));
        }
        let mut confidence = self.analyzers.first().map_or(0.0, PndAnalyzer::confidence);

        // Analysis and correction decisions advance on the same sample clock
        // as synthesis. This prevents a large callback from applying a decision
        // derived from its final samples to samples at the callback's start.
        for i in 0..num_frames {
            let mut analysis_generation = self.last_analysis_generation;
            for ch in 0..self.analyzers.len().min(self.channels) {
                let analyzer = &mut self.analyzers[ch];
                let sample = input[i * self.channels + ch];
                let previous_generation = analyzer.analysis_generation();
                let temporal_drift = analyzer.analyze(std::slice::from_ref(&sample));
                analysis_generation = analysis_generation.max(analyzer.analysis_generation());
                if analyzer.analysis_generation() > previous_generation {
                    self.channel_consensus_scratch[ch] = if self.reference_frequency_hz > 0.0 {
                        analyzer.estimate_against_reference(self.reference_frequency_hz)
                    } else {
                        (temporal_drift, analyzer.confidence())
                    };
                }
            }

            let elapsed_hops = analysis_generation.saturating_sub(self.last_analysis_generation);
            if elapsed_hops > 0 {
                self.last_analysis_generation = analysis_generation;
                let (drift_ratio, new_confidence) = if self.analyzers.is_empty() {
                    (1.0, 0.0)
                } else if self.multi_channel_analysis && self.analyzers.len() > 1 {
                    let n = self.analyzers.len().min(self.channels);
                    weighted_channel_consensus(
                        &mut self.channel_consensus_scratch[..n],
                        self.confidence_threshold,
                    )
                } else {
                    self.channel_consensus_scratch[0]
                };
                confidence = new_confidence;
                self.last_drift_ratio = f64::from(drift_ratio);

                let target = if confidence >= self.confidence_threshold {
                    self.reference_transition_pending = false;
                    Some(1.0 / f64::from(drift_ratio))
                } else if self.reference_frequency_hz > 0.0 || self.reference_transition_pending {
                    Some(1.0)
                } else {
                    None
                };
                if let Some(target) = target {
                    self.current_ratio = smooth_drift_ratio(
                        self.current_ratio,
                        target,
                        self.drift_smoothing,
                        elapsed_hops as usize * PV_HOP_SIZE,
                        self.sample_rate,
                    );
                }
            }

            self.correction_strength_current += self.correction_strength_alpha
                * (self.correction_strength - self.correction_strength_current);
            let pitch_shift = (1.0
                + (self.current_ratio - 1.0) * f64::from(self.correction_strength_current))
                as f32;
            for ch in 0..self.channels {
                let pv = &mut vocoder.channels[ch];
                pv.input_buf[pv.input_fill] = input[i * self.channels + ch];
                pv.input_fill += 1;

                // When we have a full FFT frame, process a hop
                if pv.input_fill >= PV_FFT_SIZE {
                    pv.process_hop(pitch_shift);
                }
            }

            // Drain one frame of output per channel
            for ch in 0..self.channels {
                let pv = &mut vocoder.channels[ch];
                if pv.output_fill > 0 {
                    let accum_len = pv.output_accum.len();
                    let idx = pv.output_read % accum_len;
                    output[i * self.channels + ch] = pv.output_accum[idx];
                    pv.output_accum[idx] = 0.0;
                    pv.output_read += 1;
                    pv.output_fill -= 1;
                } else {
                    output[i * self.channels + ch] = 0.0;
                }
            }
        }

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            let drift = self.last_drift_ratio;
            let correction = self.current_ratio;
            // Aggregate analyzer diagnostics across the configured channels.
            let (matched_partials, total_peaks) = if self.analyzers.is_empty() {
                (0, 0)
            } else {
                let total_matched: usize =
                    self.analyzers.iter().map(|a| a.matched_partials()).sum();
                let total_pk: usize = self.analyzers.iter().map(|a| a.total_peaks()).sum();
                (total_matched, total_pk)
            };
            self.cache.update(|d| {
                d.drift_ratio = drift;
                d.correction_ratio = correction;
                d.confidence = confidence;
                d.matched_partials = matched_partials;
                d.total_peaks = total_peaks;
            });
        }

        Ok(nf)
    }

    pub(super) fn init_analyzers(&mut self) {
        let fft_size = 2048; // Good balance for freq resolution
        let num_analyzers = if self.multi_channel_analysis {
            self.channels
        } else {
            1
        };
        self.analyzers.clear();
        for _ in 0..num_analyzers {
            self.analyzers.push(PndAnalyzer::new(
                fft_size,
                self.sample_rate,
                self.analysis_window_ms,
            ));
        }
        self.channel_consensus_scratch
            .resize(num_analyzers.max(1), (1.0, 0.0));
    }
}

/// Apply a sample-clock-derived one-pole update. `time_seconds` is the time
/// constant, so callback partitioning cannot alter convergence speed.
pub(super) fn smooth_drift_ratio(
    current: f64,
    target: f64,
    time_seconds: f32,
    elapsed_frames: usize,
    sample_rate: u32,
) -> f64 {
    if elapsed_frames == 0 || sample_rate == 0 {
        return current;
    }
    let tau = f64::from(time_seconds.max(f32::MIN_POSITIVE));
    let elapsed = elapsed_frames as f64 / sample_rate as f64;
    let alpha = 1.0 - (-elapsed / tau).exp();
    current + (target - current) * alpha
}

/// Combine channel observations without allowing silent or low-confidence
/// channels to outvote a reliable tonal channel. Observations are compacted
/// and sorted in the caller-owned scratch buffer, so this remains allocation
/// free in the processing callback.
pub(super) fn weighted_channel_consensus(
    observations: &mut [(f32, f32)],
    confidence_threshold: f32,
) -> (f32, f32) {
    let mut valid = 0;
    for index in 0..observations.len() {
        let (ratio, confidence) = observations[index];
        if ratio.is_finite()
            && ratio > 0.0
            && confidence.is_finite()
            && confidence >= confidence_threshold
        {
            observations[valid] = (ratio, confidence);
            valid += 1;
        }
    }

    if valid == 0 {
        return (1.0, 0.0);
    }

    // Channel order must not affect a shared correction decision.
    observations[..valid]
        .sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let total_weight: f32 = observations[..valid].iter().map(|(_, c)| *c).sum();
    if valid == 1 {
        return observations[0];
    }

    // A single shared clock/pitch correction must be supported by a coherent
    // channel cluster. Mutually contradictory high-confidence channels are not
    // averaged into a correction that no channel observed. A 0.4% relative
    // radius comfortably covers detector interpolation error while separating
    // musically different sources and opposite drift hypotheses.
    const CONSENSUS_RADIUS: f32 = 0.004;
    let mut best_center = observations[0].0;
    let mut best_weight = 0.0_f32;
    for &(center, _) in &observations[..valid] {
        let support = observations[..valid]
            .iter()
            .filter(|(ratio, _)| ((ratio / center) - 1.0).abs() <= CONSENSUS_RADIUS)
            .map(|(_, confidence)| *confidence)
            .sum::<f32>();
        let support_is_better = support > best_weight + f32::EPSILON;
        let support_is_tied = (support - best_weight).abs() <= f32::EPSILON;
        let center_is_safer = (center - 1.0).abs() < (best_center - 1.0).abs()
            || ((center - 1.0).abs() == (best_center - 1.0).abs() && center < best_center);
        if support_is_better || (support_is_tied && center_is_safer) {
            best_center = center;
            best_weight = support;
        }
    }

    let support_fraction = best_weight / total_weight.max(f32::MIN_POSITIVE);
    if support_fraction < 0.6 {
        return (1.0, 0.0);
    }

    let mut weighted_ratio = 0.0_f32;
    let mut members = 0usize;
    for &(ratio, confidence) in &observations[..valid] {
        if ((ratio / best_center) - 1.0).abs() <= CONSENSUS_RADIUS {
            weighted_ratio += ratio * confidence;
            members += 1;
        }
    }
    let ratio = weighted_ratio / best_weight.max(f32::MIN_POSITIVE);
    // Preserve the average confidence of the coherent observations while
    // reducing it by cluster impurity. The square-root penalty lets a clear
    // multi-channel majority remain authoritative at the default gate without
    // allowing an even split through (the split already fails above).
    let confidence = (best_weight / members.max(1) as f32) * support_fraction.sqrt();
    (ratio, confidence.clamp(0.0, 1.0))
}

impl Plugin for PndPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Pitch Drift Corrector", env!("CARGO_PKG_VERSION"), "SotF")
            .with_description("Referenced drift analysis and duration-preserving pitch correction")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        PluginCompileMetadata::nonlinear(PluginCostClass::Fft, None, self.latency_samples(), false)
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        self.validate_parameter(&id, &value)?;
        if id == self.param_correction_strength {
            let v = value
                .as_float()
                .unwrap_or(pk(PD, "correction_strength").default_f64() as f32);
            if v.is_finite() {
                self.correction_strength = v;
            }
        } else if id == self.param_analysis_window_ms {
            if self.initialized {
                return Err(
                    "analysis_window_ms is a structural setup parameter; rebuild the plugin"
                        .to_string(),
                );
            }
            let v = value
                .as_float()
                .unwrap_or(pk(PD, "analysis_window_ms").default_f64() as f32);
            if v.is_finite() {
                self.analysis_window_ms = v;
                for analyzer in &mut self.analyzers {
                    analyzer.update_analysis_window(self.analysis_window_ms);
                }
            }
        } else if id == self.param_drift_smoothing {
            let v = value
                .as_float()
                .unwrap_or(pk(PD, "drift_smoothing").default_f64() as f32);
            if v.is_finite() {
                self.drift_smoothing = v;
            }
        } else if id == self.param_multi_channel_analysis {
            if self.initialized {
                return Err(
                    "multi_channel_analysis is a structural setup parameter; rebuild the plugin"
                        .to_string(),
                );
            }
            let v = value
                .as_bool()
                .unwrap_or(pk(PD, "multi_channel_analysis").default_bool());
            self.multi_channel_analysis = v;
            // Re-create analyzers with new channel count
            self.init_analyzers();
        } else if id == self.param_confidence_threshold {
            let v = value
                .as_float()
                .unwrap_or(pk(PD, "confidence_threshold").default_f64() as f32);
            if v.is_finite() {
                self.confidence_threshold = v;
            }
        } else if id == self.param_reference_frequency_hz {
            let v = value
                .as_float()
                .unwrap_or(pk(PD, "reference_frequency_hz").default_f64() as f32);
            if self.initialized && v > 0.0 && v >= self.sample_rate as f32 * 0.5 {
                return Err(format!(
                    "reference_frequency_hz must be below Nyquist ({:.1} Hz)",
                    self.sample_rate as f32 * 0.5
                ));
            }
            if v != self.reference_frequency_hz {
                self.reference_frequency_hz = v;
                for analyzer in &mut self.analyzers {
                    analyzer.reset();
                }
                self.last_analysis_generation = 0;
                self.last_drift_ratio = 1.0;
                self.reference_transition_pending = true;
            }
        }
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_correction_strength {
            Some(ParameterValue::Float(self.correction_strength))
        } else if id == &self.param_analysis_window_ms {
            Some(ParameterValue::Float(self.analysis_window_ms))
        } else if id == &self.param_drift_smoothing {
            Some(ParameterValue::Float(self.drift_smoothing))
        } else if id == &self.param_multi_channel_analysis {
            Some(ParameterValue::Bool(self.multi_channel_analysis))
        } else if id == &self.param_confidence_threshold {
            Some(ParameterValue::Float(self.confidence_threshold))
        } else if id == &self.param_reference_frequency_hz {
            Some(ParameterValue::Float(self.reference_frequency_hz))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("PND sample rate must be non-zero".to_string());
        }
        if self.reference_frequency_hz > 0.0
            && self.reference_frequency_hz >= sample_rate as f32 * 0.5
        {
            return Err(format!(
                "reference_frequency_hz must be below Nyquist ({:.1} Hz)",
                sample_rate as f32 * 0.5
            ));
        }
        self.sample_rate = sample_rate;
        self.last_analysis_generation = 0;
        self.reference_transition_pending = false;
        self.init_analyzers();

        let smoothing_frames =
            (sample_rate as f32 * CORRECTION_STRENGTH_SMOOTH_MS / 1000.0).max(1.0);
        self.correction_strength_alpha = 1.0 - (-1.0 / smoothing_frames).exp();
        self.correction_strength_current = self.correction_strength;
        self.vocoder = Some(PhaseVocoder::new(self.channels));

        self.initialized = true;

        Ok(())
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        if num_frames == 0 {
            return Ok(0);
        }
        let total_input_samples = num_frames
            .checked_mul(self.channels)
            .ok_or_else(|| "Frame/channel count overflow".to_string())?;

        if input.len() != total_input_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                total_input_samples,
                input.len()
            ));
        }
        if output.len() != total_input_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                total_input_samples,
                output.len()
            ));
        }
        if context.sample_rate != self.sample_rate {
            return Err(format!(
                "Process context sample rate {} does not match initialized sample rate {}",
                context.sample_rate, self.sample_rate
            ));
        }
        // Keep every fallible validation ahead of analyzer/vocoder mutation.
        // Once this gate succeeds the prepared DSP path is infallible, so a
        // rejected callback can be repaired and retried without losing clock,
        // FFT, phase, or diagnostic state.
        if input.iter().any(|sample| !sample.is_finite()) {
            return Err("PND input contains a non-finite sample".to_string());
        }

        self.process_phase_vocoder(input, output, context)
    }

    fn reset(&mut self) {
        for analyzer in &mut self.analyzers {
            analyzer.reset();
        }
        self.current_ratio = 1.0;
        self.last_drift_ratio = 1.0;
        self.last_analysis_generation = 0;
        self.reference_transition_pending = false;
        self.correction_strength_current = self.correction_strength;
        self.cache_update_counter = 0;
        self.cache.update(|data| *data = PndData::default());
        if let Some(v) = &mut self.vocoder {
            v.reset();
        }
    }

    fn latency_samples(&self) -> usize {
        PV_LATENCY_FRAMES
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}
