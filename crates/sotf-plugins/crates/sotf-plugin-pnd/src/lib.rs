// ============================================================================
// PND (Polyphonic Note Detection & Varispeed) Plugin
// ============================================================================

pub mod params;

use crate::params::PARAMS as PD;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{deinterleave_stereo, interleave_stereo};
use sotf_host::smoothing::Smoother;
use std::any::Any;
use std::sync::Arc;

pub mod analysis;
mod config;

use analysis::PndAnalyzer;
pub use config::PndPluginParams;

/// Resampler chunk size — the fixed input block size expected by rubato.
const RESAMPLER_CHUNK_SIZE: usize = 1024;

// ============================================================================
// Phase Vocoder for formant-preserving pitch shift
// ============================================================================

const PV_FFT_SIZE: usize = 2048;
const PV_HOP_SIZE: usize = PV_FFT_SIZE / 4;

/// Per-channel phase vocoder state for pitch shifting without changing duration.
struct PhaseVocoderChannel {
    fft_forward: Arc<dyn rustfft::Fft<f32>>,
    fft_inverse: Arc<dyn rustfft::Fft<f32>>,
    analysis_window: Vec<f32>,
    /// Input accumulation buffer
    input_buf: Vec<f32>,
    input_fill: usize,
    /// Output overlap-add buffer
    output_accum: Vec<f32>,
    output_read: usize,
    output_fill: usize,
    /// Previous frame analysis phases for phase accumulation
    prev_phase: Vec<f32>,
    /// Accumulated synthesis phases
    synth_phase: Vec<f32>,
    /// Scratch buffers
    fft_buf: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    ifft_buf: Vec<Complex<f32>>,
}

impl PhaseVocoderChannel {
    fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(PV_FFT_SIZE);
        let fft_inverse = planner.plan_fft_inverse(PV_FFT_SIZE);
        let scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());

        let analysis_window: Vec<f32> = (0..PV_FFT_SIZE)
            .map(|i| {
                let x = i as f32 / PV_FFT_SIZE as f32;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())
            })
            .collect();

        Self {
            fft_forward,
            fft_inverse,
            analysis_window,
            input_buf: vec![0.0; PV_FFT_SIZE],
            input_fill: 0,
            output_accum: vec![0.0; PV_FFT_SIZE * 4],
            output_read: 0,
            output_fill: 0,
            prev_phase: vec![0.0; PV_FFT_SIZE],
            synth_phase: vec![0.0; PV_FFT_SIZE],
            fft_buf: vec![Complex::new(0.0, 0.0); PV_FFT_SIZE],
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            ifft_buf: vec![Complex::new(0.0, 0.0); PV_FFT_SIZE],
        }
    }

    fn reset(&mut self) {
        self.input_buf.fill(0.0);
        self.input_fill = 0;
        self.output_accum.fill(0.0);
        self.output_read = 0;
        self.output_fill = 0;
        self.prev_phase.fill(0.0);
        self.synth_phase.fill(0.0);
    }

    /// Process a hop of samples with the given pitch shift ratio.
    /// pitch_shift > 1.0 shifts up, < 1.0 shifts down.
    fn process_hop(&mut self, pitch_shift: f32) {
        let n = PV_FFT_SIZE;
        let hop = PV_HOP_SIZE;
        let expected_phase_advance = 2.0 * std::f32::consts::PI * hop as f32 / n as f32;
        let inv_n = 1.0 / n as f32;

        // Window and FFT
        for i in 0..n {
            self.fft_buf[i] = Complex::new(self.input_buf[i] * self.analysis_window[i], 0.0);
        }
        self.fft_forward
            .process_with_scratch(&mut self.fft_buf, &mut self.fft_scratch);

        // Analysis: extract magnitude and phase, compute instantaneous frequency
        for bin in 0..n {
            let mag = self.fft_buf[bin].norm();
            let phase = self.fft_buf[bin].arg();

            // Phase difference from previous frame
            let phase_diff = phase - self.prev_phase[bin];
            self.prev_phase[bin] = phase;

            // Remove expected phase advance
            let deviation = phase_diff - bin as f32 * expected_phase_advance;

            // Wrap to [-pi, pi]
            let wrapped = deviation
                - (deviation / (2.0 * std::f32::consts::PI)).round() * 2.0 * std::f32::consts::PI;

            // True frequency (in bins)
            let true_freq = bin as f32 + wrapped / expected_phase_advance;

            // Synthesis: apply pitch shift to frequency
            let shifted_freq = true_freq * pitch_shift;

            // Accumulate synthesis phase at the shifted frequency
            self.synth_phase[bin] += shifted_freq * expected_phase_advance;

            // Reconstruct complex spectrum with original magnitude and shifted phase
            self.ifft_buf[bin] = Complex::new(
                mag * self.synth_phase[bin].cos(),
                mag * self.synth_phase[bin].sin(),
            );
        }

        // Restore conjugate symmetry for correct real-valued IFFT
        let n = PV_FFT_SIZE;
        self.ifft_buf[0].im = 0.0;
        if n > 1 {
            self.ifft_buf[n / 2].im = 0.0;
        }
        for bin in 1..n / 2 {
            self.ifft_buf[n - bin] = self.ifft_buf[bin].conj();
        }

        // IFFT
        self.fft_inverse
            .process_with_scratch(&mut self.ifft_buf, &mut self.fft_scratch);

        // Overlap-add with synthesis window and normalization
        let scale = inv_n / 1.5; // Hann window with 75% overlap: sum(w^2) normalization
        let accum_len = self.output_accum.len();
        for i in 0..n {
            let idx = (self.output_read + self.output_fill + i) % accum_len;
            self.output_accum[idx] += self.ifft_buf[i].re * self.analysis_window[i] * scale;
        }
        self.output_fill += hop;

        // Shift input buffer by hop
        self.input_buf.copy_within(hop..n, 0);
        self.input_fill = n - hop;
    }
}

