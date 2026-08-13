use super::analysis::PndAnalyzer;
pub use super::config::PndPluginParams;
use super::consts::CORRECTION_STRENGTH_SMOOTH_MS;
use super::consts::PV_FFT_SIZE;
use super::consts::PV_HOP_SIZE;
use super::consts::RESAMPLER_CHUNK_SIZE;
use super::phase_vocoder::PhaseVocoder;
use super::types::PndData;
use crate::params::PARAMS as PD;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_bridge::apply_spec_update_modes;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::simd::{deinterleave_stereo, interleave_stereo};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

pub struct PndPlugin {
    // Configuration
    pub(super) channels: usize,
    pub(super) sample_rate: u32,

    // Components — one analyzer per channel for multi-channel analysis
    pub(super) analyzers: Vec<PndAnalyzer>,
    pub(super) resampler: Option<Async<f32>>,

    // State
    pub(super) current_ratio: f64,
    pub(super) last_drift_ratio: f64,
    pub(super) last_analysis_generation: u64,
    pub(super) reference_transition_pending: bool,

    // Pre-allocated buffers for zero-allocation process()
    pub(super) planar_input: Vec<Vec<f32>>,
    pub(super) planar_output: Vec<Vec<f32>>,

    // Block buffering for arbitrary host block sizes (Circular buffers)
    pub(super) input_ring: Vec<f32>,
    pub(super) input_ring_write_pos: usize,
    pub(super) input_ring_read_pos: usize,
    pub(super) input_ring_count: usize,

    pub(super) output_ring: Vec<f32>,
    pub(super) output_ring_write_pos: usize,
    pub(super) output_ring_read_pos: usize,
    pub(super) output_ring_count: usize,

    // Temp buffer for wrapped chunks
    pub(super) interleaved_chunk_buffer: Vec<f32>,

    // Scratch buffer for median computation across channels
    pub(super) channel_drift_scratch: Vec<f32>,
    // Scratch buffer for confidence-weighted channel consensus
    pub(super) channel_consensus_scratch: Vec<(f32, f32)>,

    // Parameters
    pub(super) param_correction_strength: ParameterId,
    pub(super) correction_strength: f32,
    pub(super) correction_strength_smoother: Smoother,

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

    pub(super) param_phase_vocoder: ParameterId,
    pub(super) phase_vocoder: bool,
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
            resampler: None,
            current_ratio: 1.0,
            last_drift_ratio: 1.0,
            last_analysis_generation: 0,
            reference_transition_pending: false,
            planar_input: vec![Vec::new(); channels],
            planar_output: Vec::new(),
            input_ring: Vec::new(),
            input_ring_write_pos: 0,
            input_ring_read_pos: 0,
            input_ring_count: 0,
            output_ring: Vec::new(),
            output_ring_write_pos: 0,
            output_ring_read_pos: 0,
            output_ring_count: 0,
            interleaved_chunk_buffer: Vec::new(),
            channel_drift_scratch: vec![0.0; channels],
            channel_consensus_scratch: vec![(1.0, 0.0); channels],

            param_correction_strength: ParameterId::from("correction_strength"),
            correction_strength: pk(PD, "correction_strength").default_f64() as f32,
            // Rough default; re-initialized in initialize() with correct chunk rate
            correction_strength_smoother: Smoother::new(
                pk(PD, "correction_strength").default_f64() as f32,
                CORRECTION_STRENGTH_SMOOTH_MS,
                43, // ~44100/1024
            ),

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

            param_phase_vocoder: ParameterId::from("phase_vocoder"),
            phase_vocoder: pk(PD, "phase_vocoder").default_bool(),
            vocoder: None,

