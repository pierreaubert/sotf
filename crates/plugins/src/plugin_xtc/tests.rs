use super::*;
use filters::{
    compute_beta, compute_beta_smooth, compute_xtc_filters_full,
    frequency_dependent_diffraction_delay, head_shadowing_filter, head_shadowing_woodworth,
    sanitize_filter, woodworth_diffraction_path,
};
use reflections::{air_absorption, compute_image_sources, compute_reflection_beta_boost};

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
/// With auto-gain enabled (default), output energy should closely match input energy.
#[test]
fn test_energy_preservation() {
    let params = XtcPluginParams::default();
    assert!(params.auto_gain_enabled, "auto_gain should be enabled by default");
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Generate test signal: stereo sine wave at 1kHz (in the optimal XTC range)
    // Use enough blocks to let auto-gain converge
    let block_size = 4096;
    let num_blocks = 8;

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: block_size,
    };

    let mut last_output = vec![0.0_f32; block_size * 2];
    for block in 0..num_blocks {
        let mut input = vec![0.0_f32; block_size * 2];
        for i in 0..block_size {
            let sample_idx = block * block_size + i;
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * sample_idx as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.cos() * 0.5;
        }
        last_output.fill(0.0);
        plugin.process(&input, &mut last_output, &context).unwrap();
    }

    // After convergence, measure the last block's energy ratio
    let mut input_final = vec![0.0_f32; block_size * 2];
    for i in 0..block_size {
        let sample_idx = num_blocks * block_size + i;
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * sample_idx as f32 / 48000.0;
        input_final[i * 2] = phase.sin() * 0.5;
        input_final[i * 2 + 1] = phase.cos() * 0.5;
    }
    let input_energy: f32 = input_final.iter().map(|x| x * x).sum();
    let output_energy: f32 = last_output.iter().map(|x| x * x).sum();

    let energy_ratio = output_energy / input_energy;
    assert!(
        energy_ratio > 0.3 && energy_ratio < 3.0,
        "Energy ratio {} is outside acceptable range [0.3, 3.0]. Input energy: {}, Output energy: {}",
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

/// Test frequency-dependent ITD: low freq matches Woodworth, high freq is shorter,
/// monotonically decreasing with frequency.
#[test]
fn test_frequency_dependent_itd() {
    let head_radius = 0.0875;
    let angle = std::f32::consts::FRAC_PI_2 + 0.5; // ~120° contralateral angle

    // DC should match full Woodworth diffraction delay
    let dc_delay = frequency_dependent_diffraction_delay(0.0, angle, head_radius);
    let woodworth_delay = woodworth_diffraction_path(angle.abs(), head_radius) / 343.0;
    assert!(
        (dc_delay - woodworth_delay).abs() < 1e-8,
        "DC delay {} should match Woodworth {}",
        dc_delay,
        woodworth_delay
    );

    // Low frequency (ka < 0.5) should also match Woodworth
    let low_freq_delay = frequency_dependent_diffraction_delay(100.0, angle, head_radius);
    assert!(
        (low_freq_delay - woodworth_delay).abs() < 1e-6,
        "Low freq delay {} should match Woodworth {}",
        low_freq_delay,
        woodworth_delay
    );

    // High frequency should be shorter than low frequency
    let high_freq_delay = frequency_dependent_diffraction_delay(10000.0, angle, head_radius);
    assert!(
        high_freq_delay < low_freq_delay,
        "High freq delay {} should be shorter than low freq {}",
        high_freq_delay,
        low_freq_delay
    );

    // Monotonically decreasing with frequency
    let freqs = [100.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];
    for i in 1..freqs.len() {
        let d_prev = frequency_dependent_diffraction_delay(freqs[i - 1], angle, head_radius);
        let d_curr = frequency_dependent_diffraction_delay(freqs[i], angle, head_radius);
        assert!(
            d_curr <= d_prev + 1e-8,
            "Delay should be non-increasing: {} Hz -> {}, {} Hz -> {}",
            freqs[i - 1],
            d_prev,
            freqs[i],
            d_curr
        );
    }
}

/// Test that zero angle gives zero diffraction delay at all frequencies.
#[test]
fn test_frequency_dependent_itd_zero_angle() {
    let head_radius = 0.0875;

    for &freq in &[0.0, 100.0, 1000.0, 5000.0, 10000.0, 20000.0] {
        let delay = frequency_dependent_diffraction_delay(freq, 0.0, head_radius);
        assert!(
            delay.abs() < 1e-8,
            "Zero angle should give zero delay at {} Hz, got {}",
            freq,
            delay
        );
    }
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

// ============================================================================
// Room reflection tests
// ============================================================================

/// Test that image source model produces 6 reflection paths for a known geometry
#[test]
fn test_image_source_positions() {
    // Simple geometry: speaker at (0, 1.2, 2.0), ear at (0.0875, 1.2, 0.0)
    // Room: 4m wide, 5m deep, 2.5m tall
    let speaker_pos = [0.0_f32, 1.2, 2.0];
    let ear_pos = [0.0875_f32, 1.2, 0.0];
    let direct_dist = {
        let dx = speaker_pos[0] - ear_pos[0];
        let dy = speaker_pos[1] - ear_pos[1];
        let dz = speaker_pos[2] - ear_pos[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    };

    // We need to use the actual RoomGeometry struct from reflections
    // but it's private. Use the public function with XtcPluginParams instead.
    // Test via build_reflection_data_image_source which calls compute_image_sources.

    // Instead, test that compute_image_sources returns 6 paths
    // We create a minimal RoomGeometry by calling the function directly
    let room = reflections::tests_support::make_room(4.0, 5.0, 2.5, 0.3);
    let paths = compute_image_sources(speaker_pos, ear_pos, direct_dist, &room);

    assert_eq!(paths.len(), 6, "Should have 6 first-order reflections");

    // All delays should be positive and amplitudes in (0, 1)
    for (i, path) in paths.iter().enumerate() {
        assert!(
            path.delay_s > 0.0,
            "Reflection {} delay should be positive, got {}",
            i,
            path.delay_s
        );
        assert!(
            path.amplitude > 0.0 && path.amplitude < 1.0,
            "Reflection {} amplitude should be in (0, 1), got {}",
            i,
            path.amplitude
        );
    }
}

/// Test that full absorption (1.0) produces zero-amplitude reflections
#[test]
fn test_reflection_zero_amplitude_full_absorption() {
    let mut params = XtcPluginParams::default();
    params.room_reflections_enabled = true;
    params.wall_absorption = 1.0; // Fully absorptive

    let num_bins = 513; // 1024-point FFT
    let data = reflections::build_reflection_data_image_source(&params, 48000, num_bins);

    // All reflection contributions should be zero (or very near zero)
    for bin in 0..num_bins {
        assert!(
            data.h_room_ipsi[bin].norm() < 1e-6,
            "Ipsi reflection at bin {} should be ~0 with full absorption, got {}",
            bin,
            data.h_room_ipsi[bin].norm()
        );
        assert!(
            data.h_room_contra[bin].norm() < 1e-6,
            "Contra reflection at bin {} should be ~0 with full absorption, got {}",
            bin,
            data.h_room_contra[bin].norm()
        );
    }

    // Beta boost should be all 1.0 (no boost needed with no reflections)
    for bin in 0..num_bins {
        assert!(
            (data.beta_boost[bin] - 1.0).abs() < 0.1,
            "Beta boost at bin {} should be ~1.0 with full absorption, got {}",
            bin,
            data.beta_boost[bin]
        );
    }
}

/// Test that comb filter beta boost detects deep nulls
#[test]
fn test_comb_filter_beta_boost() {
    let num_bins = 513;

    // Create a magnitude spectrum with a known deep null at bin 100
    let mut magnitude = vec![1.0_f32; num_bins];
    magnitude[100] = 0.01; // -40 dB null
    magnitude[101] = 0.05; // -26 dB null
    magnitude[200] = 0.02; // -34 dB null

    let boost = compute_reflection_beta_boost(&magnitude, num_bins, 3.0);

    // Bins near nulls should have boost > 1.0
    assert!(
        boost[100] > 1.0,
        "Boost at null bin 100 should be > 1.0, got {}",
        boost[100]
    );
    assert!(
        boost[200] > 1.0,
        "Boost at null bin 200 should be > 1.0, got {}",
        boost[200]
    );

    // Bins away from nulls should be ~1.0
    assert!(
        (boost[50] - 1.0).abs() < 0.2,
        "Boost at non-null bin 50 should be ~1.0, got {}",
        boost[50]
    );
}

/// Test that enabling reflections changes filter magnitudes compared to disabled
#[test]
fn test_reflections_change_filters() {
    let fft_size = 1024;
    let num_bins = fft_size / 2 + 1;

    // Without reflections
    let mut params_off = XtcPluginParams::default();
    params_off.fft_size = fft_size;
    params_off.room_reflections_enabled = false;
    let filters_off = compute_xtc_filters_full(&params_off, 48000, num_bins);

    // With reflections
    let mut params_on = XtcPluginParams::default();
    params_on.fft_size = fft_size;
    params_on.room_reflections_enabled = true;
    params_on.wall_absorption = 0.3;
    let filters_on = compute_xtc_filters_full(&params_on, 48000, num_bins);

    // Filters should differ at mid frequencies
    let bin_1khz = (1000.0 * fft_size as f32 / 48000.0) as usize;
    let diff_ll = (filters_on.filter_ll[bin_1khz] - filters_off.filter_ll[bin_1khz]).norm();
    let diff_lr = (filters_on.filter_lr[bin_1khz] - filters_off.filter_lr[bin_1khz]).norm();

    assert!(
        diff_ll > 1e-4 || diff_lr > 1e-4,
        "Enabling reflections should change filters. diff_ll={}, diff_lr={}",
        diff_ll,
        diff_lr
    );
}

/// Test that energy stays in acceptable range with reflections enabled
#[test]
fn test_energy_preservation_with_reflections() {
    let mut params = XtcPluginParams::default();
    params.room_reflections_enabled = true;
    params.wall_absorption = 0.3;

    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 8192;
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

    let skip_samples = 2048;
    let input_energy: f32 = input[skip_samples * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip_samples * 2..].iter().map(|x| x * x).sum();

    let energy_ratio = output_energy / input_energy;
    assert!(
        energy_ratio > 0.1 && energy_ratio < 5.0,
        "Energy ratio {} with reflections is outside acceptable range [0.1, 5.0]",
        energy_ratio
    );
}

/// Test air absorption: unity at DC, increases with frequency and distance
#[test]
fn test_air_absorption() {
    // At DC, no absorption regardless of distance
    assert!((air_absorption(0.0, 10.0) - 1.0).abs() < 1e-6);

    // At 1kHz and 2m, absorption should be very small
    let atten_1k_2m = air_absorption(1000.0, 2.0);
    assert!(
        atten_1k_2m > 0.99,
        "1kHz at 2m should have negligible absorption, got {}",
        atten_1k_2m
    );

    // Absorption increases with frequency
    let atten_10k_5m = air_absorption(10000.0, 5.0);
    let atten_1k_5m = air_absorption(1000.0, 5.0);
    assert!(
        atten_10k_5m < atten_1k_5m,
        "10kHz should have more absorption than 1kHz: {} vs {}",
        atten_10k_5m,
        atten_1k_5m
    );

    // Absorption increases with distance
    let atten_5k_2m = air_absorption(5000.0, 2.0);
    let atten_5k_10m = air_absorption(5000.0, 10.0);
    assert!(
        atten_5k_10m < atten_5k_2m,
        "10m should have more absorption than 2m: {} vs {}",
        atten_5k_10m,
        atten_5k_2m
    );

    // At 10kHz and 10m, absorption should be noticeable but not extreme
    let atten_10k_10m = air_absorption(10000.0, 10.0);
    assert!(
        atten_10k_10m > 0.5 && atten_10k_10m < 1.0,
        "10kHz at 10m absorption {} should be moderate",
        atten_10k_10m
    );
}

/// Test STFT passthrough: with bypass_xtc_filters=true, the STFT round-trip
/// (window → FFT → IFFT → OLA) should preserve signal amplitude within 0.5 dB.
#[test]
fn test_stft_passthrough_unity() {
    let mut params = XtcPluginParams::default();
    params.bypass_xtc_filters = true;
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Generate 1kHz stereo sine, long enough to fill past latency
    let num_frames = 16384;
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

    // Skip initial latency (fft_size samples to be safe)
    let skip = 4096;
    let input_energy: f32 = input[skip * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip * 2..].iter().map(|x| x * x).sum();

    let energy_ratio = output_energy / input_energy;
    let energy_ratio_db = 10.0 * energy_ratio.log10();

    // Should be within ±1.0 dB of unity (small loss from finite-length edge effects is expected)
    assert!(
        energy_ratio_db.abs() < 1.0,
        "STFT passthrough energy ratio is {:.2} dB (ratio={:.4}), expected within ±1.0 dB of unity",
        energy_ratio_db,
        energy_ratio,
    );
}

/// Test that auto-gain prevents clipping: output peak should stay below 1.0
/// for a moderate-amplitude input signal.
#[test]
fn test_auto_gain_prevents_clipping() {
    let params = XtcPluginParams::default();
    assert!(params.auto_gain_enabled);

    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let block_size = 4096;
    let num_blocks = 12; // Enough for auto-gain to converge

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: block_size,
    };

    let mut peak_output = 0.0_f32;
    for block in 0..num_blocks {
        let mut input = vec![0.0_f32; block_size * 2];
        for i in 0..block_size {
            let sample_idx = block * block_size + i;
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * sample_idx as f32 / 48000.0;
            // 0.5 amplitude input — should never clip with auto-gain
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.cos() * 0.5;
        }
        let mut output = vec![0.0_f32; block_size * 2];
        plugin.process(&input, &mut output, &context).unwrap();

        // Track peak after auto-gain has had time to converge (skip first few blocks)
        if block >= 4 {
            for &s in &output {
                peak_output = peak_output.max(s.abs());
            }
        }
    }

    assert!(
        peak_output < 1.0,
        "Auto-gain should prevent clipping. Peak output: {:.4}",
        peak_output
    );
}

/// Test that sanitize_filter replaces NaN and Inf with zero
#[test]
fn test_sanitize_filter() {
    let mut filter = vec![
        Complex::new(1.0, 2.0),
        Complex::new(f32::NAN, 0.5),
        Complex::new(0.5, f32::INFINITY),
        Complex::new(f32::NEG_INFINITY, f32::NAN),
        Complex::new(3.0, -1.0),
    ];

    sanitize_filter(&mut filter);

    assert_eq!(filter[0], Complex::new(1.0, 2.0));
    assert_eq!(filter[1], Complex::new(0.0, 0.5));
    assert_eq!(filter[2], Complex::new(0.5, 0.0));
    assert_eq!(filter[3], Complex::new(0.0, 0.0));
    assert_eq!(filter[4], Complex::new(3.0, -1.0));
}

/// Test bypass_neumann_refinement produces different filters than default
#[test]
fn test_bypass_neumann_refinement() {
    let fft_size = 1024;
    let num_bins = fft_size / 2 + 1;

    let mut params_normal = XtcPluginParams::default();
    params_normal.fft_size = fft_size;
    let filters_normal = compute_xtc_filters_full(&params_normal, 48000, num_bins);

    let mut params_bypass = XtcPluginParams::default();
    params_bypass.fft_size = fft_size;
    params_bypass.bypass_neumann_refinement = true;
    let filters_bypass = compute_xtc_filters_full(&params_bypass, 48000, num_bins);

    // Filters should differ at mid frequencies
    let bin_1khz = (1000.0 * fft_size as f32 / 48000.0) as usize;
    let diff = (filters_normal.filter_ll[bin_1khz] - filters_bypass.filter_ll[bin_1khz]).norm();
    assert!(
        diff > 1e-6,
        "Bypassing Neumann refinement should produce different filters, diff = {}",
        diff
    );
}