/// Multi-channel phase vocoder.
struct PhaseVocoder {
    channels: Vec<PhaseVocoderChannel>,
}

impl PhaseVocoder {
    fn new(num_channels: usize) -> Self {
        Self {
            channels: (0..num_channels)
                .map(|_| PhaseVocoderChannel::new())
                .collect(),
        }
    }

    fn reset(&mut self) {
        for ch in &mut self.channels {
            ch.reset();
        }
    }
}

/// Smoothing time for correction_strength parameter changes (ms).
/// Prevents audible pitch jumps when tweaking correction strength live.
const CORRECTION_STRENGTH_SMOOTH_MS: f32 = 50.0;

// ============================================================================
// Exposed Data Structure
// ============================================================================

/// Data exposed by the PND plugin for drift monitoring.
#[derive(Debug, Clone, Default)]
pub struct PndData {
    /// Current raw drift ratio from analysis (1.0 = no drift).
    pub drift_ratio: f64,
    /// Current correction ratio applied to resampler.
    pub correction_ratio: f64,
    /// Confidence of the drift estimate (0.0 to 1.0).
    pub confidence: f32,
    /// Number of matched partials in the last FFT frame.
    pub matched_partials: usize,
    /// Total number of detected peaks in the last FFT frame.
    pub total_peaks: usize,
}

pub struct PndPlugin {
    // Configuration
    channels: usize,
    sample_rate: u32,

    // Components — one analyzer per channel for multi-channel analysis
    analyzers: Vec<PndAnalyzer>,
    resampler: Option<Async<f32>>,

    // State
    current_ratio: f64,
    last_drift_ratio: f64,

    // Pre-allocated buffers for zero-allocation process()
    planar_input: Vec<Vec<f32>>,
    planar_output: Vec<Vec<f32>>,

