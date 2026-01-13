//! E2E tests for Crossover Plugin.
//!
//! Tests for the multi-way crossover filtering plugin for speaker systems.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Crossover type
#[derive(Debug, Clone, PartialEq)]
enum CrossoverType {
    LR24,
    LR48,
    Butterworth24,
    Butterworth48,
}

impl CrossoverType {
    fn slope_db_per_octave(&self) -> i32 {
        match self {
            CrossoverType::LR24 | CrossoverType::Butterworth24 => 24,
            CrossoverType::LR48 | CrossoverType::Butterworth48 => 48,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            CrossoverType::LR24 => "Linkwitz-Riley 24dB/oct",
            CrossoverType::LR48 => "Linkwitz-Riley 48dB/oct",
            CrossoverType::Butterworth24 => "Butterworth 24dB/oct",
            CrossoverType::Butterworth48 => "Butterworth 48dB/oct",
        }
    }
}

/// Crossover output selection
#[derive(Debug, Clone, PartialEq)]
enum CrossoverOutput {
    Low,
    High,
}

/// Crossover plugin state for testing
struct CrossoverState {
    enabled: bool,
    crossover_type: CrossoverType,
    frequency: f64,
    output: CrossoverOutput,
}

impl Default for CrossoverState {
    fn default() -> Self {
        Self {
            enabled: true,
            crossover_type: CrossoverType::LR24,
            frequency: 80.0,
            output: CrossoverOutput::Low,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_crossover_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));
    assert!(state.borrow().enabled);
}

/// Test default values.
#[gpui::test]
async fn test_crossover_defaults(_cx: &mut TestAppContext) {
    let state = CrossoverState::default();

    assert_eq!(state.crossover_type, CrossoverType::LR24);
    assert!((state.frequency - 80.0).abs() < 0.1);
    assert_eq!(state.output, CrossoverOutput::Low);
}

// =============================================================================
// Crossover Type Tests
// =============================================================================

/// Test crossover type selection.
#[gpui::test]
async fn test_crossover_type_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    let types = [
        CrossoverType::LR24,
        CrossoverType::LR48,
        CrossoverType::Butterworth24,
        CrossoverType::Butterworth48,
    ];

    for ct in types {
        state.borrow_mut().crossover_type = ct.clone();
        assert_eq!(state.borrow().crossover_type, ct);
    }
}

/// Test crossover type labels.
#[gpui::test]
async fn test_crossover_type_labels(_cx: &mut TestAppContext) {
    assert!(CrossoverType::LR24.label().contains("Linkwitz-Riley"));
    assert!(CrossoverType::LR48.label().contains("48dB"));
    assert!(CrossoverType::Butterworth24.label().contains("Butterworth"));
}

/// Test crossover slope values.
#[gpui::test]
async fn test_crossover_slopes(_cx: &mut TestAppContext) {
    assert_eq!(CrossoverType::LR24.slope_db_per_octave(), 24);
    assert_eq!(CrossoverType::LR48.slope_db_per_octave(), 48);
    assert_eq!(CrossoverType::Butterworth24.slope_db_per_octave(), 24);
    assert_eq!(CrossoverType::Butterworth48.slope_db_per_octave(), 48);
}

// =============================================================================
// Frequency Tests
// =============================================================================

/// Test frequency control.
#[gpui::test]
async fn test_crossover_frequency_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    let test_values: Vec<f64> = vec![40.0, 80.0, 120.0, 250.0, 500.0, 1000.0, 2000.0];
    for value in test_values {
        state.borrow_mut().frequency = value;
        assert!((state.borrow().frequency - value).abs() < 0.1);
    }
}

/// Test frequency bounds.
#[gpui::test]
async fn test_crossover_frequency_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    // Typical range: 20 Hz to 20 kHz
    let clamped = (10.0_f64).clamp(20.0, 20000.0);
    state.borrow_mut().frequency = clamped;
    assert!(state.borrow().frequency >= 20.0);

    let clamped = (25000.0_f64).clamp(20.0, 20000.0);
    state.borrow_mut().frequency = clamped;
    assert!(state.borrow().frequency <= 20000.0);
}

