//! Multiband Expander plugin parameter definitions — single source of truth for spec arrays.
//!
//! This file owns:
//! - Global parameter specs (GLOBAL_PARAMS)
//! - Per-band template specs (BAND_TEMPLATE)
//! - UI layout (LAYOUT)
//!
//! Multiband Expander has dynamic per-band params, so there is no static
//! `Params` struct or `PluginParamDef` impl here.

use sotf_host::multiband_global_params;
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;

// ============================================================================
// Constants
// ============================================================================

pub const DETECTION_MODES: &[&str] = &["Peak", "RMS"];
pub const HPF_ORDERS: &[&str] = &["2nd", "4th"];

// ============================================================================
// Single-band backward compat (PARAMS + SINGLE_BAND_LAYOUT)
// ============================================================================

/// Single-band expander params (backward compat alias for engines that reference
/// `expander::PARAMS` without crossover entries). Indices match the old standalone
/// `sotf-plugin-expander` crate.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Threshold",
        "threshold",
        -40.0,
        -80.0,
        0.0,
        1.0,
        "dB",
        "Dynamics",
    )
    .doc("Level below which expansion starts"),
    ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Expansion amount (input:output)"),
    ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Timing")
        .doc("Time to reach full expansion"),
    ParamSpec::float(
        "Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Timing",
    )
    .doc("Time to return to unity gain"),
    ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Dynamics")
        .doc("Max attenuation below threshold"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics")
        .doc("Softness of threshold transition"),
    ParamSpec::float(
        "Hysteresis",
        "hysteresis",
        4.0,
        0.0,
        12.0,
        0.1,
        "dB",
        "Dynamics",
    )
    .doc("Open/close threshold difference"),
    ParamSpec::float("Hold", "hold", 10.0, 0.0, 500.0, 1.0, "ms", "Timing")
        .doc("Minimum open time after trigger"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
    ParamSpec::bool_param("Auto Makeup", "auto_makeup", false, "Output")
        .output()
        .doc("Auto-compensate for gain reduction"),
    ParamSpec::bool_labeled(
        "Link Channels",
        "link_channels",
        true,
        "Linked",
        "Unlinked",
        "Channels",
    )
    .setup()
    .doc("Stereo-link detector for L/R"),
    ParamSpec::float(
        "Sidechain HPF",
        "sidechain_hpf_hz",
        80.0,
        0.0,
        500.0,
        5.0,
        "Hz",
        "Sidechain",
    )
    .setup()
    .doc("High-pass on detector input"),
    ParamSpec::float(
        "Lookahead",
        "lookahead_ms",
        0.0,
        0.0,
        20.0,
        0.5,
        "ms",
        "Timing",
    )
    .doc("Pre-delay for transient catching"),
    ParamSpec::choice(
        "Detection Mode",
        "detection_mode",
        0,
        DETECTION_MODES,
        "Sidechain",
    )
    .setup()
    .doc("Peak or RMS level detection"),
    ParamSpec::bool_param(
        "Measured Auto Makeup",
        "measured_auto_makeup",
        false,
        "Output",
    )
    .output()
    .doc("Makeup based on measured reduction"),
];

/// Single-band expander UI layout (backward compat, referencing PARAMS indices).
pub const SINGLE_BAND_LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::toggle(10),   // link_channels
        ControlSpec::knob(11),     // sidechain_hpf_hz
        ControlSpec::selector(13), // detection_mode
    ],
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(0),
                ControlSpec::slider(1),
                ControlSpec::slider(4),
                ControlSpec::slider(5),
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(2),
                ControlSpec::slider(3),
                ControlSpec::slider(7),
                ControlSpec::knob(12), // lookahead_ms
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-30.0, 0.0),
        ControlSpec::toggle(9),
        ControlSpec::toggle(14), // measured_auto_makeup
        ControlSpec::knob(8),
    ],
    tabs: &[TabSpec {
        name: "Advanced",
        controls: &[ControlSpec::knob(6)],
    }],
    visualizations: &[VizSlot::TransferCurve {
        position: VizPosition::BelowGroup("DYNAMICS"),
    }],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Global Parameter Specifications
// ============================================================================

