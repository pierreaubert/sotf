//! VST3/CLAP plugin wrappers for SOTF audio plugins via nih-plug.
//!
//! One cdylib per plugin, selected by feature flag. The Justfile builds all 29
//! sequentially, each producing a separate `.dylib` that exports both VST3 and CLAP.
//!
//! ```bash
//! cargo build --release -p plugins-nih --features eq --no-default-features
//! ```

pub mod params;
#[macro_use]
pub mod wrapper;

/// Wrapper for ParamBridge (unused fields reserved for future param normalization).
pub struct PluginBridgeWrapper {
    _bridge: plugins_bridge::param_bridge::ParamBridge,
}

impl PluginBridgeWrapper {
    pub fn new(bridge: plugins_bridge::param_bridge::ParamBridge) -> Self {
        Self { _bridge: bridge }
    }

    pub fn sync_params_to_plugin(
        &self,
        params: &std::sync::Arc<params::DynamicParams>,
        plugin: &mut dyn sotf_host::plugin::Plugin,
    ) {
        params.sync_to_plugin(plugin);
    }
}

// ── Plugin definitions (one per feature) ─────────────────────────────────────
// Each feature gate enables exactly one plugin struct + VST3/CLAP export.
// Only one feature should be active per build.

#[cfg(feature = "eq")]
mod plugin {
    sotf_nih_plugin!(SotfEQ, plugin_type: "EQ", name: "SOTF: Parametric EQ", clap_id: "org.spinorama.sotf.eq", vst3_class_id: *b"SotfEqPlugin0001", channels: 2);
    nih_plug::nih_export_clap!(SotfEQ);
    nih_plug::nih_export_vst3!(SotfEQ);
}

#[cfg(feature = "compressor")]
mod plugin {
    sotf_nih_plugin!(SotfCompressor, plugin_type: "Compressor", name: "SOTF: Compressor", clap_id: "org.spinorama.sotf.compressor", vst3_class_id: *b"SotfCmprssor0001", channels: 2);
    nih_plug::nih_export_clap!(SotfCompressor);
    nih_plug::nih_export_vst3!(SotfCompressor);
}

#[cfg(feature = "limiter")]
mod plugin {
    sotf_nih_plugin!(SotfLimiter, plugin_type: "Limiter", name: "SOTF: Limiter", clap_id: "org.spinorama.sotf.limiter", vst3_class_id: *b"SotfLimiter00001", channels: 2);
    nih_plug::nih_export_clap!(SotfLimiter);
    nih_plug::nih_export_vst3!(SotfLimiter);
}

#[cfg(feature = "gate")]
mod plugin {
    sotf_nih_plugin!(SotfGate, plugin_type: "Gate", name: "SOTF: Gate", clap_id: "org.spinorama.sotf.gate", vst3_class_id: *b"SotfGate00000001", channels: 2);
    nih_plug::nih_export_clap!(SotfGate);
    nih_plug::nih_export_vst3!(SotfGate);
}

#[cfg(feature = "gain")]
mod plugin {
    sotf_nih_plugin!(SotfGain, plugin_type: "Gain", name: "SOTF: Gain", clap_id: "org.spinorama.sotf.gain", vst3_class_id: *b"SotfGain00000001", channels: 2);
    nih_plug::nih_export_clap!(SotfGain);
    nih_plug::nih_export_vst3!(SotfGain);
}

#[cfg(feature = "delay")]
mod plugin {
    sotf_nih_plugin!(SotfDelay, plugin_type: "Delay", name: "SOTF: Delay", clap_id: "org.spinorama.sotf.delay", vst3_class_id: *b"SotfDelay0000001", channels: 2);
    nih_plug::nih_export_clap!(SotfDelay);
    nih_plug::nih_export_vst3!(SotfDelay);
}

#[cfg(feature = "expander")]
mod plugin {
    sotf_nih_plugin!(SotfExpander, plugin_type: "Expander", name: "SOTF: Expander", clap_id: "org.spinorama.sotf.expander", vst3_class_id: *b"SotfExpander0001", channels: 2);
    nih_plug::nih_export_clap!(SotfExpander);
    nih_plug::nih_export_vst3!(SotfExpander);
}

#[cfg(feature = "crossfeed")]
mod plugin {
    sotf_nih_plugin!(SotfCrossfeed, plugin_type: "Crossfeed", name: "SOTF: Crossfeed", clap_id: "org.spinorama.sotf.crossfeed", vst3_class_id: *b"SotfCrossfeed001", channels: 2);
    nih_plug::nih_export_clap!(SotfCrossfeed);
    nih_plug::nih_export_vst3!(SotfCrossfeed);
}

