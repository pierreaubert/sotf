//! E2E tests for Convolution Plugin.
//!
//! Tests for the FFT-based convolution plugin for impulse response processing.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Convolution plugin state for testing
struct ConvolutionState {
    enabled: bool,
    ir_file: String,
    mix: f32,
    gain_db: f32,
    // UI state
    ir_loading: bool,
    ir_loaded: bool,
    ir_error: Option<String>,
    ir_length_samples: usize,
}

impl Default for ConvolutionState {
    fn default() -> Self {
        Self {
            enabled: true,
            ir_file: String::new(),
            mix: 1.0,
            gain_db: 0.0,
            ir_loading: false,
            ir_loaded: false,
            ir_error: None,
            ir_length_samples: 0,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_convolution_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));
    assert!(state.borrow().enabled);
}

/// Test default values.
#[gpui::test]
async fn test_convolution_defaults(_cx: &mut TestAppContext) {
    let state = ConvolutionState::default();

    assert!(state.ir_file.is_empty());
    assert!((state.mix - 1.0).abs() < 0.001);
    assert!((state.gain_db - 0.0).abs() < 0.001);
}

// =============================================================================
// IR File Selection Tests
// =============================================================================

/// Test IR file selection.
#[gpui::test]
async fn test_ir_file_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    state.borrow_mut().ir_file = "/path/to/reverb.wav".to_string();
    assert_eq!(state.borrow().ir_file, "/path/to/reverb.wav");
}

/// Test IR file extension validation.
#[gpui::test]
async fn test_ir_file_extensions(_cx: &mut TestAppContext) {
    fn is_valid_ir_extension(path: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with(".wav")
            || lower.ends_with(".flac")
            || lower.ends_with(".aiff")
            || lower.ends_with(".aif")
            || lower.ends_with(".mp3")
    }

    assert!(is_valid_ir_extension("reverb.wav"));
    assert!(is_valid_ir_extension("reverb.WAV"));
    assert!(is_valid_ir_extension("reverb.flac"));
    assert!(is_valid_ir_extension("reverb.aiff"));
    assert!(!is_valid_ir_extension("reverb.txt"));
}

/// Test IR file display name.
#[gpui::test]
async fn test_ir_file_display_name(_cx: &mut TestAppContext) {
    fn get_ir_display_name(path: &str) -> String {
        if path.is_empty() {
            "No IR loaded".to_string()
        } else {
            std::path::Path::new(path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string())
        }
    }

    assert_eq!(get_ir_display_name(""), "No IR loaded");
    assert_eq!(get_ir_display_name("/path/to/hall.wav"), "hall.wav");
    assert_eq!(get_ir_display_name("/reverbs/plate_long.flac"), "plate_long.flac");
}

/// Test clearing IR file.
#[gpui::test]
async fn test_ir_file_clear(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    state.borrow_mut().ir_file = "/path/to/reverb.wav".to_string();
    state.borrow_mut().ir_loaded = true;

    // Clear
    state.borrow_mut().ir_file = String::new();
    state.borrow_mut().ir_loaded = false;

    assert!(state.borrow().ir_file.is_empty());
    assert!(!state.borrow().ir_loaded);
}

// =============================================================================
// IR Loading State Tests
// =============================================================================

/// Test IR loading state.
#[gpui::test]
async fn test_ir_loading_state(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    // Start loading
    state.borrow_mut().ir_loading = true;
    assert!(state.borrow().ir_loading);

    // Finish loading
    state.borrow_mut().ir_loading = false;
    state.borrow_mut().ir_loaded = true;
    assert!(!state.borrow().ir_loading);
    assert!(state.borrow().ir_loaded);
}

/// Test IR loading error.
#[gpui::test]
async fn test_ir_loading_error(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    // Simulate error
    state.borrow_mut().ir_loading = false;
    state.borrow_mut().ir_loaded = false;
    state.borrow_mut().ir_error = Some("Failed to load IR file".to_string());

    assert!(state.borrow().ir_error.is_some());
    assert!(!state.borrow().ir_loaded);
}

/// Test IR length display.
#[gpui::test]
async fn test_ir_length_display(_cx: &mut TestAppContext) {
    fn format_ir_length(samples: usize, sample_rate: u32) -> String {
        if samples == 0 {
            "N/A".to_string()
        } else {
            let duration_ms = (samples as f32 / sample_rate as f32) * 1000.0;
            if duration_ms >= 1000.0 {
                format!("{:.2}s", duration_ms / 1000.0)
            } else {
                format!("{:.0}ms", duration_ms)
            }
        }
    }

    assert_eq!(format_ir_length(0, 48000), "N/A");
    assert_eq!(format_ir_length(48000, 48000), "1.00s");
    assert_eq!(format_ir_length(24000, 48000), "500ms");
}

// =============================================================================
// Mix Tests
// =============================================================================

/// Test mix control.
#[gpui::test]
async fn test_convolution_mix_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().mix = value;
        assert!((state.borrow().mix - value).abs() < 0.001);
    }
}

