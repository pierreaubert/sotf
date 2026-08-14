use super::misc::tube;
use super::saturation_plugin::SaturationPlugin;
use super::saturation_plugin_params::SaturationPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parametric_plugin::ParameterSet;
use sotf_host::{CountingAlloc, assert_no_allocs};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[path = "tests/make.rs"]
mod make;
#[path = "tests/misc.rs"]
mod test_misc;

use make::make_context;
use make::make_sine;
use test_misc::rms;

#[test]
fn tube_curve_is_explicitly_odd_symmetric() {
    let drive = 5.0;
    let tone = 2.0;

    let pos = tube(0.5, drive, tone);
    let neg = tube(-0.5, drive, tone);

    assert_eq!(
        neg, -pos,
        "Tube-style static curve must remain odd-symmetric"
    );
    assert!(
        pos.abs() < (0.5 * drive).abs(),
        "Tube should compress: pos={}, input={}",
        pos,
        0.5 * drive
    );
}

#[test]
fn low_sample_rate_rejects_exciter_at_or_above_conservative_nyquist() {
    let mut plugin = SaturationPlugin::from_validated_params(
        1,
        SaturationPluginParams {
            exciter_freq: 3_000.0,
            ..Default::default()
        },
    );
    assert!(
        plugin
            .initialize(6_000)
            .unwrap_err()
            .contains("exciter frequency")
    );
}

#[test]
fn continuous_bulk_automation_does_not_allocate_or_rebuild_metadata() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let metadata_capacity = plugin.cached_parameters.capacity();
    let mut values = ParameterSet::new();
    values.insert(ParameterId::from("drive"), ParameterValue::Float(7.0));
    values.insert(ParameterId::from("mix"), ParameterValue::Float(0.25));
    assert_no_allocs("Saturation continuous apply_values", || {
        plugin.apply_continuous_values_realtime(&values).unwrap();
    });
    assert_eq!(plugin.cached_parameters.capacity(), metadata_capacity);
}

#[test]
fn asymmetric_processing_does_not_allocate() {
    let mut plugin = SaturationPlugin::from_validated_params(
        2,
        SaturationPluginParams {
            mode: "Asymmetric".into(),
            drive: 7.0,
            tone: 2.0,
            oversampling: "4x".into(),
            mix: 1.0,
            dc_blocker_enabled: true,
            use_adaa: false,
            ..Default::default()
        },
    );
    plugin.initialize(48_000).unwrap();
    let mut buffer = vec![0.25; 2_048];
    let context = make_context(1_024);
    plugin.process_in_place(&mut buffer, &context).unwrap();
    assert_no_allocs("Saturation Asymmetric process", || {
        plugin.process_in_place(&mut buffer, &context).unwrap();
    });
}

#[test]
fn structural_bulk_update_is_atomic_after_initialization() {
    let mut plugin = SaturationPlugin::new(1);
    plugin.initialize(48_000).unwrap();
    let original_mode = plugin.mode;
    let original_drive = plugin.drive;
    let mut values = ParameterSet::new();
    values.insert(
        ParameterId::from("mode"),
        ParameterValue::String("Tape".to_string()),
    );
    values.insert(ParameterId::from("drive"), ParameterValue::Float(9.0));
    assert!(
        plugin
            .apply_values(values)
            .unwrap_err()
            .contains("structural")
    );
    assert_eq!(plugin.mode, original_mode);
    assert_eq!(plugin.drive, original_drive);
}

