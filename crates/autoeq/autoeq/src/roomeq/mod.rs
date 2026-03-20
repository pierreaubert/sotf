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

// Configuration loading (shared between roomeq and roomeq_qa binaries)
mod config_loader;
pub use config_loader::{SHALLOW_MERGE_KEYS, load_config, merge_json_objects};

// Configuration validation
mod config;
pub use config::{ValidationResult, validate_room_config};

// Main optimization entry points
mod optimize;
pub use optimize::{
    CallbackAction, ChannelOptimizationResult, RoomOptimizationCallback, RoomOptimizationProgress,
    RoomOptimizationResult, SpeakerOptimizationCallback, SpeakerOptimizationResult, optimize_room,
    optimize_speaker,
};

// Individual optimization modules
mod crossover;
mod dba;
mod eq;
mod fir;
mod group_delay;
mod multisub;
pub mod workflows; // Make public to access from optimize.rs or tests

// Export to external formats (CamillaDSP, APO, EasyEffects, Wavelet, PipeWire)
mod export;
pub use export::{ExportFormat, export_dsp_chain};

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

// Spectral channel alignment (shelf filters + gain)
mod spectral_align;
pub use spectral_align::{
    SpectralAlignmentResult, compute_spectral_alignment, create_alignment_plugins,
    log_spectral_alignment,
};

// Voice of God (timbre matching between channels)
mod voice_of_god;
pub use voice_of_god::{VoGResult, compute_voice_of_god, create_vog_plugins};

// Utility modules
mod ir_waveform;
mod phase_utils;
pub mod synthetic;
mod time_align;
mod weighted_loss;

pub use time_align::{ArrivalTimeResult, calculate_alignment_delays, find_arrival_time};

// Advanced room correction features (Scenario A & B)
pub mod excursion;
pub mod multiseat;
pub mod phase_alignment;
pub mod target_tilt;

pub use excursion::{
    ExcursionProtectionResult, F3DetectionResult, detect_f3, generate_excursion_protection,
};
pub use multiseat::{MultiSeatMeasurements, MultiSeatOptimizationResult, optimize_multiseat};
pub use phase_alignment::{
    PhaseAlignmentResult, optimize_phase_alignment, optimize_phase_alignment_batch,
};
pub use target_tilt::{
    build_harman_target_curve, build_harman_target_curve_with_bass_boost,
    build_target_curve_with_tilt,
};
