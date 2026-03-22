//! Denoiser plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Choice label constants
//! - Constants (LEARN_FRAMES)
//! - Serializable state (Params struct with serde defaults)
//! - Index↔field mapping (PluginParamDef impl)
//!
//! Adding a parameter: add to PARAMS, add field to Params, add match arms.
//! Nothing else needs to change.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{find_by_key as pk, ParamSpec};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Constants
// ============================================================================

pub const LEARN_FRAMES: usize = 50; // ~1s at typical hop rates

// ============================================================================
// Choice label constants
// ============================================================================

pub const ALGORITHMS: &[&str] = &["Classical", "RNNoise", "DeepFilter", "HybridNeural"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    // 0: reduction_db
    ParamSpec::float(
        "Reduction",
        "reduction_db",
        10.0,
        0.0,
        40.0,
        0.5,
        "dB",
        "General",
    )
    .doc("Noise attenuation amount"),
    // 1: floor_db
    ParamSpec::float(
        "Floor", "floor_db", -20.0, -60.0, -10.0, 0.5, "dB", "General",
    )
    .doc("Minimum gain floor (artifact limit)"),
    // 2: smoothing
    ParamSpec::float(
        "Smoothing",
        "smoothing",
        0.3,
        0.0,
        0.99,
        0.01,
        "",
        "General",
    )
    .scaled(100.0)
    .doc("Gain curve temporal smoothing"),
    // 3: attack_ms
    ParamSpec::float("Attack", "attack_ms", 5.0, 0.1, 100.0, 0.5, "ms", "Timing")
        .doc("Time to apply reduction"),
    // 4: release_ms
    ParamSpec::float(
        "Release",
        "release_ms",
        50.0,
        10.0,
        500.0,
        5.0,
        "ms",
        "Timing",
    )
    .doc("Time to release reduction"),
    // 5: low_latency
    ParamSpec::bool_param("Low Latency", "low_latency", false, "General")
        .structural()
        .setup()
        .doc("Smaller FFT for lower latency"),
    // 6: polyphonic_detection
    ParamSpec::bool_param("Polyphonic", "polyphonic_detection", false, "Analysis")
        .secondary("Analysis")
        .doc("Detect multiple pitched signals"),
    // 7: crack_sensitivity
    ParamSpec::float(
        "Crack Sens.",
        "crack_sensitivity",
        10.0,
        1.0,
        100.0,
        1.0,
        "",
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Click/crack detection sensitivity"),
    // 8: mcra_alpha_s
    ParamSpec::float(
        "MCRA Alpha S",
        "mcra_alpha_s",
        0.9,
        0.5,
        0.99,
        0.01,
        "",
        "Advanced",
    )
    .secondary("MCRA")
    .doc("Noise spectrum smoothing factor"),
    // 9: mcra_alpha_p
    ParamSpec::float(
        "MCRA Alpha P",
        "mcra_alpha_p",
        0.7,
        0.1,
        0.99,
        0.01,
        "",
        "Advanced",
    )
    .secondary("MCRA")
    .doc("Speech presence probability smooth"),
    // 10: mcra_l
    ParamSpec::int("MCRA Window", "mcra_l", 50, 10, 200, 1, "fr", "Advanced")
        .secondary("MCRA")
        .doc("Min statistics window length"),
    // 11: mcra_delta
    ParamSpec::float(
        "MCRA Delta",
        "mcra_delta",
        5.0,
        1.0,
        20.0,
        0.5,
        "",
        "Advanced",
    )
    .secondary("MCRA")
    .doc("Speech/noise discrimination bias"),
    // 12: transparency
    ParamSpec::float(
        "Transparency",
        "transparency",
        0.0,
        0.0,
        1.0,
        0.01,
        "",
        "General",
    )
    .scaled(100.0)
    .doc("Blend denoised toward dry signal"),
    // 13: dd_enabled
    ParamSpec::bool_param("DD SNR", "dd_enabled", true, "Analysis")
        .secondary("Analysis")
        .doc("Decision-Directed SNR estimator"),
    // 14: dd_alpha
    ParamSpec::float(
        "DD Alpha", "dd_alpha", 0.98, 0.5, 0.999, 0.001, "", "Analysis",
    )
    .secondary("Analysis")
    .doc("DD SNR smoothing coefficient"),
    // 15: psychoacoustic_masking
    ParamSpec::bool_param("Psychoacoustic", "psychoacoustic_masking", true, "Analysis")
        .secondary("Analysis")
        .doc("Use auditory masking curves"),
    // 16: transient_enabled
    ParamSpec::bool_param("Transient", "transient_enabled", true, "Analysis")
        .secondary("Analysis")
        .doc("Preserve transient details"),
    // 17: spectral_smoothing_enabled
    ParamSpec::bool_param(
        "Spectral Smooth",
        "spectral_smoothing_enabled",
        true,
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Smooth gain across frequency bins"),
    // 18: temporal_smoothing_enabled
    ParamSpec::bool_param(
        "Temporal Smooth",
        "temporal_smoothing_enabled",
        true,
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Smooth gain across time frames"),
    // 19: hiss_enabled
    ParamSpec::bool_param("Hiss Remover", "hiss_enabled", false, "Hiss")
        .secondary("Hiss")
        .doc("Enable dedicated hiss reduction"),
    // 20: hiss_threshold_db
    ParamSpec::float(
        "Hiss Threshold",
        "hiss_threshold_db",
        -30.0,
        -60.0,
        -10.0,
        0.5,
        "dB",
        "Hiss",
    )
    .secondary("Hiss")
    .doc("Level above which hiss is removed"),
    // 21: hiss_frequency_hz
    ParamSpec::float(
        "Hiss Frequency",
        "hiss_frequency_hz",
        4000.0,
        1000.0,
        16000.0,
        100.0,
        "Hz",
        "Hiss",
    )
    .secondary("Hiss")
    .doc("Corner freq for hiss detection"),
    // 22: hiss_strength
    ParamSpec::float(
        "Hiss Strength",
        "hiss_strength",
        0.5,
        0.0,
        1.0,
        0.01,
        "",
        "Hiss",
    )
    .scaled(100.0)
    .secondary("Hiss")
    .doc("Hiss removal aggressiveness"),
    // 23: spectral_sub_enabled
    ParamSpec::bool_param(
        "Spectral Sub",
        "spectral_sub_enabled",
        false,
        "Spectral Sub",
    )
    .secondary("Spectral Sub")
    .doc("Enable spectral subtraction"),
    // 24: spectral_sub_alpha
    ParamSpec::float(
        "Oversub Factor",
        "spectral_sub_alpha",
        2.0,
        0.5,
        6.0,
        0.1,
        "",
        "Spectral Sub",
    )
    .secondary("Spectral Sub")
    .doc("Over-subtraction factor (alpha)"),
    // 25: spectral_sub_beta
    ParamSpec::float(
        "Spectral Floor",
        "spectral_sub_beta",
        0.02,
        0.001,
        0.5,
        0.001,
        "",
        "Spectral Sub",
    )
    .secondary("Spectral Sub")
    .doc("Spectral floor factor (beta)"),
    // 26: learn_noise
    ParamSpec::bool_labeled(
        "Learn Noise",
        "learn_noise",
        false,
        "Active",
        "Off",
        "Noise Profile",
    )
    .structural()
    .secondary("Noise Profile")
    .doc("Capture noise-only reference"),
    // 27: use_captured_profile
    ParamSpec::bool_param(
        "Use Profile",
        "use_captured_profile",
        false,
        "Noise Profile",
    )
    .secondary("Noise Profile")
    .doc("Use captured noise profile"),
    // 28: clear_profile
    ParamSpec::bool_labeled(
        "Clear Profile",
        "clear_profile",
        false,
        "Trigger",
        "Off",
        "Noise Profile",
    )
    .structural()
    .secondary("Noise Profile")
    .doc("Discard captured noise profile"),
    // 29: algorithm
    ParamSpec::choice(
        "Algorithm",
        "algorithm",
        0,
        ALGORITHMS,
        "General",
    )
    .structural()
    .doc("Denoising algorithm selection"),
    // 30: formant_preservation
    ParamSpec::bool_param(
        "Formant Preserve",
        "formant_preservation",
        false,
        "Formant",
    )
    .secondary("Formant")
    .doc("Protect vocal formant structure"),
    // 31: formant_strength
    ParamSpec::float(
        "Formant Strength",
        "formant_strength",
        0.5,
        0.0,
        1.0,
        0.01,
        "",
        "Formant",
    )
    .scaled(100.0)
    .secondary("Formant")
    .doc("Formant preservation amount"),
    // 32: multi_resolution
    ParamSpec::bool_param(
        "Multi-Res",
        "multi_resolution",
        false,
        "General",
    )
    .structural()
    .secondary("General")
    .doc("Multi-resolution FFT analysis"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Denoiser: 0=reduction, 1=floor, 2=smoothing, 3=attack, 4=release,
/// 5=low_latency, 6=polyphonic, 7=crack_sens, 8-11=MCRA, 12=transparency,
/// 13-18=analysis toggles, 19-22=hiss, 23-25=spectral_sub, 26-28=noise_profile,
/// 29=algorithm, 30=formant_preservation, 31=formant_strength, 32=multi_resolution
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::toggle(5), // low_latency
    ],
    main: &[
        ControlGroup {
            title: "REDUCTION",
            controls: &[
                ControlSpec::slider(0),  // reduction_db
                ControlSpec::slider(1),  // floor_db
                ControlSpec::slider(2),  // smoothing
                ControlSpec::slider(12), // transparency
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::knob(3), // attack
                ControlSpec::knob(4), // release
            ],
        },
        ControlGroup {
            title: "HISS REDUCTION",
            controls: &[
                ControlSpec::toggle(19), // hiss_enabled
                ControlSpec::knob(20),   // hiss_threshold
                ControlSpec::knob(21),   // hiss_frequency
                ControlSpec::knob(22),   // hiss_strength
            ],
        },
        ControlGroup {
            title: "SPECTRAL SUB",
            controls: &[
                ControlSpec::toggle(23), // spectral_sub_enabled
                ControlSpec::knob(24),   // oversub_factor
                ControlSpec::knob(25),   // spectral_floor
            ],
        },
        ControlGroup {
            title: "NOISE PROFILE",
            controls: &[
                ControlSpec::toggle(26), // learn_noise
                ControlSpec::toggle(27), // use_captured_profile
                ControlSpec::toggle(28), // clear_profile
            ],
        },
    ],
    output: &[],
    tabs: &[
        TabSpec {
            name: "Analysis",
            controls: &[
                ControlSpec::toggle(6),  // polyphonic
                ControlSpec::knob(7),    // crack_sensitivity
                ControlSpec::toggle(13), // dd_enabled
                ControlSpec::knob(14),   // dd_alpha
                ControlSpec::toggle(15), // psychoacoustic_masking
                ControlSpec::toggle(16), // transient
                ControlSpec::toggle(17), // spectral_smoothing
                ControlSpec::toggle(18), // temporal_smoothing
            ],
        },
        TabSpec {
            name: "MCRA",
            controls: &[
                ControlSpec::knob(8),  // alpha_s
                ControlSpec::knob(9),  // alpha_p
                ControlSpec::knob(10), // window (int)
                ControlSpec::knob(11), // delta
            ],
        },
        TabSpec {
            name: "Formant",
            controls: &[
                ControlSpec::toggle(30), // formant_preservation
                ControlSpec::knob(31),   // formant_strength
            ],
        },
    ],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(500.0),
    ],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Denoiser plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_reduction_db")]
    pub reduction_db: f64,
    #[serde(default = "d_floor_db")]
    pub floor_db: f64,
    #[serde(default = "d_smoothing")]
    pub smoothing: f64,
    #[serde(default = "d_attack_ms")]
    pub attack_ms: f64,
    #[serde(default = "d_release_ms")]
    pub release_ms: f64,
    #[serde(default = "d_low_latency")]
    pub low_latency: bool,
    #[serde(default = "d_polyphonic_detection")]
    pub polyphonic_detection: bool,
    #[serde(default = "d_crack_sensitivity")]
    pub crack_sensitivity: f64,
    #[serde(default = "d_mcra_alpha_s")]
    pub mcra_alpha_s: f64,
    #[serde(default = "d_mcra_alpha_p")]
    pub mcra_alpha_p: f64,
    #[serde(default = "d_mcra_l")]
    pub mcra_l: usize,
    #[serde(default = "d_mcra_delta")]
    pub mcra_delta: f64,
    #[serde(default = "d_transparency")]
    pub transparency: f64,
    #[serde(default = "d_dd_enabled")]
    pub dd_enabled: bool,
    #[serde(default = "d_dd_alpha")]
    pub dd_alpha: f64,
    #[serde(default = "d_psychoacoustic_masking")]
    pub psychoacoustic_masking: bool,
    #[serde(default = "d_transient_enabled")]
    pub transient_enabled: bool,
    #[serde(default = "d_spectral_smoothing_enabled")]
    pub spectral_smoothing_enabled: bool,
    #[serde(default = "d_temporal_smoothing_enabled")]
    pub temporal_smoothing_enabled: bool,
    #[serde(default = "d_hiss_enabled")]
    pub hiss_enabled: bool,
    #[serde(default = "d_hiss_threshold_db")]
    pub hiss_threshold_db: f64,
    #[serde(default = "d_hiss_frequency_hz")]
    pub hiss_frequency_hz: f64,
    #[serde(default = "d_hiss_strength")]
    pub hiss_strength: f64,
    #[serde(default = "d_spectral_sub_enabled")]
    pub spectral_sub_enabled: bool,
    #[serde(default = "d_spectral_sub_alpha")]
    pub spectral_sub_alpha: f64,
    #[serde(default = "d_spectral_sub_beta")]
    pub spectral_sub_beta: f64,
    #[serde(default = "d_learn_noise")]
    pub learn_noise: bool,
    #[serde(default = "d_use_captured_profile")]
    pub use_captured_profile: bool,
    #[serde(default = "d_clear_profile")]
    pub clear_profile: bool,
    #[serde(default = "d_algorithm")]
    pub algorithm: usize,
    #[serde(default = "d_formant_preservation")]
    pub formant_preservation: bool,
    #[serde(default = "d_formant_strength")]
    pub formant_strength: f64,
    #[serde(default = "d_multi_resolution")]
    pub multi_resolution: bool,
}

fn d_reduction_db() -> f64 {
    pk(PARAMS, "reduction_db").default_f64()
}
fn d_floor_db() -> f64 {
    pk(PARAMS, "floor_db").default_f64()
}
fn d_smoothing() -> f64 {
    pk(PARAMS, "smoothing").default_f64()
}
fn d_attack_ms() -> f64 {
    pk(PARAMS, "attack_ms").default_f64()
}
fn d_release_ms() -> f64 {
    pk(PARAMS, "release_ms").default_f64()
}
fn d_low_latency() -> bool {
    pk(PARAMS, "low_latency").default_bool()
}
fn d_polyphonic_detection() -> bool {
    pk(PARAMS, "polyphonic_detection").default_bool()
}
fn d_crack_sensitivity() -> f64 {
    pk(PARAMS, "crack_sensitivity").default_f64()
}
fn d_mcra_alpha_s() -> f64 {
    pk(PARAMS, "mcra_alpha_s").default_f64()
}
fn d_mcra_alpha_p() -> f64 {
    pk(PARAMS, "mcra_alpha_p").default_f64()
}
fn d_mcra_l() -> usize {
    pk(PARAMS, "mcra_l").default_usize()
}
fn d_mcra_delta() -> f64 {
    pk(PARAMS, "mcra_delta").default_f64()
}
fn d_transparency() -> f64 {
    pk(PARAMS, "transparency").default_f64()
}
fn d_dd_enabled() -> bool {
    pk(PARAMS, "dd_enabled").default_bool()
}
fn d_dd_alpha() -> f64 {
    pk(PARAMS, "dd_alpha").default_f64()
}
fn d_psychoacoustic_masking() -> bool {
    pk(PARAMS, "psychoacoustic_masking").default_bool()
}
fn d_transient_enabled() -> bool {
    pk(PARAMS, "transient_enabled").default_bool()
}
fn d_spectral_smoothing_enabled() -> bool {
    pk(PARAMS, "spectral_smoothing_enabled").default_bool()
}
fn d_temporal_smoothing_enabled() -> bool {
    pk(PARAMS, "temporal_smoothing_enabled").default_bool()
}
fn d_hiss_enabled() -> bool {
    pk(PARAMS, "hiss_enabled").default_bool()
}
fn d_hiss_threshold_db() -> f64 {
    pk(PARAMS, "hiss_threshold_db").default_f64()
}
fn d_hiss_frequency_hz() -> f64 {
    pk(PARAMS, "hiss_frequency_hz").default_f64()
}
fn d_hiss_strength() -> f64 {
    pk(PARAMS, "hiss_strength").default_f64()
}
fn d_spectral_sub_enabled() -> bool {
    pk(PARAMS, "spectral_sub_enabled").default_bool()
}
fn d_spectral_sub_alpha() -> f64 {
    pk(PARAMS, "spectral_sub_alpha").default_f64()
}
fn d_spectral_sub_beta() -> f64 {
    pk(PARAMS, "spectral_sub_beta").default_f64()
}
fn d_learn_noise() -> bool {
    pk(PARAMS, "learn_noise").default_bool()
}
fn d_use_captured_profile() -> bool {
    pk(PARAMS, "use_captured_profile").default_bool()
}
fn d_clear_profile() -> bool {
    pk(PARAMS, "clear_profile").default_bool()
}
fn d_algorithm() -> usize {
    pk(PARAMS, "algorithm").default_usize()
}
fn d_formant_preservation() -> bool {
    pk(PARAMS, "formant_preservation").default_bool()
}
fn d_formant_strength() -> f64 {
    pk(PARAMS, "formant_strength").default_f64()
}
fn d_multi_resolution() -> bool {
    pk(PARAMS, "multi_resolution").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            reduction_db: d_reduction_db(),
            floor_db: d_floor_db(),
            smoothing: d_smoothing(),
            attack_ms: d_attack_ms(),
            release_ms: d_release_ms(),
            low_latency: d_low_latency(),
            polyphonic_detection: d_polyphonic_detection(),
            crack_sensitivity: d_crack_sensitivity(),
            mcra_alpha_s: d_mcra_alpha_s(),
            mcra_alpha_p: d_mcra_alpha_p(),
            mcra_l: d_mcra_l(),
            mcra_delta: d_mcra_delta(),
            transparency: d_transparency(),
            dd_enabled: d_dd_enabled(),
            dd_alpha: d_dd_alpha(),
            psychoacoustic_masking: d_psychoacoustic_masking(),
            transient_enabled: d_transient_enabled(),
            spectral_smoothing_enabled: d_spectral_smoothing_enabled(),
            temporal_smoothing_enabled: d_temporal_smoothing_enabled(),
            hiss_enabled: d_hiss_enabled(),
            hiss_threshold_db: d_hiss_threshold_db(),
            hiss_frequency_hz: d_hiss_frequency_hz(),
            hiss_strength: d_hiss_strength(),
            spectral_sub_enabled: d_spectral_sub_enabled(),
            spectral_sub_alpha: d_spectral_sub_alpha(),
            spectral_sub_beta: d_spectral_sub_beta(),
            learn_noise: d_learn_noise(),
            use_captured_profile: d_use_captured_profile(),
            clear_profile: d_clear_profile(),
            algorithm: d_algorithm(),
            formant_preservation: d_formant_preservation(),
            formant_strength: d_formant_strength(),
            multi_resolution: d_multi_resolution(),
        }
    }
}