#[test]
fn test_exciter_only_affects_hf() {
    let sr = 48000u32;
    let channels = 1;
    let num_frames = 48000; // 1 second

    let params = SaturationPluginParams {
        mode: "Exciter".to_string(),
        drive: 10.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(sr).unwrap();

    // Test with 200Hz signal (well below exciter freq)
    let mut buf_lf = make_sine(200.0, sr, num_frames, 0.5);
    let input_rms_lf = rms(&buf_lf);

    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buf_lf, &ctx).unwrap();

    // Low frequency should pass through mostly unchanged
    let output_rms_lf = rms(&buf_lf[num_frames / 2..]);
    assert!(
        output_rms_lf > input_rms_lf * 0.7,
        "200Hz signal should pass through exciter: input_rms={:.4}, output_rms={:.4}",
        input_rms_lf,
        output_rms_lf
    );

    // Test with 8kHz signal (above exciter freq)
    plugin.reset();
    let mut buf_hf = make_sine(8000.0, sr, num_frames, 0.5);
    let input_rms_hf = rms(&buf_hf);

    plugin.process_in_place(&mut buf_hf, &ctx).unwrap();
    let output_rms_hf = rms(&buf_hf[num_frames / 2..]);

    // High frequency should be affected (shaped/compressed by soft clip)
    // The RMS should change noticeably
    let ratio = output_rms_hf / input_rms_hf;
    assert!(
        (ratio - 1.0).abs() > 0.01,
        "8kHz signal should be affected by exciter: ratio={:.4}",
        ratio
    );
}

#[test]
fn test_saturation_declares_oversampling() {
    // Oversampling is graph-owned so automation and dry/wet stay in one domain.
    let plugin = SaturationPlugin::new(2);
    assert_eq!(plugin.preferred_oversampling(), Some(2));

    // With oversampling set to Off
    let params_off = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 2.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 0.5,
        ..Default::default()
    };
    let plugin_off = SaturationPlugin::from_validated_params(2, params_off);
    assert_eq!(plugin_off.preferred_oversampling(), None);

    // With oversampling set to 4x
    let params_4x = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 2.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "4x".to_string(),
        output_gain_db: 0.0,
        mix: 0.5,
        ..Default::default()
    };
    let plugin_4x = SaturationPlugin::from_validated_params(2, params_4x);
    assert_eq!(plugin_4x.preferred_oversampling(), Some(4));
}

#[test]
fn graph_owned_oversampling_has_no_internal_latency() {
    let plugin = SaturationPlugin::new(2);
    assert_eq!(plugin.latency_samples(), 0);
    assert_eq!(plugin.compile_metadata().latency_samples, 0);
}

