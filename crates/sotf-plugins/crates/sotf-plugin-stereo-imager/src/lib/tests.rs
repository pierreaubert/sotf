use super::stereo_imager_plugin::StereoImagerPlugin;
use super::stereo_imager_plugin_params::StereoImagerPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

fn make_context(num_frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(48000, num_frames)
}

/// reset() must snap all smoothers to their current target values.
/// If a smoother is mid-transition when reset() is called, it must
/// jump immediately to the target — not resume from where it left off.
#[test]
fn test_reset_snaps_smoothers() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    // Drive the width smoother into a transition: set a new target
    plugin
        .parametric_set_parameter(ParameterId::from("width"), ParameterValue::Float(0.0))
        .unwrap();

    // reset() should snap the smoother to the new target (0.0)
    plugin.reset();

    // After reset, processing one frame: the smoother must produce 0.0,
    // meaning side is fully suppressed — L and R must be equal (both = mid).
    let num_frames = 512;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        let t = i as f32 * 0.01;
        buffer.push(t.sin() * 0.5); // L
        buffer.push(t.cos() * 0.3); // R
    }
    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // Every sample must have L == R (no smoother ramp-in from the old value)
    for frame in 0..num_frames {
        let idx = frame * 2;
        let diff = (buffer[idx] - buffer[idx + 1]).abs();
        assert!(
            diff < 0.01,
            "frame {frame}: L={} R={} — smoother was not snapped by reset()",
            buffer[idx],
            buffer[idx + 1]
        );
    }
}

#[test]
fn test_crossover_frequency_changes_are_smoothed() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    let initial = plugin.crossover_low.frequency();
    plugin
        .parametric_set_parameter(
            ParameterId::from("low_mid_freq"),
            ParameterValue::Float(1000.0),
        )
        .unwrap();

    assert!(
        (plugin.crossover_low.frequency() - initial).abs() < 1e-3,
        "set_parameter should retarget the frequency smoother, not retune LR4 coefficients instantly"
    );

    let mut buffer = vec![0.25f32; 256 * 2];
    plugin
        .process_in_place(&mut buffer, &make_context(256))
        .unwrap();

    let after = plugin.crossover_low.frequency();
    assert!(
        after > initial && after < 1000.0,
        "frequency should move gradually during processing: initial={initial}, after={after}"
    );
}

/// mono_bass=false: the per-sample mono_bass branch must not change output
/// when toggled rapidly mid-buffer (no crash, no NaN).
#[test]
fn test_rapid_mono_bass_toggle_no_nan() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let mut buffer: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.1).sin() * 0.5)
        .collect();

    // Toggle mono_bass multiple times before and during processing
    for _ in 0..5 {
        plugin
            .parametric_set_parameter(ParameterId::from("mono_bass"), ParameterValue::Bool(true))
            .unwrap();
        plugin
            .parametric_set_parameter(ParameterId::from("mono_bass"), ParameterValue::Bool(false))
            .unwrap();
    }

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    for (i, &s) in buffer.iter().enumerate() {
        assert!(
            !s.is_nan(),
            "NaN at sample {i} after rapid mono_bass toggle"
        );
        assert!(
            !s.is_infinite(),
            "Inf at sample {i} after rapid mono_bass toggle"
        );
    }
}

/// Buffer larger than 8192 frames: must not panic (dry_buf must accommodate).
#[test]
fn test_large_buffer_no_panic() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    let num_frames = 16384;
    let mut buffer = vec![0.5_f32; num_frames * 2];
    // Should not panic
    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();
}

#[test]
fn oversize_block_is_rejected_without_growing_dry_buffer() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    let initial_len = plugin.dry_buf.len();
    let num_frames = initial_len / 2 + 1;
    let mut buffer = vec![0.25_f32; num_frames * 2];

    let err = plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .expect_err("oversize audio blocks must not allocate in process_in_place");

    assert!(err.contains("exceeds preallocated scratch"));
    assert_eq!(plugin.dry_buf.len(), initial_len);
}

/// Mix=0 (fully dry): output must be byte-for-byte identical to input.
#[test]
fn test_mix_zero_full_passthrough() {
    let params = StereoImagerPluginParams {
        mix: 0.0,
        width: 2.0, // Irrelevant — mix=0 means pure dry
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
    };
    let mut plugin = StereoImagerPlugin::new(2, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let mut buffer: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.05).sin() * 0.7)
        .collect();
    let original = buffer.clone();

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    for (i, (&orig, &out)) in original.iter().zip(buffer.iter()).enumerate() {
        assert_eq!(
            orig, out,
            "sample {i}: mix=0 changed output: expected {orig}, got {out}"
        );
    }
}

