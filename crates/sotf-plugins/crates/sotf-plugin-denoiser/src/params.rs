//! Classical spectral denoiser parameter definitions.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

pub const LEARN_FRAMES: usize = 50;

pub const PARAMS: &[ParamSpec] = &[
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
    ParamSpec::float(
        "Floor", "floor_db", -20.0, -60.0, -10.0, 0.5, "dB", "General",
    )
    .doc("Minimum gain floor (artifact limit)"),
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
    ParamSpec::float("Attack", "attack_ms", 5.0, 0.1, 100.0, 0.5, "ms", "Timing")
        .doc("Time to apply reduction"),
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
    ParamSpec::bool_param("Low Latency", "low_latency", false, "General")
        .structural()
        .setup()
        .doc("Smaller FFT for lower latency"),
    ParamSpec::bool_param("Polyphonic", "polyphonic_detection", false, "Analysis")
        .secondary("Analysis")
        .doc("Detect multiple pitched signals"),
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
    ParamSpec::int("MCRA Window", "mcra_l", 50, 10, 200, 1, "fr", "Advanced")
        .secondary("MCRA")
        .doc("Min statistics window length"),
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
    ParamSpec::bool_param("DD SNR", "dd_enabled", true, "Analysis")
        .secondary("Analysis")
        .doc("Decision-Directed SNR estimator"),
    ParamSpec::float(
        "DD Alpha", "dd_alpha", 0.98, 0.5, 0.999, 0.001, "", "Analysis",
    )
    .secondary("Analysis")
    .doc("DD SNR smoothing coefficient"),
    ParamSpec::bool_param("Psychoacoustic", "psychoacoustic_masking", true, "Analysis")
        .secondary("Analysis")
        .doc("Use auditory masking curves"),
    ParamSpec::bool_param(
        "Spectral Smooth",
        "spectral_smoothing_enabled",
        true,
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Smooth gain across frequency bins"),
    ParamSpec::bool_param(
        "Temporal Smooth",
        "temporal_smoothing_enabled",
        true,
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Smooth gain across time frames"),
    ParamSpec::bool_param(
        "Spectral Sub",
        "spectral_sub_enabled",
        false,
        "Spectral Sub",
    )
    .secondary("Spectral Sub")
    .doc("Enable spectral subtraction"),
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
    ParamSpec::bool_param(
        "Use Profile",
        "use_captured_profile",
        false,
        "Noise Profile",
    )
    .secondary("Noise Profile")
    .doc("Use captured noise profile"),
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
    ParamSpec::bool_param("Formant Preserve", "formant_preservation", false, "Formant")
        .secondary("Formant")
        .doc("Protect vocal formant structure"),
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
    ParamSpec::bool_param("Multi-Res", "multi_resolution", false, "General")
        .structural()
        .setup()
        .secondary("General")
        .doc("Multi-resolution FFT analysis"),
    ParamSpec::bool_param(
        "Harmonic/Percussive",
        "harmonic_percussive",
        false,
        "Advanced",
    )
    .doc("Separate tonal and transient components for differential denoising"),
    ParamSpec::bool_param("Spatial Denoise", "spatial_denoise", false, "Advanced")
        .doc("Use inter-channel coherence for noise detection (stereo+ only)"),
    ParamSpec::float(
        "Spatial Strength",
        "spatial_strength",
        0.5,
        0.0,
        1.0,
        0.1,
        "",
        "Advanced",
    )
    .doc("Weight of inter-channel coherence in noise estimation"),
];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::toggle(5)],
    main: &[
        ControlGroup {
            title: "REDUCTION",
            controls: &[
                ControlSpec::slider(0),
                ControlSpec::slider(1),
                ControlSpec::slider(2),
                ControlSpec::slider(11),
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[ControlSpec::knob(3), ControlSpec::knob(4)],
        },
        ControlGroup {
            title: "SPECTRAL SUB",
            controls: &[
                ControlSpec::toggle(17),
                ControlSpec::knob(18),
                ControlSpec::knob(19),
            ],
        },
        ControlGroup {
            title: "NOISE PROFILE",
            controls: &[
                ControlSpec::toggle(20),
                ControlSpec::toggle(21),
                ControlSpec::toggle(22),
            ],
        },
    ],
    output: &[],
    tabs: &[
        TabSpec {
            name: "Analysis",
            controls: &[
                ControlSpec::toggle(6),
                ControlSpec::toggle(12),
                ControlSpec::knob(13),
                ControlSpec::toggle(14),
                ControlSpec::toggle(15),
                ControlSpec::toggle(16),
            ],
        },
        TabSpec {
            name: "MCRA",
            controls: &[
                ControlSpec::knob(7),
                ControlSpec::knob(8),
                ControlSpec::knob(9),
                ControlSpec::knob(10),
            ],
        },
        TabSpec {
            name: "Formant",
            controls: &[ControlSpec::toggle(23), ControlSpec::knob(24)],
        },
        TabSpec {
            name: "Advanced",
            controls: &[
                ControlSpec::toggle(25),
                ControlSpec::toggle(26),
                ControlSpec::toggle(27),
                ControlSpec::knob(28),
            ],
        },
    ],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(500.0),
    ],
    dynamic_sections: &[],
};

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
    #[serde(default = "d_spectral_smoothing_enabled")]
    pub spectral_smoothing_enabled: bool,
    #[serde(default = "d_temporal_smoothing_enabled")]
    pub temporal_smoothing_enabled: bool,
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
    #[serde(default = "d_formant_preservation")]
    pub formant_preservation: bool,
    #[serde(default = "d_formant_strength")]
    pub formant_strength: f64,
    #[serde(default = "d_multi_resolution")]
    pub multi_resolution: bool,
    #[serde(default)]
    pub harmonic_percussive: bool,
    #[serde(default)]
    pub spatial_denoise: bool,
    #[serde(default = "d_spatial_strength")]
    pub spatial_strength: f64,
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
fn d_spectral_smoothing_enabled() -> bool {
    pk(PARAMS, "spectral_smoothing_enabled").default_bool()
}
fn d_temporal_smoothing_enabled() -> bool {
    pk(PARAMS, "temporal_smoothing_enabled").default_bool()
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
fn d_formant_preservation() -> bool {
    pk(PARAMS, "formant_preservation").default_bool()
}
fn d_formant_strength() -> f64 {
    pk(PARAMS, "formant_strength").default_f64()
}
fn d_multi_resolution() -> bool {
    pk(PARAMS, "multi_resolution").default_bool()
}
fn d_spatial_strength() -> f64 {
    pk(PARAMS, "spatial_strength").default_f64()
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
            mcra_alpha_s: d_mcra_alpha_s(),
            mcra_alpha_p: d_mcra_alpha_p(),
            mcra_l: d_mcra_l(),
            mcra_delta: d_mcra_delta(),
            transparency: d_transparency(),
            dd_enabled: d_dd_enabled(),
            dd_alpha: d_dd_alpha(),
            psychoacoustic_masking: d_psychoacoustic_masking(),
            spectral_smoothing_enabled: d_spectral_smoothing_enabled(),
            temporal_smoothing_enabled: d_temporal_smoothing_enabled(),
            spectral_sub_enabled: d_spectral_sub_enabled(),
            spectral_sub_alpha: d_spectral_sub_alpha(),
            spectral_sub_beta: d_spectral_sub_beta(),
            learn_noise: d_learn_noise(),
            use_captured_profile: d_use_captured_profile(),
            clear_profile: d_clear_profile(),
            formant_preservation: d_formant_preservation(),
            formant_strength: d_formant_strength(),
            multi_resolution: d_multi_resolution(),
            harmonic_percussive: false,
            spatial_denoise: false,
            spatial_strength: d_spatial_strength(),
        }
    }
}

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 2;
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
            7 => Some(self.mcra_alpha_s),
            8 => Some(self.mcra_alpha_p),
            9 => Some(self.mcra_l as f64),
            10 => Some(self.mcra_delta),
            11 => Some(self.transparency),
            12 => Some(if self.dd_enabled { 1.0 } else { 0.0 }),
            13 => Some(self.dd_alpha),
            14 => Some(if self.psychoacoustic_masking {
                1.0
            } else {
                0.0
            }),
            15 => Some(if self.spectral_smoothing_enabled {
                1.0
            } else {
                0.0
            }),
            16 => Some(if self.temporal_smoothing_enabled {
                1.0
            } else {
                0.0
            }),
            17 => Some(if self.spectral_sub_enabled { 1.0 } else { 0.0 }),
            18 => Some(self.spectral_sub_alpha),
            19 => Some(self.spectral_sub_beta),
            20 => Some(if self.learn_noise { 1.0 } else { 0.0 }),
            21 => Some(if self.use_captured_profile { 1.0 } else { 0.0 }),
            22 => Some(if self.clear_profile { 1.0 } else { 0.0 }),
            23 => Some(if self.formant_preservation { 1.0 } else { 0.0 }),
            24 => Some(self.formant_strength),
            25 => Some(if self.multi_resolution { 1.0 } else { 0.0 }),
            26 => Some(if self.harmonic_percussive { 1.0 } else { 0.0 }),
            27 => Some(if self.spatial_denoise { 1.0 } else { 0.0 }),
            28 => Some(self.spatial_strength),
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
            7 => self.mcra_alpha_s = value,
            8 => self.mcra_alpha_p = value,
            9 => self.mcra_l = value as usize,
            10 => self.mcra_delta = value,
            11 => self.transparency = value,
            12 => self.dd_enabled = value > 0.5,
            13 => self.dd_alpha = value,
            14 => self.psychoacoustic_masking = value > 0.5,
            15 => self.spectral_smoothing_enabled = value > 0.5,
            16 => self.temporal_smoothing_enabled = value > 0.5,
            17 => self.spectral_sub_enabled = value > 0.5,
            18 => self.spectral_sub_alpha = value,
            19 => self.spectral_sub_beta = value,
            20 => self.learn_noise = value > 0.5,
            21 => self.use_captured_profile = value > 0.5,
            22 => self.clear_profile = value > 0.5,
            23 => self.formant_preservation = value > 0.5,
            24 => self.formant_strength = value,
            25 => self.multi_resolution = value > 0.5,
            26 => self.harmonic_percussive = value > 0.5,
            27 => self.spatial_denoise = value > 0.5,
            28 => self.spatial_strength = value,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_index_coverage() {
        let p = Params::default();
        for i in 0..PARAMS.len() {
            assert!(p.param_value(i).is_some());
        }
        assert!(p.param_value(PARAMS.len()).is_none());
    }

    #[test]
    fn param_count() {
        assert_eq!(PARAMS.len(), 29);
    }

    #[test]
    fn empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.reduction_db, pk(PARAMS, "reduction_db").default_f64());
        assert_eq!(
            p.multi_resolution,
            pk(PARAMS, "multi_resolution").default_bool()
        );
    }
}
