use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

pub const PARAMS: &[ParamSpec] = &[
    // Geometry
    ParamSpec::float(
        "Distance",
        "distance_m",
        2.0,
        0.5,
        10.0,
        0.05,
        "m",
        "Geometry",
    )
    .doc("Listener-to-speaker distance"),
    ParamSpec::float(
        "Speaker Angle",
        "speaker_angle_deg",
        30.0,
        10.0,
        90.0,
        0.5,
        "\u{00b0}",
        "Geometry",
    )
    .doc("Half-angle between speakers"),
    ParamSpec::float(
        "Head Radius",
        "head_radius_m",
        0.0875,
        0.05,
        0.12,
        0.001,
        "m",
        "Geometry",
    )
    .scaled(100.0)
    .doc("Acoustic head radius"),
    // Head Tracking
    ParamSpec::float(
        "Head Offset X",
        "head_offset_x",
        0.0,
        -0.5,
        0.5,
        0.01,
        "m",
        "Head Tracking",
    )
    .doc("Lateral head position offset"),
    ParamSpec::float(
        "Head Offset Z",
        "head_offset_z",
        0.0,
        -0.5,
        0.5,
        0.01,
        "m",
        "Head Tracking",
    )
    .doc("Forward/back head position"),
    ParamSpec::float(
        "Head Yaw",
        "head_yaw_deg",
        0.0,
        -90.0,
        90.0,
        1.0,
        "\u{00b0}",
        "Head Tracking",
    )
    .doc("Head rotation angle"),
    ParamSpec::float(
        "Head Tracking Smooth",
        "head_tracking_smooth_s",
        0.1,
        0.0,
        1.0,
        0.01,
        "s",
        "Head Tracking",
    )
    .doc("Tracking data smoothing time"),
    // Beta
    ParamSpec::float(
        "Beta Base",
        "beta_base",
        0.001,
        0.0001,
        0.1,
        0.001,
        "",
        "Beta",
    )
    .scaled(1000.0)
    .doc("Regularization base level"),
    ParamSpec::float(
        "Beta Low Boost",
        "beta_low_freq_boost",
        10.0,
        0.0,
        30.0,
        0.5,
        "",
        "Beta",
    )
    .doc("Extra regularization at low freq"),
    ParamSpec::float(
        "Beta High Boost",
        "beta_high_freq_boost",
        10.0,
        0.0,
        30.0,
        0.5,
        "",
        "Beta",
    )
    .doc("Extra regularization at high freq"),
    // Shadow
    ParamSpec::float(
        "Shadow Cutoff",
        "head_shadow_cutoff_hz",
        4000.0,
        1000.0,
        10000.0,
        50.0,
        "Hz",
        "Shadow",
    )
    .doc("Head shadow filter onset freq"),
    ParamSpec::float(
        "Shadow Slope",
        "head_shadow_slope_db_per_octave",
        6.0,
        0.0,
        12.0,
        0.5,
        "dB/oct",
        "Shadow",
    )
    .doc("Head shadow attenuation rate"),
    // Filter
    ParamSpec::float(
        "Max Gain",
        "max_gain_db",
        12.0,
        3.0,
        30.0,
        1.0,
        "dB",
        "Filter",
    )
    .doc("Maximum XTC filter boost"),
    // Advanced
    ParamSpec::bool_param("Spectral Norm", "spectral_normalization", true, "Advanced")
        .doc("Normalize filter energy"),
    ParamSpec::bool_param("Pinna Model", "pinna_model_enabled", false, "Advanced")
        .doc("Include pinna diffraction model"),
    // Room
    ParamSpec::bool_param(
        "Room Reflections",
        "room_reflections_enabled",
        false,
        "Room",
    )
    .doc("Include first-order reflections"),
    ParamSpec::file_path("Room IR", "room_ir_file", "Room").doc("Optional measured room impulse response"),
    ParamSpec::float(
        "Room Width",
        "room_width_m",
        4.0,
        2.0,
        10.0,
        0.1,
        "m",
        "Room",
    )
    .doc("Listening room width"),
    ParamSpec::float(
        "Room Depth",
        "room_depth_m",
        5.0,
        2.0,
        15.0,
        0.1,
        "m",
        "Room",
    )
    .doc("Listening room depth"),
    ParamSpec::float(
        "Wall Absorption",
        "wall_absorption",
        0.3,
        0.0,
        1.0,
        0.05,
        "",
        "Room",
    )
    .doc("Wall absorption coefficient"),
    ParamSpec::float(
        "Reflection Beta",
        "reflection_beta_boost",
        3.0,
        1.0,
        10.0,
        0.1,
        "",
        "Room",
    )
    .doc("Reflection path regularization"),
    // Diagnostic
    ParamSpec::bool_param(
        "Bypass XTC Filters",
        "bypass_xtc_filters",
        false,
        "Diagnostic",
    )
    .diagnostic()
    .doc("Skip crosstalk cancellation"),
    ParamSpec::bool_param(
        "Bypass Spectral Norm",
        "bypass_spectral_normalization",
        false,
        "Diagnostic",
    )
    .diagnostic()
    .doc("Skip spectral normalization"),
    ParamSpec::bool_param(
        "Bypass Neumann",
        "bypass_neumann_refinement",
        false,
        "Diagnostic",
    )
    .diagnostic()
    .doc("Skip Neumann KH refinement"),
    // Auto Gain
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", true, "Auto Gain")
        .output()
        .doc("Auto-normalize output level"),
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
    // Phase 4D: SOTA addition
    ParamSpec::choice("Head Model", "head_model", 0, HEAD_MODELS, "Geometry")
        .setup()
        .doc("Head diffraction model: Woodworth (classic) or Brown-Duda (rigid sphere, more accurate above 1.5kHz)"),
];