// ============================================================================
// PluginParamDef implementation
// ============================================================================

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "denoiser";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.reduction_db),
            1 => Some(self.floor_db),
            2 => Some(self.smoothing),
            3 => Some(self.attack_ms),
            4 => Some(self.release_ms),
            5 => Some(if self.low_latency { 1.0 } else { 0.0 }),
            6 => Some(if self.polyphonic_detection { 1.0 } else { 0.0 }),
            7 => Some(self.crack_sensitivity),
            8 => Some(self.mcra_alpha_s),
            9 => Some(self.mcra_alpha_p),
            10 => Some(self.mcra_l as f64),
            11 => Some(self.mcra_delta),
            12 => Some(self.transparency),
            13 => Some(if self.dd_enabled { 1.0 } else { 0.0 }),
            14 => Some(self.dd_alpha),
            15 => Some(if self.psychoacoustic_masking { 1.0 } else { 0.0 }),
            16 => Some(if self.transient_enabled { 1.0 } else { 0.0 }),
            17 => Some(if self.spectral_smoothing_enabled { 1.0 } else { 0.0 }),
            18 => Some(if self.temporal_smoothing_enabled { 1.0 } else { 0.0 }),
            19 => Some(if self.hiss_enabled { 1.0 } else { 0.0 }),
            20 => Some(self.hiss_threshold_db),
            21 => Some(self.hiss_frequency_hz),
            22 => Some(self.hiss_strength),
            23 => Some(if self.spectral_sub_enabled { 1.0 } else { 0.0 }),
            24 => Some(self.spectral_sub_alpha),
            25 => Some(self.spectral_sub_beta),
            26 => Some(if self.learn_noise { 1.0 } else { 0.0 }),
            27 => Some(if self.use_captured_profile { 1.0 } else { 0.0 }),
            28 => Some(if self.clear_profile { 1.0 } else { 0.0 }),
            29 => Some(self.algorithm as f64),
            30 => Some(if self.formant_preservation { 1.0 } else { 0.0 }),
            31 => Some(self.formant_strength),
            32 => Some(if self.multi_resolution { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.reduction_db = value,
            1 => self.floor_db = value,
            2 => self.smoothing = value,
            3 => self.attack_ms = value,
            4 => self.release_ms = value,
            5 => self.low_latency = value > 0.5,
            6 => self.polyphonic_detection = value > 0.5,
            7 => self.crack_sensitivity = value,
            8 => self.mcra_alpha_s = value,
            9 => self.mcra_alpha_p = value,
            10 => self.mcra_l = value as usize,
            11 => self.mcra_delta = value,
            12 => self.transparency = value,
            13 => self.dd_enabled = value > 0.5,
            14 => self.dd_alpha = value,
            15 => self.psychoacoustic_masking = value > 0.5,
            16 => self.transient_enabled = value > 0.5,
            17 => self.spectral_smoothing_enabled = value > 0.5,
            18 => self.temporal_smoothing_enabled = value > 0.5,
            19 => self.hiss_enabled = value > 0.5,
            20 => self.hiss_threshold_db = value,
            21 => self.hiss_frequency_hz = value,
            22 => self.hiss_strength = value,
            23 => self.spectral_sub_enabled = value > 0.5,
            24 => self.spectral_sub_alpha = value,
            25 => self.spectral_sub_beta = value,
            26 => self.learn_noise = value > 0.5,
            27 => self.use_captured_profile = value > 0.5,
            28 => self.clear_profile = value > 0.5,
            29 => self.algorithm = value as usize,
            30 => self.formant_preservation = value > 0.5,
            31 => self.formant_strength = value,
            32 => self.multi_resolution = value > 0.5,
            _ => {}
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_index_coverage() {
        let p = Params::default();
        for i in 0..PARAMS.len() {
            assert!(
                p.param_value(i).is_some(),
                "param_value({}) returned None",
                i
            );
        }
        assert!(
            p.param_value(PARAMS.len()).is_none(),
            "param_value beyond PARAMS.len() should return None"
        );
    }

    #[test]
    fn param_count() {
        assert_eq!(PARAMS.len(), 33, "Expected 33 params");
    }

    #[test]
    fn roundtrip_serde() {
        let original = Params::default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: Params = serde_json::from_value(json).unwrap();
        assert_eq!(original.reduction_db, restored.reduction_db);
        assert_eq!(original.floor_db, restored.floor_db);
        assert_eq!(original.smoothing, restored.smoothing);
        assert_eq!(original.attack_ms, restored.attack_ms);
        assert_eq!(original.release_ms, restored.release_ms);
        assert_eq!(original.low_latency, restored.low_latency);
        assert_eq!(
            original.polyphonic_detection,
            restored.polyphonic_detection
        );
        assert_eq!(original.crack_sensitivity, restored.crack_sensitivity);
        assert_eq!(original.mcra_alpha_s, restored.mcra_alpha_s);
        assert_eq!(original.mcra_alpha_p, restored.mcra_alpha_p);
        assert_eq!(original.mcra_l, restored.mcra_l);
        assert_eq!(original.mcra_delta, restored.mcra_delta);
        assert_eq!(original.transparency, restored.transparency);
        assert_eq!(original.dd_enabled, restored.dd_enabled);
        assert_eq!(original.dd_alpha, restored.dd_alpha);
        assert_eq!(
            original.psychoacoustic_masking,
            restored.psychoacoustic_masking
        );
        assert_eq!(original.transient_enabled, restored.transient_enabled);
        assert_eq!(
            original.spectral_smoothing_enabled,
            restored.spectral_smoothing_enabled
        );
        assert_eq!(
            original.temporal_smoothing_enabled,
            restored.temporal_smoothing_enabled
        );
        assert_eq!(original.hiss_enabled, restored.hiss_enabled);
        assert_eq!(original.hiss_threshold_db, restored.hiss_threshold_db);
        assert_eq!(original.hiss_frequency_hz, restored.hiss_frequency_hz);
        assert_eq!(original.hiss_strength, restored.hiss_strength);
        assert_eq!(
            original.spectral_sub_enabled,
            restored.spectral_sub_enabled
        );
        assert_eq!(original.spectral_sub_alpha, restored.spectral_sub_alpha);
        assert_eq!(original.spectral_sub_beta, restored.spectral_sub_beta);
        assert_eq!(original.learn_noise, restored.learn_noise);
        assert_eq!(
            original.use_captured_profile,
            restored.use_captured_profile
        );
        assert_eq!(original.clear_profile, restored.clear_profile);
        assert_eq!(original.algorithm, restored.algorithm);
        assert_eq!(
            original.formant_preservation,
            restored.formant_preservation
        );
        assert_eq!(original.formant_strength, restored.formant_strength);
        assert_eq!(original.multi_resolution, restored.multi_resolution);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.reduction_db,
            pk(PARAMS, "reduction_db").default_f64()
        );
        assert_eq!(p.floor_db, pk(PARAMS, "floor_db").default_f64());
        assert_eq!(p.smoothing, pk(PARAMS, "smoothing").default_f64());
        assert_eq!(p.attack_ms, pk(PARAMS, "attack_ms").default_f64());
        assert_eq!(p.release_ms, pk(PARAMS, "release_ms").default_f64());
        assert_eq!(p.low_latency, pk(PARAMS, "low_latency").default_bool());
        assert_eq!(
            p.polyphonic_detection,
            pk(PARAMS, "polyphonic_detection").default_bool()
        );
        assert_eq!(
            p.crack_sensitivity,
            pk(PARAMS, "crack_sensitivity").default_f64()
        );
        assert_eq!(
            p.mcra_alpha_s,
            pk(PARAMS, "mcra_alpha_s").default_f64()
        );
        assert_eq!(
            p.mcra_alpha_p,
            pk(PARAMS, "mcra_alpha_p").default_f64()
        );
        assert_eq!(p.mcra_l, pk(PARAMS, "mcra_l").default_usize());
        assert_eq!(p.mcra_delta, pk(PARAMS, "mcra_delta").default_f64());
        assert_eq!(
            p.transparency,
            pk(PARAMS, "transparency").default_f64()
        );
        assert_eq!(p.dd_enabled, pk(PARAMS, "dd_enabled").default_bool());
        assert_eq!(p.dd_alpha, pk(PARAMS, "dd_alpha").default_f64());
        assert_eq!(
            p.psychoacoustic_masking,
            pk(PARAMS, "psychoacoustic_masking").default_bool()
        );
        assert_eq!(
            p.transient_enabled,
            pk(PARAMS, "transient_enabled").default_bool()
        );
        assert_eq!(
            p.spectral_smoothing_enabled,
            pk(PARAMS, "spectral_smoothing_enabled").default_bool()
        );
        assert_eq!(
            p.temporal_smoothing_enabled,
            pk(PARAMS, "temporal_smoothing_enabled").default_bool()
        );
        assert_eq!(p.hiss_enabled, pk(PARAMS, "hiss_enabled").default_bool());
        assert_eq!(
            p.hiss_threshold_db,
            pk(PARAMS, "hiss_threshold_db").default_f64()
        );
        assert_eq!(
            p.hiss_frequency_hz,
            pk(PARAMS, "hiss_frequency_hz").default_f64()
        );
        assert_eq!(
            p.hiss_strength,
            pk(PARAMS, "hiss_strength").default_f64()
        );
        assert_eq!(
            p.spectral_sub_enabled,
            pk(PARAMS, "spectral_sub_enabled").default_bool()
        );
        assert_eq!(
            p.spectral_sub_alpha,
            pk(PARAMS, "spectral_sub_alpha").default_f64()
        );
        assert_eq!(
            p.spectral_sub_beta,
            pk(PARAMS, "spectral_sub_beta").default_f64()
        );
        assert_eq!(p.learn_noise, pk(PARAMS, "learn_noise").default_bool());
        assert_eq!(
            p.use_captured_profile,
            pk(PARAMS, "use_captured_profile").default_bool()
        );
        assert_eq!(
            p.clear_profile,
            pk(PARAMS, "clear_profile").default_bool()
        );
        assert_eq!(p.algorithm, pk(PARAMS, "algorithm").default_usize());
        assert_eq!(
            p.formant_preservation,
            pk(PARAMS, "formant_preservation").default_bool()
        );
        assert_eq!(
            p.formant_strength,
            pk(PARAMS, "formant_strength").default_f64()
        );
        assert_eq!(
            p.multi_resolution,
            pk(PARAMS, "multi_resolution").default_bool()
        );
    }

    #[test]
    fn algorithm_labels_match() {
        let labels = pk(PARAMS, "algorithm").choice_labels();
        assert_eq!(labels, ALGORITHMS);
    }
}
