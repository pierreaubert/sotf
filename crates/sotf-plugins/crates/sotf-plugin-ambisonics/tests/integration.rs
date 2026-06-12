//! Black-box integration tests for `sotf-plugin-ambisonics`.
//!
//! These tests exercise the public `Plugin` API surface from outside the crate.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_ambisonics::{AmbisonicsDecoderConfig, AmbisonicsDecoderPlugin};

fn foa_5_1_config() -> AmbisonicsDecoderConfig {
    AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "5.1".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    }
}

#[test]
fn construct_foa_5_1() {
    let plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    assert_eq!(plugin.input_channels(), 4);
    assert_eq!(plugin.output_channels(), 6);
    assert_eq!(plugin.info().name, "AmbisonicsDecoder");
}

#[test]
fn construct_soa_7_1_4() {
    let config = AmbisonicsDecoderConfig {
        order: 2,
        target_layout: "7.1.4".to_owned(),
        max_re_weighting: false,
        dual_band: false,
    };
    let plugin = AmbisonicsDecoderPlugin::new(&config).unwrap();
    assert_eq!(plugin.input_channels(), 9);
    assert_eq!(plugin.output_channels(), 12);
}

#[test]
fn invalid_layout_returns_error() {
    let config = AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "not-a-layout".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    };
    assert!(AmbisonicsDecoderPlugin::new(&config).is_err());
}

#[test]
fn parameters_listed_by_trait() {
    let plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.0.as_str()).collect();
    assert!(ids.contains(&"order"));
    assert!(ids.contains(&"target_layout"));
    assert!(ids.contains(&"max_re_weighting"));
    assert!(ids.contains(&"dual_band"));
}

#[test]
fn parameter_get_set_roundtrip() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("order")),
        Some(ParameterValue::Int(1))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("max_re_weighting")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dual_band")),
        Some(ParameterValue::Bool(false))
    );

    plugin
        .set_parameter(
            ParameterId::from("max_re_weighting"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("max_re_weighting")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn order_change_rebuilds_channels() {
    // Use a layout with enough speakers for second-order ambisonics.
    let config = AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "7.1.4".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    };
    let mut plugin = AmbisonicsDecoderPlugin::new(&config).unwrap();
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(ParameterId::from("order"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(plugin.input_channels(), 9);
    assert_eq!(plugin.output_channels(), 12);

    // Process with the new channel count
    let num_frames = 256;
    let input = vec![0.1_f32; num_frames * 9];
    let mut output = vec![0.0_f32; num_frames * 12];
    let ctx = ProcessContext::new(48000, num_frames);
    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn layout_change_rebuilds_output_channels() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("target_layout"),
            ParameterValue::String("7.1".into()),
        )
        .unwrap();
    assert_eq!(plugin.output_channels(), 8);

    let num_frames = 128;
    let input = vec![0.1_f32; num_frames * 4];
    let mut output = vec![0.0_f32; num_frames * 8];
    let ctx = ProcessContext::new(48000, num_frames);
    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
}

#[test]
fn invalid_layout_parameter_rejected() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    let result = plugin.set_parameter(
        ParameterId::from("target_layout"),
        ParameterValue::String("does-not-exist".into()),
    );
    assert!(result.is_err());
}

#[test]
fn process_silence_produces_silence() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let input = vec![0.0_f32; num_frames * 4];
    let mut output = vec![0.0_f32; num_frames * 6];
    let ctx = ProcessContext::new(48000, num_frames);

    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    assert!(output.iter().all(|s| s.abs() < 1e-9));
}

#[test]
fn process_omni_signal_reaches_all_speakers() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 64;
    // Pure W (omnidirectional) signal
    let mut input = vec![0.0_f32; num_frames * 4];
    for frame in 0..num_frames {
        input[frame * 4] = 0.5;
    }
    let mut output = vec![0.0_f32; num_frames * 6];
    let ctx = ProcessContext::new(48000, num_frames);

    plugin.process(&input, &mut output, &ctx).unwrap();

    // Non-LFE channels should be non-zero
    let non_lfe: Vec<f32> = output.iter().skip(1).step_by(6).copied().collect();
    assert!(non_lfe.iter().any(|s| s.abs() > 1e-6));
}

#[test]
fn dual_band_toggle_via_parameter() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    plugin.initialize(48000).unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dual_band")),
        Some(ParameterValue::Bool(false))
    );

    plugin
        .set_parameter(ParameterId::from("dual_band"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dual_band")),
        Some(ParameterValue::Bool(true))
    );

    // Re-initialize so that dual-band scratch buffers are allocated.
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let input = vec![0.1_f32; num_frames * 4];
    let mut output = vec![0.0_f32; num_frames * 6];
    let ctx = ProcessContext::new(48000, num_frames);
    let frames = plugin.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(frames, num_frames);
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn buffer_size_mismatch_is_error() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    plugin.initialize(48000).unwrap();

    let ctx = ProcessContext::new(48000, 32);
    let input = vec![0.0_f32; 32 * 4 - 1];
    let mut output = vec![0.0_f32; 32 * 6];
    assert!(plugin.process(&input, &mut output, &ctx).is_err());

    let input_ok = vec![0.0_f32; 32 * 4];
    let mut output_short = vec![0.0_f32; 32 * 6 - 1];
    assert!(plugin.process(&input_ok, &mut output_short, &ctx).is_err());
}

#[test]
fn reset_then_process_again() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let input = vec![0.1_f32; num_frames * 4];
    let mut output = vec![0.0_f32; num_frames * 6];
    let ctx = ProcessContext::new(48000, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    plugin.reset();

    let mut output2 = vec![0.0_f32; num_frames * 6];
    plugin.process(&input, &mut output2, &ctx).unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn latency_is_zero() {
    let plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn channel_config_support() {
    let plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    assert!(plugin.supports_channel_config(4, 6));
    assert!(!plugin.supports_channel_config(2, 6));
    assert!(!plugin.supports_channel_config(4, 8));
}

#[test]
fn output_rate_and_frame_mapping() {
    let plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    assert_eq!(plugin.output_sample_rate(96000), 96000);
    assert_eq!(plugin.output_frames_for_input(128), 128);
}

#[test]
fn invalid_layout_parameter_rejected_by_set() {
    let mut plugin = AmbisonicsDecoderPlugin::new(&foa_5_1_config()).unwrap();
    let result = plugin.set_parameter(
        ParameterId::from("target_layout"),
        ParameterValue::String("does-not-exist".into()),
    );
    assert!(result.is_err());
}