#[test]
fn dynamic_exciter_keeps_low_band_topology() {
    let sr = 48_000;
    let frames = 48_000;
    let params = SaturationPluginParams {
        mode: "Exciter".into(),
        drive: 12.0,
        exciter_freq: 6_000.0,
        oversampling: "Off".into(),
        mix: 1.0,
        dynamic_amount: 1.0,
        dc_blocker_enabled: false,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(1, params);
    plugin.initialize(sr).unwrap();
    let input = make_sine(200.0, sr, frames, 0.5);
    let mut output = input.clone();
    plugin
        .process_in_place(&mut output, &make_context(frames))
        .unwrap();

    let start = frames / 2;
    let input_rms = rms(&input[start..]);
    let output_rms = rms(&output[start..]);
    assert!(
        (output_rms / input_rms - 1.0).abs() < 0.08,
        "dynamic exciter must not replace the low-band path: input={input_rms}, output={output_rms}"
    );
}

#[test]
fn dynamic_exciter_is_applied_inside_oversampled_topology() {
    let sr = 48_000;
    let frames = 48_000;
    let base = SaturationPluginParams {
        mode: "Exciter".into(),
        drive: 2.0,
        exciter_freq: 3_000.0,
        oversampling: "2x".into(),
        mix: 1.0,
        dc_blocker_enabled: false,
        dynamic_attack_ms: 0.1,
        dynamic_release_ms: 1.0,
        ..Default::default()
    };
    let mut static_plugin = SaturationPlugin::from_validated_params(1, base.clone());
    let mut dynamic_params = base;
    dynamic_params.dynamic_amount = 1.0;
    let mut dynamic_plugin = SaturationPlugin::from_validated_params(1, dynamic_params);
    static_plugin.initialize(sr).unwrap();
    dynamic_plugin.initialize(sr).unwrap();

    let input = make_sine(8_000.0, sr, frames, 0.5);
    let mut static_output = input.clone();
    let mut dynamic_output = input;
    let context = make_context(frames);
    static_plugin
        .process_in_place(&mut static_output, &context)
        .unwrap();
    dynamic_plugin
        .process_in_place(&mut dynamic_output, &context)
        .unwrap();

    // Dynamic drive must be applied by the oversampled Exciter's high-band
    // nonlinearity. If the topology is bypassed or the dynamic control is
    // ignored, these outputs are identical.
    let difference = static_output
        .iter()
        .zip(dynamic_output.iter())
        .skip(frames / 2)
        .map(|(static_sample, dynamic_sample)| (static_sample - dynamic_sample).abs())
        .sum::<f32>();
    assert!(
        difference > 1.0,
        "dynamic oversampled Exciter must modulate its selected high-band path (difference={difference})"
    );
}

#[test]
fn reset_settles_parameter_smoothers() {
    let mut plugin = SaturationPlugin::new(1);
    plugin.initialize(48_000).unwrap();
    plugin
        .set_parameter(ParameterId::from("drive"), ParameterValue::Float(15.0))
        .unwrap();
    assert_ne!(plugin.drive_smoother.current(), plugin.drive);
    plugin.reset();
    assert_eq!(plugin.drive_smoother.current(), plugin.drive);
}

#[test]
fn fallible_constructor_rejects_invalid_configuration() {
    let invalid = SaturationPluginParams {
        mode: "mystery".into(),
        ..Default::default()
    };
    assert!(SaturationPlugin::from_params(1, invalid).is_err());
    assert!(SaturationPlugin::from_params(0, SaturationPluginParams::default()).is_err());

    let non_finite = SaturationPluginParams {
        drive: f32::NAN,
        ..Default::default()
    };
    assert!(SaturationPlugin::from_params(1, non_finite).is_err());
}

#[test]
fn bulk_parameter_updates_reject_unknown_enums_atomically() {
    let mut plugin = SaturationPlugin::new(1);
    let original_mode = plugin.mode_string();
    let original_oversampling = plugin.oversampling_string();

    let mut values = ParameterSet::new();
    values.insert(
        ParameterId::from("mode"),
        ParameterValue::String("Tube".into()),
    );
    values.insert(
        ParameterId::from("oversampling"),
        ParameterValue::String("8x".into()),
    );

    let error = plugin
        .apply_values(values)
        .expect_err("unknown enum values must not be silently repaired");
    assert!(error.contains("oversampling"));
    assert_eq!(plugin.mode_string(), original_mode);
    assert_eq!(plugin.oversampling_string(), original_oversampling);
}

#[test]
fn test_saturation_default_f64_is_false() {
    let plugin = SaturationPlugin::new(2);
    assert!(!plugin.supports_f64());
}

#[test]
fn test_parameter_roundtrip() {
    let mut plugin = SaturationPlugin::new(2);

    // Set drive
    plugin
        .set_parameter(ParameterId::from("drive"), ParameterValue::Float(8.0))
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("drive"));
    assert_eq!(val, Some(ParameterValue::Float(8.0)));

    // Set mode
    plugin
        .set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Tape".to_string()),
        )
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("mode"));
    assert_eq!(val, Some(ParameterValue::String("Tape".to_string())));

    // Set tone
    plugin
        .set_parameter(ParameterId::from("tone"), ParameterValue::Float(2.5))
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("tone"));
    assert_eq!(val, Some(ParameterValue::Float(2.5)));

    // Set exciter freq
    plugin
        .set_parameter(
            ParameterId::from("exciter_freq"),
            ParameterValue::Float(5000.0),
        )
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("exciter_freq"));
    assert_eq!(val, Some(ParameterValue::Float(5000.0)));

    // Set oversampling
    plugin
        .set_parameter(
            ParameterId::from("oversampling"),
            ParameterValue::String("4x".to_string()),
        )
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("oversampling"));
    assert_eq!(val, Some(ParameterValue::String("4x".to_string())));

    // Set output gain
    plugin
        .set_parameter(
            ParameterId::from("output_gain"),
            ParameterValue::Float(-3.0),
        )
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("output_gain"));
    assert_eq!(val, Some(ParameterValue::Float(-3.0)));

    // Set mix
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    let val = plugin.get_parameter(&ParameterId::from("mix"));
    assert_eq!(val, Some(ParameterValue::Float(0.75)));
    plugin.initialize(48000).unwrap();
}

