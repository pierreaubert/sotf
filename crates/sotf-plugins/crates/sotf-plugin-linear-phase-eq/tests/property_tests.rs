// ============================================================================
// Property-Based Tests for sotf-plugin-linear-phase-eq
// ============================================================================

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_linear_phase_eq::{BandConfig, LinearPhaseEqPlugin, LinearPhaseEqPluginParams};

fn mono_buffer_strategy() -> impl Strategy<Value = Vec<f32>> {
    (-0.5f32..0.5f32).prop_map(|v| vec![v; 64])
}

/// Build a plugin with the shortest FIR length and no active bands.
/// Tests that are not exercising FIR length itself should use this to keep
/// each proptest case cheap.
fn minimal_plugin() -> LinearPhaseEqPlugin {
    LinearPhaseEqPlugin::from_params(
        1,
        48000,
        LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 0,
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![],
        },
    )
    .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    // -------------------------------------------------------------------------
    // Finite output
    // -------------------------------------------------------------------------
    #[test]
    fn process_finite_output_default(buffer in mono_buffer_strategy()) {
        let mut plugin = minimal_plugin();
        let mut buf = buffer.clone();
        plugin.process_in_place(&mut buf, &ProcessContext::new(48000, 64)).unwrap();

        prop_assert!(buf.iter().all(|s| s.is_finite()),
            "Default FIR EQ should produce finite output");
    }

    #[test]
    fn process_finite_output_with_active_band(buffer in mono_buffer_strategy()) {
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 0,
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 6.0,
                active: true,
            }],
        };
        let mut plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
        let mut buf = buffer.clone();
        plugin.process_in_place(&mut buf, &ProcessContext::new(48000, 64)).unwrap();

        prop_assert!(buf.iter().all(|s| s.is_finite()),
            "Active FIR EQ band should produce finite output");
    }

    // -------------------------------------------------------------------------
    // Parameter round-trip
    // -------------------------------------------------------------------------
    #[test]
    fn roundtrip_mix(mix in 0.0f32..1.0f32) {
        let mut plugin = minimal_plugin();
        plugin.set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("mix"));

        match got {
            Some(ParameterValue::Float(v)) => {
                prop_assert!((v - mix).abs() < 0.001,
                    "mix round-trip drift: {} -> {}", mix, v);
            }
            other => prop_assert!(false, "Expected Float, got {:?}", other),
        }
    }

    #[test]
    fn roundtrip_auto_gain(enabled in prop::bool::ANY) {
        let mut plugin = minimal_plugin();
        plugin.set_parameter(ParameterId::from("auto_gain"), ParameterValue::Bool(enabled)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("auto_gain"));

        prop_assert_eq!(got, Some(ParameterValue::Bool(enabled)),
            "auto_gain set->get should round-trip");
    }

    #[test]
    fn roundtrip_num_filters(n in 1i32..10i32) {
        let mut plugin = minimal_plugin();
        plugin.set_parameter(ParameterId::from("num_filters"), ParameterValue::Int(n)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("num_filters"));

        prop_assert_eq!(got, Some(ParameterValue::Int(n)),
            "num_filters set->get should round-trip");
    }

    #[test]
    fn roundtrip_fir_length(idx in 0i32..4i32) {
        let mut plugin = minimal_plugin();
        plugin.set_parameter(ParameterId::from("fir_length"), ParameterValue::Int(idx)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("fir_length"));

        prop_assert_eq!(got, Some(ParameterValue::Int(idx)),
            "fir_length set->get should round-trip");
    }

    #[test]
    fn roundtrip_phase_mode(idx in 0i32..2i32) {
        let mut plugin = minimal_plugin();
        plugin.set_parameter(ParameterId::from("phase_mode"), ParameterValue::Int(idx)).unwrap();
        let got = plugin.get_parameter(&ParameterId::from("phase_mode"));

        prop_assert_eq!(got, Some(ParameterValue::Int(idx)),
            "phase_mode set->get should round-trip");
    }

    #[test]
    fn roundtrip_band_gain(gain_db in -24.0f32..24.0f32) {
        let mut plugin = minimal_plugin();
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
    fn dry_mix_passthrough(buffer in mono_buffer_strategy()) {
        // Construct with mix=0 from the start so the smoother begins at target.
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 0,
            phase_mode_index: 0,
            auto_gain: false,
            mix: 0.0,
            filters: vec![],
        };
        let mut plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();

        let input = buffer.clone();
        let mut output = buffer;
        plugin.process_in_place(&mut output, &ProcessContext::new(48000, 64)).unwrap();

        let max_error: f32 = input.iter().zip(output.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        prop_assert!(max_error < 1e-4,
            "mix=0 should be dry passthrough: max_error={}", max_error);
    }

    #[test]
    fn inactive_bands_approximately_passthrough_after_latency(buffer in mono_buffer_strategy()) {
        let params = LinearPhaseEqPluginParams {
            num_filters: 2,
            fir_length_index: 0,
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![
                BandConfig {
                    filter_type: "Peak".to_string(),
                    frequency: 1000.0,
                    q: 1.0,
                    gain_db: 12.0,
                    active: false,
                },
                BandConfig {
                    filter_type: "Lowshelf".to_string(),
                    frequency: 200.0,
                    q: 0.7,
                    gain_db: -12.0,
                    active: false,
                },
            ],
        };
        let mut plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
        let latency = plugin.latency_samples();

        // Process enough samples to get past the FIR group delay, then compare
        // steady-state output to input. Constant input is used so delay doesn't
        // shift the sample values.
        let total_frames = (latency + 256).max(512);
        let input_val = buffer.first().copied().unwrap_or(0.0);
        let mut output = vec![input_val; total_frames];
        plugin.process_in_place(&mut output, &ProcessContext::new(48000, total_frames)).unwrap();

        // Compare steady-state region after latency + a small safety margin.
        let steady_start = (latency + 64).min(total_frames - 64);
        let steady_end = steady_start + 64;
        let max_error: f32 = output[steady_start..steady_end]
            .iter()
            .map(|s| (s - input_val).abs())
            .fold(0.0f32, f32::max);
        prop_assert!(max_error < 1e-3,
            "Inactive bands should pass DC signal through after latency: max_error={}", max_error);
    }

    // -------------------------------------------------------------------------
    // Latency: linear-phase FIR should report correct group delay
    // -------------------------------------------------------------------------
    #[test]
    fn linear_phase_latency_matches_fir_length(
        gain_db in -12.0f32..12.0f32,
        freq in 200.0f32..8000.0f32
    ) {
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 0, // 1024 taps
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: freq as f64,
                q: 1.0,
                gain_db: gain_db as f64,
                active: true,
            }],
        };
        let plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
        let expected = (1024 - 1) / 2;
        prop_assert_eq!(plugin.latency_samples(), expected,
            "Linear-phase latency should equal (fir_length - 1) / 2");
    }

    #[test]
    fn minimum_phase_latency_is_zero(
        gain_db in -12.0f32..12.0f32,
        freq in 200.0f32..8000.0f32
    ) {
        let params = LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 0,
            phase_mode_index: 1,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: freq as f64,
                q: 1.0,
                gain_db: gain_db as f64,
                active: true,
            }],
        };
        let plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
        prop_assert_eq!(plugin.latency_samples(), 0,
            "Minimum-phase FIR should not report linear-phase group delay");
    }
}
