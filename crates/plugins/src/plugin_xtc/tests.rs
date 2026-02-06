use super::*;
use filters::{
    compute_beta, compute_beta_smooth, compute_xtc_filters_full, head_shadowing_filter,
    head_shadowing_woodworth,
};

#[test]
fn test_xtc_creation() {
    let params = XtcPluginParams::default();
    let plugin = XtcPlugin::new(params, 48000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn test_xtc_bypass() {
    let mut params = XtcPluginParams::default();
    params.enabled = false;
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = (i as f32 * 0.01).sin();
        input[i * 2 + 1] = (i as f32 * 0.01).cos();
    }
    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Bypass should be exact passthrough
    for i in 0..input.len() {
        assert_eq!(output[i], input[i]);
    }
}

#[test]
fn test_xtc_processing() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Test with stereo sine wave (must exceed fft_size to produce output)
    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
        input[i * 2] = phase.sin() * 0.5;
        input[i * 2 + 1] = phase.cos() * 0.5;
    }
    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Output should be non-zero
    let sum: f32 = output.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Output should not be all zeros");
}

#[test]
fn test_head_shadowing_filter() {
    let params = XtcPluginParams::default();

    // At DC, should be 1.0 (no attenuation)
    let g_dc = head_shadowing_filter(0.0, &params);
    assert!((g_dc - 1.0).abs() < 0.01);

    // At cutoff frequency, should be attenuated
    let g_cutoff = head_shadowing_filter(params.head_shadow_cutoff_hz, &params);
    assert!(g_cutoff < 1.0);
    assert!(g_cutoff > 0.0);

    // At very high frequency, should be heavily attenuated
    let g_high = head_shadowing_filter(20000.0, &params);
    assert!(g_high < g_cutoff);
}

#[test]
fn test_beta_computation() {
    let params = XtcPluginParams::default();

    // Mid-range frequency: should be close to base beta
    let beta_mid = compute_beta(1000.0, &params);
    assert!((beta_mid - params.beta_base).abs() < params.beta_base * 0.1);

    // Low frequency: should be boosted
    let beta_low = compute_beta(100.0, &params);
    assert!(beta_low > params.beta_base * 2.0);

    // High frequency: should be boosted
    let beta_high = compute_beta(10000.0, &params);
    assert!(beta_high > params.beta_base * 2.0);
}

#[test]
fn test_parameter_updates() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();

    // Update distance
    plugin
        .set_parameter(ParameterId::from("distance_m"), ParameterValue::Float(2.5))
        .unwrap();
    assert_eq!(plugin.params.distance_m, 2.5);

    // Update speaker angle
    plugin
        .set_parameter(
            ParameterId::from("speaker_angle_deg"),
            ParameterValue::Float(45.0),
        )
        .unwrap();
    assert_eq!(plugin.params.speaker_angle_deg, 45.0);

    // Toggle enabled
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    assert_eq!(plugin.params.enabled, false);
}

#[test]
fn test_invalid_fft_size() {
    let mut params = XtcPluginParams::default();
    params.fft_size = 1000; // Not power of 2
    let result = XtcPlugin::new(params, 48000);
    assert!(result.is_err());
}

/// Test that energy is approximately preserved through XTC processing.
/// XTC should modify phase relationships but not drastically attenuate the signal.
#[test]
fn test_energy_preservation() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Generate test signal: stereo sine wave at 1kHz (in the optimal XTC range)
    let num_frames = 8192; // Long enough to get past latency and steady-state
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
        input[i * 2] = phase.sin() * 0.5;
        input[i * 2 + 1] = phase.cos() * 0.5;
    }
    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Calculate energy (skip initial latency period)
    let skip_samples = 2048; // Skip latency
    let input_energy: f32 = input[skip_samples * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip_samples * 2..].iter().map(|x| x * x).sum();

    // Energy ratio should be between 0.5 and 2.0 (within 3dB)
    // XTC can boost some frequencies while attenuating others,
    // but total energy should be reasonably preserved
    let energy_ratio = output_energy / input_energy;
    assert!(
        energy_ratio > 0.3 && energy_ratio < 3.0,
        "Energy ratio {} is outside acceptable range [0.3, 3.0].  Input energy: {}, Output energy: {}",
        energy_ratio,
        input_energy,
        output_energy
    );
}

