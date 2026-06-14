#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::sandboxed_plugin_creation_options::SandboxedPluginCreationOptions;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::sync::{OnceLock, RwLock};

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) const MAX_EXTERNAL_PLUGIN_DEADLINE_MICROS: u64 = 10_000;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) static DEFAULT_SANDBOXED_PLUGIN_CREATION_OPTIONS: OnceLock<
    RwLock<Option<SandboxedPluginCreationOptions>>,
> = OnceLock::new();

/// Plugin type strings accepted by [`create_plugin`].
pub const SUPPORTED_PLUGIN_TYPES: &[&str] = &[
    "gain",
    "eq",
    "parametric_eq",
    "compressor",
    "expander",
    "limiter",
    "gate",
    "delay",
    "convolution",
    "upmixer",
    "aae",
    "active_acoustic_enhancement",
    "downmix",
    "mono_to_stereo",
    "multiband_compressor",
    "multiband_expander",
    "de_esser",
    "dynamic_eq",
    "fir_designer",
    "linear_phase_eq",
    "spectral_compressor",
    "stereo_imager",
    "transient_shaper",
    "saturation",
    "loudness_compensation",
    "fletcher_munson",
    "crossfeed",
    "xtc",
    "crosstalk_cancellation",
    "denoiser",
    "wiener_denoiser",
    "speech_denoiser",
    "rnnoise",
    "rnnoise_denoiser",
    "hiss_reducer",
    "hiss",
    "declick",
    "transient_repair",
    "pnd",
    "varispeed",
    "binaural_decoder",
    "crossover",
    "matrix",
    "channel_mute_solo",
    "loudness_monitor",
    "spectrum_analyzer",
    "resampler",
    "band_split",
    "band_merge",
    "ab_compare",
    "ab",
    "aec",
    "beamformer",
    "ambisonics_decoder",
    "external",
    "external_plugin",
    #[cfg(all(target_os = "macos", feature = "hal"))]
    "hal_input",
    #[cfg(all(target_os = "macos", feature = "hal"))]
    "hal_output",
];
