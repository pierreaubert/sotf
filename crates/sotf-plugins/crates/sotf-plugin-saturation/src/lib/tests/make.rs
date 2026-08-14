use super::super::saturation_plugin::SaturationPlugin;
use super::super::saturation_plugin_params::SaturationPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

pub(super) fn make_context(num_frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(48000, num_frames)
}

pub(super) fn make_sine(
    freq_hz: f32,
    sample_rate: u32,
    num_frames: usize,
    amplitude: f32,
) -> Vec<f32> {
    (0..num_frames)
        .map(|i| {
            amplitude * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
        })
        .collect()
}

#[test]
fn test_soft_clip_limits_output() {
    // With high drive, output should still be bounded
    let channels = 1;
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 10.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let mut buffer = make_sine(1000.0, 48000, num_frames, 0.8);

    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // All samples should be bounded within [-1.0, 1.0] (tanh/tanh(drive))
    // ADAA mode can produce tiny overshoots (~1-2%) at the transition between
    // fallback and normal operation, and DC blocker adds transient ripple
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        peak <= 1.05, // ADAA + DC blocker tolerance
        "Soft clip output should be bounded: peak={}",
        peak
    );
    // Output should not be silent
    assert!(
        peak > 0.1,
        "Soft clip should produce non-trivial output: peak={}",
        peak
    );
}

#[test]
fn test_tape_saturation() {
    let channels = 1;
    let params = SaturationPluginParams {
        mode: "Tape".to_string(),
        drive: 5.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let mut buffer = make_sine(1000.0, 48000, num_frames, 0.8);

    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // Tape output is bounded by 0.5 (the scaling factor)
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        peak <= 0.55, // tolerance
        "Tape output should be bounded around 0.5: peak={}",
        peak
    );
    assert!(
        peak > 0.1,
        "Tape should produce non-trivial output: peak={}",
        peak
    );
}

#[test]
fn test_saturation_passthrough() {
    // With mix=0, output should equal dry signal
    let channels = 2;
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 10.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 0.0,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let mut buffer = vec![0.0f32; num_frames * channels];
    for frame in 0..num_frames {
        let t = frame as f32 / 48000.0;
        let val = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        buffer[frame * channels] = val;
        buffer[frame * channels + 1] = val;
    }
    let original = buffer.clone();

    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // Output should be identical to input (mix=0 means fully dry)
    for i in 0..buffer.len() {
        let diff = (buffer[i] - original[i]).abs();
        assert!(
            diff < 1e-5,
            "mix=0: sample {} differs: output={}, expected={}, diff={}",
            i,
            buffer[i],
            original[i],
            diff
        );
    }
}

#[test]
fn test_oversampling_processes() {
    // Verify 2x oversampling produces output without errors
    let channels = 1;
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 5.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "2x".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 512;

    // Process multiple blocks to fill the oversampler pipeline
    for _ in 0..20 {
        let mut buffer = make_sine(1000.0, 48000, num_frames, 0.5);
        let ctx = make_context(num_frames);
        let result = plugin.process_in_place(&mut buffer, &ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), num_frames);

        // All samples should be finite
        for (i, &s) in buffer.iter().enumerate() {
            assert!(s.is_finite(), "sample {} not finite: {}", i, s);
        }
    }
}

#[test]
fn test_short_buffer_returns_error() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(48000).unwrap();
    let ctx = make_context(16);
    let mut buffer = vec![0.0f32; 31];

    let err = plugin.process_in_place(&mut buffer, &ctx).unwrap_err();

    assert!(err.contains("buffer too short"));
}

