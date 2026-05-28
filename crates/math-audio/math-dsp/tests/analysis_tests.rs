use math_audio_dsp::analysis::{
    compute_rt60_broadband, find_db_point, smooth_response_f32, smooth_response_f64,
};

#[test]
fn test_find_db_point_flat() {
    let frequencies = vec![20.0, 100.0, 1000.0, 20000.0];
    let magnitude_db = vec![0.0, 0.0, 0.0, 0.0];

    // In a flat response, -3dB is never reached
    assert_eq!(find_db_point(&frequencies, &magnitude_db, -3.0, true), None);
    assert_eq!(
        find_db_point(&frequencies, &magnitude_db, -3.0, false),
        None
    );
}

#[test]
fn test_find_db_point_sloping_down() {
    let frequencies = vec![100.0, 200.0, 400.0, 800.0];
    let magnitude_db = vec![0.0, -2.0, -4.0, -6.0];

    // -3dB should be between 200Hz and 400Hz
    // Linear interpolation: -2.0 + t*(-4.0 - (-2.0)) = -3.0 => -2.0 - 2t = -3.0 => 2t = 1.0 => t = 0.5
    // Freq: 200 + 0.5 * (400 - 200) = 300
    let p3db = find_db_point(&frequencies, &magnitude_db, -3.0, true);
    assert!(p3db.is_some());
    assert!((p3db.unwrap() - 300.0).abs() < 1e-3);
}

#[test]
fn test_find_db_point_sloping_up() {
    let frequencies = vec![100.0, 200.0, 400.0, 800.0];
    let magnitude_db = vec![-6.0, -4.0, -2.0, 0.0];

    // -3dB should be between 200Hz and 400Hz (ascending)
    // Linear interpolation: -4.0 + t*(-2.0 - (-4.0)) = -3.0 => -4.0 + 2t = -3.0 => 2t = 1.0 => t = 0.5
    // Freq: 200 + 0.5 * (400 - 200) = 300
    let p3db = find_db_point(&frequencies, &magnitude_db, -3.0, true);
    assert!(p3db.is_some());
    assert!((p3db.unwrap() - 300.0).abs() < 1e-3);
}

#[test]
fn test_find_db_point_subwoofer() {
    let frequencies = vec![20.0, 40.0, 80.0, 120.0, 160.0, 200.0];
    let magnitude_db = vec![-10.0, -3.0, 0.0, 0.0, -3.0, -10.0];

    // Low -3dB point at 40Hz
    let low_3db = find_db_point(&frequencies, &magnitude_db, -3.0, true);
    assert!(low_3db.is_some());
    assert!((low_3db.unwrap() - 40.0).abs() < 1e-3);

    // High -3dB point at 160Hz (searching from the end)
    let high_3db = find_db_point(&frequencies, &magnitude_db, -3.0, false);
    assert!(high_3db.is_some());
    assert!((high_3db.unwrap() - 160.0).abs() < 1e-3);
}

#[test]
fn test_compute_average_response_full_band() {
    use math_audio_dsp::analysis::compute_average_response;
    let frequencies = vec![100.0, 200.0, 400.0, 800.0];
    let magnitude_db = vec![10.0, 10.0, 10.0, 10.0];

    // Flat response should average to 10.0
    let avg = compute_average_response(&frequencies, &magnitude_db, None);
    assert!((avg - 10.0).abs() < 1e-3);
}

#[test]
fn test_compute_average_response_range() {
    use math_audio_dsp::analysis::compute_average_response;
    let frequencies = vec![100.0, 200.0, 400.0, 800.0];
    let magnitude_db = vec![0.0, 10.0, 10.0, 20.0];

    // Average between 200Hz and 400Hz (where both are 10dB)
    let avg = compute_average_response(&frequencies, &magnitude_db, Some((200.0, 400.0)));
    assert!((avg - 10.0).abs() < 1e-3);
}

#[test]
fn test_compute_average_response_log_spacing() {
    use math_audio_dsp::analysis::compute_average_response;
    let frequencies = vec![10.0, 100.0, 1000.0];
    let magnitude_db = vec![0.0, 10.0, 20.0];

    // Log-weighted average should be more accurate for acoustic data
    // Here we'll just test that it returns a value within range for now
    let avg = compute_average_response(&frequencies, &magnitude_db, None);
    assert!(avg > 0.0 && avg < 20.0);
}

#[test]
fn test_smooth_response_f64_handles_extra_values_without_panic() {
    let frequencies = vec![20.0, 100.0];
    let values = vec![1.0, 3.0, 5.0];

    let smoothed = smooth_response_f64(&frequencies, &values, 10.0);

    assert_eq!(smoothed.len(), frequencies.len());
    assert!((smoothed[0] - 2.0).abs() < 1e-9);
    assert!((smoothed[1] - 2.0).abs() < 1e-9);
}

#[test]
fn test_smooth_response_f32_handles_extra_frequencies_without_panic() {
    let frequencies = vec![20.0, 100.0, 1000.0];
    let values = vec![1.0, 3.0];

    let smoothed = smooth_response_f32(&frequencies, &values, 0.01);

    assert_eq!(smoothed.len(), values.len());
    assert!((smoothed[0] - values[0]).abs() < f32::EPSILON);
    assert!((smoothed[1] - values[1]).abs() < f32::EPSILON);
}

fn make_exponential_decay(num_samples: usize, sample_rate: f32, rt60_seconds: f32) -> Vec<f32> {
    let k = 3.0 * std::f32::consts::LN_10 / rt60_seconds;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate;
            (-k * t).exp()
        })
        .collect()
}

fn lcg_noise(n: usize, seed: u32, amplitude: f32) -> Vec<f32> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let normalised = (state as f32) / (u32::MAX as f32) - 0.5;
            normalised * 2.0 * amplitude
        })
        .collect()
}

#[test]
fn test_compute_rt60_broadband_recovers_exponential_decay() {
    let sample_rate = 48_000.0_f32;
    let expected_rt60 = 0.5_f32;
    let impulse = make_exponential_decay((sample_rate * 2.0) as usize, sample_rate, expected_rt60);

    let rt60 = compute_rt60_broadband(&impulse, sample_rate);

    assert!(
        (rt60 - expected_rt60).abs() < 0.03,
        "expected RT60 near {expected_rt60:.3}s, got {rt60:.3}s"
    );
}

#[test]
fn test_compute_rt60_broadband_trims_noise_tail_before_fit() {
    let sample_rate = 48_000.0_f32;
    let expected_rt60 = 0.45_f32;
    let mut impulse =
        make_exponential_decay((sample_rate * 0.8) as usize, sample_rate, expected_rt60);
    impulse.extend(lcg_noise((sample_rate * 1.2) as usize, 0x1234_5678, 0.0005));

    let rt60 = compute_rt60_broadband(&impulse, sample_rate);

    assert!(
        (rt60 - expected_rt60).abs() < 0.06,
        "expected tail-trimmed RT60 near {expected_rt60:.3}s, got {rt60:.3}s"
    );
}

#[test]
fn test_compute_rt60_broadband_rejects_insufficient_dynamic_range() {
    let sample_rate = 48_000.0_f32;
    let impulse = make_exponential_decay((sample_rate * 0.050) as usize, sample_rate, 0.5);

    assert_eq!(compute_rt60_broadband(&impulse, sample_rate), 0.0);
}
