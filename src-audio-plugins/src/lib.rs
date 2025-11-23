// ============================================================================
// Audio Plugin System
// ============================================================================
//
// This module provides a flexible plugin system for audio processing.
// Plugins can be chained together in a host, with each plugin processing
// N input channels and producing P output channels.
//
// Architecture:
// - Plugin trait: Defines the interface for audio processing plugins
// - PluginHost: Chains multiple plugins together
// - Parameter system: Allows dynamic parameter changes
//
// Example usage:
// ```
// let mut host = PluginHost::new(2, 44100); // 2 channels, 44.1kHz
// let gain_plugin = GainPlugin::new(2, -6.0); // -6dB gain
// host.add_plugin(Box::new(gain_plugin));
// host.process(&mut audio_buffer);
// ```

mod analyzer;
mod analyzer_loudness_monitor;
mod analyzer_spectrum;
mod host;
mod parameters;
mod plugin;
mod plugin_binaural_decoder;
mod plugin_compressor;
mod plugin_convolution;
mod plugin_crossover;
mod plugin_delay;
mod plugin_eq;
mod plugin_gain;
mod plugin_gate;
mod plugin_limiter;
mod plugin_loudness_compensation;
mod plugin_matrix;
mod plugin_resampler;
mod plugin_upmixer;
mod simd;
mod sofa;
mod speaker_config;

// HAL plugins (macOS only)
#[cfg(target_os = "macos")]
mod plugin_hal_input;
#[cfg(target_os = "macos")]
mod plugin_hal_output;

pub use analyzer::{AnalyzerData, AnalyzerPlugin, LoudnessData, SpectrumData};
pub use host::{Host, DawHost, GraphEdge as DawGraphEdge, NodeId as DawNodeId};
pub use parameters::{Parameter, ParameterId, ParameterValue};
pub use plugin::{InPlacePlugin, InPlacePluginAdapter, Plugin, PluginInfo, ProcessContext};

pub use plugin_binaural_decoder::{BinauralDecoderParams, BinauralDecoderPlugin};
pub use plugin_compressor::{CompressorPlugin, CompressorPluginParams};
pub use plugin_convolution::{ConvolutionPlugin, ConvolutionPluginParams};
pub use plugin_crossover::{CrossoverPlugin, CrossoverPluginParams};
pub use plugin_delay::{DelayPlugin, DelayPluginParams};
pub use plugin_eq::{BiquadFilterConfig, EqPlugin, EqPluginParams};
pub use plugin_gain::{GainPlugin, GainPluginParams};
pub use plugin_gate::{GatePlugin, GatePluginParams};
pub use plugin_limiter::{LimiterPlugin, LimiterPluginParams};
pub use plugin_loudness_compensation::{
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams,
};
pub use plugin_matrix::MatrixPlugin;
pub use plugin_resampler::ResamplerPlugin;
pub use plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams};
pub use speaker_config::{
    SpeakerConfig, SpeakerPosition, calculate_panning_gain, get_available_configs,
    get_speaker_config, get_speaker_config_by_channels,
};

pub use sofa::{HrtfData, SofaFile, SourcePosition};

#[allow(unused_imports)]
pub(crate) use analyzer_loudness_monitor::LoudnessMonitor;
pub use analyzer_loudness_monitor::{LoudnessInfo, LoudnessMonitorPlugin};
#[allow(unused_imports)]
pub(crate) use analyzer_spectrum::SpectrumAnalyzer;
pub use analyzer_spectrum::{SpectrumAnalyzerPlugin, SpectrumConfig, SpectrumInfo};
pub use plugin_loudness_compensation::LoudnessCompensation;

// Define PluginHost as alias for DawHost (the single supported host type)
pub type PluginHost = DawHost;

// HAL plugins (macOS only)
#[cfg(target_os = "macos")]
pub use plugin_hal_input::{HalInputPlugin, HalInputPluginParams};
#[cfg(target_os = "macos")]
pub use plugin_hal_output::{HalOutputPlugin, HalOutputPluginParams};
