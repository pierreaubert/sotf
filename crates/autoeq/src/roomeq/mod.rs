//! Room EQ - Multi-channel room equalization library
//!
//! This module provides library functions for multi-channel speaker EQ optimization,
//! including support for:
//! - Single speaker EQ optimization
//! - Multi-driver speaker groups with active crossovers
//! - Multiple subwoofer optimization
//! - Double Bass Array (DBA) optimization
//! - Group delay alignment
//! - FIR filter generation
//!
//! # Example
//!
//! ```no_run
//! use autoeq::roomeq::{RoomConfig, optimize_room};
//!
//! let config_json = r#"{
//!   "speakers": {
//!     "left": { "path": "measurements/left.csv" }
//!   },
//!   "optimizer": { "loss_type": "flat", "algorithm": "cobyla" }
//! }"#;
//! let config: RoomConfig = serde_json::from_str(config_json)?;
//! let result = optimize_room(&config, 48000.0, None, None)?;
//!
//! for (channel, chain) in &result.channels {
//!     println!("Channel {}: {} plugins", channel, chain.plugins.len());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// Core types and configuration
mod types;
pub use types::*;
// Re-export RecordingConfiguration explicitly for clarity
pub use types::RecordingConfiguration;

// Configuration validation
mod config;
pub use config::{ValidationResult, validate_room_config};

// Main optimization entry points
mod optimize;
pub use optimize::{
    CallbackAction, ChannelOptimizationResult, RoomOptimizationCallback,
    RoomOptimizationProgress, RoomOptimizationResult, SpeakerOptimizationCallback,
    SpeakerOptimizationResult, optimize_room, optimize_speaker,
};

// Individual optimization modules
mod eq;
mod crossover;
mod multisub;
mod dba;
mod fir;
mod group_delay;
pub mod workflows; // Make public to access from optimize.rs or tests

// DSP chain building
mod output;
pub use output::{
    add_delay_plugin, build_channel_dsp_chain, build_channel_dsp_chain_with_curves,
    build_dba_dsp_chain, build_dba_dsp_chain_with_curves, build_multidriver_dsp_chain,
    build_multidriver_dsp_chain_with_curves, build_multisub_dsp_chain,
    build_multisub_dsp_chain_with_curves, create_convolution_plugin, create_crossover_plugin,
    create_delay_plugin, create_dsp_chain_output, create_eq_plugin, create_gain_plugin,
    create_gain_plugin_with_invert, save_dsp_chain,
};

// Progress reporting
mod progress;
pub use progress::{MultiStageProgress, ProgressReporter};

// Utility modules
mod phase_utils;
mod time_align;
mod weighted_loss;

pub use time_align::{find_arrival_time, calculate_alignment_delays, ArrivalTimeResult};

// Advanced room correction features (Scenario A & B)
pub mod target_tilt;
pub mod excursion;
pub mod phase_alignment;
pub mod multiseat;

pub use target_tilt::{build_target_curve_with_tilt, build_harman_target_curve, build_harman_target_curve_with_bass_boost};
pub use excursion::{detect_f3, generate_excursion_protection, ExcursionProtectionResult, F3DetectionResult};
pub use phase_alignment::{optimize_phase_alignment, optimize_phase_alignment_batch, PhaseAlignmentResult};
pub use multiseat::{optimize_multiseat, MultiSeatMeasurements, MultiSeatOptimizationResult};
