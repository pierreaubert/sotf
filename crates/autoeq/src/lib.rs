#![doc = include_str!("../README.md")]

/// Conditional println macro that only prints when not in QA mode
#[macro_export]
macro_rules! qa_println {
    // Without args parameter - always log (for contexts without args access)
    // This pattern must come first to match string literals
    ($fmt:literal) => {
        log::debug!($fmt);
    };
    ($fmt:literal, $($arg:expr),* $(,)?) => {
        log::debug!($fmt, $($arg),*);
    };
    // With args parameter - conditional logging
    ($args:expr, $fmt:literal) => {
        if $args.qa.is_none() {
            log::debug!($fmt);
        }
    };
    ($args:expr, $fmt:literal, $($arg:expr),* $(,)?) => {
        if $args.qa.is_none() {
            log::debug!($fmt, $($arg),*);
        }
    };
}

// Re-export external crate functionality
pub use math_audio_iir_fir as iir;
pub use math_audio_optimisation as de;

/// CEA2034 (Spinorama) speaker measurement metrics
pub mod cea2034;

// Re-export types from CEA2034 module to ensure type compatibility
pub use cea2034::{Curve, DirectivityCurve, DirectivityData};

/// Error types for autoeq operations.
pub mod error;
pub use error::{AutoeqError, Result};

/// Common CLI argument definitions shared across binaries
pub mod cli;
/// Constraint functions for optimization
pub mod constraints;
/// FIR filter design and optimization
pub mod fir;
/// Smart initial guess generation
pub mod initial_guess;
/// Loss functions for optimization
pub mod loss;
/// Optimization algorithms, objective functions, and shared setup
pub mod optim;
/// Parameter vector utilities for different PEQ models
pub mod param_utils;
/// Plotting and visualization functions (requires the `plotly` feature).
#[cfg(feature = "plotly")]
pub mod plot;
/// Data reading and parsing functions
pub mod read;
/// Frequency response utilities
pub mod response;
/// Shared workflow steps used by binaries
pub mod workflow;
/// Mapping
pub mod x2peq;

/// Room EQ multi-channel optimization
pub mod roomeq;

// Backward-compatible re-exports for moved modules
pub use optim::callback as optim_callback;
pub use optim::de as optim_de;
pub use optim::mh as optim_mh;
pub use optim::params as optim_params;

// Re-export commonly used items
pub use cli::*;
pub use loss::{CrossoverType, HeadphoneLossData, LossType, SpeakerLossData};
pub use optim::params::OptimParams;
pub use optim::*;
#[cfg(feature = "plotly")]
pub use plot::*;
pub use read::*;
pub use workflow::*;
pub use workflow::{
    HeadphoneOptResult, OptimizationOutput, ProgressCallbackConfig, ProgressUpdate,
    SpeakerOptResult, VisualizationCurves, compute_visualization_curves, optimize_headphone,
    optimize_speaker, perform_optimization_with_progress,
};
pub use x2peq::x2peq;

// Re-export commonly used roomeq types
pub use roomeq::{
    DspChainOutput, OptimizerConfig, RecordingConfiguration, RoomConfig, RoomOptimizationProgress,
    RoomOptimizationResult, SpeakerConfig, optimize_room,
    optimize_speaker as optimize_room_speaker,
};