#[cfg(feature = "saturation")]
mod plugin {
    sotf_nih_plugin!(SotfSaturation, plugin_type: "Saturation", name: "SOTF: Saturation", clap_id: "org.spinorama.sotf.saturation", vst3_class_id: *b"SotfSaturtn00001", channels: 2);
    nih_plug::nih_export_clap!(SotfSaturation);
    nih_plug::nih_export_vst3!(SotfSaturation);
}

#[cfg(feature = "denoiser")]
mod plugin {
    sotf_nih_plugin!(SotfDenoiser, plugin_type: "Denoiser", name: "SOTF: Denoiser", clap_id: "org.spinorama.sotf.denoiser", vst3_class_id: *b"SotfDenoiser0001", channels: 2);
    nih_plug::nih_export_clap!(SotfDenoiser);
    nih_plug::nih_export_vst3!(SotfDenoiser);
}

#[cfg(feature = "downmix")]
mod plugin {
    sotf_nih_plugin!(SotfDownmix, plugin_type: "Downmix", name: "SOTF: Downmix", clap_id: "org.spinorama.sotf.downmix", vst3_class_id: *b"SotfDownmix00001", channels: 2);
    nih_plug::nih_export_clap!(SotfDownmix);
    nih_plug::nih_export_vst3!(SotfDownmix);
}

#[cfg(feature = "mono-to-stereo")]
mod plugin {
    sotf_nih_plugin!(SotfMonoToStereo, plugin_type: "MonoToStereo", name: "SOTF: Mono to Stereo", clap_id: "org.spinorama.sotf.mono-to-stereo", vst3_class_id: *b"SotfMono2Str0001", channels: 2);
    nih_plug::nih_export_clap!(SotfMonoToStereo);
    nih_plug::nih_export_vst3!(SotfMonoToStereo);
}

#[cfg(feature = "stereo-imager")]
mod plugin {
    sotf_nih_plugin!(SotfStereoImager, plugin_type: "StereoImager", name: "SOTF: Stereo Imager", clap_id: "org.spinorama.sotf.stereo-imager", vst3_class_id: *b"SotfStereoIm0001", channels: 2);
    nih_plug::nih_export_clap!(SotfStereoImager);
    nih_plug::nih_export_vst3!(SotfStereoImager);
}

#[cfg(feature = "transient-shaper")]
mod plugin {
    sotf_nih_plugin!(SotfTransientShaper, plugin_type: "TransientShaper", name: "SOTF: Transient Shaper", clap_id: "org.spinorama.sotf.transient-shaper", vst3_class_id: *b"SotfTransient001", channels: 2);
    nih_plug::nih_export_clap!(SotfTransientShaper);
    nih_plug::nih_export_vst3!(SotfTransientShaper);
}

#[cfg(feature = "de-esser")]
mod plugin {
    sotf_nih_plugin!(SotfDeEsser, plugin_type: "DeEsser", name: "SOTF: De-Esser", clap_id: "org.spinorama.sotf.de-esser", vst3_class_id: *b"SotfDeEsser00001", channels: 2);
    nih_plug::nih_export_clap!(SotfDeEsser);
    nih_plug::nih_export_vst3!(SotfDeEsser);
}

#[cfg(feature = "dynamic-eq")]
mod plugin {
    sotf_nih_plugin!(SotfDynamicEQ, plugin_type: "DynamicEQ", name: "SOTF: Dynamic EQ", clap_id: "org.spinorama.sotf.dynamic-eq", vst3_class_id: *b"SotfDynEq0000001", channels: 2);
    nih_plug::nih_export_clap!(SotfDynamicEQ);
    nih_plug::nih_export_vst3!(SotfDynamicEQ);
}

#[cfg(feature = "multiband-compressor")]
mod plugin {
    sotf_nih_plugin!(SotfMultibandCompressor, plugin_type: "MultibandCompressor", name: "SOTF: Multiband Compressor", clap_id: "org.spinorama.sotf.multiband-compressor", vst3_class_id: *b"SotfMBComprss001", channels: 2);
    nih_plug::nih_export_clap!(SotfMultibandCompressor);
    nih_plug::nih_export_vst3!(SotfMultibandCompressor);
}

#[cfg(feature = "multiband-expander")]
mod plugin {
    sotf_nih_plugin!(SotfMultibandExpander, plugin_type: "MultibandExpander", name: "SOTF: Multiband Expander", clap_id: "org.spinorama.sotf.multiband-expander", vst3_class_id: *b"SotfMBExpand0001", channels: 2);
    nih_plug::nih_export_clap!(SotfMultibandExpander);
    nih_plug::nih_export_vst3!(SotfMultibandExpander);
}

#[cfg(feature = "convolution")]
mod plugin {
    sotf_nih_plugin!(SotfConvolution, plugin_type: "Convolution", name: "SOTF: Convolution", clap_id: "org.spinorama.sotf.convolution", vst3_class_id: *b"SotfConvolutn001", channels: 2);
    nih_plug::nih_export_clap!(SotfConvolution);
    nih_plug::nih_export_vst3!(SotfConvolution);
}

