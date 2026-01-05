// ============================================================================
// PND (Polyphonic Note Detection & Varispeed) Plugin
// ============================================================================

use super::param_specs::pnd::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use rubato::{FastFixedIn, PolynomialDegree, Resampler};

mod analysis;
mod config;

use analysis::PndAnalyzer;
pub use config::PndPluginParams;

pub struct PndPlugin {
    // Configuration
    channels: usize,
    sample_rate: u32,

    // Components
    analyzer: Option<PndAnalyzer>,
    resampler: Option<FastFixedIn<f32>>,

    // State
    current_ratio: f64,

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
        let chunk_size = 1024; // Standard block size
        let resampler = FastFixedIn::<f32>::new(
            1.0, // Initial ratio
            1.1, // Max ratio
            PolynomialDegree::Cubic,
            chunk_size,
            self.channels,
        )
        .map_err(|e| format!("Failed to create resampler: {:?}", e))?;

        self.resampler = Some(resampler);
        Ok(())
    }

    fn init_analyzer(&mut self) {
        let fft_size = 2048; // Good balance for freq resolution
        self.analyzer = Some(PndAnalyzer::new(fft_size, self.sample_rate));
    }
}

impl Plugin for PndPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("PND Varispeed", "0.1.0", "SotF")
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

        Ok(())
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;

        // 1. Analyze input for drift
        // For simplicity in this prototype, we analyze the current block (channel 0)
        // Ideally we would look at a window centered on the current block or lookahead
        let drift_ratio = if let Some(analyzer) = &mut self.analyzer {
            // Interleaved to mono/first channel for analysis
            let mono_input: Vec<f32> = input.chunks(self.channels).map(|chunk| chunk[0]).collect();
            analyzer.analyze(&mono_input)
        } else {
            1.0
        };

        // 2. Calculate Correction Ratio
        // If drift_ratio > 1.0 (speed up), we need to slow down (ratio < 1.0)
        // correction = 1 / drift
        let target_correction = 1.0 / drift_ratio as f64;

        // Apply smoothing
        let alpha = self.drift_smoothing as f64;
        self.current_ratio = self.current_ratio * (1.0 - alpha) + target_correction * alpha;

        // Apply strength
        let strength = self.correction_strength as f64;
        let final_ratio = 1.0 + (self.current_ratio - 1.0) * strength;

        // 3. Resample
        // We use rubato to process.
        // Note: Rubato FastFixedIn expects fixed input size (chunk_size).
        // We need to handle arbitrary block sizes from the host.
        // For this MVP, we assume the host block size matches rubato's chunk size (1024) or we buffer.
        // Buffering logic is complex.
        // Here we just panic if block size doesn't match for the MVP,
        // or we bypass if mismatched (safety).

        if let Some(resampler) = &mut self.resampler {
            if num_frames != resampler.input_frames_next() {
                // Pass through if block size mismatch
                output.copy_from_slice(input);
                return Ok(());
            }

            resampler
                .set_resample_ratio(final_ratio, true)
                .map_err(|e| format!("{:?}", e))?;

            // De-interleave
            let mut planar_input = vec![vec![0.0; num_frames]; self.channels];
            for i in 0..num_frames {
                for c in 0..self.channels {
                    planar_input[c][i] = input[i * self.channels + c];
                }
            }

            let planar_output = resampler
                .process(&planar_input, None)
                .map_err(|e| format!("{:?}", e))?;

            // Re-interleave
            // Note: Output size might differ from input size due to resampling!
            // The host expects `num_frames` output.
            // If we produce more/less, we have a problem in a synchronous plugin API.
            // A true varispeed plugin usually requests data from a ringbuffer.
            //
            // For this implementation, we will write as much as fits into the output buffer,
            // which works if we are just "effecting" the audio, but for strict timing synchronization
            // this requires a different host architecture (pull-based with variable rate).

            let out_frames = planar_output[0].len().min(num_frames);

            for i in 0..out_frames {
                for c in 0..self.channels {
                    output[i * self.channels + c] = planar_output[c][i];
                }
            }

            // Zero fill rest if we shrank
            if out_frames < num_frames {
                for i in out_frames..num_frames {
                    for c in 0..self.channels {
                        output[i * self.channels + c] = 0.0;
                    }
                }
            }
        }

        Ok(())
    }
}
