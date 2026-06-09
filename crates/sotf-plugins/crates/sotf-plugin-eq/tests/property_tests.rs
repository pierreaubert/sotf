// ============================================================================
// Property-Based Tests for sotf-plugin-eq
// ============================================================================

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_eq::EqPlugin;

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
        let mut plugin = EqPlugin::new(1, vec![]);
        plugin.initialize(48000).unwrap();

        let mut buf = buffer.clone();
        plugin.process_in_place(&mut buf, &ProcessContext::new(48000, 64)).unwrap();

        prop_assert!(buf.iter().all(|s| s.is_finite()),
            "Empty EQ chain should produce finite output");
    }

    #[test]
    fn process_finite_output_stereo(buffer in stereo_buffer_strategy()) {
        use math_audio_iir_fir::{Biquad, BiquadFilterType};

        let f = vec![
            Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0),
            Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.707, 3.0),
        ];
        let mut plugin = EqPlugin::new(2, f);
        plugin.initialize(48000).unwrap();

        let mut buf = buffer.clone();
        plugin.process_in_place(&mut buf, &ProcessContext::new(48000, 64)).unwrap();

        prop_assert!(buf.iter().all(|s| s.is_finite()),
            "Active EQ should produce finite output");
    }

    // -------------------------------------------------------------------------
    // Parameter round-trip
    // -------------------------------------------------------------------------
    #[test]
    fn roundtrip_oversampling_factor(factor in prop::sample::select(vec![1i32, 2, 4])) {
        let mut plugin = EqPlugin::new(2, vec![]);
        plugin.initialize(48000).unwrap();

        plugin.set_parameter(ParameterId::from("oversampling"), ParameterValue::Int(factor)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("oversampling"));

        prop_assert_eq!(got, Some(ParameterValue::Int(factor)),
            "oversampling set->get should round-trip");
    }

    #[test]
    fn roundtrip_tdf2(enabled in prop::bool::ANY) {
        let mut plugin = EqPlugin::new(2, vec![]);
        plugin.initialize(48000).unwrap();

        plugin.set_parameter(ParameterId::from("tdf2"), ParameterValue::Bool(enabled)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("tdf2"));

        prop_assert_eq!(got, Some(ParameterValue::Bool(enabled)),
            "tdf2 set->get should round-trip");
    }

    #[test]
    fn roundtrip_topology(topo in 0usize..2) {
        let mut plugin = EqPlugin::new(2, vec![]);
        plugin.initialize(48000).unwrap();

        let name = if topo == 1 { "SVF" } else { "Biquad" };
        plugin.set_parameter(ParameterId::from("topology"), ParameterValue::String(name.to_string())).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("topology"));

        prop_assert_eq!(got, Some(ParameterValue::String(name.to_string())),
            "topology set->get should round-trip");
    }

    #[test]
    fn roundtrip_band_gain(gain_db in -24.0f32..24.0f32) {
        use math_audio_iir_fir::{Biquad, BiquadFilterType};

        let f = vec![Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0)];
        let mut plugin = EqPlugin::new(1, f);
        plugin.initialize(48000).unwrap();

        plugin.set_parameter(ParameterId::from("band_0_gain"), ParameterValue::Float(gain_db)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("band_0_gain"));

        match got {
            Some(ParameterValue::Float(v)) => {
                prop_assert!((v - gain_db).abs() < 0.01,
                    "band_0_gain round-trip drift: {} -> {}", gain_db, v);
            }
            other => prop_assert!(false, "Expected Float, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // Identity / passthrough
    // -------------------------------------------------------------------------
    #[test]
    fn empty_chain_passthrough(buffer in mono_buffer_strategy()) {
        let mut plugin = EqPlugin::new(1, vec![]);
        plugin.initialize(48000).unwrap();

        let input = buffer.clone();
        let mut output = buffer;
        plugin.process_in_place(&mut output, &ProcessContext::new(48000, 64)).unwrap();

        let max_error: f32 = input.iter().zip(output.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        prop_assert!(max_error < 1e-5,
            "Empty EQ chain should be exact passthrough: max_error={}", max_error);
    }

    #[test]
    fn zero_gain_peak_approximately_passthrough(buffer in mono_buffer_strategy()) {
        use math_audio_iir_fir::{Biquad, BiquadFilterType};

        let f = vec![Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0)];
        let mut plugin = EqPlugin::new(1, f);
        plugin.initialize(48000).unwrap();
        plugin.set_parameter(ParameterId::from("auto_gain_enabled"), ParameterValue::Bool(false)).unwrap();

        let input = buffer.clone();
        let mut output = buffer;
        plugin.process_in_place(&mut output, &ProcessContext::new(48000, 64)).unwrap();

        let max_error: f32 = input.iter().zip(output.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        prop_assert!(max_error < 1e-3,
            "0 dB peak should be near-passthrough: max_error={}", max_error);
    }

}
