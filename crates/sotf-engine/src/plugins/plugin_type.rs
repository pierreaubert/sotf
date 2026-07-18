use super::release_channel::ReleaseChannel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginType {
    EQ,
    Gain,
    Upmixer,
    AAE,
    Compressor,
    Limiter,
    Gate,
    Expander,
    MultibandCompressor,
    MultibandExpander,
    LoudnessCompensation,
    FletcherMunson,
    BinauralDecoder,
    Convolution,
    LoudnessMonitor,
    SpectrumAnalyzer,
    ChannelMuteSolo,
    Matrix,
    XTC,
    Denoiser,
    Declick,
    HissReducer,
    SpeechDenoiser,
    Pnd,
    ABCompare,
    Crossover,
    BandSplit,
    BandMerge,
    Downmix,
    MonoToStereo,
    Crossfeed,
    Delay,
    Aec,
    Beamformer,
    AmbisonicsDecoder,
    StereoImager,
    DeEsser,
    TransientShaper,
    Saturation,
    DynamicEq,
    FirDesigner,
    LinearPhaseEq,
    SpectralCompressor,
}

impl PluginType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::EQ => "EQ",
            Self::Gain => "Gain",
            Self::Upmixer => "Upmixer",
            Self::AAE => "AAE",
            Self::Compressor => "Compressor",
            Self::Gate => "Gate",
            Self::Limiter => "Limiter",
            Self::Expander => "Expander",
            Self::MultibandCompressor => "Multiband Compressor",
            Self::MultibandExpander => "Multiband Expander",
            Self::LoudnessCompensation => "Loudness Compensation",
            Self::FletcherMunson => "Fletcher-Munson",
            Self::BinauralDecoder => "Binaural Decoder",
            Self::Convolution => "Convolution",
            Self::LoudnessMonitor => "Loudness Monitor",
            Self::SpectrumAnalyzer => "Spectrum Analyzer",
            Self::ChannelMuteSolo => "Channel Mute/Solo",
            Self::Matrix => "Matrix Mixer",
            Self::XTC => "Crosstalk Cancellation",
            Self::Denoiser => "Denoiser",
            Self::Declick => "Declick",
            Self::HissReducer => "Hiss Reducer",
            Self::SpeechDenoiser => "Speech Denoiser",
            Self::Pnd => "PND Varispeed",
            Self::ABCompare => "A/B Compare",
            Self::Crossover => "Crossover",
            Self::BandSplit => "Band Split",
            Self::BandMerge => "Band Merge",
            Self::Downmix => "Downmix",
            Self::MonoToStereo => "Mono to Stereo",
            Self::Crossfeed => "Crossfeed",
            Self::Delay => "Delay",
            Self::Aec => "AEC",
            Self::Beamformer => "Beamformer",
            Self::AmbisonicsDecoder => "Ambisonics Decoder",
            Self::StereoImager => "Stereo Imager",
            Self::DeEsser => "De-Esser",
            Self::TransientShaper => "Transient Shaper",
            Self::Saturation => "Saturation",
            Self::DynamicEq => "Dynamic EQ",
            Self::FirDesigner => "FIR Designer",
            Self::LinearPhaseEq => "Linear-Phase EQ",
            Self::SpectralCompressor => "Spectral Compressor",
        }
    }

    /// Wire / serde-friendly identifier used in `PluginConfig.plugin_type` and the factory.
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::EQ => "eq",
            Self::Gain => "gain",
            Self::Upmixer => "upmixer",
            Self::AAE => "aae",
            Self::Compressor => "compressor",
            Self::Limiter => "limiter",
            Self::Gate => "gate",
            Self::Expander => "expander",
            Self::MultibandCompressor => "multiband_compressor",
            Self::MultibandExpander => "multiband_expander",
            Self::LoudnessCompensation => "loudness_compensation",
            Self::FletcherMunson => "fletcher_munson",
            Self::BinauralDecoder => "binaural_decoder",
            Self::Convolution => "convolution",
            Self::LoudnessMonitor => "loudness_monitor",
            Self::SpectrumAnalyzer => "spectrum_analyzer",
            Self::ChannelMuteSolo => "channel_mute_solo",
            Self::Matrix => "matrix",
            Self::XTC => "xtc",
            Self::Denoiser => "denoiser",
            Self::Declick => "declick",
            Self::HissReducer => "hiss_reducer",
            Self::SpeechDenoiser => "speech_denoiser",
            Self::Pnd => "pnd",
            Self::ABCompare => "ab_compare",
            Self::Crossover => "crossover",
            Self::BandSplit => "band_split",
            Self::BandMerge => "band_merge",
            Self::Downmix => "downmix",
            Self::MonoToStereo => "mono_to_stereo",
            Self::Crossfeed => "crossfeed",
            Self::Delay => "delay",
            Self::Aec => "aec",
            Self::Beamformer => "beamformer",
            Self::AmbisonicsDecoder => "ambisonics_decoder",
            Self::StereoImager => "stereo_imager",
            Self::DeEsser => "de_esser",
            Self::TransientShaper => "transient_shaper",
            Self::Saturation => "saturation",
            Self::DynamicEq => "dynamic_eq",
            Self::FirDesigner => "fir_designer",
            Self::LinearPhaseEq => "linear_phase_eq",
            Self::SpectralCompressor => "spectral_compressor",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::EQ => "Parametric Equalizer IIR",
            Self::Gain => "Simple Volume/Gain Control",
            Self::Upmixer => "Stereo to Surround 5.1 to 9.1.6",
            Self::AAE => "Active Acoustic Enhancement (LARES-inspired reverb)",
            Self::Compressor => "Dynamic Range Compressor",
            Self::Limiter => "Peak Limiter",
            Self::Gate => "Noise Gate",
            Self::Expander => "Dynamic Range Expander with Hysteresis",
            Self::MultibandCompressor => "Multiband Dynamic Range Compressor",
            Self::MultibandExpander => "Multiband Dynamic Range Expander",
            Self::LoudnessCompensation => "Equal Loudness Compensation",
            Self::FletcherMunson => "Volume-dependent ISO 226 loudness curves",
            Self::BinauralDecoder => "Multi-channel to Binaural (HRTF)",
            Self::Convolution => "FFT-based Convolution (IR Processing)",
            Self::LoudnessMonitor => "Real-time EBU R128 loudness monitoring",
            Self::SpectrumAnalyzer => "Real-time frequency spectrum analysis",
            Self::ChannelMuteSolo => "Mute or solo individual channels",
            Self::Matrix => "Channel routing and mixing matrix",
            Self::XTC => "Crosstalk cancellation for speaker playback",
            Self::Denoiser => "Wiener filter denoiser with MCRA noise estimation",
            Self::Declick => "Time-domain click and transient repair",
            Self::HissReducer => "Stationary high-frequency hiss reducer",
            Self::SpeechDenoiser => "RNNoise voice denoiser",
            Self::Pnd => "Polyphonic note detection and varispeed correction",
            Self::ABCompare => "A/B comparison with auto-gain loudness matching",
            Self::Crossover => "Linkwitz-Riley / linear-phase crossover",
            Self::BandSplit => "Split audio into low/high frequency bands",
            Self::BandMerge => "Merge frequency bands back together",
            Self::Downmix => "Phase-coherent surround to stereo downmix",
            Self::MonoToStereo => "Convert mono signal to pseudo-stereo",
            Self::Crossfeed => "Headphone crossfeed for speaker-like listening",
            Self::Delay => "Simple delay effect with feedback",
            Self::Aec => "Acoustic Echo Cancellation (PBFDAF + Two-Path + Post-Filter)",
            Self::Beamformer => "Microphone array beamformer (MVDR / Superdirective / GSC)",
            Self::AmbisonicsDecoder => "HOA Ambisonics decoder (AllRAD) to speaker layouts",
            Self::StereoImager => "Multi-band M/S stereo width control",
            Self::DeEsser => "Sibilance reduction via bandpass detection and compression",
            Self::TransientShaper => "SPL Transient Designer — attack/sustain shaping",
            Self::Saturation => "Harmonic saturation / exciter with multiple modes",
            Self::DynamicEq => "Frequency-selective dynamics (hybrid EQ + compressor)",
            Self::FirDesigner => "FIR magnitude and phase design",
            Self::LinearPhaseEq => "Parametric EQ with linear-phase FIR convolution",
            Self::SpectralCompressor => {
                "Per-bin FFT dynamics processor for surgical spectral compression"
            }
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::EQ,
            Self::Gain,
            Self::Upmixer,
            Self::AAE,
            Self::Compressor,
            Self::Limiter,
            Self::Gate,
            Self::Expander,
            Self::MultibandCompressor,
            Self::MultibandExpander,
            Self::LoudnessCompensation,
            Self::FletcherMunson,
            Self::BinauralDecoder,
            Self::Convolution,
            Self::LoudnessMonitor,
            Self::SpectrumAnalyzer,
            Self::ChannelMuteSolo,
            Self::Matrix,
            Self::XTC,
            Self::Denoiser,
            Self::Declick,
            Self::HissReducer,
            Self::SpeechDenoiser,
            Self::Pnd,
            Self::ABCompare,
            Self::Crossover,
            Self::BandSplit,
            Self::BandMerge,
            Self::Downmix,
            Self::MonoToStereo,
            Self::Crossfeed,
            Self::Delay,
            Self::Aec,
            Self::Beamformer,
            Self::AmbisonicsDecoder,
            Self::StereoImager,
            Self::DeEsser,
            Self::TransientShaper,
            Self::Saturation,
            Self::DynamicEq,
            Self::FirDesigner,
            Self::LinearPhaseEq,
            Self::SpectralCompressor,
        ]
    }

    /// Parse a plugin type from its name or serde variant (case-insensitive).
    ///
    /// Accepts both human names (e.g. `"Loudness Compensation"`) and short
    /// serde names (e.g. `"loudnesscompensation"`, `"eq"`, `"EQ"`).
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        Self::all()
            .into_iter()
            .find(|pt| pt.name().to_lowercase() == lower)
            .or_else(|| {
                // Also try matching with spaces/hyphens/underscores stripped
                let normalized = lower.replace([' ', '-', '_'], "");
                Self::all().into_iter().find(|pt| {
                    let variant = format!("{:?}", pt).to_lowercase();
                    variant == normalized
                })
            })
    }

    /// Returns true if this is a monitoring/analyzer plugin (non-processing)
    pub fn is_monitoring(&self) -> bool {
        matches!(
            self,
            Self::LoudnessMonitor | Self::SpectrumAnalyzer | Self::ChannelMuteSolo
        )
    }

    /// Returns the maturity level of this plugin type.
    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            Self::EQ
            | Self::Gain
            | Self::Compressor
            | Self::ChannelMuteSolo
            | Self::Crossfeed
            | Self::Delay
            | Self::Expander
            | Self::FletcherMunson
            | Self::Gate
            | Self::Limiter
            | Self::LoudnessMonitor
            | Self::Matrix
            | Self::MultibandCompressor
            | Self::MultibandExpander
            | Self::SpectrumAnalyzer
            | Self::Upmixer
            | Self::XTC => ReleaseChannel::Prod,

            Self::AAE
            | Self::ABCompare
            | Self::Crossover
            | Self::BandSplit
            | Self::BandMerge
            | Self::Downmix
            | Self::LoudnessCompensation
            | Self::MonoToStereo
            | Self::StereoImager
            | Self::DeEsser
            | Self::TransientShaper
            | Self::Saturation
            | Self::DynamicEq
            | Self::FirDesigner
            | Self::LinearPhaseEq => ReleaseChannel::Beta,

            Self::BinauralDecoder
            | Self::Convolution
            | Self::Pnd
            | Self::Denoiser
            | Self::Declick
            | Self::HissReducer
            | Self::SpeechDenoiser
            | Self::Aec
            | Self::Beamformer
            | Self::AmbisonicsDecoder
            | Self::SpectralCompressor => ReleaseChannel::Alpha,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_app_plugin_type_has_a_canonical_factory_catalog_entry() {
        let mut missing = Vec::new();
        for plugin_type in PluginType::all() {
            let wire_name = plugin_type.wire_name();
            match sotf_plugins::catalog_entry(wire_name) {
                Some(entry) if entry.canonical_type == wire_name => {}
                Some(entry) => missing.push(format!(
                    "{wire_name} resolves to non-canonical entry {}",
                    entry.canonical_type
                )),
                None => missing.push(format!("{wire_name} is absent from the plugin catalog")),
            }
        }
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }
}
