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
pub mod envelope;
pub mod envelope_follower;
pub mod error;
pub mod fir_crossover;
pub mod host;
pub mod layout_solver;
pub mod lookahead;
pub mod lr4_crossover;
pub mod lufs_target;
pub mod oversampling;
pub mod param_bridge;
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
#[cfg(any(feature = "qa", test, debug_assertions))]
pub mod test_utils;
pub mod true_peak;
pub mod vbap;

pub use adaa::{
    Adaa1, Adaa2, adaa1_hardclip, adaa1_softclip, adaa1_tanh, adaa2_hardclip, adaa2_softclip,
    adaa2_tanh,
};
pub use analyzer::{AnalyzerData, LoudnessData, SpectrumData};

/// Function signature for plugin factories.
/// Takes (plugin_type, parameters, channels, sample_rate) and returns a boxed Plugin.
pub type PluginFactoryFn = fn(
    plugin_type: &str,
    parameters: &serde_json::Value,
    channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn plugin::Plugin>, String>;
pub use analyzer_loudness_monitor::{LoudnessMonitor, LoudnessMonitorPlugin};
pub use analyzer_spectrum::{
    SpectralTiltCorrection, SpectrumAnalyzer, SpectrumAnalyzerPlugin, SpectrumConfig, SpectrumInfo,
    TiltReferenceFreq,
};
pub use auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
pub use auto_makeup::MeasuredMakeup;
pub use channel_linking::{compute_linked_levels, link_stereo};
pub use dc_blocker::DcBlocker;
pub use delta_monitor::DeltaMonitor;
pub use detector::{DetectionMode, LevelDetector};
pub use dynamics_core::SidechainFilterMode;
pub use envelope::DualRelease;
pub use envelope_follower::EnvelopeFollower;
pub use fir_crossover::{FirCrossover, MultibandFirCrossover};
pub use host::{DawHost, GraphEdge, Host};
pub use lookahead::LookaheadBuffer;
pub use lr4_crossover::{Lr4Crossover, MultibandLr4Crossover};
pub use lufs_target::LufsTarget;
pub use oversampling::{
    OversampledPlugin, Oversampler, interleaved_to_planar, planar_to_interleaved,
};
pub use parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
pub use plugin::{
    InPlacePlugin, InPlacePluginAdapter, Plugin, PluginInfo, PluginResult, ProcessContext,
};
pub use true_peak::TruePeakDetector;

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

// ============================================================================
// Shared audio utility functions
// ============================================================================

/// Convert dB to linear gain.
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Convert linear gain to dB. Returns `NEG_INFINITY` for zero or negative input.
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

// ============================================================================
// Shared audio constants
// ============================================================================

// Re-export from math-iir-fir (canonical location)
pub use math_audio_iir_fir::{AUDIBLE_MAX_FREQ, AUDIBLE_MIN_FREQ};

/// Default sample rate used for UI preview calculations (Hz).
pub const DEFAULT_PREVIEW_SAMPLE_RATE: f64 = 48_000.0;
