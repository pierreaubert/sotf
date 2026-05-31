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

// Core types and configuration (modular)
pub mod types;
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
mod artifacts;
mod pipeline;
pub use pipeline::{
    PipelineControl, PipelineEvent, PipelineObserver, PipelineStepId, PipelineStepStatus,
    RoomPipeline, RoomPipelineRequest,
};

mod optimize;
pub use optimize::{
    CallbackAction, ChannelOptimizationResult, RoomOptimizationCallback, RoomOptimizationProgress,
    RoomOptimizationResult, SpeakerOptimizationCallback, SpeakerOptimizationResult, optimize_room,
    optimize_room_with_probe_arrivals, optimize_speaker,
};

// Extracted optimization submodules
mod auto_tune;
mod crossover_utils;
mod group_processing; // Multi-speaker groups, multisub, DBA, cardioid, mixed-mode
mod speaker_eq; // Single-speaker EQ optimization // Crossover and group consistency utilities

// Individual optimization modules
mod crossover;
mod dba;
mod eq;
mod fir;
mod frequency_grid;
pub mod home_cinema;
pub mod multisub;
pub mod workflows; // Make public to access from optimize.rs or tests

// Export to external formats (CamillaDSP, APO, EasyEffects, Wavelet, PipeWire)
mod export;
pub use export::{
    ExportFormat, export_dsp_chain, export_dsp_chain_with_convolution_sidecars,
    external_export_supported, package_convolution_sidecars,
};

// DSP chain building
mod output;
pub use output::{
    add_delay_plugin, build_channel_dsp_chain, build_channel_dsp_chain_with_curves,
    build_dba_dsp_chain, build_dba_dsp_chain_with_curves, build_multidriver_dsp_chain,
    build_multidriver_dsp_chain_with_curves, build_multisub_dsp_chain,
    build_multisub_dsp_chain_with_allpass, build_multisub_dsp_chain_with_curves,
    create_convolution_plugin, create_crossover_plugin, create_delay_plugin,
    create_dsp_chain_output, create_eq_plugin, create_gain_plugin, create_gain_plugin_with_invert,
    create_labeled_eq_plugin, create_sparse_matrix_plugin, save_dsp_chain,
};

// Progress reporting
mod progress;
pub use progress::{MultiStageProgress, ProgressReporter};

// Spectral channel alignment (shelf filters + gain)
mod spectral_align;
pub use spectral_align::{
    SpectralAlignmentResult, compute_inter_channel_deviation, compute_spectral_alignment,
    create_alignment_plugins, log_spectral_alignment,
};

// Voice of God (timbre matching between channels)
mod voice_of_god;
pub use voice_of_god::{VoGResult, compute_voice_of_god, create_vog_plugins};

// Spatial robustness (multi-position analysis)
pub mod spatial_robustness;
pub use spatial_robustness::{SpatialRobustnessResult, analyze_spatial_robustness};

// Mixed-phase correction (IIR + short FIR)
pub mod mixed_phase;
pub use mixed_phase::{MixedPhaseConfig, MixedPhaseResult, decompose_phase};

// Decomposed correction (modes vs reflections vs steady-state)
pub mod impulse_analysis;
pub use impulse_analysis::{
    DecomposedCorrectionConfig, DecomposedCorrectionResult, analyze_decomposed_correction,
};

// CEA2034 speaker pre-correction (3-pass pipeline)
pub mod cea2034_correction;

// Cross-talk cancellation / binaural-aware correction
pub mod ctc;

// First-reflection cancellation (Johnston IIR filter)
pub mod reflection_cancel;
pub use reflection_cancel::{
    ReflectionCancellationConfig, ReflectionCancellationResult, compute_reflection_cancellation,
};

// Group delay optimisation v2 (LowLatency IIR path)
pub mod gd_opt;

// Bass-phase confidence gate for GD-Opt v2 (§3.5)
pub mod bass_phase_confidence;
pub use bass_phase_confidence::{
    BassPhaseConfidence, DEFAULT_COHERENCE_THRESHOLD, MIN_BASS_OCTAVE_DURATION_S, MIN_NUM_SWEEPS,
    MIN_SNR_DB, bass_phase_confidence as compute_bass_phase_confidence,
};

// Utility modules
mod ir_waveform;
pub(crate) mod phase_utils;
pub mod synthetic;
mod time_align;
// GD-Opt v2 Phase GD-1f — microphone phase calibration loader. See
// `docs/gd_opt_v2_plan.md` §2.6 and §2.8.
pub mod mic_phase_calibration;
pub use mic_phase_calibration::{MicPhaseCalibration, load_mic_phase_calibration};

pub use time_align::{
    ArrivalTimeResult, ProbeDelayResult, calculate_alignment_delays, detect_delay_with_probe,
    detect_delays_multi_channel, find_arrival_time,
};

// Perceptual temporal decay thresholds for modal ringing
pub mod temporal_targets;

// Broadband slope estimation from measurement curves
pub mod slope;

// Advanced room correction features (Scenario A & B)
pub mod excursion;
pub mod listening_area;
pub mod multiseat;
pub mod phase_alignment;
pub mod target_tilt;

pub use listening_area::{ListeningArea, ListeningAreaInterpolatorConfig};

pub use excursion::{
    ExcursionProtectionResult, F3DetectionResult, detect_f3, detect_f3_with_config,
    detect_f3_with_reference_band, generate_excursion_protection,
};
pub use home_cinema::{
    BassBusHeadroomSimulationReport, BassBusOutputHeadroomReport, BassManagementGroupReport,
    BassManagementMatrix, BassManagementOptimizationReport, BassManagementReport,
    BassManagementRoute, BassManagementRoutingGraph, BassManagementSignalFlowEntry,
    BassManagementSubOutputReport, ChannelTimingReport, HomeCinemaChannelReport,
    HomeCinemaLayoutReport, HomeCinemaRole, HomeCinemaRoleGroup, MultiSeatCoverageReport,
    TimingDiagnosticsReport, analyze_layout as analyze_home_cinema_layout,
};
pub use multiseat::{
    MultiSeatMeasurements, MultiSeatOptimizationResult, optimize_multiseat,
    optimize_multiseat_continuous_area,
};
pub use phase_alignment::{
    PhaseAlignmentResult, optimize_phase_alignment, optimize_phase_alignment_batch,
};
pub use target_tilt::build_complete_target_curve;
