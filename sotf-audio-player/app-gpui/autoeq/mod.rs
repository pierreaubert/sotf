//! AutoEQ optimization modules
pub mod params;
pub mod headphone_eq;

// Re-export commonly used types
pub use params::{
    OptimizationParams, ParamLimits,
    ALGORITHM_OPTIONS, DE_STRATEGY_OPTIONS, PEQ_MODEL_OPTIONS,
    SPEAKER_LOSS_OPTIONS, HEADPHONE_LOSS_OPTIONS, CURVE_NAME_OPTIONS, LOCAL_ALGO_OPTIONS,
};
