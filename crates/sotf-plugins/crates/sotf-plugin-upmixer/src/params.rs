//! Upmixer plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Choice label constants
//! - Serializable state (Params struct with serde defaults)
//! - Index↔field mapping (PluginParamDef impl)
//!
//! Adding a parameter: add to PARAMS, add field to Params, add match arms.
//! Nothing else needs to change.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Choice label constants
// ============================================================================

pub const SPEAKER_CONFIGS: &[&str] = &[
    "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
];

pub const DECORRELATION_MODES: &[&str] = &["Velvet Noise", "LFO Phase"];

pub const FREQUENCY_RESOLUTIONS: &[&str] = &["ERB", "Fine ERB", "Per Bin"];

// ============================================================================
// Parameter Specifications
// ============================================================================

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
        .setup()
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
        .setup()
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
    ParamSpec::bool_param("Binaural Preview", "binaural_preview", false, "Output")
        .structural()
        .doc("Preview surround output binaurally (headphone monitoring, changes output to 2ch)"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Upmixer: 0=speaker_config, 1-4=gains, 5-11=LFE, 12-14=spatial,
/// 15-17=hr_direct, 18-21=decorrelation, 22-24=height, 25-27=surround,
/// 28-33=dialogue, 34=safety_cap, 35-36=analysis, 37-40=diagnostics,
/// 41-42=source_extraction, 43=binaural_preview
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(0), // speaker_config
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
        ControlSpec::knob(34), // safety_cap_db
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
            ],
        },
        TabSpec {
            name: "Source",
            controls: &[
                ControlSpec::toggle(41), // multi_source_extraction
                ControlSpec::knob(42),   // multi_source_threshold
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
        ColumnConstraint::config(120.0, 0.5),
        ColumnConstraint::main(400.0),
        ColumnConstraint::output(100.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Upmixer plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_speaker_config")]
    pub speaker_config: usize,
    #[serde(default = "d_gain_front_direct")]
    pub gain_front_direct: f64,
    #[serde(default = "d_gain_front_ambient")]
    pub gain_front_ambient: f64,
    #[serde(default = "d_gain_rear_ambient")]
    pub gain_rear_ambient: f64,
    #[serde(default = "d_height_gain")]
    pub height_gain: f64,
    #[serde(default = "d_lfe_gain")]
    pub lfe_gain: f64,
    #[serde(default = "d_lfe_cutoff_hz")]
    pub lfe_cutoff_hz: f64,
    #[serde(default = "d_enable_subharmonic_synth")]
    pub enable_subharmonic_synth: bool,
    #[serde(default = "d_subharmonic_gain")]
    pub subharmonic_gain: f64,
    #[serde(default = "d_subharmonic_freq_hz")]
    pub subharmonic_freq_hz: f64,
    #[serde(default = "d_subharmonic_attack_ms")]
    pub subharmonic_attack_ms: f64,
    #[serde(default = "d_subharmonic_release_ms")]
    pub subharmonic_release_ms: f64,
    #[serde(default = "d_stereo_width")]
    pub stereo_width: f64,
    #[serde(default = "d_center_spread")]
    pub center_spread: f64,
    #[serde(default = "d_bandpass_hz")]
    pub bandpass_hz: f64,
    #[serde(default = "d_enable_hr_direct")]
    pub enable_hr_direct: bool,
    #[serde(default = "d_hr_sharpen")]
    pub hr_sharpen: f64,
    #[serde(default = "d_ambient_boost")]
    pub ambient_boost: f64,
    #[serde(default = "d_decorrelation_mode")]
    pub decorrelation_mode: usize,
    #[serde(default = "d_decorrelation_lfo_rate_hz")]
    pub decorrelation_lfo_rate_hz: f64,
    #[serde(default = "d_velvet_noise_duration_ms")]
    pub velvet_noise_duration_ms: f64,
    #[serde(default = "d_velvet_noise_density")]
    pub velvet_noise_density: f64,
    #[serde(default = "d_height_hf_cap_hz")]
    pub height_hf_cap_hz: f64,
    #[serde(default = "d_height_transient_reduction")]
    pub height_transient_reduction: f64,
    #[serde(default = "d_height_direct_leak")]
    pub height_direct_leak: f64,
    #[serde(default = "d_surround_direct_bleed")]
    pub surround_direct_bleed: f64,
    #[serde(default = "d_rear_ambient_boost")]
    pub rear_ambient_boost: f64,
    #[serde(default = "d_rear_late_reflection")]
    pub rear_late_reflection: f64,
    #[serde(default = "d_dialogue_weight")]
    pub dialogue_weight: f64,
    #[serde(default = "d_voice_freq_min_hz")]
    pub voice_freq_min_hz: f64,
    #[serde(default = "d_voice_freq_max_hz")]
    pub voice_freq_max_hz: f64,
    #[serde(default = "d_dialogue_centroid_weight")]
    pub dialogue_centroid_weight: f64,
    #[serde(default = "d_dialogue_variance_weight")]
    pub dialogue_variance_weight: f64,
    #[serde(default = "d_dialogue_coherence_weight")]
    pub dialogue_coherence_weight: f64,
    #[serde(default = "d_safety_cap_db")]
    pub safety_cap_db: f64,
    #[serde(default = "d_low_latency")]
    pub low_latency: bool,
    #[serde(default = "d_frequency_resolution")]
    pub frequency_resolution: usize,
    #[serde(default = "d_bypass_decorrelation")]
    pub bypass_decorrelation: bool,
    #[serde(default = "d_bypass_transient_detection")]
    pub bypass_transient_detection: bool,
    #[serde(default = "d_bypass_all_processing")]
    pub bypass_all_processing: bool,
    #[serde(default = "d_enable_ml_detection")]
    pub enable_ml_detection: bool,
    #[serde(default = "d_multi_source_extraction")]
    pub multi_source_extraction: bool,
    #[serde(default = "d_multi_source_threshold")]
    pub multi_source_threshold: f64,
    #[serde(default)]
    pub binaural_preview: bool,
}

fn d_speaker_config() -> usize {
    pk(PARAMS, "speaker_config").default_usize()
}
fn d_gain_front_direct() -> f64 {
    pk(PARAMS, "gain_front_direct").default_f64()
}
fn d_gain_front_ambient() -> f64 {
    pk(PARAMS, "gain_front_ambient").default_f64()
}
fn d_gain_rear_ambient() -> f64 {
    pk(PARAMS, "gain_rear_ambient").default_f64()
}
fn d_height_gain() -> f64 {
    pk(PARAMS, "height_gain").default_f64()
}
fn d_lfe_gain() -> f64 {
    pk(PARAMS, "lfe_gain").default_f64()
}
fn d_lfe_cutoff_hz() -> f64 {
    pk(PARAMS, "lfe_cutoff_hz").default_f64()
}
fn d_enable_subharmonic_synth() -> bool {
    pk(PARAMS, "enable_subharmonic_synth").default_bool()
}
fn d_subharmonic_gain() -> f64 {
    pk(PARAMS, "subharmonic_gain").default_f64()
}
fn d_subharmonic_freq_hz() -> f64 {
    pk(PARAMS, "subharmonic_freq_hz").default_f64()
}
fn d_subharmonic_attack_ms() -> f64 {
    pk(PARAMS, "subharmonic_attack_ms").default_f64()
}
fn d_subharmonic_release_ms() -> f64 {
    pk(PARAMS, "subharmonic_release_ms").default_f64()
}
fn d_stereo_width() -> f64 {
    pk(PARAMS, "stereo_width").default_f64()
}
fn d_center_spread() -> f64 {
    pk(PARAMS, "center_spread").default_f64()
}
fn d_bandpass_hz() -> f64 {
    pk(PARAMS, "bandpass_hz").default_f64()
}
fn d_enable_hr_direct() -> bool {
    pk(PARAMS, "enable_hr_direct").default_bool()
}
fn d_hr_sharpen() -> f64 {
    pk(PARAMS, "hr_sharpen").default_f64()
}
fn d_ambient_boost() -> f64 {
    pk(PARAMS, "ambient_boost").default_f64()
}
fn d_decorrelation_mode() -> usize {
    pk(PARAMS, "decorrelation_mode").default_usize()
}
fn d_decorrelation_lfo_rate_hz() -> f64 {
    pk(PARAMS, "decorrelation_lfo_rate_hz").default_f64()
}
fn d_velvet_noise_duration_ms() -> f64 {
    pk(PARAMS, "velvet_noise_duration_ms").default_f64()
}
fn d_velvet_noise_density() -> f64 {
    pk(PARAMS, "velvet_noise_density").default_f64()
}
fn d_height_hf_cap_hz() -> f64 {
    pk(PARAMS, "height_hf_cap_hz").default_f64()
}
fn d_height_transient_reduction() -> f64 {
    pk(PARAMS, "height_transient_reduction").default_f64()
}
fn d_height_direct_leak() -> f64 {
    pk(PARAMS, "height_direct_leak").default_f64()
}
fn d_surround_direct_bleed() -> f64 {
    pk(PARAMS, "surround_direct_bleed").default_f64()
}
fn d_rear_ambient_boost() -> f64 {
    pk(PARAMS, "rear_ambient_boost").default_f64()
}
fn d_rear_late_reflection() -> f64 {
    pk(PARAMS, "rear_late_reflection").default_f64()
}
fn d_dialogue_weight() -> f64 {
    pk(PARAMS, "dialogue_weight").default_f64()
}
fn d_voice_freq_min_hz() -> f64 {
    pk(PARAMS, "voice_freq_min_hz").default_f64()
}
fn d_voice_freq_max_hz() -> f64 {
    pk(PARAMS, "voice_freq_max_hz").default_f64()
}
fn d_dialogue_centroid_weight() -> f64 {
    pk(PARAMS, "dialogue_centroid_weight").default_f64()
}
fn d_dialogue_variance_weight() -> f64 {
    pk(PARAMS, "dialogue_variance_weight").default_f64()
}
fn d_dialogue_coherence_weight() -> f64 {
    pk(PARAMS, "dialogue_coherence_weight").default_f64()
}
fn d_safety_cap_db() -> f64 {
    pk(PARAMS, "safety_cap_db").default_f64()
}
fn d_low_latency() -> bool {
    pk(PARAMS, "low_latency").default_bool()
}
fn d_frequency_resolution() -> usize {
    pk(PARAMS, "frequency_resolution").default_usize()
}
fn d_bypass_decorrelation() -> bool {
    pk(PARAMS, "bypass_decorrelation").default_bool()
}
fn d_bypass_transient_detection() -> bool {
    pk(PARAMS, "bypass_transient_detection").default_bool()
}
fn d_bypass_all_processing() -> bool {
    pk(PARAMS, "bypass_all_processing").default_bool()
}
fn d_enable_ml_detection() -> bool {
    pk(PARAMS, "enable_ml_detection").default_bool()
}
fn d_multi_source_extraction() -> bool {
    pk(PARAMS, "multi_source_extraction").default_bool()
}
fn d_multi_source_threshold() -> f64 {
    pk(PARAMS, "multi_source_threshold").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            speaker_config: d_speaker_config(),
            gain_front_direct: d_gain_front_direct(),
            gain_front_ambient: d_gain_front_ambient(),
            gain_rear_ambient: d_gain_rear_ambient(),
            height_gain: d_height_gain(),
            lfe_gain: d_lfe_gain(),
            lfe_cutoff_hz: d_lfe_cutoff_hz(),
            enable_subharmonic_synth: d_enable_subharmonic_synth(),
            subharmonic_gain: d_subharmonic_gain(),
            subharmonic_freq_hz: d_subharmonic_freq_hz(),
            subharmonic_attack_ms: d_subharmonic_attack_ms(),
            subharmonic_release_ms: d_subharmonic_release_ms(),
            stereo_width: d_stereo_width(),
            center_spread: d_center_spread(),
            bandpass_hz: d_bandpass_hz(),
            enable_hr_direct: d_enable_hr_direct(),
            hr_sharpen: d_hr_sharpen(),
            ambient_boost: d_ambient_boost(),
            decorrelation_mode: d_decorrelation_mode(),
            decorrelation_lfo_rate_hz: d_decorrelation_lfo_rate_hz(),
            velvet_noise_duration_ms: d_velvet_noise_duration_ms(),
            velvet_noise_density: d_velvet_noise_density(),
            height_hf_cap_hz: d_height_hf_cap_hz(),
            height_transient_reduction: d_height_transient_reduction(),
            height_direct_leak: d_height_direct_leak(),
            surround_direct_bleed: d_surround_direct_bleed(),
            rear_ambient_boost: d_rear_ambient_boost(),
            rear_late_reflection: d_rear_late_reflection(),
            dialogue_weight: d_dialogue_weight(),
            voice_freq_min_hz: d_voice_freq_min_hz(),
            voice_freq_max_hz: d_voice_freq_max_hz(),
            dialogue_centroid_weight: d_dialogue_centroid_weight(),
            dialogue_variance_weight: d_dialogue_variance_weight(),
            dialogue_coherence_weight: d_dialogue_coherence_weight(),
            safety_cap_db: d_safety_cap_db(),
            low_latency: d_low_latency(),
            frequency_resolution: d_frequency_resolution(),
            bypass_decorrelation: d_bypass_decorrelation(),
            bypass_transient_detection: d_bypass_transient_detection(),
            bypass_all_processing: d_bypass_all_processing(),
            enable_ml_detection: d_enable_ml_detection(),
            multi_source_extraction: d_multi_source_extraction(),
            multi_source_threshold: d_multi_source_threshold(),
            binaural_preview: false,
        }
    }
}

