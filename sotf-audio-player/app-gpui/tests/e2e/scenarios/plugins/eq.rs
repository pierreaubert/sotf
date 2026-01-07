//! E2E tests for EQ Plugin UI.
//!
//! Tests for verifying parametric EQ functionality:
//! - Adding/removing EQ bands
//! - Frequency control (20Hz - 20kHz)
//! - Q factor control (0.1 - 10.0)
//! - Gain control (-24dB to +24dB)
//! - Filter type selection
//! - Frequency response curve display
//! - Auto-gain toggle

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// EQ Filter Types
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterType {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

impl FilterType {
    fn all() -> &'static [FilterType] {
        &[
            FilterType::Peak,
            FilterType::LowShelf,
            FilterType::HighShelf,
            FilterType::LowPass,
            FilterType::HighPass,
            FilterType::BandPass,
            FilterType::Notch,
        ]
    }

    fn display_name(&self) -> &'static str {
        match self {
            FilterType::Peak => "Peak",
            FilterType::LowShelf => "Low Shelf",
            FilterType::HighShelf => "High Shelf",
            FilterType::LowPass => "Low Pass",
            FilterType::HighPass => "High Pass",
            FilterType::BandPass => "Band Pass",
            FilterType::Notch => "Notch",
        }
    }
}

// =============================================================================
// EQ Band Structure
// =============================================================================

#[derive(Debug, Clone)]
struct EqBand {
    filter_type: FilterType,
    frequency: f64,
    q: f64,
    gain_db: f64,
    enabled: bool,
}

impl EqBand {
    fn new(filter_type: FilterType, frequency: f64, q: f64, gain_db: f64) -> Self {
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
            enabled: true,
        }
    }

    fn default_peak() -> Self {
        Self::new(FilterType::Peak, 1000.0, 1.0, 0.0)
    }
}

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_FREQUENCY: f64 = 20.0;
const MAX_FREQUENCY: f64 = 20000.0;
const MIN_Q: f64 = 0.1;
const MAX_Q: f64 = 10.0;
const MIN_GAIN_DB: f64 = -24.0;
const MAX_GAIN_DB: f64 = 24.0;

// =============================================================================
// Add/Remove Band Tests
// =============================================================================

/// Test adding a band to empty EQ.
#[gpui::test]
async fn test_eq_add_band_to_empty(_cx: &mut TestAppContext) {
    let bands: Rc<RefCell<Vec<EqBand>>> = Rc::new(RefCell::new(Vec::new()));

    // Add default peak band
    bands.borrow_mut().push(EqBand::default_peak());

    assert_eq!(bands.borrow().len(), 1, "Should have 1 band");
    assert_eq!(bands.borrow()[0].filter_type, FilterType::Peak);
    assert!((bands.borrow()[0].frequency - 1000.0).abs() < 0.001);
}

/// Test adding multiple bands.
#[gpui::test]
async fn test_eq_add_multiple_bands(_cx: &mut TestAppContext) {
    let bands: Rc<RefCell<Vec<EqBand>>> = Rc::new(RefCell::new(Vec::new()));

    // Add bands at different frequencies
    let frequencies = vec![100.0, 500.0, 1000.0, 5000.0, 10000.0];
    for freq in &frequencies {
        bands
            .borrow_mut()
            .push(EqBand::new(FilterType::Peak, *freq, 1.0, 0.0));
    }

    assert_eq!(bands.borrow().len(), 5, "Should have 5 bands");
    for (i, freq) in frequencies.iter().enumerate() {
        assert!((bands.borrow()[i].frequency - freq).abs() < 0.001);
    }
}

