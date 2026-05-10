//! AAE plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Choice label constants
//! - Serializable state (AaePluginParams struct with serde defaults)
//!
//! Adding a parameter: add to PARAMS, add field to AaePluginParams, add match arms.
//! Nothing else needs to change.

use crate::early_reflections::RoomPreset;
use serde::{Deserialize, Serialize};
use sotf_host::param_specs::ParamSpec;
use sotf_host::parameters::Parameter;
use sotf_host::plugin_layout::*;

// ============================================================================
// Choice label constants
// ============================================================================

pub const SPEAKER_CONFIGS: &[&str] = &[
    "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
];

pub const ROOM_PRESETS: &[&str] = &["small", "medium", "large", "cathedral"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    // 0: speaker_config
    ParamSpec::choice(
        "Speaker Config",
        "speaker_config",
        1,
        SPEAKER_CONFIGS,
        "Spatial",
    )
    .structural()
    .setup()
    .doc("Output speaker layout"),
    // 1: room_size
    ParamSpec::float("Room Size", "room_size", 1.0, 0.2, 3.0, 0.1, "x", "Room")
        .doc("Scales all delay line lengths"),
    // 2: rt60
    ParamSpec::float("RT60", "rt60", 1.8, 0.3, 6.0, 0.1, "s", "Room")
        .doc("Mid-frequency reverberation time"),
    // 3: bass_ratio
    ParamSpec::float("Bass Ratio", "bass_ratio", 1.2, 0.8, 2.0, 0.05, "x", "Room")
        .doc("RT60_bass / RT60_mid ratio"),
    // 4: treble_ratio
    ParamSpec::float(
        "Treble Ratio",
        "treble_ratio",
        0.5,
        0.2,
        1.0,
        0.05,
        "x",
        "Room",
    )
    .doc("RT60_treble / RT60_mid ratio"),
    // 5: pre_delay_ms
    ParamSpec::float(
        "Pre-delay",
        "pre_delay_ms",
        20.0,
        0.0,
        100.0,
        1.0,
        "ms",
        "Room",
    )
    .doc("Gap before first reflection"),
    // 6: room_preset
    ParamSpec::choice("Room Preset", "room_preset", 1, ROOM_PRESETS, "Room")
        .doc("Early reflection tap configuration"),
    // 7: dry_level
    ParamSpec::float("Dry Level", "dry_level", 0.5, 0.0, 1.0, 0.01, "x", "Levels")
        .doc("Direct signal level"),
    // 8: er_level
    ParamSpec::float("ER Level", "er_level", 0.3, 0.0, 1.0, 0.01, "x", "Levels")
        .doc("Early reflection level"),
    // 9: late_level
    ParamSpec::float(
        "Late Level",
        "late_level",
        0.2,
        0.0,
        1.0,
        0.01,
        "x",
        "Levels",
    )
    .doc("Late reverb (FDN) level"),
    // 10: lfe_level
    ParamSpec::float("LFE Level", "lfe_level", 0.2, 0.0, 1.0, 0.01, "x", "Levels")
        .doc("Bass sent to LFE channel"),
    // 11: mod_depth
    ParamSpec::float(
        "Mod Depth",
        "mod_depth",
        0.5,
        0.0,
        1.0,
        0.01,
        "x",
        "Modulation",
    )
    .doc("FDN time-variant delay modulation (Griesinger)"),
    // 12: er_mod_depth
    ParamSpec::float(
        "ER Mod Depth",
        "er_mod_depth",
        0.3,
        0.0,
        1.0,
        0.01,
        "x",
        "Modulation",
    )
    .doc("Early reflection tap modulation"),
    // 13: input_diffusion
    ParamSpec::float(
        "Input Diffusion",
        "input_diffusion",
        0.7,
        0.0,
        1.0,
        0.01,
        "x",
        "Character",
    )
    .doc("Pre-FDN allpass diffusion"),
    // 14: envelopment
    ParamSpec::float(
        "Envelopment",
        "envelopment",
        0.7,
        0.0,
        1.0,
        0.01,
        "x",
        "Spatial",
    )
    .doc("Rear/surround vs front reverb balance"),
    // 15: height_amount
    ParamSpec::float(
        "Height Amount",
        "height_amount",
        0.5,
        0.0,
        1.0,
        0.01,
        "x",
        "Spatial",
    )
    .doc("Height channel contribution"),
    // 16: content_aware
    ParamSpec::bool_param("Content Aware", "content_aware", true, "Intelligence")
        .doc("Enable speech detection for reverb ducking"),
    // 17: dialogue_attenuation_db
    ParamSpec::float(
        "Dialogue Atten.",
        "dialogue_attenuation_db",
        6.0,
        0.0,
        12.0,
        0.5,
        "dB",
        "Intelligence",
    )
    .doc("Reverb reduction during detected speech"),
    // 18: safety_limit_db
    ParamSpec::float(
        "Safety Limit",
        "safety_limit_db",
        6.0,
        0.0,
        12.0,
        0.5,
        "dB",
        "Intelligence",
    )
    .doc("FDN feedback limiter threshold"),
    // 19: auto_gain_enabled
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", false, "Auto Gain")
        .output()
        .doc("Match rendered output loudness to the stereo input"),
    // 20: auto_gain_max_db
    ParamSpec::float(
        "AG Max",
        "auto_gain_max_db",
        12.0,
        0.0,
        24.0,
        1.0,
        "dB",
        "Auto Gain",
    )
    .output()
    .doc("Maximum auto gain correction"),
    // 21: auto_gain_smoothing_ms
    ParamSpec::float(
        "AG Smoothing",
        "auto_gain_smoothing_ms",
        100.0,
        10.0,
        500.0,
        5.0,
        "ms",
        "Auto Gain",
    )
    .output()
    .doc("Auto gain transition time"),
    // 22: bypass
    ParamSpec::bool_param("Bypass", "bypass", false, "Diagnostic").doc("Pass-through mode"),
    // 23: solo_early
    ParamSpec::bool_param("Solo Early", "solo_early", false, "Diagnostic")
        .doc("Hear only early reflections"),
    // 24: solo_late
    ParamSpec::bool_param("Solo Late", "solo_late", false, "Diagnostic")
        .doc("Hear only late reverb"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // speaker_config
        ControlSpec::selector(6), // room_preset
    ],
    main: &[
        ControlGroup {
            title: "ROOM",
            controls: &[
                ControlSpec::slider(1), // room_size
                ControlSpec::slider(2), // rt60
                ControlSpec::slider(3), // bass_ratio
                ControlSpec::slider(4), // treble_ratio
                ControlSpec::slider(5), // pre_delay_ms
            ],
        },
        ControlGroup {
            title: "LEVELS",
            controls: &[
                ControlSpec::slider(7),  // dry_level
                ControlSpec::slider(8),  // er_level
                ControlSpec::slider(9),  // late_level
                ControlSpec::slider(10), // lfe_level
            ],
        },
    ],
    output: &[
        ControlSpec::knob(18),   // safety_limit_db
        ControlSpec::toggle(19), // auto_gain_enabled
        ControlSpec::knob(20),   // auto_gain_max_db
        ControlSpec::knob(21),   // auto_gain_smoothing_ms
    ],
    tabs: &[
        TabSpec {
            name: "Spatial",
            controls: &[
                ControlSpec::knob(14), // envelopment
                ControlSpec::knob(15), // height_amount
            ],
        },
        TabSpec {
            name: "Modulation",
            controls: &[
                ControlSpec::knob(11), // mod_depth
                ControlSpec::knob(12), // er_mod_depth
                ControlSpec::knob(13), // input_diffusion
            ],
        },
        TabSpec {
            name: "Intelligence",
            controls: &[
                ControlSpec::toggle(16), // content_aware
                ControlSpec::knob(17),   // dialogue_attenuation_db
            ],
        },
        TabSpec {
            name: "Diagnostics",
            controls: &[
                ControlSpec::toggle(22), // bypass
                ControlSpec::toggle(23), // solo_early
                ControlSpec::toggle(24), // solo_late
            ],
        },
    ],
    visualizations: &[],
    column_constraints: &[],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Params (serde defaults from PARAMS)
// ============================================================================

sotf_host::serde_param_default! {
    PARAMS;
    fn default_room_size() -> f32 = "room_size";
    fn default_rt60() -> f32 = "rt60";
    fn default_bass_ratio() -> f32 = "bass_ratio";
    fn default_treble_ratio() -> f32 = "treble_ratio";
    fn default_pre_delay_ms() -> f32 = "pre_delay_ms";
    fn default_dry_level() -> f32 = "dry_level";
    fn default_er_level() -> f32 = "er_level";
    fn default_late_level() -> f32 = "late_level";
    fn default_lfe_level() -> f32 = "lfe_level";
    fn default_mod_depth() -> f32 = "mod_depth";
    fn default_er_mod_depth() -> f32 = "er_mod_depth";
    fn default_input_diffusion() -> f32 = "input_diffusion";
    fn default_envelopment() -> f32 = "envelopment";
    fn default_height_amount() -> f32 = "height_amount";
    fn default_content_aware() -> bool = "content_aware";
    fn default_dialogue_attenuation_db() -> f32 = "dialogue_attenuation_db";
    fn default_safety_limit_db() -> f32 = "safety_limit_db";
    fn default_auto_gain_enabled() -> bool = "auto_gain_enabled";
    fn default_auto_gain_max_db() -> f32 = "auto_gain_max_db";
    fn default_auto_gain_smoothing_ms() -> f32 = "auto_gain_smoothing_ms";
}

fn default_speaker_config() -> String {
    "5.1".to_string()
}
fn default_room_preset() -> String {
    "medium".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AaePluginParams {
    #[serde(default = "default_speaker_config")]
    pub speaker_config: String,
    #[serde(default = "default_room_size")]
    pub room_size: f32,
    #[serde(default = "default_rt60")]
    pub rt60: f32,
    #[serde(default = "default_bass_ratio")]
    pub bass_ratio: f32,
    #[serde(default = "default_treble_ratio")]
    pub treble_ratio: f32,
    #[serde(default = "default_pre_delay_ms")]
    pub pre_delay_ms: f32,
    #[serde(default = "default_room_preset")]
    pub room_preset: String,
    #[serde(default = "default_dry_level")]
    pub dry_level: f32,
    #[serde(default = "default_er_level")]
    pub er_level: f32,
    #[serde(default = "default_late_level")]
    pub late_level: f32,
    #[serde(default = "default_lfe_level")]
    pub lfe_level: f32,
    #[serde(default = "default_mod_depth")]
    pub mod_depth: f32,
    #[serde(default = "default_er_mod_depth")]
    pub er_mod_depth: f32,
    #[serde(default = "default_input_diffusion")]
    pub input_diffusion: f32,
    #[serde(default = "default_envelopment")]
    pub envelopment: f32,
    #[serde(default = "default_height_amount")]
    pub height_amount: f32,
    #[serde(default = "default_content_aware")]
    pub content_aware: bool,
    #[serde(default = "default_dialogue_attenuation_db")]
    pub dialogue_attenuation_db: f32,
    #[serde(default = "default_safety_limit_db")]
    pub safety_limit_db: f32,
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "default_auto_gain_max_db")]
    pub auto_gain_max_db: f32,
    #[serde(default = "default_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f32,
    #[serde(default)]
    pub bypass: bool,
    #[serde(default)]
    pub solo_early: bool,
    #[serde(default)]
    pub solo_late: bool,
}