            cache: RealTimeCache::new(PndData::default()),
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
            Parameter::new_bool("phase_vocoder", "Phase Vocoder", self.phase_vocoder)
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
        plugin.phase_vocoder = params.phase_vocoder;
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
        if self.planar_input.iter().any(|ch| ch.len() < num_frames) {
            let capacity = self.planar_input.first().map_or(0, Vec::len);
            if capacity == 0 {
                return Err("Phase vocoder has no prepared input capacity".to_string());
            }

            let mut processed = 0;
            while processed < num_frames {
                let chunk_frames = (num_frames - processed).min(capacity);
                let sample_start = processed * self.channels;
                let sample_end = sample_start + chunk_frames * self.channels;
                let chunk_ctx = ProcessContext::new(context.sample_rate, chunk_frames);
                self.process_phase_vocoder(
                    &input[sample_start..sample_end],
                    &mut output[sample_start..sample_end],
                    &chunk_ctx,
                )?;
                processed += chunk_frames;
            }
            return Ok(num_frames);
        }

        // Deinterleave input into planar buffers for analysis
        for c in 0..self.channels {
            for i in 0..num_frames {
                self.planar_input[c][i] = input[i * self.channels + c];
            }
        }

        // Analyze drift — use multi-channel median when enabled (same logic as resampler path)
        let (drift_ratio, confidence) = if self.analyzers.is_empty() {
            (1.0, 0.0)
        } else if self.multi_channel_analysis && self.analyzers.len() > 1 {
            let n = self.analyzers.len().min(self.channels);
            for (ch, analyzer) in self.analyzers.iter_mut().enumerate().take(n) {
                let temporal_drift = analyzer.analyze(&self.planar_input[ch][..num_frames]);
                self.channel_consensus_scratch[ch] = if self.reference_frequency_hz > 0.0 {
                    analyzer.estimate_against_reference(self.reference_frequency_hz)
                } else {
                    (temporal_drift, analyzer.confidence())
                };
            }
            weighted_channel_consensus(
                &mut self.channel_consensus_scratch[..n],
                self.confidence_threshold,
            )
        } else {
            let temporal_drift = self.analyzers[0].analyze(&self.planar_input[0][..num_frames]);
            if self.reference_frequency_hz > 0.0 {
                self.analyzers[0].estimate_against_reference(self.reference_frequency_hz)
            } else {
                (temporal_drift, self.analyzers[0].confidence())
            }
        };
        self.last_drift_ratio = drift_ratio as f64;

        // Calculate correction ratio
        let analysis_generation = self
            .analyzers
            .iter()
            .map(PndAnalyzer::analysis_generation)
            .max()
            .unwrap_or(0);
        let elapsed_hops = analysis_generation.saturating_sub(self.last_analysis_generation);
        self.last_analysis_generation = analysis_generation;
        if confidence >= self.confidence_threshold && elapsed_hops > 0 {
            let target_correction = 1.0 / drift_ratio as f64;
            self.current_ratio = smooth_drift_ratio(
                self.current_ratio,
                target_correction,
                self.drift_smoothing,
                elapsed_hops as usize * (2048 / 4),
                self.sample_rate,
            );
            self.reference_transition_pending = false;
        } else if self.reference_transition_pending && elapsed_hops > 0 {
            self.current_ratio = smooth_drift_ratio(
                self.current_ratio,
                1.0,
                self.drift_smoothing,
                elapsed_hops as usize * (2048 / 4),
                self.sample_rate,
            );
        }

        // Advance the smoother to prevent zipper noise on rapid strength changes.
        let strength = self.correction_strength_smoother.advance() as f64;
        let pitch_shift = (1.0 + (self.current_ratio - 1.0) * strength) as f32;

