//! Canonical plugin inventory and release-evidence status.
//!
//! Factory aliases remain in `SUPPORTED_PLUGIN_TYPES` for API compatibility.
//! This catalog groups those aliases into the user-visible plugin concepts that
//! must independently satisfy the release gates.

#[cfg(test)]
use super::SUPPORTED_PLUGIN_TYPES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCategory {
    Processor,
    Analyzer,
    Routing,
    Spatial,
    Utility,
    ExternalHost,
    PlatformIo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StabilityEvidenceState {
    Covered(&'static str),
    Pending(&'static str),
    NotApplicable(&'static str),
}

impl StabilityEvidenceState {
    pub const fn is_satisfied(self) -> bool {
        matches!(self, Self::Covered(_) | Self::NotApplicable(_))
    }

    pub const fn evidence_or_reason(self) -> &'static str {
        match self {
            Self::Covered(value) | Self::Pending(value) | Self::NotApplicable(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginStabilityEvidence {
    pub dsp_reference: StabilityEvidenceState,
    pub parameter_preset: StabilityEvidenceState,
    pub channel_layout: StabilityEvidenceState,
    pub realtime_allocation: StabilityEvidenceState,
    pub latency: StabilityEvidenceState,
    pub ui: StabilityEvidenceState,
    pub listening: StabilityEvidenceState,
}

impl PluginStabilityEvidence {
    pub const fn is_complete(self) -> bool {
        self.dsp_reference.is_satisfied()
            && self.parameter_preset.is_satisfied()
            && self.channel_layout.is_satisfied()
            && self.realtime_allocation.is_satisfied()
            && self.latency.is_satisfied()
            && self.ui.is_satisfied()
            && self.listening.is_satisfied()
    }

    pub fn pending_gates(self) -> Vec<&'static str> {
        [
            ("dsp_reference", self.dsp_reference),
            ("parameter_preset", self.parameter_preset),
            ("channel_layout", self.channel_layout),
            ("realtime_allocation", self.realtime_allocation),
            ("latency", self.latency),
            ("ui", self.ui),
            ("listening", self.listening),
        ]
        .into_iter()
        .filter_map(|(name, state)| {
            matches!(state, StabilityEvidenceState::Pending(_)).then_some(name)
        })
        .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginCatalogEntry {
    /// User-facing/factory canonical identifier. The first alias must match it.
    pub canonical_type: &'static str,
    /// Every factory spelling that constructs this plugin concept.
    pub aliases: &'static [&'static str],
    pub category: PluginCategory,
    pub evidence: PluginStabilityEvidence,
}

impl PluginCatalogEntry {
    pub const fn is_ready_for_stable(self) -> bool {
        self.evidence.is_complete()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginStabilitySummary {
    pub plugins: usize,
    pub ready: usize,
    /// Pending counts in DSP, parameter/preset, channel, realtime allocation,
    /// latency, UI, and listening order.
    pub pending_by_gate: [usize; 7],
}

const CROSS_PLUGIN_PARAMETERS: StabilityEvidenceState = StabilityEvidenceState::Covered(
    "tests/param_parity_tests.rs and tests/parameter_roundtrip_tests.rs; sotf-host serialization preset/version contracts; sotf-engine PluginSettings round trips",
);
const CROSS_PLUGIN_CHANNELS: StabilityEvidenceState = StabilityEvidenceState::Covered(
    "tests/all_plugins_channel_count_support.rs asserts exact accepted/rejected mono, stereo, FOA, 5.1, 7.1, and 7.1.4 widths; tests/plugin_high_channel_tests.rs covers channel-changing declarations",
);

const ZERO_ALLOCATION: StabilityEvidenceState =
    StabilityEvidenceState::Covered("tests/realtime_allocation_tests.rs");

const fn builtin_evidence_with_allocation(
    realtime_allocation: StabilityEvidenceState,
    dsp_reference: &'static str,
) -> PluginStabilityEvidence {
    PluginStabilityEvidence {
        dsp_reference: StabilityEvidenceState::Covered(dsp_reference),
        parameter_preset: CROSS_PLUGIN_PARAMETERS,
        channel_layout: CROSS_PLUGIN_CHANNELS,
        realtime_allocation,
        latency: StabilityEvidenceState::Covered(
            "tests/plugin_high_channel_tests.rs streamed measurements plus plugin-specific configuration tests",
        ),
        ui: StabilityEvidenceState::Pending(
            "automated GPUI custom/declarative and simple-view parity, dual-MIDI mapping parity, 320/700/1400px layout solving, and all-plugin render lifecycle pass; attach reviewed narrow/wide screenshots and keyboard/accessibility sign-off",
        ),
        listening: StabilityEvidenceState::Pending(
            "attach a saved level-matched blind listening session",
        ),
    }
}

const fn zero_alloc_evidence(dsp_reference: &'static str) -> PluginStabilityEvidence {
    builtin_evidence_with_allocation(ZERO_ALLOCATION, dsp_reference)
}

const fn analyzer_evidence(dsp_reference: &'static str) -> PluginStabilityEvidence {
    PluginStabilityEvidence {
        dsp_reference: StabilityEvidenceState::Covered(dsp_reference),
        parameter_preset: CROSS_PLUGIN_PARAMETERS,
        channel_layout: CROSS_PLUGIN_CHANNELS,
        realtime_allocation: ZERO_ALLOCATION,
        latency: StabilityEvidenceState::NotApplicable(
            "analyzers pass audio through without algorithmic delay",
        ),
        ui: StabilityEvidenceState::Pending(
            "automated GPUI custom-view and simple-view parity, dual-MIDI mapping parity, and all-plugin render lifecycle pass; attach reviewed narrow/wide analyzer screenshots and keyboard/accessibility sign-off",
        ),
        listening: StabilityEvidenceState::NotApplicable(
            "analyzers must be bit-transparent; audibility is covered by the DSP gate",
        ),
    }
}

const fn external_host_evidence() -> PluginStabilityEvidence {
    PluginStabilityEvidence {
        dsp_reference: StabilityEvidenceState::Covered(
            "sotf-host external_plugin and external_plugin_isolated tests cover deterministic passthrough/state parity, timeout fallback, quarantine, worker channel agreement, and error isolation",
        ),
        parameter_preset: StabilityEvidenceState::Covered(
            "sotf-host serialization and isolated-host tests cover descriptor/state consistency, automation metadata, restoration, and malformed preset rejection",
        ),
        channel_layout: StabilityEvidenceState::Covered(
            "sotf-host external plugin, IPC layout, isolated-host, and worker tests enforce descriptor and shared-layout input/output channel agreement",
        ),
        realtime_allocation: StabilityEvidenceState::Covered(
            "tests/realtime_allocation_tests.rs external timeout/quarantine/fallback contract",
        ),
        latency: StabilityEvidenceState::Covered(
            "sotf-host external worker publishes latency metadata through IPC; in-process placeholder is zero-latency passthrough",
        ),
        ui: StabilityEvidenceState::Pending(
            "link discovery, permission, generic UI, and failure-state review evidence",
        ),
        listening: StabilityEvidenceState::Pending(
            "attach a saved level-matched hosted-versus-reference session",
        ),
    }
}

#[cfg(all(target_os = "macos", feature = "hal"))]
const fn platform_io_evidence() -> PluginStabilityEvidence {
    PluginStabilityEvidence {
        dsp_reference: StabilityEvidenceState::Pending(
            "link bit-exact platform I/O loopback tests",
        ),
        parameter_preset: StabilityEvidenceState::NotApplicable(
            "platform I/O is configured by the systemwide transport contract",
        ),
        channel_layout: StabilityEvidenceState::Pending(
            "link negotiated platform channel-layout tests",
        ),
        realtime_allocation: StabilityEvidenceState::Pending(
            "link callback allocation and underrun evidence",
        ),
        latency: StabilityEvidenceState::Pending("link negotiated transport latency evidence"),
        ui: StabilityEvidenceState::NotApplicable(
            "platform I/O nodes are managed by the systemwide routing UI",
        ),
        listening: StabilityEvidenceState::NotApplicable(
            "bit-exact loopback and transport integrity are the applicable gates",
        ),
    }
}

macro_rules! entry {
    ($canonical:literal, [$($alias:literal),+ $(,)?], $category:ident, $evidence:expr) => {
        PluginCatalogEntry {
            canonical_type: $canonical,
            aliases: &[$($alias),+],
            category: PluginCategory::$category,
            evidence: $evidence,
        }
    };
}

/// Canonical inventory for every factory-exposed plugin concept.
pub const PLUGIN_CATALOG: &[PluginCatalogEntry] = &[
    entry!(
        "gain",
        ["gain"],
        Processor,
        zero_alloc_evidence(
            "tests/property_tests.rs verifies the analytical dB scale, unity transparency, linearity, mute, energy, and finite-output bounds"
        )
    ),
    entry!(
        "eq",
        ["eq", "parametric_eq"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-eq unit/integration tests cover exact bypass, boost response, compiled/reference parity, topology transitions, oversampling, and sample-rate scaling"
        )
    ),
    entry!(
        "compressor",
        ["compressor"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-multiband-compressor dynamics and crossover-reconstruction tests cover the one-band compressor transfer path and multiband implementation"
        )
    ),
    entry!(
        "expander",
        ["expander"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-multiband-expander unity-ratio, expansion, crossover-reconstruction, and spectral/time-domain parity tests"
        )
    ),
    entry!(
        "limiter",
        ["limiter"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-limiter ceiling, knee, true-peak, dry-path, attack/release, and dynamics integration tests"
        )
    ),
    entry!(
        "gate",
        ["gate"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-gate open/close, hysteresis, hold, sidechain, bypass, and dynamics reference tests"
        )
    ),
    entry!(
        "delay",
        ["delay"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-delay known-sample impulse delay, feedback decay, bypass, all-pass response, and sample-rate tests"
        )
    ),
    entry!(
        "convolution",
        ["convolution"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-convolution Dirac/unity-IR, dry mix, impulse-response, reset, latency, and streamed convolution tests"
        )
    ),
    entry!(
        "upmixer",
        ["upmixer"],
        Spatial,
        zero_alloc_evidence(
            "stft_normalization_tests.rs plus upmixer phase, VBAP, crossover, energy, streamed-latency, and distortion-regression tests"
        )
    ),
    entry!(
        "aae",
        ["aae", "active_acoustic_enhancement"],
        Spatial,
        zero_alloc_evidence(
            "sotf-plugin-aae impulse-response, early-reflection, FDN decay, routing-energy, reset, and finite-output tests"
        )
    ),
    entry!(
        "downmix",
        ["downmix"],
        Routing,
        zero_alloc_evidence(
            "sotf-plugin-downmix WOLA perfect reconstruction, phase-coherence, LtRt all-pass, routing, bypass, and high-layout tests"
        )
    ),
    entry!(
        "mono_to_stereo",
        ["mono_to_stereo"],
        Routing,
        zero_alloc_evidence(
            "sotf-plugin-mono-to-stereo mono/stereo energy, Haas delay, decorrelation, bypass, and reset tests"
        )
    ),
    entry!(
        "multiband_compressor",
        ["multiband_compressor"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-multiband-compressor transfer, knee, detector, dry-path, crossover-reconstruction, and multirate tests"
        )
    ),
    entry!(
        "multiband_expander",
        ["multiband_expander"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-multiband-expander transfer, unity-ratio, spectral/time-domain, crossover, and streamed-latency tests"
        )
    ),
    entry!(
        "de_esser",
        ["de_esser"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-de-esser split-band tests verify high-frequency attenuation with low-frequency passthrough, detector, mix, and reset behavior"
        )
    ),
    entry!(
        "dynamic_eq",
        ["dynamic_eq"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-dynamic-eq frequency-selective gain, inactive/dry transparency, linked/unlinked stereo, filter rebuild, and dynamics tests"
        )
    ),
    entry!(
        "fir_designer",
        ["fir_designer"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-fir-designer impulse, phase-linearity, minimum/linear-phase latency, dry-path, transition, and response tests"
        )
    ),
    entry!(
        "linear_phase_eq",
        ["linear_phase_eq"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-linear-phase-eq boost response, phase linearity, dry transparency, latency, reset, and varied-block tests"
        )
    ),
    entry!(
        "spectral_compressor",
        ["spectral_compressor"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-spectral-compressor Hann magnitude calibration, FFT roundtrip, hard/soft knee, loud/quiet-bin, delta-listen, and latency tests"
        )
    ),
    entry!(
        "stereo_imager",
        ["stereo_imager"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-stereo-imager M/S width, mono-bass, crossover smoothing, constant-signal transparency, dry mix, and non-stereo bypass tests"
        )
    ),
    entry!(
        "transient_shaper",
        ["transient_shaper"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-transient-shaper impulse/envelope response, stereo bypass, attack/sustain gain, mix, and reset tests"
        )
    ),
    entry!(
        "saturation",
        ["saturation"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-saturation transfer-mode, symmetry, oversampling, auto-gain, dry transparency, and bounded-output tests"
        )
    ),
    entry!(
        "loudness_compensation",
        ["loudness_compensation"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-loudness-compensation ISO 226 1 kHz reference, equal-level transparency, auto-mode flat response, and filter rebuild tests"
        )
    ),
    entry!(
        "fletcher_munson",
        ["fletcher_munson"],
        Processor,
        zero_alloc_evidence(
            "Fletcher-Munson compatibility canonicalizes to the loudness-compensation ISO 226 reference path and is covered by config/engine round trips"
        )
    ),
    entry!(
        "crossfeed",
        ["crossfeed"],
        Spatial,
        zero_alloc_evidence(
            "sotf-plugin-crossfeed low/high frequency response, mode-off/dry transparency, delay/crossfeed level, and reset tests"
        )
    ),
    entry!(
        "xtc",
        ["xtc", "crosstalk_cancellation"],
        Spatial,
        zero_alloc_evidence(
            "sotf-plugin-xtc analytical ITD geometry, ILD frequency dependence, 2x2 inversion, STFT reconstruction, phase, limiter, and streamed-latency tests"
        )
    ),
    entry!(
        "denoiser",
        ["denoiser", "wiener_denoiser"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-denoiser per-channel offline reference parity, frequency-selective behavior, profile state, streamed latency, and reset tests"
        )
    ),
    entry!(
        "speech_denoiser",
        ["speech_denoiser", "rnnoise", "rnnoise_denoiser"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-speech-denoiser RNNoise 48 kHz/frame-size contract, disabled delayed transparency, enabled processing, latency, and reset tests"
        )
    ),
    entry!(
        "hiss_reducer",
        ["hiss_reducer", "hiss"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-hiss-reducer high-frequency attenuation, disabled transparency, state-preserving updates, sample-rate initialization, and reset tests"
        )
    ),
    entry!(
        "declick",
        ["declick", "transient_repair"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-declick channel-aware click-reduction, disabled transparency, suppressor reset, buffer validation, and zero-latency tests"
        )
    ),
    entry!(
        "pnd",
        ["pnd", "varispeed"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-pnd stable-tone near-unity, known-drift correction, phase-vocoder transition, smoothing, latency, reset, and block-size tests"
        )
    ),
    entry!(
        "binaural_decoder",
        ["binaural_decoder"],
        Spatial,
        zero_alloc_evidence(
            "sotf-plugin-binaural HRTF geometry/rotation, convolution, diffuse-field, near-field, reset, and plugin_chain_channel_preservation spatial-chain tests"
        )
    ),
    entry!(
        "crossover",
        ["crossover"],
        Routing,
        zero_alloc_evidence(
            "sotf-plugin-crossover LR reconstruction, linear-phase delayed reconstruction, per-channel routing, smoothing, multiband, and multirate tests"
        )
    ),
    entry!(
        "matrix",
        ["matrix"],
        Routing,
        zero_alloc_evidence(
            "sotf-plugin-matrix identity, sparse/full mapping, gain, phase inversion, channel-state, and 7.1.4 channel-identifiable tests"
        )
    ),
    entry!(
        "channel_mute_solo",
        ["channel_mute_solo"],
        Routing,
        zero_alloc_evidence(
            "sotf-plugin-channel-mute-solo exact mute/solo routing, all-muted safety, identity passthrough, channel-count, and fuzzer tests"
        )
    ),
    entry!(
        "loudness_monitor",
        ["loudness_monitor"],
        Analyzer,
        analyzer_evidence(
            "sotf-host test_analyzer_plugins verifies exact passthrough, -20 dBFS 1 kHz loudness within 0.2 LU, peak, correlation, and multichannel slots"
        )
    ),
    entry!(
        "spectrum_analyzer",
        ["spectrum_analyzer"],
        Analyzer,
        analyzer_evidence(
            "sotf-host analyzer_spectrum tests verify exact passthrough, bin-centered and Nyquist 0 dBFS calibration within 0.1 dB, silence floor, and stereo/multichannel analysis"
        )
    ),
    entry!(
        "resampler",
        ["resampler"],
        Utility,
        zero_alloc_evidence(
            "sotf-plugin-resampler ratio/frame-count, continuity, anti-aliasing, quality cutoff, flush, latency, multichannel, and variable-block tests"
        )
    ),
    entry!(
        "band_split",
        ["band_split"],
        Routing,
        zero_alloc_evidence(
            "sotf-plugin-band-split two/three/four-band DC reconstruction, crossover spacing, smoothing, routing, and split/merge high-layout round trips"
        )
    ),
    entry!(
        "band_merge",
        ["band_merge"],
        Routing,
        zero_alloc_evidence(
            "sotf-plugin-band-merge reconstruction-error reference, unity/gain/mute behavior, routing, and split/merge high-layout round trips"
        )
    ),
    entry!(
        "ab_compare",
        ["ab_compare", "ab"],
        Utility,
        zero_alloc_evidence(
            "sotf-plugin-ab-compare level-match, phase inversion, delay compensation, switching/crossfade, sub-rack, reset, and channel-preservation tests"
        )
    ),
    entry!(
        "aec",
        ["aec"],
        Processor,
        zero_alloc_evidence(
            "sotf-plugin-aec no-echo passthrough, synthetic echo cancellation/convergence, two-path, post-filter, latency, reset, and stability tests"
        )
    ),
    entry!(
        "beamformer",
        ["beamformer"],
        Spatial,
        zero_alloc_evidence(
            "sotf-plugin-beamformer analytical steering delays/vectors, MVDR/GSC/superdirective weight, noise cancellation, STFT overlap, reset, and finite-output tests"
        )
    ),
    entry!(
        "ambisonics_decoder",
        ["ambisonics_decoder"],
        Spatial,
        zero_alloc_evidence(
            "sotf-plugin-ambisonics spherical-harmonic/decode-matrix, max-rE, dual-band, channel-order, energy, reset, and ambisonics-to-binaural chain tests"
        )
    ),
    entry!(
        "external",
        ["external", "external_plugin"],
        ExternalHost,
        external_host_evidence()
    ),
    #[cfg(all(target_os = "macos", feature = "hal"))]
    entry!(
        "hal_input",
        ["hal_input"],
        PlatformIo,
        platform_io_evidence()
    ),
    #[cfg(all(target_os = "macos", feature = "hal"))]
    entry!(
        "hal_output",
        ["hal_output"],
        PlatformIo,
        platform_io_evidence()
    ),
];

pub fn catalog_entry(plugin_type: &str) -> Option<&'static PluginCatalogEntry> {
    PLUGIN_CATALOG.iter().find(|entry| {
        entry
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(plugin_type))
    })
}

pub fn plugin_stability_summary() -> PluginStabilitySummary {
    let mut summary = PluginStabilitySummary {
        plugins: PLUGIN_CATALOG.len(),
        ready: 0,
        pending_by_gate: [0; 7],
    };

    for entry in PLUGIN_CATALOG {
        summary.ready += usize::from(entry.is_ready_for_stable());
        for (index, state) in [
            entry.evidence.dsp_reference,
            entry.evidence.parameter_preset,
            entry.evidence.channel_layout,
            entry.evidence.realtime_allocation,
            entry.evidence.latency,
            entry.evidence.ui,
            entry.evidence.listening,
        ]
        .into_iter()
        .enumerate()
        {
            summary.pending_by_gate[index] +=
                usize::from(matches!(state, StabilityEvidenceState::Pending(_)));
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_aliases_exactly_cover_factory_types() {
        let aliases: Vec<_> = PLUGIN_CATALOG
            .iter()
            .flat_map(|entry| entry.aliases.iter().copied())
            .collect();
        let alias_set: HashSet<_> = aliases.iter().copied().collect();
        let supported_set: HashSet<_> = SUPPORTED_PLUGIN_TYPES.iter().copied().collect();

        assert_eq!(
            aliases.len(),
            alias_set.len(),
            "catalog aliases must be unique"
        );
        assert_eq!(
            SUPPORTED_PLUGIN_TYPES.len(),
            supported_set.len(),
            "factory types must be unique"
        );
        assert_eq!(alias_set, supported_set);
    }

    #[test]
    fn canonical_types_are_unique_and_first_aliases() {
        let mut canonical = HashSet::new();
        for entry in PLUGIN_CATALOG {
            assert!(canonical.insert(entry.canonical_type));
            assert_eq!(entry.aliases.first().copied(), Some(entry.canonical_type));
        }
    }

    #[test]
    fn every_evidence_state_has_an_actionable_reference_or_reason() {
        for entry in PLUGIN_CATALOG {
            for state in [
                entry.evidence.dsp_reference,
                entry.evidence.parameter_preset,
                entry.evidence.channel_layout,
                entry.evidence.realtime_allocation,
                entry.evidence.latency,
                entry.evidence.ui,
                entry.evidence.listening,
            ] {
                assert!(
                    !state.evidence_or_reason().trim().is_empty(),
                    "{} has an undocumented evidence state",
                    entry.canonical_type
                );
            }
        }
    }

    #[test]
    fn summary_matches_entry_readiness() {
        let summary = plugin_stability_summary();
        assert_eq!(summary.plugins, PLUGIN_CATALOG.len());
        assert_eq!(
            summary.ready,
            PLUGIN_CATALOG
                .iter()
                .filter(|entry| entry.is_ready_for_stable())
                .count()
        );
        assert_eq!(
            PLUGIN_CATALOG
                .iter()
                .filter(|entry| entry.category != PluginCategory::PlatformIo)
                .filter(|entry| matches!(
                    entry.evidence.dsp_reference,
                    StabilityEvidenceState::Pending(_)
                ))
                .count(),
            0,
            "the DSP/reference gate must cover every built-in and external-host contract"
        );
        assert_eq!(
            summary.pending_by_gate[1], 0,
            "the parameter/preset gate must cover every built-in and external-host contract"
        );
        assert_eq!(
            PLUGIN_CATALOG
                .iter()
                .filter(|entry| entry.category != PluginCategory::PlatformIo)
                .filter(|entry| matches!(
                    entry.evidence.channel_layout,
                    StabilityEvidenceState::Pending(_)
                ))
                .count(),
            0,
            "the channel-layout gate must cover every built-in and external-host contract"
        );
        assert_eq!(
            summary.pending_by_gate[3], 0,
            "the allocation gate must cover all built-in and external-host process paths"
        );
        assert_eq!(
            summary.pending_by_gate[4], 0,
            "the latency gate must cover built-in and external-host contracts"
        );
    }
}
