// Integration tests for sotf-plugin-transient-shaper exercising the public InPlacePlugin trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePluginAdapter;
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_transient_shaper::{TransientShaperPlugin, TransientShaperPluginParams};

#[test]
fn integration_plugin_info_and_channels() {
    let plugin = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::new(2));
    assert_eq!(plugin.channels(), 2);
    assert_eq!(plugin.input_channels(), 2);
    let info = plugin.info();
    assert_eq!(info.name, "TransientShaper");
}

#[test]
fn integration_default_parameters() {
    let plugin = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::new(1));
    let params = plugin.parameters();
    assert_eq!(params.len(), 5);

    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"attack"));
    assert!(ids.contains(&"sustain"));
    assert!(ids.contains(&"sensitivity"));
    assert!(ids.contains(&"output_gain"));
    assert!(ids.contains(&"mix"));
}

#[test]
fn integration_parameter_roundtrip_and_validation() {
    let mut plugin = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::new(1));
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(50.0))
        .unwrap();
    let v = plugin
        .get_parameter(&ParameterId::from("attack"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!((v - 50.0).abs() < 1e-4);

    plugin
        .set_parameter(ParameterId::from("sustain"), ParameterValue::Float(-25.0))
        .unwrap();
    let v = plugin
        .get_parameter(&ParameterId::from("sustain"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!((v - (-25.0)).abs() < 1e-4);

    plugin
        .set_parameter(ParameterId::from("sensitivity"), ParameterValue::Float(6.0))
        .unwrap();
    let v = plugin
        .get_parameter(&ParameterId::from("sensitivity"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!((v - 6.0).abs() < 1e-4);

    plugin
        .set_parameter(
            ParameterId::from("output_gain"),
            ParameterValue::Float(-6.0),
        )
        .unwrap();
    let v = plugin
        .get_parameter(&ParameterId::from("output_gain"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!((v - (-6.0)).abs() < 1e-4);

    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    let v = plugin
        .get_parameter(&ParameterId::from("mix"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!((v - 0.5).abs() < 1e-4);

    // Unknown parameter.
    let res = plugin.set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(0.0));
    assert!(res.is_err());

    // Out-of-range value.
    let res = plugin.set_parameter(ParameterId::from("attack"), ParameterValue::Float(200.0));
    assert!(res.is_err());

    // NaN.
    let res = plugin.set_parameter(ParameterId::from("attack"), ParameterValue::Float(f32::NAN));
    assert!(res.is_err());

    // Type mismatch.
    let res = plugin.set_parameter(
        ParameterId::from("attack"),
        ParameterValue::String("hello".into()),
    );
    assert!(res.is_err());
}

#[test]
fn integration_from_params_applies_initial_state() {
    let plugin = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
        2,
        TransientShaperPluginParams {
            attack: 100.0,
            sustain: -100.0,
            sensitivity_db: 12.0,
            output_gain_db: 6.0,
            mix: 0.0,
        },
    ));
    assert_eq!(plugin.channels(), 2);
    let mix = plugin
        .get_parameter(&ParameterId::from("mix"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!((mix - 0.0).abs() < 1e-6);
}

#[test]
fn integration_bypass_preserves_input_after_warmup() {
    let mut plugin =
        ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: 0.0,
                sustain: 0.0,
                sensitivity_db: 0.0,
                output_gain_db: 0.0,
                mix: 0.0,
            },
        ));
    plugin.initialize(48000).unwrap();

    // Warm up the mix smoother so it converges to the dry target.
    let mut warmup = vec![0.0f32; 4800];
    let ctx_warm = ProcessContext::new(48000, warmup.len());
    plugin.process_in_place(&mut warmup, &ctx_warm).unwrap();

    let sample = 0.3f32;
    let frames = 128usize;
    let mut buf = vec![sample; frames];
    let ctx = ProcessContext::new(48000, frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let max_error = buf
        .iter()
        .map(|o| (o - sample).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-4,
        "bypassed plugin should preserve input: max_error={}",
        max_error
    );
}

#[test]
fn integration_output_gain_changes_level() {
    let mut p_low =
        ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: 0.0,
                sustain: 0.0,
                sensitivity_db: -12.0,
                output_gain_db: 0.0,
                mix: 1.0,
            },
        ));
    p_low.initialize(48000).unwrap();

    let mut p_high =
        ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: 0.0,
                sustain: 0.0,
                sensitivity_db: -12.0,
                output_gain_db: 6.0,
                mix: 1.0,
            },
        ));
    p_high.initialize(48000).unwrap();

    // Warm up smoothers.
    let mut warm_low = vec![0.2f32; 4800];
    let mut warm_high = vec![0.2f32; 4800];
    let ctx_warm = ProcessContext::new(48000, warm_low.len());
    p_low.process_in_place(&mut warm_low, &ctx_warm).unwrap();
    p_high.process_in_place(&mut warm_high, &ctx_warm).unwrap();

    let frames = 128usize;
    let mut buf_low = vec![0.2f32; frames];
    let mut buf_high = vec![0.2f32; frames];
    let ctx = ProcessContext::new(48000, frames);
    p_low.process_in_place(&mut buf_low, &ctx).unwrap();
    p_high.process_in_place(&mut buf_high, &ctx).unwrap();

    let rms_low = (buf_low.iter().map(|x| x * x).sum::<f32>() / frames as f32).sqrt();
    let rms_high = (buf_high.iter().map(|x| x * x).sum::<f32>() / frames as f32).sqrt();

    let expected_ratio = 10.0f32.powf(6.0 / 20.0);
    let actual_ratio = rms_high / (rms_low + 1e-12);
    assert!(
        (actual_ratio - expected_ratio).abs() < 0.01,
        "output gain mismatch: expected {} got {}",
        expected_ratio,
        actual_ratio
    );
}

#[test]
fn integration_reset_clears_envelope_state() {
    let mut plugin = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::new(1));
    plugin.initialize(48000).unwrap();

    // Drive the envelope detectors with a loud signal.
    let mut buffer = vec![0.8f32; 4800];
    let ctx = ProcessContext::new(48000, buffer.len());
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    plugin.reset();

    // After reset, silence should remain silence (no leaking envelope state).
    let mut silence = vec![0.0f32; 4800];
    plugin.process_in_place(&mut silence, &ctx).unwrap();
    assert!(silence.iter().all(|s| *s == 0.0));
}

#[test]
fn integration_process_finite_output_with_extreme_parameters() {
    let mut plugin =
        ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: 100.0,
                sustain: -100.0,
                sensitivity_db: 12.0,
                output_gain_db: 12.0,
                mix: 1.0,
            },
        ));
    plugin.initialize(48000).unwrap();

    let frames = 256usize;
    let mut buf: Vec<f32> = (0..frames)
        .map(|i| (i as f32 / frames as f32) * 0.5)
        .collect();
    let ctx = ProcessContext::new(48000, frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}
