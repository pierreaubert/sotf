//! EQ plugin parameter definitions — single source of truth for spec arrays.
//!
//! This file owns:
//! - Global parameter specs (GLOBAL_PARAMS)
//! - Per-band template specs (BAND_TEMPLATE)
//!
//! EQ has dynamic per-filter params, so there is no static `Params` struct
//! or `PluginParamDef` impl here.

use math_audio_iir_fir::BiquadFilterType;
use sotf_host::param_specs::ParamSpec;

/// Minimum Q for every filter type.
pub const Q_MIN: f64 = 0.1;

/// UI edit ceiling for every filter type except Notch (knob/drag range).
pub const Q_MAX_STANDARD: f64 = 10.0;

/// Validation/load ceiling for every filter type except Notch. Matches the
/// optimizers' `max_q` ceiling so optimized chains load unclamped.
pub const Q_MAX_OPTIMIZED: f64 = 20.0;

/// Maximum Q for Notch filters (very narrow rejection bands).
pub const Q_MAX_NOTCH: f64 = 40.0;

/// Per-filter-type validation ceiling: notch filters accept much higher Q
/// than all other types. Used by the DSP validation, the settings layer, and
/// preset/config loading.
pub fn q_max_for(filter_type: BiquadFilterType) -> f64 {
    match filter_type {
        BiquadFilterType::Notch => Q_MAX_NOTCH,
        _ => Q_MAX_OPTIMIZED,
    }
}

/// Per-filter-type UI edit ceiling for knobs and drag handles.
pub fn q_max_ui(filter_type: BiquadFilterType) -> f64 {
    match filter_type {
        BiquadFilterType::Notch => Q_MAX_NOTCH,
        _ => Q_MAX_STANDARD,
    }
}

/// Clamp a Q value into the accepted range for the given filter type.
pub fn clamp_q(filter_type: BiquadFilterType, q: f64) -> f64 {
    q.clamp(Q_MIN, q_max_for(filter_type))
}

// ============================================================================
// Global Parameter Specifications
// ============================================================================

/// Global params before the per-filter params.
pub const TOPOLOGIES: &[&str] = &["Biquad", "SVF"];

pub const GLOBAL_PARAMS: &[ParamSpec] = &[
    ParamSpec::int("Max Filters", "max_filters", 20, 1, 20, 1, "", "Global")
        .structural()
        .doc("Maximum number of EQ bands"),
    ParamSpec::bool_param("TDF-II", "tdf2", false, "Algorithm")
        .doc("Use Transposed Direct Form II"),
    // Phase 4C: SOTA addition
    ParamSpec::choice("Topology", "topology", 0, TOPOLOGIES, "Algorithm")
        .structural()
        .doc("Filter topology: Biquad (classic) or SVF (zero-delay feedback, modulation-stable)"),
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", false, "Output")
        .doc("Automatically compensate measured EQ level change"),
    ParamSpec::choice(
        "Oversampling",
        "oversampling",
        0,
        &["Off", "2x", "4x"],
        "Quality",
    )
    .structural()
    .doc("Internal oversampling factor for biquad topology"),
];

// ============================================================================
// Per-Band Template
// ============================================================================

/// Template for each filter band (repeated per filter).
pub const BAND_TEMPLATE: &[ParamSpec] = &[
    ParamSpec::float(
        "Frequency",
        "freq",
        1000.0,
        20.0,
        20000.0,
        10.0,
        "Hz",
        "Filter",
    )
    .doc("Filter center/corner frequency"),
    ParamSpec::float("Q", "q", 1.0, 0.1, 40.0, 0.05, "", "Filter")
        .doc("Filter bandwidth (quality factor); notch filters accept up to 40"),
    ParamSpec::float("Gain", "gain", 0.0, -24.0, 24.0, 0.5, "dB", "Filter")
        .doc("Boost or cut amount"),
    ParamSpec::choice(
        "Type",
        "filter_type",
        0,
        &[
            "Peak",
            "Lowshelf",
            "Highshelf",
            "Lowpass",
            "Highpass",
            "Bandpass",
            "Notch",
            "AllPass",
        ],
        "Filter",
    )
    .doc("Biquad filter shape"),
    ParamSpec::int("Order", "order", 2, 2, 8, 2, "", "Filter")
        .structural()
        .doc("Even filter order: 2, 4, 6, or 8"),
];

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
    fn global_params_include_runtime_and_structural_controls() {
        let keys: Vec<_> = GLOBAL_PARAMS.iter().map(|param| param.engine_key).collect();
        assert_eq!(
            keys,
            vec![
                "max_filters",
                "tdf2",
                "topology",
                "auto_gain_enabled",
                "oversampling",
            ]
        );
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
