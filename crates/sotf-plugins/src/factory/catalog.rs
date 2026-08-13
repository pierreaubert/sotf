//! Canonical plugin inventory and release-evidence status.
//!
//! Factory aliases, application pickers, and QA enumeration are derived from
//! this catalog so every exposed concept independently satisfies release gates
//! without parallel hand-maintained lists.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginCategory {
    Processor,
    Analyzer,
    Routing,
    Spatial,
    Utility,
    ExternalHost,
    PlatformIo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

/// Release maturity recorded by the canonical catalog.
///
/// `Stable` is deliberately evidence-gated: catalog tests reject any stable
/// entry whose applicable release gates are not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginMaturity {
    Stable,
    Beta,
    Alpha,
    Infrastructure,
}

pub const STANDARD_CHANNEL_WIDTHS: &[usize] = &[1, 2, 4, 6, 8, 12];
pub const MONO_CHANNEL_WIDTH: &[usize] = &[1];
pub const STEREO_CHANNEL_WIDTH: &[usize] = &[2];
pub const AMBISONIC_WIDTHS: &[usize] = &[4, 9, 16];
/// Backward-compatible alias retained for callers that still name FOA while
/// using the complete Ambisonics-width admission contract.
pub const FIRST_ORDER_AMBISONIC_WIDTH: &[usize] = AMBISONIC_WIDTHS;
pub const MICROPHONE_ARRAY_WIDTHS: &[usize] = &[2, 4, 6, 8];
pub const EVEN_BAND_CHANNEL_WIDTHS: &[usize] = &[2, 4, 6, 8, 12];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginSupportedInputLayouts {
    Enumerated(&'static [usize]),
    DescriptorDefined,
    PlatformNegotiated,
}

impl PluginSupportedInputLayouts {
    pub fn supports(self, channels: usize) -> Option<bool> {
        match self {
            Self::Enumerated(widths) => Some(widths.contains(&channels)),
            Self::DescriptorDefined | Self::PlatformNegotiated => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginDefaultChannelOutput {
    PreservesInput,
    Fixed(usize),
}

impl PluginDefaultChannelOutput {
    pub const fn channels(self, input_channels: usize) -> usize {
        match self {
            Self::PreservesInput => input_channels,
            Self::Fixed(channels) => channels,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginChannelOutputModel {
    PreservesInput,
    Fixed(usize),
    Configurable {
        description: &'static str,
        default_output: PluginDefaultChannelOutput,
    },
    InputTimesBands,
    InputDividedByBands,
    DescriptorDefined,
    PlatformNegotiated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PluginChannelLayoutContract {
    pub supported_inputs: PluginSupportedInputLayouts,
    pub output: PluginChannelOutputModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginLatencyModel {
    Zero,
    PluginReported(&'static str),
    FrameBased(&'static str),
    HostedPluginReported,
    PlatformNegotiated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginParameterSchema {
    Static(&'static str),
    DescriptorProvided,
    PlatformTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginPresetSupport {
    VersionedSettings,
    ExternalOpaqueState,
    InfrastructureConfiguration,
    PlatformTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginUiKind {
    Custom,
    Generated,
    ExternalHost,
    NotExposed,
    Systemwide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ExternalReferenceImplementation {
    Installed(&'static str),
    NoneRecorded,
    DescriptorDefined,
    NotApplicable(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginPickerExposure {
    Generic,
    DiscoveredExternal,
    Infrastructure,
    Systemwide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PluginCatalogMetadata {
    pub owning_crate: &'static str,
    pub exposed_name: &'static str,
    pub maturity: PluginMaturity,
    pub channel_layout: PluginChannelLayoutContract,
    pub latency_model: PluginLatencyModel,
    pub parameter_schema: PluginParameterSchema,
    pub preset_support: PluginPresetSupport,
    pub ui: PluginUiKind,
    pub external_reference: ExternalReferenceImplementation,
    pub picker: PluginPickerExposure,
    pub allowed_in_ab_compare: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PluginCatalogEntry {
    /// User-facing/factory canonical identifier. The first alias must match it.
    pub canonical_type: &'static str,
    /// Every factory spelling that constructs this plugin concept.
    pub aliases: &'static [&'static str],
    pub category: PluginCategory,
    pub metadata: PluginCatalogMetadata,
    pub evidence: PluginStabilityEvidence,
}

impl PluginCatalogEntry {
    pub const fn is_ready_for_stable(self) -> bool {
        self.evidence.is_complete()
    }

    pub const fn is_generic_app_plugin(self) -> bool {
        matches!(self.metadata.picker, PluginPickerExposure::Generic)
    }

    pub const fn is_allowed_in_ab_compare(self) -> bool {
        self.metadata.allowed_in_ab_compare
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
const APP_PLUGIN_UI: StabilityEvidenceState = StabilityEvidenceState::Covered(
    "crates/app-gpui/bin/plugin_visual_review.rs: 86 reviewed 700x900 and 1600x1000 captures for all 43 app-visible plugins, with a manifest proving every focusable accessibility node is named (2026-07-18)",
);

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
        ui: APP_PLUGIN_UI,
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
        ui: APP_PLUGIN_UI,
        listening: StabilityEvidenceState::NotApplicable(
            "analyzers must be bit-transparent; audibility is covered by the DSP gate",
        ),
    }
}

const fn infrastructure_evidence(dsp_reference: &'static str) -> PluginStabilityEvidence {
    PluginStabilityEvidence {
        dsp_reference: StabilityEvidenceState::Covered(dsp_reference),
        parameter_preset: CROSS_PLUGIN_PARAMETERS,
        channel_layout: CROSS_PLUGIN_CHANNELS,
        realtime_allocation: ZERO_ALLOCATION,
        latency: StabilityEvidenceState::Covered(
            "tests/plugin_high_channel_tests.rs streamed measurements plus plugin-specific configuration tests",
        ),
        ui: StabilityEvidenceState::NotApplicable(
            "infrastructure processors are not exposed in application plugin pickers",
        ),
        listening: StabilityEvidenceState::NotApplicable(
            "deterministic signal-integrity tests are the applicable infrastructure gate",
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
        ui: StabilityEvidenceState::Covered(
            "crates/app-gpui/bin/plugin_visual_review.rs: reviewed external-host rack detail and Settings discovery/permission/failure surfaces at 700x900 and 1600x1000, with named focusable accessibility nodes and consistent 2-to-4 channel summaries (2026-07-18)",
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

macro_rules! builtin_metadata {
    (
        $crate_name:literal,
        $exposed_name:literal,
        $maturity:ident,
        $inputs:expr,
        $output:expr,
        $latency:expr,
        $parameter_schema:literal,
        $ui:ident,
        $allowed_in_ab_compare:literal $(,)?
    ) => {
        PluginCatalogMetadata {
            owning_crate: $crate_name,
            exposed_name: $exposed_name,
            maturity: PluginMaturity::$maturity,
            channel_layout: PluginChannelLayoutContract {
                supported_inputs: $inputs,
                output: $output,
            },
            latency_model: $latency,
            parameter_schema: PluginParameterSchema::Static($parameter_schema),
            preset_support: PluginPresetSupport::VersionedSettings,
            ui: PluginUiKind::$ui,
            external_reference: ExternalReferenceImplementation::NoneRecorded,
            picker: PluginPickerExposure::Generic,
            allowed_in_ab_compare: $allowed_in_ab_compare,
        }
    };
}

macro_rules! entry {
    (
        $canonical:literal,
        [$($alias:literal),+ $(,)?],
        $category:ident,
        $metadata:expr,
        $evidence:expr $(,)?
    ) => {
        PluginCatalogEntry {
            canonical_type: $canonical,
            aliases: &[$($alias),+],
            category: PluginCategory::$category,
            metadata: $metadata,
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
        builtin_metadata!(
            "sotf-plugin-gain",
            "Gain",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::Zero,
            "sotf_plugin_gain::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "tests/property_tests.rs verifies the analytical dB scale, unity transparency, linearity, mute, energy, and finite-output bounds"
        )
    ),
    entry!(
        "eq",
        ["eq", "parametric_eq"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-eq",
            "EQ",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported(
                "IIR is zero-latency; compiled and oversampled modes report their active latency",
            ),
            "sotf_plugin_eq::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-eq unit/integration tests cover exact bypass, boost response, compiled/reference parity, topology transitions, oversampling, and sample-rate scaling"
        )
    ),
    entry!(
        "compressor",
        ["compressor"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-multiband-compressor",
            "Compressor",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("detector and lookahead configuration"),
            "runtime broadband schema (unsupported legacy sidechain controls rejected)",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-multiband-compressor dynamics and crossover-reconstruction tests cover the one-band compressor transfer path and multiband implementation"
        )
    ),
    entry!(
        "expander",
        ["expander"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-multiband-expander",
            "Expander",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("detector and lookahead configuration"),
            "sotf_plugin_multiband_expander::params::SINGLE_BAND_LAYOUT",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-multiband-expander unity-ratio, expansion, crossover-reconstruction, and spectral/time-domain parity tests"
        )
    ),
    entry!(
        "limiter",
        ["limiter"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-limiter",
            "Limiter",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("lookahead and true-peak configuration"),
            "sotf_plugin_limiter::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-limiter ceiling, knee, true-peak, dry-path, attack/release, and dynamics integration tests"
        )
    ),
    entry!(
        "gate",
        ["gate"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-gate",
            "Gate",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("detector configuration"),
            "sotf_plugin_gate::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-gate open/close, hysteresis, hold, sidechain, bypass, and dynamics reference tests"
        )
    ),
    entry!(
        "delay",
        ["delay"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-delay",
            "Delay",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported(
                "effect delay is signal behavior; the host contract reports active latency",
            ),
            "sotf_plugin_delay::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-delay known-sample impulse delay, feedback decay, bypass, all-pass response, and sample-rate tests"
        )
    ),
    entry!(
        "convolution",
        ["convolution"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-convolution",
            "Convolution",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::FrameBased("IR partition and active convolution configuration"),
            "sotf_plugin_convolution::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-convolution Dirac/unity-IR, dry mix, impulse-response, reset, latency, and streamed convolution tests"
        )
    ),
    entry!(
        "upmixer",
        ["upmixer"],
        Spatial,
        builtin_metadata!(
            "sotf-plugin-upmixer",
            "Upmixer",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STEREO_CHANNEL_WIDTH),
            PluginChannelOutputModel::Configurable {
                description: "stereo to configured speaker layout",
                default_output: PluginDefaultChannelOutput::Fixed(6),
            },
            PluginLatencyModel::FrameBased("STFT frame and hop configuration"),
            "sotf_plugin_upmixer::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "stft_normalization_tests.rs plus upmixer phase, VBAP, crossover, energy, streamed-latency, and distortion-regression tests"
        )
    ),
    entry!(
        "aae",
        ["aae", "active_acoustic_enhancement"],
        Spatial,
        builtin_metadata!(
            "sotf-plugin-aae",
            "AAE",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STEREO_CHANNEL_WIDTH),
            PluginChannelOutputModel::Configurable {
                description: "stereo to configured enhancement layout",
                default_output: PluginDefaultChannelOutput::Fixed(6),
            },
            PluginLatencyModel::PluginReported("reflection and FDN configuration"),
            "sotf_plugin_aae::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-aae impulse-response, early-reflection, FDN decay, routing-energy, reset, and finite-output tests"
        )
    ),
    entry!(
        "downmix",
        ["downmix"],
        Routing,
        builtin_metadata!(
            "sotf-plugin-downmix",
            "Downmix",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::Fixed(2),
            PluginLatencyModel::FrameBased("WOLA mode and active downmix configuration"),
            "sotf_plugin_downmix::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-downmix WOLA perfect reconstruction, phase-coherence, LtRt all-pass, routing, bypass, and high-layout tests"
        )
    ),
    entry!(
        "mono_to_stereo",
        ["mono_to_stereo"],
        Routing,
        builtin_metadata!(
            "sotf-plugin-mono-to-stereo",
            "Mono to Stereo",
            Beta,
            PluginSupportedInputLayouts::Enumerated(MONO_CHANNEL_WIDTH),
            PluginChannelOutputModel::Fixed(2),
            PluginLatencyModel::PluginReported("Haas/decorrelation configuration"),
            "sotf_plugin_mono_to_stereo::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-mono-to-stereo mono/stereo energy, Haas delay, decorrelation, bypass, and reset tests"
        )
    ),
    entry!(
        "multiband_compressor",
        ["multiband_compressor"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-multiband-compressor",
            "Multiband Compressor",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("crossover, detector, and lookahead configuration"),
            "sotf_plugin_multiband_compressor::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-multiband-compressor transfer, knee, detector, dry-path, crossover-reconstruction, and multirate tests"
        )
    ),
    entry!(
        "multiband_expander",
        ["multiband_expander"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-multiband-expander",
            "Multiband Expander",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("crossover, detector, and lookahead configuration"),
            "sotf_plugin_multiband_expander::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-multiband-expander transfer, unity-ratio, spectral/time-domain, crossover, and streamed-latency tests"
        )
    ),
    entry!(
        "de_esser",
        ["de_esser"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-de-esser",
            "De-Esser",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("split-band detector configuration"),
            "sotf_plugin_de_esser::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-de-esser split/wide detector response, phase-matched inactive null, structural controls, reset/cache cadence, non-finite recovery, and realtime process/setter allocation tests"
        )
    ),
    entry!(
        "dynamic_eq",
        ["dynamic_eq"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-dynamic-eq",
            "Dynamic EQ",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("filter and detector configuration"),
            "sotf_plugin_dynamic_eq::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-dynamic-eq frequency-selective gain, inactive/dry transparency, linked/unlinked stereo, filter rebuild, and dynamics tests"
        )
    ),
    entry!(
        "linear_phase_eq",
        ["linear_phase_eq", "fir_designer"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-linear-phase-eq",
            "FIR EQ",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::FrameBased("FIR length and phase mode"),
            "sotf_plugin_linear_phase_eq::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-linear-phase-eq boost response, linear/minimum phase, dry transparency, latency, reset, and varied-block tests"
        )
    ),
    entry!(
        "spectral_compressor",
        ["spectral_compressor"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-spectral-compressor",
            "Spectral Compressor",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::FrameBased("one full FFT frame"),
            "sotf_plugin_spectral_compressor::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-spectral-compressor Hann magnitude calibration, FFT roundtrip, hard/soft knee, loud/quiet-bin, delta-listen, and latency tests"
        )
    ),
    entry!(
        "stereo_imager",
        ["stereo_imager"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-stereo-imager",
            "Stereo Imager",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("crossover and active width configuration"),
            "sotf_plugin_stereo_imager::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-stereo-imager M/S width, mono-bass, crossover smoothing, constant-signal transparency, dry mix, and non-stereo bypass tests"
        )
    ),
    entry!(
        "transient_shaper",
        ["transient_shaper"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-transient-shaper",
            "Transient Shaper",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("envelope detector configuration"),
            "sotf_plugin_transient_shaper::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-transient-shaper impulse/envelope response, stereo bypass, attack/sustain gain, mix, and reset tests"
        )
    ),
    entry!(
        "saturation",
        ["saturation"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-saturation",
            "Saturation",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("oversampling mode"),
            "sotf_plugin_saturation::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-saturation transfer-mode, symmetry, oversampling, auto-gain, dry transparency, and bounded-output tests"
        )
    ),
    entry!(
        "loudness_compensation",
        ["loudness_compensation"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-loudness-compensation",
            "Loudness Compensation",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("active filter topology"),
            "sotf_plugin_loudness_compensation::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-loudness-compensation ISO 226 1 kHz reference, equal-level transparency, auto-mode flat response, and filter rebuild tests"
        )
    ),
    entry!(
        "fletcher_munson",
        ["fletcher_munson"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-loudness-compensation",
            "Fletcher-Munson",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("active ISO 226 filter topology"),
            "sotf_plugin_loudness_compensation::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "Fletcher-Munson compatibility canonicalizes to the loudness-compensation ISO 226 reference path and is covered by config/engine round trips"
        )
    ),
    entry!(
        "crossfeed",
        ["crossfeed"],
        Spatial,
        builtin_metadata!(
            "sotf-plugin-crossfeed",
            "Crossfeed",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STEREO_CHANNEL_WIDTH),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("crossfeed delay configuration"),
            "sotf_plugin_crossfeed::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-crossfeed low/high frequency response, mode-off/dry transparency, delay/crossfeed level, and reset tests"
        )
    ),
    entry!(
        "xtc",
        ["xtc", "crosstalk_cancellation"],
        Spatial,
        builtin_metadata!(
            "sotf-plugin-xtc",
            "Crosstalk Cancellation",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STEREO_CHANNEL_WIDTH),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::FrameBased("one full FFT frame"),
            "sotf_plugin_xtc::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-xtc analytical ITD geometry, ILD frequency dependence, 2x2 inversion, STFT reconstruction, phase, limiter, and streamed-latency tests"
        )
    ),
    entry!(
        "denoiser",
        ["denoiser", "wiener_denoiser"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-denoiser",
            "Denoiser",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::FrameBased("Wiener/MCRA analysis frame"),
            "sotf_plugin_denoiser::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-denoiser per-channel offline reference parity, frequency-selective behavior, profile state, streamed latency, and reset tests"
        )
    ),
    entry!(
        "speech_denoiser",
        ["speech_denoiser", "rnnoise", "rnnoise_denoiser"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-speech-denoiser",
            "Speech Denoiser",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::FrameBased("RNNoise 480-sample frame at 48 kHz"),
            "sotf_plugin_speech_denoiser::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-speech-denoiser RNNoise 48 kHz/frame-size contract, disabled delayed transparency, enabled processing, latency, and reset tests"
        )
    ),
    entry!(
        "hiss_reducer",
        ["hiss_reducer", "hiss"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-hiss-reducer",
            "Hiss Reducer",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("active reduction configuration"),
            "sotf_plugin_hiss_reducer::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-hiss-reducer high-frequency attenuation, disabled transparency, state-preserving updates, sample-rate initialization, and reset tests"
        )
    ),
    entry!(
        "declick",
        ["declick", "transient_repair"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-declick",
            "Declick",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::Zero,
            "sotf_plugin_declick::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-declick channel-aware click-reduction, disabled transparency, suppressor reset, buffer validation, and zero-latency tests"
        )
    ),
    entry!(
        "pnd",
        ["pnd", "varispeed"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-pnd",
            "PND Varispeed",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::FrameBased("phase-vocoder analysis frame"),
            "sotf_plugin_pnd::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-pnd stable-tone near-unity, known-drift correction, phase-vocoder transition, smoothing, latency, reset, and block-size tests"
        )
    ),
    entry!(
        "binaural_decoder",
        ["binaural_decoder"],
        Spatial,
        builtin_metadata!(
            "sotf-plugin-binaural",
            "Binaural Decoder",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::Fixed(2),
            PluginLatencyModel::FrameBased("HRTF convolution partition"),
            "sotf_plugin_binaural::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-binaural HRTF geometry/rotation, convolution, diffuse-field, near-field, reset, and plugin_chain_channel_preservation spatial-chain tests"
        )
    ),
    entry!(
        "crossover",
        ["crossover"],
        Routing,
        builtin_metadata!(
            "sotf-plugin-crossover",
            "Crossover",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("IIR is zero-latency; FIR mode reports its delay"),
            "sotf_plugin_crossover::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-crossover LR reconstruction, linear-phase delayed reconstruction, per-channel routing, smoothing, multiband, and multirate tests"
        )
    ),
    entry!(
        "matrix",
        ["matrix"],
        Routing,
        builtin_metadata!(
            "sotf-plugin-matrix",
            "Matrix Mixer",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::Configurable {
                description: "declared matrix output width",
                default_output: PluginDefaultChannelOutput::PreservesInput,
            },
            PluginLatencyModel::Zero,
            "sotf_plugin_matrix::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-matrix identity, sparse/full mapping, gain, phase inversion, channel-state, and 7.1.4 channel-identifiable tests"
        )
    ),
    entry!(
        "channel_mute_solo",
        ["channel_mute_solo"],
        Routing,
        builtin_metadata!(
            "sotf-plugin-channel-mute-solo",
            "Channel Mute/Solo",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::Zero,
            "sotf_plugin_channel_mute_solo::params::PARAMS",
            Custom,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-channel-mute-solo exact mute/solo routing, all-muted safety, identity passthrough, channel-count, and fuzzer tests"
        )
    ),
    entry!(
        "loudness_monitor",
        ["loudness_monitor"],
        Analyzer,
        builtin_metadata!(
            "sotf-host",
            "Loudness Monitor",
            Stable,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::Zero,
            "sotf_host::analyzer_loudness_monitor::parameters",
            Custom,
            false,
        ),
        analyzer_evidence(
            "sotf-host test_analyzer_plugins verifies exact passthrough, -20 dBFS 1 kHz loudness within 0.2 LU, peak, correlation, and multichannel slots"
        )
    ),
    entry!(
        "spectrum_analyzer",
        ["spectrum_analyzer"],
        Analyzer,
        builtin_metadata!(
            "sotf-host",
            "Spectrum Analyzer",
            Stable,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::Zero,
            "sotf_host::analyzer_spectrum::SpectrumConfig",
            Custom,
            false,
        ),
        analyzer_evidence(
            "sotf-host analyzer_spectrum tests verify exact passthrough, bin-centered and Nyquist 0 dBFS calibration within 0.1 dB, silence floor, and stereo/multichannel analysis"
        )
    ),
    entry!(
        "resampler",
        ["resampler"],
        Utility,
        PluginCatalogMetadata {
            owning_crate: "sotf-plugin-resampler",
            exposed_name: "Resampler",
            maturity: PluginMaturity::Infrastructure,
            channel_layout: PluginChannelLayoutContract {
                supported_inputs: PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS,),
                output: PluginChannelOutputModel::PreservesInput,
            },
            latency_model: PluginLatencyModel::FrameBased(
                "resampling ratio, quality, and chunk size",
            ),
            parameter_schema: PluginParameterSchema::Static(
                "sotf_plugin_resampler::params::PARAMS",
            ),
            preset_support: PluginPresetSupport::InfrastructureConfiguration,
            ui: PluginUiKind::NotExposed,
            external_reference: ExternalReferenceImplementation::NotApplicable(
                "validated against analytical resampling and anti-aliasing contracts",
            ),
            picker: PluginPickerExposure::Infrastructure,
            allowed_in_ab_compare: false,
        },
        infrastructure_evidence(
            "sotf-plugin-resampler ratio/frame-count, continuity, anti-aliasing, quality cutoff, flush, latency, multichannel, and variable-block tests"
        )
    ),
    entry!(
        "band_split",
        ["band_split"],
        Routing,
        builtin_metadata!(
            "sotf-plugin-band-split",
            "Band Split",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::InputTimesBands,
            PluginLatencyModel::PluginReported("crossover topology and band count"),
            "sotf_plugin_band_split::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-band-split two/three/four-band DC reconstruction, crossover spacing, smoothing, routing, and split/merge high-layout round trips"
        )
    ),
    entry!(
        "band_merge",
        ["band_merge"],
        Routing,
        builtin_metadata!(
            "sotf-plugin-band-merge",
            "Band Merge",
            Beta,
            PluginSupportedInputLayouts::Enumerated(EVEN_BAND_CHANNEL_WIDTHS),
            PluginChannelOutputModel::InputDividedByBands,
            PluginLatencyModel::PluginReported("band count and matching split latency"),
            "sotf_plugin_band_merge::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-band-merge reconstruction-error reference, unity/gain/mute behavior, routing, and split/merge high-layout round trips"
        )
    ),
    entry!(
        "ab_compare",
        ["ab_compare", "ab"],
        Utility,
        builtin_metadata!(
            "sotf-plugin-ab-compare",
            "A/B Compare",
            Beta,
            PluginSupportedInputLayouts::Enumerated(STANDARD_CHANNEL_WIDTHS),
            PluginChannelOutputModel::PreservesInput,
            PluginLatencyModel::PluginReported("maximum latency of the configured A/B paths"),
            "sotf_plugin_ab_compare::params::PARAMS",
            Custom,
            false,
        ),
        zero_alloc_evidence(
            "sotf-plugin-ab-compare level-match, phase inversion, delay compensation, switching/crossfade, sub-rack, reset, and channel-preservation tests"
        )
    ),
    entry!(
        "aec",
        ["aec"],
        Processor,
        builtin_metadata!(
            "sotf-plugin-aec",
            "AEC",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(STEREO_CHANNEL_WIDTH),
            PluginChannelOutputModel::Fixed(1),
            PluginLatencyModel::FrameBased("partitioned adaptive-filter frame"),
            "sotf_plugin_aec::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-aec no-echo passthrough, synthetic echo cancellation/convergence, two-path, post-filter, latency, reset, and stability tests"
        )
    ),
    entry!(
        "beamformer",
        ["beamformer"],
        Spatial,
        builtin_metadata!(
            "sotf-plugin-beamformer",
            "Beamformer",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(MICROPHONE_ARRAY_WIDTHS),
            PluginChannelOutputModel::Fixed(1),
            PluginLatencyModel::FrameBased("STFT frame and beamformer mode"),
            "sotf_plugin_beamformer::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-beamformer analytical steering delays/vectors, MVDR/GSC/superdirective weight, noise cancellation, STFT overlap, reset, and finite-output tests"
        )
    ),
    entry!(
        "ambisonics_decoder",
        ["ambisonics_decoder"],
        Spatial,
        builtin_metadata!(
            "sotf-plugin-ambisonics",
            "Ambisonics Decoder",
            Alpha,
            PluginSupportedInputLayouts::Enumerated(AMBISONIC_WIDTHS),
            PluginChannelOutputModel::Configurable {
                description: "decoder order and target speaker layout",
                default_output: PluginDefaultChannelOutput::Fixed(6),
            },
            PluginLatencyModel::PluginReported("dual-band crossover and decoder configuration"),
            "sotf_plugin_ambisonics::params::PARAMS",
            Generated,
            true,
        ),
        zero_alloc_evidence(
            "sotf-plugin-ambisonics spherical-harmonic/decode-matrix, max-rE, dual-band, channel-order, energy, reset, and ambisonics-to-binaural chain tests"
        )
    ),
    entry!(
        "external",
        ["external", "external_plugin"],
        ExternalHost,
        PluginCatalogMetadata {
            owning_crate: "sotf-host",
            exposed_name: "External Plugin",
            maturity: PluginMaturity::Alpha,
            channel_layout: PluginChannelLayoutContract {
                supported_inputs: PluginSupportedInputLayouts::DescriptorDefined,
                output: PluginChannelOutputModel::DescriptorDefined,
            },
            latency_model: PluginLatencyModel::HostedPluginReported,
            parameter_schema: PluginParameterSchema::DescriptorProvided,
            preset_support: PluginPresetSupport::ExternalOpaqueState,
            ui: PluginUiKind::ExternalHost,
            external_reference: ExternalReferenceImplementation::DescriptorDefined,
            picker: PluginPickerExposure::DiscoveredExternal,
            allowed_in_ab_compare: false,
        },
        external_host_evidence()
    ),
    #[cfg(all(target_os = "macos", feature = "hal"))]
    entry!(
        "hal_input",
        ["hal_input"],
        PlatformIo,
        PluginCatalogMetadata {
            owning_crate: "sotf-plugin-hal-input",
            exposed_name: "Systemwide HAL Input",
            maturity: PluginMaturity::Alpha,
            channel_layout: PluginChannelLayoutContract {
                supported_inputs: PluginSupportedInputLayouts::PlatformNegotiated,
                output: PluginChannelOutputModel::PlatformNegotiated,
            },
            latency_model: PluginLatencyModel::PlatformNegotiated,
            parameter_schema: PluginParameterSchema::PlatformTransport,
            preset_support: PluginPresetSupport::PlatformTransport,
            ui: PluginUiKind::Systemwide,
            external_reference: ExternalReferenceImplementation::NotApplicable(
                "platform transport node",
            ),
            picker: PluginPickerExposure::Systemwide,
            allowed_in_ab_compare: false,
        },
        platform_io_evidence()
    ),
    #[cfg(all(target_os = "macos", feature = "hal"))]
    entry!(
        "hal_output",
        ["hal_output"],
        PlatformIo,
        PluginCatalogMetadata {
            owning_crate: "sotf-plugin-hal-output",
            exposed_name: "Systemwide HAL Output",
            maturity: PluginMaturity::Alpha,
            channel_layout: PluginChannelLayoutContract {
                supported_inputs: PluginSupportedInputLayouts::PlatformNegotiated,
                output: PluginChannelOutputModel::PlatformNegotiated,
            },
            latency_model: PluginLatencyModel::PlatformNegotiated,
            parameter_schema: PluginParameterSchema::PlatformTransport,
            preset_support: PluginPresetSupport::PlatformTransport,
            ui: PluginUiKind::Systemwide,
            external_reference: ExternalReferenceImplementation::NotApplicable(
                "platform transport node",
            ),
            picker: PluginPickerExposure::Systemwide,
            allowed_in_ab_compare: false,
        },
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

/// Every alias accepted by the factory, derived from the canonical catalog.
pub fn supported_plugin_types() -> impl Iterator<Item = &'static str> {
    PLUGIN_CATALOG
        .iter()
        .flat_map(|entry| entry.aliases.iter().copied())
}

/// Catalog entries exposed through the generic built-in application picker.
pub fn generic_app_catalog_entries() -> impl Iterator<Item = &'static PluginCatalogEntry> {
    PLUGIN_CATALOG
        .iter()
        .filter(|entry| entry.is_generic_app_plugin())
}

/// Catalog entries safe to construct inside an A/B path without extra
/// discovery or platform state.
pub fn ab_compare_catalog_entries() -> impl Iterator<Item = &'static PluginCatalogEntry> {
    PLUGIN_CATALOG
        .iter()
        .filter(|entry| entry.is_allowed_in_ab_compare())
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
    fn catalog_aliases_are_unique_and_drive_factory_types() {
        let aliases: Vec<_> = PLUGIN_CATALOG
            .iter()
            .flat_map(|entry| entry.aliases.iter().copied())
            .collect();
        let alias_set: HashSet<_> = aliases.iter().copied().collect();
        let supported: Vec<_> = supported_plugin_types().collect();

        assert_eq!(
            aliases.len(),
            alias_set.len(),
            "catalog aliases must be unique"
        );
        assert_eq!(aliases, supported);
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
    fn every_catalog_row_has_complete_structured_inventory_metadata() {
        for entry in PLUGIN_CATALOG {
            assert!(!entry.metadata.owning_crate.trim().is_empty());
            assert!(!entry.metadata.exposed_name.trim().is_empty());
            match entry.metadata.parameter_schema {
                PluginParameterSchema::Static(path) => assert!(!path.trim().is_empty()),
                PluginParameterSchema::DescriptorProvided
                | PluginParameterSchema::PlatformTransport => {}
            }
            match entry.metadata.latency_model {
                PluginLatencyModel::PluginReported(description)
                | PluginLatencyModel::FrameBased(description) => {
                    assert!(!description.trim().is_empty())
                }
                PluginLatencyModel::Zero
                | PluginLatencyModel::HostedPluginReported
                | PluginLatencyModel::PlatformNegotiated => {}
            }
            if let PluginChannelOutputModel::Configurable { description, .. } =
                entry.metadata.channel_layout.output
            {
                assert!(!description.trim().is_empty());
            }
        }
    }

    #[test]
    fn stable_maturity_requires_every_applicable_gate() {
        let incomplete_stable: Vec<_> = PLUGIN_CATALOG
            .iter()
            .filter(|entry| entry.metadata.maturity == PluginMaturity::Stable)
            .filter(|entry| !entry.is_ready_for_stable())
            .map(|entry| (entry.canonical_type, entry.evidence.pending_gates()))
            .collect();
        assert!(
            incomplete_stable.is_empty(),
            "stable plugins lack release evidence: {incomplete_stable:?}"
        );
    }

    #[test]
    fn picker_views_are_unique_catalog_subsets() {
        let generic: Vec<_> = generic_app_catalog_entries().collect();
        let ab_compare: Vec<_> = ab_compare_catalog_entries().collect();
        let generic_types: HashSet<_> = generic.iter().map(|entry| entry.canonical_type).collect();
        let generic_names: HashSet<_> = generic
            .iter()
            .map(|entry| entry.metadata.exposed_name)
            .collect();

        assert_eq!(generic_types.len(), generic.len());
        assert_eq!(generic_names.len(), generic.len());
        for entry in ab_compare {
            assert!(generic_types.contains(entry.canonical_type));
            assert!(!matches!(
                entry.category,
                PluginCategory::Analyzer
                    | PluginCategory::ExternalHost
                    | PluginCategory::PlatformIo
            ));
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
            PLUGIN_CATALOG
                .iter()
                .filter(|entry| entry.category != PluginCategory::PlatformIo)
                .filter(|entry| matches!(
                    entry.evidence.realtime_allocation,
                    StabilityEvidenceState::Pending(_)
                ))
                .count(),
            0,
            "the allocation gate must cover all built-in and external-host process paths"
        );
        assert_eq!(
            PLUGIN_CATALOG
                .iter()
                .filter(|entry| entry.category != PluginCategory::PlatformIo)
                .filter(|entry| matches!(
                    entry.evidence.latency,
                    StabilityEvidenceState::Pending(_)
                ))
                .count(),
            0,
            "the latency gate must cover built-in and external-host contracts"
        );
        let pending_ui: Vec<_> = PLUGIN_CATALOG
            .iter()
            .filter(|entry| matches!(entry.evidence.ui, StabilityEvidenceState::Pending(_)))
            .map(|entry| entry.canonical_type)
            .collect();
        assert!(
            pending_ui.is_empty(),
            "every catalog entry must have covered or reviewed-not-applicable UI evidence: {pending_ui:?}"
        );
    }
}