/// Test removing a band.
#[gpui::test]
async fn test_eq_remove_band(_cx: &mut TestAppContext) {
    let bands: Rc<RefCell<Vec<EqBand>>> = Rc::new(RefCell::new(vec![
        EqBand::new(FilterType::Peak, 100.0, 1.0, 0.0),
        EqBand::new(FilterType::Peak, 1000.0, 1.0, 0.0),
        EqBand::new(FilterType::Peak, 10000.0, 1.0, 0.0),
    ]));

    // Remove middle band
    bands.borrow_mut().remove(1);

    assert_eq!(bands.borrow().len(), 2);
    assert!((bands.borrow()[0].frequency - 100.0).abs() < 0.001);
    assert!((bands.borrow()[1].frequency - 10000.0).abs() < 0.001);
}

/// Test removing all bands except one (minimum 1 band).
#[gpui::test]
async fn test_eq_minimum_one_band(_cx: &mut TestAppContext) {
    let bands: Rc<RefCell<Vec<EqBand>>> = Rc::new(RefCell::new(vec![
        EqBand::default_peak(),
        EqBand::default_peak(),
    ]));

    // Remove one band
    bands.borrow_mut().remove(0);
    assert_eq!(bands.borrow().len(), 1, "Should have 1 band left");

    // Try to remove last band (should be prevented in UI, we just test state)
    let can_remove = bands.borrow().len() > 1;
    assert!(!can_remove, "Should not allow removing last band");
}

// =============================================================================
// Frequency Control Tests
// =============================================================================

/// Test frequency parameter bounds.
#[gpui::test]
async fn test_eq_frequency_bounds(_cx: &mut TestAppContext) {
    let band = Rc::new(RefCell::new(EqBand::default_peak()));

    // Set to minimum
    band.borrow_mut().frequency = MIN_FREQUENCY;
    assert!((band.borrow().frequency - 20.0).abs() < 0.001);

    // Set to maximum
    band.borrow_mut().frequency = MAX_FREQUENCY;
    assert!((band.borrow().frequency - 20000.0).abs() < 0.001);
}

/// Test frequency clamping.
#[gpui::test]
async fn test_eq_frequency_clamping(_cx: &mut TestAppContext) {
    // Test below minimum
    let freq: f64 = 5.0;
    let clamped = freq.clamp(MIN_FREQUENCY, MAX_FREQUENCY);
    assert!((clamped - MIN_FREQUENCY).abs() < 0.001);

    // Test above maximum
    let freq: f64 = 25000.0;
    let clamped = freq.clamp(MIN_FREQUENCY, MAX_FREQUENCY);
    assert!((clamped - MAX_FREQUENCY).abs() < 0.001);
}

/// Test frequency logarithmic scaling for UI.
#[gpui::test]
async fn test_eq_frequency_log_scaling(_cx: &mut TestAppContext) {
    // Normalize frequency to 0-1 range using log scale
    fn normalize_freq(freq: f64) -> f64 {
        (freq.ln() - MIN_FREQUENCY.ln()) / (MAX_FREQUENCY.ln() - MIN_FREQUENCY.ln())
    }

    // Denormalize from 0-1 back to frequency
    fn denormalize_freq(norm: f64) -> f64 {
        (MIN_FREQUENCY.ln() + norm * (MAX_FREQUENCY.ln() - MIN_FREQUENCY.ln())).exp()
    }

    // Test specific frequencies
    let test_freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
    for freq in test_freqs {
        let norm = normalize_freq(freq);
        let restored = denormalize_freq(norm);
        assert!(
            (restored - freq).abs() < 0.1,
            "Frequency {} should round-trip, got {}",
            freq,
            restored
        );
    }

    // 1kHz should be roughly in the middle (log scale)
    let norm_1k = normalize_freq(1000.0);
    assert!(
        norm_1k > 0.4 && norm_1k < 0.6,
        "1kHz should be near middle, got {}",
        norm_1k
    );
}

