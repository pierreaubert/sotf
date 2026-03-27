//! Single-band expander parameter definitions.
//!
//! Mirrors the backward-compat PARAMS from sotf-plugin-multiband-expander/src/params.rs.

use sotf_host::param_specs::ParamSpec;

pub const DETECTION_MODES: &[&str] = &["Peak", "RMS"];
pub const HPF_ORDERS: &[&str] = &["2nd", "4th"];

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float("Threshold", "threshold", -40.0, -80.0, 0.0, 1.0, "dB", "Dynamics")
        .doc("Level below which expansion starts"),
    ParamSpec::float("Ratio", "ratio", 2.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Expansion amount (input:output)"),
    ParamSpec::float("Attack", "attack", 1.0, 0.1, 50.0, 0.1, "ms", "Timing")
        .doc("Time to reach full expansion"),
    ParamSpec::float("Release", "release", 100.0, 10.0, 2000.0, 5.0, "ms", "Timing")
        .doc("Time to return to unity gain"),
    ParamSpec::float("Range", "range", 40.0, 0.0, 80.0, 1.0, "dB", "Dynamics")
        .doc("Max attenuation below threshold"),
    ParamSpec::float("Knee", "knee", 6.0, 0.0, 20.0, 0.5, "dB", "Dynamics")
        .doc("Softness of threshold transition"),
    ParamSpec::float("Hysteresis", "hysteresis", 4.0, 0.0, 12.0, 0.1, "dB", "Dynamics")
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
    ParamSpec::bool_labeled("Link Channels", "link_channels", true, "Linked", "Unlinked", "Channels")
        .setup()
        .doc("Stereo-link detector for L/R"),
    ParamSpec::float("Sidechain HPF", "sidechain_hpf_hz", 80.0, 0.0, 500.0, 5.0, "Hz", "Sidechain")
        .setup()
        .doc("High-pass on detector input"),
    ParamSpec::float("Lookahead", "lookahead_ms", 0.0, 0.0, 20.0, 0.5, "ms", "Timing")
        .doc("Pre-delay for transient catching"),
    ParamSpec::choice("Detection Mode", "detection_mode", 0, DETECTION_MODES, "Sidechain")
        .setup()
        .doc("Peak or RMS level detection"),
    ParamSpec::bool_param("Measured Auto Makeup", "measured_auto_makeup", false, "Output")
        .output()
        .doc("Makeup based on measured reduction"),
];
