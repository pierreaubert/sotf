use math_audio_dsp::analysis::find_db_point;

#[test]
fn test_find_db_point_flat() {
    let frequencies = vec![20.0, 100.0, 1000.0, 20000.0];
    let magnitude_db = vec![0.0, 0.0, 0.0, 0.0];
    
    // In a flat response, -3dB is never reached
    assert_eq!(find_db_point(&frequencies, &magnitude_db, -3.0, true), None);
    assert_eq!(find_db_point(&frequencies, &magnitude_db, -3.0, false), None);
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
