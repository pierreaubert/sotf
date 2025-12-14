//! AutoEQ optimization modules
//!
//! This module provides:
//! - Room EQ optimization for multi-channel speaker systems
//! - Headphone EQ optimization with target curve matching
//! - Speaker EQ optimization with spinorama analysis
//!
//! # Architecture
//!
//! The module is organized into:
//! - [`types`] - Type definitions for room EQ configuration, results, and DSP output
//! - [`optimizer`] - High-level room EQ optimizer API
//! - [`output`] - DSP chain generation for room EQ
//! - [`params`] - Shared optimization parameter types and constants
//! - [`headphone`] - Headphone EQ optimization logic and result types
//! - [`speaker`] - Speaker EQ optimization logic and result types
//! - [`spinorama`] - Spinorama room correction calculations
//!
//! # Usage
//!
//! ```ignore
//! use sotf_audio_player::autoeq::{
//!     RoomEqOptimizer, OptimizerConfig, ChannelMeasurements,
//!     OptimizationParams, HeadphoneOptimizationResult, SpeakerOptimizationResult,
//! };
//!
//! // Create room EQ optimizer with default config
//! let optimizer = RoomEqOptimizer::with_defaults();
//!
//! // Run headphone optimization
//! let params = OptimizationParams::headphone_defaults();
//! let result = run_headphone_optimization(
//!     "measurement.csv",
//!     "harman-over-ear-2018",
//!     "",
//!     &params,
//!     "json",
//! )?;
//! ```

mod optimizer;
mod output;
pub mod types;

// New modules
pub mod headphone;
pub mod params;
pub mod speaker;
pub mod spinorama;

// Re-export room EQ types and functions
pub use optimizer::{run_optimization_task, RoomEqOptimizer};
pub use output::{load_dsp_chain, save_dsp_chain};
pub use types::{
    Algorithm, ChannelConfig, ChannelDspChain, ChannelMeasurements, ChannelOptimizationResult,
    ChannelOptStatus, CrossoverType, Curve, DspChainOutput, DspPluginConfig, DriverDspChain,
    EqFilterResult, Measurement, OptimizationMetadata, OptimizationProgress, OptimizerConfig,
    SpeakerConfigType,
};

// Re-export params types
pub use params::{
    get_export_extension, OptimizationParams, ParamLimits, ALGORITHM_OPTIONS, CURVE_NAME_OPTIONS,
    DE_STRATEGY_OPTIONS, EQ_EXPORT_FORMAT_OPTIONS, HEADPHONE_LOSS_OPTIONS, LOCAL_ALGO_OPTIONS,
    PEQ_MODEL_OPTIONS, SPEAKER_LOSS_OPTIONS,
};

// Re-export headphone types
pub use headphone::{
    load_target_curve, parse_csv_curve, run_headphone_optimization, target_curves,
    HeadphoneOptimizationResult,
};

// Re-export speaker types
pub use speaker::{run_speaker_optimization, SpeakerOptimizationResult};

// Re-export spinorama types
pub use spinorama::{
    calculate_room_correction, MeasurementCurve, RoomCorrectionInput, RoomMeasurement,
};