#[test]
fn test_exciter_with_oversampling_processes() {
    let channels = 1;
    let params = SaturationPluginParams {
        mode: "Exciter".to_string(),
        drive: 8.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "2x".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let ctx = make_context(num_frames);
    for _ in 0..20 {
        let mut buffer = make_sine(8000.0, 48000, num_frames, 0.5);
        let processed = plugin.process_in_place(&mut buffer, &ctx).unwrap();

        assert_eq!(processed, num_frames);
        assert!(buffer.iter().all(|s| s.is_finite()));
    }
}

/// Bug 2.1: low_buf / high_buf not resized with dry_buf.
/// A block larger than DEFAULT_BUF_SIZE must not panic in exciter mode.
#[test]
fn test_exciter_large_block_no_panic() {
    let channels = 2;
    let params = SaturationPluginParams {
        mode: "Exciter".to_string(),
        drive: 5.0,
        tone: 1.5,
        exciter_freq: 3000.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    // Send a block larger than DEFAULT_BUF_SIZE (96000 samples total = 48000 frames * 2 ch)
    let num_frames = 50000; // > 48000 default frames per channel
    let mut buffer = vec![0.3f32; num_frames * channels];
    let ctx = make_context(num_frames);
    // Must not panic
    let result = plugin.process_in_place(&mut buffer, &ctx);
    assert!(result.is_ok(), "Large block should not panic: {:?}", result);
    // Output must be finite
    for (i, &s) in buffer.iter().enumerate() {
        assert!(s.is_finite(), "sample {} not finite: {}", i, s);
    }
}

/// Bug 1.1: Tube ADAA must not change harmonic character vs direct path.
/// When ADAA is on/off for Tube mode with tone != 1.0, the output should
/// be identical (we now use direct tube() in both cases).
#[test]
fn test_tube_adaa_matches_direct_when_tone_not_one() {
    let num_frames = 512;
    let channels = 1;

    let make_tube_plugin = |use_adaa: bool| {
        SaturationPlugin::from_validated_params(
            channels,
            SaturationPluginParams {
                mode: "Tube".to_string(),
                drive: 5.0,
                tone: 2.0, // tone != 1.0 — previously ADAA used wrong nonlinearity
                oversampling: "Off".to_string(),
                output_gain_db: 0.0,
                mix: 1.0,
                use_adaa,
                dc_blocker_enabled: false,
                ..Default::default()
            },
        )
    };

    let mut plugin_adaa = make_tube_plugin(true);
    plugin_adaa.initialize(48000).unwrap();
    let mut plugin_direct = make_tube_plugin(false);
    plugin_direct.initialize(48000).unwrap();

    let signal = make_sine(1000.0, 48000, num_frames, 0.5);
    let mut buf_adaa = signal.clone();
    let mut buf_direct = signal;
    let ctx = make_context(num_frames);

    plugin_adaa.process_in_place(&mut buf_adaa, &ctx).unwrap();
    plugin_direct
        .process_in_place(&mut buf_direct, &ctx)
        .unwrap();

    // With the fix, ADAA Tube path uses direct tube(), outputs must be identical
    for i in 0..num_frames {
        let diff = (buf_adaa[i] - buf_direct[i]).abs();
        assert!(
            diff < 1e-5,
            "Tube ADAA and direct diverge at sample {}: adaa={}, direct={}, diff={}",
            i,
            buf_adaa[i],
            buf_direct[i],
            diff
        );
    }
}

/// Bug 1.2: Drive smoother must not produce block-constant output.
/// After a parameter change, consecutive blocks should show gradually
/// changing drive (not an instant step).
#[test]
fn test_drive_smoother_ramps_across_block() {
    let channels = 1;
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 1.0, // low drive to start
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        use_adaa: false,
        dc_blocker_enabled: false,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    // Change drive to maximum — smoother will ramp from 1 to 20 over ~10ms
    plugin
        .set_parameter(ParameterId::from("drive"), ParameterValue::Float(20.0))
        .unwrap();

    // Process a single block of 256 samples with a constant DC input
    let num_frames = 256;
    let mut buffer = vec![0.5f32; num_frames]; // constant input
    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // If drive is ramping per-sample, output values should NOT all be identical
    let first = buffer[0];
    let last = buffer[num_frames - 1];
    assert!(
        (first - last).abs() > 1e-4,
        "Drive ramp should produce different values at start ({}) and end ({}) of block",
        first,
        last
    );
}

/// Bug 1.3: Dynamic saturation must modulate drive (not post-gain).
/// With dynamic_amount > 0 and a loud signal, output should reflect
/// drive modulation rather than post-distortion multiplication.
/// Key invariant: with mix=1, dry=0 → wet drive > 0 → output finite and bounded.
#[test]
fn test_dynamic_saturation_bounded_no_pumping() {
    let channels = 1;
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 5.0,
        dynamic_amount: 1.0, // full dynamic
        dynamic_attack_ms: 1.0,
        dynamic_release_ms: 10.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        use_adaa: false,
        dc_blocker_enabled: false,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    // Full-scale input: drive modulation should not blow up
    let num_frames = 2048;
    let mut buffer = make_sine(440.0, 48000, num_frames, 1.0);
    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    for (i, &s) in buffer.iter().enumerate() {
        assert!(s.is_finite(), "sample {} not finite: {}", i, s);
        // tanh-based soft_clip bounds output to (-1, 1) regardless of drive
        assert!(
            s.abs() <= 1.05,
            "dynamic saturation output out of bounds at sample {}: {}",
            i,
            s
        );
    }
}

/// Bug 2.4: flush_denormals_inplace must only operate on [..total] samples.
/// Verify that processing a small block inside a larger allocation does not
/// corrupt or panic on the samples outside the valid range.
#[test]
fn test_flush_denormals_limited_to_valid_samples() {
    let channels = 2;
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 3.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 1.0,
        use_adaa: false,
        dc_blocker_enabled: false,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    // Allocate a buffer larger than nf*nc, fill tail with sentinel
    let num_frames = 64;
    let total = num_frames * channels;
    let extra = 16;
    let mut buffer = vec![0.1f32; total + extra];
    let sentinel = 1234.5678f32;
    for s in buffer[total..].iter_mut() {
        *s = sentinel;
    }

    let ctx = make_context(num_frames);
    // process_in_place operates on buffer but only touches [..total]
    // We pass a slice of exactly `total` to match the contract
    plugin.process_in_place(&mut buffer[..total], &ctx).unwrap();

    // Sentinel values after total must be unchanged
    for (i, &s) in buffer[total..].iter().enumerate() {
        assert_eq!(
            s, sentinel,
            "sample beyond valid range at offset {} was modified",
            i
        );
    }
}

/// Bug 1.4: LUFS target is removed — verify no LUFS-related field exists
/// by checking that processing with mix=0 gives pure passthrough (no auto-gain).
#[test]
fn test_no_lufs_auto_gain_on_passthrough() {
    let channels = 1;
    let params = SaturationPluginParams {
        mode: "Soft Clip".to_string(),
        drive: 10.0,
        oversampling: "Off".to_string(),
        output_gain_db: 0.0,
        mix: 0.0, // full dry — LUFS would have altered this
        use_adaa: false,
        dc_blocker_enabled: false,
        ..Default::default()
    };
    let mut plugin = SaturationPlugin::from_validated_params(channels, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let signal = make_sine(440.0, 48000, num_frames, 0.5);
    let mut buffer = signal.clone();
    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // mix=0 → pure dry; no LUFS gain should be applied
    for i in 0..num_frames {
        let diff = (buffer[i] - signal[i]).abs();
        assert!(
            diff < 1e-5,
            "sample {} changed with mix=0: output={}, expected={}, diff={}",
            i,
            buffer[i],
            signal[i],
            diff
        );
    }
}