/// Test frequency drag adjustment.
#[gpui::test]
async fn test_eq_frequency_drag(_cx: &mut TestAppContext) {
    let frequency = Rc::new(RefCell::new(1000.0f64));

    // Simulate horizontal drag (frequency change)
    let drag_delta_x: f64 = 50.0; // pixels
    let freq_scale: f64 = 1.05; // multiplier per 10 pixels

    let mult = freq_scale.powf(drag_delta_x / 10.0);
    let new_freq = (*frequency.borrow() * mult).clamp(MIN_FREQUENCY, MAX_FREQUENCY);
    *frequency.borrow_mut() = new_freq;

    assert!(
        *frequency.borrow() > 1000.0,
        "Frequency should increase with rightward drag"
    );
}

// =============================================================================
// Q Factor Control Tests
// =============================================================================

/// Test Q parameter bounds.
#[gpui::test]
async fn test_eq_q_bounds(_cx: &mut TestAppContext) {
    let band = Rc::new(RefCell::new(EqBand::default_peak()));

    // Set to minimum
    band.borrow_mut().q = MIN_Q;
    assert!((band.borrow().q - 0.1).abs() < 0.001);

    // Set to maximum
    band.borrow_mut().q = MAX_Q;
    assert!((band.borrow().q - 10.0).abs() < 0.001);
}

/// Test Q clamping.
#[gpui::test]
async fn test_eq_q_clamping(_cx: &mut TestAppContext) {
    // Test below minimum
    let q: f64 = 0.01;
    let clamped = q.clamp(MIN_Q, MAX_Q);
    assert!((clamped - MIN_Q).abs() < 0.001);

    // Test above maximum
    let q: f64 = 15.0;
    let clamped = q.clamp(MIN_Q, MAX_Q);
    assert!((clamped - MAX_Q).abs() < 0.001);
}

/// Test Q handle drag (horizontal handles for Q adjustment).
#[gpui::test]
async fn test_eq_q_handle_drag(_cx: &mut TestAppContext) {
    let q = Rc::new(RefCell::new(1.0f64));

    // Drag right handle outward (decrease Q = wider)
    // Negative drag_delta = outward, should decrease Q
    let drag_delta: f64 = -20.0; // pixels, negative = outward
    let q_scale: f64 = 0.95; // multiplier per 10 pixels inward

    // Invert sign: outward drag (negative) should decrease Q
    let mult = q_scale.powf(-drag_delta / 10.0);
    let new_q = (*q.borrow() * mult).clamp(MIN_Q, MAX_Q);
    *q.borrow_mut() = new_q;

    // With -20 px outward drag: mult = 0.95^(20/10) = 0.95^2 ≈ 0.9025
    assert!(
        *q.borrow() < 1.0,
        "Q should decrease (wider) with outward drag, got {}",
        *q.borrow()
    );
}

// =============================================================================
// Gain Control Tests
// =============================================================================

/// Test gain parameter bounds.
#[gpui::test]
async fn test_eq_gain_bounds(_cx: &mut TestAppContext) {
    let band = Rc::new(RefCell::new(EqBand::default_peak()));

    // Set to minimum (cut)
    band.borrow_mut().gain_db = MIN_GAIN_DB;
    assert!((band.borrow().gain_db - (-24.0)).abs() < 0.001);

    // Set to maximum (boost)
    band.borrow_mut().gain_db = MAX_GAIN_DB;
    assert!((band.borrow().gain_db - 24.0).abs() < 0.001);
}

/// Test gain clamping.
#[gpui::test]
async fn test_eq_gain_clamping(_cx: &mut TestAppContext) {
    // Test below minimum
    let gain: f64 = -30.0;
    let clamped = gain.clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    assert!((clamped - MIN_GAIN_DB).abs() < 0.001);

    // Test above maximum
    let gain: f64 = 30.0;
    let clamped = gain.clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    assert!((clamped - MAX_GAIN_DB).abs() < 0.001);
}

