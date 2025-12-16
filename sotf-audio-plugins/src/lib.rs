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
pub mod param_specs;
mod parameters;
mod plugin;
mod plugin_binaural;
mod plugin_channel_mute_solo;
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

// HAL plugins (macOS only, requires 'hal' feature)
#[cfg(all(target_os = "macos", feature = "hal"))]
mod plugin_hal_input;
#[cfg(all(target_os = "macos", feature = "hal"))]
mod plugin_hal_output;

pub use analyzer::{AnalyzerData, AnalyzerPlugin, LoudnessData, SpectrumData};
pub use host::{DawHost, GraphEdge as DawGraphEdge, Host, NodeId as DawNodeId};
pub use parameters::{Parameter, ParameterId, ParameterValue};
pub use plugin::{InPlacePlugin, InPlacePluginAdapter, Plugin, PluginInfo, ProcessContext};

pub use plugin_binaural::{
    BinauralDecoderParams, BinauralDecoderPlugin, RoomModel, binaural_default_enable_optimization,
};
pub use plugin_channel_mute_solo::{ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState};
pub use plugin_compressor::{CompressorData, CompressorPlugin, CompressorPluginParams};
pub use plugin_convolution::{ConvolutionPlugin, ConvolutionPluginParams};
pub use plugin_crossover::{CrossoverPlugin, CrossoverPluginParams};
pub use plugin_delay::{DelayPlugin, DelayPluginParams};
pub use plugin_eq::{BiquadFilterConfig, EqPlugin, EqPluginParams};
pub use plugin_gain::{GainPlugin, GainPluginParams};
pub use plugin_gate::{GateData, GatePlugin, GatePluginParams};
pub use plugin_limiter::{LimiterPlugin, LimiterPluginParams};
pub use plugin_loudness_compensation::{
    LoudnessCompensationPlugin, LoudnessCompensationPluginParams,
};
pub use plugin_matrix::MatrixPlugin;
pub use plugin_resampler::ResamplerPlugin;
pub use plugin_upmixer::{
    UpmixerPlugin,
    UpmixerPluginParams,
    default_hr_sharpen as upmixer_default_hr_sharpen,
    default_safety_cap_db as upmixer_default_safety_cap_db,
    // Re-export upmixer defaults for preset migration
    default_subharmonic_gain as upmixer_default_subharmonic_gain,
};
// Re-export compressor defaults for preset migration
pub use plugin_compressor::{
    default_auto_makeup as compressor_default_auto_makeup,
    default_link_channels as compressor_default_link_channels,
    default_sidechain_hpf_hz as compressor_default_sidechain_hpf_hz,
};
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

// HAL plugins (macOS only, requires 'hal' feature)
#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_input::{HalInputPlugin, HalInputPluginParams};
#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_output::{HalOutputPlugin, HalOutputPluginParams};
