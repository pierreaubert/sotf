// Property-based tests for sotf-plugin-matrix.
//
// Invariants exercised:
//   - Finite output for all finite inputs and matrix coefficients.
//   - set_parameter / get_parameter round-trip for gain and phase_invert.
//   - Identity matrix passes signal through unchanged.
//   - Increasing a gain increases output for positive input (monotonicity).
//   - Phase inversion negates the output.

use proptest::prelude::*;
use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_matrix::MatrixPlugin;

fn matrix_strategy() -> impl Strategy<Value = (usize, usize, Vec<f32>)> {
    (1usize..5, 1usize..5).prop_flat_map(|(in_ch, out_ch)| {
        let len = in_ch * out_ch;
        prop::collection::vec(-1.0f32..1.0f32, len..=len)
            .prop_map(move |matrix| (in_ch, out_ch, matrix))
    })
}

fn last_output_frame(plugin: &mut MatrixPlugin, channels: usize, frames: usize) -> Vec<f32> {
    let context = ProcessContext::new(48_000, frames);
    let input = vec![1.0f32; frames * plugin.input_channels()];
    let mut output = vec![0.0f32; frames * plugin.output_channels()];
    plugin.process(&input, &mut output, &context).unwrap();
    output[output.len() - channels..].to_vec()
}

#[test]
fn finite_output() {
    proptest!(ProptestConfig::with_cases(100), |(args in matrix_strategy())| {
        let (in_ch, out_ch, matrix) = args;
        let mut plugin = MatrixPlugin::with_matrix(in_ch, out_ch, matrix).unwrap();
        plugin.initialize(48_000).unwrap();

        let frames = 16;
        let input = vec![0.5f32; frames * in_ch];
        let mut output = vec![0.0f32; frames * out_ch];
        let context = ProcessContext::new(48_000, frames);
        plugin.process(&input, &mut output, &context).unwrap();

        prop_assert!(
            output.iter().all(|x| x.is_finite()),
            "process produced non-finite output"
        );
    });
}

#[test]
fn parameter_round_trip_gain() {
    proptest!(ProptestConfig::with_cases(100), |(gain in -144.0f32..24.0f32)| {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin
            .set_parameter(ParameterId::from("gain_0_0"), ParameterValue::Float(gain))
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("gain_0_0"));
        prop_assert_eq!(got, Some(ParameterValue::Float(gain)));
    });
}

#[test]
fn parameter_round_trip_phase_invert() {
    proptest!(ProptestConfig::with_cases(100), |(invert in any::<bool>())| {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin
            .set_parameter(
                ParameterId::from("phase_invert_0_0"),
                ParameterValue::Bool(invert),
            )
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("phase_invert_0_0"));
        prop_assert_eq!(got, Some(ParameterValue::Bool(invert)));
    });
}

#[test]
fn identity_passthrough() {
    proptest!(ProptestConfig::with_cases(100), |(ch in 1usize..5)| {
        let mut plugin = MatrixPlugin::new(ch, ch);
        plugin.initialize(48_000).unwrap();

        let last = last_output_frame(&mut plugin, ch, 1_024);
        for (i, v) in last.iter().enumerate() {
            prop_assert!(
                (v - 1.0).abs() < 1e-3,
                "identity passthrough failed on channel {}: got {}",
                i,
                v
            );
        }
    });
}

#[test]
fn monotonic_gain_increases_output() {
    proptest!(
        ProptestConfig::with_cases(100),
        |(gain1 in -1.0f32..1.0f32, delta in 0.01f32..1.0f32)| {
            let gain2 = (gain1 + delta).min(24.0);

            let mut plugin1 = MatrixPlugin::with_matrix(1, 1, vec![gain1]).unwrap();
            plugin1.initialize(48_000).unwrap();
            let out1 = last_output_frame(&mut plugin1, 1, 1_024);

            let mut plugin2 = MatrixPlugin::with_matrix(1, 1, vec![gain2]).unwrap();
            plugin2.initialize(48_000).unwrap();
            let out2 = last_output_frame(&mut plugin2, 1, 1_024);

            prop_assert!(
                out2[0] > out1[0],
                "higher gain should increase output for positive input: {} vs {}",
                out2[0],
                out1[0]
            );
        }
    );
}

#[test]
fn phase_invert_negates_output() {
    let mut plugin = MatrixPlugin::new(1, 1);
    plugin.set_phase_invert(0, 0, true).unwrap();
    plugin.initialize(48_000).unwrap();

    let last = last_output_frame(&mut plugin, 1, 1_024);
    assert!(
        (last[0] - (-1.0)).abs() < 1e-3,
        "phase invert should negate output, got {}",
        last[0]
    );
}