        // Feed samples to each channel's vocoder and drain output
        for i in 0..num_frames {
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
            // Aggregate analyzer diagnostics (same logic as resampler path)
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

    pub(super) fn init_resampler(&mut self) -> PluginResult<()> {
        let resampler = Async::<f32>::new_poly(
            1.0, // Initial ratio
            1.1, // Max ratio
            PolynomialDegree::Cubic,
            RESAMPLER_CHUNK_SIZE,
            self.channels,
            FixedAsync::Input,
        )
        .map_err(|e| format!("Failed to create resampler: {:?}", e))?;

        self.resampler = Some(resampler);
        Ok(())
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
        self.channel_drift_scratch.resize(num_analyzers.max(1), 0.0);
        self.channel_consensus_scratch
            .resize(num_analyzers.max(1), (1.0, 0.0));
    }

    /// Process one resampler chunk from the input ring buffer.
    /// Appends resampled output to the output ring buffer.
    pub(super) fn process_one_chunk(&mut self) -> Result<(), String> {
        let resampler = self.resampler.as_mut().ok_or("Resampler not initialized")?;
        let chunk_frames = resampler.input_frames_next();
        let chunk_samples = chunk_frames * self.channels;

        // 1. Get contiguous input chunk (Circular)
        let cap_in = self.input_ring.len();
        let input_slice = if self.input_ring_read_pos + chunk_samples <= cap_in {
            // Contiguous path
            &self.input_ring[self.input_ring_read_pos..self.input_ring_read_pos + chunk_samples]
        } else {
            // Wrapped path: copy to temp buffer
            let first_part = cap_in - self.input_ring_read_pos;
            self.interleaved_chunk_buffer[..first_part]
                .copy_from_slice(&self.input_ring[self.input_ring_read_pos..]);
            let second_part = chunk_samples - first_part;
            self.interleaved_chunk_buffer[first_part..chunk_samples]
                .copy_from_slice(&self.input_ring[..second_part]);
            &self.interleaved_chunk_buffer[..chunk_samples]
        };

        // 2. De-interleave input into planar buffers
        debug_assert!(self.planar_input.iter().all(|ch| ch.len() >= chunk_frames));
        if self.channels == 2 {
            let (left, rest) = self.planar_input.split_at_mut(1);
            deinterleave_stereo(
                &input_slice[..chunk_frames * 2],
                &mut left[0][..chunk_frames],
                &mut rest[0][..chunk_frames],
            );
        } else {
            for i in 0..chunk_frames {
                for c in 0..self.channels {
                    self.planar_input[c][i] = input_slice[i * self.channels + c];
                }
            }
        }

        // 3. Analyze channels for drift
        let (drift_ratio, confidence) = if self.analyzers.is_empty() {
            (1.0_f32, 0.0_f32)
        } else if self.multi_channel_analysis && self.analyzers.len() > 1 {
            // Analyze each channel independently and take the median drift ratio
            let n = self.analyzers.len().min(self.channels);
            for (ch, analyzer) in self.analyzers.iter_mut().enumerate().take(n) {
                let temporal_drift = analyzer.analyze(&self.planar_input[ch][..chunk_frames]);
                self.channel_consensus_scratch[ch] = if self.reference_frequency_hz > 0.0 {
                    analyzer.estimate_against_reference(self.reference_frequency_hz)
                } else {
                    (temporal_drift, analyzer.confidence())
                };
            }
            weighted_channel_consensus(
                &mut self.channel_consensus_scratch[..n],
                self.confidence_threshold,
            )
        } else {
            // Single-channel mode: analyze channel 0 only
            let temporal_drift = self.analyzers[0].analyze(&self.planar_input[0][..chunk_frames]);
            if self.reference_frequency_hz > 0.0 {
                self.analyzers[0].estimate_against_reference(self.reference_frequency_hz)
            } else {
                (temporal_drift, self.analyzers[0].confidence())
            }
        };
        self.last_drift_ratio = drift_ratio as f64;

        // 4. Calculate correction ratio (with confidence-based bypass)
        // When confidence is below threshold, freeze the current ratio
        // instead of applying an unreliable correction.
        let analysis_generation = self
            .analyzers
            .iter()
            .map(PndAnalyzer::analysis_generation)
            .max()
            .unwrap_or(0);
        let elapsed_hops = analysis_generation.saturating_sub(self.last_analysis_generation);
        self.last_analysis_generation = analysis_generation;
        if confidence >= self.confidence_threshold && elapsed_hops > 0 {
            // Rubato defines ratios above unity as producing more output
            // frames, which lowers pitch. A detected/reference ratio above
            // unity must therefore be passed through, unlike the reciprocal
            // factor required by the phase vocoder.
            let target_correction = drift_ratio as f64;
            self.current_ratio = smooth_drift_ratio(
                self.current_ratio,
                target_correction,
                self.drift_smoothing,
                elapsed_hops as usize * (2048 / 4),
                self.sample_rate,
            );
            self.reference_transition_pending = false;
        } else if self.reference_transition_pending && elapsed_hops > 0 {
            self.current_ratio = smooth_drift_ratio(
                self.current_ratio,
                1.0,
                self.drift_smoothing,
                elapsed_hops as usize * (2048 / 4),
                self.sample_rate,
            );
        }
        // else: current_ratio stays frozen at its last reliable value

        let strength = self.correction_strength_smoother.advance() as f64;
        let final_ratio = 1.0 + (self.current_ratio - 1.0) * strength;

        resampler
            .set_resample_ratio(final_ratio, true)
            .map_err(|e| format!("{:?}", e))?;

        // Update read position (Circular)
        self.input_ring_read_pos = (self.input_ring_read_pos + chunk_samples) % cap_in;
        self.input_ring_count -= chunk_samples;

        // 5. Resample
        let max_output_frames = resampler.output_frames_max();
        debug_assert!(
            self.planar_output
                .iter()
                .all(|ch| ch.len() >= max_output_frames)
        );

        let input_adapter =
            SequentialSliceOfVecs::new(&self.planar_input, self.channels, chunk_frames)
                .map_err(|e| format!("{:?}", e))?;
        let mut output_adapter = SequentialSliceOfVecs::new_mut(
            &mut self.planar_output,
            self.channels,
            max_output_frames,
        )
        .map_err(|e| format!("{:?}", e))?;

        let (_, out_written) = resampler
            .process_into_buffer(&input_adapter, &mut output_adapter, None)
            .map_err(|e| format!("{:?}", e))?;

        // 6. Re-interleave into output ring (Circular)
        let out_samples = out_written * self.channels;
        let cap_out = self.output_ring.len();
        if out_samples > cap_out {
            return Err(format!(
                "Resampler output chunk ({out_samples} samples) exceeds prepared ring ({cap_out})"
            ));
        }
        // A fixed-frame host cannot wait for a variable-rate SRC. Keep the
        // queue bounded by discarding the oldest complete frames when a
        // sustained correction produces faster than the callback consumes.
        // The callback has a dry fallback for the opposite (underrun) case,
        // so this policy preserves the frame contract without overflow errors
        // or unbounded latency.
        let required_drop = self
            .output_ring_count
            .saturating_add(out_samples)
            .saturating_sub(cap_out);
        if required_drop > 0 {
            let drop_samples = (required_drop / self.channels) * self.channels;
            self.output_ring_read_pos = (self.output_ring_read_pos + drop_samples) % cap_out;
            self.output_ring_count -= drop_samples;
        }

        if self.channels == 2 {
            // Need a contiguous target slice for interleave_stereo
            if self.output_ring_write_pos + out_samples <= cap_out {
                interleave_stereo(
                    &self.planar_output[0][..out_written],
                    &self.planar_output[1][..out_written],
                    &mut self.output_ring
                        [self.output_ring_write_pos..self.output_ring_write_pos + out_samples],
                );
            } else {
                // Wrapped output write: interleave to temp then copy in two parts
                interleave_stereo(
                    &self.planar_output[0][..out_written],
                    &self.planar_output[1][..out_written],
                    &mut self.interleaved_chunk_buffer[..out_samples],
                );
                let first_part = cap_out - self.output_ring_write_pos;
                self.output_ring[self.output_ring_write_pos..]
                    .copy_from_slice(&self.interleaved_chunk_buffer[..first_part]);
                let second_part = out_samples - first_part;
                self.output_ring[..second_part]
                    .copy_from_slice(&self.interleaved_chunk_buffer[first_part..out_samples]);
            }
        } else {
            for i in 0..out_written {
                for c in 0..self.channels {
                    let idx = (self.output_ring_write_pos + i * self.channels + c) % cap_out;
                    self.output_ring[idx] = self.planar_output[c][i];
                }
            }
        }
        self.output_ring_write_pos = (self.output_ring_write_pos + out_samples) % cap_out;
        self.output_ring_count += out_samples;

        Ok(())
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
fn weighted_channel_consensus(
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

    observations[..valid]
        .sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let total_weight: f32 = observations[..valid].iter().map(|(_, c)| *c).sum();
    let target_weight = total_weight * 0.5;
    let mut accumulated = 0.0;
    let mut weighted_median = observations[valid - 1].0;
    for &(ratio, confidence) in &observations[..valid] {
        accumulated += confidence;
        if accumulated >= target_weight {
            weighted_median = ratio;
            break;
        }
    }

    (weighted_median, total_weight / valid as f32)
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
                self.correction_strength_smoother
                    .set_target(self.correction_strength);
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
        } else if id == self.param_phase_vocoder {
            if self.initialized {
                return Err(
                    "phase_vocoder is a structural setup parameter; rebuild the plugin".to_string(),
                );
            }
            let v = value
                .as_bool()
                .unwrap_or(pk(PD, "phase_vocoder").default_bool());
            self.phase_vocoder = v;
            if v && self.vocoder.is_none() {
                self.vocoder = Some(PhaseVocoder::new(self.channels));
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
        } else if id == &self.param_phase_vocoder {
            Some(ParameterValue::Bool(self.phase_vocoder))
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
        self.init_resampler()?;
        self.init_analyzers();

        // Pre-allocate buffers sized for resampler chunk requirements
        self.planar_input = vec![vec![0.0; RESAMPLER_CHUNK_SIZE]; self.channels];

        let max_output_frames = if let Some(ref resampler) = self.resampler {
            resampler.output_frames_max()
        } else {
            (RESAMPLER_CHUNK_SIZE as f64 * 1.1) as usize + 16
        };
        self.planar_output = vec![vec![0.0; max_output_frames]; self.channels];

        // Allocate block buffering rings
        // Input ring: hold up to 4 resampler chunks worth of interleaved samples
        let input_ring_capacity = RESAMPLER_CHUNK_SIZE * self.channels * 4;
        self.input_ring = vec![0.0; input_ring_capacity];
        self.input_ring_write_pos = 0;
        self.input_ring_read_pos = 0;
        self.input_ring_count = 0;

        // Output ring: hold up to 4 chunks worth of resampled output
        let output_ring_capacity = max_output_frames * self.channels * 4;
        self.output_ring = vec![0.0; output_ring_capacity];
        self.output_ring_write_pos = 0;
        self.output_ring_read_pos = 0;
        self.output_ring_count = 0;

        self.interleaved_chunk_buffer = vec![0.0; RESAMPLER_CHUNK_SIZE * self.channels * 2];

        // Initialize correction_strength smoother at chunk rate
        let chunk_rate = (sample_rate as f32 / RESAMPLER_CHUNK_SIZE as f32) as u32;
        self.correction_strength_smoother = Smoother::new(
            self.correction_strength,
            CORRECTION_STRENGTH_SMOOTH_MS,
            chunk_rate.max(1),
        );

        // Initialize phase vocoder if enabled
        if self.phase_vocoder {
            self.vocoder = Some(PhaseVocoder::new(self.channels));
        }

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

        if self.phase_vocoder {
            return self.process_phase_vocoder(input, output, context);
        }

        let input_space = self.input_ring.len().saturating_sub(self.input_ring_count);
        if total_input_samples > input_space {
            return Err(format!(
                "Input block too large for prepared PND ring: need {}, available {}",
                total_input_samples, input_space
            ));
        }

        // 1. Accumulate input into input ring (Circular)
        {
            let cap = self.input_ring.len();
            let first_part = total_input_samples.min(cap - self.input_ring_write_pos);
            self.input_ring[self.input_ring_write_pos..self.input_ring_write_pos + first_part]
                .copy_from_slice(&input[..first_part]);
            if total_input_samples > first_part {
                let second_part = total_input_samples - first_part;
                self.input_ring[..second_part].copy_from_slice(&input[first_part..]);
            }
            self.input_ring_write_pos = (self.input_ring_write_pos + total_input_samples) % cap;
            self.input_ring_count += total_input_samples;
        }

        // 2. Process complete resampler chunks
        if let Some(resampler) = &self.resampler {
            let chunk_samples = resampler.input_frames_next() * self.channels;
            while self.input_ring_count >= chunk_samples {
                self.process_one_chunk()?;
            }
        }

        // 3. Drain output ring to output buffer (Circular)
        let total_output_samples = num_frames * self.channels;
        let drain_samples = self.output_ring_count.min(total_output_samples);
        let drain_frames = drain_samples / self.channels;

        if drain_samples > 0 {
            let cap = self.output_ring.len();
            let first_part = drain_samples.min(cap - self.output_ring_read_pos);
            output[..first_part].copy_from_slice(
                &self.output_ring
                    [self.output_ring_read_pos..self.output_ring_read_pos + first_part],
            );
            if drain_samples > first_part {
                let second_part = drain_samples - first_part;
                output[first_part..drain_samples].copy_from_slice(&self.output_ring[..second_part]);
            }
            self.output_ring_read_pos = (self.output_ring_read_pos + drain_samples) % cap;
            self.output_ring_count -= drain_samples;
        }

        // The SRC may temporarily underrun while its variable-rate output
        // catches up. Preserve the fixed-frame contract with the corresponding
        // input samples instead of synthesizing silence.
        if drain_frames < num_frames {
            let zero_start = drain_frames * self.channels;
            output[zero_start..total_output_samples]
                .copy_from_slice(&input[zero_start..total_output_samples]);
        }

        // Report num_frames to prevent ring buffer underruns in host
        let nf = context.num_frames;

        // Update diagnostic cache (throttled)
        self.cache_update_counter += 1;
        if self.cache_update_counter >= 10 {
            self.cache_update_counter = 0;
            let (confidence, matched_partials, total_peaks) = if self.analyzers.is_empty() {
                (0.0, 0, 0)
            } else {
                // Aggregate: average confidence, sum matched/total across analyzers
                let n = self.analyzers.len();
                let avg_conf =
                    self.analyzers.iter().map(|a| a.confidence()).sum::<f32>() / n as f32;
                let total_matched: usize =
                    self.analyzers.iter().map(|a| a.matched_partials()).sum();
                let total_pk: usize = self.analyzers.iter().map(|a| a.total_peaks()).sum();
                (avg_conf, total_matched, total_pk)
            };

            let drift = self.last_drift_ratio;
            let correction = self.current_ratio;
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

    fn reset(&mut self) {
        for analyzer in &mut self.analyzers {
            analyzer.reset();
        }
        self.current_ratio = 1.0;
        self.last_drift_ratio = 1.0;
        self.last_analysis_generation = 0;
        self.reference_transition_pending = false;
        self.input_ring_write_pos = 0;
        self.input_ring_read_pos = 0;
        self.input_ring_count = 0;
        self.output_ring_write_pos = 0;
        self.output_ring_read_pos = 0;
        self.output_ring_count = 0;
        self.correction_strength_smoother
            .reset(self.correction_strength);
        // Re-create the resampler to flush rubato's internal delay lines.
        // Errors are ignored here (init_resampler only fails if parameters are
        // out of range, which cannot change between initialize() and reset()).
        let _ = self.init_resampler();
        if let Some(v) = &mut self.vocoder {
            v.reset();
        }
    }

    fn latency_samples(&self) -> usize {
        if self.phase_vocoder {
            // Phase vocoder latency: one full FFT frame plus one hop of overlap-add.
            PV_FFT_SIZE + PV_HOP_SIZE
        } else {
            RESAMPLER_CHUNK_SIZE
        }
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}
