//! Resampler plugin parameter definitions.
//!
//! Static ParamSpec metadata for documentation generation and UI descriptors.
//! Runtime parameter construction in `resampler_plugin.rs` is the source of
//! truth for the audio thread; this module mirrors the user-visible surface.

use sotf_host::param_specs::ParamSpec;

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::choice(
        "Quality",
        "quality",
        1,
        &["Fast", "Medium", "High"],
        "Quality",
    )
    .structural()
    .setup()
    .doc("Resampling quality: fast (64-tap), medium (128-tap), high (256-tap) sinc filter"),
    ParamSpec::bool_param("Dynamic Ratio", "dynamic_ratio", false, "Ratio")
        .setup()
        .doc("Enable runtime ratio changes without rebuilding the resampler"),
    ParamSpec::float("Ratio", "ratio", 1.0, 0.25, 4.0, 0.01, "", "Ratio")
        .doc("Current resampling ratio (only adjustable when Dynamic Ratio is enabled)"),
];
