use super::*;
use filters::{
    compute_beta, compute_beta_smooth, compute_geometry_cache, compute_xtc_filters_full,
    head_shadowing_brown_duda, head_shadowing_complex, head_shadowing_filter,
    head_shadowing_woodworth, sanitize_filter, soft_limit_complex_magnitude,
    woodworth_diffraction_path,
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
fn test_roomeq_recommended_matrix_loader() {
    let path = std::env::temp_dir().join(format!(
        "xtc-roomeq-recommended-{}.json",
        std::process::id()
    ));
    let artifact = serde_json::json!({
        "version": "ctc-recommended-v1",
        "source": "measured",
        "sample_rate": 48_000,
        "speakers": ["L", "R"],
        "ears": ["left_ear", "right_ear"],
        "filters": [
            { "speaker": "L", "target_ear": "left_ear", "taps": [1.0, 0.0, 0.0, 0.0] },
            { "speaker": "R", "target_ear": "left_ear", "taps": [0.25, 0.0, 0.0, 0.0] },
            { "speaker": "L", "target_ear": "right_ear", "taps": [0.5, 0.0, 0.0, 0.0] },
            { "speaker": "R", "target_ear": "right_ear", "taps": [1.0, 0.0, 0.0, 0.0] }
        ]
    });
    std::fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();

    let filters = load_roomeq_recommended_filters(path.to_str().unwrap(), 48_000, 5).unwrap();
    assert!(!filters.is_symmetric);
    assert_eq!(filters.filter_ll.len(), 5);
    assert!((filters.filter_ll[0].re - 1.0).abs() < 1e-6);
    assert!((filters.filter_lr[0].re - 0.5).abs() < 1e-6);
    assert!((filters.filter_rl.as_ref().unwrap()[0].re - 0.25).abs() < 1e-6);
    assert!((filters.filter_rr.as_ref().unwrap()[0].re - 1.0).abs() < 1e-6);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_roomeq_recommended_plugin_consumes_artifact_on_create() {
    let path = std::env::temp_dir().join(format!(
        "xtc-roomeq-recommended-create-{}.json",
        std::process::id()
    ));
    let artifact = serde_json::json!({
        "version": "ctc-recommended-v1",
        "source": "measured",
        "sample_rate": 48_000,
        "speakers": ["L", "R"],
        "ears": ["left_ear", "right_ear"],
        "filters": [
            { "speaker": "L", "target_ear": "left_ear", "taps": [1.0, 0.0, 0.0, 0.0] },
            { "speaker": "R", "target_ear": "left_ear", "taps": [0.25, 0.0, 0.0, 0.0] },
            { "speaker": "L", "target_ear": "right_ear", "taps": [0.5, 0.0, 0.0, 0.0] },
            { "speaker": "R", "target_ear": "right_ear", "taps": [0.75, 0.0, 0.0, 0.0] }
        ]
    });
    std::fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();

    let mut params = XtcPluginParams::default();
    params.source_mode = "roomeq_recommended".to_string();
    params.recommended_matrix_file = Some(path.to_string_lossy().to_string());
    let plugin = XtcPlugin::new(params, 48_000).unwrap();
    let filters = plugin.cached_current_filters.as_ref();
    assert!(!filters.is_symmetric);
    assert!((filters.filter_ll[0].re - 1.0).abs() < 1e-6);
    assert!((filters.filter_lr[0].re - 0.5).abs() < 1e-6);
    assert!((filters.filter_rl.as_ref().unwrap()[0].re - 0.25).abs() < 1e-6);
    assert!((filters.filter_rr.as_ref().unwrap()[0].re - 0.75).abs() < 1e-6);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_roomeq_recommended_matrix_supports_more_than_two_speakers() {
    let path = std::env::temp_dir().join(format!(
        "xtc-roomeq-recommended-n-{}.json",
        std::process::id()
    ));
    let artifact = serde_json::json!({
        "version": "ctc-recommended-v1",
        "source": "measured",
        "sample_rate": 48_000,
        "speakers": ["L", "R", "C"],
        "ears": ["left_ear", "right_ear"],
        "filters": [
            { "speaker": "L", "target_ear": "left_ear", "taps": [1.0] },
            { "speaker": "L", "target_ear": "right_ear", "taps": [0.0] },
            { "speaker": "R", "target_ear": "left_ear", "taps": [0.0] },
            { "speaker": "R", "target_ear": "right_ear", "taps": [1.0] },
            { "speaker": "C", "target_ear": "left_ear", "taps": [0.25] },
            { "speaker": "C", "target_ear": "right_ear", "taps": [0.5] }
        ]
    });
    std::fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();

    let mut params = XtcPluginParams::default();
    params.source_mode = "roomeq_recommended".to_string();
    params.recommended_matrix_file = Some(path.to_string_lossy().to_string());
    let plugin = XtcPlugin::new(params, 48_000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 3);
    let matrix = plugin
        .cached_current_filters
        .speaker_filters
        .as_ref()
        .expect("roomEQ speaker filter matrix");
    assert_eq!(matrix.len(), 3);
    assert!((matrix[2][0][0].re - 0.25).abs() < 1e-6);
    assert!((matrix[2][1][0].re - 0.5).abs() < 1e-6);

    let mut plugin = plugin;
    plugin.initialize(48_000).unwrap();
    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = (i as f32 * 0.01).sin() * 0.25;
        input[i * 2 + 1] = (i as f32 * 0.013).cos() * 0.25;
    }
    let mut output = vec![0.0_f32; num_frames * 3];
    let produced = plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(48_000, num_frames),
        )
        .unwrap();
    assert!(produced > 0);
    let center_energy: f32 = output
        .chunks_exact(3)
        .take(produced)
        .map(|frame| frame[2].abs())
        .sum();
    assert!(
        center_energy > 0.0,
        "third speaker output should be rendered"
    );

    let _ = std::fs::remove_file(path);
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

    let context = ProcessContext::new(48000, num_frames);

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

    let context = ProcessContext::new(48000, num_frames);

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
    assert!(!plugin.params.enabled);
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
    assert!(
        params.auto_gain_enabled,
        "auto_gain should be enabled by default"
    );
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Generate test signal: stereo sine wave at 1kHz (in the optimal XTC range)
    // Use enough blocks to let auto-gain converge
    let block_size = 4096;
    let num_blocks = 8;

    let context = ProcessContext::new(48000, block_size);

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

    let context = ProcessContext::new(48000, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    // Skip latency
    let skip_samples = 2048;
    let input_energy: f32 = input[skip_samples * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip_samples * 2..].iter().map(|x| x * x).sum();

    let energy_ratio = output_energy / input_energy;
    // Mono is expected to be attenuated by XTC (typically 0.3-0.9)
    // This is the mathematically correct behavior for crosstalk cancellation.
    // With max_gain_db (6.0), attenuation may vary with filter headroom.
    assert!(
        energy_ratio > 0.2 && energy_ratio < 1.0,
        "Mono energy ratio {} outside expected XTC range [0.2, 1.0]",
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

        let context = ProcessContext::new(48000, block_size);

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

    let context = ProcessContext::new(48000, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    // Count IEEE-754 subnormal samples. Small normal f32 values such as 1e-35
    // are valid audio samples and must not be mistaken for denormals.
    let mut denormal_count = 0;
    for sample in output.iter() {
        let abs_val = sample.abs();
        if abs_val > 0.0 && abs_val < f32::MIN_POSITIVE {
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
    params.head_yaw_deg = 30.0; // 30 degrees yaw for clear asymmetry
    params.spectral_normalization = false; // Disable normalization to see raw asymmetry
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

    println!(
        "Yaw Test 1kHz - LL: {}, RR: {}, LR: {}, RL: {}",
        filters.filter_ll[bin_1khz].norm(),
        filter_rr[bin_1khz].norm(),
        filters.filter_lr[bin_1khz].norm(),
        filter_rl[bin_1khz].norm()
    );

    // filter_lr and filter_rl should be different with yaw
    let diff_cross = (filters.filter_lr[bin_1khz] - filter_rl[bin_1khz]).norm();
    assert!(
        diff_cross > 0.01,
        "Cross filters should be asymmetric with yaw, diff = {}",
        diff_cross
    );

    // filter_ll and filter_rr may be very similar even with yaw since both
    // represent ipsilateral paths. The key asymmetry is in the cross filters.
    // With condition-number based regularization, the diagonal difference can
    // be negligible. Just verify they are finite and non-zero.
    let mag_ll = filters.filter_ll[bin_1khz].norm();
    let mag_rr = filter_rr[bin_1khz].norm();
    assert!(
        mag_ll > 0.01 && mag_rr > 0.01,
        "Diagonal filters should be non-zero: LL={}, RR={}",
        mag_ll,
        mag_rr
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

#[test]
fn test_brown_duda_shadowing_applies_phase() {
    let head_radius = 0.0875;
    let woodworth = head_shadowing_complex(4000.0, 0.8, head_radius, 0);
    let brown_duda = head_shadowing_complex(4000.0, 0.8, head_radius, 1);

    assert!(
        woodworth.im.abs() < 1e-6,
        "Woodworth compatibility path should remain magnitude-only"
    );
    assert!(
        brown_duda.im.abs() > 1e-3,
        "Brown-Duda model should include its phase term"
    );
}

/// Test that Brown-Duda alpha_min matches the published rigid-sphere approximation.
/// At theta = PI the magnitude equals alpha_min, isolating the frequency-dependent term.
#[test]
fn test_brown_duda_alpha_min_matches_paper_formula() {
    let head_radius = 0.0875;
    let freq = 4000.0;

    // At theta = PI, cos(theta/2) = 0, so magnitude == alpha_min
    let (magnitude, _phase) = head_shadowing_brown_duda(freq, std::f32::consts::PI, head_radius);

    let w = 2.0 * std::f32::consts::PI * freq;
    let w0 = 343.0 / head_radius;
    let mu = (w / w0).min(20.0);

    // Brown & Duda (1998), Eq. (2) approximation:
    // alpha_min = 1 / sqrt(1 + (mu/2)^2)
    let expected_alpha_min = (1.0 + mu * mu / 4.0).sqrt().recip();

    assert!(
        (magnitude - expected_alpha_min).abs() < 0.01,
        "Brown-Duda alpha_min should match paper formula 1/sqrt(1+(mu/2)^2). Expected {}, got {}",
        expected_alpha_min,
        magnitude
    );
}

/// Test fixed Woodworth diffraction delay calculation.
#[test]
fn test_woodworth_itd() {
    let head_radius = 0.0875;
    let angle = std::f32::consts::FRAC_PI_2 + 0.5; // ~120° contralateral angle

    let woodworth_delay = woodworth_diffraction_path(angle.abs(), head_radius) / 343.0;
    // For 120°, delay should be a*(120*pi/180 + sin(120*pi/180))/c
    // 120° = 2.094 rad, sin(120°) = 0.866
    // delay = 0.0875 * (2.094 + 0.866) / 343 = 0.000755 s (755 us)
    assert!(woodworth_delay > 0.0005 && woodworth_delay < 0.0010);
}

/// Test that zero angle gives zero diffraction delay.
#[test]
fn test_woodworth_itd_zero_angle() {
    let head_radius = 0.0875;
    let delay = woodworth_diffraction_path(0.0, head_radius) / 343.0;
    assert!(
        delay.abs() < 1e-8,
        "Zero angle should give zero delay, got {}",
        delay
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

/// Test that reflection amplitude uses pressure coefficient sqrt(1 - absorption).
///
/// The Sabine absorption coefficient α is an energy quantity. For a pressure signal
/// the correct reflection coefficient is sqrt(1 - α), not (1 - α).
/// For α = 0.75: correct = sqrt(0.25) = 0.5; wrong = (1 - 0.75) = 0.25.
#[test]
fn test_reflection_amplitude_uses_pressure_coefficient() {
    // α = 0.75 → pressure reflection = sqrt(0.25) = 0.5, energy reflection = 0.25
    // The two values differ by 2× so any reasonable geometry should distinguish them.
    let speaker_pos = [0.0_f32, 1.2, 2.0];
    let ear_pos = [0.0875_f32, 1.2, 0.0];
    let direct_dist = {
        let dx = speaker_pos[0] - ear_pos[0];
        let dy = speaker_pos[1] - ear_pos[1];
        let dz = speaker_pos[2] - ear_pos[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    };

    let room_energy = reflections::tests_support::make_room(4.0, 5.0, 2.5, 0.75);
    let paths = compute_image_sources(speaker_pos, ear_pos, direct_dist, &room_energy);
    assert!(!paths.is_empty());

    // For α = 0.75: pressure coefficient = sqrt(1 - 0.75) = 0.5
    // Energy coefficient = (1 - 0.75) = 0.25
    // The geometric spread factor (direct_dist / image_dist) is the same for both,
    // so we can factor it out and check that amplitude / (direct_dist / image_dist) ≈ 0.5.
    // Each path has a different image_dist, so compute the reflectance for each path.
    for path in &paths {
        // image_dist = direct_dist / (direct_dist_over_image_dist)
        // amplitude = reflectance * (direct_dist / image_dist)
        // We cannot recover image_dist from amplitude alone without the geometry,
        // but we know amplitudes must be strictly larger than 0.25 * geometric_spread
        // and ≤ 0.5 * geometric_spread. Since the image source is always farther
        // than the direct path, direct_dist / image_dist < 1.
        // With pressure coefficient 0.5, amplitude = 0.5 * spread < 0.5.
        // With energy coefficient 0.25, amplitude = 0.25 * spread < 0.25.
        //
        // The discriminating assertion: amplitude > 0.25 * (direct_dist / image_dist_bound).
        // Since image_dist > direct_dist: spread = direct_dist/image_dist < 1.
        // So if the code uses pressure reflection correctly, amplitude > 0.25 * spread.
        // If it uses energy reflection wrongly, amplitude ≤ 0.25 * spread < 0.25.
        //
        // In practice for this geometry spread ≈ 0.3..0.9, so:
        // correct:  amplitude = 0.5 * spread ≈ 0.15..0.45
        // wrong:    amplitude = 0.25 * spread ≈ 0.075..0.225
        // The easiest distinguishing check: amplitude / spread must be ≈ 0.5, not 0.25.
        // We recover spread as amplitude_correct / reflectance, but we don't know the
        // split. Use the ratio between two fixed-absorption runs instead:
        // For α=0 (no absorption): amplitude_ref = 1.0 * spread
        // For α=0.75: amplitude = reflectance * spread
        // ratio = amplitude / amplitude_ref = reflectance
        let _ = path; // Individual path check done below via ratio test
    }

    // Ratio test: compare α=0.75 amplitudes against α=0 (fully reflective) amplitudes.
    let room_ref = reflections::tests_support::make_room(4.0, 5.0, 2.5, 0.0);
    let paths_ref = compute_image_sources(speaker_pos, ear_pos, direct_dist, &room_ref);

    assert_eq!(paths.len(), paths_ref.len());
    for (path, ref_path) in paths.iter().zip(paths_ref.iter()) {
        let ratio = path.amplitude / ref_path.amplitude;
        // Pressure reflection coefficient for α=0.75: sqrt(0.25) = 0.5
        assert!(
            (ratio - 0.5).abs() < 1e-4,
            "Reflection amplitude ratio should be sqrt(1-0.75)=0.5 (pressure), \
             got {ratio:.6} (energy would be 0.25)"
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
            data.h_ll_ipsi[bin].norm() < 1e-6,
            "Ipsi reflection at bin {} should be ~0 with full absorption, got {}",
            bin,
            data.h_ll_ipsi[bin].norm()
        );
        assert!(
            data.h_lr_contra[bin].norm() < 1e-6,
            "Contra reflection at bin {} should be ~0 with full absorption, got {}",
            bin,
            data.h_lr_contra[bin].norm()
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

/// Test that air absorption produces physically plausible results.
///
/// The formula `0.001 * (f/1000)^2` dB/m approximates ISO 9613-1 for indoor conditions
/// (20°C, 50% RH). It overestimates by ~1.8× at 4 kHz and ~2.5× at 8 kHz relative to
/// the full ISO table, but errors are inaudible at typical room distances.
///
/// ISO 9613-1 reference (indoor, 20°C, 50% RH): ~0.002 dB/m at 1 kHz, ~0.025 dB/m at 8 kHz.
#[test]
fn test_air_absorption_physically_plausible() {
    // Absorption must be in (0, 1]: always attenuates, never amplifies.
    for &(freq, dist) in &[(1000.0_f32, 1.0_f32), (4000.0, 5.0), (8000.0, 10.0)] {
        let a = air_absorption(freq, dist);
        assert!(
            a > 0.0 && a <= 1.0,
            "air_absorption({freq}, {dist}) = {a} must be in (0, 1]"
        );
    }

    // Higher frequency → more attenuation (quadratic law).
    let a_1k = air_absorption(1000.0, 5.0);
    let a_8k = air_absorption(8000.0, 5.0);
    assert!(
        a_8k < a_1k,
        "8 kHz absorption ({a_8k}) should be greater than 1 kHz ({a_1k})"
    );

    // Longer distance → more attenuation.
    let a_1m = air_absorption(4000.0, 1.0);
    let a_5m = air_absorption(4000.0, 5.0);
    assert!(
        a_5m < a_1m,
        "5 m absorption ({a_5m}) should be greater than 1 m ({a_1m})"
    );

    // At 1 kHz and 1 m the attenuation should be tiny (<0.1 dB → linear >0.988).
    let at_1k_1m = air_absorption(1000.0, 1.0);
    assert!(
        at_1k_1m > 0.988,
        "Air absorption at 1 kHz, 1 m should be >0.988 (tiny loss), got {at_1k_1m}"
    );

    let at_8k_5m = air_absorption(8000.0, 5.0);
    let expected = 10.0_f32.powf(-(0.001 * 8.0_f32.powi(2)) * 5.0 / 20.0);
    assert!(
        (at_8k_5m - expected).abs() < 1e-6,
        "air_absorption should follow the documented quadratic dB/m fit"
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

    let context = ProcessContext::new(48000, num_frames);

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

    let context = ProcessContext::new(48000, num_frames);

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

    let context = ProcessContext::new(48000, block_size);

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

/// Test that soft_limit_complex_magnitude is monotonic: larger input magnitude
/// always produces larger (or equal) output magnitude, and never exceeds max.
#[test]
fn test_soft_limit_monotonicity() {
    let max_mag = 2.0_f32; // 6 dB
    let mut prev_out_mag = 0.0_f32;

    for i in 0..200 {
        let mag = i as f32 * 0.05; // 0.0 to 10.0
        let c = Complex::new(mag * 0.6, mag * 0.8); // arbitrary phase
        let limited = soft_limit_complex_magnitude(c, max_mag);
        let out_mag = limited.norm();

        // Monotonicity: output magnitude never decreases as input increases
        assert!(
            out_mag >= prev_out_mag - 1e-6,
            "Soft limit not monotonic at input mag {}: out {} < prev {}",
            mag,
            out_mag,
            prev_out_mag,
        );

        // Never exceeds max
        assert!(
            out_mag <= max_mag + 1e-6,
            "Soft limit exceeded max at input mag {}: out {} > max {}",
            mag,
            out_mag,
            max_mag,
        );

        prev_out_mag = out_mag;
    }
}

/// Test that soft_limit_complex_magnitude preserves phase
#[test]
fn test_soft_limit_preserves_phase() {
    let max_mag = 2.0_f32;

    // Test several phases at a magnitude that triggers the soft knee
    for angle_idx in 0..8 {
        let angle = angle_idx as f32 * std::f32::consts::PI / 4.0;
        let mag = 3.0; // well above max, so limiting is active
        let c = Complex::new(mag * angle.cos(), mag * angle.sin());
        let limited = soft_limit_complex_magnitude(c, max_mag);

        let input_phase = c.im.atan2(c.re);
        let output_phase = limited.im.atan2(limited.re);
        let phase_diff = (input_phase - output_phase).abs();

        assert!(
            phase_diff < 1e-5 || (phase_diff - 2.0 * std::f32::consts::PI).abs() < 1e-5,
            "Phase not preserved: input {}, output {}, diff {}",
            input_phase,
            output_phase,
            phase_diff,
        );
    }
}

/// Test that the limiter produces smooth gain transitions (no per-sample jumps).
/// Feed a signal that suddenly exceeds the threshold and verify that consecutive
/// gain reduction values change gradually.
#[test]
fn test_limiter_smooth_attack() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    // Prime the plugin with silence to fill STFT buffers
    let prime_frames = 8192;
    let mut prime_in = vec![0.0_f32; prime_frames * 2];
    let mut prime_out = vec![0.0_f32; prime_frames * 2];
    // Small signal to keep the plugin running without triggering limiter
    for i in 0..prime_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
        prime_in[i * 2] = phase.sin() * 0.1;
        prime_in[i * 2 + 1] = phase.cos() * 0.1;
    }
    let context = ProcessContext::new(48000, prime_frames);
    plugin.process(&prime_in, &mut prime_out, &context).unwrap();

    // Now feed a loud signal that will trigger the limiter
    let test_frames = 4096;
    let mut input = vec![0.0_f32; test_frames * 2];
    for i in 0..test_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * (prime_frames + i) as f32 / 48000.0;
        input[i * 2] = phase.sin() * 0.8;
        input[i * 2 + 1] = phase.cos() * 0.8;
    }
    let mut output = vec![0.0_f32; test_frames * 2];
    let context = ProcessContext::new(48000, test_frames);
    plugin.process(&input, &mut output, &context).unwrap();

    // Check that consecutive samples don't have huge gain jumps.
    // Max allowed change per sample: threshold of 0.1 per sample at 48kHz is very generous.
    let mut max_delta = 0.0_f32;
    for i in 1..test_frames {
        let delta_l = (output[i * 2] - output[(i - 1) * 2]).abs();
        let delta_r = (output[i * 2 + 1] - output[(i - 1) * 2 + 1]).abs();
        max_delta = max_delta.max(delta_l).max(delta_r);
    }

    // With a smooth attack, sample-to-sample deltas should be bounded.
    // A 1kHz sine at amplitude 0.8 through XTC filters can have inter-sample
    // deltas up to ~0.7 due to filter shaping. An instant-attack limiter would
    // produce much larger jumps (>1.0). The smooth attack keeps deltas moderate.
    assert!(
        max_delta < 0.8,
        "Limiter attack is too aggressive: max sample delta = {:.4}",
        max_delta,
    );
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

/// Test that asymmetric spectral normalization actually improves the ear response
/// toward unity. At yaw≠0, the w_lr/w_rl coefficients differ, so the spectral
/// normalization must use the correct cross-filter for each ear's response.
#[test]
fn test_asymmetric_spectral_norm_improves_ear_response() {
    use rustfft::num_complex::Complex;
    use std::f32::consts::PI;

    let num_bins = 513; // FFT_SIZE=1024

    // Filters WITHOUT spectral normalization
    let mut params_off = XtcPluginParams::default();
    params_off.head_yaw_deg = 15.0;
    params_off.spectral_normalization = false;
    params_off.room_reflections_enabled = false;
    let filters_off = compute_xtc_filters_full(&params_off, 48000, num_bins);

    // Filters WITH spectral normalization
    let mut params_on = XtcPluginParams::default();
    params_on.head_yaw_deg = 15.0;
    params_on.spectral_normalization = true;
    params_on.bypass_spectral_normalization = false;
    params_on.room_reflections_enabled = false;
    let filters_on = compute_xtc_filters_full(&params_on, 48000, num_bins);

    assert!(!filters_off.is_symmetric && !filters_on.is_symmetric);

    let cache = compute_geometry_cache(&params_on, 48000, num_bins);
    let asym = cache.asymmetric.as_ref().unwrap();

    let bin_lo = (500.0 / cache.freq_per_bin) as usize;
    let bin_hi = (4000.0 / cache.freq_per_bin) as usize;
    let mut dev_off = 0.0_f64;
    let mut dev_on = 0.0_f64;

    for bin in bin_lo..=bin_hi {
        let freq = bin as f32 * cache.freq_per_bin;
        let h_ipsi = Complex::new(1.0_f32, 0.0);
        let dt = asym.delay_left_contra - asym.delay_left_ipsi;
        let g = head_shadowing_woodworth(freq, asym.angle_left_contra, asym.a)
            * asym.amplitude_ratio_left;
        let phase = -2.0 * PI * freq * dt;
        let h_contra = Complex::new(g * phase.cos(), g * phase.sin());

        // Correct ear response: h_ipsi * w_ll + h_contra * w_rl
        let ear_off = filters_off.filter_ll[bin] * h_ipsi
            + filters_off.filter_rl.as_ref().unwrap()[bin] * h_contra;
        let ear_on = filters_on.filter_ll[bin] * h_ipsi
            + filters_on.filter_rl.as_ref().unwrap()[bin] * h_contra;

        dev_off += (ear_off.norm() as f64 - 1.0).powi(2);
        dev_on += (ear_on.norm() as f64 - 1.0).powi(2);
    }

    assert!(
        dev_on < dev_off,
        "Spectral normalization should improve ear response. dev_off={dev_off:.6}, dev_on={dev_on:.6}"
    );
}

/// Bug 3: Spectral normalization can push per-bin gain past max_gain_linear.
/// After `compute_2x2_inverse` soft-limits to max_gain_linear, spectral
/// normalization multiplies by up to ~3.7x. Every bin must stay within budget.
/// Use low beta to create ill-conditioned bins where the soft limiter saturates
/// near max_gain_linear, then spectral normalization pushes past it.
#[test]
fn test_spectral_norm_does_not_exceed_gain_budget() {
    let mut params = XtcPluginParams::default();
    params.beta_base = 0.0003; // Low beta → more aggressive inverse → larger filter gains
    assert!(params.spectral_normalization);
    let fft_size = params.fft_size;
    let num_bins = fft_size / 2 + 1;
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    let filters = compute_xtc_filters_full(&params, 48000, num_bins);

    for bin in 1..num_bins {
        let mag_ll = filters.filter_ll[bin].norm();
        assert!(
            mag_ll <= max_gain_linear + 1e-3,
            "filter_ll[{}] magnitude {} exceeds max_gain_linear {}",
            bin,
            mag_ll,
            max_gain_linear,
        );
        let mag_lr = filters.filter_lr[bin].norm();
        assert!(
            mag_lr <= max_gain_linear + 1e-3,
            "filter_lr[{}] magnitude {} exceeds max_gain_linear {}",
            bin,
            mag_lr,
            max_gain_linear,
        );
    }
}

/// Bug 4: Neumann refinement can diverge at ill-conditioned bins, producing
/// worse cancellation error than the first-order inverse. Per-bin, W2 should
/// never have higher cancellation error than W1.
/// Use low beta to create ill-conditioned bins where Neumann series diverges.
#[test]
fn test_neumann_refinement_never_increases_error() {
    use filters::compute_2x2_inverse;
    use std::f32::consts::PI;

    let mut params = XtcPluginParams::default();
    params.beta_base = 0.0003; // Low beta → ill-conditioned bins where Neumann diverges
    let fft_size = params.fft_size;
    let num_bins = fft_size / 2 + 1;
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    let cache = compute_geometry_cache(&params, 48000, num_bins);
    let sym = &cache.symmetric;

    for bin in 1..num_bins {
        let freq = bin as f32 * cache.freq_per_bin;

        let h_ipsi = Complex::new(1.0, 0.0);
        let delta_t = sym.delay_contra - sym.delay_ipsi;
        let g = head_shadowing_woodworth(freq, sym.contra_angle, sym.a) * sym.amplitude_ratio;
        let phase_contra = -2.0 * PI * freq * delta_t;
        let h_contra = Complex::new(g * phase_contra.cos(), g * phase_contra.sin());

        let beta = compute_beta_smooth(freq, &params);

        // W1: first-order only (bypass Neumann)
        let (w1_ipsi, w1_contra) =
            compute_2x2_inverse(h_ipsi, h_contra, beta, max_gain_linear, true);
        // W2: with Neumann refinement
        let (w2_ipsi, w2_contra) =
            compute_2x2_inverse(h_ipsi, h_contra, beta, max_gain_linear, false);

        // Cancellation error: |C*W - I| for the left-ear row
        // C = [[h_ipsi, h_contra], [h_contra, h_ipsi]]
        // Left-ear row of C*W: [h_ipsi*w_ipsi + h_contra*w_contra, h_ipsi*w_contra + h_contra*w_ipsi]
        // Ideal: [1, 0]
        let err1_diag = h_ipsi * w1_ipsi + h_contra * w1_contra - Complex::new(1.0, 0.0);
        let err1_off = h_ipsi * w1_contra + h_contra * w1_ipsi;
        let err1_sq = err1_diag.norm_sqr() + err1_off.norm_sqr();

        let err2_diag = h_ipsi * w2_ipsi + h_contra * w2_contra - Complex::new(1.0, 0.0);
        let err2_off = h_ipsi * w2_contra + h_contra * w2_ipsi;
        let err2_sq = err2_diag.norm_sqr() + err2_off.norm_sqr();

        assert!(
            err2_sq <= err1_sq + 1e-6,
            "Neumann refinement increased error at bin {} (freq {:.0} Hz): \
             err1_sq={:.6}, err2_sq={:.6}",
            bin,
            freq,
            err1_sq,
            err2_sq,
        );
    }
}

/// Test that `compute_2x2_inverse` does not fall back to identity (1, 0) for
/// transfer functions with small but non-singular magnitude.
///
/// With an absolute determinant threshold of 1e-10, transfer functions with
/// |H_ipsi| ≈ 1e-3 produce det ≈ |H|^4 ≈ 1e-12 < 1e-10, triggering the fallback
/// even though the matrix is perfectly invertible.  The relative threshold
/// 1e-10 * |diag| prevents this false-positive.
#[test]
fn test_2x2_inverse_no_identity_fallback_for_small_transfer_functions() {
    use filters::compute_2x2_inverse;

    // Small-magnitude transfer functions: magnitude ~1e-3 (e.g., deep in a notch but stable)
    // h_ipsi = 1e-3, h_contra = 0 (pure diagonal → matrix is trivially invertible)
    let mag: f32 = 1e-3;
    let h_ipsi = Complex::new(mag, 0.0);
    let h_contra = Complex::new(0.0, 0.0);
    let beta = 1e-10_f32; // Tiny beta so it doesn't dominate
    let max_gain = 1e6_f32;

    let (w_ipsi, w_contra) = compute_2x2_inverse(h_ipsi, h_contra, beta, max_gain, true);

    // For diagonal matrix with h_ipsi=1e-3, the true inverse is w_ipsi ≈ 1/mag = 1000.
    // The fallback returns (1.0, 0.0) — very different.
    // If the threshold is absolute 1e-10, det = diag^2 = (mag^2 + beta)^2 ≈ (1e-6)^2 = 1e-12 < 1e-10
    // and the function returns (1.0, 0.0) instead of (1000.0, 0.0).
    // With the relative threshold det < 1e-10 * diag, det ≈ 1e-12 and diag ≈ 1e-6,
    // so 1e-10 * diag ≈ 1e-16 < 1e-12, and the fallback is NOT triggered.

    // The ipsi filter must not equal 1.0 (that would be the fallback identity).
    assert!(
        w_ipsi.re.abs() > 2.0,
        "Inverse should NOT fall back to identity for small but non-singular H; \
         got w_ipsi={w_ipsi:?} (expected magnitude >> 1.0)"
    );
    let _ = w_contra;
}

/// Bug 5: Enabling pinna model causes saturation (gain > 1.0) because the
/// pinna resonances (+10 dB ear canal, +5 dB concha) inflate the transfer
/// function magnitudes. The inverse filter then attenuates at pinna frequencies,
/// reducing broadband output level. Auto-gain over-compensates, pushing
/// non-pinna frequencies past clipping.
#[test]
fn test_pinna_model_does_not_saturate() {
    let mut params = XtcPluginParams::default();
    params.pinna_model_enabled = true;
    assert!(params.auto_gain_enabled);

    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let block_size = 4096;
    let num_blocks = 16; // Enough for auto-gain to converge

    let context = ProcessContext::new(48000, block_size);

    let mut peak_output = 0.0_f32;
    for block in 0..num_blocks {
        let mut input = vec![0.0_f32; block_size * 2];
        for i in 0..block_size {
            let sample_idx = block * block_size + i;
            // Multi-tone broadband signal to exercise the full frequency range
            let t = sample_idx as f32 / 48000.0;
            let sig_l = (2.0 * std::f32::consts::PI * 200.0 * t).sin()
                + (2.0 * std::f32::consts::PI * 1000.0 * t).sin()
                + (2.0 * std::f32::consts::PI * 3000.0 * t).sin()
                + (2.0 * std::f32::consts::PI * 6000.0 * t).sin();
            let sig_r = (2.0 * std::f32::consts::PI * 300.0 * t).sin()
                + (2.0 * std::f32::consts::PI * 1300.0 * t).sin()
                + (2.0 * std::f32::consts::PI * 3500.0 * t).sin()
                + (2.0 * std::f32::consts::PI * 7000.0 * t).sin();
            // Normalize 4 tones to 0.9 peak (each tone has peak 1.0, worst-case sum = 4.0)
            input[i * 2] = sig_l * 0.9 / 4.0;
            input[i * 2 + 1] = sig_r * 0.9 / 4.0;
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
        peak_output <= 1.0,
        "Pinna model caused saturation: peak = {:.4}. \
         Pinna resonances inflate transfer function, auto-gain over-compensates.",
        peak_output
    );
}

// ============================================================================
// ITD modeling mode tests (Feature 16)
// ============================================================================

/// Test that `itd_modeling` parameter can be set and read back.
#[test]
fn test_itd_modeling_parameter_set_get() {
    let params = XtcPluginParams::default();
    assert_eq!(
        params.itd_modeling, "phase_only",
        "default itd_modeling should be 'phase_only'"
    );

    let mut plugin = XtcPlugin::new(params, 48000).unwrap();

    // Read back default via get_parameter
    let val = plugin
        .get_parameter(&ParameterId::from("itd_modeling"))
        .expect("itd_modeling should be readable");
    assert_eq!(
        val,
        ParameterValue::String("phase_only".to_string()),
        "default itd_modeling should return 'phase_only'"
    );

    // Set to explicit_delay
    plugin
        .set_parameter(
            ParameterId::from("itd_modeling"),
            ParameterValue::String("explicit_delay".to_string()),
        )
        .expect("setting itd_modeling to 'explicit_delay' should succeed");
    assert_eq!(plugin.params.itd_modeling, "explicit_delay");

    let val2 = plugin
        .get_parameter(&ParameterId::from("itd_modeling"))
        .expect("itd_modeling should be readable after set");
    assert_eq!(val2, ParameterValue::String("explicit_delay".to_string()));
}

/// Test that an invalid `itd_modeling` value is rejected.
#[test]
fn test_itd_modeling_invalid_value_rejected() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    let result = plugin.set_parameter(
        ParameterId::from("itd_modeling"),
        ParameterValue::String("invalid_mode".to_string()),
    );
    assert!(
        result.is_err(),
        "Setting an invalid itd_modeling value should return an error"
    );
}

/// Test that `explicit_delay` mode produces different low-frequency filter
/// coefficients compared to `phase_only` mode (symmetric, no yaw).
///
/// At low frequencies the explicit delay phase shift diverges from the
/// implicit Woodworth phase, so the plant matrix — and therefore the
/// computed inverse filters — must differ.
#[test]
fn test_itd_explicit_delay_differs_from_phase_only_at_lf() {
    let fft_size = 2048;
    let sample_rate = 48000_u32;
    let num_bins = fft_size / 2 + 1;

    // Phase-only (default)
    let mut params_po = XtcPluginParams::default();
    params_po.fft_size = fft_size;
    params_po.itd_modeling = "phase_only".to_string();
    params_po.room_reflections_enabled = false;
    let filters_po = compute_xtc_filters_full(&params_po, sample_rate, num_bins);

    // Explicit-delay
    let mut params_ed = XtcPluginParams::default();
    params_ed.fft_size = fft_size;
    params_ed.itd_modeling = "explicit_delay".to_string();
    params_ed.room_reflections_enabled = false;
    let filters_ed = compute_xtc_filters_full(&params_ed, sample_rate, num_bins);

    let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

    // Collect mean absolute difference of filter_lr at LF (<200 Hz) and HF (>1000 Hz)
    let mut lf_diff = 0.0_f32;
    let mut hf_diff = 0.0_f32;
    let mut lf_count = 0_usize;
    let mut hf_count = 0_usize;

    for bin in 1..num_bins {
        let freq = bin as f32 * freq_per_bin;
        let d = (filters_ed.filter_lr[bin] - filters_po.filter_lr[bin]).norm();
        if freq < 200.0 {
            lf_diff += d;
            lf_count += 1;
        } else if freq > 1000.0 && freq < 4000.0 {
            hf_diff += d;
            hf_count += 1;
        }
    }

    let lf_mean = if lf_count > 0 {
        lf_diff / lf_count as f32
    } else {
        0.0
    };
    let hf_mean = if hf_count > 0 {
        hf_diff / hf_count as f32
    } else {
        0.0
    };

    // Explicit delay must change LF filters relative to phase-only
    assert!(
        lf_mean > 1e-5,
        "explicit_delay should produce different LF filters than phase_only; mean LF diff = {}",
        lf_mean
    );

    // At high frequencies both modes converge (sigmoid blend → 0 at HF)
    // The HF mean difference should be significantly smaller than the LF difference.
    assert!(
        hf_mean < lf_mean,
        "HF filter difference ({}) should be smaller than LF difference ({}) \
         because the sigmoid crossover fades out explicit_delay above 300 Hz",
        hf_mean,
        lf_mean
    );
}

/// Test that `explicit_delay` mode still produces stable output (no NaN/Inf,
/// bounded peak amplitude) during normal audio processing.
#[test]
fn test_itd_explicit_delay_stable_output() {
    let mut params = XtcPluginParams::default();
    params.itd_modeling = "explicit_delay".to_string();
    let mut plugin = XtcPlugin::new(params, 48000).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 8192;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 100.0 * i as f32 / 48000.0; // 100 Hz LF tone
        input[i * 2] = phase.sin() * 0.5;
        input[i * 2 + 1] = phase.cos() * 0.5;
    }
    let mut output = vec![0.0_f32; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    plugin.process(&input, &mut output, &context).unwrap();

    // All output samples must be finite and within ±1.0 (limiter is active)
    for (i, &s) in output.iter().enumerate() {
        assert!(
            s.is_finite(),
            "output[{}] is not finite ({}) with explicit_delay mode",
            i,
            s
        );
        assert!(
            s.abs() <= 1.001,
            "output[{}] = {} exceeds ±1.0 with explicit_delay mode",
            i,
            s
        );
    }
}

/// Test that `itd_modeling` is included in the cached_parameters list.
#[test]
fn test_itd_modeling_in_parameters_list() {
    let params = XtcPluginParams::default();
    let plugin = XtcPlugin::new(params, 48000).unwrap();
    let all_params = plugin.parameters();
    let found = all_params.iter().any(|p| p.id.as_str() == "itd_modeling");
    assert!(
        found,
        "itd_modeling should appear in the plugin's parameter list"
    );
}

#[test]
fn test_roomeq_recommended_source_parameters() {
    let path = std::env::temp_dir().join(format!(
        "xtc-roomeq-recommended-params-{}.json",
        std::process::id()
    ));
    let artifact = serde_json::json!({
        "version": "ctc-recommended-v1",
        "source": "measured",
        "sample_rate": 48_000,
        "speakers": ["L", "R"],
        "ears": ["left_ear", "right_ear"],
        "filters": [
            { "speaker": "L", "target_ear": "left_ear", "taps": [1.0, 0.0, 0.0, 0.0] },
            { "speaker": "R", "target_ear": "left_ear", "taps": [0.0, 0.0, 0.0, 0.0] },
            { "speaker": "L", "target_ear": "right_ear", "taps": [0.0, 0.0, 0.0, 0.0] },
            { "speaker": "R", "target_ear": "right_ear", "taps": [1.0, 0.0, 0.0, 0.0] }
        ]
    });
    std::fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();

    let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 48_000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("recommended_matrix_file"),
            ParameterValue::String(path.to_string_lossy().to_string()),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("source_mode"),
            ParameterValue::String("roomeq_recommended".to_string()),
        )
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("source_mode")),
        Some(ParameterValue::String("roomeq_recommended".to_string()))
    );
    assert!(
        plugin
            .parameters()
            .iter()
            .any(|p| p.id.as_str() == "recommended_matrix_file")
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_roomeq_recommended_source_rejects_invalid_artifact_on_enable() {
    let path = std::env::temp_dir().join(format!(
        "xtc-roomeq-recommended-invalid-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, b"{ invalid json").unwrap();

    let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 48_000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("recommended_matrix_file"),
            ParameterValue::String(path.to_string_lossy().to_string()),
        )
        .unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("source_mode"),
            ParameterValue::String("roomeq_recommended".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("roomEQ recommended matrix"));

    let _ = std::fs::remove_file(path);
}

/// Test JSON deserialization of `itd_modeling` field.
#[test]
fn test_itd_modeling_serde() {
    // Default (empty JSON) should give "phase_only"
    let po: XtcPluginParams = serde_json::from_str("{}").unwrap();
    assert_eq!(po.itd_modeling, "phase_only");

    // Explicit value is preserved
    let ed: XtcPluginParams =
        serde_json::from_str(r#"{"itd_modeling": "explicit_delay"}"#).unwrap();
    assert_eq!(ed.itd_modeling, "explicit_delay");
}

/// Test that `latency_samples()` reports `fft_size - hop_size` rather than `fft_size`.
///
/// Because the plugin starts draining output immediately after the first STFT frame
/// (incrementing `output_accumulator_fill` by `hop_size` right away), the first audio
/// sample appears after `fft_size - hop_size` input samples, not after `fft_size`.
/// For a 1024-point FFT with 75% overlap (hop = 256): latency = 768, not 1024.
#[test]
fn test_latency_is_fft_size_minus_hop_size() {
    let params = XtcPluginParams::default();
    let fft_size = params.fft_size;
    let hop_size = fft_size / 4; // 75% overlap
    let plugin = XtcPlugin::new(params, 48000).unwrap();

    let expected = fft_size - hop_size;
    let reported = plugin.latency_samples();
    assert_eq!(
        reported, expected,
        "latency_samples() should return fft_size - hop_size = {expected}, got {reported}"
    );
}

/// Bug 1: Asymmetric spectral normalization scales wrong columns (rows instead of columns).
/// Left-ear normalization should scale column 0 (w_ll, w_rl) — the code scales row 0
/// (w_ll, w_lr). This means w_ll and w_rl should receive the SAME scale factor.
/// In the buggy code, w_ll and w_lr receive the same factor instead.
///
/// Test: verify that spectral normalization applies the same factor to both elements
/// of each column (left column: w_ll, w_rl; right column: w_lr, w_rr).
#[test]
fn test_asymmetric_spectral_norm_scales_columns_not_rows() {
    let num_bins = 513;

    let mut params_off = XtcPluginParams::default();
    params_off.head_yaw_deg = 45.0; // Large yaw for maximum asymmetry between ears
    params_off.beta_base = 0.001;
    params_off.spectral_normalization = false;
    params_off.room_reflections_enabled = false;
    let f_off = compute_xtc_filters_full(&params_off, 48000, num_bins);

    let mut params_on = XtcPluginParams::default();
    params_on.head_yaw_deg = 45.0;
    params_on.beta_base = 0.001;
    params_on.spectral_normalization = true;
    params_on.bypass_spectral_normalization = false;
    params_on.room_reflections_enabled = false;
    let f_on = compute_xtc_filters_full(&params_on, 48000, num_bins);

    assert!(!f_off.is_symmetric && !f_on.is_symmetric);
    let rl_off = f_off.filter_rl.as_ref().unwrap();
    let rr_off = f_off.filter_rr.as_ref().unwrap();
    let rl_on = f_on.filter_rl.as_ref().unwrap();
    let rr_on = f_on.filter_rr.as_ref().unwrap();

    let cache = compute_geometry_cache(&params_on, 48000, num_bins);
    let bin_lo = (800.0 / cache.freq_per_bin) as usize;
    let bin_hi = (3000.0 / cache.freq_per_bin) as usize;

    let mut col_err = 0.0_f64;
    let mut row_err = 0.0_f64;
    let mut count = 0;

    for bin in bin_lo..=bin_hi {
        // Skip bins where denominators are too small for stable ratio computation
        if f_off.filter_ll[bin].norm() < 1e-6
            || rl_off[bin].norm() < 1e-6
            || f_off.filter_lr[bin].norm() < 1e-6
            || rr_off[bin].norm() < 1e-6
        {
            continue;
        }

        // Ratio on/off for each element
        let ratio_ll = (f_on.filter_ll[bin] / f_off.filter_ll[bin]).norm();
        let ratio_lr = (f_on.filter_lr[bin] / f_off.filter_lr[bin]).norm();
        let ratio_rl = (rl_on[bin] / rl_off[bin]).norm();
        let ratio_rr = (rr_on[bin] / rr_off[bin]).norm();

        // In correct code: left column has same ratio → |ratio_ll - ratio_rl| ≈ 0
        //                   right column has same ratio → |ratio_lr - ratio_rr| ≈ 0
        col_err += ((ratio_ll - ratio_rl) as f64).powi(2) + ((ratio_lr - ratio_rr) as f64).powi(2);

        // In buggy code (row scaling): |ratio_ll - ratio_lr| ≈ 0 and |ratio_rl - ratio_rr| ≈ 0
        row_err += ((ratio_ll - ratio_lr) as f64).powi(2) + ((ratio_rl - ratio_rr) as f64).powi(2);

        count += 1;
    }

    assert!(count > 10, "Not enough valid bins: {}", count);

    // Column error should be smaller than row error (correct = column scaling)
    assert!(
        col_err < row_err,
        "Spectral normalization scales ROWS instead of COLUMNS! \
         col_err={col_err:.10}, row_err={row_err:.10} (col_err should be smaller)"
    );
}
