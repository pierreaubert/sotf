//! Black-box integration tests for `sotf-plugin-binaural`.
//!
//! These tests exercise the public `Plugin` API surface from outside the crate.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_binaural::{BinauralDecoderParams, BinauralDecoderPlugin, RoomModel};

fn default_params() -> BinauralDecoderParams {
    BinauralDecoderParams {
        hrtf_file: String::new(),
        fft_size: 1024,
        input_channels: 2,
        enable_optimization: true,
        externalization: 0.0,
        near_field_strength: 0.0,
        diffuse_field_eq: false,
        lfe_crossover: 120.0,
        lfe_distance: 2.0,
        lfe_level: 0.0,
        room_model: RoomModel::default(),
        srir_file: String::new(),
        hrtf_database_dir: String::new(),
        head_width_cm: 15.0,
        ear_height_cm: 10.0,
    }
}

#[test]
fn construct_from_params_default() {
    let plugin = BinauralDecoderPlugin::from_params(default_params());
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    assert_eq!(plugin.info().name, "Binaural Decoder");
    assert!(!plugin.parameters().is_empty());
}

#[test]
fn construct_new_and_trait_metadata() {
    let plugin = BinauralDecoderPlugin::new(
        6,
        2048,
        None,
        false,
        0.3,
        0.5,
        true,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    assert_eq!(plugin.input_channels(), 6);
    assert_eq!(plugin.output_channels(), 2);
    assert_eq!(plugin.info().version, "2.1.0");
    assert_eq!(plugin.info().author, "SotF");
}

#[test]
fn initialize_then_process_silence() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let input = vec![0.0_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * 2];
    let ctx = ProcessContext::new(48000, num_frames);

    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    for s in &output {
        assert!(s.abs() < 1e-6, "silence should stay near zero, got {}", s);
    }
}

#[test]
fn process_tone_produces_output() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    plugin.initialize(48000).unwrap();

    let num_frames = 4096;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| {
            let t = (i / 2) as f32 / 48000.0;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3
        })
        .collect();
    let mut output = vec![0.0_f32; num_frames * 2];
    let ctx = ProcessContext::new(48000, num_frames);

    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    assert!(
        output.iter().any(|s| s.abs() > 1e-6),
        "tone should produce non-zero output"
    );
    assert!(
        output.iter().all(|s| s.is_finite()),
        "output must be finite"
    );
}

#[test]
fn parameter_get_set_roundtrip() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());

    // Known parameters exposed by the public trait
    plugin
        .set_parameter(
            ParameterId::from("externalization"),
            ParameterValue::Float(0.75),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("externalization")),
        Some(ParameterValue::Float(0.75))
    );

    plugin
        .set_parameter(
            ParameterId::from("near_field_strength"),
            ParameterValue::Float(0.6),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("near_field_strength")),
        Some(ParameterValue::Float(0.6))
    );

    plugin
        .set_parameter(
            ParameterId::from("crossfade_ms"),
            ParameterValue::Float(200.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("crossfade_ms")),
        Some(ParameterValue::Float(200.0))
    );

    plugin
        .set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(30.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("head_yaw_deg")),
        Some(ParameterValue::Float(30.0))
    );
}

#[test]
fn parameters_listed_by_trait() {
    let plugin = BinauralDecoderPlugin::from_params(default_params());
    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"externalization"));
    assert!(ids.contains(&"near_field_strength"));
    assert!(ids.contains(&"crossfade_ms"));
    assert!(ids.contains(&"head_yaw_deg"));
    assert!(ids.contains(&"hrtf_file"));
}

#[test]
fn invalid_parameter_type_rejected() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    let result = plugin.set_parameter(
        ParameterId::from("externalization"),
        ParameterValue::String("not-a-number".into()),
    );
    assert!(
        result.is_err(),
        "string value for float parameter must be rejected"
    );
}

#[test]
fn unknown_parameter_rejected() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    let result = plugin.set_parameter(
        ParameterId::from("does_not_exist"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err(), "unknown parameter must be rejected");
}

