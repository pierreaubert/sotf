//! Room EQ - Multi-channel room equalization optimization
//!
//! This module provides room equalization optimization for multi-channel speaker systems.
//! It supports both single-driver speakers and multi-driver configurations with active
//! crossovers.
//!
//! # Architecture
//!
//! The module is organized into:
//! - [`types`] - Type definitions for configuration, results, and DSP output
//! - [`optimizer`] - High-level optimizer API for GPUI integration
//! - [`output`] - DSP chain generation
//!
//! Note: The actual optimization logic will be integrated later by adding
//! the `autoeq` crate as a dependency. For now, this provides the API
//! structure and stub implementations.
//!
//! # Usage
//!
//! ```ignore
//! use sotf_audio_player::room_eq::{RoomEqOptimizer, OptimizerConfig, ChannelMeasurements};
//!
//! // Create optimizer with default config
//! let optimizer = RoomEqOptimizer::with_defaults();
//!
//! // Optimize a single channel
//! let result = optimizer.optimize_single_channel("left", &measurement)?;
//!
//! // Generate DSP chain output
//! let dsp_output = optimizer.generate_dsp_output(&results, &crossover_types);
//! ```
//!
//! # Async Usage (GPUI)
//!
//! For GPUI integration, use the async task runner:
//!
//! ```ignore
//! use sotf_audio_player::room_eq::{run_optimization_task, RoomEqOptimizer};
//! use std::sync::Arc;
//!
//! let optimizer = Arc::new(RoomEqOptimizer::with_defaults());
//! let (progress_tx, progress_rx) = tokio::sync::mpsc::channel(16);
//!
//! // Spawn optimization task
//! let handle = tokio::spawn(run_optimization_task(
//!     optimizer,
//!     channels,
//!     configs,
//!     progress_tx,
//! ));
//!
//! // Receive progress updates
//! while let Some(progress) = progress_rx.recv().await {
//!     // Update UI with progress
//! }
//!
//! let results = handle.await??;
//! ```

mod optimizer;
mod output;
pub mod types;

// Re-export main types and functions
pub use optimizer::{run_optimization_task, RoomEqOptimizer};
pub use output::{load_dsp_chain, save_dsp_chain};
pub use types::{
    Algorithm, ChannelConfig, ChannelDspChain, ChannelMeasurements, ChannelOptimizationResult,
    ChannelOptStatus, CrossoverType, Curve, DspChainOutput, DspPluginConfig, DriverDspChain,
    EqFilterResult, Measurement, OptimizationMetadata, OptimizationProgress, OptimizerConfig,
    SpeakerConfigType,
};
