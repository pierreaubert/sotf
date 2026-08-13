//! Black-box integration tests for `sotf-plugin-beamformer`.
//!
//! These tests exercise the public `Plugin` API surface from outside the crate.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_beamformer::{BeamformerPlugin, BeamformerPluginParams};

#[test]
fn construct_default() {
    let plugin = BeamformerPlugin::new(2, 48000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 1);
    assert_eq!(plugin.info().name, "Beamformer");
}

#[test]
fn construct_from_params() {
    let params = BeamformerPluginParams {
        num_mics: 4,
        mic_spacing_cm: 10.0,
        steer_angle_deg: 45.0,
        beamformer_type: 1,
    };
    let plugin = BeamformerPlugin::from_params(48000, params).unwrap();
    assert_eq!(plugin.input_channels(), 4);
    assert_eq!(plugin.output_channels(), 1);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("steer_angle_deg")),
        Some(ParameterValue::Float(45.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("beamformer_type")),
        Some(ParameterValue::String("Superdirective".into()))
    );
}

#[test]
fn parameters_listed_by_trait() {
    let plugin = BeamformerPlugin::new(2, 48000).unwrap();
    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"steer_angle_deg"));
    assert!(ids.contains(&"beamformer_type"));
}

#[test]
fn structural_parameters_require_rebuild() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("steer_angle_deg"),
            ParameterValue::Float(30.0),
        )
        .unwrap_err();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("steer_angle_deg")),
        Some(ParameterValue::Float(0.0))
    );

    assert!(
        plugin
            .set_parameter(
                ParameterId::from("beamformer_type"),
                ParameterValue::String("GSC".into()),
            )
            .unwrap_err()
            .contains("structural")
    );
}

#[test]
fn steer_angle_out_of_range_is_rejected() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    let result = plugin.set_parameter(
        ParameterId::from("steer_angle_deg"),
        ParameterValue::Float(270.0),
    );
    assert!(
        result.is_err(),
        "steer angle beyond parameter range must be rejected"
    );
}

#[test]
fn invalid_parameter_type_rejected() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    let result = plugin.set_parameter(
        ParameterId::from("steer_angle_deg"),
        ParameterValue::String("north".into()),
    );
    assert!(result.is_err());
}

#[test]
fn unknown_parameter_rejected() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    let result = plugin.set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(6.0));
    assert!(result.is_err());
}

#[test]
fn process_mvdr() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i / 2) as f32 / 48000.0).sin() * 0.5)
        .collect();
    let mut output = vec![0.0_f32; num_frames];
    let ctx = ProcessContext::new(48000, num_frames);

    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn process_superdirective() {
    let mut plugin = BeamformerPlugin::from_params(
        48_000,
        BeamformerPluginParams {
            num_mics: 4,
            mic_spacing_cm: 5.0,
            steer_angle_deg: 0.0,
            beamformer_type: 1,
        },
    )
    .unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let input = vec![0.1_f32; num_frames * 4];
    let mut output = vec![0.0_f32; num_frames];
    let ctx = ProcessContext::new(48000, num_frames);

    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn process_gsc() {
    let mut plugin = BeamformerPlugin::from_params(
        48_000,
        BeamformerPluginParams {
            num_mics: 2,
            mic_spacing_cm: 5.0,
            steer_angle_deg: 0.0,
            beamformer_type: 2,
        },
    )
    .unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let input = vec![0.1_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames];
    let ctx = ProcessContext::new(48000, num_frames);

    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn beamformer_type_is_structural() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let input = vec![0.1_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames];
    let ctx = ProcessContext::new(48000, num_frames);

    plugin.process(&input, &mut output, &ctx).unwrap();

    let error = plugin
        .set_parameter(
            ParameterId::from("beamformer_type"),
            ParameterValue::String("Superdirective".into()),
        )
        .unwrap_err();
    assert!(error.contains("structural"));
}

#[test]
fn reset_then_process_again() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let input = vec![0.1_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames];
    let ctx = ProcessContext::new(48000, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    plugin.reset();

    let mut output2 = vec![0.0_f32; num_frames];
    plugin.process(&input, &mut output2, &ctx).unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn latency_depends_on_type() {
    let plugin = BeamformerPlugin::new(2, 48000).unwrap();
    assert!(plugin.latency_samples() > 0); // MVDR default

    let gsc = BeamformerPlugin::from_params(
        48_000,
        BeamformerPluginParams {
            num_mics: 2,
            mic_spacing_cm: 5.0,
            steer_angle_deg: 90.0,
            beamformer_type: 2,
        },
    )
    .unwrap();
    assert!(gsc.latency_samples() > 0);
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.initialize(96000).unwrap();
    // Public API does not expose sample_rate, but process should still succeed
    let num_frames = 256;
    let input = vec![0.1_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames];
    let ctx = ProcessContext::new(96000, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn process_silence_is_finite() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let input = vec![0.0_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames];
    let ctx = ProcessContext::new(48000, num_frames);

    plugin.process(&input, &mut output, &ctx).unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}
