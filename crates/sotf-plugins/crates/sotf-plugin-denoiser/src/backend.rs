// ============================================================================
// Denoiser Backend Trait
// ============================================================================
//
// Defines the interface for pluggable denoising algorithm backends.
// The DenoiserPlugin delegates to the active backend for processing.

use crate::DenoiserData;

/// Available denoising algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenoiserAlgorithm {
    /// Current MCRA + Wiener (default, no extra deps)
    Classical,
    /// nnnoiseless — lightweight RNN-based, speech-only, 10ms latency
    RNNoise,
    /// DeepFilterNet3 — premium neural denoiser, needs ONNX models
    DeepFilter,
    /// Classical + neural post-filter for artifact suppression
    HybridNeural,
}

impl std::fmt::Display for DenoiserAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Classical => write!(f, "Classical"),
            Self::RNNoise => write!(f, "RNNoise"),
            Self::DeepFilter => write!(f, "DeepFilter"),
            Self::HybridNeural => write!(f, "HybridNeural"),
        }
    }
}

/// Trait for denoising algorithm backends.
///
/// Each backend implements a specific denoising algorithm that can be
/// hot-swapped at runtime via the `algorithm` parameter.
pub trait DenoiserBackend: Send {
    /// Initialize with sample rate and channel count.
    fn initialize(&mut self, sample_rate: u32, channels: usize);

    /// Process one block of interleaved audio in-place.
    ///
    /// # Arguments
    /// * `buffer` - Interleaved audio samples [ch0_f0, ch1_f0, ch0_f1, ...]
    /// * `num_frames` - Number of frames in the buffer
    /// * `channels` - Number of channels
    fn process(&mut self, buffer: &mut [f32], num_frames: usize, channels: usize);

    /// Reset internal state (called on seek, track change, etc.)
    fn reset(&mut self);

    /// Get processing latency in samples.
    fn latency_samples(&self) -> usize;

    /// Get monitoring data for UI display.
    fn get_data(&self) -> DenoiserData;

    /// Get the algorithm type.
    fn algorithm(&self) -> DenoiserAlgorithm;
}