#[test]
fn reset_clears_processing_state() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    plugin.initialize(48000).unwrap();

    let num_frames = 1024;
    let input = vec![0.1_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * 2];
    let ctx = ProcessContext::new(48000, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    plugin.reset();

    let mut output_after = vec![0.0_f32; num_frames * 2];
    plugin.process(&input, &mut output_after, &ctx).unwrap();
    assert!(output_after.iter().all(|s| s.is_finite()));
}

#[test]
fn buffer_size_mismatch_is_error() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    plugin.initialize(48000).unwrap();

    let ctx = ProcessContext::new(48000, 64);
    let input = vec![0.0_f32; 64 * 2 - 1]; // one sample short
    let mut output = vec![0.0_f32; 64 * 2];
    assert!(plugin.process(&input, &mut output, &ctx).is_err());

    let input_ok = vec![0.0_f32; 64 * 2];
    let mut output_short = vec![0.0_f32; 64 * 2 - 1];
    assert!(plugin.process(&input_ok, &mut output_short, &ctx).is_err());
}

#[test]
fn missing_hrtf_file_error_during_set() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    plugin.initialize(48000).unwrap();

    let result = plugin.set_parameter(
        ParameterId::from("hrtf_file"),
        ParameterValue::String("/definitely/does/not/exist.sofa".into()),
    );
    assert!(result.is_err(), "missing SOFA file should produce an error");
}

#[test]
fn empty_hrtf_file_clears_path() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("hrtf_file"),
            ParameterValue::String("".into()),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("hrtf_file")),
        Some(ParameterValue::String(String::new()))
    );
}

#[test]
fn crossfade_mode_parameter_set_get() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("crossfade_mode")),
        Some(ParameterValue::Int(0))
    );

    plugin
        .set_parameter(ParameterId::from("crossfade_mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("crossfade_mode")),
        Some(ParameterValue::Int(1))
    );
}

#[test]
fn late_reverb_parameter_roundtrip() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("late_reverb_enabled"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("late_reverb_enabled")),
        Some(ParameterValue::Bool(true))
    );

    plugin
        .set_parameter(
            ParameterId::from("late_reverb_mix"),
            ParameterValue::Float(0.5),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("late_reverb_mix")),
        Some(ParameterValue::Float(0.5))
    );

    plugin
        .set_parameter(
            ParameterId::from("late_reverb_rt60"),
            ParameterValue::Float(1.5),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("late_reverb_damping"),
            ParameterValue::Float(0.5),
        )
        .unwrap();
}

#[test]
fn head_tracking_parameters_clamped() {
    let mut plugin = BinauralDecoderPlugin::from_params(default_params());

    plugin
        .set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(300.0),
        )
        .unwrap();
    let yaw = plugin.get_parameter(&ParameterId::from("head_yaw_deg"));
    assert!(matches!(yaw, Some(ParameterValue::Float(v)) if v <= 180.0));

    plugin
        .set_parameter(
            ParameterId::from("head_pitch_deg"),
            ParameterValue::Float(-300.0),
        )
        .unwrap();
    let pitch = plugin.get_parameter(&ParameterId::from("head_pitch_deg"));
    assert!(matches!(pitch, Some(ParameterValue::Float(v)) if v >= -180.0));
}

#[test]
fn latency_reported() {
    let plugin = BinauralDecoderPlugin::from_params(default_params());
    assert_eq!(plugin.latency_samples(), 1024);
}

#[test]
fn default_output_rate_and_frame_mapping() {
    let plugin = BinauralDecoderPlugin::from_params(default_params());
    assert_eq!(plugin.output_sample_rate(48000), 48000);
    assert_eq!(plugin.output_frames_for_input(256), 256);
    assert!(plugin.last_output_frames().is_none());
}

#[test]
fn from_params_preserves_custom_fields() {
    let mut params = default_params();
    params.input_channels = 6;
    params.fft_size = 2048;
    params.externalization = 0.5;
    params.hrtf_database_dir = "/tmp".into();

    let plugin = BinauralDecoderPlugin::from_params(params);
    assert_eq!(plugin.input_channels(), 6);
    assert_eq!(plugin.latency_samples(), 2048);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("hrtf_database_dir")),
        Some(ParameterValue::String("/tmp".into()))
    );
}
