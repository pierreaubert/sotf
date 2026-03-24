//! Multiband Compressor plugin parameter definitions — single source of truth for spec arrays.
//!
//! This file owns:
//! - Global parameter specs (GLOBAL_PARAMS)
//! - Per-band template specs (BAND_TEMPLATE)
//! - UI layout (LAYOUT)
//!
//! Multiband Compressor has dynamic per-band params, so there is no static
//! `Params` struct or `PluginParamDef` impl here.

use sotf_host::multiband_global_params;
use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

// ============================================================================
// Global Parameter Specifications
// ============================================================================

/// Global params for multiband compressor.
/// First 6 entries are the shared multiband crossover params (bands, preset, crossover 1-4).
pub const GLOBAL_PARAMS: &[ParamSpec] = multiband_global_params![
    ParamSpec::float(
        "Threshold",
        "threshold",
        -20.0,
        -60.0,
        0.0,
        1.0,
        "dB",
        "Global",
    )
    .doc("Global compression threshold"),
    ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Global")
        .doc("Global compression ratio"),
    ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.5, "ms", "Global")
        .doc("Global attack time"),
    ParamSpec::float(
        "Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Global",
    )
    .doc("Global release time"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Global")
        .doc("Global knee softness"),
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
    ParamSpec::float(
        "Lookahead",
        "per_band_lookahead_ms",
        0.0,
        0.0,
        20.0,
        0.5,
        "ms",
        "Global",
    )
    .doc("Per-band pre-delay"),
    ParamSpec::bool_param("M/S Mode", "ms_mode", false, "Global")
        .setup()
        .doc("Mid/Side processing mode"),
];

// ============================================================================
// Per-Band Template
// ============================================================================

/// Template for each compressor band (repeated per band).
pub const BAND_TEMPLATE: &[ParamSpec] = &[
    ParamSpec::bool_param("Solo", "solo", false, "Band")
        .doc("Solo this band (mute others)"),
    ParamSpec::bool_param("Bypass", "bypass", false, "Band")
        .doc("Bypass compression for this band"),
    ParamSpec::float(
        "Threshold",
        "threshold",
        -20.0,
        -60.0,
        0.0,
        1.0,
        "dB",
        "Band",
    )
    .doc("Band compression threshold"),
    ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Band")
        .doc("Band compression ratio"),
    ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.5, "ms", "Band")
        .doc("Band attack time"),
    ParamSpec::float("Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Band")
        .doc("Band release time"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Band")
        .doc("Band knee softness"),
    ParamSpec::float(
        "Makeup Gain",
        "makeup_gain",
        0.0,
        -24.0,
        24.0,
        0.5,
        "dB",
        "Band",
    )
    .doc("Band post-compression boost"),
    ParamSpec::bool_param("Auto Makeup", "auto_makeup", false, "Band")
        .doc("Auto-compensate band gain"),
    ParamSpec::bool_labeled("Active", "active", true, "Active", "Passive", "Band")
        .doc("Enable band processing"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Multiband Compressor: GLOBAL_PARAMS 0-12, BAND_TEMPLATE 0-9 per band.
/// Global: 0=bands, 1=preset, 2-5=crossovers, 6=threshold, 7=ratio,
/// 8=attack, 9=release, 10=knee, 11=mix, 12=link_channels
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::knob(0),     // num_bands
        ControlSpec::selector(1), // crossover_preset
        ControlSpec::knob(2),     // crossover_freq_1
        ControlSpec::knob(3),     // crossover_freq_2
        ControlSpec::knob(4),     // crossover_freq_3
        ControlSpec::knob(5),     // crossover_freq_4
        ControlSpec::toggle(12),  // link_channels
    ],
    main: &[
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(6),  // threshold
                ControlSpec::slider(7),  // ratio
                ControlSpec::slider(10), // knee
            ],
        },
        ControlGroup {
            title: "TIMING",
            controls: &[
                ControlSpec::slider(8), // attack
                ControlSpec::slider(9), // release
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-30.0, 0.0), // GR meter
        ControlSpec::knob(11),          // mix
    ],
    tabs: &[],
    visualizations: &[VizSlot::Custom {
        name: "band_selector",
        position: VizPosition::FullCenter,
    }],
    column_constraints: &[
        ColumnConstraint::config(140.0, 0.4),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(120.0, 0.6),
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