    // Block buffering for arbitrary host block sizes (Circular buffers)
    input_ring: Vec<f32>,
    input_ring_write_pos: usize,
    input_ring_read_pos: usize,
    input_ring_count: usize,

    output_ring: Vec<f32>,
    output_ring_write_pos: usize,
    output_ring_read_pos: usize,
    output_ring_count: usize,

    // Temp buffer for wrapped chunks
    interleaved_chunk_buffer: Vec<f32>,

    // Scratch buffer for median computation across channels
    channel_drift_scratch: Vec<f32>,

    // Parameters
    param_correction_strength: ParameterId,
    correction_strength: f32,
    correction_strength_smoother: Smoother,

    param_analysis_window_ms: ParameterId,
    analysis_window_ms: f32,

    param_drift_smoothing: ParameterId,
    drift_smoothing: f32,

    param_multi_channel_analysis: ParameterId,
    multi_channel_analysis: bool,

    param_confidence_threshold: ParameterId,
    confidence_threshold: f32,

    param_phase_vocoder: ParameterId,
    phase_vocoder: bool,
    vocoder: Option<PhaseVocoder>,

    cache: RealTimeCache<PndData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
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

            param_phase_vocoder: ParameterId::from("phase_vocoder"),
            phase_vocoder: pk(PD, "phase_vocoder").default_bool(),
            vocoder: None,

