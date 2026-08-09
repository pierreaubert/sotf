use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

pub const MODE_LABELS: &[&str] = &["Disable", "Bauer", "Meier", "Multiband"];

pub const PRESET_LABELS: &[&str] = &["Default", "Cmoy", "Meier", "Mb", "Off"];

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::choice("Mode", "crossfeed_mode", 3, MODE_LABELS, "General")
        .structural()
        .setup()
        .doc("Crossfeed algorithm selection"),
    ParamSpec::choice("Preset", "crossfeed_preset", 0, PRESET_LABELS, "General")
        .structural()
        .setup()
        .doc("Load preset parameter values"),
    ParamSpec::bool_param("Enabled", "enabled", true, "General")
        .setup()
        .doc("Enable crossfeed processing"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.05, "%", "General")
        .output()
        .doc("Dry/wet blend"),
    // Bauer
    ParamSpec::float(
        "Bauer Cutoff",
        "bauer_fcut_hz",
        700.0,
        400.0,
        1000.0,
        10.0,
        "Hz",
        "Bauer",
    )
    .doc("Bauer shelving filter frequency"),
    ParamSpec::float(
        "Bauer Feed",
        "bauer_feed_db",
        4.5,
        0.0,
        15.0,
        0.5,
        "dB",
        "Bauer",
    )
    .doc("Bauer cross-feed level"),
    // Meier
    ParamSpec::float(
        "Meier Level",
        "meier_level",
        30.0,
        0.0,
        100.0,
        1.0,
        "%",
        "Meier",
    )
    .doc("Meier crossfeed strength"),
    // Multiband
    ParamSpec::float(
        "MB Low Freq",
        "mb_low_freq_hz",
        150.0,
        50.0,
        500.0,
        5.0,
        "Hz",
        "Multiband",
    )
    .doc("Low/mid band split frequency"),
    ParamSpec::float(
        "MB Mid/High Freq",
        "mb_mid_high_freq_hz",
        5700.0,
        2000.0,
        15000.0,
        50.0,
        "Hz",
        "Multiband",
    )
    .doc("Mid/high band split frequency"),
    ParamSpec::float(
        "MB Low Feed",
        "mb_low_feed_db",
        0.0,
        -20.0,
        0.0,
        0.5,
        "dB",
        "Multiband",
    )
    .doc("Low band cross-feed level"),
    ParamSpec::float(
        "MB Mid Feed",
        "mb_mid_feed_db",
        6.0,
        0.0,
        15.0,
        0.5,
        "dB",
        "Multiband",
    )
    .doc("Mid band cross-feed level"),
    ParamSpec::float(
        "MB High Feed",
        "mb_high_feed_db",
        3.0,
        0.0,
        15.0,
        0.5,
        "dB",
        "Multiband",
    )
    .doc("High band cross-feed level"),
    // ITD (Interaural Time Difference)
    ParamSpec::float(
        "ITD Delay",
        "itd_delay_ms",
        0.0,
        0.0,
        1.0,
        0.01,
        "ms",
        "General",
    )
    .doc("Interaural time difference"),
    // Auto Gain
    ParamSpec::bool_param("Auto Gain", "autogain_enabled", false, "Auto Gain")
        .output()
        .doc("Auto-normalize output level"),
    ParamSpec::float(
        "Target LUFS",
        "autogain_target_lufs",
        -18.0,
        -40.0,
        -12.0,
        0.5,
        "LUFS",
        "Auto Gain",
    )
    .output()
    .doc("Target loudness level"),
    ParamSpec::float(
        "Max Gain",
        "autogain_max_gain_db",
        12.0,
        0.0,
        24.0,
        1.0,
        "dB",
        "Auto Gain",
    )
    .output()
    .doc("Maximum auto gain correction"),
    ParamSpec::float(
        "Smoothing",
        "autogain_smoothing_ms",
        100.0,
        10.0,
        5000.0,
        10.0,
        "ms",
        "Auto Gain",
    )
    .output()
    .doc("Auto gain transition time"),
];

/// Crossfeed: idx 0=mode, 1=preset, 2=enabled, 3=mix,
/// 4=bauer_fcut, 5=bauer_feed, 6=meier_level,
/// 7=mb_low_freq, 8=mb_mid_high_freq, 9=mb_low_feed, 10=mb_mid_feed, 11=mb_high_feed,
/// 12=itd_delay_ms,
/// 13=autogain_enabled, 14=target_lufs, 15=max_gain, 16=smoothing
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(1), // crossfeed_preset
    ],
    main: &[
        ControlGroup::new(
            "PRIMARY",
            "PRIMARY",
            &[
                ControlSpec::toggle(2), // enabled
                ControlSpec::knob(3),   // mix
            ],
        ),
        ControlGroup::new(
            "mode-selector",
            "",
            &[
                ControlSpec::button_set(0, MODE_LABELS), // mode
            ],
        ),
        ControlGroup::new(
            "BAUER",
            "BAUER",
            &[
                ControlSpec::knob(4), // bauer_fcut_hz
                ControlSpec::knob(5), // bauer_feed_db
            ],
        )
        .visible_when(ParamCondition::choice(0, 1)),
        ControlGroup::new(
            "meier",
            "MEIER",
            &[ControlSpec::knob(6)], // meier_level
        )
        .visible_when(ParamCondition::choice(0, 2)),
        ControlGroup::new(
            "MULTIBAND",
            "MULTIBAND",
            &[
                ControlSpec::knob(7),  // mb_low_freq_hz
                ControlSpec::knob(8),  // mb_mid_high_freq_hz
                ControlSpec::knob(9),  // mb_low_feed_db
                ControlSpec::knob(10), // mb_mid_feed_db
                ControlSpec::knob(11), // mb_high_feed_db
            ],
        )
        .visible_when(ParamCondition::choice(0, 3)),
        ControlGroup::new(
            "ITD",
            "ITD",
            &[
                ControlSpec::knob(12), // itd_delay_ms
            ],
        ),
    ],
    output: &[
        ControlSpec::knob(14).enabled_when(ParamCondition::bool(13, true)), // target_lufs
        ControlSpec::toggle(13),                                            // autogain_enabled
        ControlSpec::knob(15).enabled_when(ParamCondition::bool(13, true)), // max_gain
        ControlSpec::knob(16).enabled_when(ParamCondition::bool(13, true)), // smoothing
    ],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};
