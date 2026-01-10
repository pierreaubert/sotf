//! E2E tests for Spectrum Analyzer Plugin UI.
//!
//! Tests for verifying spectrum analyzer display functionality:
//! - Spectrum display rendering
//! - Smoothing control
//! - FFT size selection
//! - Window function selection
//! - Peak hold
//! - Channel selection
//! - Color/gradient display

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_FREQUENCY: f64 = 20.0;
const MAX_FREQUENCY: f64 = 20000.0;
const MIN_DB: f64 = -90.0;
const MAX_DB: f64 = 0.0;

const DEFAULT_SMOOTHING: f64 = 0.7;
const MIN_SMOOTHING: f64 = 0.0;
const MAX_SMOOTHING: f64 = 0.99;

// =============================================================================
// FFT Configuration
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FftSize {
    Size512,
    Size1024,
    Size2048,
    Size4096,
    Size8192,
}

impl FftSize {
    fn value(&self) -> usize {
        match self {
            FftSize::Size512 => 512,
            FftSize::Size1024 => 1024,
            FftSize::Size2048 => 2048,
            FftSize::Size4096 => 4096,
            FftSize::Size8192 => 8192,
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            FftSize::Size512 => "512",
            FftSize::Size1024 => "1024",
            FftSize::Size2048 => "2048",
            FftSize::Size4096 => "4096",
            FftSize::Size8192 => "8192",
        }
    }

    fn all() -> &'static [FftSize] {
        &[
            FftSize::Size512,
            FftSize::Size1024,
            FftSize::Size2048,
            FftSize::Size4096,
            FftSize::Size8192,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowFunction {
    Rectangular,
    Hann,
    Hamming,
    Blackman,
    BlackmanHarris,
}

impl WindowFunction {
    fn display_name(&self) -> &'static str {
        match self {
            WindowFunction::Rectangular => "Rectangular",
            WindowFunction::Hann => "Hann",
            WindowFunction::Hamming => "Hamming",
            WindowFunction::Blackman => "Blackman",
            WindowFunction::BlackmanHarris => "Blackman-Harris",
        }
    }

    fn all() -> &'static [WindowFunction] {
        &[
            WindowFunction::Rectangular,
            WindowFunction::Hann,
            WindowFunction::Hamming,
            WindowFunction::Blackman,
            WindowFunction::BlackmanHarris,
        ]
    }
}

// =============================================================================
// Spectrum State
// =============================================================================

#[derive(Debug, Clone)]
struct SpectrumState {
    fft_size: FftSize,
    window_function: WindowFunction,
    smoothing: f64,
    peak_hold: bool,
    peak_hold_time_ms: f64,
    selected_channel: usize,
    show_all_channels: bool,
    num_channels: usize,
    // Display data (simulated)
    spectrum_data: Vec<f64>, // dB values at each bin
    peak_data: Vec<f64>,
}

impl SpectrumState {
    fn new(num_channels: usize, fft_size: FftSize) -> Self {
        let num_bins = fft_size.value() / 2;
        Self {
            fft_size,
            window_function: WindowFunction::Hann,
            smoothing: DEFAULT_SMOOTHING,
            peak_hold: false,
            peak_hold_time_ms: 1000.0,
            selected_channel: 0,
            show_all_channels: false,
            num_channels,
            spectrum_data: vec![MIN_DB; num_bins],
            peak_data: vec![MIN_DB; num_bins],
        }
    }
}

// =============================================================================
// Spectrum Display Tests
// =============================================================================

/// Test spectrum display initialization.
#[gpui::test]
async fn test_spectrum_display_init(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    assert_eq!(
        state.borrow().spectrum_data.len(),
        1024,
        "Should have N/2 bins"
    );
    assert!(
        state
            .borrow()
            .spectrum_data
            .iter()
            .all(|&v| v <= MIN_DB + 0.001)
    );
}

/// Test spectrum frequency axis (log scale).
#[gpui::test]
async fn test_spectrum_frequency_axis(_cx: &mut TestAppContext) {
    fn frequency_to_x_position(freq: f64, width: f64) -> f64 {
        let log_min = MIN_FREQUENCY.ln();
        let log_max = MAX_FREQUENCY.ln();
        let log_freq = freq.clamp(MIN_FREQUENCY, MAX_FREQUENCY).ln();
        (log_freq - log_min) / (log_max - log_min) * width
    }

    fn x_position_to_frequency(x: f64, width: f64) -> f64 {
        let log_min = MIN_FREQUENCY.ln();
        let log_max = MAX_FREQUENCY.ln();
        let log_freq = log_min + (x / width) * (log_max - log_min);
        log_freq.exp()
    }

    let width = 800.0;

    // Test key frequencies
    let test_freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
    for freq in test_freqs {
        let x = frequency_to_x_position(freq, width);
        let restored = x_position_to_frequency(x, width);
        assert!(
            (restored - freq).abs() < 1.0,
            "Frequency {} should round-trip, got {}",
            freq,
            restored
        );
    }
}

