use super::release_channel::ReleaseChannel;
use serde::{Deserialize, Serialize};
use sotf_plugins::{PluginMaturity, catalog_entry, generic_app_catalog_entries};

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
    #[serde(alias = "FirDesigner")]
    LinearPhaseEq,
    SpectralCompressor,
    /// A concrete external plugin. External plugins are intentionally omitted
    /// from [`PluginType::all`] because they require a discovered descriptor
    /// and therefore have no generic default settings.
    External,
}

impl PluginType {
    pub fn name(&self) -> &'static str {
        catalog_entry(self.wire_name())
            .map(|entry| entry.metadata.exposed_name)
            .expect("every PluginType must have canonical catalog metadata")
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
            Self::LinearPhaseEq => "linear_phase_eq",
            Self::SpectralCompressor => "spectral_compressor",
            Self::External => "external",
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
            Self::Pnd => "Referenced drift analysis and duration-preserving pitch correction",
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
            Self::LinearPhaseEq => "Parametric EQ with linear or minimum-phase FIR convolution",
            Self::SpectralCompressor => {
                "Per-bin FFT dynamics processor for surgical spectral compression"
            }
            Self::External => "Third-party audio plugin hosted in an isolated worker",
        }
    }

    pub fn all() -> Vec<Self> {
        generic_app_catalog_entries()
            .map(|entry| {
                Self::from_wire_name(entry.canonical_type)
                    .expect("generic catalog entries must map to PluginType")
            })
            .collect()
    }

    /// Resolve a canonical factory name or compatibility alias.
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match catalog_entry(name)?.canonical_type {
            "eq" => Some(Self::EQ),
            "gain" => Some(Self::Gain),
            "upmixer" => Some(Self::Upmixer),
            "aae" => Some(Self::AAE),
            "compressor" => Some(Self::Compressor),
            "limiter" => Some(Self::Limiter),
            "gate" => Some(Self::Gate),
            "expander" => Some(Self::Expander),
            "multiband_compressor" => Some(Self::MultibandCompressor),
            "multiband_expander" => Some(Self::MultibandExpander),
            "loudness_compensation" => Some(Self::LoudnessCompensation),
            "fletcher_munson" => Some(Self::FletcherMunson),
            "binaural_decoder" => Some(Self::BinauralDecoder),
            "convolution" => Some(Self::Convolution),
            "loudness_monitor" => Some(Self::LoudnessMonitor),
            "spectrum_analyzer" => Some(Self::SpectrumAnalyzer),
            "channel_mute_solo" => Some(Self::ChannelMuteSolo),
            "matrix" => Some(Self::Matrix),
            "xtc" => Some(Self::XTC),
            "denoiser" => Some(Self::Denoiser),
            "declick" => Some(Self::Declick),
            "hiss_reducer" => Some(Self::HissReducer),
            "speech_denoiser" => Some(Self::SpeechDenoiser),
            "pnd" => Some(Self::Pnd),
            "ab_compare" => Some(Self::ABCompare),
            "crossover" => Some(Self::Crossover),
            "band_split" => Some(Self::BandSplit),
            "band_merge" => Some(Self::BandMerge),
            "downmix" => Some(Self::Downmix),
            "mono_to_stereo" => Some(Self::MonoToStereo),
            "crossfeed" => Some(Self::Crossfeed),
            "delay" => Some(Self::Delay),
            "aec" => Some(Self::Aec),
            "beamformer" => Some(Self::Beamformer),
            "ambisonics_decoder" => Some(Self::AmbisonicsDecoder),
            "stereo_imager" => Some(Self::StereoImager),
            "de_esser" => Some(Self::DeEsser),
            "transient_shaper" => Some(Self::TransientShaper),
            "saturation" => Some(Self::Saturation),
            "dynamic_eq" => Some(Self::DynamicEq),
            "linear_phase_eq" => Some(Self::LinearPhaseEq),
            "spectral_compressor" => Some(Self::SpectralCompressor),
            "external" => Some(Self::External),
            _ => None,
        }
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
        match catalog_entry(self.wire_name())
            .expect("every PluginType must have canonical catalog metadata")
            .metadata
            .maturity
        {
            PluginMaturity::Stable => ReleaseChannel::Prod,
            PluginMaturity::Beta => ReleaseChannel::Beta,
            PluginMaturity::Alpha => ReleaseChannel::Alpha,
            PluginMaturity::Infrastructure => {
                unreachable!("infrastructure entries are not application PluginType values")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_app_plugin_type_has_a_canonical_factory_catalog_entry() {
        let mut missing = Vec::new();
        let plugin_types = PluginType::all();
        let generic_entries: Vec<_> = generic_app_catalog_entries().collect();
        assert_eq!(
            plugin_types.len(),
            generic_entries.len(),
            "generic catalog and PluginType enumeration drifted"
        );

        for plugin_type in plugin_types {
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

    #[test]
    fn compatibility_aliases_resolve_to_the_same_plugin_type() {
        for entry in generic_app_catalog_entries() {
            let canonical = PluginType::from_wire_name(entry.canonical_type).unwrap();
            for alias in entry.aliases {
                assert_eq!(
                    PluginType::from_wire_name(alias),
                    Some(canonical.clone()),
                    "alias '{alias}' drifted from {}",
                    entry.canonical_type
                );
            }
        }
    }
}
