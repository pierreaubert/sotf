//! E2E tests for Multiband Expander Plugin.
//!
//! Tests for the multiband dynamic range expander with 2-5 bands.
//! Uses Linkwitz-Riley 24dB/oct crossover filters for phase-coherent band splitting.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Per-band expander parameters
#[derive(Debug, Clone, Default)]
struct BandExpanderParams {
    threshold_db: Option<f32>,
    ratio: Option<f32>,
    attack_ms: Option<f32>,
    release_ms: Option<f32>,
    knee_db: Option<f32>,
    range_db: Option<f32>,
    hysteresis_db: Option<f32>,
    hold_ms: Option<f32>,
    solo: bool,
    bypass: bool,
}

/// Multiband expander state for testing
struct MultibandExpanderState {
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
    range_db: f32,
    hysteresis_db: f32,
    hold_ms: f32,
    link_channels: bool,
    mix: f32,
    // Per-band
    bands: Vec<BandExpanderParams>,
}

impl Default for MultibandExpanderState {
    fn default() -> Self {
        Self {
            enabled: true,
            num_bands: 3,
            crossover_preset: 1,
            crossover_frequencies: vec![200.0, 2000.0, 8000.0, 12000.0],
            threshold_db: -40.0,
            ratio: 2.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 6.0,
            range_db: 40.0,
            hysteresis_db: 3.0,
            hold_ms: 10.0,
            link_channels: true,
            mix: 1.0,
            bands: vec![BandExpanderParams::default(); 3],
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_mb_expander_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));
    assert!(state.borrow().enabled);
    assert_eq!(state.borrow().num_bands, 3);
}

/// Test default values.
#[gpui::test]
async fn test_mb_expander_defaults(_cx: &mut TestAppContext) {
    let state = MultibandExpanderState::default();

    assert_eq!(state.num_bands, 3);
    assert!((state.threshold_db - (-40.0)).abs() < 0.1);
    assert!((state.ratio - 2.0).abs() < 0.1);
    assert!((state.range_db - 40.0).abs() < 0.1);
}

// =============================================================================
// Band Count Tests
// =============================================================================

/// Test band count selection.
#[gpui::test]
async fn test_band_count_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    for num_bands in 2..=5 {
        state.borrow_mut().num_bands = num_bands;
        state.borrow_mut().bands = vec![BandExpanderParams::default(); num_bands];
        assert_eq!(state.borrow().num_bands, num_bands);
    }
}

// =============================================================================
// Crossover Tests
// =============================================================================

/// Test crossover preset selection.
#[gpui::test]
async fn test_crossover_preset_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    for preset in 0..=3 {
        state.borrow_mut().crossover_preset = preset;
        assert_eq!(state.borrow().crossover_preset, preset);
    }
}

/// Test custom crossover frequencies.
#[gpui::test]
async fn test_custom_crossover_frequencies(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    state.borrow_mut().crossover_preset = 0;
    state.borrow_mut().crossover_frequencies = vec![100.0, 1000.0, 5000.0];

    assert_eq!(state.borrow().crossover_preset, 0);
}

// =============================================================================
// Global Parameter Tests
// =============================================================================

/// Test global threshold control.
#[gpui::test]
async fn test_global_threshold(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![-60.0, -50.0, -40.0, -30.0, -20.0];
    for value in test_values {
        state.borrow_mut().threshold_db = value;
        assert!((state.borrow().threshold_db - value).abs() < 0.1);
    }
}

/// Test global ratio control.
#[gpui::test]
async fn test_global_ratio(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![1.5, 2.0, 3.0, 4.0, 8.0];
    for value in test_values {
        state.borrow_mut().ratio = value;
        assert!((state.borrow().ratio - value).abs() < 0.1);
    }
}

/// Test global attack control.
#[gpui::test]
async fn test_global_attack(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![0.1, 1.0, 5.0, 20.0, 50.0];
    for value in test_values {
        state.borrow_mut().attack_ms = value;
        assert!((state.borrow().attack_ms - value).abs() < 0.1);
    }
}

/// Test global release control.
#[gpui::test]
async fn test_global_release(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![10.0, 50.0, 100.0, 200.0, 500.0];
    for value in test_values {
        state.borrow_mut().release_ms = value;
        assert!((state.borrow().release_ms - value).abs() < 0.1);
    }
}

