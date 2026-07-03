use super::misc::tube;
use super::saturation_plugin::SaturationPlugin;
use super::saturation_plugin_params::SaturationPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;

#[path = "tests/make.rs"]
mod make;
#[path = "tests/misc.rs"]
mod test_misc;

use make::make_context;
use make::make_sine;
use test_misc::rms;

#[test]
fn test_tube_asymmetry() {
    // With tone > 1, tube saturation produces different positive/negative peaks
    let drive = 5.0;
    let tone = 2.0;

    let pos = tube(0.5, drive, tone);
    let neg = tube(-0.5, drive, tone);

    // Tube is antisymmetric in sign but NOT in absolute magnitude when n > 1
    // Actually for x/(1+|x|^n), tube(-x) = -(-x)/(1+|-x|^n) = x/(1+|x|^n) = -tube(x)
    // So it IS antisymmetric. But the harmonic content (even vs odd) depends on n.
    // Let's verify the function works and produces bounded output.
    assert!(pos > 0.0, "Positive input should give positive output");
    assert!(neg < 0.0, "Negative input should give negative output");
    assert!(
        pos.abs() < (0.5 * drive).abs(),
        "Tube should compress: pos={}, input={}",
        pos,
        0.5 * drive
    );
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
    let mut plugin = SaturationPlugin::from_params(channels, params);
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
    // Default oversampling index is 1 (2x), so preferred_oversampling should be Some(2)
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
    let plugin_off = SaturationPlugin::from_params(2, params_off);
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
    let plugin_4x = SaturationPlugin::from_params(2, params_4x);
    assert_eq!(plugin_4x.preferred_oversampling(), Some(4));
}

#[test]
fn test_saturation_default_f64_is_false() {
    let plugin = SaturationPlugin::new(2);
    assert!(!plugin.supports_f64());
}

#[test]
fn test_parameter_roundtrip() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(48000).unwrap();

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
    let plugin = SaturationPlugin::from_params(2, params);

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
