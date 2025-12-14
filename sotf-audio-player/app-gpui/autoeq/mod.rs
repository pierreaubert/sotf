//! AutoEQ optimization modules for GPUI
//!
//! This module provides GPUI-specific wrappers around the common library's
//! AutoEQ optimization functionality.

pub mod headphone_eq;
pub mod result_graphs;
pub mod speaker_eq;
pub mod spinorama_calculation;

// Re-export types from the common library for convenience
pub use sotf_audio_player::autoeq::{
    get_export_extension, HeadphoneOptimizationResult, OptimizationParams, ParamLimits,
    SpeakerOptimizationResult, ALGORITHM_OPTIONS, CURVE_NAME_OPTIONS, DE_STRATEGY_OPTIONS,
    EQ_EXPORT_FORMAT_OPTIONS, HEADPHONE_LOSS_OPTIONS, LOCAL_ALGO_OPTIONS, PEQ_MODEL_OPTIONS,
    SPEAKER_LOSS_OPTIONS,
};
