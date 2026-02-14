// ============================================================================
// PND (Polyphonic Note Detection & Varispeed) Plugin
// ============================================================================

use super::param_specs::pnd::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
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

// ============================================================================
// Exposed Data Structure
// ============================================================================

/// Data exposed by the PND plugin for drift monitoring.
#[derive(Debug, Clone)]
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
    mono_buffer: Vec<f32>,
    planar_input: Vec<Vec<f32>>,
    planar_output: Vec<Vec<f32>>,

    // Block buffering for arbitrary host block sizes
    input_ring: Vec<f32>, // Interleaved input accumulator
    input_ring_fill: usize,
    output_ring: Vec<f32>, // Interleaved output drain buffer
    output_ring_fill: usize,
    output_ring_read_pos: usize,

    // Parameters
    param_correction_strength: ParameterId,
    correction_strength: f32,

    param_analysis_window_ms: ParameterId,
    analysis_window_ms: f32,

    param_drift_smoothing: ParameterId,
    drift_smoothing: f32,
}

impl PndPlugin {
    pub fn new(channels: usize) -> Self {
        Self {
            channels,
            sample_rate: 44100, // Default, updated in initialize
            analyzer: None,
            resampler: None,
            current_ratio: 1.0,
            last_drift_ratio: 1.0,
            mono_buffer: Vec::new(),
            planar_input: vec![Vec::new(); channels],
            planar_output: Vec::new(),
            input_ring: Vec::new(),
            input_ring_fill: 0,
            output_ring: Vec::new(),
            output_ring_fill: 0,
            output_ring_read_pos: 0,

            param_correction_strength: ParameterId::from("correction_strength"),
            correction_strength: CORRECTION_STRENGTH_DEFAULT,

            param_analysis_window_ms: ParameterId::from("analysis_window_ms"),
            analysis_window_ms: ANALYSIS_WINDOW_MS_DEFAULT,

            param_drift_smoothing: ParameterId::from("drift_smoothing"),
            drift_smoothing: DRIFT_SMOOTHING_DEFAULT,
        }
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

        // 1. Analyze input for drift (mono/first channel)
        if self.mono_buffer.len() < chunk_frames {
            self.mono_buffer.resize(chunk_frames, 0.0);
        }
        for i in 0..chunk_frames {
            self.mono_buffer[i] = self.input_ring[i * self.channels];
        }

        let drift_ratio = if let Some(analyzer) = &mut self.analyzer {
            analyzer.analyze(&self.mono_buffer[..chunk_frames])
        } else {
            1.0
        };
        self.last_drift_ratio = drift_ratio as f64;

        // 2. Calculate Correction Ratio
        let target_correction = 1.0 / drift_ratio as f64;
        let alpha = self.drift_smoothing as f64;
        self.current_ratio = self.current_ratio * (1.0 - alpha) + target_correction * alpha;

        let strength = self.correction_strength as f64;
        let final_ratio = 1.0 + (self.current_ratio - 1.0) * strength;

        resampler
            .set_resample_ratio(final_ratio, true)
            .map_err(|e| format!("{:?}", e))?;

        // 3. De-interleave input into planar buffers
        for c in 0..self.channels {
            if self.planar_input[c].len() < chunk_frames {
                self.planar_input[c].resize(chunk_frames, 0.0);
            }
        }
        for i in 0..chunk_frames {
            for c in 0..self.channels {
                self.planar_input[c][i] = self.input_ring[i * self.channels + c];
            }
        }

        // Shift consumed samples out of input ring
        let consumed_samples = chunk_frames * self.channels;
        self.input_ring.copy_within(consumed_samples.., 0);
        self.input_ring_fill -= consumed_samples;

        // 4. Resample
        let max_output_frames = resampler.output_frames_max();
        for c in 0..self.channels {
            if self.planar_output[c].len() < max_output_frames {
                self.planar_output[c].resize(max_output_frames, 0.0);
            }
        }

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

        // 5. Re-interleave into output ring
        let needed = self.output_ring_fill + out_written * self.channels;
        if self.output_ring.len() < needed {
            self.output_ring.resize(needed, 0.0);
        }
        for i in 0..out_written {
            for c in 0..self.channels {
                self.output_ring[self.output_ring_fill + i * self.channels + c] =
                    self.planar_output[c][i];
            }
        }
        self.output_ring_fill += out_written * self.channels;

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
        vec![
            Parameter::new_float(
                "correction_strength",
                "Correction Strength",
                CORRECTION_STRENGTH_DEFAULT,
                CORRECTION_STRENGTH_MIN,
                CORRECTION_STRENGTH_MAX,
            )
            .with_group("Correction")
            .with_importance(ParameterImportance::Critical),
            Parameter::new_float(
                "analysis_window_ms",
                "Analysis Window (ms)",
                ANALYSIS_WINDOW_MS_DEFAULT,
                ANALYSIS_WINDOW_MS_MIN,
                ANALYSIS_WINDOW_MS_MAX,
            )
            .with_group("Analysis")
            .with_importance(ParameterImportance::FineTuning),
            Parameter::new_float(
                "drift_smoothing",
                "Drift Smoothing",
                DRIFT_SMOOTHING_DEFAULT,
                DRIFT_SMOOTHING_MIN,
                DRIFT_SMOOTHING_MAX,
            )
            .with_group("Correction")
            .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_correction_strength {
            self.correction_strength = value.as_float().ok_or("Invalid value")?;
        } else if id == self.param_analysis_window_ms {
            self.analysis_window_ms = value.as_float().ok_or("Invalid value")?;
            if let Some(analyzer) = &mut self.analyzer {
                analyzer.update_analysis_window(self.analysis_window_ms);
            }
        } else if id == self.param_drift_smoothing {
            self.drift_smoothing = value.as_float().ok_or("Invalid value")?;
        }
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

        // Pre-allocate buffers
        self.mono_buffer.resize(RESAMPLER_CHUNK_SIZE, 0.0);
        self.planar_input = vec![vec![0.0; RESAMPLER_CHUNK_SIZE]; self.channels];

        // Pre-allocate planar output (resampler may produce up to ~1.1x input frames)
        let max_output_frames = if let Some(ref resampler) = self.resampler {
            resampler.output_frames_max()
        } else {
            (RESAMPLER_CHUNK_SIZE as f64 * 1.1) as usize + 16
        };
        self.planar_output = vec![vec![0.0; max_output_frames]; self.channels];

        // Allocate block buffering rings
        // Input ring: hold up to 2 resampler chunks worth of interleaved samples
        let input_ring_capacity = RESAMPLER_CHUNK_SIZE * self.channels * 2;
        self.input_ring = vec![0.0; input_ring_capacity];
        self.input_ring_fill = 0;

        // Output ring: hold up to 2 chunks worth of resampled output
        let output_ring_capacity = max_output_frames * self.channels * 2;
        self.output_ring = vec![0.0; output_ring_capacity];
        self.output_ring_fill = 0;
        self.output_ring_read_pos = 0;

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

        // 1. Accumulate input into input ring
        let space_available = self.input_ring.len() - self.input_ring_fill;
        let samples_to_copy = total_input_samples.min(space_available);
        self.input_ring[self.input_ring_fill..self.input_ring_fill + samples_to_copy]
            .copy_from_slice(&input[..samples_to_copy]);
        self.input_ring_fill += samples_to_copy;

        // 2. Process complete resampler chunks
        if let Some(resampler) = &self.resampler {
            let chunk_samples = resampler.input_frames_next() * self.channels;
            while self.input_ring_fill >= chunk_samples {
                self.process_one_chunk()?;
            }
        }

        // 3. Drain output ring to output buffer
        let total_output_samples = num_frames * self.channels;
        let available = self.output_ring_fill - self.output_ring_read_pos;
        let drain_samples = available.min(total_output_samples);
        let drain_frames = drain_samples / self.channels;

        output[..drain_samples].copy_from_slice(
            &self.output_ring[self.output_ring_read_pos..self.output_ring_read_pos + drain_samples],
        );
        self.output_ring_read_pos += drain_samples;

        // Compact output ring when fully drained
        if self.output_ring_read_pos > 0 {
            self.output_ring
                .copy_within(self.output_ring_read_pos..self.output_ring_fill, 0);
            self.output_ring_fill -= self.output_ring_read_pos;
            self.output_ring_read_pos = 0;
        }

        // Zero remaining output if not enough data (initial latency period)
        if drain_frames < num_frames {
            let zero_start = drain_frames * self.channels;
            output[zero_start..total_output_samples].fill(0.0);
        }

        // Report num_frames to prevent ring buffer underruns in host
        Ok(context.num_frames)
    }

    fn reset(&mut self) {
        if let Some(analyzer) = &mut self.analyzer {
            analyzer.reset();
        }
        self.current_ratio = 1.0;
        self.last_drift_ratio = 1.0;
        self.input_ring_fill = 0;
        self.output_ring_fill = 0;
        self.output_ring_read_pos = 0;
    }

    fn latency_samples(&self) -> usize {
        RESAMPLER_CHUNK_SIZE
    }

    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        let (confidence, matched_partials, total_peaks) = if let Some(analyzer) = &self.analyzer {
            (
                analyzer.confidence(),
                analyzer.matched_partials(),
                analyzer.total_peaks(),
            )
        } else {
            (0.0, 0, 0)
        };

        Some(Arc::new(PndData {
            drift_ratio: self.last_drift_ratio,
            correction_ratio: self.current_ratio,
            confidence,
            matched_partials,
            total_peaks,
        }))
    }
}