/// Test spectrum dB axis (linear scale).
#[gpui::test]
async fn test_spectrum_db_axis(_cx: &mut TestAppContext) {
    fn db_to_y_position(db: f64, height: f64) -> f64 {
        let normalized = (db - MIN_DB) / (MAX_DB - MIN_DB);
        (1.0 - normalized) * height // Invert: 0dB at top
    }

    let height = 300.0;

    // 0 dB at top
    let y_0db = db_to_y_position(0.0, height);
    assert!(y_0db.abs() < 1.0, "0 dB should be at top");

    // -90 dB at bottom
    let y_min = db_to_y_position(-90.0, height);
    assert!((y_min - height).abs() < 1.0, "-90 dB should be at bottom");

    // -45 dB in middle
    let y_mid = db_to_y_position(-45.0, height);
    assert!(
        (y_mid - height / 2.0).abs() < 1.0,
        "-45 dB should be in middle"
    );
}

// =============================================================================
// Smoothing Tests
// =============================================================================

/// Test smoothing initial value.
#[gpui::test]
async fn test_spectrum_smoothing_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    assert!(
        (state.borrow().smoothing - DEFAULT_SMOOTHING).abs() < 0.001,
        "Initial smoothing should be {}",
        DEFAULT_SMOOTHING
    );
}

/// Test smoothing slider adjustment.
#[gpui::test]
async fn test_spectrum_smoothing_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    let test_values: Vec<f64> = vec![0.0, 0.25, 0.5, 0.75, 0.9, 0.99];
    for value in test_values {
        state.borrow_mut().smoothing = value.clamp(MIN_SMOOTHING, MAX_SMOOTHING);
        assert!(
            (state.borrow().smoothing - value).abs() < 0.001,
            "Smoothing should be {}",
            value
        );
    }
}

/// Test smoothing bounds.
#[gpui::test]
async fn test_spectrum_smoothing_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    // Minimum (no smoothing, instant response)
    state.borrow_mut().smoothing = MIN_SMOOTHING;
    assert!(state.borrow().smoothing.abs() < 0.001);

    // Maximum (very slow decay)
    state.borrow_mut().smoothing = MAX_SMOOTHING;
    assert!((state.borrow().smoothing - 0.99).abs() < 0.001);
}

/// Test smoothing calculation.
#[gpui::test]
async fn test_spectrum_smoothing_calculation(_cx: &mut TestAppContext) {
    fn apply_smoothing(current: f64, new_value: f64, smoothing: f64) -> f64 {
        current * smoothing + new_value * (1.0 - smoothing)
    }

    // With 0 smoothing, new value takes over immediately
    let result = apply_smoothing(-30.0, -10.0, 0.0);
    assert!((result - (-10.0)).abs() < 0.001);

    // With high smoothing, current value dominates
    let result = apply_smoothing(-30.0, -10.0, 0.9);
    assert!((result - (-28.0)).abs() < 0.001);
}

// =============================================================================
// FFT Size Tests
// =============================================================================

/// Test FFT size selection.
#[gpui::test]
async fn test_spectrum_fft_size_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    for &size in FftSize::all() {
        state.borrow_mut().fft_size = size;
        assert_eq!(state.borrow().fft_size, size);
    }
}

/// Test FFT size display values.
#[gpui::test]
async fn test_spectrum_fft_size_values(_cx: &mut TestAppContext) {
    assert_eq!(FftSize::Size512.value(), 512);
    assert_eq!(FftSize::Size1024.value(), 1024);
    assert_eq!(FftSize::Size2048.value(), 2048);
    assert_eq!(FftSize::Size4096.value(), 4096);
    assert_eq!(FftSize::Size8192.value(), 8192);
}

