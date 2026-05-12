//! EQ plugin parameter definitions — single source of truth for spec arrays.
//!
//! This file owns:
//! - Global parameter specs (GLOBAL_PARAMS)
//! - Per-band template specs (BAND_TEMPLATE)
//!
//! EQ has dynamic per-filter params, so there is no static `Params` struct
//! or `PluginParamDef` impl here.

use sotf_host::param_specs::ParamSpec;

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
    ParamSpec::float("Q", "q", 1.0, 0.1, 10.0, 0.05, "", "Filter")
        .doc("Filter bandwidth (quality factor)"),
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
