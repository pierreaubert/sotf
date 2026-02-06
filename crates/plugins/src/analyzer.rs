//! ============================================================================
//! Analyzer Plugin Trait
//! ============================================================================
//!
//! Analyzer plugins process audio but don't produce audio output.
//! Instead, they compute metrics/visualizations that can be read by the host.
//!
//! Examples: loudness monitoring, spectrum analysis, phase meters, etc.

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
    /// Integrated loudness (I) - whole program loudness, LUFS
    pub integrated_lufs: f64,
    /// Current sample peak (0.0 to 1.0+)
    pub peak: f64,
    /// Per-channel sample peaks (0.0 to 1.0+)
    pub channel_peaks: Vec<f64>,
    /// Per-channel true peaks in dBTP (dB True Peak)
    /// True peaks account for inter-sample peaks via oversampling
    pub true_peaks_dbtp: Vec<f64>,
    /// L/R correlation coefficient (ICC - Inter-Channel Correlation)
    /// Only valid for stereo signals (2 channels)
    /// Range: -1.0 (anti-correlated) to +1.0 (fully correlated)
    /// None if not stereo or not enough data
    pub correlation_lr: Option<f64>,
}

impl LoudnessData {
    pub fn new(channels: usize) -> Self {
        Self {
            momentary_lufs: f64::NEG_INFINITY,
            shortterm_lufs: f64::NEG_INFINITY,
            integrated_lufs: f64::NEG_INFINITY,
            peak: 0.0,
            channel_peaks: vec![0.0; channels],
            true_peaks_dbtp: vec![f64::NEG_INFINITY; channels],
            correlation_lr: None,
        }
    }

    /// Update all fields in-place from another LoudnessData (zero allocation)
    pub fn update_from(&mut self, other: &LoudnessData) {
        self.momentary_lufs = other.momentary_lufs;
        self.shortterm_lufs = other.shortterm_lufs;
        self.integrated_lufs = other.integrated_lufs;
        self.peak = other.peak;
        self.channel_peaks.clear();
        self.channel_peaks.extend_from_slice(&other.channel_peaks);
        self.true_peaks_dbtp.clear();
        self.true_peaks_dbtp.extend_from_slice(&other.true_peaks_dbtp);
        self.correlation_lr = other.correlation_lr;
    }
}

impl Default for LoudnessData {
    fn default() -> Self {
        Self {
            momentary_lufs: f64::NEG_INFINITY,
            shortterm_lufs: f64::NEG_INFINITY,
            integrated_lufs: f64::NEG_INFINITY,
            peak: 0.0,
            channel_peaks: Vec::new(),
            true_peaks_dbtp: Vec::new(),
            correlation_lr: None,
        }
    }
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

impl SpectrumData {
    /// Update all fields in-place from another SpectrumData (zero allocation
    /// when Vec lengths match, which is the common case).
    pub fn update_from(&mut self, other: &SpectrumData) {
        self.frequencies.clear();
        self.frequencies.extend_from_slice(&other.frequencies);
        self.magnitudes.clear();
        self.magnitudes.extend_from_slice(&other.magnitudes);
        self.peak_magnitude = other.peak_magnitude;
    }
}
