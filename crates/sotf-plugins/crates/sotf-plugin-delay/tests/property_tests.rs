// Property-based tests for sotf-plugin-delay.
//
// Invariants exercised:
//   - Finite output for all finite inputs and parameter values.
//   - set_parameter / get_parameter round-trip for every parameter.
//   - Dry (mix = 0) signal passes through unchanged.

use proptest::prelude::*;
use sotf_host::{InPlacePlugin, ParameterId, ParameterValue, ProcessContext};
use sotf_plugin_delay::DelayPlugin;

proptest! {
    #[test]
    fn finite_output(
        input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 64]),
        delay_ms in 0.0f32..500.0f32,
        feedback in -0.9f32..0.9f32,
        mix in 0.0f32..1.0f32,
    ) {
        let mut plugin = DelayPlugin::new(2, delay_ms, feedback, mix);
        plugin.initialize(48_000).unwrap();

        let mut buffer = input;
        let context = ProcessContext::new(48_000, 32);
        plugin.process_in_place(&mut buffer, &context).unwrap();

        prop_assert!(
            buffer.iter().all(|x| x.is_finite()),
            "process_in_place produced non-finite output"
        );
    }

    #[test]
    fn parameter_round_trip_delay_ms(delay_ms in 0.1f32..5_000.0f32) {
        let mut plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        plugin
            .set_parameter(ParameterId::from("delay_ms"), ParameterValue::Float(delay_ms))
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("delay_ms"));
        prop_assert_eq!(got, Some(ParameterValue::Float(delay_ms)));
    }

    #[test]
    fn parameter_round_trip_feedback(feedback in 0.0f32..0.95f32) {
        let mut plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        plugin
            .set_parameter(ParameterId::from("feedback"), ParameterValue::Float(feedback))
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("feedback"));
        prop_assert_eq!(got, Some(ParameterValue::Float(feedback)));
    }

    #[test]
    fn parameter_round_trip_mix(mix in 0.0f32..1.0f32) {
        let mut plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix))
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("mix"));
        prop_assert_eq!(got, Some(ParameterValue::Float(mix)));
    }

    #[test]
    fn parameter_round_trip_lfo_rate_hz(lfo_rate_hz in 0.0f32..10.0f32) {
        let mut plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        plugin
            .set_parameter(
                ParameterId::from("lfo_rate_hz"),
                ParameterValue::Float(lfo_rate_hz),
            )
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("lfo_rate_hz"));
        prop_assert_eq!(got, Some(ParameterValue::Float(lfo_rate_hz)));
    }

    #[test]
    fn parameter_round_trip_lfo_depth_ms(lfo_depth_ms in 0.0f32..5.0f32) {
        let mut plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        plugin
            .set_parameter(
                ParameterId::from("lfo_depth_ms"),
                ParameterValue::Float(lfo_depth_ms),
            )
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("lfo_depth_ms"));
        prop_assert_eq!(got, Some(ParameterValue::Float(lfo_depth_ms)));
    }

    #[test]
    fn parameter_round_trip_allpass_feedback(allpass_feedback in any::<bool>()) {
        let mut plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        plugin
            .set_parameter(
                ParameterId::from("allpass_feedback"),
                ParameterValue::Bool(allpass_feedback),
            )
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("allpass_feedback"));
        prop_assert_eq!(got, Some(ParameterValue::Bool(allpass_feedback)));
    }

    #[test]
    fn parameter_round_trip_allpass_coeff(allpass_coeff in 0.0f32..0.99f32) {
        let mut plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        plugin
            .set_parameter(
                ParameterId::from("allpass_coeff"),
                ParameterValue::Float(allpass_coeff),
            )
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from("allpass_coeff"));
        prop_assert_eq!(got, Some(ParameterValue::Float(allpass_coeff)));
    }

    #[test]
    fn dry_passthrough(input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 32])) {
        // mix = 0 and feedback = 0 => output should equal input exactly.
        let mut plugin = DelayPlugin::new(2, 100.0, 0.0, 0.0);
        plugin.initialize(48_000).unwrap();

        let mut buffer = input.clone();
        let context = ProcessContext::new(48_000, 16);
        plugin.process_in_place(&mut buffer, &context).unwrap();

        for (inp, out) in input.iter().zip(buffer.iter()) {
            prop_assert!(
                (out - inp).abs() < 1e-5,
                "dry passthrough failed: got {} expected {}",
                out,
                inp
            );
        }
    }
}
