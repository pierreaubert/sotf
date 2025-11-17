// ============================================================================
// Analyzer Plugin Trait
// ============================================================================
//
// Analyzer plugins process audio but don't produce audio output.
// Instead, they compute metrics/visualizations that can be read by the host.
//
// Examples: loudness monitoring, spectrum analysis, phase meters, etc.

use super::plugin::{PluginInfo, PluginResult, ProcessContext};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Trait for analyzer plugins that compute metrics without audio output
///
/// Unlike regular Plugin, AnalyzerPlugin:
/// - Takes N input channels
/// - Produces 0 output channels (no audio)
/// - Exposes computed data via get_data()
pub trait AnalyzerPlugin: Send {
    /// Get plugin information
    fn info(&self) -> PluginInfo;

    /// Get number of input channels this analyzer expects
    fn input_channels(&self) -> usize;

    /// Initialize the analyzer with a sample rate
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()>;

    /// Reset the analyzer state
    fn reset(&mut self);

    /// Process audio samples (no output, just analysis)
    ///
    /// # Arguments
    /// * `input` - Interleaved input samples
    /// * `context` - Processing context (sample rate, num frames)
    fn process(&mut self, input: &[f32], context: &ProcessContext) -> PluginResult<()>;

    /// Get current analyzer data as a trait object
    ///
    /// The returned data can be downcast to the specific data type
    /// (e.g., LoudnessInfo, SpectrumInfo)
    fn get_data(&self) -> Box<dyn Any + Send>;

    /// Get latency in samples (usually 0 for analyzers)
    fn latency_samples(&self) -> usize {
        0
    }
}

/// Common analyzer data types that can be serialized
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnalyzerData {
    /// Loudness measurements (LUFS, peaks)
    Loudness(LoudnessData),
    /// Spectrum measurements (frequency bins)
    Spectrum(SpectrumData),
}

/// Loudness analyzer data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessData {
    /// Momentary loudness (M) - 400ms window, LUFS
    pub momentary_lufs: f64,
    /// Short-term loudness (S) - 3 second window, LUFS
    pub shortterm_lufs: f64,
    /// Current sample peak (0.0 to 1.0+)
    pub peak: f64,
    /// Per-channel peaks (0.0 to 1.0+)
    pub channel_peaks: Vec<f64>,
}

/// Spectrum analyzer data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumData {
    /// Frequency bin centers in Hz
    pub frequencies: Vec<f32>,
    /// Magnitude values in dB
    pub magnitudes: Vec<f32>,
    /// Peak magnitude across all bins
    pub peak_magnitude: f32,
}
