// ============================================================================
// PND (Polyphonic Note Detection & Varispeed) Plugin
// ============================================================================

use sotf_host::analyzer::RealTimeCache;
use sotf_host::param_specs::{find_by_key as pk, pnd::PARAMS as PD};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{deinterleave_stereo, interleave_stereo};
use sotf_host::smoothing::Smoother;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};
use std::any::Any;
use std::sync::Arc;

mod analysis;
mod config;

use analysis::PndAnalyzer;
pub use config::PndPluginParams;

/// Resampler chunk size — the fixed input block size expected by rubato.
const RESAMPLER_CHUNK_SIZE: usize = 1024;

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

    // Components
    analyzer: Option<PndAnalyzer>,
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

    // Parameters
    param_correction_strength: ParameterId,
    correction_strength: f32,
    correction_strength_smoother: Smoother,

    param_analysis_window_ms: ParameterId,
    analysis_window_ms: f32,

    param_drift_smoothing: ParameterId,
    drift_smoothing: f32,

    cache: RealTimeCache<PndData>,
    cache_update_counter: usize,
    cached_parameters: Vec<Parameter>,
}

impl PndPlugin {
    pub fn new(channels: usize) -> Self {
        let mut p = Self {
            channels,
            sample_rate: 44100, // Default, updated in initialize
            analyzer: None,
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
        ];
    }

    pub fn from_params(channels: usize, params: PndPluginParams) -> Self {
        let mut plugin = Self::new(channels);
        plugin.correction_strength = params.correction_strength;
        plugin.analysis_window_ms = params.analysis_window_ms;
        plugin.drift_smoothing = params.drift_smoothing;
        plugin
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

    fn init_analyzer(&mut self) {
        let fft_size = 2048; // Good balance for freq resolution
        self.analyzer = Some(PndAnalyzer::new(
            fft_size,
            self.sample_rate,
            self.analysis_window_ms,
        ));
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

        // 3. Analyze first channel for drift
        let drift_ratio = if let Some(analyzer) = &mut self.analyzer {
            analyzer.analyze(&self.planar_input[0][..chunk_frames])
        } else {
            1.0
        };
        self.last_drift_ratio = drift_ratio as f64;

        // 4. Calculate correction ratio
        let target_correction = 1.0 / drift_ratio as f64;
        let alpha = self.drift_smoothing as f64;
        self.current_ratio = self.current_ratio * (1.0 - alpha) + target_correction * alpha;

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
        debug_assert!(
            self.output_ring_count + out_samples <= cap_out,
            "output_ring overflow"
        );

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
        PluginInfo::new("PND Varispeed", "0.2.0", "SotF")
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
            let v = value.as_float().unwrap_or(pk(PD, "correction_strength").default_f64() as f32);
            if v.is_finite() {
                self.correction_strength = v;
                self.correction_strength_smoother
                    .set_target(self.correction_strength);
            }
        } else if id == self.param_analysis_window_ms {
            let v = value.as_float().unwrap_or(pk(PD, "analysis_window_ms").default_f64() as f32);
            if v.is_finite() {
                self.analysis_window_ms = v;
                if let Some(analyzer) = &mut self.analyzer {
                    analyzer.update_analysis_window(self.analysis_window_ms);
                }
            }
        } else if id == self.param_drift_smoothing {
            let v = value.as_float().unwrap_or(pk(PD, "drift_smoothing").default_f64() as f32);
            if v.is_finite() {
                self.drift_smoothing = v;
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
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.init_resampler()?;
        self.init_analyzer();

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

        Ok(())
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        let num_frames = context.num_frames;
        let total_input_samples = num_frames * self.channels;

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
            let (confidence, matched_partials, total_peaks) = if let Some(analyzer) = &self.analyzer
            {
                (
                    analyzer.confidence(),
                    analyzer.matched_partials(),
                    analyzer.total_peaks(),
                )
            } else {
                (0.0, 0, 0)
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
        if let Some(analyzer) = &mut self.analyzer {
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
    }

    fn latency_samples(&self) -> usize {
        RESAMPLER_CHUNK_SIZE
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.cache.load() as Arc<dyn Any + Send + Sync>)
    }
}