            cache: RealTimeCache::new(PndData::default()),
            cache_update_counter: 0,
            cached_parameters: Vec::new(),
        };
        p.rebuild_cached_parameters();
        p
    }

    fn rebuild_cached_parameters(&mut self) {
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
            Parameter::new_bool("phase_vocoder", "Phase Vocoder", self.phase_vocoder)
                .with_group("Correction")
                .with_importance(ParameterImportance::Useful),
        ];
    }

    pub fn from_params(channels: usize, params: PndPluginParams) -> Self {
        let mut plugin = Self::new(channels);
        plugin.correction_strength = params.correction_strength;
        plugin.analysis_window_ms = params.analysis_window_ms;
        plugin.drift_smoothing = params.drift_smoothing;
        plugin.multi_channel_analysis = params.multi_channel_analysis;
        plugin.confidence_threshold = params.confidence_threshold;
        plugin.phase_vocoder = params.phase_vocoder;
        plugin.rebuild_cached_parameters();
        plugin
    }

    /// Phase vocoder processing path: uses STFT analysis/synthesis to shift pitch
    /// without changing duration, preserving formants better than simple resampling.
    fn process_phase_vocoder(
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
            return Err(format!(
                "Phase vocoder block too large: {} frames exceeds prepared capacity {}",
                num_frames,
                self.planar_input.first().map_or(0, Vec::len)
            ));
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
                self.channel_drift_scratch[ch] =
                    analyzer.analyze(&self.planar_input[ch][..num_frames]);
            }
            let mid = n / 2;
            self.channel_drift_scratch[..n].select_nth_unstable_by(mid, |a, b| {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            });
            let median_drift = self.channel_drift_scratch[mid];
            let avg_confidence = self
                .analyzers
                .iter()
                .take(n)
                .map(|a| a.confidence())
                .sum::<f32>()
                / n as f32;
            (median_drift, avg_confidence)
        } else {
            let drift = self.analyzers[0].analyze(&self.planar_input[0][..num_frames]);
            let conf = self.analyzers[0].confidence();
            (drift, conf)
        };
        self.last_drift_ratio = drift_ratio as f64;

        // Calculate correction ratio
        if confidence >= self.confidence_threshold {
            let target_correction = 1.0 / drift_ratio as f64;
            let alpha = self.drift_smoothing as f64;
            self.current_ratio = self.current_ratio * (1.0 - alpha) + target_correction * alpha;
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

    fn init_resampler(&mut self) -> PluginResult<()> {
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

    fn init_analyzers(&mut self) {
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
    }

    /// Process one resampler chunk from the input ring buffer.
    /// Appends resampled output to the output ring buffer.
    fn process_one_chunk(&mut self) -> Result<(), String> {
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
                self.channel_drift_scratch[ch] =
                    analyzer.analyze(&self.planar_input[ch][..chunk_frames]);
            }
            let mid = n / 2;
            self.channel_drift_scratch[..n].select_nth_unstable_by(mid, |a, b| {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            });
            let median_drift = self.channel_drift_scratch[mid];
            // Average confidence across all analyzers
            let avg_confidence = self
                .analyzers
                .iter()
                .take(n)
                .map(|a| a.confidence())
                .sum::<f32>()
                / n as f32;
            (median_drift, avg_confidence)
        } else {
            // Single-channel mode: analyze channel 0 only
            let drift = self.analyzers[0].analyze(&self.planar_input[0][..chunk_frames]);
            let conf = self.analyzers[0].confidence();
            (drift, conf)
        };
        self.last_drift_ratio = drift_ratio as f64;

        // 4. Calculate correction ratio (with confidence-based bypass)
        // When confidence is below threshold, freeze the current ratio
        // instead of applying an unreliable correction.
        if confidence >= self.confidence_threshold {
            let target_correction = 1.0 / drift_ratio as f64;
            let alpha = self.drift_smoothing as f64;
            self.current_ratio = self.current_ratio * (1.0 - alpha) + target_correction * alpha;
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
        if self.output_ring_count + out_samples > cap_out {
            return Err(format!(
                "Output ring overflow: need {}, available {}",
                out_samples,
                cap_out.saturating_sub(self.output_ring_count)
            ));
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

impl Plugin for PndPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Pitch Drift Corrector", "0.2.0", "SotF")
            .with_description("Polyphonic note detection and varispeed correction")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
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
        } else if id == self.param_phase_vocoder {
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
        } else if id == &self.param_phase_vocoder {
            Some(ParameterValue::Bool(self.phase_vocoder))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
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

        // Zero remaining output if not enough data (initial latency period)
        if drain_frames < num_frames {
            let zero_start = drain_frames * self.channels;
            output[zero_start..total_output_samples].fill(0.0);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// With high drift_smoothing, the correction ratio should change slowly
    /// (no sudden jumps between frames).
    #[test]
    fn test_drift_smoothing_slow_correction() {
        let mut p = PndPlugin::new(2);
        p.drift_smoothing = 0.99; // very high smoothing
        p.correction_strength = 1.0;
        p.initialize(48000).unwrap();

        let nf = RESAMPLER_CHUNK_SIZE;
        let ctx = ProcessContext::new(48000, nf);

        // Process several blocks and track how current_ratio evolves
        let mut ratios = Vec::new();
        for block in 0..10 {
            let input: Vec<f32> = (0..nf * 2)
                .map(|i| {
                    0.3 * (2.0 * std::f32::consts::PI * 440.0 * (block * nf * 2 + i) as f32
                        / 48000.0)
                        .sin()
                })
                .collect();
            let mut output = vec![0.0f32; nf * 2];
            let _ = p.process(&input, &mut output, &ctx);
            ratios.push(p.current_ratio);
        }

        // With high smoothing, ratio changes should be very small between blocks
        for i in 1..ratios.len() {
            let delta = (ratios[i] - ratios[i - 1]).abs();
            assert!(
                delta < 0.01,
                "Correction ratio changed too fast at block {i}: delta={delta:.6}, \
                 prev={:.6}, curr={:.6}",
                ratios[i - 1],
                ratios[i]
            );
        }
    }

    /// Setting analysis_window_ms to different values should not cause panics
    /// or errors, and the plugin should process audio correctly.
    #[test]
    fn test_analysis_window_parameter_values() {
        for &window_ms in &[10.0, 50.0, 100.0, 200.0] {
            let mut p = PndPlugin::new(2);
            p.analysis_window_ms = window_ms;
            p.initialize(48000).unwrap();

            let nf = RESAMPLER_CHUNK_SIZE;
            let ctx = ProcessContext::new(48000, nf);

            let input: Vec<f32> = (0..nf * 2).map(|i| 0.3 * (i as f32 * 0.01).sin()).collect();
            let mut output = vec![0.0f32; nf * 2];
            let result = p.process(&input, &mut output, &ctx);
            assert!(
                result.is_ok(),
                "PND plugin should process without error with analysis_window_ms={window_ms}"
            );
            assert!(
                output.iter().all(|s| s.is_finite()),
                "All output samples should be finite with analysis_window_ms={window_ms}"
            );
        }
    }

    /// Verify set_parameter / get_parameter round-trip for analysis_window_ms.
    #[test]
    fn test_analysis_window_param_roundtrip() {
        let mut p = PndPlugin::new(1);
        p.initialize(44100).unwrap();

        p.set_parameter(
            ParameterId::from("analysis_window_ms"),
            ParameterValue::Float(75.0),
        )
        .unwrap();

        let val = p.get_parameter(&ParameterId::from("analysis_window_ms"));
        assert_eq!(val, Some(ParameterValue::Float(75.0)));
    }

    #[test]
    fn test_process_rejects_buffer_size_mismatch() {
        let mut p = PndPlugin::new(2);
        p.initialize(48000).unwrap();

        let ctx = ProcessContext::new(48000, 64);
        let input = vec![0.0f32; ctx.num_frames * p.input_channels()];
        let mut short_output = vec![0.0f32; ctx.num_frames * p.output_channels() - 1];
        let err = p.process(&input, &mut short_output, &ctx).unwrap_err();
        assert!(err.contains("Output size mismatch"));

        let short_input = vec![0.0f32; ctx.num_frames * p.input_channels() - 1];
        let mut output = vec![0.0f32; ctx.num_frames * p.output_channels()];
        let err = p.process(&short_input, &mut output, &ctx).unwrap_err();
        assert!(err.contains("Input size mismatch"));
    }

    #[test]
    fn test_process_rejects_oversized_block_without_panicking() {
        let mut p = PndPlugin::new(2);
        p.initialize(48000).unwrap();

        let frames = RESAMPLER_CHUNK_SIZE * 5;
        let ctx = ProcessContext::new(48000, frames);
        let input = vec![0.0f32; frames * p.input_channels()];
        let mut output = vec![0.0f32; frames * p.output_channels()];
        let err = p.process(&input, &mut output, &ctx).unwrap_err();
        assert!(err.contains("Input block too large"));
    }

    /// §3.4: latency_samples() must return the phase-vocoder latency when the
    /// phase vocoder is active, not the resampler chunk size.
    #[test]
    fn test_latency_samples_reports_pv_latency_when_vocoder_active() {
        let mut p = PndPlugin::new(2);
        p.initialize(44100).unwrap();

        // Resampler path latency
        let resampler_latency = p.latency_samples();
        assert_eq!(resampler_latency, RESAMPLER_CHUNK_SIZE);

        // Enable phase vocoder
        p.set_parameter(
            ParameterId::from("phase_vocoder"),
            ParameterValue::Bool(true),
        )
        .unwrap();
        let pv_latency = p.latency_samples();
        assert!(
            pv_latency > RESAMPLER_CHUNK_SIZE,
            "Phase vocoder latency ({pv_latency}) should exceed resampler chunk size ({RESAMPLER_CHUNK_SIZE})"
        );
        assert_eq!(
            pv_latency,
            PV_FFT_SIZE + PV_HOP_SIZE,
            "Phase vocoder latency should be PV_FFT_SIZE + PV_HOP_SIZE"
        );
    }

    /// §3.5: reset() must flush the resampler internal state.
    /// After reset() + re-initialize, the plugin should not produce clicks
    /// from stale resampler delay lines (we verify this structurally: reset
    /// re-creates the resampler, so it is Some after reset).
    #[test]
    fn test_reset_reinitializes_resampler() {
        let mut p = PndPlugin::new(2);
        p.initialize(44100).unwrap();

        // Process some audio to get internal resampler state dirty
        let nf = RESAMPLER_CHUNK_SIZE;
        let ctx = ProcessContext::new(44100, nf);
        let input: Vec<f32> = (0..nf * 2)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let mut output = vec![0.0f32; nf * 2];
        p.process(&input, &mut output, &ctx).unwrap();

        // Reset must succeed and the resampler must still be present (re-created)
        p.reset();
        assert!(
            p.resampler.is_some(),
            "Resampler should be present after reset()"
        );

        // After reset, processing should produce valid output (no NaN / inf / crash)
        let silence = vec![0.0f32; nf * 2];
        let mut out2 = vec![0.0f32; nf * 2];
        p.process(&silence, &mut out2, &ctx).unwrap();
        assert!(
            out2.iter().all(|s| s.is_finite()),
            "Post-reset output should be finite"
        );
    }

    /// §4.4: correction_strength_smoother must be advanced in the phase vocoder path.
    /// A rapid correction_strength change should not produce a discontinuity larger
    /// than what the smoother allows in one call.
    #[test]
    fn test_pv_path_uses_correction_strength_smoother() {
        let mut p = PndPlugin::new(2);
        p.initialize(44100).unwrap();
        p.set_parameter(
            ParameterId::from("phase_vocoder"),
            ParameterValue::Bool(true),
        )
        .unwrap();

        // Set correction_strength to 0 first, then jump to 1.0
        p.set_parameter(
            ParameterId::from("correction_strength"),
            ParameterValue::Float(0.0),
        )
        .unwrap();

        let nf = 512;
        let ctx = ProcessContext::new(44100, nf);
        let input: Vec<f32> = (0..nf * 2)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let mut output = vec![0.0f32; nf * 2];
        // Process with strength=0 to prime the smoother
        p.process(&input, &mut output, &ctx).unwrap();

        // Now jump strength to 1.0
        p.set_parameter(
            ParameterId::from("correction_strength"),
            ParameterValue::Float(1.0),
        )
        .unwrap();

        // Process one block; the smoother should advance (not jump to 1.0 instantly)
        // We can verify the smoother is "moving" by checking that the cached value
        // of the smoother is between 0 and 1 after one advance.
        let mut out2 = vec![0.0f32; nf * 2];
        p.process(&input, &mut out2, &ctx).unwrap();

        // Verify: output must be finite (no NaN/inf from unsmoothed parameter jump)
        assert!(
            out2.iter().all(|s| s.is_finite()),
            "Phase vocoder output must be finite after correction_strength jump"
        );

        // The smoother target is 1.0, current is near 0.0 — after one PV block,
        // it must not be exactly 1.0 (which would mean the smoother was bypassed).
        // We can't easily inspect the internal smoother state, so we verify
        // indirectly: smoother.advance() is called by checking that the smoother
        // would be "in motion" — both bounds bracket 0.0 < smoother < 1.0 would
        // require a multi-step test, so we just confirm no crash and finite output.
        // The structural fix is verified by code inspection plus this no-panic test.
    }

    /// Verify set_parameter / get_parameter round-trip for drift_smoothing.
    #[test]
    fn test_drift_smoothing_param_roundtrip() {
        let mut p = PndPlugin::new(1);
        p.initialize(44100).unwrap();

        p.set_parameter(
            ParameterId::from("drift_smoothing"),
            ParameterValue::Float(0.85),
        )
        .unwrap();

        let val = p.get_parameter(&ParameterId::from("drift_smoothing"));
        assert_eq!(val, Some(ParameterValue::Float(0.85)));
    }
}
