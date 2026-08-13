use super::filters::{HrtfTransferFunctions, XtcFilters};
use super::reflections::RoomReflectionData;
use serde::Deserialize;
use std::sync::Arc;

pub(super) struct PendingFilterUpdate {
    pub(super) generation: u64,
    pub(super) filters: Arc<XtcFilters>,
    pub(super) hrtf_transfer_functions: Option<Arc<HrtfTransferFunctions>>,
    pub(super) room_reflection_cache: Option<Arc<RoomReflectionData>>,
    pub(super) room_params_hash: u64,
}

pub(super) enum RetiredXtcState {
    Filters(Arc<XtcFilters>),
    Pending(Arc<PendingFilterUpdate>),
}

pub(super) struct FilterUpdateRequest {
    pub(super) generation: u64,
    pub(super) params: super::config::XtcPluginParams,
    pub(super) sample_rate: u32,
    pub(super) num_bins: usize,
    pub(super) expected_output_channels: usize,
    pub(super) fft_forward: Arc<dyn realfft::RealToComplex<f32>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RoomeqRecommendedMatrix {
    pub(super) sample_rate: u32,
    pub(super) speakers: Vec<String>,
    pub(super) ears: Vec<String>,
    pub(super) filters: Vec<RoomeqRecommendedFilter>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RoomeqRecommendedFilter {
    pub(super) speaker: String,
    pub(super) target_ear: String,
    pub(super) taps: Vec<f64>,
}