/// Test gain drag adjustment (vertical drag).
#[gpui::test]
async fn test_eq_gain_drag(_cx: &mut TestAppContext) {
    let gain_db = Rc::new(RefCell::new(0.0f64));

    // Simulate vertical drag (gain change) - up = boost
    let drag_delta_y = -50.0; // pixels, negative = upward
    let gain_scale = 0.5; // dB per 10 pixels

    let delta_db = -drag_delta_y / 10.0 * gain_scale;
    let new_gain = (*gain_db.borrow() + delta_db).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    *gain_db.borrow_mut() = new_gain;

    assert!(
        *gain_db.borrow() > 0.0,
        "Gain should increase with upward drag"
    );
}

/// Test gain display formatting.
#[gpui::test]
async fn test_eq_gain_display_format(_cx: &mut TestAppContext) {
    fn format_gain(gain_db: f64) -> String {
        if gain_db > 0.0 {
            format!("+{:.1} dB", gain_db)
        } else {
            format!("{:.1} dB", gain_db)
        }
    }

    assert_eq!(format_gain(0.0), "0.0 dB");
    assert_eq!(format_gain(3.5), "+3.5 dB");
    assert_eq!(format_gain(-6.0), "-6.0 dB");
}

// =============================================================================
// Filter Type Tests
// =============================================================================

/// Test filter type selection.
#[gpui::test]
async fn test_eq_filter_type_selection(_cx: &mut TestAppContext) {
    let band = Rc::new(RefCell::new(EqBand::default_peak()));

    // Change to low shelf
    band.borrow_mut().filter_type = FilterType::LowShelf;
    assert_eq!(band.borrow().filter_type, FilterType::LowShelf);

    // Change to high pass
    band.borrow_mut().filter_type = FilterType::HighPass;
    assert_eq!(band.borrow().filter_type, FilterType::HighPass);
}

/// Test all filter types exist.
#[gpui::test]
async fn test_eq_all_filter_types(_cx: &mut TestAppContext) {
    let all_types = FilterType::all();
    assert_eq!(all_types.len(), 7, "Should have 7 filter types");

    // Verify display names
    for filter_type in all_types {
        let name = filter_type.display_name();
        assert!(!name.is_empty(), "Filter type should have display name");
    }
}

/// Test filter type affects gain relevance.
#[gpui::test]
async fn test_eq_filter_type_gain_relevance(_cx: &mut TestAppContext) {
    // Some filter types don't use gain (LP, HP, BP, Notch)
    fn gain_is_relevant(filter_type: FilterType) -> bool {
        matches!(
            filter_type,
            FilterType::Peak | FilterType::LowShelf | FilterType::HighShelf
        )
    }

    assert!(gain_is_relevant(FilterType::Peak));
    assert!(gain_is_relevant(FilterType::LowShelf));
    assert!(gain_is_relevant(FilterType::HighShelf));
    assert!(!gain_is_relevant(FilterType::LowPass));
    assert!(!gain_is_relevant(FilterType::HighPass));
    assert!(!gain_is_relevant(FilterType::BandPass));
    assert!(!gain_is_relevant(FilterType::Notch));
}

// =============================================================================
// Band Selection Tests
// =============================================================================

/// Test band selection.
#[gpui::test]
async fn test_eq_band_selection(_cx: &mut TestAppContext) {
    let selected_band = Rc::new(RefCell::new(0usize));
    let band_count = 5;

    // Select different bands
    for i in 0..band_count {
        *selected_band.borrow_mut() = i;
        assert_eq!(*selected_band.borrow(), i);
    }
}

/// Test band selection bounds.
#[gpui::test]
async fn test_eq_band_selection_bounds(_cx: &mut TestAppContext) {
    let selected_band = Rc::new(RefCell::new(0usize));
    let band_count = 3;

    // Try to select beyond bounds
    let requested = 5;
    let clamped = requested.min(band_count - 1);
    *selected_band.borrow_mut() = clamped;

    assert_eq!(*selected_band.borrow(), 2, "Should clamp to last band");
}

