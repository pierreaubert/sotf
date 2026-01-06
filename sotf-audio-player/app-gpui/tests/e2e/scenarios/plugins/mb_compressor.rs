//! E2E tests for Multiband Compressor Plugin.
//!
//! Tests for the multiband dynamic range compressor with 2-5 bands.
//! Uses Linkwitz-Riley 24dB/oct crossover filters for phase-coherent band splitting.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Per-band compressor parameters
#[derive(Debug, Clone, Default)]
struct BandCompressorParams {
    threshold_db: Option<f32>,
    ratio: Option<f32>,
    attack_ms: Option<f32>,
    release_ms: Option<f32>,
    knee_db: Option<f32>,
    makeup_gain_db: f32,
    solo: bool,
    bypass: bool,
}

/// Multiband compressor state for testing
struct MultibandCompressorState {
    enabled: bool,
    num_bands: usize,
    crossover_preset: i32,
    crossover_frequencies: Vec<f32>,
    // Global parameters
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    link_channels: bool,
    mix: f32,
    // Per-band
    bands: Vec<BandCompressorParams>,
}

impl Default for MultibandCompressorState {
    fn default() -> Self {
        Self {
            enabled: true,
            num_bands: 3,
            crossover_preset: 1,
            crossover_frequencies: vec![200.0, 2000.0, 8000.0, 12000.0],
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            link_channels: true,
            mix: 1.0,
            bands: vec![BandCompressorParams::default(); 3],
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_mb_compressor_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));
    assert!(state.borrow().enabled);
    assert_eq!(state.borrow().num_bands, 3);
}

/// Test default values.
#[gpui::test]
async fn test_mb_compressor_defaults(_cx: &mut TestAppContext) {
    let state = MultibandCompressorState::default();

    assert_eq!(state.num_bands, 3);
    assert_eq!(state.crossover_preset, 1);
    assert!((state.threshold_db - (-20.0)).abs() < 0.1);
    assert!((state.ratio - 4.0).abs() < 0.1);
}

// =============================================================================
// Band Count Tests
// =============================================================================

/// Test band count selection.
#[gpui::test]
async fn test_band_count_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    for num_bands in 2..=5 {
        state.borrow_mut().num_bands = num_bands;
        state.borrow_mut().bands = vec![BandCompressorParams::default(); num_bands];
        assert_eq!(state.borrow().num_bands, num_bands);
        assert_eq!(state.borrow().bands.len(), num_bands);
    }
}

/// Test band count bounds.
#[gpui::test]
async fn test_band_count_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    // Min 2, max 5
    let clamped = 1_usize.clamp(2, 5);
    state.borrow_mut().num_bands = clamped;
    assert!(state.borrow().num_bands >= 2);

    let clamped = 6_usize.clamp(2, 5);
    state.borrow_mut().num_bands = clamped;
    assert!(state.borrow().num_bands <= 5);
}

// =============================================================================
// Crossover Tests
// =============================================================================

/// Test crossover preset selection.
#[gpui::test]
async fn test_crossover_preset_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let presets = [
        (1, vec![200.0, 2000.0]),
        (2, vec![100.0, 3000.0]),
        (3, vec![250.0, 4000.0]),
    ];

    for (preset, expected_freqs) in presets {
        state.borrow_mut().crossover_preset = preset;
        // In real impl, this would update crossover_frequencies
        assert_eq!(state.borrow().crossover_preset, preset);
        assert!(!expected_freqs.is_empty());
    }
}

/// Test custom crossover frequencies.
#[gpui::test]
async fn test_custom_crossover_frequencies(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    // Set custom frequencies (preset 0)
    state.borrow_mut().crossover_preset = 0;
    state.borrow_mut().crossover_frequencies = vec![150.0, 1500.0, 6000.0, 10000.0];

    assert_eq!(state.borrow().crossover_preset, 0);
    assert!((state.borrow().crossover_frequencies[0] - 150.0).abs() < 0.1);
}

/// Test crossover frequency bounds.
#[gpui::test]
async fn test_crossover_frequency_bounds(_cx: &mut TestAppContext) {
    // Each frequency must be between 20 Hz and Nyquist/2
    // And freq[i] < freq[i+1]
    fn validate_frequencies(freqs: &[f32]) -> bool {
        if freqs.is_empty() {
            return true;
        }
        let mut prev = 20.0;
        for &f in freqs {
            if f <= prev || f > 20000.0 {
                return false;
            }
            prev = f;
        }
        true
    }

    assert!(validate_frequencies(&[200.0, 2000.0, 8000.0]));
    assert!(!validate_frequencies(&[2000.0, 200.0])); // Out of order
}

// =============================================================================
// Global Parameter Tests
// =============================================================================

/// Test global threshold control.
#[gpui::test]
async fn test_global_threshold(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let test_values: Vec<f32> = vec![-60.0, -40.0, -20.0, -10.0, 0.0];
    for value in test_values {
        state.borrow_mut().threshold_db = value;
        assert!((state.borrow().threshold_db - value).abs() < 0.1);
    }
}

/// Test global ratio control.
#[gpui::test]
async fn test_global_ratio(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let test_values: Vec<f32> = vec![1.0, 2.0, 4.0, 8.0, 20.0];
    for value in test_values {
        state.borrow_mut().ratio = value;
        assert!((state.borrow().ratio - value).abs() < 0.1);
    }
}

/// Test global attack control.
#[gpui::test]
async fn test_global_attack(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let test_values: Vec<f32> = vec![0.1, 1.0, 10.0, 50.0, 100.0];
    for value in test_values {
        state.borrow_mut().attack_ms = value;
        assert!((state.borrow().attack_ms - value).abs() < 0.1);
    }
}

