//! AutoEQ optimization modules
pub mod headphone_eq;
pub mod params;

// Re-export commonly used types
pub use params::{
    ALGORITHM_OPTIONS, CURVE_NAME_OPTIONS, DE_STRATEGY_OPTIONS, HEADPHONE_LOSS_OPTIONS,
    LOCAL_ALGO_OPTIONS, OptimizationParams, PEQ_MODEL_OPTIONS, ParamLimits, SPEAKER_LOSS_OPTIONS,
};