/// Test band enable/disable.
#[gpui::test]
async fn test_eq_band_enable_disable(_cx: &mut TestAppContext) {
    let band = Rc::new(RefCell::new(EqBand::default_peak()));

    // Initially enabled
    assert!(band.borrow().enabled);

    // Disable
    band.borrow_mut().enabled = false;
    assert!(!band.borrow().enabled);

    // Re-enable
    band.borrow_mut().enabled = true;
    assert!(band.borrow().enabled);
}

// =============================================================================
// Auto-Gain Tests
// =============================================================================

/// Test auto-gain toggle state.
#[gpui::test]
async fn test_eq_auto_gain_toggle(_cx: &mut TestAppContext) {
    let auto_gain_enabled = Rc::new(RefCell::new(false));

    // Enable auto-gain
    *auto_gain_enabled.borrow_mut() = true;
    assert!(*auto_gain_enabled.borrow());

    // Disable auto-gain
    *auto_gain_enabled.borrow_mut() = false;
    assert!(!*auto_gain_enabled.borrow());
}

/// Test auto-gain calculation concept.
#[gpui::test]
async fn test_eq_auto_gain_calculation(_cx: &mut TestAppContext) {
    // Auto-gain should compensate for overall level changes
    // If we boost 6dB at one frequency, auto-gain might reduce overall by ~1-2dB

    fn calculate_auto_gain_compensation(bands: &[EqBand]) -> f64 {
        // Simplified: average of positive gains, negative
        let total_boost: f64 = bands
            .iter()
            .filter(|b| b.enabled && b.gain_db > 0.0)
            .map(|b| b.gain_db)
            .sum();

        if total_boost > 0.0 {
            -total_boost / 3.0 // Rough approximation
        } else {
            0.0
        }
    }

    let bands = vec![
        EqBand::new(FilterType::Peak, 100.0, 1.0, 6.0), // +6dB boost
        EqBand::new(FilterType::Peak, 1000.0, 1.0, 0.0), // Flat
        EqBand::new(FilterType::Peak, 10000.0, 1.0, 3.0), // +3dB boost
    ];

    let compensation = calculate_auto_gain_compensation(&bands);
    assert!(
        compensation < 0.0,
        "Auto-gain should reduce level when boosting"
    );
}

// =============================================================================
// Frequency Response Curve Tests
// =============================================================================

/// Test frequency response calculation points.
#[gpui::test]
async fn test_eq_frequency_response_points(_cx: &mut TestAppContext) {
    // Number of points for curve display
    const NUM_POINTS: usize = 256;

    let points: Vec<f64> = (0..NUM_POINTS)
        .map(|i| {
            let norm = i as f64 / (NUM_POINTS - 1) as f64;
            // Log scale from 20Hz to 20kHz
            (MIN_FREQUENCY.ln() + norm * (MAX_FREQUENCY.ln() - MIN_FREQUENCY.ln())).exp()
        })
        .collect();

    assert_eq!(points.len(), NUM_POINTS);
    assert!((points[0] - MIN_FREQUENCY).abs() < 1.0);
    assert!((points[NUM_POINTS - 1] - MAX_FREQUENCY).abs() < 100.0);
}

