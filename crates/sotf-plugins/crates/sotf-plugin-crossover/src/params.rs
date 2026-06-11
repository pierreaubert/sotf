//! Crossover plugin parameter definitions.
//!
//! Static ParamSpec metadata for documentation generation and UI descriptors.
//! Runtime parameter construction in `crossover_plugin.rs` is the source of
//! truth for the audio thread; this module mirrors the user-visible surface.

use sotf_host::param_specs::ParamSpec;

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::choice("Type", "type", 0, &["LR24", "LinearPhase"], "General")
        .structural()
        .setup()
        .doc("Crossover filter family: LR24 (Linkwitz-Riley 24 dB/octave) or linear-phase FIR"),
    ParamSpec::float(
        "Frequency",
        "frequency",
        1000.0,
        20.0,
        20000.0,
        1.0,
        "Hz",
        "General",
    )
    .setup()
    .doc("Primary crossover frequency"),
    ParamSpec::choice(
        "Mode",
        "mode",
        0,
        &["Lowpass", "Highpass", "Both"],
        "General",
    )
    .setup()
    .doc("Output mode for the primary crossover"),
    ParamSpec::int("FIR Taps", "fir_taps", 1025, 31, 16385, 2, "", "General")
        .structural()
        .setup()
        .doc("FIR length for linear-phase mode (odd values are rounded up)"),
    ParamSpec::float(
        "Frequency 2",
        "frequency_2",
        1000.0,
        20.0,
        20000.0,
        1.0,
        "Hz",
        "Multi-way",
    )
    .setup()
    .doc("Second crossover frequency for 3-way/4-way operation"),
    ParamSpec::float(
        "Frequency 3",
        "frequency_3",
        1000.0,
        20.0,
        20000.0,
        1.0,
        "Hz",
        "Multi-way",
    )
    .setup()
    .doc("Third crossover frequency for 4-way operation"),
];
