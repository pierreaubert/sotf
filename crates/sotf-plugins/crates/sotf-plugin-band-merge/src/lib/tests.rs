use super::band_merge_plugin::BandMergePlugin;
use super::misc::{MAX_BANDS, db_to_linear};
use super::types::BandMergePluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};

#[test]
fn test_band_merge_basic() {
    let mut p = BandMergePlugin::new(2, 2).unwrap();
    let i = vec![1.0, 2.0, 3.0, 4.0];
    let mut o = vec![0.0, 0.0];
    p.process(&i, &mut o, &ProcessContext::new(48000, 1))
        .unwrap();
    assert_eq!(o, vec![4.0, 6.0]);
}

#[test]
fn test_band_merge_with_gain() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    p.initialize(48000).unwrap();
    // Set band 0 gain to +6 dB (~2x), band 1 gain stays at 0 dB (1x)
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(6.0206),
    )
    .unwrap();

    // Process in small blocks to let the smoother converge across many calls.
    // The smoother advances `num_frames` steps per process() call, so the
    // total convergence is: 100 calls × 128 frames = 12 800 steps (>>480).
    let block = 128usize;
    let i_block = vec![1.0f32; block * 2]; // band0=1.0, band1=1.0
    let mut o_block = vec![0.0f32; block];
    let mut last = 0.0f32;
    for _ in 0..100 {
        p.process(&i_block, &mut o_block, &ProcessContext::new(48000, block))
            .unwrap();
        last = o_block[block - 1];
    }
    // After settling: Band 0 * 2.0 + Band 1 * 1.0 = 3.0
    assert!((last - 3.0).abs() < 0.01, "got {last}");
}

#[test]
fn test_band_merge_with_mute() {
    let mut p = BandMergePlugin::new(2, 2).unwrap();
    // Mute band 1
    p.set_parameter(ParameterId::from("band_1_mute"), ParameterValue::Bool(true))
        .unwrap();

    let i = vec![1.0, 2.0, 3.0, 4.0];
    let mut o = vec![0.0, 0.0];
    p.process(&i, &mut o, &ProcessContext::new(48000, 1))
        .unwrap();
    // Only band 0 contributes: [1.0, 2.0]
    assert_eq!(o, vec![1.0, 2.0]);
}

#[test]
fn test_band_merge_mute_and_gain_combined() {
    let mut p = BandMergePlugin::new(1, 3).unwrap();
    p.initialize(48000).unwrap();
    // Mute band 0
    p.set_parameter(ParameterId::from("band_0_mute"), ParameterValue::Bool(true))
        .unwrap();
    // Set band 2 gain to +6 dB (~2x)
    p.set_parameter(
        ParameterId::from("band_2_gain_db"),
        ParameterValue::Float(6.0206),
    )
    .unwrap();

    // Process in small blocks (see test_band_merge_with_gain for rationale).
    // 3 bands, 1 channel: band0=10.0, band1=1.0, band2=1.0 per frame.
    let block = 128usize;
    let frame = [10.0f32, 1.0, 1.0];
    let i_block: Vec<f32> = frame.iter().copied().cycle().take(block * 3).collect();
    let mut o_block = vec![0.0f32; block];
    let mut last = 0.0f32;
    for _ in 0..100 {
        p.process(&i_block, &mut o_block, &ProcessContext::new(48000, block))
            .unwrap();
        last = o_block[block - 1];
    }
    // After settling: band0 muted, band1 * 1.0 + band2 * 2.0 = 3.0
    assert!((last - 3.0).abs() < 0.01, "got {last}");
}

