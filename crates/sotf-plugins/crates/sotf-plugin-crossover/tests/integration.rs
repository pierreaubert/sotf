//! Integration tests for sotf-plugin-crossover exercising the public `Plugin` trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginCostClass, ProcessContext};
use sotf_plugin_crossover::{CrossoverPlugin, CrossoverPluginParams, PerChannelOpMode};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

#[test]
fn info_is_reported() {
    let plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
    let info = plugin.info();
    assert_eq!(info.name, "Crossover");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(info.author, "SotF");
    assert!(!info.description.is_empty());
}

#[test]
fn lr24_lowpass_happy_path() {
    let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    plugin.initialize(SR).unwrap();

    let input = vec![0.5f32; 64 * 2];
    let mut output = vec![0.0f32; 64 * 2];
    let frames = plugin.process(&input, &mut output, &ctx(64)).unwrap();
    assert_eq!(frames, 64);
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn lr24_highpass_reduces_dc() {
    let mut plugin = CrossoverPlugin::new(1, "LR24", 500.0, "high").unwrap();
    plugin.initialize(SR).unwrap();

    let input = vec![0.8f32; 256];
    let mut output = vec![0.0f32; 256];
    plugin.process(&input, &mut output, &ctx(256)).unwrap();

    // LR24 highpass should reject DC after settling.
    let tail = output.iter().skip(200).map(|s| s.abs()).sum::<f32>() / 56.0;
    assert!(
        tail < 0.05,
        "DC should be attenuated by highpass: tail={tail}"
    );
}

#[test]
fn both_mode_doubles_output_channels() {
    let plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "both").unwrap();
    assert_eq!(plugin.output_channels(), 4);
}

#[test]
fn multiway_three_way_output_channels() {
    let plugin = CrossoverPlugin::new_multiway(2, "LR24", 500.0, "both", &[2000.0]).unwrap();
    // 2 input channels * 3 bands
    assert_eq!(plugin.output_channels(), 6);
    let mut plugin = plugin;
    plugin.initialize(SR).unwrap();

    let input = vec![0.3f32; 128 * 2];
    let mut output = vec![0.0f32; 128 * 6];
    plugin.process(&input, &mut output, &ctx(128)).unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn linear_phase_has_latency() {
    let plugin = CrossoverPlugin::new(2, "FIR", 1000.0, "low").unwrap();
    assert!(
        plugin.latency_samples() > 0,
        "linear-phase crossover must report latency"
    );
}

#[test]
fn frequency_set_get_roundtrip() {
    let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    plugin.initialize(SR).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(1234.5),
        )
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("frequency"))
        .and_then(|v| v.as_float())
        .unwrap();
    assert!((got - 1234.5).abs() < 1e-3);
}

#[test]
fn mode_set_get_roundtrip() {
    let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    plugin
        .set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("highpass".into()),
        )
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("mode"))
        .and_then(|v| v.as_string().map(|s| s.to_owned()))
        .unwrap();
    assert_eq!(got, "highpass");
}

#[test]
fn extra_frequency_roundtrip() {
    let mut plugin = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[2000.0]).unwrap();
    plugin.initialize(SR).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(1500.0),
        )
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("frequency_2"))
        .and_then(|v| v.as_float())
        .unwrap();
    assert!((got - 1500.0).abs() < 1e-3);
}

#[test]
fn fir_taps_live_update_requires_graph_rebuild() {
    let mut plugin = CrossoverPlugin::new(1, "FIR", 1000.0, "low").unwrap();
    plugin.initialize(SR).unwrap();

    let result = plugin.set_parameter(ParameterId::from("fir_taps"), ParameterValue::Int(101));
    assert!(
        result.is_err(),
        "changing FIR length after initialization must require a graph rebuild"
    );
    let got = plugin
        .get_parameter(&ParameterId::from("fir_taps"))
        .and_then(|v| v.as_int())
        .unwrap();
    assert_eq!(got, 1025);
}

#[test]
fn fir_frequency_live_update_requires_graph_rebuild() {
    let mut plugin = CrossoverPlugin::new(1, "FIR", 1000.0, "low").unwrap();
    plugin.initialize(SR).unwrap();

    let result = plugin.set_parameter(
        ParameterId::from("frequency"),
        ParameterValue::Float(2000.0),
    );
    assert!(
        result.is_err(),
        "changing FIR cutoff after initialization must require a graph rebuild"
    );
    assert_eq!(
        plugin
            .get_parameter(&ParameterId::from("frequency"))
            .and_then(|v| v.as_float()),
        Some(1000.0)
    );
}

