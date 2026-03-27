//! Single-band compressor parameter definitions.
//!
//! Mirrors the backward-compat PARAMS from sotf-plugin-multiband-compressor/src/params.rs.

use sotf_host::param_specs::ParamSpec;

pub const DETECTION_MODES: &[&str] = &["Peak", "RMS"];
pub const HPF_ORDERS: &[&str] = &["2nd", "4th"];

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float("Threshold", "threshold", -20.0, -60.0, 0.0, 1.0, "dB", "Dynamics")
        .doc("Level above which compression starts"),
    ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Compression amount (input:output)"),
    ParamSpec::float("Attack", "attack", 5.0, 0.1, 100.0, 0.5, "ms", "Timing")
        .doc("Time to reach full compression"),
    ParamSpec::float("Release", "release", 50.0, 10.0, 1000.0, 5.0, "ms", "Timing")
        .doc("Time to return to unity gain"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics")
        .doc("Softness of threshold transition"),
    ParamSpec::float("Makeup Gain", "makeup_gain", 0.0, -24.0, 24.0, 0.5, "dB", "Output")
        .output()
        .doc("Post-compression gain boost"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet blend (parallel comp)"),
    ParamSpec::bool_param("Auto Makeup", "auto_makeup", false, "Output")
        .output()
        .doc("Auto-compensate for gain reduction"),
    ParamSpec::bool_labeled("Link Channels", "link_channels", true, "Linked", "Unlinked", "Channels")
        .setup()
        .doc("Stereo-link detector for L/R"),
    ParamSpec::float("Sidechain HPF", "sidechain_hpf_hz", 80.0, 0.0, 200.0, 5.0, "Hz", "Sidechain")
        .setup()
        .doc("High-pass on detector input"),
    ParamSpec::choice("Sidechain HPF Order", "sidechain_hpf_order", 0, HPF_ORDERS, "Sidechain")
        .setup()
        .doc("Butterworth HPF slope"),
    ParamSpec::choice("Detection Mode", "detection_mode", 0, DETECTION_MODES, "Sidechain")
        .setup()
        .doc("Peak or RMS level detection"),
    ParamSpec::float("Lookahead", "lookahead_ms", 0.0, 0.0, 20.0, 0.5, "ms", "Timing")
        .doc("Pre-delay for transient catching"),
    ParamSpec::bool_param("Program Dependent Release", "program_dependent_release", false, "Timing")
        .doc("Adapts release to signal content"),
    ParamSpec::bool_param("Measured Auto Makeup", "measured_auto_makeup", false, "Output")
        .output()
        .doc("Makeup based on measured reduction"),
    ParamSpec::bool_param("External Sidechain", "sidechain_external", false, "Sidechain")
        .setup()
        .doc("Use external signal for detection"),
];