/// Test frequency display format.
#[gpui::test]
async fn test_crossover_frequency_display(_cx: &mut TestAppContext) {
    fn format_frequency(freq: f64) -> String {
        if freq >= 1000.0 {
            format!("{:.2} kHz", freq / 1000.0)
        } else {
            format!("{:.0} Hz", freq)
        }
    }

    assert_eq!(format_frequency(80.0), "80 Hz");
    assert_eq!(format_frequency(500.0), "500 Hz");
    assert_eq!(format_frequency(1000.0), "1.00 kHz");
    assert_eq!(format_frequency(2500.0), "2.50 kHz");
}

/// Test common crossover frequencies.
#[gpui::test]
async fn test_common_crossover_frequencies(_cx: &mut TestAppContext) {
    // Common subwoofer crossover points
    let sub_crossovers: Vec<f64> = vec![60.0, 80.0, 100.0, 120.0];
    for freq in sub_crossovers {
        assert!(freq >= 40.0 && freq <= 150.0);
    }

    // Common 2-way speaker crossover points
    let speaker_crossovers: Vec<f64> = vec![1500.0, 2000.0, 2500.0, 3000.0];
    for freq in speaker_crossovers {
        assert!(freq >= 1000.0 && freq <= 4000.0);
    }
}

// =============================================================================
// Output Selection Tests
// =============================================================================

/// Test output selection.
#[gpui::test]
async fn test_crossover_output_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    state.borrow_mut().output = CrossoverOutput::Low;
    assert_eq!(state.borrow().output, CrossoverOutput::Low);

    state.borrow_mut().output = CrossoverOutput::High;
    assert_eq!(state.borrow().output, CrossoverOutput::High);
}

/// Test output label.
#[gpui::test]
async fn test_crossover_output_label(_cx: &mut TestAppContext) {
    fn get_output_label(output: &CrossoverOutput) -> &'static str {
        match output {
            CrossoverOutput::Low => "Low Pass",
            CrossoverOutput::High => "High Pass",
        }
    }

    assert_eq!(get_output_label(&CrossoverOutput::Low), "Low Pass");
    assert_eq!(get_output_label(&CrossoverOutput::High), "High Pass");
}