/// Width=1.0, all band widths=1.0, mono_bass=false, mix=1.0 --> output equals input
#[test]
fn test_stereo_imager_passthrough() {
    let params = StereoImagerPluginParams {
        width: 1.0,
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
        mix: 1.0,
    };
    let mut plugin = StereoImagerPlugin::new(2, params);
    plugin.initialize(48000).unwrap();

    // Feed a constant stereo signal for long enough to settle crossover transients
    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        buffer.push(0.7); // L
        buffer.push(0.3); // R
    }
    let original = buffer.clone();

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // Check the settled region (skip initial crossover transient)
    let settle = 2000;
    for frame in settle..num_frames {
        let idx = frame * 2;
        assert!(
            (buffer[idx] - original[idx]).abs() < 0.02,
            "frame {frame} L: expected {}, got {}",
            original[idx],
            buffer[idx]
        );
        assert!(
            (buffer[idx + 1] - original[idx + 1]).abs() < 0.02,
            "frame {frame} R: expected {}, got {}",
            original[idx + 1],
            buffer[idx + 1]
        );
    }
}

/// Width=0.0 --> L and R should be identical (mono)
#[test]
fn test_stereo_imager_mono() {
    let params = StereoImagerPluginParams {
        width: 0.0,
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
        mix: 1.0,
    };
    let mut plugin = StereoImagerPlugin::new(2, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        buffer.push((i as f32 * 0.01).sin() * 0.5); // L
        buffer.push((i as f32 * 0.02).cos() * 0.3); // R
    }

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // After settling, L and R should be identical (both = mid only)
    let settle = 2000;
    for frame in settle..num_frames {
        let idx = frame * 2;
        let diff = (buffer[idx] - buffer[idx + 1]).abs();
        assert!(
            diff < 0.01,
            "frame {frame}: L={} R={} diff={diff}",
            buffer[idx],
            buffer[idx + 1]
        );
    }
}

/// mono_bass=true --> low frequencies should be mono
#[test]
fn test_stereo_imager_mono_bass() {
    let params = StereoImagerPluginParams {
        width: 1.0,
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: true,
        mix: 1.0,
    };
    let mut plugin = StereoImagerPlugin::new(2, params);
    plugin.initialize(48000).unwrap();

    // Feed a DC offset (which is all "low" band): L=0.8, R=0.2
    // With mono_bass, the low band side is collapsed to zero.
    // So the low band becomes mid only: (0.8+0.2)*0.5 = 0.5 for both L and R.
    // But mid and high bands still carry the original stereo difference.
    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        buffer.push(0.8); // L
        buffer.push(0.2); // R
    }

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // After settling, DC should be in low band only, and mono_bass collapses
    // the side, so L and R converge for this DC signal.
    let settle = 2000;
    for frame in settle..num_frames {
        let idx = frame * 2;
        let diff = (buffer[idx] - buffer[idx + 1]).abs();
        assert!(
            diff < 0.05,
            "frame {frame}: L={} R={} diff={diff} (expected mono bass)",
            buffer[idx],
            buffer[idx + 1]
        );
    }
}

/// Width=2.0 --> side component should be doubled
#[test]
fn test_stereo_imager_wide() {
    let params = StereoImagerPluginParams {
        width: 2.0,
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
        mix: 1.0,
    };
    let mut plugin = StereoImagerPlugin::new(2, params);
    plugin.initialize(48000).unwrap();

    // With width=2.0 and all band widths=1.0, the side is scaled by 2.0.
    // For a constant signal: L=0.8, R=0.2:
    //   mid = 0.5, side = 0.3
    //   scaled_side = 0.3 * 2.0 = 0.6
    //   wet_L = 0.5 + 0.6 = 1.1
    //   wet_R = 0.5 - 0.6 = -0.1
    let num_frames = 10000;
    let mut buffer = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        buffer.push(0.8);
        buffer.push(0.2);
    }

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    let last = (num_frames - 1) * 2;
    let l = buffer[last];
    let r = buffer[last + 1];

    // L should be wider than original 0.8, R should be narrower than 0.2
    assert!(l > 0.9, "Wide L should be > 0.9, got {l}");
    assert!(r < 0.1, "Wide R should be < 0.1, got {r}");

    // The difference (L-R) should be roughly 4x the original side
    // Original side content: (0.8-0.2)/2 = 0.3 * 2 (width) = 0.6 each way
    // So L-R = 2*scaled_side = 1.2 vs original L-R = 0.6
    let diff = l - r;
    let original_diff = 0.6; // 0.8 - 0.2
    assert!(
        diff > original_diff * 1.5,
        "L-R difference ({diff}) should be significantly larger than original ({original_diff})"
    );
}