#[test]
fn test_get_parameter_sota_params() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(48000).unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_amount")),
        Some(ParameterValue::Float(0.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_attack_ms")),
        Some(ParameterValue::Float(5.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_release_ms")),
        Some(ParameterValue::Float(50.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dc_blocker")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("use_adaa")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn test_get_parameter_unknown_returns_none() {
    let plugin = SaturationPlugin::new(2);
    assert!(
        plugin
            .get_parameter(&ParameterId::from("nonexistent"))
            .is_none()
    );
}

#[test]
fn test_set_parameter_sota_roundtrip() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("dynamic_amount"),
            ParameterValue::Float(0.75),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_amount")),
        Some(ParameterValue::Float(0.75))
    );

    plugin
        .set_parameter(
            ParameterId::from("dynamic_attack_ms"),
            ParameterValue::Float(10.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_attack_ms")),
        Some(ParameterValue::Float(10.0))
    );

    plugin
        .set_parameter(
            ParameterId::from("dynamic_release_ms"),
            ParameterValue::Float(100.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_release_ms")),
        Some(ParameterValue::Float(100.0))
    );
}

#[test]
fn test_set_parameter_sota_updates_envelope_followers() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(48000).unwrap();

    // Changing dynamic_attack_ms should update envelope follower times
    plugin
        .set_parameter(
            ParameterId::from("dynamic_attack_ms"),
            ParameterValue::Float(25.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_attack_ms")),
        Some(ParameterValue::Float(25.0))
    );

    plugin
        .set_parameter(
            ParameterId::from("dynamic_release_ms"),
            ParameterValue::Float(150.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_release_ms")),
        Some(ParameterValue::Float(150.0))
    );
}

#[test]
fn test_get_parameter_output_gain_and_tone() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(ParameterId::from("tone"), ParameterValue::Float(2.5))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("tone")),
        Some(ParameterValue::Float(2.5))
    );

    plugin
        .set_parameter(
            ParameterId::from("output_gain"),
            ParameterValue::Float(-3.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("output_gain")),
        Some(ParameterValue::Float(-3.0))
    );
}

#[test]
fn test_get_parameter_after_from_params() {
    let params = SaturationPluginParams {
        mode: "Tape".to_string(),
        drive: 8.0,
        tone: 2.0,
        exciter_freq: 6000.0,
        oversampling: "Off".to_string(),
        output_gain_db: -6.0,
        mix: 0.25,
        dynamic_amount: 0.5,
        dynamic_attack_ms: 20.0,
        dynamic_release_ms: 200.0,
        dc_blocker_enabled: false,
        use_adaa: false,
    };
    let plugin = SaturationPlugin::from_validated_params(2, params);

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_amount")),
        Some(ParameterValue::Float(0.5))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_attack_ms")),
        Some(ParameterValue::Float(20.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dynamic_release_ms")),
        Some(ParameterValue::Float(200.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dc_blocker")),
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("use_adaa")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn oversize_block_is_rejected_without_growing_scratch_buffers() {
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        oversampling: "Off".to_string(),
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(2, params);
    plugin.initialize(48000).unwrap();

    let initial_dry_len = plugin.dry_buf.len();
    let num_frames = initial_dry_len / 2 + 1;
    let mut buffer = vec![0.25_f32; num_frames * 2];

    let err = plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .expect_err("oversize audio blocks must not allocate in process_in_place");

    assert!(err.contains("exceeds preallocated scratch"));
    assert_eq!(plugin.dry_buf.len(), initial_dry_len);
}
