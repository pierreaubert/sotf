// Property-based tests for sotf-plugin-delay.
//
// Invariants exercised:
//   - Finite output for all finite inputs and parameter values.
//   - set_parameter / get_parameter round-trip for every parameter.
//   - Dry (mix = 0) signal passes through unchanged.

use proptest::prelude::*;
use sotf_host::{
    ParameterId, ParameterValue, ParametricInPlacePluginAdapter, Plugin, ProcessContext,
};
use sotf_plugin_delay::DelayPlugin;

// Small, fast buffers: 64 frames stereo = 128 samples.
fn stereo_buffer_strategy() -> impl Strategy<Value = Vec<f32>> {
    (-0.9f32..0.9f32).prop_map(|v| vec![v; 128])
}

fn mono_buffer_strategy() -> impl Strategy<Value = Vec<f32>> {
    (-0.9f32..0.9f32).prop_map(|v| vec![v; 64])
}

proptest! {
    // -------------------------------------------------------------------------
    // Finite output
    // -------------------------------------------------------------------------
    #[test]
    fn process_finite_output_mono(buffer in mono_buffer_strategy()) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        let mut buf = vec![0.0f32; buffer.len()];
        adapter.process(&buffer, &mut buf, &ProcessContext::new(48000, 32)).unwrap();

        prop_assert!(buf.iter().all(|s| s.is_finite()),
            "Delay should produce finite output");
    }

    #[test]
    fn process_finite_output_stereo(buffer in stereo_buffer_strategy()) {
        let plugin = DelayPlugin::new(2, 100.0, 0.5, 0.5);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        let mut buf = vec![0.0f32; buffer.len()];
        adapter.process(&buffer, &mut buf, &ProcessContext::new(48000, 64)).unwrap();

        prop_assert!(buf.iter().all(|s| s.is_finite()),
            "Active delay should produce finite output");
    }

    // -------------------------------------------------------------------------
    // Parameter round-trip
    // -------------------------------------------------------------------------
    #[test]
    fn roundtrip_delay_ms(delay_ms in 0.1f32..5_000.0f32) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        adapter.set_parameter(ParameterId::from("delay_ms"), ParameterValue::Float(delay_ms)).unwrap();
        let got = adapter.get_parameter(&ParameterId::from("delay_ms"));

        prop_assert_eq!(got, Some(ParameterValue::Float(delay_ms)),
            "delay_ms set->get should round-trip");
    }

    #[test]
    fn roundtrip_feedback(feedback in 0.0f32..0.95f32) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        adapter.set_parameter(ParameterId::from("feedback"), ParameterValue::Float(feedback)).unwrap();
        let got = adapter.get_parameter(&ParameterId::from("feedback"));

        prop_assert_eq!(got, Some(ParameterValue::Float(feedback)),
            "feedback set->get should round-trip");
    }

    #[test]
    fn roundtrip_mix(mix in 0.0f32..1.0f32) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        adapter.set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix)).unwrap();
        let got = adapter.get_parameter(&ParameterId::from("mix"));

        prop_assert_eq!(got, Some(ParameterValue::Float(mix)),
            "mix set->get should round-trip");
    }

    #[test]
    fn roundtrip_lfo_rate_hz(lfo_rate_hz in 0.0f32..10.0f32) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        adapter.set_parameter(
            ParameterId::from("lfo_rate_hz"),
            ParameterValue::Float(lfo_rate_hz),
        ).unwrap();
        let got = adapter.get_parameter(&ParameterId::from("lfo_rate_hz"));

        prop_assert_eq!(got, Some(ParameterValue::Float(lfo_rate_hz)),
            "lfo_rate_hz set->get should round-trip");
    }

    #[test]
    fn roundtrip_lfo_depth_ms(lfo_depth_ms in 0.0f32..5.0f32) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        adapter.set_parameter(
            ParameterId::from("lfo_depth_ms"),
            ParameterValue::Float(lfo_depth_ms),
        ).unwrap();
        let got = adapter.get_parameter(&ParameterId::from("lfo_depth_ms"));

        prop_assert_eq!(got, Some(ParameterValue::Float(lfo_depth_ms)),
            "lfo_depth_ms set->get should round-trip");
    }

    #[test]
    fn roundtrip_allpass_feedback(allpass_feedback in any::<bool>()) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        adapter.set_parameter(
            ParameterId::from("allpass_feedback"),
            ParameterValue::Bool(allpass_feedback),
        ).unwrap();
        let got = adapter.get_parameter(&ParameterId::from("allpass_feedback"));

        prop_assert_eq!(got, Some(ParameterValue::Bool(allpass_feedback)),
            "allpass_feedback set->get should round-trip");
    }

    #[test]
    fn roundtrip_allpass_coeff(allpass_coeff in 0.0f32..0.99f32) {
        let plugin = DelayPlugin::new(2, 0.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48000).unwrap();

        adapter.set_parameter(
            ParameterId::from("allpass_coeff"),
            ParameterValue::Float(allpass_coeff),
        ).unwrap();
        let got = adapter.get_parameter(&ParameterId::from("allpass_coeff"));

        prop_assert_eq!(got, Some(ParameterValue::Float(allpass_coeff)),
            "allpass_coeff set->get should round-trip");
    }

    #[test]
    fn dry_passthrough(input in (-1.0f32..1.0f32).prop_map(|v| vec![v; 32])) {
        let plugin = DelayPlugin::new(2, 100.0, 0.0, 0.0);
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter.initialize(48_000).unwrap();

        let mut buffer = input.clone();
        let context = ProcessContext::new(48_000, 16);
        adapter.process(&buffer.clone(), &mut buffer, &context).unwrap();

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
