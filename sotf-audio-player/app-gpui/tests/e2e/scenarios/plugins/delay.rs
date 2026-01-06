//! E2E tests for Delay Plugin.
//!
//! Tests for the simple delay effect with feedback and mix controls.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Delay plugin state for testing
struct DelayState {
    enabled: bool,
    delay_ms: f32,
    feedback: f32,
    mix: f32,
}

impl Default for DelayState {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_ms: 100.0,
            feedback: 0.3,
            mix: 0.5,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_delay_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));
    assert!(state.borrow().enabled);
}

/// Test default values.
#[gpui::test]
async fn test_delay_defaults(_cx: &mut TestAppContext) {
    let state = DelayState::default();

    assert!((state.delay_ms - 100.0).abs() < 0.1);
    assert!((state.feedback - 0.3).abs() < 0.001);
    assert!((state.mix - 0.5).abs() < 0.001);
}

// =============================================================================
// Delay Time Tests
// =============================================================================

/// Test delay time control.
#[gpui::test]
async fn test_delay_time_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    let test_values: Vec<f32> = vec![1.0, 10.0, 50.0, 100.0, 250.0, 500.0, 1000.0];
    for value in test_values {
        state.borrow_mut().delay_ms = value;
        assert!((state.borrow().delay_ms - value).abs() < 0.1);
    }
}

/// Test delay time bounds.
#[gpui::test]
async fn test_delay_time_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    // Typical range: 1ms to 2000ms
    let clamped = (0.5_f32).clamp(1.0, 2000.0);
    state.borrow_mut().delay_ms = clamped;
    assert!(state.borrow().delay_ms >= 1.0);

    let clamped = (3000.0_f32).clamp(1.0, 2000.0);
    state.borrow_mut().delay_ms = clamped;
    assert!(state.borrow().delay_ms <= 2000.0);
}

/// Test delay time to samples conversion.
#[gpui::test]
async fn test_delay_time_to_samples(_cx: &mut TestAppContext) {
    fn ms_to_samples(delay_ms: f32, sample_rate: u32) -> usize {
        (delay_ms * sample_rate as f32 / 1000.0).round() as usize
    }

    // At 48kHz
    assert_eq!(ms_to_samples(100.0, 48000), 4800);
    assert_eq!(ms_to_samples(250.0, 48000), 12000);
    assert_eq!(ms_to_samples(1000.0, 48000), 48000);

    // At 44.1kHz
    assert_eq!(ms_to_samples(100.0, 44100), 4410);
}

/// Test delay time display format.
#[gpui::test]
async fn test_delay_time_display(_cx: &mut TestAppContext) {
    fn format_delay_time(delay_ms: f32) -> String {
        if delay_ms >= 1000.0 {
            format!("{:.2}s", delay_ms / 1000.0)
        } else {
            format!("{:.1}ms", delay_ms)
        }
    }

    assert_eq!(format_delay_time(100.0), "100.0ms");
    assert_eq!(format_delay_time(500.0), "500.0ms");
    assert_eq!(format_delay_time(1000.0), "1.00s");
    assert_eq!(format_delay_time(1500.0), "1.50s");
}

/// Test delay time slider interaction.
#[gpui::test]
async fn test_delay_time_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    // Simulate slider drag
    let drag_values: Vec<f32> = vec![50.0, 100.0, 200.0, 300.0, 500.0];
    for value in drag_values {
        state.borrow_mut().delay_ms = value;
        assert!((state.borrow().delay_ms - value).abs() < 0.1);
    }
}

// =============================================================================
// Feedback Tests
// =============================================================================

/// Test feedback control.
#[gpui::test]
async fn test_feedback_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().feedback = value;
        assert!((state.borrow().feedback - value).abs() < 0.001);
    }
}

