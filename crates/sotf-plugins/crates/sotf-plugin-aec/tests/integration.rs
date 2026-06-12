//! Integration tests for the SOTF Acoustic Echo Cancellation (AEC) plugin.
//!
//! These tests exercise the public `Plugin` API as a black box.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_aec::{AecPlugin, AecPluginParams};

fn ctx(sample_rate: u32, num_frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(sample_rate, num_frames)
}

#[test]
fn info_and_channels() {
    let plugin = AecPlugin::new(48_000);

    let info = plugin.info();
    assert_eq!(info.name, "AEC");
    assert_eq!(info.version, "1.0.0");
    assert_eq!(info.author, "Sotf");
    assert!(info.description.contains("Acoustic Echo Cancellation"));

    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 1);
    assert_eq!(plugin.latency_samples(), 256);

    let params = plugin.parameters();
    assert!(!params.is_empty());
    assert!(
        params
            .iter()
            .any(|p| p.id == ParameterId::from("echo_tail_ms"))
    );
}

#[test]
fn new_and_from_params() {
    let default_plugin = AecPlugin::new(48_000);
    assert_eq!(
        default_plugin.get_parameter(&ParameterId::from("echo_tail_ms")),
        Some(ParameterValue::Float(200.0))
    );

    let params = AecPluginParams {
        echo_tail_ms: 300.0,
        step_size: 0.7,
        post_filter_enabled: false,
    };
    let plugin = AecPlugin::from_params(48_000, params);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("echo_tail_ms")),
        Some(ParameterValue::Float(300.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("step_size")),
        Some(ParameterValue::Float(0.7))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("post_filter_enabled")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn parameter_roundtrip_and_validation() {
    let mut plugin = AecPlugin::new(48_000);

    plugin
        .set_parameter(
            ParameterId::from("echo_tail_ms"),
            ParameterValue::Float(120.0),
        )
        .expect("valid echo tail");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("echo_tail_ms")),
        Some(ParameterValue::Float(120.0))
    );

    plugin
        .set_parameter(ParameterId::from("step_size"), ParameterValue::Float(0.25))
        .expect("valid step size");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("step_size")),
        Some(ParameterValue::Float(0.25))
    );

    plugin
        .set_parameter(
            ParameterId::from("post_filter_enabled"),
            ParameterValue::Bool(false),
        )
        .expect("valid post-filter toggle");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("post_filter_enabled")),
        Some(ParameterValue::Bool(false))
    );

    // Out-of-range value
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("echo_tail_ms"),
                ParameterValue::Float(10.0)
            )
            .is_err(),
        "echo tail below minimum should fail"
    );

    // Unknown parameter
    assert!(
        plugin
            .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
            .is_err(),
        "unknown parameter should fail"
    );

    // Type mismatch
    assert!(
        plugin
            .set_parameter(ParameterId::from("step_size"), ParameterValue::Bool(true))
            .is_err(),
        "type mismatch should fail"
    );
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = AecPlugin::new(48_000);
    plugin.initialize(44_100).expect("initialize succeeds");
    // The internal AEC is rebuilt; processing should still work at the new rate.
    let num_frames = 512;
    let input = vec![0.0f32; num_frames * 2];
    let mut output = vec![0.0f32; num_frames];
    plugin
        .process(&input, &mut output, &ctx(44_100, num_frames))
        .unwrap();
}

#[test]
fn process_silence() {
    let mut plugin = AecPlugin::new(48_000);
    let num_frames = 512;
    let input = vec![0.0f32; num_frames * plugin.input_channels()];
    let mut output = vec![0.0f32; num_frames * plugin.output_channels()];

    let frames = plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .expect("process succeeds");
    assert_eq!(frames, num_frames);

    assert!(
        output.iter().all(|s| s.is_finite()),
        "silent input must produce finite output"
    );
    assert!(
        output.iter().all(|s| s.abs() < 1e-6),
        "silent input should remain silent"
    );
}

#[test]
fn process_with_independent_mic_ref() {
    let mut plugin = AecPlugin::new(48_000);
    let num_frames = 1_024;
    let mut input = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        let mic = (i as f32 * 0.05).sin() * 0.8;
        let reference = (i as f32 * 0.07).cos() * 0.6;
        input.push(mic);
        input.push(reference);
    }
    let mut output = vec![0.0f32; num_frames];

    plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));

    // The first block is zero-filled latency; after that the cancelled output
    // should be non-zero because the mic and reference are independent.
    let latency = plugin.latency_samples();
    let tail_max = output[latency..]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        tail_max > 1e-6,
        "output after latency should contain processed audio"
    );
}

#[test]
fn reset_restarts_latency() {
    let mut plugin = AecPlugin::new(48_000);
    let num_frames = 512;
    let mut input = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        let mic = (i as f32 * 0.05).sin() * 0.8;
        input.push(mic);
        input.push(0.0);
    }
    let mut output = vec![0.0f32; num_frames];

    plugin
        .process(&input, &mut output, &ctx(48_000, num_frames))
        .unwrap();
    assert!(
        output[plugin.latency_samples()..]
            .iter()
            .any(|s| s.abs() > 1e-6)
    );

    plugin.reset();

    // Process fewer samples than the internal block size: the input accumulator
    // has been cleared, so no output should be produced yet.
    let partial = plugin.latency_samples() - 1;
    let mut partial_input = Vec::with_capacity(partial * 2);
    for i in 0..partial {
        let mic = (i as f32 * 0.05).sin() * 0.8;
        partial_input.push(mic);
        partial_input.push(0.0);
    }
    let mut output_after = vec![0.0f32; partial];
    plugin
        .process(&partial_input, &mut output_after, &ctx(48_000, partial))
        .unwrap();
    assert!(
        output_after.iter().all(|s| s.abs() < 1e-6),
        "after reset, a sub-block should still be silent (latency)"
    );
}

#[test]
fn wrong_buffer_sizes_return_error() {
    let mut plugin = AecPlugin::new(48_000);
    let num_frames = 64;
    let good_input = vec![0.0f32; num_frames * plugin.input_channels()];
    let mut good_output = vec![0.0f32; num_frames * plugin.output_channels()];
    let mut bad_output = vec![0.0f32; num_frames * plugin.output_channels() - 1];

    assert!(
        plugin
            .process(
                &good_input[..good_input.len() - 1],
                &mut bad_output,
                &ctx(48_000, num_frames)
            )
            .is_err(),
        "input size mismatch should fail"
    );
    assert!(
        plugin
            .process(&good_input, &mut bad_output, &ctx(48_000, num_frames))
            .is_err(),
        "output size mismatch should fail"
    );
    assert!(
        plugin
            .process(&good_input, &mut good_output, &ctx(48_000, num_frames))
            .is_ok(),
        "correct sizes should succeed"
    );
}

#[test]
fn post_filter_toggle() {
    for enabled in [false, true] {
        let mut plugin = AecPlugin::new(48_000);
        plugin
            .set_parameter(
                ParameterId::from("post_filter_enabled"),
                ParameterValue::Bool(enabled),
            )
            .unwrap();

        let num_frames = 512;
        let mut input = Vec::with_capacity(num_frames * 2);
        for i in 0..num_frames {
            input.push((i as f32 * 0.05).sin() * 0.5);
            input.push((i as f32 * 0.03).cos() * 0.5);
        }
        let mut output = vec![0.0f32; num_frames];
        plugin
            .process(&input, &mut output, &ctx(48_000, num_frames))
            .unwrap();
        assert!(
            output.iter().all(|s| s.is_finite()),
            "post_filter_enabled={enabled} must produce finite output"
        );
    }
}