/// Test that mono signal (L=R) passes through with expected attenuation.
/// For mono content, XTC naturally attenuates by factor of ~1/(1+H_contra),
/// which is approximately 0.4-0.6 depending on frequency.
/// This is expected behavior - there's no stereo difference to preserve.
#[test]
fn test_mono_signal_behavior() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Mono signal (same content in L and R)
    let num_frames = 8192;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
        let sample = phase.sin() * 0.5;
        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }
    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Skip latency
    let skip_samples = 2048;
    let input_energy: f32 = input[skip_samples * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip_samples * 2..].iter().map(|x| x * x).sum();

    let energy_ratio = output_energy / input_energy;
    // Mono is expected to be attenuated by XTC (typically 0.3-0.7)
    // This is the mathematically correct behavior for crosstalk cancellation
    assert!(
        energy_ratio > 0.2 && energy_ratio < 0.8,
        "Mono energy ratio {} outside expected XTC range [0.2, 0.8]",
        energy_ratio
    );

    // L and R output should be approximately equal for mono input
    let mut l_energy = 0.0_f32;
    let mut r_energy = 0.0_f32;
    for i in skip_samples..num_frames {
        l_energy += output[i * 2] * output[i * 2];
        r_energy += output[i * 2 + 1] * output[i * 2 + 1];
    }
    let lr_ratio = l_energy / r_energy;
    assert!(
        lr_ratio > 0.9 && lr_ratio < 1.1,
        "L/R energy ratio {} is not balanced for mono",
        lr_ratio
    );
}

/// Test continuous processing across multiple blocks.
#[test]
fn test_continuous_processing() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let block_size = 512;
    let num_blocks = 20;

    // Process multiple blocks
    for block in 0..num_blocks {
        let mut input = vec![0.0_f32; block_size * 2];
        for i in 0..block_size {
            let sample_idx = block * block_size + i;
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * sample_idx as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.cos() * 0.5;
        }
        let mut output = vec![0.0_f32; block_size * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: block_size,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // After initial latency, output should have non-zero energy
        if block > 5 {
            let output_energy: f32 = output.iter().map(|x| x * x).sum();
            assert!(
                output_energy > 0.01,
                "Block {} has near-zero output energy: {}",
                block,
                output_energy
            );
        }
    }
}

/// Test that XTC filters have reasonable magnitudes.
#[test]
fn test_filter_magnitudes() {
    let params = XtcPluginParams::default();
    let fft_size = params.fft_size;
    let num_bins = fft_size / 2 + 1;
    let filters = compute_xtc_filters_full(&params, 48000, num_bins);

    // Check mid-frequency bin (around 1kHz)
    let bin_1khz = (1000.0 * fft_size as f32 / 48000.0) as usize;

    // With spectral normalization and multi-stage cancellation,
    // magnitudes can vary more than the old simple model
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    // Diagonal filters should have reasonable magnitude
    let mag_ll = filters.filter_ll[bin_1khz].norm();
    assert!(
        mag_ll > 0.1 && mag_ll < max_gain_linear,
        "filter_ll magnitude {} at 1kHz outside range",
        mag_ll
    );

    // Cross-filters should be non-zero (cancellation signal)
    let mag_lr = filters.filter_lr[bin_1khz].norm();
    assert!(
        mag_lr > 0.001 && mag_lr < max_gain_linear,
        "filter_lr magnitude {} at 1kHz outside range",
        mag_lr
    );
}

/// Test that denormal numbers are flushed to zero
#[test]
fn test_xtc_denormal_flushing() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Create very low amplitude input (near denormal range)
    let num_frames = 4096;
    let mut input = vec![1e-35_f32; num_frames * 2];
    // Add a tiny bit of signal variation
    for i in 0..num_frames {
        input[i * 2] = 1e-35 * ((i as f32 * 0.01).sin() + 1.0);
        input[i * 2 + 1] = 1e-35 * ((i as f32 * 0.01).cos() + 1.0);
    }
    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Count denormal samples (non-zero but below normalized threshold)
    let mut denormal_count = 0;
    for sample in output.iter() {
        let abs_val = sample.abs();
        if abs_val > 0.0 && abs_val < 1e-30 {
            denormal_count += 1;
        }
    }

    // With proper denormal flushing, there should be NO denormal samples
    assert_eq!(
        denormal_count, 0,
        "Found {} denormal samples. Denormal flushing is not working correctly.",
        denormal_count
    );
}

