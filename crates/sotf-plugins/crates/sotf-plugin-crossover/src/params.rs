//! Crossover plugin parameter definitions.
//!
//! Static ParamSpec metadata for documentation generation and UI descriptors.
//! Runtime parameter construction in `crossover_plugin.rs` is the source of
//! truth for the audio thread; this module mirrors the user-visible surface.

use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

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
];

/// Crossover: idx 0=type, 1=frequency, 2=mode, 3=fir_taps.
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::button_set(0, &["LR24", "LinearPhase"]),
        ControlSpec::button_set(2, &["Lowpass", "Highpass", "Both"]),
    ],
    main: &[ControlGroup {
        title: "CROSSOVER",
        controls: &[ControlSpec::knob_large(1)],
    }],
    output: &[],
    tabs: &[TabSpec {
        name: "Linear Phase",
        controls: &[ControlSpec::knob(3)],
    }],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(170.0, 0.65),
        ColumnConstraint::main(300.0),
    ],
    dynamic_sections: &[],
};