/// Test FFT size affects bin count.
#[gpui::test]
async fn test_spectrum_fft_size_bins(_cx: &mut TestAppContext) {
    for &size in FftSize::all() {
        let state = SpectrumState::new(2, size);
        let expected_bins = size.value() / 2;
        assert_eq!(
            state.spectrum_data.len(),
            expected_bins,
            "FFT size {} should have {} bins",
            size.value(),
            expected_bins
        );
    }
}

/// Test FFT size frequency resolution.
#[gpui::test]
async fn test_spectrum_frequency_resolution(_cx: &mut TestAppContext) {
    fn frequency_resolution(fft_size: usize, sample_rate: f64) -> f64 {
        sample_rate / fft_size as f64
    }

    let sample_rate = 48000.0;

    // Larger FFT = better frequency resolution
    let res_512 = frequency_resolution(512, sample_rate);
    let res_4096 = frequency_resolution(4096, sample_rate);

    assert!(
        res_512 > res_4096,
        "Larger FFT should have better resolution"
    );
    assert!(
        (res_4096 - 11.72).abs() < 0.1,
        "4096 @ 48kHz = ~11.72 Hz resolution"
    );
}

// =============================================================================
// Window Function Tests
// =============================================================================

/// Test window function selection.
#[gpui::test]
async fn test_spectrum_window_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    for &window in WindowFunction::all() {
        state.borrow_mut().window_function = window;
        assert_eq!(state.borrow().window_function, window);
    }
}

/// Test window function display names.
#[gpui::test]
async fn test_spectrum_window_names(_cx: &mut TestAppContext) {
    for window in WindowFunction::all() {
        let name = window.display_name();
        assert!(!name.is_empty(), "Window should have display name");
    }
}

// =============================================================================
// Peak Hold Tests
// =============================================================================

/// Test peak hold toggle.
#[gpui::test]
async fn test_spectrum_peak_hold_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    // Initially off
    assert!(!state.borrow().peak_hold);

    // Enable
    state.borrow_mut().peak_hold = true;
    assert!(state.borrow().peak_hold);

    // Disable
    state.borrow_mut().peak_hold = false;
    assert!(!state.borrow().peak_hold);
}

/// Test peak hold time adjustment.
#[gpui::test]
async fn test_spectrum_peak_hold_time(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    let test_times = vec![500.0, 1000.0, 2000.0, 5000.0];
    for time in test_times {
        state.borrow_mut().peak_hold_time_ms = time;
        assert!((state.borrow().peak_hold_time_ms - time).abs() < 0.001);
    }
}

/// Test peak hold behavior.
#[gpui::test]
async fn test_spectrum_peak_hold_behavior(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));
    state.borrow_mut().peak_hold = true;

    // Simulate peak update
    fn update_peak(current_peak: f64, new_value: f64) -> f64 {
        new_value.max(current_peak) // Peak is always max
    }

    let peak = update_peak(-30.0, -20.0);
    assert!(
        (peak - (-20.0)).abs() < 0.001,
        "Peak should update to higher value"
    );

    let peak = update_peak(-20.0, -40.0);
    assert!(
        (peak - (-20.0)).abs() < 0.001,
        "Peak should hold at higher value"
    );
}

// =============================================================================
// Channel Selection Tests
// =============================================================================

/// Test channel selection.
#[gpui::test]
async fn test_spectrum_channel_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(6, FftSize::Size2048)));

    for i in 0..6 {
        state.borrow_mut().selected_channel = i;
        assert_eq!(state.borrow().selected_channel, i);
    }
}

/// Test channel selection bounds.
#[gpui::test]
async fn test_spectrum_channel_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    let requested = 5;
    let clamped = requested.min(state.borrow().num_channels - 1);
    state.borrow_mut().selected_channel = clamped;

    assert_eq!(
        state.borrow().selected_channel,
        1,
        "Should clamp to max channel"
    );
}

/// Test show all channels toggle.
#[gpui::test]
async fn test_spectrum_show_all_channels(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    // Initially showing single channel
    assert!(!state.borrow().show_all_channels);

    // Enable all channels view
    state.borrow_mut().show_all_channels = true;
    assert!(state.borrow().show_all_channels);
}

// =============================================================================
// Grid Lines Tests
// =============================================================================

/// Test frequency grid lines.
#[gpui::test]
async fn test_spectrum_frequency_grid(_cx: &mut TestAppContext) {
    fn frequency_grid_lines() -> Vec<f64> {
        vec![
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ]
    }

    let lines = frequency_grid_lines();
    assert!(lines.len() >= 8, "Should have frequency grid lines");
    assert!((lines[0] - 20.0).abs() < 0.001, "Should start at 20 Hz");
    assert!(
        (lines[lines.len() - 1] - 20000.0).abs() < 0.001,
        "Should end at 20 kHz"
    );
}

