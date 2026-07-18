use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

pub const SPEAKER_CONFIGS: &[&str] = &[
    "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
];

pub const DECORRELATION_MODES: &[&str] = &["Velvet Noise", "LFO Phase"];

pub const FREQUENCY_RESOLUTIONS: &[&str] = &["ERB", "Fine ERB", "Per Bin"];

pub const PARAMS: &[ParamSpec] = &[
    // 0: speaker_config
    ParamSpec::choice(
        "Speaker Config",
        "speaker_config",
        2,
        SPEAKER_CONFIGS,
        "Output",
    )
    .structural()
    .setup()
    .doc("Target surround speaker layout"),
    // Gains
    // 1: gain_front_direct
    ParamSpec::float(
        "Front Direct",
        "gain_front_direct",
        1.0,
        0.0,
        2.0,
        0.05,
        "x",
        "Gains",
    )
    .doc("Direct sound to front speakers"),
    // 2: gain_front_ambient
    ParamSpec::float(
        "Front Ambient",
        "gain_front_ambient",
        0.5,
        0.0,
        2.0,
        0.05,
        "x",
        "Gains",
    )
    .doc("Ambient sound to front speakers"),
    // 3: gain_rear_ambient
    ParamSpec::float(
        "Rear Ambient",
        "gain_rear_ambient",
        1.0,
        0.0,
        2.0,
        0.05,
        "x",
        "Gains",
    )
    .doc("Ambient sound to rear speakers"),
    // 4: height_gain
    ParamSpec::float(
        "Height Gain",
        "height_gain",
        1.0,
        0.0,
        2.0,
        0.05,
        "x",
        "Gains",
    )
    .doc("Level sent to height speakers"),
    // LFE
    // 5: lfe_gain
    ParamSpec::float("LFE Gain", "lfe_gain", 1.0, 0.0, 2.0, 0.05, "x", "LFE")
        .secondary("LFE & Bass")
        .doc("Subwoofer channel level"),
    // 6: lfe_cutoff_hz
    ParamSpec::float(
        "LFE Cutoff",
        "lfe_cutoff_hz",
        120.0,
        20.0,
        180.0,
        5.0,
        "Hz",
        "LFE",
    )
    .secondary("LFE & Bass")
    .doc("LFE low-pass filter frequency"),
    // 7: enable_subharmonic_synth
    ParamSpec::bool_param(
        "Subharmonic Synth",
        "enable_subharmonic_synth",
        false,
        "LFE",
    )
    .secondary("LFE & Bass")
    .doc("Generate sub-bass harmonics"),
    // 8: subharmonic_gain
    ParamSpec::float(
        "Sub Gain",
        "subharmonic_gain",
        0.5,
        0.0,
        1.0,
        0.05,
        "x",
        "LFE",
    )
    .secondary("LFE & Bass")
    .doc("Synthesized sub-bass level"),
    // 9: subharmonic_freq_hz
    ParamSpec::float(
        "Sub Freq",
        "subharmonic_freq_hz",
        40.0,
        20.0,
        80.0,
        1.0,
        "Hz",
        "LFE",
    )
    .secondary("LFE & Bass")
    .doc("Sub-harmonic target frequency"),
    // 10: subharmonic_attack_ms
    ParamSpec::float(
        "Sub Attack",
        "subharmonic_attack_ms",
        10.0,
        1.0,
        100.0,
        1.0,
        "ms",
        "LFE",
    )
    .secondary("LFE & Bass")
    .doc("Sub-harmonic envelope attack"),
    // 11: subharmonic_release_ms
    ParamSpec::float(
        "Sub Release",
        "subharmonic_release_ms",
        50.0,
        10.0,
        500.0,
        5.0,
        "ms",
        "LFE",
    )
    .secondary("LFE & Bass")
    .doc("Sub-harmonic envelope release"),
    // Spatial
    // 12: stereo_width
    ParamSpec::float(
        "Stereo Width",
        "stereo_width",
        0.5,
        0.0,
        1.0,
        0.05,
        "",
        "Spatial",
    )
    .doc("Front L/R separation amount"),
    // 13: center_spread
    ParamSpec::float(
        "Center Spread",
        "center_spread",
        0.0,
        0.0,
        1.0,
        0.05,
        "",
        "Spatial",
    )
    .doc("Center image width to L/R"),
    // 14: bandpass_hz
    ParamSpec::float(
        "Upmix Crossover",
        "bandpass_hz",
        250.0,
        150.0,
        350.0,
        5.0,
        "Hz",
        "Spatial",
    )
    .doc("Direct/ambient split frequency"),
    // HR Direct
    // 15: enable_hr_direct
    ParamSpec::bool_param("HR Direct", "enable_hr_direct", true, "HR Direct")
        .secondary("HR Direct")
        .doc("Enable high-resolution direct path"),
    // 16: hr_sharpen
    ParamSpec::float(
        "HR Sharpen",
        "hr_sharpen",
        1.0,
        0.0,
        1.0,
        0.05,
        "",
        "HR Direct",
    )
    .secondary("HR Direct")
    .doc("Spatial image sharpening"),
    // 17: ambient_boost
    ParamSpec::float(
        "Ambient Boost",
        "ambient_boost",
        1.2,
        0.5,
        2.0,
        0.05,
        "x",
        "HR Direct",
    )
    .secondary("HR Direct")
    .doc("Incoherent signal boost factor"),
    // Decorrelation
    // 18: decorrelation_mode
    ParamSpec::choice(
        "Decor Mode",
        "decorrelation_mode",
        0,
        DECORRELATION_MODES,
        "Decorrelation",
    )
    .secondary("Decorrelation")
    .structural()
    .setup()
    .doc("Channel decorrelation method"),
    // 19: decorrelation_lfo_rate_hz
    ParamSpec::float(
        "Decor LFO Rate",
        "decorrelation_lfo_rate_hz",
        0.15,
        0.01,
        1.0,
        0.01,
        "Hz",
        "Decorrelation",
    )
    .secondary("Decorrelation")
    .doc("LFO decorrelation modulation rate"),
    // 20: velvet_noise_duration_ms
    ParamSpec::float(
        "Velvet Duration",
        "velvet_noise_duration_ms",
        30.0,
        10.0,
        100.0,
        1.0,
        "ms",
        "Decorrelation",
    )
    .secondary("Decorrelation")
    .structural()
    .setup()
    .doc("Velvet noise impulse length"),
    // 21: velvet_noise_density
    ParamSpec::float(
        "Velvet Density",
        "velvet_noise_density",
        2000.0,
        500.0,
        5000.0,
        100.0,
        "",
        "Decorrelation",
    )
    .secondary("Decorrelation")
    .structural()
    .setup()
    .doc("Velvet noise pulses per second"),
    // Height
    // 22: height_hf_cap_hz
    ParamSpec::float(
        "Height HF Cap",
        "height_hf_cap_hz",
        16000.0,
        8000.0,
        20000.0,
        100.0,
        "Hz",
        "Height",
    )
    .secondary("Height")
    .doc("High-frequency limit for heights"),
    // 23: height_transient_reduction
    ParamSpec::float(
        "Height Trans Red",
        "height_transient_reduction",
        0.6,
        0.0,
        1.0,
        0.05,
        "",
        "Height",
    )
    .secondary("Height")
    .doc("Soften transients in height feed"),
    // 24: height_direct_leak
    ParamSpec::float(
        "Height Direct Leak",
        "height_direct_leak",
        0.15,
        0.0,
        0.5,
        0.01,
        "",
        "Height",
    )
    .secondary("Height")
    .doc("Direct signal bleed into heights"),
    // Surround
    // 25: surround_direct_bleed
    ParamSpec::float(
        "Surround Bleed",
        "surround_direct_bleed",
        0.5,
        0.0,
        1.0,
        0.05,
        "",
        "Surround",
    )
    .secondary("Surround")
    .doc("Direct signal bleed to surrounds"),
    // 26: rear_ambient_boost
    ParamSpec::float(
        "Rear Amb Boost",
        "rear_ambient_boost",
        1.5,
        1.0,
        3.0,
        0.05,
        "x",
        "Surround",
    )
    .secondary("Surround")
    .doc("Extra gain for rear ambience"),
    // 27: rear_late_reflection
    ParamSpec::float(
        "Rear Late Refl",
        "rear_late_reflection",
        0.1,
        0.0,
        0.5,
        0.01,
        "",
        "Surround",
    )
    .secondary("Surround")
    .doc("Simulated rear room reflections"),
    // Dialogue
    // 28: dialogue_weight
    ParamSpec::float(
        "Dialogue Weight",
        "dialogue_weight",
        0.4,
        0.0,
        1.0,
        0.05,
        "",
        "Dialogue",
    )
    .secondary("Dialogue")
    .doc("Voice routing to center channel"),
    // 29: voice_freq_min_hz
    ParamSpec::float(
        "Voice Freq Min",
        "voice_freq_min_hz",
        500.0,
        200.0,
        800.0,
        10.0,
        "Hz",
        "Dialogue",
    )
    .secondary("Dialogue")
    .doc("Voice detection low bound"),
    // 30: voice_freq_max_hz
    ParamSpec::float(
        "Voice Freq Max",
        "voice_freq_max_hz",
        3000.0,
        2000.0,
        5000.0,
        50.0,
        "Hz",
        "Dialogue",
    )
    .secondary("Dialogue")
    .doc("Voice detection high bound"),
    // 31: dialogue_centroid_weight
    ParamSpec::float(
        "Diag Centroid W",
        "dialogue_centroid_weight",
        0.3,
        0.0,
        1.0,
        0.05,
        "",
        "Dialogue",
    )
    .secondary("Dialogue")
    .doc("Spectral centroid score weight"),
    // 32: dialogue_variance_weight
    ParamSpec::float(
        "Diag Variance W",
        "dialogue_variance_weight",
        0.2,
        0.0,
        1.0,
        0.05,
        "",
        "Dialogue",
    )
    .secondary("Dialogue")
    .doc("Spectral variance score weight"),
    // 33: dialogue_coherence_weight
    ParamSpec::float(
        "Diag Coherence W",
        "dialogue_coherence_weight",
        0.5,
        0.0,
        1.0,
        0.05,
        "",
        "Dialogue",
    )
    .secondary("Dialogue")
    .doc("L/R coherence score weight"),
    // Output
    // 34: safety_cap_db
    ParamSpec::float(
        "Safety Cap",
        "safety_cap_db",
        3.0,
        0.0,
        3.0,
        0.1,
        "dB",
        "Output",
    )
    .output()
    .doc("Max output headroom limit"),
    // Analysis
    // 35: low_latency
    ParamSpec::bool_param("Low Latency", "low_latency", false, "Analysis")
        .secondary("Analysis")
        .structural()
        .doc("Smaller FFT for lower latency"),
    // 36: frequency_resolution
    ParamSpec::choice(
        "Freq Resolution",
        "frequency_resolution",
        0,
        FREQUENCY_RESOLUTIONS,
        "Analysis",
    )
    .secondary("Analysis")
    .structural()
    .doc("Frequency band grouping method"),
    // Diagnostics
    // 37: bypass_decorrelation
    ParamSpec::bool_param("Bypass Decor", "bypass_decorrelation", false, "Diagnostics")
        .diagnostic()
        .structural()
        .doc("Skip channel decorrelation"),
    // 38: bypass_transient_detection
    ParamSpec::bool_param(
        "Bypass Transients",
        "bypass_transient_detection",
        false,
        "Diagnostics",
    )
    .diagnostic()
    .doc("Skip transient detection"),
    // 39: bypass_all_processing
    ParamSpec::bool_param("Bypass All", "bypass_all_processing", false, "Diagnostics")
        .diagnostic()
        .doc("Pass audio through unprocessed"),
    // 40: enable_ml_detection
    ParamSpec::bool_param("ML Detection", "enable_ml_detection", false, "Diagnostics")
        .diagnostic()
        .doc("Use ML model for source detect"),
    // Analysis & Source Extraction
    // 41: multi_source_extraction
    ParamSpec::bool_param(
        "Multi-Source Extraction",
        "multi_source_extraction",
        false,
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Extract multiple sound sources"),
    // 42: multi_source_threshold
    ParamSpec::float(
        "Multi-Source Threshold",
        "multi_source_threshold",
        0.1,
        0.05,
        0.5,
        0.01,
        "",
        "Analysis",
    )
    .secondary("Analysis")
    .doc("Source separation sensitivity"),
    // Phase 4G: SOTA addition
    ParamSpec::bool_param("Binaural Preview", "binaural_preview", false, "Config")
        .structural()
        .setup()
        .doc("Preview surround output binaurally (headphone monitoring, changes output to 2ch)"),
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", false, "Auto Gain")
        .output()
        .doc("Match rendered output loudness to the stereo input"),
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
];

/// Upmixer: 0=speaker_config, 1-4=gains, 5-11=LFE, 12-14=spatial,
/// 15-17=hr_direct, 18-21=decorrelation, 22-24=height, 25-27=surround,
/// 28-33=dialogue, 34=safety_cap, 35-36=analysis, 37-40=diagnostics,
/// 41-42=source_extraction, 43=binaural_preview, 44-46=auto_gain
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // output channels / speaker_config
        ControlSpec::toggle(43),  // binaural_preview
    ],
    main: &[
        ControlGroup {
            title: "GAINS",
            controls: &[
                ControlSpec::slider(1), // front_direct
                ControlSpec::slider(2), // front_ambient
                ControlSpec::slider(3), // rear_ambient
                ControlSpec::slider(4), // height_gain
            ],
        },
        ControlGroup {
            title: "SPATIAL",
            controls: &[
                ControlSpec::slider(12), // stereo_width
                ControlSpec::slider(13), // center_spread
                ControlSpec::slider(14), // bandpass_hz
            ],
        },
    ],
    output: &[
        ControlSpec::knob(34),   // safety_cap_db
        ControlSpec::toggle(44), // auto_gain_enabled
        ControlSpec::knob(45),   // auto_gain_max_db
        ControlSpec::knob(46),   // auto_gain_smoothing_ms
    ],
    tabs: &[
        TabSpec {
            name: "LFE & Bass",
            controls: &[
                ControlSpec::knob(5),   // lfe_gain
                ControlSpec::knob(6),   // lfe_cutoff_hz
                ControlSpec::toggle(7), // subharmonic_synth
                ControlSpec::knob(8),   // sub_gain
                ControlSpec::knob(9),   // sub_freq
                ControlSpec::knob(10),  // sub_attack
                ControlSpec::knob(11),  // sub_release
            ],
        },
        TabSpec {
            name: "Dialogue",
            controls: &[
                ControlSpec::knob(28), // dialogue_weight
                ControlSpec::knob(29), // voice_freq_min
                ControlSpec::knob(30), // voice_freq_max
                ControlSpec::knob(31), // centroid_weight
                ControlSpec::knob(32), // variance_weight
                ControlSpec::knob(33), // coherence_weight
            ],
        },
        TabSpec {
            name: "Ambient",
            controls: &[
                ControlSpec::knob(25), // surround_direct_bleed
                ControlSpec::knob(26), // rear_ambient_boost
                ControlSpec::knob(27), // rear_late_reflection
            ],
        },
        TabSpec {
            name: "Height",
            controls: &[
                ControlSpec::knob(22), // height_hf_cap
                ControlSpec::knob(23), // height_transient_reduction
                ControlSpec::knob(24), // height_direct_leak
            ],
        },
        TabSpec {
            name: "Enhancement",
            controls: &[
                ControlSpec::toggle(15),   // hr_direct
                ControlSpec::knob(16),     // hr_sharpen
                ControlSpec::knob(17),     // ambient_boost
                ControlSpec::selector(18), // decor_mode
                ControlSpec::knob(19),     // decor_lfo_rate
                ControlSpec::knob(20),     // velvet_duration
                ControlSpec::knob(21),     // velvet_density
            ],
        },
        TabSpec {
            name: "Analysis",
            controls: &[
                ControlSpec::toggle(35),   // low_latency
                ControlSpec::selector(36), // frequency_resolution
                ControlSpec::toggle(41),   // multi_source_extraction
                ControlSpec::knob(42),     // multi_source_threshold
            ],
        },
        TabSpec {
            name: "Diagnostics",
            controls: &[
                ControlSpec::toggle(37), // bypass_decorrelation
                ControlSpec::toggle(38), // bypass_transient_detection
                ControlSpec::toggle(39), // bypass_all
                ControlSpec::toggle(40), // ml_detection
            ],
        },
    ],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(180.0, 0.55),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(220.0, 0.65),
    ],
    dynamic_sections: &[],
};
