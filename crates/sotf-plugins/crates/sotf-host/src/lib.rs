//! ============================================================================
//! SOTF Host - Core traits, host, and shared utilities for audio plugins
//! ============================================================================

pub mod adaa;
pub mod analyzer;
pub mod analyzer_loudness_monitor;
pub mod analyzer_spectrum;
pub mod auto_gain;
pub mod auto_makeup;
pub mod automation;
pub mod channel_linking;
pub mod custom_views;
pub mod dc_blocker;
pub mod delta_monitor;
pub mod design_system;
pub mod detector;
pub mod dynamics_core;
pub mod envelope_follower;
pub mod fir_crossover;
pub mod envelope;
pub mod error;
pub mod host;
pub mod layout_solver;
pub mod lookahead;
pub mod lr4_crossover;
pub mod lufs_target;
pub mod oversampling;
pub mod param_registry;
pub mod param_specs;
pub mod parameters;
pub mod plugin;
pub mod plugin_layout;
pub mod plugin_params;
pub mod render_plan;
pub mod serialization;
pub mod simd;
pub mod smoothing;
pub mod sofa;
pub mod speaker_config;
pub mod stft_common;
pub mod true_peak;
pub mod vbap;
#[cfg(any(feature = "qa", test, debug_assertions))]
pub mod test_utils;

pub use adaa::{Adaa1, Adaa2, adaa1_hardclip, adaa1_softclip, adaa1_tanh, adaa2_hardclip, adaa2_softclip, adaa2_tanh};
pub use analyzer::{AnalyzerData, LoudnessData, SpectrumData};
pub use auto_makeup::MeasuredMakeup;
pub use channel_linking::{compute_linked_levels, link_stereo};
pub use dc_blocker::DcBlocker;
pub use delta_monitor::DeltaMonitor;
pub use detector::{DetectionMode, LevelDetector};
pub use fir_crossover::{FirCrossover, MultibandFirCrossover};
pub use dynamics_core::SidechainFilterMode;
pub use envelope::DualRelease;
pub use envelope_follower::EnvelopeFollower;
pub use lookahead::LookaheadBuffer;
pub use lr4_crossover::{Lr4Crossover, MultibandLr4Crossover};
pub use lufs_target::LufsTarget;
pub use oversampling::{Oversampler, interleaved_to_planar, planar_to_interleaved};
pub use true_peak::TruePeakDetector;
pub use analyzer_loudness_monitor::{LoudnessMonitor, LoudnessMonitorPlugin};
pub use analyzer_spectrum::{
    SpectralTiltCorrection, SpectrumAnalyzer, SpectrumAnalyzerPlugin, SpectrumConfig, SpectrumInfo,
    TiltReferenceFreq,
};
pub use auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
pub use host::{DawHost, GraphEdge, Host};
pub use parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
pub use plugin::{
    InPlacePlugin, InPlacePluginAdapter, Plugin, PluginInfo, PluginResult, ProcessContext,
};

#[cfg(feature = "qa")]
pub use test_utils::benchmark_plugin_full;
#[cfg(any(feature = "qa", test, debug_assertions))]
pub use test_utils::{
    BufferComparison, CountingAlloc, PerformanceProfiler, SignalGen, assert_no_allocs,
    detect_latency, generate_dc, measure_peak_db, measure_rms_db, run_standard_tests,
    test_parameter_ramp, test_varied_buffer_sizes,
};

pub use analyzer_loudness_monitor::LoudnessInfo;
pub use simd::enable_ftz_daz;
pub use sofa::{HrtfData, SofaFile, SourcePosition};
pub use speaker_config::{
    SpeakerPosition, get_meter_groups, get_meter_groups_by_channels, get_speaker_config_by_channels,
};

pub type PluginHost = DawHost;
