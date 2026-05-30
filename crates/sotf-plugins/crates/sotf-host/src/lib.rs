//! ============================================================================
//! SOTF Host - Core traits, host, and shared utilities for audio plugins
//! ============================================================================

pub mod analyzer;
pub mod analyzer_channel_correlation;
pub mod analyzer_loudness_monitor;
pub mod analyzer_spectrum;
pub mod auto_gain;
pub mod automation;
pub mod custom_views;
pub mod error;
pub mod external_plugin;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod external_plugin_host;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod external_plugin_ipc;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod external_plugin_isolated;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod external_plugin_process;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod external_plugin_sandbox;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod external_plugin_worker;
pub mod host;
pub mod layout_solver;
pub mod lufs_target;
pub mod multichannel_auto_gain;
pub mod oversampling;
pub mod param_bridge;
pub mod param_registry;
pub mod param_specs;
pub mod parameters;
pub mod plugin;
pub mod plugin_layout;
pub mod plugin_params;
pub mod rate_limit;
pub mod render_plan;
pub mod serialization;
pub mod sofa;
pub mod speaker_config;
#[cfg(any(feature = "qa", test, debug_assertions))]
pub mod test_utils;
pub mod vbap;

// Re-export math-dsp modules (previously separate single-line files)
pub use math_audio_dsp::stft as stft_common;
pub use math_audio_dsp::true_peak;
pub use math_audio_dsp::{
    adaa, auto_makeup, channel_linking, dc_blocker, delta_monitor, detector, dynamics_core,
    envelope, envelope_follower, lookahead, simd, smoothing,
};

// Re-export math-iir-fir modules
pub use math_audio_iir_fir::{fir_crossover, lr4_crossover, lr8_crossover};

// Re-export gpui-design
pub use gpui_design as design_system;

// Flat re-exports for commonly used types
pub use analyzer::{AnalyzerData, CorrelationData, LoudnessData, SpectrumData};
pub use analyzer_channel_correlation::{ChannelCorrelationMonitor, ChannelCorrelationPlugin};
pub use math_audio_dsp::adaa::{
    Adaa1, Adaa2, adaa1_hardclip, adaa1_softclip, adaa1_tanh, adaa2_hardclip, adaa2_softclip,
    adaa2_tanh,
};

/// Function signature for plugin factories.
/// Takes (plugin_type, parameters, channels, sample_rate) and returns a boxed Plugin.
pub type PluginFactoryFn = fn(
    plugin_type: &str,
    parameters: &serde_json::Value,
    channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn plugin::Plugin>, String>;
pub use analyzer_loudness_monitor::{LoudnessInfo, LoudnessMonitor, LoudnessMonitorPlugin};
pub use analyzer_spectrum::{
    SpectralTiltCorrection, SpectrumAnalyzer, SpectrumAnalyzerPlugin, SpectrumConfig, SpectrumInfo,
    TiltReferenceFreq,
};
pub use auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
pub use external_plugin::{
    ExternalHostingBackend, ExternalPlugin, ExternalPluginHostingPlan, ExternalPluginSandboxMode,
    ExternalPluginState, PluginDescriptor, PluginFormat, PluginFormatCapability, PluginScanStatus,
    PluginScanStatusMode, PluginScanner, plan_external_plugin_hosting, plugin_format_capabilities,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use external_plugin_host::{ExternalPluginHostBlockStatus, ExternalPluginHostProxy};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use external_plugin_ipc::{
    PluginIpcLayout, PluginSandboxBackendCode, PluginSandboxRuntimeStatus, PluginSandboxStatusCode,
    SecurePluginSharedMemory,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use external_plugin_isolated::{IsolatedExternalPlugin, IsolatedExternalPluginConfig};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use external_plugin_process::{
    ExternalPluginProcessEvent, ExternalPluginProcessSupervisor, ExternalPluginWorkerCommand,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use external_plugin_sandbox::{
    ExternalPluginSandboxPolicy, ExternalPluginSandboxStatus, ExternalPluginSandboxTiming,
    ExternalPluginTrust, enter_external_plugin_sandbox,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use external_plugin_worker::{ExternalPluginWorker, ExternalPluginWorkerStep};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use host::IsolatedExternalPluginWorkerReport;
pub use host::{DawHost, GraphEdge, GraphMutationSender, Host, ParameterEventSender};
pub use lufs_target::LufsTarget;
pub use math_audio_dsp::auto_makeup::MeasuredMakeup;
pub use math_audio_dsp::channel_linking::{compute_linked_levels, link_stereo};
pub use math_audio_dsp::dc_blocker::DcBlocker;
pub use math_audio_dsp::delta_monitor::DeltaMonitor;
pub use math_audio_dsp::detector::{DetectionMode, LevelDetector};
pub use math_audio_dsp::dynamics_core::SidechainFilterMode;
pub use math_audio_dsp::envelope::DualRelease;
pub use math_audio_dsp::envelope_follower::EnvelopeFollower;
pub use math_audio_dsp::lookahead::LookaheadBuffer;
pub use math_audio_dsp::simd::enable_ftz_daz;
pub use math_audio_dsp::true_peak::TruePeakDetector;
pub use math_audio_iir_fir::fir_crossover::{FirCrossover, MultibandFirCrossover};
pub use math_audio_iir_fir::lr4_crossover::{Lr4Crossover, MultibandLr4Crossover};
pub use math_audio_iir_fir::lr8_crossover::{Lr8Crossover, MultibandLr8Crossover};
pub use multichannel_auto_gain::MultichannelAutoGain;
pub use oversampling::{
    AutoOversampledPlugin, OversampledPlugin, Oversampler, interleaved_to_planar,
    planar_to_interleaved,
};
pub use parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
pub use plugin::{
    InPlacePlugin, InPlacePluginAdapter, LoopRange, MidiEvent, MidiMessage, Plugin, PluginInfo,
    PluginResult, ProcessContext, TimeSignature, TransportInfo,
};

#[cfg(feature = "qa")]
pub use test_utils::benchmark_plugin_full;
#[cfg(any(feature = "qa", test, debug_assertions))]
pub use test_utils::{
    BufferComparison, CountingAlloc, PerformanceProfiler, SignalGen, assert_no_allocs,
    detect_latency, generate_dc, measure_peak_db, measure_rms_db, run_standard_tests,
    test_parameter_ramp, test_varied_buffer_sizes,
};

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
