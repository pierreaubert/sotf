//! AutoEQ optimization modules
//!
//! This module provides thin wrappers around the `autoeq` library for:
//! - Headphone EQ optimization with target curve matching
//! - Speaker EQ optimization with spinorama analysis
//!
//! # Architecture
//!
//! Most functionality is delegated to the `autoeq` library. This module provides:
//! - [`types`] - CrossoverType and SpeakerConfigType
//! - [`params`] - UI helpers (dropdown options, parameter limits)
//! - [`headphone`] - Headphone EQ optimization entry point
//! - [`speaker`] - Speaker EQ optimization entry point
//!
//! # Usage
//!
//! ```ignore
//! use sotf_audio_player::autoeq::{
//!     run_speaker_optimization, run_headphone_optimization,
//! };
//!
//! // Use library defaults
//! let args = autoeq::Args::speaker_defaults();
//! let result = run_speaker_optimization("KEF R3", &args)?;
//!
//! // Or for headphone
//! let args = autoeq::Args::headphone_defaults();
//! let result = run_headphone_optimization(
//!     "measurement.csv",
//!     "harman-over-ear-2018",
//!     "",
//!     &args,
//!     "json",
//! )?;
//! ```

pub mod types;

// Modules
pub mod headphone;
pub mod params;
pub mod speaker;

// Re-export types
pub use types::{CrossoverType, SpeakerConfigType};

// Re-export params types
pub use params::{
    OptimizationParams, OptimizationParamsSerializable, ParamLimits,
    ALGORITHM_OPTIONS, CURVE_NAME_OPTIONS, DE_STRATEGY_OPTIONS, EQ_EXPORT_FORMAT_OPTIONS,
    HEADPHONE_LOSS_OPTIONS, LOCAL_ALGO_OPTIONS, PEQ_MODEL_OPTIONS, SPEAKER_LOSS_OPTIONS,
    get_export_extension, parse_loss_type, parse_peq_model,
};

// Re-export headphone types
pub use headphone::{
    HeadphoneOptResult, HeadphoneOptimizationResult, VisualizationCurves,
    load_target_curve, parse_csv_curve, run_headphone_optimization, target_curves,
};

// Re-export speaker types
pub use speaker::{
    CallbackAction, CallbackConfig, Cea2034Data, MeasurementInput, OptimizationOutput,
    OptimizationStage, PreviewCurves, ProgressCallbackConfig, ProgressUpdate,
    SpeakerConfigTypeExt, SpeakerOptResult, SpeakerOptimizationCallback,
    SpeakerOptimizationConfig, SpeakerOptimizationConfigExt, SpeakerOptimizationProgress,
    SpeakerOptimizationResult, load_preview_curves, load_preview_curves_async,
    run_speaker_optimization, run_speaker_optimization_extended,
    run_speaker_optimization_with_callback,
};
