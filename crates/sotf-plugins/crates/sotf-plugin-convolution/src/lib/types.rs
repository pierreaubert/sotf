use super::default::default_head_taps;
use super::default::default_use_nupc;
use plugins_spatial::nupc;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Result of loading an IR on a background thread, ready to be swapped into the audio thread.
pub(super) struct IrLoadResult {
    pub(super) state: Arc<Option<ConvolutionState>>,
    pub(super) nupc_engines: Vec<nupc::NupcEngine>,
    pub(super) fdl_flat: Vec<Complex<f32>>,
    pub(super) fdl_head: usize,
    pub(super) fft_scratch: Vec<Complex<f32>>,
    pub(super) rayon_accum_pool: Vec<Vec<Complex<f32>>>,
    pub(super) ir_file: String,
}

pub(super) struct IrLoadCompletion {
    pub(super) generation: u64,
    pub(super) result: Result<IrLoadResult, String>,
}

pub(super) struct RetiredIrState {
    pub(super) state: Arc<Option<ConvolutionState>>,
    pub(super) nupc_engines: Vec<nupc::NupcEngine>,
    pub(super) fdl_flat: Vec<Complex<f32>>,
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

impl Default for ConvolutionPluginParams {
    fn default() -> Self {
        let params = crate::params::PARAMS;
        Self {
            ir_file: String::new(),
            mix: params[1].default_f32(),
            gain_db: params[2].default_f32(),
            use_nupc: params[3].default_bool(),
            zero_latency_head: params[4].default_bool(),
            head_taps: params[5].default_usize(),
        }
    }
}

pub(super) struct ConvolutionState {
    pub(super) partitions: Vec<Vec<Vec<Complex<f32>>>>, // [channel][partition][bin]
    pub(super) num_partitions: usize,
    pub(super) ir_channels: usize,
    /// UPC-only plans. NUPC construction leaves these empty so the two
    /// backends do not duplicate immutable IR spectra and FFT plans.
    pub(super) fft_forward: Option<Arc<dyn rustfft::Fft<f32>>>,
    pub(super) fft_inverse: Option<Arc<dyn rustfft::Fft<f32>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConvolutionLoadStatus {
    Idle = 0,
    Loading = 1,
    Ready = 2,
    Failed = 3,
}
