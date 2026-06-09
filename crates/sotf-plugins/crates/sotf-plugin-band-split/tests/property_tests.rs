// ============================================================================
// Property-Based Tests for sotf-plugin-band-split
// ============================================================================

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_merge::BandMergePlugin;
use sotf_plugin_band_split::BandSplitPlugin;

proptest! {
    // ------------------------------------------------------------------------
    // Finite output
    // ------------------------------------------------------------------------
    #[test]
    fn finite_output_on_random_input(
        num_frames in 16usize..128,
        input_val in -1.0f32..1.0f32,
    ) {
        let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![input_val; num_frames];
        let mut output = vec![0.0f32; num_frames * 2];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        prop_assert!(
            output.iter().all(|s| s.is_finite()),
            "BandSplit output must be finite for finite input"
        );
    }

    // ------------------------------------------------------------------------
    // NaN propagation
    // ------------------------------------------------------------------------
    #[test]
    fn nan_propagates(nan_offset in 0usize..64) {
        let num_frames = 64;
        let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.5f32; num_frames];
        input[nan_offset] = f32::NAN;
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
    // DC reconstruction (sum of split bands)
    // ------------------------------------------------------------------------
    #[test]
    fn dc_reconstructs_unity(dc in -0.9f32..0.9f32) {
        let num_frames = 256;
        let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
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
            "Split bands should reconstruct DC: got {} (low={}, high={}, dc={})",
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
        let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
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

    #[test]
    fn band_gain_set_get_roundtrip(gain_db in -24.0f32..24.0f32) {
        let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
        plugin.initialize(48000).unwrap();

        plugin
            .set_parameter(
                ParameterId::from("band_0_gain_db"),
                ParameterValue::Float(gain_db),
            )
            .unwrap();
        let got = plugin
            .get_parameter(&ParameterId::from("band_0_gain_db"))
            .and_then(|v| v.as_float())
            .unwrap();

        prop_assert_eq!(got, gain_db, "band gain set/get roundtrip failed");
    }

    // ------------------------------------------------------------------------
    // DC reconstruction through split + band-merge
    // ------------------------------------------------------------------------
    #[test]
    fn split_then_merge_reconstructs_dc(dc in -0.9f32..0.9f32) {
        let num_frames = 256;
        let channels = 2;

        let mut split = BandSplitPlugin::new(channels, 1000.0, "LR24").unwrap();
        split.initialize(48000).unwrap();

        let mut merge = BandMergePlugin::new(channels, 2).unwrap();
        merge.initialize(48000).unwrap();

        let input = vec![dc; num_frames * channels];
        let mut split_out = vec![0.0f32; num_frames * channels * 2];
        split
            .process(&input, &mut split_out, &ProcessContext::new(48000, num_frames))
            .unwrap();

        let mut merge_out = vec![0.0f32; num_frames * channels];
        merge
            .process(&split_out, &mut merge_out, &ProcessContext::new(48000, num_frames))
            .unwrap();

        for ch in 0..channels {
            let out = merge_out[(num_frames - 1) * channels + ch];
            prop_assert!(
                (out - dc).abs() < 0.05,
                "split+merge DC reconstruction failed: got {} expected {} on ch {}",
                out,
                dc,
                ch
            );
        }
    }
}
