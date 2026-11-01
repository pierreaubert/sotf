//! Multi-speaker / Room EQ optimization module
//!
//! Provides the ability to optimize EQ filters for multiple speakers simultaneously.
//! This module wraps the `roomeq_workflow` library for room optimization.
//!
//! # Architecture
//!
//! The primary entry point is `run_room_optimization` which uses `roomeq_workflow::optimize_room`
//! for parallel multi-channel optimization. The older `run_multi_speaker_optimization` is kept
//! for backward compatibility but delegates to the new implementation.
//!
//! # Usage
//!
//! ```ignore
//! use sotf_audio_player::autoeq::multi_speaker::{
//!     run_room_optimization, RoomOptimizationConfig, ChannelConfig,
//! };
//! use roomeq_model::RoomConfig;
//! use roomeq_model::SpeakerConfig;
//! use roomeq_model::OptimizerConfig;
//! use autoeq_core::MeasurementSource;
//!
//! // Build room config with in-memory curves
//! let mut speakers = std::collections::HashMap::new();
//! speakers.insert("left".to_string(), SpeakerConfig::Single(
//!     MeasurementSource::InMemory(left_curve)
//! ));
//! speakers.insert("right".to_string(), SpeakerConfig::Single(
//!     MeasurementSource::InMemory(right_curve)
//! ));
//!
//! let config = RoomConfig {
//!     speakers,
//!     optimizer: OptimizerConfig::default(),
//!     ..Default::default()
//! };
//!
//! let result = run_room_optimization(&config, 48000.0, None)?;
//! ```

pub use autoeq_core::MeasurementSource;
pub use roomeq_engine::PipelineControl;
pub use roomeq_engine::PipelineEvent;
pub use roomeq_engine::PipelineObserver;
pub use roomeq_engine::PipelineStepId;
pub use roomeq_engine::PipelineStepStatus;
pub use roomeq_model::ChannelDspChain;
pub use roomeq_model::CrossoverConfig;
pub use roomeq_model::CurveData;
pub use roomeq_model::DBAConfig;
pub use roomeq_model::DriverDspChain;
pub use roomeq_model::DspChainOutput;
pub use roomeq_model::FirConfig;
pub use roomeq_model::MultiSubGroup;
pub use roomeq_model::OptimizationMetadata;
pub use roomeq_model::OptimizerConfig;
pub use roomeq_model::PluginConfigWrapper;
pub use roomeq_model::ProcessingMode;
pub use roomeq_model::RoomConfig;
pub use roomeq_model::SpeakerConfig;
pub use roomeq_model::SpeakerGroup;
pub use roomeq_model::SubwooferStrategy;
pub use roomeq_model::TargetCurveConfig;
pub use roomeq_workflow::CallbackAction;
pub use roomeq_workflow::ChannelOptimizationResult;
pub use roomeq_workflow::RoomOptimizationCallback;
pub use roomeq_workflow::RoomOptimizationProgress;
pub use roomeq_workflow::RoomOptimizationResult;
pub use roomeq_workflow::RoomPipeline;
pub use roomeq_workflow::RoomPipelineRequest;
pub use roomeq_workflow::SpeakerOptimizationResult as RoomSpeakerOptResult;
pub use roomeq_workflow::optimize_room;
pub use roomeq_workflow::optimize_room_with_probe_arrivals;
pub use roomeq_workflow::optimize_speaker;
pub use roomeq_workflow::save_dsp_chain;

mod misc;
mod multi_speaker_optimization_config;
mod run;
mod speaker_measurement_data;
#[cfg(test)]
mod tests;
mod to;
mod types;

pub use misc::*;
pub use multi_speaker_optimization_config::*;
pub use run::*;
pub use speaker_measurement_data::*;
pub use to::*;
pub use types::*;
