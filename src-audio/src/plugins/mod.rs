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
mod plugin_compressor;
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

pub use analyzer::{AnalyzerData, AnalyzerPlugin, LoudnessData, SpectrumData};
pub use host::{PluginHost, SharedPluginHost};
pub use parameters::{Parameter, ParameterId, ParameterValue};
pub use plugin::{InPlacePlugin, InPlacePluginAdapter, Plugin, PluginInfo, ProcessContext};

pub use plugin_compressor::{CompressorPlugin, CompressorPluginParams};
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

#[allow(unused_imports)]
pub(crate) use analyzer_loudness_monitor::LoudnessMonitor;
pub use analyzer_loudness_monitor::{LoudnessInfo, LoudnessMonitorPlugin};
#[allow(unused_imports)]
pub(crate) use analyzer_spectrum::SpectrumAnalyzer;
pub use analyzer_spectrum::{SpectrumAnalyzerPlugin, SpectrumConfig, SpectrumInfo};
pub use plugin_loudness_compensation::LoudnessCompensation;