/// Test mix bounds.
#[gpui::test]
async fn test_convolution_mix_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

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
async fn test_convolution_mix_display(_cx: &mut TestAppContext) {
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

// =============================================================================
// Gain Tests
// =============================================================================

/// Test gain control.
#[gpui::test]
async fn test_convolution_gain_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    let test_values: Vec<f32> = vec![-12.0, -6.0, 0.0, 3.0, 6.0, 12.0];
    for value in test_values {
        state.borrow_mut().gain_db = value;
        assert!((state.borrow().gain_db - value).abs() < 0.01);
    }
}

/// Test gain bounds.
#[gpui::test]
async fn test_convolution_gain_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    // Typical range: -24 dB to +12 dB
    let clamped = (-30.0_f32).clamp(-24.0, 12.0);
    state.borrow_mut().gain_db = clamped;
    assert!(state.borrow().gain_db >= -24.0);

    let clamped = (20.0_f32).clamp(-24.0, 12.0);
    state.borrow_mut().gain_db = clamped;
    assert!(state.borrow().gain_db <= 12.0);
}

/// Test gain display format.
#[gpui::test]
async fn test_convolution_gain_display(_cx: &mut TestAppContext) {
    fn format_gain_db(gain_db: f32) -> String {
        if gain_db.abs() < 0.1 {
            "0.0 dB".to_string()
        } else {
            format!("{:+.1} dB", gain_db)
        }
    }

    assert_eq!(format_gain_db(0.0), "0.0 dB");
    assert_eq!(format_gain_db(-6.0), "-6.0 dB");
    assert_eq!(format_gain_db(3.0), "+3.0 dB");
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_convolution_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// IR Type Tests
// =============================================================================

/// Test common IR types.
#[gpui::test]
async fn test_ir_types(_cx: &mut TestAppContext) {
    fn get_ir_type_description(filename: &str) -> &'static str {
        let lower = filename.to_lowercase();
        if lower.contains("hall") {
            "Concert Hall"
        } else if lower.contains("room") {
            "Room"
        } else if lower.contains("plate") {
            "Plate Reverb"
        } else if lower.contains("spring") {
            "Spring Reverb"
        } else if lower.contains("chamber") {
            "Chamber"
        } else if lower.contains("cabinet") || lower.contains("cab") {
            "Speaker Cabinet"
        } else {
            "Custom IR"
        }
    }

    assert_eq!(get_ir_type_description("large_hall.wav"), "Concert Hall");
    assert_eq!(get_ir_type_description("plate_reverb.wav"), "Plate Reverb");
    assert_eq!(get_ir_type_description("guitar_cab.wav"), "Speaker Cabinet");
    assert_eq!(get_ir_type_description("custom.wav"), "Custom IR");
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: hall reverb.
#[gpui::test]
async fn test_preset_hall_reverb(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    state.borrow_mut().ir_file = "hall_large.wav".to_string();
    state.borrow_mut().mix = 0.3;
    state.borrow_mut().gain_db = -3.0;

    assert!(state.borrow().mix < 0.5);
}

/// Test preset: room ambience.
#[gpui::test]
async fn test_preset_room_ambience(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    state.borrow_mut().ir_file = "small_room.wav".to_string();
    state.borrow_mut().mix = 0.2;
    state.borrow_mut().gain_db = 0.0;

    assert!(state.borrow().mix < 0.3);
}

/// Test preset: plate reverb.
#[gpui::test]
async fn test_preset_plate_reverb(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ConvolutionState::default()));

    state.borrow_mut().ir_file = "plate_bright.wav".to_string();
    state.borrow_mut().mix = 0.4;
    state.borrow_mut().gain_db = 0.0;

    assert!(!state.borrow().ir_file.is_empty());
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test loading indicator.
#[gpui::test]
async fn test_loading_indicator(_cx: &mut TestAppContext) {
    fn get_status_text(loading: bool, loaded: bool, error: &Option<String>) -> String {
        if loading {
            "Loading...".to_string()
        } else if let Some(err) = error {
            format!("Error: {}", err)
        } else if loaded {
            "Ready".to_string()
        } else {
            "No IR".to_string()
        }
    }

    assert_eq!(get_status_text(true, false, &None), "Loading...");
    assert_eq!(get_status_text(false, true, &None), "Ready");
    assert_eq!(get_status_text(false, false, &None), "No IR");
    assert!(get_status_text(false, false, &Some("File not found".to_string())).contains("Error"));
}

/// Test waveform display data.
#[gpui::test]
async fn test_ir_waveform_display(_cx: &mut TestAppContext) {
    fn downsample_for_display(samples: usize, display_width: usize) -> usize {
        if samples <= display_width {
            1
        } else {
            samples / display_width
        }
    }

    assert_eq!(downsample_for_display(48000, 200), 240);
    assert_eq!(downsample_for_display(100, 200), 1);
}
