//! AutoEQ optimization modules
pub mod headphone_eq;
pub mod params;
pub mod result_graphs;
pub mod speaker_eq;
pub mod spinorama_calculation;


// Re-export commonly used types
pub use headphone_eq::HeadphoneOptimizationResult;
pub use params::{
    ALGORITHM_OPTIONS, CURVE_NAME_OPTIONS, DE_STRATEGY_OPTIONS, EQ_EXPORT_FORMAT_OPTIONS,
    HEADPHONE_LOSS_OPTIONS, LOCAL_ALGO_OPTIONS, OptimizationParams, PEQ_MODEL_OPTIONS, ParamLimits,
    SPEAKER_LOSS_OPTIONS, get_export_extension,
};
