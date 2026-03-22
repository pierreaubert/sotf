//! ============================================================================
//! SOTF Audio Plugins Library
//! ============================================================================
//!
//! This is the facade crate that re-exports shared infrastructure from `sotf-host`
//! and all plugin implementations from their individual crates.

// Re-export the serde_param_default macro from sotf-host
pub use sotf_host::serde_param_default;

// Re-export infrastructure modules from sotf-host
pub use sotf_host::analyzer;
pub use sotf_host::analyzer_loudness_monitor;
pub use sotf_host::analyzer_spectrum;
pub use sotf_host::auto_gain;
pub use sotf_host::automation;
pub use sotf_host::error;
pub use sotf_host::layout_solver;
pub use sotf_host::param_registry;
/// Parameter specifications: types from `sotf-host`, per-plugin definitions
/// from individual plugin crates.
///
/// Migrated plugins export PARAMS/LAYOUT/Params from their own `params.rs`.
/// Un-migrated plugins still have inline modules in `sotf_host::param_specs`.
pub mod param_specs {
    // Re-export all types, utilities, and macros from sotf-host
    pub use sotf_host::param_specs::*;

    // Migrated plugins: re-export from plugin crate (canonical source)
    pub mod gain {
        pub use sotf_plugin_gain::params::*;
    }
    pub mod delay {
        pub use sotf_plugin_delay::params::*;
    }
    pub mod band_split {
        pub use sotf_plugin_band_split::params::*;
    }
    pub mod band_merge {
        pub use sotf_plugin_band_merge::params::*;
    }
    pub mod aec {
        pub use sotf_plugin_aec::params::*;
    }
    pub mod beamformer {
        pub use sotf_plugin_beamformer::params::*;
    }
    pub mod ambisonics {
        pub use sotf_plugin_ambisonics::params::*;
    }
    pub mod limiter {
        pub use sotf_plugin_limiter::params::*;
    }
    pub mod gate {
        pub use sotf_plugin_gate::params::*;
    }
    pub mod pnd {
        pub use sotf_plugin_pnd::params::*;
    }
    pub mod downmix {
        pub use sotf_plugin_downmix::params::*;
    }
    pub mod mono_to_stereo {
        pub use sotf_plugin_mono_to_stereo::params::*;
    }
    pub mod compressor {
        pub use sotf_plugin_compressor::params::*;
    }
    pub mod expander {
        pub use sotf_plugin_expander::params::*;
    }
    pub mod loudness_compensation {
        pub use sotf_plugin_loudness_compensation::params::*;
    }
    pub mod convolution {
        pub use sotf_plugin_convolution::params::*;
    }
    pub mod binaural {
        pub use sotf_plugin_binaural::params::*;
    }
    pub mod crossfeed {
        pub use sotf_plugin_crossfeed::params::*;
    }
    pub mod xtc {
        pub use sotf_plugin_xtc::params::*;
    }
    pub mod ab_compare {
        pub use sotf_plugin_ab_compare::params::*;
    }
    pub mod fletcher_munson {
        pub use sotf_plugin_fletcher_munson::params::*;
    }
    pub mod upmixer {
        pub use sotf_plugin_upmixer::params::*;
    }
    pub mod denoiser {
        pub use sotf_plugin_denoiser::params::*;
    }
    pub mod eq {
        pub use sotf_plugin_eq::params::*;
    }
    pub mod multiband_compressor {
        pub use sotf_plugin_multiband_compressor::params::*;
    }
    pub mod multiband_expander {
        pub use sotf_plugin_multiband_expander::params::*;
    }
    pub mod matrix {
        pub use sotf_plugin_matrix::params::*;
    }
    pub mod channel_mute_solo {
        pub use sotf_plugin_channel_mute_solo::params::*;
    }
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub mod hal_input {
        pub use sotf_plugin_hal_input::params::*;
    }
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub mod hal_output {
        pub use sotf_plugin_hal_output::params::*;
    }
}
pub use sotf_host::parameters;
pub use sotf_host::plugin;
pub use sotf_host::plugin_layout;
pub use sotf_host::serialization;
pub use sotf_host::simd;
pub use sotf_host::smoothing;
pub use sotf_host::sofa;
pub use sotf_host::speaker_config;
pub use sotf_host::stft_common;
#[cfg(any(feature = "qa", test, debug_assertions))]
pub use sotf_host::test_utils;

// Re-export all plugin crates
pub use sotf_plugin_ab_compare as plugin_ab_compare;
pub use sotf_plugin_aec as plugin_aec;
pub use sotf_plugin_band_merge as plugin_band_merge;
pub use sotf_plugin_beamformer as plugin_beamformer;
pub use sotf_plugin_band_split as plugin_band_split;
pub use sotf_plugin_binaural as plugin_binaural;
pub use sotf_plugin_channel_mute_solo as plugin_channel_mute_solo;
pub use sotf_plugin_compressor as plugin_compressor;
pub use sotf_plugin_convolution as plugin_convolution;
pub use sotf_plugin_crossfeed as plugin_crossfeed;
pub use sotf_plugin_crossover as plugin_crossover;
pub use sotf_plugin_delay as plugin_delay;
pub use sotf_plugin_denoiser as plugin_denoiser;
pub use sotf_plugin_downmix as plugin_downmix;
pub use sotf_plugin_eq as plugin_eq;
pub use sotf_plugin_expander as plugin_expander;
pub use sotf_plugin_fletcher_munson as plugin_fletcher_munson;
pub use sotf_plugin_gain as plugin_gain;
pub use sotf_plugin_gate as plugin_gate;
pub use sotf_plugin_limiter as plugin_limiter;
pub use sotf_plugin_loudness_compensation as plugin_loudness_compensation;
pub use sotf_plugin_matrix as plugin_matrix;
pub use sotf_plugin_mono_to_stereo as plugin_mono_to_stereo;
pub use sotf_plugin_multiband_compressor as plugin_multiband_compressor;
pub use sotf_plugin_multiband_expander as plugin_multiband_expander;
pub use sotf_plugin_pnd as plugin_pnd;
pub use sotf_plugin_resampler as plugin_resampler;
pub use sotf_plugin_upmixer as plugin_upmixer;
pub use sotf_plugin_xtc as plugin_xtc;