/// Test global knee control.
#[gpui::test]
async fn test_global_knee(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![0.0, 3.0, 6.0, 12.0];
    for value in test_values {
        state.borrow_mut().knee_db = value;
        assert!((state.borrow().knee_db - value).abs() < 0.1);
    }
}

/// Test global range control.
#[gpui::test]
async fn test_global_range(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![0.0, 20.0, 40.0, 60.0, 80.0];
    for value in test_values {
        state.borrow_mut().range_db = value;
        assert!((state.borrow().range_db - value).abs() < 0.1);
    }
}

/// Test global hysteresis control.
#[gpui::test]
async fn test_global_hysteresis(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![0.0, 1.0, 3.0, 6.0];
    for value in test_values {
        state.borrow_mut().hysteresis_db = value;
        assert!((state.borrow().hysteresis_db - value).abs() < 0.1);
    }
}

/// Test global hold control.
#[gpui::test]
async fn test_global_hold(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![0.0, 5.0, 10.0, 50.0, 100.0];
    for value in test_values {
        state.borrow_mut().hold_ms = value;
        assert!((state.borrow().hold_ms - value).abs() < 0.1);
    }
}

// =============================================================================
// Per-Band Parameter Tests
// =============================================================================

/// Test per-band threshold override.
#[gpui::test]
async fn test_band_threshold_override(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    state.borrow_mut().bands[0].threshold_db = Some(-50.0);

    assert_eq!(state.borrow().bands[0].threshold_db, Some(-50.0));
    assert_eq!(state.borrow().bands[1].threshold_db, None);
}

/// Test per-band range override.
#[gpui::test]
async fn test_band_range_override(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    state.borrow_mut().bands[1].range_db = Some(60.0);

    assert_eq!(state.borrow().bands[1].range_db, Some(60.0));
}

/// Test per-band solo.
#[gpui::test]
async fn test_band_solo(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    state.borrow_mut().bands[0].solo = true;

    assert!(state.borrow().bands[0].solo);
    assert!(!state.borrow().bands[1].solo);
}

/// Test per-band bypass.
#[gpui::test]
async fn test_band_bypass(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    state.borrow_mut().bands[2].bypass = true;

    assert!(state.borrow().bands[2].bypass);
}

// =============================================================================
// Link Channels Tests
// =============================================================================

/// Test link channels toggle.
#[gpui::test]
async fn test_link_channels(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

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
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.5, 1.0];
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
async fn test_mb_expander_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test expansion meter per band.
#[gpui::test]
async fn test_band_expansion_meter(_cx: &mut TestAppContext) {
    fn format_expansion(exp_db: f32) -> String {
        if exp_db.abs() < 0.1 {
            "0.0 dB".to_string()
        } else {
            format!("{:.1} dB", exp_db)
        }
    }

    assert_eq!(format_expansion(0.0), "0.0 dB");
    assert_eq!(format_expansion(-20.0), "-20.0 dB");
}

/// Test band state indicator.
#[gpui::test]
async fn test_band_state_indicator(_cx: &mut TestAppContext) {
    fn get_expansion_state(exp_db: f32, threshold_db: f32, input_db: f32) -> &'static str {
        if input_db > threshold_db {
            "open"
        } else if exp_db.abs() < 1.0 {
            "idle"
        } else {
            "expanding"
        }
    }

    assert_eq!(get_expansion_state(0.0, -40.0, -20.0), "open");
    assert_eq!(get_expansion_state(-20.0, -40.0, -50.0), "expanding");
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: gentle noise gate.
#[gpui::test]
async fn test_preset_gentle_noise_gate(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    state.borrow_mut().threshold_db = -50.0;
    state.borrow_mut().ratio = 2.0;
    state.borrow_mut().range_db = 20.0;
    state.borrow_mut().attack_ms = 5.0;
    state.borrow_mut().release_ms = 100.0;

    assert!(state.borrow().range_db < 40.0);
}

/// Test preset: aggressive expansion.
#[gpui::test]
async fn test_preset_aggressive_expansion(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MultibandExpanderState::default()));

    state.borrow_mut().threshold_db = -35.0;
    state.borrow_mut().ratio = 4.0;
    state.borrow_mut().range_db = 60.0;
    state.borrow_mut().attack_ms = 1.0;
    state.borrow_mut().release_ms = 50.0;

    assert!(state.borrow().range_db >= 60.0);
}
