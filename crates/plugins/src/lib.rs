//! ============================================================================
//! SOTF Audio Plugins Library
//! ============================================================================

pub mod analyzer;
pub mod analyzer_loudness_monitor;
pub mod analyzer_spectrum;
pub mod auto_gain;
pub mod automation;
pub mod error;
pub mod host;
pub mod param_registry;
pub mod param_specs;
pub mod parameters;
pub mod plugin;
pub mod plugin_ab_compare;
pub mod plugin_band_merge;
pub mod plugin_band_split;
pub mod plugin_binaural;
pub mod plugin_channel_mute_solo;
pub mod plugin_compressor;
pub mod plugin_convolution;
pub mod plugin_crossover;
pub mod plugin_delay;
pub mod plugin_denoiser;
pub mod plugin_downmix;
pub mod plugin_eq;
pub mod plugin_expander;
pub mod plugin_fletcher_munson;
pub mod plugin_gain;
pub mod plugin_gate;
pub mod plugin_hal_input;
pub mod plugin_hal_output;
pub mod plugin_limiter;
pub mod plugin_loudness_compensation;
pub mod plugin_matrix;
pub mod plugin_mono_to_stereo;
pub mod plugin_multiband_compressor;
pub mod plugin_multiband_expander;
pub mod plugin_pnd;
pub mod plugin_resampler;
pub mod plugin_upmixer;
pub mod plugin_xtc;
pub mod serialization;
pub mod simd;
pub mod smoothing;
pub mod sofa;
pub mod speaker_config;
pub mod stft_common;

pub use analyzer::{AnalyzerData, LoudnessData, SpectrumData};
pub use analyzer_loudness_monitor::{LoudnessMonitor, LoudnessMonitorPlugin};
pub use analyzer_spectrum::{SpectrumAnalyzerPlugin, SpectrumConfig, SpectrumInfo, SpectrumAnalyzer, SpectralTiltCorrection, TiltReferenceFreq};
pub use auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
pub use host::{DawHost, Host, GraphEdge};
pub use parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
pub use plugin::{InPlacePlugin, Plugin, PluginInfo, PluginResult, ProcessContext, InPlacePluginAdapter};
pub use plugin_gain::{GainPlugin, GainPluginParams};
pub use plugin_matrix::MatrixPlugin;
pub use plugin_channel_mute_solo::{ChannelMuteSoloPlugin, ChannelState, ChannelMuteSoloParams};
pub use plugin_compressor::{CompressorPlugin, CompressorPluginParams, CompressorData, default_link_channels as compressor_default_link_channels, default_sidechain_hpf_hz as compressor_default_sidechain_hpf_hz};
pub use plugin_limiter::{LimiterPlugin, LimiterPluginParams};
pub use plugin_expander::{ExpanderPlugin, ExpanderPluginParams};
pub use plugin_gate::{GatePlugin, GatePluginParams, GateData};
pub use plugin_eq::{EqPlugin, EqPluginParams, BiquadFilterConfig};
pub use plugin_crossover::{CrossoverPlugin, CrossoverPluginParams};
pub use plugin_delay::{DelayPlugin, DelayPluginParams};
pub use plugin_downmix::{DownmixPlugin, DownmixPluginParams};
pub use plugin_mono_to_stereo::{MonoToStereoPlugin, MonoToStereoPluginParams};
pub use plugin_multiband_compressor::{MultibandCompressorPlugin, MultibandCompressorPluginParams, BandCompressorParams};
pub use plugin_multiband_expander::{MultibandExpanderPlugin, MultibandExpanderPluginParams, BandExpanderParams};
pub use plugin_loudness_compensation::{LoudnessCompensation, LoudnessCompensationPlugin, LoudnessCompensationPluginParams};
pub use plugin_fletcher_munson::{FletcherMunsonPlugin, FletcherMunsonPluginParams};
pub use plugin_resampler::ResamplerPlugin;
pub use plugin_convolution::{ConvolutionPlugin, ConvolutionPluginParams};
pub use plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams, default_hr_sharpen as upmixer_default_hr_sharpen, default_safety_cap_db as upmixer_default_safety_cap_db, default_subharmonic_gain as upmixer_default_subharmonic_gain};
pub use plugin_binaural::{BinauralDecoderPlugin, BinauralDecoderParams, RoomModel, binaural_default_enable_optimization};
pub use plugin_xtc::{XtcPlugin, XtcPluginParams};
pub use plugin_denoiser::{DenoiserData, DenoiserPlugin, DenoiserPluginParams};
pub use plugin_pnd::{PndPlugin, PndPluginParams};
pub use plugin_ab_compare::{ABComparePlugin, ABComparePluginParams};
pub use plugin_band_split::{BandSplitPlugin, BandSplitPluginParams};
pub use plugin_band_merge::{BandMergePlugin, BandMergePluginParams};

pub use speaker_config::{
    SpeakerPosition, get_speaker_config_by_channels, get_meter_groups, get_meter_groups_by_channels,
};
pub use sofa::{HrtfData, SofaFile, SourcePosition};
pub use simd::enable_ftz_daz;

pub type PluginHost = DawHost;

#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_input::{HalInputPlugin, HalInputPluginParams};
#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_output::{HalOutputPlugin, HalOutputPluginParams};
pub use analyzer_loudness_monitor::LoudnessInfo;