/// Global params for multiband expander.
/// First 6 entries are the shared multiband crossover params (bands, preset, crossover 1-4).
pub const GLOBAL_PARAMS: &[ParamSpec] = multiband_global_params![
    ParamSpec::float(
        "Threshold",
        "threshold",
        -40.0,
        -80.0,
        0.0,
        1.0,
        "dB",
        "Global",
    )
    .doc("Global expansion threshold"),
    ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Global")
        .doc("Global expansion ratio"),
    ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Global")
        .doc("Global attack time"),
    ParamSpec::float(
        "Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Global",
    )
    .doc("Global release time"),
    ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Global")
        .doc("Max attenuation below threshold"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Global")
        .doc("Global knee softness"),
    ParamSpec::float(
        "Hysteresis",
        "hysteresis",
        4.0,
        0.0,
        12.0,
        0.1,
        "dB",
        "Global",
    )
    .doc("Open/close threshold difference"),
    ParamSpec::float("Hold", "hold", 10.0, 0.0, 500.0, 1.0, "ms", "Global")
        .doc("Minimum open time after trigger"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Global")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend"),
    ParamSpec::bool_labeled(
        "Link Channels",
        "link_channels",
        true,
        "Linked",
        "Unlinked",
        "Global",
    )
    .setup()
    .doc("Stereo-link detector for L/R"),
    ParamSpec::choice(
        "Detection Mode",
        "detection_mode",
        0,
        &["Peak", "RMS"],
        "Global",
    )
    .setup()
    .doc("Peak or RMS level detection"),
    ParamSpec::float(
        "Lookahead",
        "lookahead_ms",
        0.0,
        0.0,
        20.0,
        0.5,
        "ms",
        "Timing",
    )
    .doc("Pre-delay for transient catching"),
];

// ============================================================================
// Per-Band Template
// ============================================================================

/// Template for each expander band (repeated per band).
pub const BAND_TEMPLATE: &[ParamSpec] = &[
    ParamSpec::bool_param("Solo", "solo", false, "Band").doc("Solo this band (mute others)"),
    ParamSpec::bool_param("Bypass", "bypass", false, "Band").doc("Bypass expansion for this band"),
    ParamSpec::float(
        "Threshold",
        "threshold",
        -40.0,
        -80.0,
        0.0,
        1.0,
        "dB",
        "Band",
    )
    .doc("Band expansion threshold"),
    ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Band")
        .doc("Band expansion ratio"),
    ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Band").doc("Band attack time"),
    ParamSpec::float("Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Band")
        .doc("Band release time"),
    ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Band")
        .doc("Band max attenuation"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Band").doc("Band knee softness"),
    ParamSpec::float(
        "Hysteresis",
        "hysteresis",
        4.0,
        0.0,
        12.0,
        0.1,
        "dB",
        "Band",
    )
    .doc("Band open/close threshold gap"),
    ParamSpec::float("Hold", "hold", 10.0, 0.0, 500.0, 1.0, "ms", "Band")
        .doc("Band minimum open time"),
    ParamSpec::bool_param("Auto Makeup", "auto_makeup", false, "Band")
        .doc("Auto-compensate band gain"),
    ParamSpec::bool_labeled("Active", "active", true, "Active", "Passive", "Band")
        .doc("Enable band processing"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Multiband Expander: GLOBAL_PARAMS idx 0=bands, 1=preset, 2-5=crossovers,
/// 6=threshold, 7=ratio, 8=attack, 9=release, 10=range, 11=knee,
/// 12=hysteresis, 13=hold, 14=mix, 15=link_channels, 16=detection_mode,
/// 17=lookahead_ms
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0),  // num_bands
        ControlSpec::selector(1),  // crossover_preset
        ControlSpec::toggle(15),   // link_channels
        ControlSpec::selector(16), // detection_mode
    ],
    main: &[
        ControlGroup {
            title: "CROSSOVERS",
            controls: &[
                ControlSpec::knob(2), // crossover_freq_1
                ControlSpec::knob(3), // crossover_freq_2
                ControlSpec::knob(4), // crossover_freq_3
                ControlSpec::knob(5), // crossover_freq_4
            ],
        },
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(6),  // threshold
                ControlSpec::slider(7),  // ratio
                ControlSpec::slider(10), // range
                ControlSpec::slider(11), // knee
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(8),  // attack
                ControlSpec::slider(9),  // release
                ControlSpec::slider(13), // hold
                ControlSpec::slider(12), // hysteresis
                ControlSpec::knob(17),   // lookahead_ms
            ],
        },
    ],
    output: &[
        ControlSpec::knob(14), // mix
    ],
    tabs: &[],
    visualizations: &[VizSlot::Custom {
        name: "band_selector",
        position: VizPosition::FullCenter,
    }],
    column_constraints: &[
        ColumnConstraint::config(120.0, 0.5),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(80.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Public default helpers for the runtime MultibandExpanderPluginParams
// ============================================================================

pub fn default_num_bands() -> usize {
    pk(GLOBAL_PARAMS, "num_bands").default_f64() as usize
}

pub fn default_crossover_preset() -> i32 {
    pk(GLOBAL_PARAMS, "crossover_preset").default_f64() as i32
}

pub fn default_crossover_frequencies() -> Vec<f32> {
    vec![
        pk(GLOBAL_PARAMS, "crossover_freq_1").default_f64() as f32,
        pk(GLOBAL_PARAMS, "crossover_freq_2").default_f64() as f32,
        pk(GLOBAL_PARAMS, "crossover_freq_3").default_f64() as f32,
        pk(GLOBAL_PARAMS, "crossover_freq_4").default_f64() as f32,
    ]
}

pub fn default_threshold_db() -> f32 {
    pk(PARAMS, "threshold").default_f64() as f32
}

pub fn default_ratio() -> f32 {
    pk(PARAMS, "ratio").default_f64() as f32
}

pub fn default_attack_ms() -> f32 {
    pk(PARAMS, "attack").default_f64() as f32
}

pub fn default_release_ms() -> f32 {
    pk(PARAMS, "release").default_f64() as f32
}

pub fn default_knee_db() -> f32 {
    pk(PARAMS, "knee").default_f64() as f32
}

pub fn default_range_db() -> f32 {
    pk(PARAMS, "range").default_f64() as f32
}

pub fn default_hysteresis_db() -> f32 {
    pk(PARAMS, "hysteresis").default_f64() as f32
}

pub fn default_hold_ms() -> f32 {
    pk(PARAMS, "hold").default_f64() as f32
}

pub fn default_link_channels() -> bool {
    pk(PARAMS, "link_channels").default_bool()
}

pub fn default_mix() -> f32 {
    pk(PARAMS, "mix").default_f64() as f32
}

pub fn default_detection_mode() -> String {
    DETECTION_MODES[0].to_string()
}

pub fn default_lookahead_ms() -> f32 {
    pk(PARAMS, "lookahead_ms").default_f64() as f32
}

pub fn default_processing_mode() -> String {
    "time_domain".to_string()
}

pub fn default_auto_makeup() -> Option<bool> {
    Some(pk(PARAMS, "auto_makeup").default_bool())
}

pub fn default_measured_auto_makeup() -> Option<bool> {
    Some(pk(PARAMS, "measured_auto_makeup").default_bool())
}

pub fn default_sidechain_hpf_hz() -> Option<f32> {
    Some(pk(PARAMS, "sidechain_hpf_hz").default_f64() as f32)
}

pub fn default_active() -> bool {
    pk(BAND_TEMPLATE, "active").default_bool()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_params_non_empty() {
        assert!(!GLOBAL_PARAMS.is_empty());
    }

    #[test]
    fn band_template_non_empty() {
        assert!(!BAND_TEMPLATE.is_empty());
    }

    #[test]
    fn global_params_have_unique_engine_keys() {
        for (i, a) in GLOBAL_PARAMS.iter().enumerate() {
            for b in &GLOBAL_PARAMS[i + 1..] {
                assert_ne!(
                    a.engine_key, b.engine_key,
                    "duplicate engine_key '{}' in GLOBAL_PARAMS",
                    a.engine_key
                );
            }
        }
    }

    #[test]
    fn band_template_has_unique_engine_keys() {
        for (i, a) in BAND_TEMPLATE.iter().enumerate() {
            for b in &BAND_TEMPLATE[i + 1..] {
                assert_ne!(
                    a.engine_key, b.engine_key,
                    "duplicate engine_key '{}' in BAND_TEMPLATE",
                    a.engine_key
                );
            }
        }
    }
}
