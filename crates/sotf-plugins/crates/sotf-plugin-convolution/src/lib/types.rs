use super::default::default_head_taps;
use super::default::default_use_nupc;
use plugins_spatial::nupc;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Result of loading an IR on a background thread, ready to be swapped into the audio thread.
pub(super) struct IrLoadResult {
    pub(super) state: ConvolutionState,
    pub(super) nupc_engines: Vec<nupc::NupcEngine>,
    pub(super) fdl_flat: Vec<Complex<f32>>,
    pub(super) fdl_head: usize,
    pub(super) fft_scratch: Vec<Complex<f32>>,
    pub(super) rayon_accum_pool: Vec<Vec<Complex<f32>>>,
    pub(super) ir_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvolutionPluginParams {
    pub ir_file: String,
    pub mix: f32,
    pub gain_db: f32,
    /// Use Non-Uniform Partitioned Convolution for long IRs
    #[serde(default = "default_use_nupc")]
    pub use_nupc: bool,
    #[serde(default)]
    pub zero_latency_head: bool,
    #[serde(default = "default_head_taps")]
    pub head_taps: usize,
}

pub(super) struct ConvolutionState {
    pub(super) partitions: Vec<Vec<Vec<Complex<f32>>>>, // [channel][partition][bin]
    pub(super) num_partitions: usize,
    pub(super) ir_channels: usize,
    pub(super) fft_forward: Arc<dyn rustfft::Fft<f32>>,
    pub(super) fft_inverse: Arc<dyn rustfft::Fft<f32>>,
}
