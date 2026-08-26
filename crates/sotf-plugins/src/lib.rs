//! ============================================================================
//! SOTF Audio Plugins Library
//! ============================================================================
//!
//! This is the facade crate that re-exports shared infrastructure from `sotf-host`
//! and all plugin implementations from their individual crates.

// Re-export the serde_param_default macro from sotf-host
pub use sotf_host::serde_param_default;

// Shared plugin factory
pub mod factory;
pub use factory::{
    EVEN_BAND_CHANNEL_WIDTHS, ExternalReferenceImplementation, FIRST_ORDER_AMBISONIC_WIDTH,
    MICROPHONE_ARRAY_WIDTHS, MONO_CHANNEL_WIDTH, PLUGIN_CATALOG, PluginCatalogEntry,
    PluginCatalogMetadata, PluginCategory, PluginChannelLayoutContract, PluginChannelOutputModel,
    PluginDefaultChannelOutput, PluginLatencyModel, PluginMaturity, PluginParameterSchema,
    PluginPickerExposure, PluginPresetSupport, PluginStabilityEvidence, PluginStabilitySummary,
    PluginSupportedInputLayouts, PluginUiKind, STANDARD_CHANNEL_WIDTHS, STEREO_CHANNEL_WIDTH,
    StabilityEvidenceState, ab_compare_catalog_entries, catalog_entry, create_plugin,
    generic_app_catalog_entries, is_supported_plugin_type, plugin_stability_summary,
    supported_plugin_types, validate_plugin_security_config,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use factory::{
    SandboxedPluginCreationOptions, create_plugin_with_sandbox_grants,
    create_plugin_with_sandbox_grants_for_backend,
    create_plugin_with_sandbox_grants_for_backend_and_launcher, create_plugin_with_sandbox_options,
    default_sandboxed_plugin_creation_options, set_default_sandboxed_plugin_creation_options,
};

// Re-export infrastructure modules from sotf-host
pub use sotf_host::analyzer;
pub use sotf_host::analyzer_loudness_monitor;
pub use sotf_host::analyzer_spectrum;
pub use sotf_host::auto_gain;
pub use sotf_host::automation;
pub use sotf_host::error;
pub use sotf_host::layout_solver;
pub use sotf_host::param_registry;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use sotf_host::{
    DenyPluginSandboxPermissionBroker, ExternalPluginProcessEvent, ExternalPluginSandboxPolicy,
    ExternalPluginSandboxStatus, ExternalPluginSandboxTiming, ExternalPluginTrust,
    ExternalPluginWorkerCommand, IsolatedExternalPlugin, IsolatedExternalPluginConfig,
    IsolatedExternalPluginWorkerReport, PluginSandboxAuthorizationGrant, PluginSandboxBackend,
    PluginSandboxBackendCapabilities, PluginSandboxBackendCode, PluginSandboxBrokerPolicy,
    PluginSandboxChildProcessGrant, PluginSandboxFileGrant, PluginSandboxGrantPersistence,
    PluginSandboxGrantStore, PluginSandboxIdentity, PluginSandboxLaunchBackend,
    PluginSandboxLaunchPlan, PluginSandboxLifecycleMode, PluginSandboxNetworkGrant,
    PluginSandboxPermission, PluginSandboxPermissionBroker, PluginSandboxPermissionDecision,
    PluginSandboxPermissionOutcome, PluginSandboxPermissionRequest, PluginSandboxPolicy,
    PluginSandboxPolicyAdapterIssue, PluginSandboxPolicySupportIssue, PluginSandboxStatusCode,
    PluginSandboxUserGrant, current_plugin_sandbox_backend_capabilities,
    current_plugin_sandbox_launch_backend, current_plugin_sandbox_launcher_command,
    default_plugin_sandbox_launcher_command_for_backend,
    default_plugin_sandbox_protected_media_paths, enter_external_plugin_sandbox,
};
pub use sotf_host::{
    EXTERNAL_PLUGIN_INSTANCE_ID_PARAMETER, ExternalHostingBackend, ExternalPlugin,
    ExternalPluginSandboxMode, ExternalPluginState, ParameterEventSender, PluginDescriptor,
    PluginFormat, PluginFormatCapability, PluginScanStatus, PluginScanner,
    plugin_format_capabilities,
};
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
    pub mod aae {
        pub use sotf_plugin_aae::params::*;
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
    pub mod resampler {
        pub use sotf_plugin_resampler::params::*;
    }
    pub mod downmix {
        pub use sotf_plugin_downmix::params::*;
    }
    pub mod mono_to_stereo {
        pub use sotf_plugin_mono_to_stereo::params::*;
    }
    pub mod compressor {
        pub use sotf_plugin_multiband_compressor::params::{
            DETECTION_MODES, GLOBAL_PARAMS, HPF_ORDERS, LAYOUT, PARAMS, SINGLE_BAND_LAYOUT,
        };
    }
    pub mod expander {
        pub use sotf_plugin_multiband_expander::params::{
            DETECTION_MODES, GLOBAL_PARAMS, HPF_ORDERS, LAYOUT, PARAMS, SINGLE_BAND_LAYOUT,
        };
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
    pub mod crossover {
        pub use sotf_plugin_crossover::params::*;
    }
    pub mod xtc {
        pub use sotf_plugin_xtc::params::*;
    }
    pub mod ab_compare {
        pub use sotf_plugin_ab_compare::params::*;
    }
    /// Backward compat: Fletcher-Munson params now live in loudness_compensation.
    pub mod fletcher_munson {
        pub use sotf_plugin_loudness_compensation::params::*;
    }
    pub mod upmixer {
        pub use sotf_plugin_upmixer::params::*;
    }
    pub mod de_esser {
        pub use sotf_plugin_de_esser::params::*;
    }
    pub mod dynamic_eq {
        pub use sotf_plugin_dynamic_eq::params::*;
    }
    pub mod denoiser {
        pub use sotf_plugin_denoiser::params::*;
    }
    pub mod speech_denoiser {
        pub use sotf_plugin_speech_denoiser::params::*;
    }
    pub mod hiss_reducer {
        pub use sotf_plugin_hiss_reducer::params::*;
    }
    pub mod declick {
        pub use sotf_plugin_declick::params::*;
    }
    pub mod dither {
        pub use sotf_plugin_dither::params::*;
    }
    pub mod eq {
        pub use sotf_plugin_eq::params::*;
    }
    pub mod fir_designer {
        //! Compatibility alias for the consolidated FIR EQ parameter schema.
        pub use sotf_plugin_linear_phase_eq::params::*;
    }
    pub mod linear_phase_eq {
        pub use sotf_plugin_linear_phase_eq::params::*;
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
    pub mod saturation {
        pub use sotf_plugin_saturation::params::*;
    }
    pub mod spectral_compressor {
        pub use sotf_plugin_spectral_compressor::params::*;
    }
    pub mod stereo_imager {
        pub use sotf_plugin_stereo_imager::params::*;
    }
    pub mod transient_shaper {
        pub use sotf_plugin_transient_shaper::params::*;
    }
    pub mod channel_mute_solo {
        pub use sotf_plugin_channel_mute_solo::params::*;
    }

    /// Return the canonical parameter specifications for a built-in plugin.
    ///
    /// Consumers which can only identify a plugin by its persisted factory
    /// name (for example A/B path editors) should use this instead of keeping
    /// a second, potentially stale mapping of parameter ranges and defaults.
    /// Plugins whose parameters are discovered dynamically return an empty
    /// slice; unknown and external plugin types return `None`.
    pub fn for_plugin_type(plugin_type: &str) -> Option<&'static [ParamSpec]> {
        let specs = match plugin_type {
            "EQ" | "eq" => eq::GLOBAL_PARAMS,
            "Compressor" | "compressor" => compressor::PARAMS,
            "Limiter" | "limiter" => limiter::PARAMS,
            "Gate" | "gate" => gate::PARAMS,
            "Gain" | "gain" => gain::PARAMS,
            "Expander" | "expander" => expander::PARAMS,
            "Crossfeed" | "crossfeed" => crossfeed::PARAMS,
            "FletcherMunson"
            | "fletcher_munson"
            | "LoudnessCompensation"
            | "loudness_compensation" => loudness_compensation::PARAMS,
            "MultibandCompressor" | "multiband_compressor" => multiband_compressor::GLOBAL_PARAMS,
            "MultibandExpander" | "multiband_expander" => multiband_expander::GLOBAL_PARAMS,
            "Upmixer" | "upmixer" => upmixer::PARAMS,
            "AAE" | "aae" => aae::PARAMS,
            "XTC" | "xtc" => xtc::PARAMS,
            "Binaural" | "binaural" | "BinauralDecoder" | "binaural_decoder" => binaural::PARAMS,
            "ChannelMuteSolo" | "channel_mute_solo" => channel_mute_solo::PARAMS,
            "Convolution" | "convolution" => convolution::PARAMS,
            "ABCompare" | "ab_compare" => ab_compare::PARAMS,
            "MonoToStereo" | "mono_to_stereo" => mono_to_stereo::PARAMS,
            "PND" | "pnd" => pnd::PARAMS,
            "Denoiser" | "denoiser" => denoiser::PARAMS,
            "SpeechDenoiser" | "speech_denoiser" | "RNNoise" | "rnnoise" => speech_denoiser::PARAMS,
            "HissReducer" | "hiss_reducer" => hiss_reducer::PARAMS,
            "Declick" | "declick" | "TransientRepair" | "transient_repair" => declick::PARAMS,
            "Downmix" | "downmix" => downmix::PARAMS,
            "Saturation" | "saturation" => saturation::PARAMS,
            "StereoImager" | "stereo_imager" => stereo_imager::PARAMS,
            "TransientShaper" | "transient_shaper" => transient_shaper::PARAMS,
            "DeEsser" | "de_esser" => de_esser::PARAMS,
            "DynamicEQ" | "dynamic_eq" => dynamic_eq::PARAMS,
            "LinearPhaseEQ" | "linear_phase_eq" => linear_phase_eq::PARAMS,
            "Dither" | "dither" => dither::PARAMS,
            "BandSplit" | "band_split" => band_split::PARAMS,
            "BandMerge" | "band_merge" => band_merge::PARAMS,
            "AEC" | "aec" => aec::PARAMS,
            "Beamformer" | "beamformer" => beamformer::PARAMS,
            "SpectralCompressor" | "spectral_compressor" => spectral_compressor::PARAMS,
            "AmbisonicsDecoder" | "ambisonics_decoder" => ambisonics::PARAMS,
            "Delay" | "delay" | "Matrix" | "matrix" | "Crossover" | "crossover"
            | "LoudnessMonitor" | "loudness_monitor" | "SpectrumAnalyzer" | "spectrum_analyzer" => {
                &[]
            }
            _ => return None,
        };
        Some(specs)
    }

    #[cfg(test)]
    mod tests {
        use super::for_plugin_type;

        #[test]
        fn resolves_persisted_plugin_aliases_without_panicking() {
            assert!(for_plugin_type("gain").is_some_and(|specs| !specs.is_empty()));
            assert!(for_plugin_type("Gain").is_some_and(|specs| !specs.is_empty()));
            assert!(for_plugin_type("external").is_none());
        }
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
#[cfg(any(feature = "qa", debug_assertions))]
pub use sotf_host::test_utils;

// Re-export all plugin crates
pub use sotf_plugin_aae as plugin_aae;
pub use sotf_plugin_ab_compare as plugin_ab_compare;
pub use sotf_plugin_aec as plugin_aec;
pub use sotf_plugin_band_merge as plugin_band_merge;
pub use sotf_plugin_band_split as plugin_band_split;
pub use sotf_plugin_beamformer as plugin_beamformer;
pub use sotf_plugin_binaural as plugin_binaural;
pub use sotf_plugin_channel_mute_solo as plugin_channel_mute_solo;
pub use sotf_plugin_convolution as plugin_convolution;
pub use sotf_plugin_crossfeed as plugin_crossfeed;
pub use sotf_plugin_crossover as plugin_crossover;
pub use sotf_plugin_de_esser as plugin_de_esser;
pub use sotf_plugin_declick as plugin_declick;
pub use sotf_plugin_delay as plugin_delay;
pub use sotf_plugin_denoiser as plugin_denoiser;
pub use sotf_plugin_dither;
pub use sotf_plugin_downmix as plugin_downmix;
pub use sotf_plugin_dynamic_eq as plugin_dynamic_eq;
pub use sotf_plugin_eq as plugin_eq;
pub use sotf_plugin_gain as plugin_gain;
pub use sotf_plugin_gate as plugin_gate;
pub use sotf_plugin_hiss_reducer as plugin_hiss_reducer;
pub use sotf_plugin_limiter as plugin_limiter;
pub use sotf_plugin_linear_phase_eq as plugin_linear_phase_eq;
/// Backward compat: Fletcher-Munson is now part of loudness_compensation.
pub use sotf_plugin_loudness_compensation as plugin_fletcher_munson;
pub use sotf_plugin_loudness_compensation as plugin_loudness_compensation;
pub use sotf_plugin_matrix as plugin_matrix;
pub use sotf_plugin_mono_to_stereo as plugin_mono_to_stereo;
pub use sotf_plugin_multiband_compressor as plugin_compressor;
pub use sotf_plugin_multiband_compressor as plugin_multiband_compressor;
pub use sotf_plugin_multiband_expander as plugin_expander;
pub use sotf_plugin_multiband_expander as plugin_multiband_expander;
pub use sotf_plugin_pnd as plugin_pnd;
pub use sotf_plugin_resampler as plugin_resampler;
pub use sotf_plugin_saturation as plugin_saturation;
pub use sotf_plugin_spectral_compressor as plugin_spectral_compressor;
pub use sotf_plugin_speech_denoiser as plugin_speech_denoiser;
pub use sotf_plugin_stereo_imager as plugin_stereo_imager;
pub use sotf_plugin_transient_shaper as plugin_transient_shaper;
pub use sotf_plugin_upmixer as plugin_upmixer;
pub use sotf_plugin_xtc as plugin_xtc;

#[cfg(all(target_os = "macos", feature = "hal"))]
pub use sotf_plugin_hal_input as plugin_hal_input;
#[cfg(all(target_os = "macos", feature = "hal"))]
pub use sotf_plugin_hal_output as plugin_hal_output;

// Re-export all public types for backward compatibility
pub use plugin_aae::{AaePlugin, params::AaePluginParams};
pub use plugin_ab_compare::{ABComparePlugin, ABComparePluginParams};
pub use plugin_aec::{AecPlugin, AecPluginParams};
pub use plugin_band_merge::{BandMergePlugin, BandMergePluginParams};
pub use plugin_band_split::{BandSplitPlugin, BandSplitPluginParams};
pub use plugin_beamformer::{BeamformerPlugin, BeamformerPluginParams, BeamformerType};
pub use plugin_binaural::{BinauralDecoderParams, BinauralDecoderPlugin, RoomModel};
pub use plugin_channel_mute_solo::{ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState};
pub type CompressorPlugin = sotf_plugin_multiband_compressor::MultibandCompressorPlugin;
pub type CompressorPluginParams = sotf_plugin_multiband_compressor::MultibandCompressorPluginParams;
pub type CompressorData = sotf_plugin_multiband_compressor::MultibandCompressorData;
pub use plugin_convolution::{ConvolutionPlugin, ConvolutionPluginParams};
pub use plugin_crossfeed::{
    CrossfeedMode, CrossfeedPlugin, CrossfeedPluginParams, CrossfeedPreset,
};
pub use plugin_crossover::{CrossoverPlugin, CrossoverPluginParams};
pub use plugin_de_esser::{DeEsserData, DeEsserPlugin, DeEsserPluginParams};
pub use plugin_declick::{DeclickPlugin, DeclickPluginParams};
pub use plugin_delay::{DelayPlugin, DelayPluginParams};
pub use plugin_denoiser::{DenoiserData, DenoiserPlugin, DenoiserPluginParams};
pub use plugin_downmix::{DownmixPlugin, DownmixPluginParams};
pub use plugin_dynamic_eq::{
    DynEqBandParams, DynamicEqData, DynamicEqPlugin, DynamicEqPluginParams,
};
pub use plugin_eq::{
    BiquadFilterConfig, EqFilterTopology, EqPlugin, EqPluginParams, KautzSectionConfig,
};
pub use plugin_linear_phase_eq::{LinearPhaseEqPlugin, LinearPhaseEqPluginParams};
pub use sotf_plugin_dither::{DitherPlugin, DitherPluginParams};
pub type ExpanderPlugin = sotf_plugin_multiband_expander::MultibandExpanderPlugin;
pub type ExpanderPluginParams = sotf_plugin_multiband_expander::MultibandExpanderPluginParams;
pub use plugin_gain::{GainPlugin, GainPluginParams};
pub use plugin_gate::{GateData, GatePlugin, GatePluginParams};
pub use plugin_hiss_reducer::{HissReducerPlugin, HissReducerPluginParams};
pub use plugin_limiter::{LimiterData, LimiterPlugin, LimiterPluginParams};
pub use plugin_loudness_compensation::{FletcherMunsonPlugin, FletcherMunsonPluginParams};
pub use plugin_loudness_compensation::{
    LoudnessCompensation, LoudnessCompensationPlugin, LoudnessCompensationPluginParams,
};
pub use plugin_matrix::MatrixPlugin;
pub use plugin_mono_to_stereo::{MonoToStereoPlugin, MonoToStereoPluginParams};
pub use plugin_multiband_compressor::{
    BandCompressorParams, MultibandCompressorPlugin, MultibandCompressorPluginParams,
};
pub use plugin_multiband_expander::{
    BandExpanderParams, MultibandExpanderData, MultibandExpanderPlugin,
    MultibandExpanderPluginParams,
};
pub use plugin_pnd::{PndPlugin, PndPluginParams};
pub use plugin_resampler::ResamplerPlugin;
pub use plugin_saturation::{SaturationPlugin, SaturationPluginParams};
pub use plugin_spectral_compressor::{SpectralCompressorPlugin, SpectralCompressorPluginParams};
pub use plugin_speech_denoiser::{
    SPEECH_DENOISER_FRAME_SIZE, SpeechDenoiserPlugin, SpeechDenoiserPluginParams,
};
pub use plugin_stereo_imager::{StereoImagerPlugin, StereoImagerPluginParams};
pub use plugin_transient_shaper::{
    TransientShaperData, TransientShaperPlugin, TransientShaperPluginParams,
};
pub use plugin_upmixer::{
    UpmixerPlugin, UpmixerPluginBypassParams, UpmixerPluginCoreParams,
    UpmixerPluginDecorrelationParams, UpmixerPluginDialogueParams, UpmixerPluginGainsParams,
    UpmixerPluginHeightParams, UpmixerPluginMlParams, UpmixerPluginOutputParams,
    UpmixerPluginParams, UpmixerPluginSpectralParams, UpmixerPluginSubharmonicParams,
    UpmixerPluginSurroundParams, default_hr_sharpen as upmixer_default_hr_sharpen,
    default_safety_cap_db as upmixer_default_safety_cap_db,
    default_subharmonic_gain as upmixer_default_subharmonic_gain,
};
pub use plugin_xtc::{XtcPlugin, XtcPluginParams, validation};
// Shared audio utility functions and constants
pub use sotf_host::{
    AUDIBLE_MAX_FREQ, AUDIBLE_MIN_FREQ, DEFAULT_PREVIEW_SAMPLE_RATE, db_to_linear, linear_to_db,
};

pub use sotf_host::analyzer::{
    AnalyzerData, CorrelationData, IntegratedLoudnessMode, LoudnessData, LoudnessQueryError,
    SpectrumData,
};
pub use sotf_host::analyzer_channel_correlation::{
    ChannelCorrelationMonitor, ChannelCorrelationPlugin,
};
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
    InPlacePlugin, InPlacePluginAdapter, Plugin, PluginCostClass, PluginInfo, PluginResult,
    ProcessContext,
};
#[cfg(feature = "qa")]
pub use sotf_host::test_utils::benchmark_plugin_full;
#[cfg(any(feature = "qa", debug_assertions))]
pub use sotf_host::test_utils::{
    BufferComparison, CountingAlloc, PerformanceProfiler, SignalGen, assert_no_allocs,
    detect_latency, generate_dc, measure_peak_db, measure_rms_db, run_standard_tests,
    test_parameter_ramp, test_varied_buffer_sizes,
};
pub use sotf_host::{
    ParametricInPlacePlugin, ParametricInPlacePluginAdapter, ParametricPlugin,
    ParametricPluginAdapter,
};

pub use sotf_host::simd::enable_ftz_daz;
pub use sotf_host::sofa::{HrtfData, SofaFile, SourcePosition};
pub use sotf_host::speaker_config::{
    ChannelAssignment, ChannelLayout, ChannelRole, SpeakerPosition, get_meter_groups,
    get_meter_groups_by_channels, get_speaker_config_by_channels,
};

pub type PluginHost = DawHost;

#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_input::{HalInputPlugin, HalInputPluginParams};
#[cfg(all(target_os = "macos", feature = "hal"))]
pub use plugin_hal_output::{HalOutputPlugin, HalOutputPluginParams};
