use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

pub const SPEAKER_CONFIGS: &[&str] = &[
    "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
];

pub const ROOM_PRESETS: &[&str] = &["small", "medium", "large", "cathedral"];

pub const PARAMS: &[ParamSpec] = &[
    // 0: speaker_config
    ParamSpec::choice(
        "Speaker Config",
        "speaker_config",
        1,
        SPEAKER_CONFIGS,
        "Spatial",
    )
    .structural()
    .setup()
    .doc("Output speaker layout"),
    // 1: room_size
    ParamSpec::float("Room Size", "room_size", 1.0, 0.2, 3.0, 0.1, "x", "Room")
        .doc("Scales all delay line lengths"),
    // 2: rt60
    ParamSpec::float("RT60", "rt60", 1.8, 0.3, 6.0, 0.1, "s", "Room")
        .doc("Mid-frequency reverberation time"),
    // 3: bass_ratio
    ParamSpec::float("Bass Ratio", "bass_ratio", 1.2, 0.8, 2.0, 0.05, "x", "Room")
        .doc("RT60_bass / RT60_mid ratio"),
    // 4: treble_ratio
    ParamSpec::float(
        "Treble Ratio",
        "treble_ratio",
        0.5,
        0.2,
        1.0,
        0.05,
        "x",
        "Room",
    )
    .doc("RT60_treble / RT60_mid ratio"),
    // 5: pre_delay_ms
    ParamSpec::float(
        "Pre-delay",
        "pre_delay_ms",
        20.0,
        0.0,
        100.0,
        1.0,
        "ms",
        "Room",
    )
    .doc("Gap before first reflection"),
    // 6: room_preset
    ParamSpec::choice("Room Preset", "room_preset", 1, ROOM_PRESETS, "Room")
        .setup()
        .doc("Early reflection tap configuration"),
    // 7: dry_level
    ParamSpec::float("Dry Level", "dry_level", 0.5, 0.0, 1.0, 0.01, "x", "Levels")
        .output()
        .doc("Direct dry output level"),
    // 8: er_level
    ParamSpec::float("ER Level", "er_level", 0.3, 0.0, 1.0, 0.01, "x", "Levels")
        .doc("Early reflection level"),
    // 9: late_level
    ParamSpec::float(
        "Late Level",
        "late_level",
        0.2,
        0.0,
        1.0,
        0.01,
        "x",
        "Levels",
    )
    .doc("Late reverb (FDN) level"),
    // 10: lfe_level
    ParamSpec::float("LFE Level", "lfe_level", 0.2, 0.0, 1.0, 0.01, "x", "Levels")
        .doc("Bass sent to LFE channel"),
    // 11: mod_depth
    ParamSpec::float(
        "Mod Depth",
        "mod_depth",
        0.5,
        0.0,
        1.0,
        0.01,
        "x",
        "Modulation",
    )
    .doc("FDN time-variant delay modulation (Griesinger)"),
    // 12: er_mod_depth
    ParamSpec::float(
        "ER Mod Depth",
        "er_mod_depth",
        0.3,
        0.0,
        1.0,
        0.01,
        "x",
        "Modulation",
    )
    .doc("Early reflection tap modulation"),
    // 13: input_diffusion
    ParamSpec::float(
        "Input Diffusion",
        "input_diffusion",
        0.7,
        0.0,
        1.0,
        0.01,
        "x",
        "Character",
    )
    .doc("Pre-FDN allpass diffusion"),
    // 14: envelopment
    ParamSpec::float(
        "Envelopment",
        "envelopment",
        0.7,
        0.0,
        1.0,
        0.01,
        "x",
        "Spatial",
    )
    .doc("Rear/surround vs front reverb balance"),
    // 15: height_amount
    ParamSpec::float(
        "Height Amount",
        "height_amount",
        0.5,
        0.0,
        1.0,
        0.01,
        "x",
        "Spatial",
    )
    .doc("Height channel contribution"),
    // 16: content_aware
    ParamSpec::bool_param("Content Aware", "content_aware", true, "Intelligence")
        .doc("Enable speech detection for reverb ducking"),
    // 17: dialogue_attenuation_db
    ParamSpec::float(
        "Dialogue Atten.",
        "dialogue_attenuation_db",
        6.0,
        0.0,
        12.0,
        0.5,
        "dB",
        "Intelligence",
    )
    .doc("Reverb reduction during detected speech"),
    // 18: safety_limit_db
    ParamSpec::float(
        "Safety Limit",
        "safety_limit_db",
        6.0,
        0.0,
        12.0,
        0.5,
        "dB",
        "Intelligence",
    )
    .output()
    .doc("FDN feedback limiter threshold"),
    // 19: auto_gain_enabled
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", false, "Auto Gain")
        .output()
        .doc("Match rendered output loudness to the stereo input"),
    // 20: auto_gain_max_db
    ParamSpec::float(
        "AG Max",
        "auto_gain_max_db",
        12.0,
        0.0,
        24.0,
        1.0,
        "dB",
        "Auto Gain",
    )
    .output()
    .doc("Maximum auto gain correction"),
    // 21: auto_gain_smoothing_ms
    ParamSpec::float(
        "AG Smoothing",
        "auto_gain_smoothing_ms",
        100.0,
        10.0,
        500.0,
        5.0,
        "ms",
        "Auto Gain",
    )
    .output()
    .doc("Auto gain transition time"),
    // 22: bypass
    ParamSpec::bool_param("Bypass", "bypass", false, "Diagnostic").doc("Pass-through mode"),
    // 23: solo_early
    ParamSpec::bool_param("Solo Early", "solo_early", false, "Diagnostic")
        .doc("Hear only early reflections"),
    // 24: solo_late
    ParamSpec::bool_param("Solo Late", "solo_late", false, "Diagnostic")
        .doc("Hear only late reverb"),
];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // speaker_config
        ControlSpec::selector(6), // room_preset
    ],
    main: &[
        ControlGroup {
            title: "ROOM",
            controls: &[
                ControlSpec::slider(1), // room_size
                ControlSpec::slider(2), // rt60
                ControlSpec::slider(3), // bass_ratio
                ControlSpec::slider(4), // treble_ratio
                ControlSpec::slider(5), // pre_delay_ms
            ],
        },
        ControlGroup {
            title: "LEVELS",
            controls: &[
                ControlSpec::slider(8),  // er_level
                ControlSpec::slider(9),  // late_level
                ControlSpec::slider(10), // lfe_level
            ],
        },
    ],
    output: &[
        ControlSpec::slider(7),  // dry_level
        ControlSpec::knob(18),   // safety_limit_db
        ControlSpec::toggle(19), // auto_gain_enabled
        ControlSpec::knob(20),   // auto_gain_max_db
        ControlSpec::knob(21),   // auto_gain_smoothing_ms
    ],
    tabs: &[
        TabSpec {
            name: "Spatial",
            controls: &[
                ControlSpec::knob(14), // envelopment
                ControlSpec::knob(15), // height_amount
            ],
        },
        TabSpec {
            name: "Modulation",
            controls: &[
                ControlSpec::knob(11), // mod_depth
                ControlSpec::knob(12), // er_mod_depth
                ControlSpec::knob(13), // input_diffusion
            ],
        },
        TabSpec {
            name: "Intelligence",
            controls: &[
                ControlSpec::toggle(16), // content_aware
                ControlSpec::knob(17),   // dialogue_attenuation_db
            ],
        },
        TabSpec {
            name: "Diagnostics",
            controls: &[
                ControlSpec::toggle(22), // bypass
                ControlSpec::toggle(23), // solo_early
                ControlSpec::toggle(24), // solo_late
            ],
        },
    ],
    // Spatial spider (SPL / inter-channel correlation) — opt-in via the
    // generic ui_layout_renderer "spatial_spider" custom-viz hook. Plugins
    // that produce a multichannel output benefit from this; it lets the
    // user see per-channel level / phase relationships at a glance.
    visualizations: &[VizSlot::Custom {
        name: viz_names::SPATIAL_SPIDER,
        position: VizPosition::FullCenter,
    }],
    column_constraints: &[
        ColumnConstraint::config(180.0, 0.55),
        ColumnConstraint::main(500.0),
        ColumnConstraint::output(220.0, 0.65),
    ],
    dynamic_sections: &[],
};
