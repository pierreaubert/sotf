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
pub mod apply;
pub mod headphone;
pub mod multi_speaker;
pub mod params;
pub mod presets;
pub mod speaker;

// Re-export the shared "apply Room EQ result to chain" API.
pub use apply::{
    GraphApplyOutcome, RackApplyOutcome, RoomEqApplyOutcome, apply_room_eq_graph_to_chain,
    apply_room_eq_rack_to_chain, apply_room_eq_to_chain, build_ui_graph_from_config,
    classify_channel_eq_filters, upsert_named_room_eq_plugins,
};

// Re-export types
pub use types::{CrossoverType, SpeakerConfigType};

// Re-export params types
pub use params::{
    ALGORITHM_OPTIONS, CURVE_NAME_OPTIONS, DE_STRATEGY_OPTIONS, EQ_EXPORT_FORMAT_OPTIONS,
    HEADPHONE_LOSS_OPTIONS, LOCAL_ALGO_OPTIONS, OptimizationParams, OptimizationParamsSerializable,
    PEQ_MODEL_OPTIONS, ParamLimits, SPEAKER_LOSS_OPTIONS, format_peq_export, get_export_extension,
    label_for, parse_loss_type, parse_peq_model,
};

// Re-export preset types
pub use presets::{
    DetailLevel, EqPreset, EqWorkflow, default_preset_id, field_hint, field_warning, find_preset,
    population_to_quality, preset_options, presets_for, quality_label, quality_to_optimizer_params,
};

// Re-export headphone types
pub use headphone::{
    HeadphoneOptResult, HeadphoneOptimizationResult, VisualizationCurves, load_target_curve,
    parse_csv_curve, run_headphone_optimization, run_headphone_optimization_with_callback,
    target_curves,
};

// Re-export speaker types
pub use speaker::{
    CallbackAction, CallbackConfig, Cea2034Data, MeasurementInput, OptimizationOutput,
    OptimizationStage, PreviewCurves, ProgressCallbackConfig, ProgressUpdate, SpeakerConfigTypeExt,
    SpeakerOptResult, SpeakerOptimizationCallback, SpeakerOptimizationConfig,
    SpeakerOptimizationConfigExt, SpeakerOptimizationProgress, SpeakerOptimizationResult,
    load_preview_curves, load_preview_curves_async, run_speaker_optimization,
    run_speaker_optimization_extended, run_speaker_optimization_with_callback,
};

// Re-export multi-speaker types (legacy)
pub use multi_speaker::{
    MultiSpeakerOptimizationCallback, MultiSpeakerOptimizationConfig,
    MultiSpeakerOptimizationResult, MultiSpeakerProgress, SingleSpeakerResult,
    SpeakerMeasurementData, run_multi_speaker_optimization, to_speaker_results,
};

// Re-export roomeq types (new API)
pub use multi_speaker::{
    ChannelDspChain,
    ChannelOptimizationResult,
    CrossoverConfig,
    CurveData,
    DBAConfig,
    DriverDspChain,
    DspChainOutput,
    FirConfig,
    MeasurementSource,
    MultiSubGroup,
    OptimizationMetadata,
    OptimizerConfig,
    PipelineStepId,
    PipelineStepStatus,
    PluginConfigWrapper,
    // Core types
    RoomConfig,
    RoomOptimizationCallback,
    RoomOptimizationProgress,
    RoomOptimizationResult,
    SpeakerConfig,
    // Extended types
    SpeakerGroup,
    TargetCurveConfig,
    build_room_config_from_curves,
    optimize_room,
    optimize_room_with_probe_arrivals,
    optimize_speaker,
    optimizer_config_from_args,
    // Functions
    run_room_optimization,
    run_room_optimization_with_output_dir,
    run_room_optimization_with_probe_arrivals,
    run_room_optimization_with_probe_arrivals_and_output_dir,
    save_dsp_chain,
    to_single_speaker_results,
};