#[cfg(all(target_os = "macos", feature = "hal"))]
pub use sotf_plugin_hal_input as plugin_hal_input;
#[cfg(all(target_os = "macos", feature = "hal"))]
pub use sotf_plugin_hal_output as plugin_hal_output;

// Re-export all public types for backward compatibility
pub use plugin_ab_compare::{ABComparePlugin, ABComparePluginParams};
pub use plugin_aec::{AecPlugin, AecPluginParams};
pub use plugin_beamformer::{BeamformerPlugin, BeamformerPluginParams, BeamformerType};
pub use plugin_band_merge::{BandMergePlugin, BandMergePluginParams};
pub use plugin_band_split::{BandSplitPlugin, BandSplitPluginParams};
pub use plugin_binaural::{
    BinauralDecoderParams, BinauralDecoderPlugin, RoomModel, binaural_default_enable_optimization,
};
pub use plugin_channel_mute_solo::{ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState};
pub use plugin_compressor::{
    CompressorData, CompressorPlugin, CompressorPluginParams,
    default_link_channels as compressor_default_link_channels,
    default_sidechain_hpf_hz as compressor_default_sidechain_hpf_hz,
};
pub use plugin_convolution::{ConvolutionPlugin, ConvolutionPluginParams};
pub use plugin_crossfeed::{
    CrossfeedMode, CrossfeedPlugin, CrossfeedPluginParams, CrossfeedPreset,
};
pub use plugin_crossover::{CrossoverPlugin, CrossoverPluginParams};
pub use plugin_delay::{DelayPlugin, DelayPluginParams};
pub use plugin_denoiser::{DenoiserData, DenoiserPlugin, DenoiserPluginParams};
pub use plugin_downmix::{DownmixPlugin, DownmixPluginParams};
pub use plugin_eq::{BiquadFilterConfig, EqPlugin, EqPluginParams};
pub use plugin_expander::{ExpanderPlugin, ExpanderPluginParams};
pub use plugin_fletcher_munson::{FletcherMunsonPlugin, FletcherMunsonPluginParams};
pub use plugin_gain::{GainPlugin, GainPluginParams};
pub use plugin_gate::{GateData, GatePlugin, GatePluginParams};
pub use plugin_limiter::{LimiterData, LimiterPlugin, LimiterPluginParams};
pub use plugin_loudness_compensation::{
    LoudnessCompensation, LoudnessCompensationPlugin, LoudnessCompensationPluginParams,
};
pub use plugin_matrix::MatrixPlugin;
pub use plugin_mono_to_stereo::{MonoToStereoPlugin, MonoToStereoPluginParams};
pub use plugin_multiband_compressor::{
    BandCompressorParams, MultibandCompressorPlugin, MultibandCompressorPluginParams,
};
pub use plugin_multiband_expander::{
    BandExpanderParams, MultibandExpanderPlugin, MultibandExpanderPluginParams,
};
pub use plugin_pnd::{PndPlugin, PndPluginParams};
pub use plugin_resampler::ResamplerPlugin;
pub use plugin_upmixer::{
    UpmixerPlugin, UpmixerPluginParams, default_hr_sharpen as upmixer_default_hr_sharpen,
    default_safety_cap_db as upmixer_default_safety_cap_db,
    default_subharmonic_gain as upmixer_default_subharmonic_gain,
};
pub use plugin_xtc::{XtcPlugin, XtcPluginParams, validation};
pub use sotf_host::analyzer::{AnalyzerData, LoudnessData, SpectrumData};
pub use sotf_host::analyzer_loudness_monitor::{
    LoudnessInfo, LoudnessMonitor, LoudnessMonitorPlugin,
};
pub use sotf_host::analyzer_spectrum::{
    SpectralTiltCorrection, SpectrumAnalyzer, SpectrumAnalyzerPlugin, SpectrumConfig, SpectrumInfo,
    TiltReferenceFreq,
};
pub use sotf_host::auto_gain::{AutoGain, AutoGainData, AutoGainLoudnessType, AutoGainParams};
pub use sotf_host::host::{DawHost, GraphEdge, Host};
pub use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
pub use sotf_host::plugin::{
    InPlacePlugin, InPlacePluginAdapter, Plugin, PluginInfo, PluginResult, ProcessContext,
};

#[cfg(feature = "qa")]
pub use sotf_host::test_utils::benchmark_plugin_full;
#[cfg(any(feature = "qa", test, debug_assertions))]
pub use sotf_host::test_utils::{
    BufferComparison, CountingAlloc, PerformanceProfiler, SignalGen, assert_no_allocs,
    detect_latency, generate_dc, measure_peak_db, measure_rms_db, run_standard_tests,
    test_parameter_ramp, test_varied_buffer_sizes,
};

pub use sotf_host::simd::enable_ftz_daz;
pub use sotf_host::sofa::{HrtfData, SofaFile, SourcePosition};
pub use sotf_host::speaker_config::{
    SpeakerPosition, get_meter_groups, get_meter_groups_by_channels, get_speaker_config_by_channels,
};

pub type PluginHost = DawHost;

#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_input::{HalInputPlugin, HalInputPluginParams};
#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_output::{HalOutputPlugin, HalOutputPluginParams};