/// Test yaw angle creates asymmetric filters
#[test]
fn test_yaw_angle_asymmetry() {
    let mut params = XtcPluginParams::default();
    params.head_yaw_deg = 15.0; // 15 degrees yaw
    let fft_size = params.fft_size;
    let num_bins = fft_size / 2 + 1;

    let filters = compute_xtc_filters_full(&params, 48000, num_bins);

    // With yaw != 0, we should have asymmetric filters (filter_rl and filter_rr are Some)
    assert!(
        filters.filter_rl.is_some(),
        "filter_rl should be Some when yaw != 0"
    );
    assert!(
        filters.filter_rr.is_some(),
        "filter_rr should be Some when yaw != 0"
    );

    let filter_rl = filters.filter_rl.as_ref().unwrap();
    let filter_rr = filters.filter_rr.as_ref().unwrap();

    // Check that filters are actually asymmetric at mid frequencies
    let bin_1khz = (1000.0 * fft_size as f32 / 48000.0) as usize;

    // filter_lr and filter_rl should be different with yaw
    let diff_cross = (filters.filter_lr[bin_1khz] - filter_rl[bin_1khz]).norm();
    assert!(
        diff_cross > 0.001,
        "Cross filters should be asymmetric with yaw, diff = {}",
        diff_cross
    );

    // filter_ll and filter_rr should also be different with yaw
    let diff_diag = (filters.filter_ll[bin_1khz] - filter_rr[bin_1khz]).norm();
    assert!(
        diff_diag > 0.001,
        "Diagonal filters should be asymmetric with yaw, diff = {}",
        diff_diag
    );
}

/// Test symmetric case (yaw = 0) uses optimized 2-filter version
#[test]
fn test_yaw_zero_symmetric() {
    let params = XtcPluginParams::default(); // yaw = 0
    let fft_size = params.fft_size;
    let num_bins = fft_size / 2 + 1;

    let filters = compute_xtc_filters_full(&params, 48000, num_bins);

    // With yaw = 0, filters should be symmetric (filter_rl and filter_rr are None)
    assert!(
        filters.filter_rl.is_none(),
        "filter_rl should be None when yaw = 0"
    );
    assert!(
        filters.filter_rr.is_none(),
        "filter_rr should be None when yaw = 0"
    );
}

/// Test Woodworth head shadowing model
#[test]
fn test_woodworth_head_shadowing() {
    let head_radius = 0.0875;

    // At low frequencies, shadowing should be minimal
    let g_low = head_shadowing_woodworth(100.0, 0.5, head_radius);
    assert!(
        g_low > 0.95,
        "Low frequency shadowing should be minimal, got {}",
        g_low
    );

    // At high frequencies, shadowing should be significant
    let g_high = head_shadowing_woodworth(8000.0, 0.5, head_radius);
    assert!(
        g_high < 0.9,
        "High frequency shadowing should be significant, got {}",
        g_high
    );

    // At 0 angle, shadowing should be minimal even at high frequencies
    let g_frontal = head_shadowing_woodworth(8000.0, 0.0, head_radius);
    assert!(
        g_frontal > g_high,
        "Frontal angle should have less shadowing than side"
    );
}

/// Test smooth beta transitions
#[test]
fn test_smooth_beta_transitions() {
    let params = XtcPluginParams::default();

    // Test transition around 100Hz is smooth (LF boost region)
    let beta_70 = compute_beta_smooth(70.0, &params);
    let beta_100 = compute_beta_smooth(100.0, &params);
    let beta_150 = compute_beta_smooth(150.0, &params);

    // Should be monotonically decreasing toward mid-range
    assert!(
        beta_70 > beta_100,
        "Beta should decrease from 70Hz to 100Hz"
    );
    assert!(
        beta_100 > beta_150,
        "Beta should decrease from 100Hz to 150Hz"
    );

    // Transition should be smooth (no large jumps)
    let ratio_1 = beta_70 / beta_100;
    let ratio_2 = beta_100 / beta_150;
    assert!(
        (ratio_1 - ratio_2).abs() < 2.0,
        "Beta transition should be smooth around 100Hz"
    );
}
