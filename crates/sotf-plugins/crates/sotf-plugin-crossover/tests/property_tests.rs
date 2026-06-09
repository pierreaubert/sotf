// ============================================================================
// Property-Based Tests for sotf-plugin-crossover
// ============================================================================

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_crossover::CrossoverPlugin;

proptest! {
    // ------------------------------------------------------------------------
    // Finite output
    // ------------------------------------------------------------------------
    #[test]
    fn finite_output_on_random_input_lr24(
        num_frames in 16usize..128,
        input_val in -1.0f32..1.0f32,
    ) {
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![input_val; num_frames];
        let mut output = vec![0.0f32; num_frames * 2];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        prop_assert!(
            output.iter().all(|s| s.is_finite()),
            "LR24 both-mode output must be finite for finite input"
        );
    }

    // ------------------------------------------------------------------------
    // NaN propagation
    // ------------------------------------------------------------------------
    #[test]
    fn nan_propagates_through_lr24(nan_offset in 0usize..64) {
        let num_frames = 64;
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.5f32; num_frames];
        input[nan_offset % num_frames] = f32::NAN;
        let mut output = vec![0.0f32; num_frames * 2];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        prop_assert!(
            output.iter().any(|s| s.is_nan()),
            "NaN input must produce at least one NaN output sample"
        );
        prop_assert!(
            !output.iter().any(|s| s.is_infinite()),
            "NaN input must not produce Inf output"
        );
    }

    // ------------------------------------------------------------------------
    // DC reconstruction (identity / passthrough for LR24 sum)
    // ------------------------------------------------------------------------
    #[test]
    fn dc_reconstructs_in_both_mode(dc in -0.9f32..0.9f32) {
        let num_frames = 256;
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![dc; num_frames];
        let mut output = vec![0.0f32; num_frames * 2];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        let low = output[(num_frames - 1) * 2];
        let high = output[(num_frames - 1) * 2 + 1];
        let sum = low + high;

        prop_assert!(
            (sum - dc).abs() < 0.05,
            "LR24 low+high should reconstruct DC: got {} (low={}, high={}, dc={})",
            sum,
            low,
            high,
            dc
        );
    }

    // ------------------------------------------------------------------------
    // Round-trip: set -> get returns the original value
    // ------------------------------------------------------------------------
    #[test]
    fn frequency_set_get_roundtrip(freq in 20.0f32..20000.0f32) {
        let mut plugin = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(48000).unwrap();

        plugin
            .set_parameter(ParameterId::from("frequency"), ParameterValue::Float(freq))
            .unwrap();
        let got = plugin
            .get_parameter(&ParameterId::from("frequency"))
            .and_then(|v| v.as_float())
            .unwrap();

        prop_assert_eq!(got, freq, "frequency set/get roundtrip failed");
    }
}