#[test]
fn test_band_merge_get_set_parameters() {
    let mut p = BandMergePlugin::new(2, 3).unwrap();

    // Default gain is 0.0
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_gain_db")),
        Some(ParameterValue::Float(0.0))
    );
    // Default mute is false
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_1_mute")),
        Some(ParameterValue::Bool(false))
    );

    // Set and retrieve
    p.set_parameter(
        ParameterId::from("band_2_gain_db"),
        ParameterValue::Float(-3.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_2_gain_db")),
        Some(ParameterValue::Float(-3.0))
    );

    p.set_parameter(ParameterId::from("band_0_mute"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_mute")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn test_band_merge_from_params() {
    let params = BandMergePluginParams {
        bands: 3,
        band_gains_db: vec![6.0206, 0.0, -60.0],
        band_mutes: vec![false, true, false],
    };
    let mut p = BandMergePlugin::from_params(1, &params).unwrap();

    // band0=1.0 * ~2.0, band1=1.0 muted, band2=1.0 * ~0.001
    let i = vec![1.0, 1.0, 1.0];
    let mut o = vec![0.0];
    p.process(&i, &mut o, &ProcessContext::new(48000, 1))
        .unwrap();
    // ~2.0 + 0 + ~0.001 ≈ 2.001
    assert!((o[0] - 2.0).abs() < 0.05, "got {}", o[0]);
}

/// With all bands at unity gain (0 dB) and no mutes, the reconstruction
/// error should be near 0 dB.
#[test]
fn test_reconstruction_error_db_unity() {
    let mut p = BandMergePlugin::new(2, 3).unwrap();

    // Process with non-trivial signal
    let nf = 100;
    let in_ch = 2 * 3; // 2 output channels * 3 bands
    let out_ch = 2;
    let mut input = vec![0.0f32; nf * in_ch];
    for frame in 0..nf {
        for band in 0..3 {
            for ch in 0..2 {
                input[frame * in_ch + band * out_ch + ch] =
                    0.3 * ((frame * 3 + band) as f32 * 0.1).sin();
            }
        }
    }
    let mut output = vec![0.0f32; nf * out_ch];
    let _ = p.get_parameter(&ParameterId::from("reconstruction_error_db"));
    p.process(&input, &mut output, &ProcessContext::new(48000, nf))
        .unwrap();

    // Get reconstruction_error_db via get_parameter
    let err = p
        .get_parameter(&ParameterId::from("reconstruction_error_db"))
        .unwrap();
    if let ParameterValue::Float(err_db) = err {
        assert!(
            err_db.abs() < 0.1,
            "With unity gains and no mutes, reconstruction error should be near 0 dB, got {err_db:.4}"
        );
    } else {
        panic!("reconstruction_error_db should be a Float parameter");
    }
}

#[test]
fn test_reconstruction_error_db_is_computed_on_demand() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();

    // Set a non-unity gain to make the diagnostic value clearly non-zero.
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(6.0206),
    )
    .unwrap();
    p.reset();

    let input = vec![1.0f32, 1.0];
    let mut output = vec![0.0f32];
    p.process(&input, &mut output, &ProcessContext::new(48000, 1))
        .unwrap();

    // No diagnostic read was requested yet, so the value should still be the
    // default 0 dB in-place (no on-demand work performed this frame).
    let err_before = match p
        .get_parameter(&ParameterId::from("reconstruction_error_db"))
        .unwrap()
    {
        ParameterValue::Float(v) => v,
        _ => panic!("reconstruction_error_db should be a Float parameter"),
    };
    assert!(
        err_before.abs() < 0.0001,
        "expected on-demand metric to remain untouched before request-processing cycle, got {err_before}"
    );

    // Next process should perform the diagnostic now that the host requested it.
    p.process(&input, &mut output, &ProcessContext::new(48000, 1))
        .unwrap();

    let err_after = match p
        .get_parameter(&ParameterId::from("reconstruction_error_db"))
        .unwrap()
    {
        ParameterValue::Float(v) => v,
        _ => panic!("reconstruction_error_db should be a Float parameter"),
    };
    assert!(
        err_after.abs() > 1.0,
        "expected reconstructed-error metric to be calculated after request, got {err_after}"
    );
}

#[test]
fn test_band_merge_parameters_list() {
    let p = BandMergePlugin::new(2, 3).unwrap();
    let params = p.parameters();
    // 1 (bands) + 3 * 2 (gain + mute per band) + 1 (reconstruction_error_db) = 8
    assert_eq!(params.len(), 8);
}