pub const HEAD_MODELS: &[&str] = &["Woodworth", "Brown-Duda"];

/// XTC: idx 0=distance, 1=speaker_angle, 2=head_radius,
/// 3=head_offset_x, 4=head_offset_z, 5=head_yaw, 6=head_tracking_smooth,
/// 7=beta_base, 8=beta_low_boost, 9=beta_high_boost,
/// 10=shadow_cutoff, 11=shadow_slope, 12=max_gain,
/// 13=spectral_norm, 14=pinna_model,
/// 15=room_reflections, 16=room_ir_file, 17=room_width, 18=room_depth,
/// 19=wall_absorption, 20=reflection_beta,
/// 21=bypass_xtc, 22=bypass_spectral_norm, 23=bypass_neumann,
/// 24=auto_gain, 25=ag_max, 26=ag_smoothing, 27=head_model
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::knob(0),      // distance_m
        ControlSpec::knob(1),      // speaker_angle_deg
        ControlSpec::knob(2),      // head_radius_m
        ControlSpec::selector(27), // head_model
    ],
    main: &[
        ControlGroup::new(
            "BETA",
            "BETA",
            &[
                ControlSpec::knob(7), // beta_base
                ControlSpec::knob(8), // beta_low_boost
                ControlSpec::knob(9), // beta_high_boost
            ],
        ),
        ControlGroup::new(
            "SHADOW",
            "SHADOW",
            &[
                ControlSpec::knob(10), // shadow_cutoff
                ControlSpec::knob(11), // shadow_slope
                ControlSpec::knob(12), // max_gain
            ],
        ),
        ControlGroup::new(
            "ADVANCED",
            "ADVANCED",
            &[
                ControlSpec::toggle(13), // spectral_norm
                ControlSpec::toggle(14), // pinna_model
            ],
        ),
        ControlGroup::new(
            "ROOM",
            "ROOM",
            &[
                ControlSpec::toggle(15), // room_reflections
                ControlSpec::file_picker(16).enabled_when(ParamCondition::bool(15, true)),
                ControlSpec::knob(17).enabled_when(ParamCondition::bool(15, true)),
                ControlSpec::knob(18).enabled_when(ParamCondition::bool(15, true)),
                ControlSpec::knob(19).enabled_when(ParamCondition::bool(15, true)),
                ControlSpec::knob(20).enabled_when(ParamCondition::bool(15, true)),
            ],
        ),
    ],
    output: &[
        ControlSpec::toggle(21), // bypass_xtc (diagnostic)
        ControlSpec::toggle(22), // bypass_spectral_norm (diagnostic)
        ControlSpec::toggle(23), // bypass_neumann (diagnostic)
        ControlSpec::toggle(24), // auto_gain
        ControlSpec::knob(25).enabled_when(ParamCondition::bool(24, true)),
        ControlSpec::knob(26).enabled_when(ParamCondition::bool(24, true)),
    ],
    tabs: &[TabSpec {
        name: "Head Tracking",
        controls: &[
            ControlSpec::knob(3), // head_offset_x
            ControlSpec::knob(4), // head_offset_z
            ControlSpec::knob(5), // head_yaw
            ControlSpec::knob(6), // head_tracking_smooth
        ],
    }],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(120.0, 0.5),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(130.0, 0.6),
    ],
    dynamic_sections: &[],
};