#[test]
fn per_channel_live_cutoff_rejects_values_above_current_nyquist() {
    let mut plugin = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![1000.0],
        vec![sotf_plugin_crossover::PerChannelOpMode::Lowpass],
    )
    .unwrap();
    plugin.initialize(32_000).unwrap();

    let result = plugin.set_parameter(
        ParameterId::from("channel_frequency_0"),
        ParameterValue::Float(16_000.0),
    );
    assert!(
        result.is_err(),
        "a live per-channel cutoff at/above the current Nyquist guard must be rejected"
    );
    assert_eq!(
        plugin
            .get_parameter(&ParameterId::from("channel_frequency_0"))
            .and_then(|v| v.as_float()),
        Some(1000.0)
    );
}

#[test]
fn from_params_happy_path() {
    let params = CrossoverPluginParams {
        crossover_type: "LR24".into(),
        frequency: 800.0,
        output: "both".into(),
        extra_frequencies: vec![2000.0],
        fir_taps: None,
        channel_frequencies_hz: vec![],
        channel_modes: vec![],
    };
    let mut plugin = CrossoverPlugin::from_params(2, &params).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 6);
    plugin.initialize(SR).unwrap();

    let input = vec![0.1f32; 64 * 2];
    let mut output = vec![0.0f32; 64 * 6];
    plugin.process(&input, &mut output, &ctx(64)).unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn per_channel_mode_happy_path() {
    let mut plugin = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![200.0, 4000.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    assert!(plugin.is_per_channel());
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
    plugin.initialize(SR).unwrap();

    let input = vec![0.5f32, -0.5f32, 0.5f32, -0.5f32];
    let mut output = vec![0.0f32; 4];
    plugin.process(&input, &mut output, &ctx(2)).unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn per_channel_rejects_global_frequency() {
    let mut plugin = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![200.0, 4000.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("frequency"), ParameterValue::Float(500.0))
        .unwrap_err();
    assert!(err.contains("per-channel mode"), "unexpected error: {err}");
}

#[test]
fn per_channel_frequency_roundtrip() {
    let mut plugin = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![200.0, 4000.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("channel_frequency_1"),
            ParameterValue::Float(3500.0),
        )
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("channel_frequency_1"))
        .and_then(|v| v.as_float())
        .unwrap();
    assert!((got - 3500.0).abs() < 1e-3);
}

#[test]
fn initialized_per_channel_controls_require_graph_rebuild() {
    let mut plugin =
        CrossoverPlugin::new_per_channel("LR24", vec![200.0], vec![PerChannelOpMode::Lowpass])
            .unwrap();
    plugin.initialize(SR).unwrap();
    for (id, value) in [
        ("channel_frequency_0", ParameterValue::Float(350.0)),
        ("channel_mode_0", ParameterValue::String("mute".into())),
    ] {
        let error = plugin
            .set_parameter(ParameterId::from(id), value)
            .unwrap_err();
        assert!(error.contains("rebuild"), "unexpected error: {error}");
    }
}

#[test]
fn multiway_frequency_crossing_is_rejected_without_rebinding_ids() {
    let mut plugin = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[2_000.0]).unwrap();
    plugin.initialize(SR).unwrap();
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("frequency"),
                ParameterValue::Float(3_000.0),
            )
            .is_err()
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("frequency")),
        Some(ParameterValue::Float(500.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("frequency_2")),
        Some(ParameterValue::Float(2_000.0))
    );
}

fn render_partitioned(partitions: &[usize]) -> Vec<f32> {
    let mut plugin = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[2_000.0]).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("frequency"), ParameterValue::Float(800.0))
        .unwrap();
    let total: usize = partitions.iter().sum();
    let input: Vec<f32> = (0..total)
        .map(|frame| (frame as f32 * 0.071).sin() * 0.25)
        .collect();
    let mut output = Vec::with_capacity(total * 3);
    let mut offset = 0;
    for &frames in partitions {
        let mut block = vec![0.0; frames * 3];
        plugin
            .process(&input[offset..offset + frames], &mut block, &ctx(frames))
            .unwrap();
        output.extend(block);
        offset += frames;
    }
    output
}

#[test]
fn smoothed_multiway_updates_are_callback_partition_invariant() {
    let contiguous = render_partitioned(&[257]);
    let partitioned = render_partitioned(&[3, 1, 17, 29, 64, 7, 83, 53]);
    assert_eq!(contiguous.len(), partitioned.len());
    let max_error = contiguous
        .iter()
        .zip(&partitioned)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_error < 1.0e-6, "partition error {max_error}");
}