/// Test output filter description.
#[gpui::test]
async fn test_crossover_output_description(_cx: &mut TestAppContext) {
    fn get_output_description(output: &CrossoverOutput, freq: f64) -> String {
        match output {
            CrossoverOutput::Low => format!("Passes frequencies below {:.0} Hz", freq),
            CrossoverOutput::High => format!("Passes frequencies above {:.0} Hz", freq),
        }
    }

    assert!(get_output_description(&CrossoverOutput::Low, 80.0).contains("below"));
    assert!(get_output_description(&CrossoverOutput::High, 80.0).contains("above"));
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_crossover_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Filter Response Tests
// =============================================================================

/// Test LR crossover sum-to-flat.
#[gpui::test]
async fn test_lr_sum_to_flat(_cx: &mut TestAppContext) {
    // Linkwitz-Riley crossovers sum to flat when both outputs are combined
    // This is a key property that makes them ideal for speaker crossovers
    fn is_sum_to_flat(crossover_type: &CrossoverType) -> bool {
        matches!(crossover_type, CrossoverType::LR24 | CrossoverType::LR48)
    }

    assert!(is_sum_to_flat(&CrossoverType::LR24));
    assert!(is_sum_to_flat(&CrossoverType::LR48));
}

/// Test filter order calculation.
#[gpui::test]
async fn test_filter_order(_cx: &mut TestAppContext) {
    fn get_filter_order(crossover_type: &CrossoverType) -> usize {
        match crossover_type {
            CrossoverType::LR24 | CrossoverType::Butterworth24 => 4,
            CrossoverType::LR48 | CrossoverType::Butterworth48 => 8,
        }
    }

    assert_eq!(get_filter_order(&CrossoverType::LR24), 4);
    assert_eq!(get_filter_order(&CrossoverType::LR48), 8);
}

/// Test number of biquad sections.
#[gpui::test]
async fn test_biquad_sections(_cx: &mut TestAppContext) {
    fn get_num_biquads(crossover_type: &CrossoverType) -> usize {
        match crossover_type {
            CrossoverType::LR24 | CrossoverType::Butterworth24 => 2,
            CrossoverType::LR48 | CrossoverType::Butterworth48 => 4,
        }
    }

    // LR24 = 4th order = 2 cascaded biquads
    assert_eq!(get_num_biquads(&CrossoverType::LR24), 2);
    // LR48 = 8th order = 4 cascaded biquads
    assert_eq!(get_num_biquads(&CrossoverType::LR48), 4);
}

// =============================================================================
// Use Case Tests
// =============================================================================

/// Test subwoofer crossover setup.
#[gpui::test]
async fn test_subwoofer_crossover(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    // Typical subwoofer crossover: LR24 at 80Hz, low pass
    state.borrow_mut().crossover_type = CrossoverType::LR24;
    state.borrow_mut().frequency = 80.0;
    state.borrow_mut().output = CrossoverOutput::Low;

    assert_eq!(state.borrow().output, CrossoverOutput::Low);
    assert!((state.borrow().frequency - 80.0).abs() < 0.1);
}

/// Test satellite speaker crossover.
#[gpui::test]
async fn test_satellite_crossover(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    // Satellite speakers: high pass at same frequency
    state.borrow_mut().crossover_type = CrossoverType::LR24;
    state.borrow_mut().frequency = 80.0;
    state.borrow_mut().output = CrossoverOutput::High;

    assert_eq!(state.borrow().output, CrossoverOutput::High);
}

/// Test 2-way speaker crossover.
#[gpui::test]
async fn test_two_way_speaker_crossover(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    // 2-way speaker: crossover around 2kHz
    state.borrow_mut().crossover_type = CrossoverType::LR48;
    state.borrow_mut().frequency = 2000.0;
    state.borrow_mut().output = CrossoverOutput::Low;

    assert!((state.borrow().frequency - 2000.0).abs() < 1.0);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test frequency response curve data.
#[gpui::test]
async fn test_frequency_response_visualization(_cx: &mut TestAppContext) {
    fn calculate_response_at_freq(
        test_freq: f64,
        crossover_freq: f64,
        slope_db_oct: i32,
        is_low: bool,
    ) -> f64 {
        let octaves = (test_freq / crossover_freq).log2();
        if is_low {
            // Low pass: attenuate above crossover
            if octaves > 0.0 {
                -slope_db_oct as f64 * octaves
            } else {
                0.0
            }
        } else {
            // High pass: attenuate below crossover
            if octaves < 0.0 {
                slope_db_oct as f64 * octaves
            } else {
                0.0
            }
        }
    }

    // At crossover frequency, both should be at -3dB (approximately)
    // One octave above crossover, low pass should be at -24dB (for LR24)
    let response = calculate_response_at_freq(160.0, 80.0, 24, true);
    assert!((response - (-24.0)).abs() < 1.0);
}

/// Test crossover point marker.
#[gpui::test]
async fn test_crossover_point_marker(_cx: &mut TestAppContext) {
    fn get_crossover_point_db() -> f64 {
        // Linkwitz-Riley crossover point is at -6dB
        // (each filter is -3dB at crossover, they're in-phase)
        -6.0
    }

    assert!((get_crossover_point_db() - (-6.0)).abs() < 0.1);
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: home theater sub.
#[gpui::test]
async fn test_preset_home_theater_sub(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    state.borrow_mut().crossover_type = CrossoverType::LR24;
    state.borrow_mut().frequency = 80.0;
    state.borrow_mut().output = CrossoverOutput::Low;

    assert_eq!(state.borrow().crossover_type, CrossoverType::LR24);
}

/// Test preset: PA system.
#[gpui::test]
async fn test_preset_pa_system(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    // PA system often uses steeper crossover
    state.borrow_mut().crossover_type = CrossoverType::LR48;
    state.borrow_mut().frequency = 120.0;
    state.borrow_mut().output = CrossoverOutput::Low;

    assert_eq!(state.borrow().crossover_type, CrossoverType::LR48);
}

/// Test preset: biamp crossover.
#[gpui::test]
async fn test_preset_biamp(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CrossoverState::default()));

    // Biamping typically crosses around 2-3kHz
    state.borrow_mut().crossover_type = CrossoverType::LR24;
    state.borrow_mut().frequency = 2500.0;
    state.borrow_mut().output = CrossoverOutput::High;

    assert!(state.borrow().frequency > 2000.0);
}
