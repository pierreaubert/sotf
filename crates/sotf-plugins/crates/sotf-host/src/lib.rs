//! ============================================================================
//! SOTF Host - Core traits, host, and shared utilities for audio plugins
//! ============================================================================

pub mod analyzer;
pub mod analyzer_loudness_monitor;
pub mod analyzer_spectrum;
pub mod auto_gain;
pub mod automation;
pub mod error;
pub mod host;
pub mod param_registry;
pub mod parameters;
pub mod param_specs;
pub mod plugin;
pub mod serialization;
pub mod simd;
pub mod smoothing;
pub mod sofa;
pub mod speaker_config;
pub mod stft_common;
#[cfg(any(feature = "qa", test, debug_assertions))]
pub mod test_utils;

pub use analyzer::{AnalyzerData, LoudnessData, SpectrumData};
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

#[cfg(any(feature = "qa", test, debug_assertions))]
pub use test_utils::{
    assert_no_allocs, detect_latency, generate_dc, measure_peak_db, measure_rms_db,
    run_standard_tests, test_parameter_ramp, test_varied_buffer_sizes, BufferComparison,
    CountingAlloc, PerformanceProfiler, SignalGen,
};
#[cfg(feature = "qa")]
pub use test_utils::benchmark_plugin_full;

pub use analyzer_loudness_monitor::LoudnessInfo;
pub use simd::enable_ftz_daz;
pub use sofa::{HrtfData, SofaFile, SourcePosition};
pub use speaker_config::{
    SpeakerPosition, get_meter_groups, get_meter_groups_by_channels, get_speaker_config_by_channels,
};

pub type PluginHost = DawHost;
