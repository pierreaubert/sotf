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
use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

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
];

// ============================================================================
// Per-Band Template
// ============================================================================

/// Template for each expander band (repeated per band).
pub const BAND_TEMPLATE: &[ParamSpec] = &[
    ParamSpec::bool_param("Solo", "solo", false, "Band")
        .doc("Solo this band (mute others)"),
    ParamSpec::bool_param("Bypass", "bypass", false, "Band")
        .doc("Bypass expansion for this band"),
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
    ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Band")
        .doc("Band attack time"),
    ParamSpec::float("Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Band")
        .doc("Band release time"),
    ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Band")
        .doc("Band max attenuation"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Band")
        .doc("Band knee softness"),
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
/// 12=hysteresis, 13=hold, 14=mix, 15=link_channels
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // num_bands
        ControlSpec::selector(1), // crossover_preset
        ControlSpec::toggle(15),  // link_channels
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
        ColumnConstraint::main(400.0),
        ColumnConstraint::output(80.0, 0.6),
    ],
};

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