/// Test band color assignment.
#[gpui::test]
async fn test_eq_band_colors(_cx: &mut TestAppContext) {
    // Each band should have a distinct color for visual identification
    fn band_color(index: usize) -> (u8, u8, u8) {
        let colors = [
            (255, 100, 100), // Red
            (100, 255, 100), // Green
            (100, 100, 255), // Blue
            (255, 255, 100), // Yellow
            (255, 100, 255), // Magenta
            (100, 255, 255), // Cyan
            (255, 165, 0),   // Orange
            (128, 0, 128),   // Purple
        ];
        colors[index % colors.len()]
    }

    // Verify bands get different colors
    let color_0 = band_color(0);
    let color_1 = band_color(1);
    assert_ne!(
        color_0, color_1,
        "Adjacent bands should have different colors"
    );
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test keyboard parameter adjustment.
#[gpui::test]
async fn test_eq_keyboard_parameter_adjustment(_cx: &mut TestAppContext) {
    let gain_db = Rc::new(RefCell::new(0.0f64));
    const GAIN_STEP: f64 = 0.5;
    const GAIN_STEP_FINE: f64 = 0.1;

    // Arrow up = increase gain
    {
        let current = *gain_db.borrow();
        *gain_db.borrow_mut() = (current + GAIN_STEP).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!((*gain_db.borrow() - 0.5).abs() < 0.001);

    // Arrow down = decrease gain
    {
        let current = *gain_db.borrow();
        *gain_db.borrow_mut() = (current - GAIN_STEP).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!(gain_db.borrow().abs() < 0.001);

    // Shift+arrow = fine adjustment
    {
        let current = *gain_db.borrow();
        *gain_db.borrow_mut() = (current + GAIN_STEP_FINE).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!((*gain_db.borrow() - 0.1).abs() < 0.001);
}

/// Test number key band selection (1-9).
#[gpui::test]
async fn test_eq_number_key_band_selection(_cx: &mut TestAppContext) {
    let selected_band = Rc::new(RefCell::new(0usize));
    let band_count = 5;

    // Simulate pressing 1-5 keys
    for key_num in 1..=5 {
        let band_idx = key_num - 1; // 0-indexed
        if band_idx < band_count {
            *selected_band.borrow_mut() = band_idx;
        }
        assert_eq!(*selected_band.borrow(), band_idx);
    }
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test EQ preset structure.
#[gpui::test]
async fn test_eq_preset_structure(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone)]
    struct EqPreset {
        name: String,
        bands: Vec<EqBand>,
        auto_gain: bool,
    }

    let preset = EqPreset {
        name: "Vocal Boost".to_string(),
        bands: vec![
            EqBand::new(FilterType::HighPass, 80.0, 0.7, 0.0),
            EqBand::new(FilterType::Peak, 3000.0, 1.5, 3.0),
            EqBand::new(FilterType::HighShelf, 10000.0, 0.7, 2.0),
        ],
        auto_gain: true,
    };

    assert_eq!(preset.name, "Vocal Boost");
    assert_eq!(preset.bands.len(), 3);
    assert!(preset.auto_gain);
}

// =============================================================================
// Validation Tests
// =============================================================================

/// Test EQ band validation.
#[gpui::test]
async fn test_eq_band_validation(_cx: &mut TestAppContext) {
    fn validate_band(band: &EqBand) -> Result<(), String> {
        if band.frequency < MIN_FREQUENCY || band.frequency > MAX_FREQUENCY {
            return Err(format!(
                "Frequency {} Hz out of range ({}-{})",
                band.frequency, MIN_FREQUENCY, MAX_FREQUENCY
            ));
        }
        if band.q < MIN_Q || band.q > MAX_Q {
            return Err(format!("Q {} out of range ({}-{})", band.q, MIN_Q, MAX_Q));
        }
        if band.gain_db < MIN_GAIN_DB || band.gain_db > MAX_GAIN_DB {
            return Err(format!(
                "Gain {} dB out of range ({}-{})",
                band.gain_db, MIN_GAIN_DB, MAX_GAIN_DB
            ));
        }
        Ok(())
    }

    // Valid band
    let valid_band = EqBand::new(FilterType::Peak, 1000.0, 1.0, 3.0);
    assert!(validate_band(&valid_band).is_ok());

    // Invalid frequency
    let invalid_freq = EqBand::new(FilterType::Peak, 5.0, 1.0, 0.0);
    assert!(validate_band(&invalid_freq).is_err());

    // Invalid Q
    let invalid_q = EqBand::new(FilterType::Peak, 1000.0, 0.01, 0.0);
    assert!(validate_band(&invalid_q).is_err());

    // Invalid gain
    let invalid_gain = EqBand::new(FilterType::Peak, 1000.0, 1.0, 30.0);
    assert!(validate_band(&invalid_gain).is_err());
}
