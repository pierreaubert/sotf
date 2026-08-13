use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Reduction",
        "reduction_db",
        10.0,
        0.0,
        40.0,
        0.5,
        "dB",
        "General",
    )
    .doc("Noise attenuation amount"),
    ParamSpec::float(
        "Floor", "floor_db", -20.0, -60.0, -10.0, 0.5, "dB", "General",
    )
    .doc("Minimum gain floor (artifact limit)"),
    ParamSpec::float(
        "Smoothing",
        "smoothing",
        0.3,
        0.0,
        0.99,
        0.01,
        "",
        "General",
    )
    .scaled(100.0)
    .doc("Gain curve temporal smoothing"),
    ParamSpec::float("Attack", "attack_ms", 5.0, 0.1, 100.0, 0.5, "ms", "Timing")
        .doc("Time to apply reduction"),
    ParamSpec::float(
        "Release",
        "release_ms",
        50.0,
        10.0,
        500.0,
        5.0,
        "ms",
        "Timing",
    )
    .doc("Time to release reduction"),
    ParamSpec::bool_param("Low Latency", "low_latency", false, "General")
        .structural()
        .setup()
        .doc("Smaller FFT for lower latency"),
    ParamSpec::bool_param("Polyphonic", "polyphonic_detection", false, "Analysis")
        .secondary("Analysis")
        .doc("Detect multiple pitched signals"),
    ParamSpec::float(
        "MCRA Alpha S",
        "mcra_alpha_s",
        0.9,
        0.5,
        0.99,
        0.01,
        "",
        "Advanced",
    )
    .secondary("MCRA")
    .doc("Noise spectrum smoothing factor"),
    ParamSpec::float(
        "MCRA Alpha P",
        "mcra_alpha_p",
        0.7,
        0.1,
        0.99,
        0.01,
        "",
        "Advanced",
    )
    .secondary("MCRA")
    .doc("Speech presence probability smooth"),
    ParamSpec::int("MCRA Window", "mcra_l", 50, 10, 200, 1, "fr", "Advanced")
        .secondary("MCRA")
        .doc("Min statistics window length"),
    ParamSpec::float(
        "MCRA Delta",
        "mcra_delta",
        5.0,
        1.0,
        20.0,
        0.5,
        "",
        "Advanced",
    )
    .secondary("MCRA")
    .doc("Speech/noise discrimination bias"),
    ParamSpec::float(
        "Transparency",
        "transparency",
        0.0,
        0.0,
        1.0,
        0.01,
        "",
        "General",
    )
    .scaled(100.0)
    .doc("Blend denoised toward dry signal"),
    ParamSpec::bool_param("DD SNR", "dd_enabled", true, "Analysis")
        .secondary("Analysis")
        .doc("Decision-Directed SNR estimator"),
    ParamSpec::float(
        "DD Alpha", "dd_alpha", 0.98, 0.5, 0.999, 0.001, "", "Analysis",
    )
    .secondary("Analysis")
    .doc("DD SNR smoothing coefficient"),
    ParamSpec::bool_param("Psychoacoustic", "psychoacoustic_masking", true, "Analysis")
        .secondary("Analysis")
        .doc("Use auditory masking curves"),
    ParamSpec::bool_param(
        "Spectral Smooth",
        "spectral_smoothing_enabled",
        true,
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Smooth gain across frequency bins"),
    ParamSpec::bool_param(
        "Temporal Smooth",
        "temporal_smoothing_enabled",
        true,
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Smooth gain across time frames"),
    ParamSpec::bool_param(
        "Spectral Sub",
        "spectral_sub_enabled",
        false,
        "Spectral Sub",
    )
    .secondary("Spectral Sub")
    .doc("Enable spectral subtraction"),
    ParamSpec::float(
        "Oversub Factor",
        "spectral_sub_alpha",
        2.0,
        0.5,
        6.0,
        0.1,
        "",
        "Spectral Sub",
    )
    .secondary("Spectral Sub")
    .doc("Over-subtraction factor (alpha)"),
    ParamSpec::float(
        "Spectral Floor",
        "spectral_sub_beta",
        0.02,
        0.001,
        0.5,
        0.001,
        "",
        "Spectral Sub",
    )
    .secondary("Spectral Sub")
    .doc("Spectral floor factor (beta)"),
    ParamSpec::bool_labeled(
        "Learn Noise",
        "learn_noise",
        false,
        "Active",
        "Off",
        "Noise Profile",
    )
    .structural()
    .secondary("Noise Profile")
    .doc("Capture noise-only reference"),
    ParamSpec::bool_param(
        "Use Profile",
        "use_captured_profile",
        false,
        "Noise Profile",
    )
    .secondary("Noise Profile")
    .doc("Use captured noise profile"),
    ParamSpec::bool_labeled(
        "Clear Profile",
        "clear_profile",
        false,
        "Trigger",
        "Off",
        "Noise Profile",
    )
    .structural()
    .secondary("Noise Profile")
    .doc("Discard captured noise profile"),
    ParamSpec::bool_param("Formant Preserve", "formant_preservation", false, "Formant")
        .secondary("Formant")
        .doc("Protect vocal formant structure"),
    ParamSpec::float(
        "Formant Strength",
        "formant_strength",
        0.5,
        0.0,
        1.0,
        0.01,
        "",
        "Formant",
    )
    .scaled(100.0)
    .secondary("Formant")
    .doc("Formant preservation amount"),
    ParamSpec::bool_param("Multi-Res", "multi_resolution", false, "General")
        .structural()
        .setup()
        .secondary("General")
        .doc("Multi-resolution FFT analysis"),
    ParamSpec::bool_param(
        "Harmonic/Percussive",
        "harmonic_percussive",
        false,
        "Advanced",
    )
    .doc("Separate tonal and transient components for differential denoising"),
    ParamSpec::bool_param("Spatial Denoise", "spatial_denoise", false, "Advanced")
        .doc("Use inter-channel coherence for noise detection (stereo+ only)"),
    ParamSpec::float(
        "Spatial Strength",
        "spatial_strength",
        0.5,
        0.0,
        1.0,
        0.1,
        "",
        "Advanced",
    )
    .doc("Weight of inter-channel coherence in noise estimation"),
];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::toggle(5)],
    main: &[
        ControlGroup::new(
            "REDUCTION",
            "REDUCTION",
            &[
                ControlSpec::slider(0),
                ControlSpec::slider(1),
                ControlSpec::slider(2),
                ControlSpec::slider(11),
            ],
        ),
        ControlGroup::new(
            "TIMING",
            "TIMING",
            &[ControlSpec::knob(3), ControlSpec::knob(4)],
        ),
        ControlGroup::new(
            "SPECTRAL SUB",
            "SPECTRAL SUB",
            &[
                ControlSpec::toggle(17),
                ControlSpec::knob(18),
                ControlSpec::knob(19),
            ],
        ),
        ControlGroup::new(
            "NOISE PROFILE",
            "NOISE PROFILE",
            &[
                ControlSpec::toggle(20),
                ControlSpec::toggle(21),
                ControlSpec::toggle(22),
            ],
        ),
    ],
    output: &[],
    tabs: &[
        TabSpec {
            name: "Analysis",
            controls: &[
                ControlSpec::toggle(6),
                ControlSpec::toggle(12),
                ControlSpec::knob(13),
                ControlSpec::toggle(14),
                ControlSpec::toggle(15),
                ControlSpec::toggle(16),
            ],
        },
        TabSpec {
            name: "MCRA",
            controls: &[
                ControlSpec::knob(7),
                ControlSpec::knob(8),
                ControlSpec::knob(9),
                ControlSpec::knob(10),
            ],
        },
        TabSpec {
            name: "Formant",
            controls: &[
                ControlSpec::toggle(23),
                ControlSpec::knob(24).enabled_when(ParamCondition::bool(23, true)),
            ],
        },
        TabSpec {
            name: "Advanced",
            controls: &[
                ControlSpec::toggle(25),
                ControlSpec::toggle(26),
                ControlSpec::toggle(27),
                ControlSpec::knob(28).enabled_when(ParamCondition::bool(27, true)),
            ],
        },
    ],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
    ],
    dynamic_sections: &[],
};
