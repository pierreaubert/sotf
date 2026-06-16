// Property-based tests for sotf-plugin-gain.
//
// Invariants exercised:
//   - Finite output for all finite inputs and parameter values.
//   - set_parameter / get_parameter round-trip.
//   - Unity gain passes signal through unchanged.
//   - Increasing gain increases output for a positive input.

use proptest::prelude::*;
use sotf_host::{ParametricPlugin, ParameterId, ParameterValue, ProcessContext};
use sotf_plugin_gain::GainPlugin;

proptest! {
    #[test]
    fn finite_output(
        input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 64]),
        gain_db in -60.0f32..20.0f32,
    ) {
        let mut plugin = GainPlugin::new(2, gain_db);
        plugin.plugin_initialize(48_000).unwrap();

        let mut buffer = vec![0.0f32; input.len()];
        let context = ProcessContext::new(48_000, 32);
        plugin.process(&input, &mut buffer, &context).unwrap();

        prop_assert!(
            buffer.iter().all(|x| x.is_finite()),
            "process produced non-finite output"
        );
    }

    #[test]
    fn parameter_round_trip_gain(gain_db in -60.0f32..20.0f32) {
        let mut plugin = GainPlugin::new(2, 0.0);
        plugin
            .parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(gain_db))
            .unwrap();
        let got = plugin.parametric_get_parameter(&ParameterId::from("gain_db"));
        prop_assert_eq!(got, Some(ParameterValue::Float(gain_db)));
    }

    #[test]
    fn parameter_round_trip_smoothing(smoothing_ms in 0.0f32..100.0f32) {
        let mut plugin = GainPlugin::new(2, 0.0);
        plugin
            .parametric_set_parameter(
                ParameterId::from("smoothing_ms"),
                ParameterValue::Float(smoothing_ms),
            )
            .unwrap();
        let got = plugin.parametric_get_parameter(&ParameterId::from("smoothing_ms"));
        prop_assert_eq!(got, Some(ParameterValue::Float(smoothing_ms)));
    }

    #[test]
    fn parameter_round_trip_channel_gain(
        ch in 0usize..2,
        gain_db in -60.0f32..20.0f32,
    ) {
        let mut plugin = GainPlugin::new(2, 0.0);
        plugin.set_channel_gain_db(ch, gain_db).unwrap();
        let id = format!("gain_db_{}", ch);
        let got = plugin.parametric_get_parameter(&ParameterId::from(id.as_str()));
        prop_assert_eq!(got, Some(ParameterValue::Float(gain_db)));
    }

    #[test]
    fn unity_gain_passthrough(
        input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 32]),
    ) {
        let mut plugin = GainPlugin::with_smoothing(2, 0.0, 0.0);
        plugin.plugin_initialize(48_000).unwrap();

        let mut buffer = vec![0.0f32; input.len()];
        let context = ProcessContext::new(48_000, 16);
        plugin.process(&input, &mut buffer, &context).unwrap();

        for (inp, out) in input.iter().zip(buffer.iter()) {
            prop_assert!(
                (out - inp).abs() < 1e-5,
                "unity gain should be passthrough: got {} expected {}",
                out,
                inp
            );
        }
    }

    #[test]
    fn monotonic_gain_increases_output(
        gain_db in -20.0f32..10.0f32,
        delta in 0.1f32..10.0f32,
    ) {
        let gain1 = gain_db;
        let gain2 = (gain_db + delta).min(20.0);

        let input = vec![0.5f32; 32];

        let mut plugin1 = GainPlugin::with_smoothing(2, gain1, 0.0);
        plugin1.plugin_initialize(48_000).unwrap();
        let mut buf1 = vec![0.0f32; input.len()];
        plugin1
            .process(&input, &mut buf1, &ProcessContext::new(48_000, 16))
            .unwrap();

        let mut plugin2 = GainPlugin::with_smoothing(2, gain2, 0.0);
        plugin2.plugin_initialize(48_000).unwrap();
        let mut buf2 = vec![0.0f32; input.len()];
        plugin2
            .process(&input, &mut buf2, &ProcessContext::new(48_000, 16))
            .unwrap();

        prop_assert!(
            buf2[0] > buf1[0],
            "higher gain should produce larger output for positive input: {} vs {}",
            buf2[0],
            buf1[0]
        );
    }
}