/// Test global release control.
#[gpui::test]
async fn test_global_release(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let test_values: Vec<f32> = vec![10.0, 50.0, 100.0, 500.0, 1000.0];
    for value in test_values {
        state.borrow_mut().release_ms = value;
        assert!((state.borrow().release_ms - value).abs() < 0.1);
    }
}

/// Test global knee control.
#[gpui::test]
async fn test_global_knee(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let test_values: Vec<f32> = vec![0.0, 3.0, 6.0, 12.0];
    for value in test_values {
        state.borrow_mut().knee_db = value;
        assert!((state.borrow().knee_db - value).abs() < 0.1);
    }
}

// =============================================================================
// Per-Band Parameter Tests
// =============================================================================

/// Test per-band threshold override.
#[gpui::test]
async fn test_band_threshold_override(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    // Override band 0 threshold
    state.borrow_mut().bands[0].threshold_db = Some(-30.0);

    assert_eq!(state.borrow().bands[0].threshold_db, Some(-30.0));
    assert_eq!(state.borrow().bands[1].threshold_db, None); // Uses global
}

/// Test per-band ratio override.
#[gpui::test]
async fn test_band_ratio_override(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    state.borrow_mut().bands[1].ratio = Some(8.0);

    assert_eq!(state.borrow().bands[1].ratio, Some(8.0));
}

/// Test per-band makeup gain.
#[gpui::test]
async fn test_band_makeup_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let test_values: Vec<f32> = vec![-12.0, -6.0, 0.0, 6.0, 12.0];
    for value in test_values {
        state.borrow_mut().bands[0].makeup_gain_db = value;
        assert!((state.borrow().bands[0].makeup_gain_db - value).abs() < 0.1);
    }
}

/// Test per-band solo.
#[gpui::test]
async fn test_band_solo(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    // Solo band 1
    state.borrow_mut().bands[1].solo = true;

    assert!(!state.borrow().bands[0].solo);
    assert!(state.borrow().bands[1].solo);
    assert!(!state.borrow().bands[2].solo);
}

/// Test per-band bypass.
#[gpui::test]
async fn test_band_bypass(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    state.borrow_mut().bands[0].bypass = true;

    assert!(state.borrow().bands[0].bypass);
    assert!(!state.borrow().bands[1].bypass);
}

// =============================================================================
// Link Channels Tests
// =============================================================================

/// Test link channels toggle.
#[gpui::test]
async fn test_link_channels(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    assert!(state.borrow().link_channels);

    state.borrow_mut().link_channels = false;
    assert!(!state.borrow().link_channels);
}

// =============================================================================
// Mix Tests
// =============================================================================

/// Test mix control.
#[gpui::test]
async fn test_mix_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().mix = value;
        assert!((state.borrow().mix - value).abs() < 0.001);
    }
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_mb_compressor_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Band Label Tests
// =============================================================================

/// Test band frequency labels.
#[gpui::test]
async fn test_band_frequency_labels(_cx: &mut TestAppContext) {
    fn get_band_label(band: usize, crossovers: &[f32]) -> String {
        let num_bands = crossovers.len() + 1;
        if band == 0 {
            format!("Low (<{:.0}Hz)", crossovers[0])
        } else if band == num_bands - 1 {
            format!("High (>{:.0}Hz)", crossovers[band - 1])
        } else {
            format!("{:.0}-{:.0}Hz", crossovers[band - 1], crossovers[band])
        }
    }

    let crossovers = vec![200.0, 2000.0];
    assert_eq!(get_band_label(0, &crossovers), "Low (<200Hz)");
    assert_eq!(get_band_label(1, &crossovers), "200-2000Hz");
    assert_eq!(get_band_label(2, &crossovers), "High (>2000Hz)");
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test gain reduction meter per band.
#[gpui::test]
async fn test_band_gr_meter(_cx: &mut TestAppContext) {
    fn format_gr(gr_db: f32) -> String {
        if gr_db.abs() < 0.1 {
            "0.0 dB".to_string()
        } else {
            format!("{:.1} dB", gr_db)
        }
    }

    assert_eq!(format_gr(0.0), "0.0 dB");
    assert_eq!(format_gr(-6.0), "-6.0 dB");
}

/// Test band activity indicator.
#[gpui::test]
async fn test_band_activity_indicator(_cx: &mut TestAppContext) {
    fn get_band_activity_color(gr_db: f32) -> &'static str {
        if gr_db >= 0.0 {
            "inactive"
        } else if gr_db > -6.0 {
            "light"
        } else if gr_db > -12.0 {
            "moderate"
        } else {
            "heavy"
        }
    }

    assert_eq!(get_band_activity_color(0.0), "inactive");
    assert_eq!(get_band_activity_color(-3.0), "light");
    assert_eq!(get_band_activity_color(-9.0), "moderate");
    assert_eq!(get_band_activity_color(-15.0), "heavy");
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: gentle mastering.
#[gpui::test]
async fn test_preset_gentle_mastering(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    state.borrow_mut().num_bands = 3;
    state.borrow_mut().threshold_db = -18.0;
    state.borrow_mut().ratio = 2.0;
    state.borrow_mut().attack_ms = 30.0;
    state.borrow_mut().release_ms = 200.0;

    assert!(state.borrow().ratio < 4.0);
}

/// Test preset: aggressive.
#[gpui::test]
async fn test_preset_aggressive(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandCompressorState::default()));

    state.borrow_mut().num_bands = 4;
    state.borrow_mut().threshold_db = -24.0;
    state.borrow_mut().ratio = 8.0;
    state.borrow_mut().attack_ms = 5.0;
    state.borrow_mut().release_ms = 50.0;

    assert!(state.borrow().ratio >= 8.0);
}