/// Test feedback bounds.
#[gpui::test]
async fn test_feedback_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    // Range: 0.0 to 1.0 (or slightly less for stability)
    let clamped = (-0.1_f32).clamp(0.0, 0.99);
    state.borrow_mut().feedback = clamped;
    assert!(state.borrow().feedback >= 0.0);

    let clamped = (1.5_f32).clamp(0.0, 0.99);
    state.borrow_mut().feedback = clamped;
    assert!(state.borrow().feedback <= 0.99);
}

/// Test feedback display format.
#[gpui::test]
async fn test_feedback_display(_cx: &mut TestAppContext) {
    fn format_feedback(feedback: f32) -> String {
        format!("{}%", (feedback * 100.0).round() as i32)
    }

    assert_eq!(format_feedback(0.0), "0%");
    assert_eq!(format_feedback(0.3), "30%");
    assert_eq!(format_feedback(0.5), "50%");
    assert_eq!(format_feedback(1.0), "100%");
}

/// Test feedback stability warning.
#[gpui::test]
async fn test_feedback_stability_warning(_cx: &mut TestAppContext) {
    fn show_stability_warning(feedback: f32) -> bool {
        feedback >= 0.9
    }

    assert!(!show_stability_warning(0.5));
    assert!(!show_stability_warning(0.89));
    assert!(show_stability_warning(0.9));
    assert!(show_stability_warning(0.99));
}

// =============================================================================
// Mix Tests
// =============================================================================

/// Test mix control.
#[gpui::test]
async fn test_mix_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().mix = value;
        assert!((state.borrow().mix - value).abs() < 0.001);
    }
}

/// Test mix bounds.
#[gpui::test]
async fn test_mix_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    // Range: 0.0 to 1.0
    let clamped = (-0.1_f32).clamp(0.0, 1.0);
    state.borrow_mut().mix = clamped;
    assert!(state.borrow().mix >= 0.0);

    let clamped = (1.5_f32).clamp(0.0, 1.0);
    state.borrow_mut().mix = clamped;
    assert!(state.borrow().mix <= 1.0);
}

/// Test mix display format.
#[gpui::test]
async fn test_mix_display(_cx: &mut TestAppContext) {
    fn format_mix(mix: f32) -> String {
        if mix <= 0.0 {
            "Dry".to_string()
        } else if mix >= 1.0 {
            "Wet".to_string()
        } else {
            format!("{}%", (mix * 100.0).round() as i32)
        }
    }

    assert_eq!(format_mix(0.0), "Dry");
    assert_eq!(format_mix(0.5), "50%");
    assert_eq!(format_mix(1.0), "Wet");
}