impl Default for AaePluginParams {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}

impl AaePluginParams {
    pub fn room_preset_enum(&self) -> RoomPreset {
        match self.room_preset.to_lowercase().as_str() {
            "small" => RoomPreset::Small,
            "large" => RoomPreset::Large,
            "cathedral" => RoomPreset::Cathedral,
            _ => RoomPreset::Medium,
        }
    }
}

/// Build the cached parameter list for the Plugin trait.
pub fn build_parameters(params: &AaePluginParams) -> Vec<Parameter> {
    vec![
        Parameter::new_float("room_size", "Room Size", params.room_size, 0.2, 3.0),
        Parameter::new_float("rt60", "RT60", params.rt60, 0.3, 6.0).with_unit("s"),
        Parameter::new_float("bass_ratio", "Bass Ratio", params.bass_ratio, 0.8, 2.0),
        Parameter::new_float(
            "treble_ratio",
            "Treble Ratio",
            params.treble_ratio,
            0.2,
            1.0,
        ),
        Parameter::new_float("pre_delay_ms", "Pre-delay", params.pre_delay_ms, 0.0, 100.0)
            .with_unit("ms"),
        Parameter::new_string("room_preset", "Room Preset", params.room_preset.clone()),
        Parameter::new_float("dry_level", "Dry Level", params.dry_level, 0.0, 1.0),
        Parameter::new_float(
            "er_level",
            "Early Reflection Level",
            params.er_level,
            0.0,
            1.0,
        ),
        Parameter::new_float(
            "late_level",
            "Late Reverb Level",
            params.late_level,
            0.0,
            1.0,
        ),
        Parameter::new_float("lfe_level", "LFE Reverb Level", params.lfe_level, 0.0, 1.0),
        Parameter::new_float("mod_depth", "Mod Depth", params.mod_depth, 0.0, 1.0),
        Parameter::new_float(
            "er_mod_depth",
            "ER Mod Depth",
            params.er_mod_depth,
            0.0,
            1.0,
        ),
        Parameter::new_float(
            "input_diffusion",
            "Input Diffusion",
            params.input_diffusion,
            0.0,
            1.0,
        ),
        Parameter::new_string(
            "speaker_config",
            "Speaker Config",
            params.speaker_config.clone(),
        ),
        Parameter::new_float("envelopment", "Envelopment", params.envelopment, 0.0, 1.0),
        Parameter::new_float(
            "height_amount",
            "Height Amount",
            params.height_amount,
            0.0,
            1.0,
        ),
        Parameter::new_bool("content_aware", "Content Aware", params.content_aware),
        Parameter::new_float(
            "dialogue_attenuation_db",
            "Dialogue Attenuation",
            params.dialogue_attenuation_db,
            0.0,
            12.0,
        )
        .with_unit("dB"),
        Parameter::new_float(
            "safety_limit_db",
            "Safety Limit",
            params.safety_limit_db,
            0.0,
            12.0,
        )
        .with_unit("dB"),
        Parameter::new_bool("auto_gain_enabled", "Auto Gain", params.auto_gain_enabled),
        Parameter::new_float(
            "auto_gain_max_db",
            "Auto Gain Max",
            params.auto_gain_max_db,
            0.0,
            24.0,
        )
        .with_unit("dB"),
        Parameter::new_float(
            "auto_gain_smoothing_ms",
            "Auto Gain Smoothing",
            params.auto_gain_smoothing_ms,
            10.0,
            500.0,
        )
        .with_unit("ms"),
        Parameter::new_bool("bypass", "Bypass", params.bypass),
        Parameter::new_bool("solo_early", "Solo Early", params.solo_early),
        Parameter::new_bool("solo_late", "Solo Late", params.solo_late),
    ]
}