/// Gain changes must be smoothed: after a step change from 0 dB to +6 dB,
/// the output on the very first frame must be between the old gain (1.0)
/// and the new gain (~2.0), not equal to 2.0.  This verifies there is no
/// step discontinuity (zipper noise) on the first processed frame.
#[test]
fn test_gain_change_is_smoothed() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    // initialize() sets the smoother coefficient for 48 kHz.
    p.initialize(48000).unwrap();

    // Band gains start at 0 dB (linear 1.0).  Process one frame to lock
    // the smoother at 1.0 (unity).
    let i_unity = vec![1.0f32, 1.0]; // band0=1.0, band1=1.0
    let mut o = vec![0.0f32];
    p.process(&i_unity, &mut o, &ProcessContext::new(48000, 1))
        .unwrap();
    assert!((o[0] - 2.0).abs() < 1e-4, "baseline unity: got {}", o[0]);

    // Now apply a +6 dB step to band 0.  The linear gain jumps from 1.0 → ~2.0.
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(6.0206),
    )
    .unwrap();

    // First frame after the step: the smoother must NOT have reached 2.0 yet.
    // At 48 kHz with a 10 ms time constant the gain after 1 sample is
    // approximately 1.002 — strictly between 1.0 and 2.0.
    let mut o_step = vec![0.0f32];
    p.process(&i_unity, &mut o_step, &ProcessContext::new(48000, 1))
        .unwrap();

    // band0_smoothed (≈1.002) + band1_gain (1.0) = ≈2.002, well below 3.0
    assert!(
        o_step[0] > 2.0 && o_step[0] < 2.1,
        "expected smoothed output between 2.0 and 2.1 on first frame after gain step, got {}",
        o_step[0]
    );
}

/// reset() must snap smoothers to their current target immediately so that
/// playback resumes at the correct gain without an unwanted ramp.
#[test]
fn test_reset_snaps_smoother() {
    // Minimum 2 bands required by the plugin.
    // Both bands will have input 1.0; band 0 gets +6 dB (~2.0 linear), band 1 stays at 0 dB.
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    p.initialize(48000).unwrap();

    // Apply a gain that the smoother has not yet reached.
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(6.0206), // ~2.0 linear
    )
    .unwrap();

    // reset() should snap the smoother to 2.0 immediately.
    p.reset();

    // First frame after reset must already be at the target gain.
    // Input: [band0=1.0, band1=1.0]; expected output: 2.0*1.0 + 1.0*1.0 = 3.0.
    let i = vec![1.0f32, 1.0]; // 2 bands, 1 channel, 1 frame
    let mut o = vec![0.0f32];
    p.process(&i, &mut o, &ProcessContext::new(48000, 1))
        .unwrap();
    assert!(
        (o[0] - 3.0).abs() < 0.01,
        "after reset(), first frame should equal settled output (~3.0), got {}",
        o[0]
    );
}

#[test]
fn test_new_invalid_band_count_errors() {
    assert!(BandMergePlugin::new(1, 1).is_err());
    assert!(BandMergePlugin::new(1, MAX_BANDS + 1).is_err());
}

#[test]
fn test_set_parameter_bands_out_of_range_errors() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    assert!(
        p.set_parameter(ParameterId::from("bands"), ParameterValue::Int(1))
            .is_err()
    );
    assert!(
        p.set_parameter(
            ParameterId::from("bands"),
            ParameterValue::Int(MAX_BANDS as i32 + 1)
        )
        .is_err()
    );
}

#[test]
fn test_set_parameter_gain_wrong_type_errors() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    assert!(
        p.set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Bool(true)
        )
        .is_err()
    );
}

#[test]
fn test_set_parameter_mute_wrong_type_errors() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    assert!(
        p.set_parameter(ParameterId::from("band_0_mute"), ParameterValue::Float(1.0))
            .is_err()
    );
}

#[test]
fn test_set_parameter_unknown_id_errors() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    assert!(
        p.set_parameter(ParameterId::from("band_xyz"), ParameterValue::Float(1.0))
            .is_err()
    );
}

#[test]
fn test_set_parameter_out_of_range_band_index_errors() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    assert!(
        p.set_parameter(
            ParameterId::from("band_99_gain_db"),
            ParameterValue::Float(1.0)
        )
        .is_err()
    );
    assert!(
        p.set_parameter(
            ParameterId::from("band_99_mute"),
            ParameterValue::Bool(true)
        )
        .is_err()
    );
}

#[test]
fn test_get_parameter_unknown_returns_none() {
    let p = BandMergePlugin::new(1, 2).unwrap();
    assert_eq!(p.get_parameter(&ParameterId::from("no_such_param")), None);
}