/// Parameter set/get roundtrip
#[test]
fn test_parameter_roundtrip() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    // Set width
    plugin
        .parametric_set_parameter(ParameterId::from("width"), ParameterValue::Float(1.5))
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("width"));
    assert_eq!(val, Some(ParameterValue::Float(1.5)));

    // Set mono_bass
    plugin
        .parametric_set_parameter(ParameterId::from("mono_bass"), ParameterValue::Bool(true))
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("mono_bass"));
    assert_eq!(val, Some(ParameterValue::Bool(true)));

    // Set mix
    plugin
        .parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    let val = plugin.parametric_get_parameter(&ParameterId::from("mix"));
    assert_eq!(val, Some(ParameterValue::Float(0.75)));

    // Unknown parameter should fail
    let result = plugin
        .parametric_set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0));
    assert!(result.is_err());

    // Unknown get should return None
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("nonexistent")),
        None
    );
}

#[test]
fn test_plugin_info_version_matches_manifest() {
    let plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    assert_eq!(plugin.info().version, env!("CARGO_PKG_VERSION"));
}

/// Non-stereo channels should pass through unchanged
#[test]
fn test_non_stereo_passthrough() {
    let mut plugin = StereoImagerPlugin::new(1, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    let mut buffer = vec![0.5, 0.3, 0.7, 0.1];
    let original = buffer.clone();

    plugin
        .process_in_place(&mut buffer, &make_context(4))
        .unwrap();

    assert_eq!(buffer, original);
}

/// Bug: process_in_place must NOT call initialize() on sample-rate mismatch.
/// Audio-thread allocation inside the callback causes dropouts.
#[test]
fn test_process_does_not_reinitialize_on_sample_rate_mismatch() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();
    assert_eq!(plugin.sample_rate, 48000);

    let mut buffer = vec![0.5_f32; 256 * 2];
    let ctx = ProcessContext::new(44100, 256);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // initialize() should NOT have been called, so sample_rate stays 48000
    assert_eq!(
        plugin.sample_rate, 48000,
        "process_in_place must not call initialize() when context.sample_rate differs"
    );
}

// -------------------------------------------------------------------------
// set_parameter extended coverage
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_all_band_params_roundtrip() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    let cases: &[(&str, f32)] = &[
        ("low_mid_freq", 500.0),
        ("mid_high_freq", 3000.0),
        ("low_width", 0.5),
        ("mid_width", 1.5),
        ("high_width", 2.0),
    ];

    for &(id, value) in cases {
        plugin
            .parametric_set_parameter(ParameterId::from(id), ParameterValue::Float(value))
            .unwrap();
        let got = plugin.parametric_get_parameter(&ParameterId::from(id));
        assert_eq!(
            got,
            Some(ParameterValue::Float(value)),
            "roundtrip failed for {}",
            id
        );
    }
}

#[test]
fn test_set_parameter_unknown_returns_error() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    assert!(
        plugin
            .set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0),)
            .is_err()
    );
}

#[test]
fn test_set_parameter_out_of_range_returns_error() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    // width range [0, 2]
    assert!(
        plugin
            .set_parameter(ParameterId::from("width"), ParameterValue::Float(-1.0))
            .is_err()
    );
    assert!(
        plugin
            .set_parameter(ParameterId::from("width"), ParameterValue::Float(5.0))
            .is_err()
    );

    // mix range [0, 1]
    assert!(
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.5))
            .is_err()
    );
}

// -------------------------------------------------------------------------
// process_in_place extended coverage
// -------------------------------------------------------------------------

#[test]
fn test_process_empty_buffer() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    let mut buffer = vec![0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    let frames = plugin.process_in_place(&mut buffer, &ctx).unwrap();
    assert_eq!(frames, 0);
}

#[test]
fn test_process_mix_one_wet_path() {
    let params = StereoImagerPluginParams {
        mix: 1.0,
        width: 2.0,
        low_mid_freq: 250.0,
        mid_high_freq: 4000.0,
        low_width: 1.0,
        mid_width: 1.0,
        high_width: 1.0,
        mono_bass: false,
    };
    let mut plugin = StereoImagerPlugin::new(2, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let mut buffer: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.05).sin() * 0.7)
        .collect();

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // All outputs should be finite
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_freq_swap_when_low_greater_than_high() {
    let mut plugin = StereoImagerPlugin::new(2, StereoImagerPluginParams::default());
    plugin.initialize(48000).unwrap();

    // Deliberately set low > high by mutating fields directly (bypasses
    // validate_parameter which would reject this combination).
    plugin.low_mid_freq = 5000.0;
    plugin.low_mid_freq_smoother.set_target(5000.0);
    plugin.mid_high_freq = 200.0;
    plugin.mid_high_freq_smoother.set_target(200.0);

    let num_frames = 512;
    let mut buffer: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.05).sin() * 0.7)
        .collect();

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // Processing should not crash and output should be finite
    assert!(buffer.iter().all(|s| s.is_finite()));
}