// ============================================================================
// PluginParamDef implementation
// ============================================================================

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "upmixer";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.speaker_config as f64),
            1 => Some(self.gain_front_direct),
            2 => Some(self.gain_front_ambient),
            3 => Some(self.gain_rear_ambient),
            4 => Some(self.height_gain),
            5 => Some(self.lfe_gain),
            6 => Some(self.lfe_cutoff_hz),
            7 => Some(if self.enable_subharmonic_synth {
                1.0
            } else {
                0.0
            }),
            8 => Some(self.subharmonic_gain),
            9 => Some(self.subharmonic_freq_hz),
            10 => Some(self.subharmonic_attack_ms),
            11 => Some(self.subharmonic_release_ms),
            12 => Some(self.stereo_width),
            13 => Some(self.center_spread),
            14 => Some(self.bandpass_hz),
            15 => Some(if self.enable_hr_direct { 1.0 } else { 0.0 }),
            16 => Some(self.hr_sharpen),
            17 => Some(self.ambient_boost),
            18 => Some(self.decorrelation_mode as f64),
            19 => Some(self.decorrelation_lfo_rate_hz),
            20 => Some(self.velvet_noise_duration_ms),
            21 => Some(self.velvet_noise_density),
            22 => Some(self.height_hf_cap_hz),
            23 => Some(self.height_transient_reduction),
            24 => Some(self.height_direct_leak),
            25 => Some(self.surround_direct_bleed),
            26 => Some(self.rear_ambient_boost),
            27 => Some(self.rear_late_reflection),
            28 => Some(self.dialogue_weight),
            29 => Some(self.voice_freq_min_hz),
            30 => Some(self.voice_freq_max_hz),
            31 => Some(self.dialogue_centroid_weight),
            32 => Some(self.dialogue_variance_weight),
            33 => Some(self.dialogue_coherence_weight),
            34 => Some(self.safety_cap_db),
            35 => Some(if self.low_latency { 1.0 } else { 0.0 }),
            36 => Some(self.frequency_resolution as f64),
            37 => Some(if self.bypass_decorrelation { 1.0 } else { 0.0 }),
            38 => Some(if self.bypass_transient_detection {
                1.0
            } else {
                0.0
            }),
            39 => Some(if self.bypass_all_processing { 1.0 } else { 0.0 }),
            40 => Some(if self.enable_ml_detection { 1.0 } else { 0.0 }),
            41 => Some(if self.multi_source_extraction {
                1.0
            } else {
                0.0
            }),
            42 => Some(self.multi_source_threshold),
            43 => Some(if self.binaural_preview { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.speaker_config = value as usize,
            1 => self.gain_front_direct = value,
            2 => self.gain_front_ambient = value,
            3 => self.gain_rear_ambient = value,
            4 => self.height_gain = value,
            5 => self.lfe_gain = value,
            6 => self.lfe_cutoff_hz = value,
            7 => self.enable_subharmonic_synth = value > 0.5,
            8 => self.subharmonic_gain = value,
            9 => self.subharmonic_freq_hz = value,
            10 => self.subharmonic_attack_ms = value,
            11 => self.subharmonic_release_ms = value,
            12 => self.stereo_width = value,
            13 => self.center_spread = value,
            14 => self.bandpass_hz = value,
            15 => self.enable_hr_direct = value > 0.5,
            16 => self.hr_sharpen = value,
            17 => self.ambient_boost = value,
            18 => self.decorrelation_mode = value as usize,
            19 => self.decorrelation_lfo_rate_hz = value,
            20 => self.velvet_noise_duration_ms = value,
            21 => self.velvet_noise_density = value,
            22 => self.height_hf_cap_hz = value,
            23 => self.height_transient_reduction = value,
            24 => self.height_direct_leak = value,
            25 => self.surround_direct_bleed = value,
            26 => self.rear_ambient_boost = value,
            27 => self.rear_late_reflection = value,
            28 => self.dialogue_weight = value,
            29 => self.voice_freq_min_hz = value,
            30 => self.voice_freq_max_hz = value,
            31 => self.dialogue_centroid_weight = value,
            32 => self.dialogue_variance_weight = value,
            33 => self.dialogue_coherence_weight = value,
            34 => self.safety_cap_db = value,
            35 => self.low_latency = value > 0.5,
            36 => self.frequency_resolution = value as usize,
            37 => self.bypass_decorrelation = value > 0.5,
            38 => self.bypass_transient_detection = value > 0.5,
            39 => self.bypass_all_processing = value > 0.5,
            40 => self.enable_ml_detection = value > 0.5,
            41 => self.multi_source_extraction = value > 0.5,
            42 => self.multi_source_threshold = value,
            43 => self.binaural_preview = value > 0.5,
            _ => {}
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_index_coverage() {
        let p = Params::default();
        for i in 0..PARAMS.len() {
            assert!(
                p.param_value(i).is_some(),
                "param_value({}) returned None",
                i
            );
        }
        assert!(
            p.param_value(PARAMS.len()).is_none(),
            "param_value beyond PARAMS.len() should return None"
        );
    }

    #[test]
    fn param_count() {
        assert_eq!(PARAMS.len(), 44, "Expected 44 params");
    }

    #[test]
    fn roundtrip_serde() {
        let original = Params::default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: Params = serde_json::from_value(json).unwrap();
        assert_eq!(original.speaker_config, restored.speaker_config);
        assert_eq!(original.gain_front_direct, restored.gain_front_direct);
        assert_eq!(original.gain_front_ambient, restored.gain_front_ambient);
        assert_eq!(original.gain_rear_ambient, restored.gain_rear_ambient);
        assert_eq!(original.height_gain, restored.height_gain);
        assert_eq!(original.lfe_gain, restored.lfe_gain);
        assert_eq!(original.lfe_cutoff_hz, restored.lfe_cutoff_hz);
        assert_eq!(
            original.enable_subharmonic_synth,
            restored.enable_subharmonic_synth
        );
        assert_eq!(original.subharmonic_gain, restored.subharmonic_gain);
        assert_eq!(original.subharmonic_freq_hz, restored.subharmonic_freq_hz);
        assert_eq!(
            original.subharmonic_attack_ms,
            restored.subharmonic_attack_ms
        );
        assert_eq!(
            original.subharmonic_release_ms,
            restored.subharmonic_release_ms
        );
        assert_eq!(original.stereo_width, restored.stereo_width);
        assert_eq!(original.center_spread, restored.center_spread);
        assert_eq!(original.bandpass_hz, restored.bandpass_hz);
        assert_eq!(original.enable_hr_direct, restored.enable_hr_direct);
        assert_eq!(original.hr_sharpen, restored.hr_sharpen);
        assert_eq!(original.ambient_boost, restored.ambient_boost);
        assert_eq!(original.decorrelation_mode, restored.decorrelation_mode);
        assert_eq!(
            original.decorrelation_lfo_rate_hz,
            restored.decorrelation_lfo_rate_hz
        );
        assert_eq!(
            original.velvet_noise_duration_ms,
            restored.velvet_noise_duration_ms
        );
        assert_eq!(original.velvet_noise_density, restored.velvet_noise_density);
        assert_eq!(original.height_hf_cap_hz, restored.height_hf_cap_hz);
        assert_eq!(
            original.height_transient_reduction,
            restored.height_transient_reduction
        );
        assert_eq!(original.height_direct_leak, restored.height_direct_leak);
        assert_eq!(
            original.surround_direct_bleed,
            restored.surround_direct_bleed
        );
        assert_eq!(original.rear_ambient_boost, restored.rear_ambient_boost);
        assert_eq!(original.rear_late_reflection, restored.rear_late_reflection);
        assert_eq!(original.dialogue_weight, restored.dialogue_weight);
        assert_eq!(original.voice_freq_min_hz, restored.voice_freq_min_hz);
        assert_eq!(original.voice_freq_max_hz, restored.voice_freq_max_hz);
        assert_eq!(
            original.dialogue_centroid_weight,
            restored.dialogue_centroid_weight
        );
        assert_eq!(
            original.dialogue_variance_weight,
            restored.dialogue_variance_weight
        );
        assert_eq!(
            original.dialogue_coherence_weight,
            restored.dialogue_coherence_weight
        );
        assert_eq!(original.safety_cap_db, restored.safety_cap_db);
        assert_eq!(original.low_latency, restored.low_latency);
        assert_eq!(original.frequency_resolution, restored.frequency_resolution);
        assert_eq!(original.bypass_decorrelation, restored.bypass_decorrelation);
        assert_eq!(
            original.bypass_transient_detection,
            restored.bypass_transient_detection
        );
        assert_eq!(
            original.bypass_all_processing,
            restored.bypass_all_processing
        );
        assert_eq!(original.enable_ml_detection, restored.enable_ml_detection);
        assert_eq!(
            original.multi_source_extraction,
            restored.multi_source_extraction
        );
        assert_eq!(
            original.multi_source_threshold,
            restored.multi_source_threshold
        );
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.speaker_config,
            pk(PARAMS, "speaker_config").default_usize()
        );
        assert_eq!(
            p.gain_front_direct,
            pk(PARAMS, "gain_front_direct").default_f64()
        );
        assert_eq!(
            p.gain_front_ambient,
            pk(PARAMS, "gain_front_ambient").default_f64()
        );
        assert_eq!(
            p.gain_rear_ambient,
            pk(PARAMS, "gain_rear_ambient").default_f64()
        );
        assert_eq!(p.height_gain, pk(PARAMS, "height_gain").default_f64());
        assert_eq!(p.lfe_gain, pk(PARAMS, "lfe_gain").default_f64());
        assert_eq!(p.lfe_cutoff_hz, pk(PARAMS, "lfe_cutoff_hz").default_f64());
        assert_eq!(
            p.enable_subharmonic_synth,
            pk(PARAMS, "enable_subharmonic_synth").default_bool()
        );
        assert_eq!(
            p.subharmonic_gain,
            pk(PARAMS, "subharmonic_gain").default_f64()
        );
        assert_eq!(
            p.subharmonic_freq_hz,
            pk(PARAMS, "subharmonic_freq_hz").default_f64()
        );
        assert_eq!(
            p.subharmonic_attack_ms,
            pk(PARAMS, "subharmonic_attack_ms").default_f64()
        );
        assert_eq!(
            p.subharmonic_release_ms,
            pk(PARAMS, "subharmonic_release_ms").default_f64()
        );
        assert_eq!(p.stereo_width, pk(PARAMS, "stereo_width").default_f64());
        assert_eq!(p.center_spread, pk(PARAMS, "center_spread").default_f64());
        assert_eq!(p.bandpass_hz, pk(PARAMS, "bandpass_hz").default_f64());
        assert_eq!(
            p.enable_hr_direct,
            pk(PARAMS, "enable_hr_direct").default_bool()
        );
        assert_eq!(p.hr_sharpen, pk(PARAMS, "hr_sharpen").default_f64());
        assert_eq!(p.ambient_boost, pk(PARAMS, "ambient_boost").default_f64());
        assert_eq!(
            p.decorrelation_mode,
            pk(PARAMS, "decorrelation_mode").default_usize()
        );
        assert_eq!(
            p.decorrelation_lfo_rate_hz,
            pk(PARAMS, "decorrelation_lfo_rate_hz").default_f64()
        );
        assert_eq!(
            p.velvet_noise_duration_ms,
            pk(PARAMS, "velvet_noise_duration_ms").default_f64()
        );
        assert_eq!(
            p.velvet_noise_density,
            pk(PARAMS, "velvet_noise_density").default_f64()
        );
        assert_eq!(
            p.height_hf_cap_hz,
            pk(PARAMS, "height_hf_cap_hz").default_f64()
        );
        assert_eq!(
            p.height_transient_reduction,
            pk(PARAMS, "height_transient_reduction").default_f64()
        );
        assert_eq!(
            p.height_direct_leak,
            pk(PARAMS, "height_direct_leak").default_f64()
        );
        assert_eq!(
            p.surround_direct_bleed,
            pk(PARAMS, "surround_direct_bleed").default_f64()
        );
        assert_eq!(
            p.rear_ambient_boost,
            pk(PARAMS, "rear_ambient_boost").default_f64()
        );
        assert_eq!(
            p.rear_late_reflection,
            pk(PARAMS, "rear_late_reflection").default_f64()
        );
        assert_eq!(
            p.dialogue_weight,
            pk(PARAMS, "dialogue_weight").default_f64()
        );
        assert_eq!(
            p.voice_freq_min_hz,
            pk(PARAMS, "voice_freq_min_hz").default_f64()
        );
        assert_eq!(
            p.voice_freq_max_hz,
            pk(PARAMS, "voice_freq_max_hz").default_f64()
        );
        assert_eq!(
            p.dialogue_centroid_weight,
            pk(PARAMS, "dialogue_centroid_weight").default_f64()
        );
        assert_eq!(
            p.dialogue_variance_weight,
            pk(PARAMS, "dialogue_variance_weight").default_f64()
        );
        assert_eq!(
            p.dialogue_coherence_weight,
            pk(PARAMS, "dialogue_coherence_weight").default_f64()
        );
        assert_eq!(p.safety_cap_db, pk(PARAMS, "safety_cap_db").default_f64());
        assert_eq!(p.low_latency, pk(PARAMS, "low_latency").default_bool());
        assert_eq!(
            p.frequency_resolution,
            pk(PARAMS, "frequency_resolution").default_usize()
        );
        assert_eq!(
            p.bypass_decorrelation,
            pk(PARAMS, "bypass_decorrelation").default_bool()
        );
        assert_eq!(
            p.bypass_transient_detection,
            pk(PARAMS, "bypass_transient_detection").default_bool()
        );
        assert_eq!(
            p.bypass_all_processing,
            pk(PARAMS, "bypass_all_processing").default_bool()
        );
        assert_eq!(
            p.enable_ml_detection,
            pk(PARAMS, "enable_ml_detection").default_bool()
        );
        assert_eq!(
            p.multi_source_extraction,
            pk(PARAMS, "multi_source_extraction").default_bool()
        );
        assert_eq!(
            p.multi_source_threshold,
            pk(PARAMS, "multi_source_threshold").default_f64()
        );
    }

    #[test]
    fn speaker_config_labels_match() {
        let labels = pk(PARAMS, "speaker_config").choice_labels();
        assert_eq!(labels, SPEAKER_CONFIGS);
    }

    #[test]
    fn decorrelation_mode_labels_match() {
        let labels = pk(PARAMS, "decorrelation_mode").choice_labels();
        assert_eq!(labels, DECORRELATION_MODES);
    }

    #[test]
    fn frequency_resolution_labels_match() {
        let labels = pk(PARAMS, "frequency_resolution").choice_labels();
        assert_eq!(labels, FREQUENCY_RESOLUTIONS);
    }
}