#[test]
fn compile_metadata_distinguishes_iir_and_fir_cost() {
    let lr = CrossoverPlugin::new_multiway(2, "LR24", 500.0, "both", &[2_000.0]).unwrap();
    let fir = CrossoverPlugin::new_multiway(2, "FIR", 500.0, "both", &[2_000.0]).unwrap();
    assert_eq!(lr.compile_metadata().cost_class, PluginCostClass::Iir);
    assert_eq!(
        fir.compile_metadata().cost_class,
        PluginCostClass::Convolution
    );
    assert!(!lr.compile_metadata().boundary);
    assert!(fir.compile_metadata().boundary);
}

#[test]
fn lr_multiway_recombination_is_allpass_at_every_test_frequency() {
    for frequency_hz in [80.0_f32, 300.0, 700.0, 1_500.0, 3_500.0, 9_000.0] {
        let mut plugin =
            CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[2_000.0, 6_000.0]).unwrap();
        plugin.initialize(SR).unwrap();
        let frames = 16_384;
        let input: Vec<f32> = (0..frames)
            .map(|frame| {
                (std::f32::consts::TAU * frequency_hz * frame as f32 / SR as f32).sin() * 0.25
            })
            .collect();
        let mut split = vec![0.0; frames * 4];
        plugin.process(&input, &mut split, &ctx(frames)).unwrap();
        let start = frames / 2;
        let input_rms =
            (input[start..].iter().map(|x| x * x).sum::<f32>() / (frames - start) as f32).sqrt();
        let output_rms = (split[start * 4..]
            .chunks_exact(4)
            .map(|bands| bands.iter().sum::<f32>())
            .map(|x| x * x)
            .sum::<f32>()
            / (frames - start) as f32)
            .sqrt();
        let ratio = output_rms / input_rms;
        assert!(
            (ratio - 1.0).abs() < 0.01,
            "{frequency_hz} Hz recombination ratio {ratio}"
        );
    }
}

#[test]
fn per_channel_mode_roundtrip() {
    let mut plugin = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![200.0, 4000.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("channel_mode_0"),
            ParameterValue::String("mute".into()),
        )
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("channel_mode_0"))
        .and_then(|v| v.as_string().map(|s| s.to_owned()))
        .unwrap();
    assert_eq!(got, "mute");
}

#[test]
fn reset_clears_state() {
    let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    plugin.initialize(SR).unwrap();

    let mut output = vec![0.0f32; 64];
    plugin
        .process(&[0.5f32; 64], &mut output, &ctx(64))
        .unwrap();
    plugin.reset();

    let mut output2 = vec![0.0f32; 64];
    plugin
        .process(&[0.5f32; 64], &mut output2, &ctx(64))
        .unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn unknown_parameter_errors() {
    let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn invalid_crossover_type_is_rejected() {
    let result = CrossoverPlugin::new(1, "bogus", 1000.0, "low");
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        err.contains("Unsupported crossover type"),
        "unexpected error: {err}"
    );
}

#[test]
fn invalid_output_mode_is_rejected() {
    let result = CrossoverPlugin::new(1, "LR24", 1000.0, "sideways");
    assert!(result.is_err());
}

#[test]
fn get_unknown_parameter_returns_none() {
    let plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    assert!(plugin.get_parameter(&ParameterId::from("nope")).is_none());
}

#[test]
fn parameter_list_contains_frequency_and_mode() {
    let plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    let ids: Vec<_> = plugin
        .parameters()
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    assert!(ids.contains(&"frequency".to_string()));
    assert!(ids.contains(&"mode".to_string()));
}

#[test]
fn runtime_schema_matches_compiled_topology() {
    let regular = CrossoverPlugin::new_multiway(1, "FIR", 500.0, "both", &[2_000.0]).unwrap();
    let ids: Vec<String> = regular
        .parameters()
        .into_iter()
        .map(|p| p.id.0.to_string())
        .collect();
    assert_eq!(
        ids,
        ["type", "frequency", "mode", "frequency_2", "fir_taps"]
    );

    let per_channel = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![200.0, 4000.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    let ids: Vec<String> = per_channel
        .parameters()
        .into_iter()
        .map(|p| p.id.0.to_string())
        .collect();
    assert_eq!(
        ids,
        [
            "type",
            "channel_frequency_0",
            "channel_mode_0",
            "channel_frequency_1",
            "channel_mode_1",
        ]
    );
}