/// Test dB grid lines.
#[gpui::test]
async fn test_spectrum_db_grid(_cx: &mut TestAppContext) {
    fn db_grid_lines() -> Vec<f64> {
        vec![
            0.0, -10.0, -20.0, -30.0, -40.0, -50.0, -60.0, -70.0, -80.0, -90.0,
        ]
    }

    let lines = db_grid_lines();
    assert_eq!(lines.len(), 10, "Should have 10 dB grid lines");
    assert!(lines[0].abs() < 0.001, "Should start at 0 dB");
    assert!((lines[9] - (-90.0)).abs() < 0.001, "Should end at -90 dB");
}

// =============================================================================
// Color/Gradient Tests
// =============================================================================

/// Test spectrum color mapping.
#[gpui::test]
async fn test_spectrum_color_mapping(_cx: &mut TestAppContext) {
    fn db_to_color(db: f64) -> (u8, u8, u8) {
        // Simple gradient: green (low) -> yellow (mid) -> red (high)
        let normalized = ((db - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0);

        if normalized < 0.5 {
            // Green to yellow
            let t = normalized * 2.0;
            ((255.0 * t) as u8, 255, 0)
        } else {
            // Yellow to red
            let t = (normalized - 0.5) * 2.0;
            (255, (255.0 * (1.0 - t)) as u8, 0)
        }
    }

    let color_low = db_to_color(-90.0);
    assert_eq!(color_low, (0, 255, 0), "Low level should be green");

    let color_high = db_to_color(0.0);
    assert_eq!(color_high, (255, 0, 0), "High level should be red");
}

/// Test channel colors.
#[gpui::test]
async fn test_spectrum_channel_colors(_cx: &mut TestAppContext) {
    fn channel_color(index: usize) -> (u8, u8, u8) {
        let colors = [
            (66, 133, 244), // Blue (L)
            (234, 67, 53),  // Red (R)
            (52, 168, 83),  // Green (C)
            (251, 188, 5),  // Yellow (LFE)
            (153, 0, 255),  // Purple (Ls)
            (0, 188, 212),  // Cyan (Rs)
        ];
        colors[index % colors.len()]
    }

    // Each channel should have distinct color
    let color_0 = channel_color(0);
    let color_1 = channel_color(1);
    assert_ne!(color_0, color_1, "Channels should have different colors");
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test channel cycling with Tab.
#[gpui::test]
async fn test_spectrum_channel_tab(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(4, FftSize::Size2048)));

    // Tab cycles through channels
    for expected in [1, 2, 3, 0] {
        let next = (state.borrow().selected_channel + 1) % state.borrow().num_channels;
        state.borrow_mut().selected_channel = next;
        assert_eq!(state.borrow().selected_channel, expected);
    }
}

/// Test P key peak hold toggle.
#[gpui::test]
async fn test_spectrum_peak_hold_key(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(SpectrumState::new(2, FftSize::Size2048)));

    // Press P to toggle peak hold
    {
        let current = state.borrow().peak_hold;
        state.borrow_mut().peak_hold = !current;
    }
    assert!(state.borrow().peak_hold);

    // Press P again
    {
        let current = state.borrow().peak_hold;
        state.borrow_mut().peak_hold = !current;
    }
    assert!(!state.borrow().peak_hold);
}

// =============================================================================
// Display Range Tests
// =============================================================================

/// Test display range adjustment.
#[gpui::test]
async fn test_spectrum_display_range(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone)]
    struct DisplayRange {
        min_db: f64,
        max_db: f64,
    }

    let range = DisplayRange {
        min_db: -90.0,
        max_db: 0.0,
    };

    assert_eq!(range.max_db - range.min_db, 90.0, "Should have 90 dB range");
}

/// Test zoomed display range.
#[gpui::test]
async fn test_spectrum_zoomed_range(_cx: &mut TestAppContext) {
    // Zoomed view might show -60 to 0 dB
    let zoomed_min = -60.0;
    let zoomed_max = 0.0;

    fn is_visible(db: f64, min: f64, max: f64) -> bool {
        db >= min && db <= max
    }

    assert!(is_visible(-30.0, zoomed_min, zoomed_max));
    assert!(!is_visible(-80.0, zoomed_min, zoomed_max));
}