#[test]
fn test_from_params_defaults_missing_gains_and_mutes() {
    let params = BandMergePluginParams {
        bands: 3,
        band_gains_db: vec![6.0206],
        band_mutes: vec![true],
    };
    let p = BandMergePlugin::from_params(1, &params).unwrap();
    assert!((p.band_gains_db[0] - 6.0206).abs() < 0.001);
    assert!(p.band_mutes[0]);
    // Missing entries keep their defaults from Self::new.
    assert_eq!(p.band_gains_db[1], 0.0);
    assert!(!p.band_mutes[1]);
    assert_eq!(p.band_gains_db[2], 0.0);
    assert!(!p.band_mutes[2]);
}

#[test]
fn test_process_zero_frames_is_noop() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    let mut output = vec![99.0f32];
    p.process(&[], &mut output, &ProcessContext::new(48000, 0))
        .unwrap();
    assert_eq!(output[0], 99.0);
}

#[test]
fn test_process_nan_input_does_not_panic() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    let input = vec![f32::NAN, f32::NAN];
    let mut output = vec![0.0f32];
    p.process(&input, &mut output, &ProcessContext::new(48000, 1))
        .unwrap();
    assert!(output[0].is_nan());
}

#[test]
fn test_reconstruction_error_with_gain_and_mute() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(6.0206),
    )
    .unwrap();
    p.set_parameter(ParameterId::from("band_1_mute"), ParameterValue::Bool(true))
        .unwrap();

    // Request the diagnostic.
    let _ = p.get_parameter(&ParameterId::from("reconstruction_error_db"));

    let input = vec![1.0f32, 1.0];
    let mut output = vec![0.0f32];
    p.process(&input, &mut output, &ProcessContext::new(48000, 1))
        .unwrap();

    let err = p
        .get_parameter(&ParameterId::from("reconstruction_error_db"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!(
        err.abs() > 1.0,
        "expected large reconstruction error with gain+mute, got {}",
        err
    );
}

#[test]
fn test_db_to_linear_round_trip() {
    // Smoke-test the helper re-exported from sotf_host.
    assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
    assert!((db_to_linear(6.0206) - 2.0).abs() < 0.001);
    assert!((db_to_linear(-6.0206) - 0.5).abs() < 0.001);
}

#[test]
fn test_set_parameter_bands_valid() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    assert_eq!(p.num_bands, 2);

    p.set_parameter(ParameterId::from("bands"), ParameterValue::Int(4))
        .unwrap();
    assert_eq!(p.num_bands, 4);
    assert_eq!(
        p.get_parameter(&ParameterId::from("bands")),
        Some(ParameterValue::Int(4))
    );
}

#[test]
fn test_set_parameter_gain_non_finite_ignored() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    let before = p.band_gains_db[0];

    // NaN should be ignored (returns Ok but does not change)
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(f32::NAN),
    )
    .unwrap();
    assert_eq!(p.band_gains_db[0], before);

    // Infinity should be ignored
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(f32::INFINITY),
    )
    .unwrap();
    assert_eq!(p.band_gains_db[0], before);
}

#[test]
fn test_set_parameter_gain_extreme_finite_values() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(-60.0),
    )
    .unwrap();
    assert!((p.band_gains_db[0] - (-60.0)).abs() < 1e-4);
    assert!((p.band_gains_linear[0] - 0.001).abs() < 1e-4);

    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(24.0),
    )
    .unwrap();
    assert!((p.band_gains_db[0] - 24.0).abs() < 1e-4);
}

#[test]
fn test_get_parameter_all_band_params() {
    let mut p = BandMergePlugin::new(1, 3).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_gain_db"),
        ParameterValue::Float(-3.0),
    )
    .unwrap();
    p.set_parameter(ParameterId::from("band_1_mute"), ParameterValue::Bool(true))
        .unwrap();

    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_gain_db")),
        Some(ParameterValue::Float(-3.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_1_mute")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_2_gain_db")),
        Some(ParameterValue::Float(0.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_2_mute")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn test_process_reconstruction_error_silence() {
    let mut p = BandMergePlugin::new(1, 2).unwrap();
    // Request the diagnostic
    let _ = p.get_parameter(&ParameterId::from("reconstruction_error_db"));

    // Process silence
    let input = vec![0.0f32, 0.0];
    let mut output = vec![0.0f32];
    p.process(&input, &mut output, &ProcessContext::new(48000, 1))
        .unwrap();

    let err = p
        .get_parameter(&ParameterId::from("reconstruction_error_db"))
        .unwrap()
        .as_float()
        .unwrap();
    // When reference is silence, reconstruction error should be 0.0
    assert_eq!(err, 0.0);
}
