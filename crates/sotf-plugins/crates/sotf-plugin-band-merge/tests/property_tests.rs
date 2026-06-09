// ============================================================================
// Property-Based Tests for sotf-plugin-band-merge
// ============================================================================

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_merge::BandMergePlugin;

proptest! {
    // ------------------------------------------------------------------------
    // Finite output
    // ------------------------------------------------------------------------
    #[test]
    fn finite_output_on_random_input(
        num_frames in 16usize..128,
        num_bands in 2usize..5,
        out_channels in 1usize..3,
        val in -1.0f32..1.0f32,
    ) {
        prop_assume!(num_bands <= 4); // MAX_BANDS

        let mut plugin = BandMergePlugin::new(out_channels, num_bands).unwrap();
        plugin.initialize(48000).unwrap();

        let in_channels = out_channels * num_bands;
        let input = vec![val; num_frames * in_channels];
        let mut output = vec![0.0f32; num_frames * out_channels];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        prop_assert!(
            output.iter().all(|s| s.is_finite()),
            "BandMerge output must be finite for finite input"
        );
    }

    // ------------------------------------------------------------------------
    // NaN propagation
    // ------------------------------------------------------------------------
    #[test]
    fn nan_propagates(nan_offset in 0usize..64, num_bands in 2usize..5) {
        prop_assume!(num_bands <= 4);

        let out_channels = 1;
        let in_channels = out_channels * num_bands;
        let num_frames = 64;

        let mut plugin = BandMergePlugin::new(out_channels, num_bands).unwrap();
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.5f32; num_frames * in_channels];
        let idx = nan_offset % input.len();
        input[idx] = f32::NAN;
        let mut output = vec![0.0f32; num_frames * out_channels];
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
    // Identity / passthrough: unity gains on identical band inputs
    // ------------------------------------------------------------------------
    #[test]
    fn dc_reconstructs_with_unity_gains(
        dc in 0.1f32..0.9f32,
        num_bands in 2usize..5,
    ) {
        prop_assume!(num_bands <= 4);

        let out_channels = 1;
        let in_channels = out_channels * num_bands;
        let num_frames = 128;

        let mut plugin = BandMergePlugin::new(out_channels, num_bands).unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![dc; num_frames * in_channels];
        let mut output = vec![0.0f32; num_frames * out_channels];
        plugin
            .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
            .unwrap();

        let expected = dc * num_bands as f32;
        let out = output[num_frames - 1];
        prop_assert!(
            (out - expected).abs() < 0.01,
            "merge output {} expected {}",
            out,
            expected
        );
    }

    // ------------------------------------------------------------------------
    // Round-trip: set -> get returns the original value
    // ------------------------------------------------------------------------
    #[test]
    fn band_gain_set_get_roundtrip(gain_db in -60.0f32..24.0f32) {
        let mut plugin = BandMergePlugin::new(1, 2).unwrap();
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
    // Monotonicity: muting a band reduces the summed output for positive DC
    // ------------------------------------------------------------------------
    #[test]
    fn mute_attenuates_output(dc in 0.1f32..0.9f32) {
        let num_frames = 64;
        let mut plugin = BandMergePlugin::new(1, 2).unwrap();
        plugin.initialize(48000).unwrap();

        let input = vec![dc; num_frames * 2];
        let mut out_unmuted = vec![0.0f32; num_frames];
        plugin
            .process(&input, &mut out_unmuted, &ProcessContext::new(48000, num_frames))
            .unwrap();

        plugin
            .set_parameter(
                ParameterId::from("band_0_mute"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        let mut out_muted = vec![0.0f32; num_frames];
        plugin
            .process(&input, &mut out_muted, &ProcessContext::new(48000, num_frames))
            .unwrap();

        let last_unmuted = out_unmuted[num_frames - 1];
        let last_muted = out_muted[num_frames - 1];
        prop_assert!(
            last_muted < last_unmuted,
            "muting band 0 should reduce output: muted={}, unmuted={}",
            last_muted,
            last_unmuted
        );
    }
}
