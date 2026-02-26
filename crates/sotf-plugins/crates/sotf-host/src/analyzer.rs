//! ============================================================================
//! Analyzer Plugin Trait
//! ============================================================================
//!
//! Analyzer plugins process audio but don't produce audio output.
//! Instead, they compute metrics/visualizations that can be read by the host.
//!
//! Examples: loudness monitoring, spectrum analysis, phase meters, etc.

use crate::plugin::{PluginInfo, PluginResult, ProcessContext};
use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;

/// A real-time safe cache for data of type T.
///
/// Uses two Arcs to allow the audio thread to update data in-place
/// if the UI thread is not holding a reference to the previous version.
pub struct RealTimeCache<T> {
    shared: Arc<ArcSwap<T>>,
    spare: Option<Arc<T>>,
}

impl<T: Clone + Default + Send + Sync> RealTimeCache<T> {
    /// Create a new cache with initial data
    pub fn new(initial: T) -> Self {
        let arc = Arc::new(initial);
        Self {
            shared: Arc::new(ArcSwap::from(arc.clone())),
            spare: Some(arc),
        }
    }

    /// Update the cached data using a closure.
    ///
    /// The closure receives a mutable reference to the data.
    /// If possible, the update is performed in-place on a spare Arc.
    /// If the spare Arc is still in use by another thread, a new one is allocated.
    pub fn update<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut T),
    {
        // 1. Try to get a spare Arc we can mutate
        let data_arc = if let Some(mut spare) = self.spare.take() {
            if let Some(data) = Arc::get_mut(&mut spare) {
                // Sole owner - we can mutate in place!
                update_fn(data);
                spare
            } else {
                // Someone else (UI thread) is still holding this Arc.
                // Fallback: clone the data and allocate a new Arc.
                // This is the non-RT-safe path, but should be rare.
                let mut data = (**self.shared.load()).clone();
                update_fn(&mut data);
                Arc::new(data)
            }
        } else {
            // No spare available (shouldn't happen with this logic)
            let mut data = (**self.shared.load()).clone();
            update_fn(&mut data);
            Arc::new(data)
        };

        // 2. Swap the updated Arc into the shared state
        let old_arc = self.shared.swap(data_arc);

        // 3. Keep the old Arc as the next spare
        self.spare = Some(old_arc);
    }

    /// Get a handle to the shared state for reading
    pub fn shared(&self) -> Arc<ArcSwap<T>> {
        self.shared.clone()
    }

    /// Load the current data as an Arc
    pub fn load(&self) -> Arc<T> {
        self.shared.load_full()
    }
}

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
    pub channel_peaks: Arc<Vec<f64>>,
    /// Per-channel true peaks in dBTP (dB True Peak)
    /// True peaks account for inter-sample peaks via oversampling
    pub true_peaks_dbtp: Arc<Vec<f64>>,
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
            channel_peaks: Arc::new(vec![0.0; channels]),
            true_peaks_dbtp: Arc::new(vec![f64::NEG_INFINITY; channels]),
            correlation_lr: None,
        }
    }

    /// Update all fields in-place from another LoudnessData (zero allocation
    /// if Arc::get_mut succeeds on internal arrays).
    pub fn update_from(&mut self, other: &LoudnessData) {
        self.momentary_lufs = other.momentary_lufs;
        self.shortterm_lufs = other.shortterm_lufs;
        self.integrated_lufs = other.integrated_lufs;
        self.peak = other.peak;

        self.update_peaks(&other.channel_peaks);
        self.update_true_peaks(&other.true_peaks_dbtp);

        self.correlation_lr = other.correlation_lr;
    }

    /// Update channel peaks efficiently
    pub fn update_peaks(&mut self, new_peaks: &[f64]) {
        if let Some(mut_peaks) = Arc::get_mut(&mut self.channel_peaks)
            && mut_peaks.len() == new_peaks.len()
        {
            mut_peaks.copy_from_slice(new_peaks);
            return;
        }
        self.channel_peaks = Arc::new(new_peaks.to_vec());
    }

    /// Update true peaks efficiently
    pub fn update_true_peaks(&mut self, new_tps: &[f64]) {
        if let Some(mut_tps) = Arc::get_mut(&mut self.true_peaks_dbtp)
            && mut_tps.len() == new_tps.len()
        {
            mut_tps.copy_from_slice(new_tps);
            return;
        }
        self.true_peaks_dbtp = Arc::new(new_tps.to_vec());
    }
}

impl Default for LoudnessData {
    fn default() -> Self {
        Self {
            momentary_lufs: f64::NEG_INFINITY,
            shortterm_lufs: f64::NEG_INFINITY,
            integrated_lufs: f64::NEG_INFINITY,
            peak: 0.0,
            channel_peaks: Arc::new(Vec::new()),
            true_peaks_dbtp: Arc::new(Vec::new()),
            correlation_lr: None,
        }
    }
}

/// Spectrum analyzer data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumData {
    /// Frequency bin centers in Hz
    pub frequencies: Arc<Vec<f32>>,
    /// Magnitude values in dB
    pub magnitudes: Arc<Vec<f32>>,
    /// Peak magnitude across all bins
    pub peak_magnitude: f32,
}

impl SpectrumData {
    /// Update all fields in-place from another SpectrumData (zero allocation
    /// if Arc::get_mut succeeds on internal arrays).
    pub fn update_from(&mut self, other: &SpectrumData) {
        self.update_frequencies(&other.frequencies);
        self.update_magnitudes(&other.magnitudes);
        self.peak_magnitude = other.peak_magnitude;
    }

    /// Update frequencies efficiently
    pub fn update_frequencies(&mut self, new_freqs: &[f32]) {
        if let Some(mut_freqs) = Arc::get_mut(&mut self.frequencies)
            && mut_freqs.len() == new_freqs.len()
        {
            mut_freqs.copy_from_slice(new_freqs);
            return;
        }
        self.frequencies = Arc::new(new_freqs.to_vec());
    }

    /// Update magnitudes efficiently
    pub fn update_magnitudes(&mut self, new_mags: &[f32]) {
        if let Some(mut_mags) = Arc::get_mut(&mut self.magnitudes)
            && mut_mags.len() == new_mags.len()
        {
            mut_mags.copy_from_slice(new_mags);
            return;
        }
        self.magnitudes = Arc::new(new_mags.to_vec());
    }
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            frequencies: Arc::new(Vec::new()),
            magnitudes: Arc::new(Vec::new()),
            peak_magnitude: -100.0,
        }
    }
}
