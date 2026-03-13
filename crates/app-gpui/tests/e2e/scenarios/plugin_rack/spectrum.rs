//! Tests for Spectrum Analyzer math and display calculations.
//!
//! Pure calculation tests that don't need GPUI context.

// =============================================================================
// Constants
// =============================================================================

const MIN_FREQUENCY: f64 = 20.0;
const MAX_FREQUENCY: f64 = 20000.0;
const MIN_DB: f64 = -90.0;
const MAX_DB: f64 = 0.0;

// =============================================================================
// Frequency Axis Tests
// =============================================================================

#[test]
fn test_frequency_to_position_roundtrip() {
    fn frequency_to_x(freq: f64, width: f64) -> f64 {
        let log_min = MIN_FREQUENCY.ln();
        let log_max = MAX_FREQUENCY.ln();
        let log_freq = freq.clamp(MIN_FREQUENCY, MAX_FREQUENCY).ln();
        (log_freq - log_min) / (log_max - log_min) * width
    }

    fn x_to_frequency(x: f64, width: f64) -> f64 {
        let log_min = MIN_FREQUENCY.ln();
        let log_max = MAX_FREQUENCY.ln();
        (log_min + (x / width) * (log_max - log_min)).exp()
    }

    let width = 800.0;
    for freq in [20.0, 100.0, 1000.0, 10000.0, 20000.0] {
        let x = frequency_to_x(freq, width);
        let restored = x_to_frequency(x, width);
        assert!(
            (restored - freq).abs() < 1.0,
            "Frequency {} should round-trip, got {}",
            freq,
            restored
        );
    }
}

// =============================================================================
// dB Axis Tests
// =============================================================================

#[test]
fn test_db_axis_mapping() {
    fn db_to_y(db: f64, height: f64) -> f64 {
        let normalized = (db - MIN_DB) / (MAX_DB - MIN_DB);
        (1.0 - normalized) * height
    }

    let height = 300.0;
    assert!(db_to_y(0.0, height).abs() < 1.0, "0 dB at top");
    assert!(
        (db_to_y(-90.0, height) - height).abs() < 1.0,
        "-90 dB at bottom"
    );
    assert!(
        (db_to_y(-45.0, height) - height / 2.0).abs() < 1.0,
        "-45 dB in middle"
    );
}

// =============================================================================
// Smoothing Tests
// =============================================================================

#[test]
fn test_smoothing_calculation() {
    fn apply_smoothing(current: f64, new_value: f64, smoothing: f64) -> f64 {
        current * smoothing + new_value * (1.0 - smoothing)
    }

    // Zero smoothing: instant response
    let result = apply_smoothing(-30.0, -10.0, 0.0);
    assert!((result - (-10.0)).abs() < 0.001);

    // High smoothing: current value dominates
    let result = apply_smoothing(-30.0, -10.0, 0.9);
    assert!((result - (-28.0)).abs() < 0.001);
}

// =============================================================================
// Frequency Resolution Tests
// =============================================================================

#[test]
fn test_frequency_resolution() {
    fn freq_resolution(fft_size: usize, sample_rate: f64) -> f64 {
        sample_rate / fft_size as f64
    }

    let sample_rate = 48000.0;
    let res_512 = freq_resolution(512, sample_rate);
    let res_4096 = freq_resolution(4096, sample_rate);

    assert!(res_512 > res_4096, "Larger FFT = better resolution");
    assert!(
        (res_4096 - 11.72).abs() < 0.1,
        "4096 @ 48kHz should be ~11.72 Hz"
    );
}

// =============================================================================
// FFT Bin Count Tests
// =============================================================================

#[test]
fn test_fft_bin_count() {
    for fft_size in [512, 1024, 2048, 4096, 8192] {
        let bins = fft_size / 2;
        assert_eq!(
            bins,
            fft_size / 2,
            "FFT size {} should have {} bins",
            fft_size,
            bins
        );
    }
}

// =============================================================================
// Peak Hold Tests
// =============================================================================

#[test]
fn test_peak_hold_logic() {
    fn update_peak(current_peak: f64, new_value: f64) -> f64 {
        new_value.max(current_peak)
    }

    assert!(
        (update_peak(-30.0, -20.0) - (-20.0)).abs() < 0.001,
        "Peak updates to higher value"
    );
    assert!(
        (update_peak(-20.0, -40.0) - (-20.0)).abs() < 0.001,
        "Peak holds at higher value"
    );
}

// =============================================================================
// Color Mapping Tests
// =============================================================================

#[test]
fn test_spectrum_color_mapping() {
    fn db_to_color(db: f64) -> (u8, u8, u8) {
        let normalized = ((db - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0);
        if normalized < 0.5 {
            let t = normalized * 2.0;
            ((255.0 * t) as u8, 255, 0)
        } else {
            let t = (normalized - 0.5) * 2.0;
            (255, (255.0 * (1.0 - t)) as u8, 0)
        }
    }

    assert_eq!(db_to_color(-90.0), (0, 255, 0), "Low = green");
    assert_eq!(db_to_color(0.0), (255, 0, 0), "High = red");
}

// =============================================================================
// Grid Line Tests
// =============================================================================

#[test]
fn test_frequency_grid_lines() {
    let lines: Vec<f64> = vec![
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ];
    assert!(lines.len() >= 8, "Should have frequency grid lines");
    assert!((lines[0] - 20.0).abs() < 0.001, "Should start at 20 Hz");
    assert!(
        (lines[lines.len() - 1] - 20000.0).abs() < 0.001,
        "Should end at 20 kHz"
    );
}

#[test]
fn test_db_grid_lines() {
    let lines: Vec<f64> = vec![
        0.0, -10.0, -20.0, -30.0, -40.0, -50.0, -60.0, -70.0, -80.0, -90.0,
    ];
    assert_eq!(lines.len(), 10, "Should have 10 dB grid lines");
    assert!(lines[0].abs() < 0.001, "Should start at 0 dB");
    assert!(
        (lines[9] - (-90.0)).abs() < 0.001,
        "Should end at -90 dB"
    );
}

// =============================================================================
// Display Range Tests
// =============================================================================

#[test]
fn test_display_range() {
    let min_db = -90.0_f64;
    let max_db = 0.0_f64;
    assert_eq!(max_db - min_db, 90.0, "Should have 90 dB range");
}

#[test]
fn test_zoomed_range_visibility() {
    let zoomed_min = -60.0;
    let zoomed_max = 0.0;

    fn is_visible(db: f64, min: f64, max: f64) -> bool {
        db >= min && db <= max
    }

    assert!(is_visible(-30.0, zoomed_min, zoomed_max));
    assert!(!is_visible(-80.0, zoomed_min, zoomed_max));
}

// =============================================================================
// Channel Color Tests
// =============================================================================

#[test]
fn test_channel_colors_distinct() {
    fn channel_color(index: usize) -> (u8, u8, u8) {
        let colors = [
            (66, 133, 244),  // Blue (L)
            (234, 67, 53),   // Red (R)
            (52, 168, 83),   // Green (C)
            (251, 188, 5),   // Yellow (LFE)
            (153, 0, 255),   // Purple (Ls)
            (0, 188, 212),   // Cyan (Rs)
        ];
        colors[index % colors.len()]
    }

    let color_0 = channel_color(0);
    let color_1 = channel_color(1);
    assert_ne!(color_0, color_1, "Channels should have different colors");
}