#[cfg(feature = "fletcher-munson")]
mod plugin {
    sotf_nih_plugin!(SotfFletcherMunson, plugin_type: "FletcherMunson", name: "SOTF: Fletcher-Munson", clap_id: "org.spinorama.sotf.fletcher-munson", vst3_class_id: *b"SotfFletcherM001", channels: 2);
    nih_plug::nih_export_clap!(SotfFletcherMunson);
    nih_plug::nih_export_vst3!(SotfFletcherMunson);
}

#[cfg(feature = "loudness-compensation")]
mod plugin {
    sotf_nih_plugin!(SotfLoudnessCompensation, plugin_type: "LoudnessCompensation", name: "SOTF: Loudness Compensation", clap_id: "org.spinorama.sotf.loudness-comp", vst3_class_id: *b"SotfLoudnssC0001", channels: 2);
    nih_plug::nih_export_clap!(SotfLoudnessCompensation);
    nih_plug::nih_export_vst3!(SotfLoudnessCompensation);
}

#[cfg(feature = "channel-mute-solo")]
mod plugin {
    sotf_nih_plugin!(SotfChannelMuteSolo, plugin_type: "ChannelMuteSolo", name: "SOTF: Channel Mute/Solo", clap_id: "org.spinorama.sotf.channel-mute-solo", vst3_class_id: *b"SotfChMuteSol001", channels: 2);
    nih_plug::nih_export_clap!(SotfChannelMuteSolo);
    nih_plug::nih_export_vst3!(SotfChannelMuteSolo);
}

#[cfg(feature = "upmixer")]
mod plugin {
    sotf_nih_plugin!(SotfUpmixer, plugin_type: "Upmixer", name: "SOTF: Upmixer", clap_id: "org.spinorama.sotf.upmixer", vst3_class_id: *b"SotfUpmixer00001", channels: 2);
    nih_plug::nih_export_clap!(SotfUpmixer);
    nih_plug::nih_export_vst3!(SotfUpmixer);
}

#[cfg(feature = "xtc")]
mod plugin {
    sotf_nih_plugin!(SotfXTC, plugin_type: "XTC", name: "SOTF: Crosstalk Cancellation", clap_id: "org.spinorama.sotf.xtc", vst3_class_id: *b"SotfXTC000000001", channels: 2);
    nih_plug::nih_export_clap!(SotfXTC);
    nih_plug::nih_export_vst3!(SotfXTC);
}

#[cfg(feature = "binaural")]
mod plugin {
    sotf_nih_plugin!(SotfBinaural, plugin_type: "Binaural", name: "SOTF: Binaural", clap_id: "org.spinorama.sotf.binaural", vst3_class_id: *b"SotfBinaural0001", channels: 2);
    nih_plug::nih_export_clap!(SotfBinaural);
    nih_plug::nih_export_vst3!(SotfBinaural);
}

#[cfg(feature = "matrix")]
mod plugin {
    sotf_nih_plugin!(SotfMatrix, plugin_type: "Matrix", name: "SOTF: Channel Matrix", clap_id: "org.spinorama.sotf.matrix", vst3_class_id: *b"SotfMatrix000001", channels: 2);
    nih_plug::nih_export_clap!(SotfMatrix);
    nih_plug::nih_export_vst3!(SotfMatrix);
}

#[cfg(feature = "pnd")]
mod plugin {
    sotf_nih_plugin!(SotfPND, plugin_type: "PND", name: "SOTF: Perceptual Noise Diffusion", clap_id: "org.spinorama.sotf.pnd", vst3_class_id: *b"SotfPND000000001", channels: 2);
    nih_plug::nih_export_clap!(SotfPND);
    nih_plug::nih_export_vst3!(SotfPND);
}

#[cfg(feature = "ab-compare")]
mod plugin {
    sotf_nih_plugin!(SotfABCompare, plugin_type: "ABCompare", name: "SOTF: A/B Compare", clap_id: "org.spinorama.sotf.ab-compare", vst3_class_id: *b"SotfABCompare001", channels: 2);
    nih_plug::nih_export_clap!(SotfABCompare);
    nih_plug::nih_export_vst3!(SotfABCompare);
}

#[cfg(feature = "crossover")]
mod plugin {
    sotf_nih_plugin!(SotfCrossover, plugin_type: "Crossover", name: "SOTF: Crossover", clap_id: "org.spinorama.sotf.crossover", vst3_class_id: *b"SotfCrossover001", channels: 2);
    nih_plug::nih_export_clap!(SotfCrossover);
    nih_plug::nih_export_vst3!(SotfCrossover);
}