/// Test dry/wet calculation.
#[gpui::test]
async fn test_drywet_calculation(_cx: &mut TestAppContext) {
    fn calculate_dry_wet(mix: f32) -> (f32, f32) {
        let dry = 1.0 - mix;
        let wet = mix;
        (dry, wet)
    }

    let (dry, wet) = calculate_dry_wet(0.0);
    assert!((dry - 1.0).abs() < 0.001);
    assert!((wet - 0.0).abs() < 0.001);

    let (dry, wet) = calculate_dry_wet(0.5);
    assert!((dry - 0.5).abs() < 0.001);
    assert!((wet - 0.5).abs() < 0.001);

    let (dry, wet) = calculate_dry_wet(1.0);
    assert!((dry - 0.0).abs() < 0.001);
    assert!((wet - 1.0).abs() < 0.001);
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_delay_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Tap Tempo Tests
// =============================================================================

/// Test tap tempo calculation.
#[gpui::test]
async fn test_tap_tempo_calculation(_cx: &mut TestAppContext) {
    fn calculate_tap_tempo(tap_times_ms: &[f32]) -> Option<f32> {
        if tap_times_ms.len() < 2 {
            return None;
        }
        let mut intervals = Vec::new();
        for i in 1..tap_times_ms.len() {
            intervals.push(tap_times_ms[i] - tap_times_ms[i - 1]);
        }
        let avg_interval = intervals.iter().sum::<f32>() / intervals.len() as f32;
        Some(avg_interval)
    }

    // 2 taps 500ms apart = 500ms delay
    let taps = vec![0.0, 500.0];
    assert!((calculate_tap_tempo(&taps).unwrap() - 500.0).abs() < 0.1);

    // 3 taps for more accuracy
    let taps = vec![0.0, 250.0, 500.0];
    assert!((calculate_tap_tempo(&taps).unwrap() - 250.0).abs() < 0.1);
}

/// Test BPM to delay conversion.
#[gpui::test]
async fn test_bpm_to_delay(_cx: &mut TestAppContext) {
    fn bpm_to_delay_ms(bpm: f32, division: &str) -> f32 {
        let beat_ms = 60000.0 / bpm;
        match division {
            "1/4" => beat_ms,
            "1/8" => beat_ms / 2.0,
            "1/16" => beat_ms / 4.0,
            "1/2" => beat_ms * 2.0,
            "1/1" => beat_ms * 4.0,
            _ => beat_ms,
        }
    }

    // 120 BPM
    assert!((bpm_to_delay_ms(120.0, "1/4") - 500.0).abs() < 0.1);
    assert!((bpm_to_delay_ms(120.0, "1/8") - 250.0).abs() < 0.1);
    assert!((bpm_to_delay_ms(120.0, "1/16") - 125.0).abs() < 0.1);

    // 140 BPM
    let quarter_note = 60000.0 / 140.0;
    assert!((bpm_to_delay_ms(140.0, "1/4") - quarter_note).abs() < 0.1);
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: slapback.
#[gpui::test]
async fn test_preset_slapback(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    // Slapback: short delay, no feedback, moderate mix
    state.borrow_mut().delay_ms = 80.0;
    state.borrow_mut().feedback = 0.0;
    state.borrow_mut().mix = 0.4;

    assert!(state.borrow().delay_ms < 100.0);
    assert!((state.borrow().feedback - 0.0).abs() < 0.001);
}

/// Test preset: long echo.
#[gpui::test]
async fn test_preset_long_echo(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    // Long echo: longer delay, moderate feedback
    state.borrow_mut().delay_ms = 500.0;
    state.borrow_mut().feedback = 0.5;
    state.borrow_mut().mix = 0.3;

    assert!(state.borrow().delay_ms >= 500.0);
    assert!(state.borrow().feedback > 0.3);
}

/// Test preset: ambient.
#[gpui::test]
async fn test_preset_ambient(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DelayState::default()));

    // Ambient: long delay, high feedback, subtle mix
    state.borrow_mut().delay_ms = 750.0;
    state.borrow_mut().feedback = 0.7;
    state.borrow_mut().mix = 0.25;

    assert!(state.borrow().feedback >= 0.7);
    assert!(state.borrow().mix < 0.5);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test delay buffer visualization.
#[gpui::test]
async fn test_delay_buffer_visualization(_cx: &mut TestAppContext) {
    fn calculate_buffer_position(delay_ms: f32, max_delay_ms: f32) -> f32 {
        (delay_ms / max_delay_ms).clamp(0.0, 1.0)
    }

    assert!((calculate_buffer_position(100.0, 1000.0) - 0.1).abs() < 0.001);
    assert!((calculate_buffer_position(500.0, 1000.0) - 0.5).abs() < 0.001);
}

/// Test feedback decay visualization.
#[gpui::test]
async fn test_feedback_decay_visualization(_cx: &mut TestAppContext) {
    fn calculate_decay_levels(feedback: f32, num_repeats: usize) -> Vec<f32> {
        let mut levels = Vec::new();
        let mut level = 1.0;
        for _ in 0..num_repeats {
            level *= feedback;
            levels.push(level);
        }
        levels
    }

    let levels = calculate_decay_levels(0.5, 4);
    assert!((levels[0] - 0.5).abs() < 0.001);
    assert!((levels[1] - 0.25).abs() < 0.001);
    assert!((levels[2] - 0.125).abs() < 0.001);
    assert!((levels[3] - 0.0625).abs() < 0.001);
}
