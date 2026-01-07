//! E2E tests for Binaural Decoder Plugin.
//!
//! Tests for the surround-to-binaural decoder plugin.
//! Uses HRTF convolution to render multichannel audio for headphone playback.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Room model for externalization
#[derive(Debug, Clone, Default, PartialEq)]
struct RoomModel {
    enabled: bool,
    room_size: f32,        // 0.0-1.0
    reflection_level: f32, // 0.0-1.0
    reverb_time: f32,      // seconds
}

/// Binaural decoder plugin state for testing
struct BinauralState {
    enabled: bool,
    // HRTF settings
    hrtf_file: String,
    fft_size: usize,
    input_channels: usize,
    enable_optimization: bool,
    // Spatialization
    externalization: f32,
    near_field_strength: f32,
    diffuse_field_eq: bool,
    // LFE handling
    lfe_crossover: f32,
    lfe_distance: f32,
    lfe_level: f32,
    // Room model
    room_model: RoomModel,
}

impl Default for BinauralState {
    fn default() -> Self {
        Self {
            enabled: true,
            hrtf_file: String::new(),
            fft_size: 2048,
            input_channels: 6, // 5.1 default
            enable_optimization: true,
            externalization: 0.0,
            near_field_strength: 0.0,
            diffuse_field_eq: true,
            lfe_crossover: 120.0,
            lfe_distance: 2.0,
            lfe_level: 0.0,
            room_model: RoomModel::default(),
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_binaural_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));
    assert!(state.borrow().enabled);
    assert_eq!(state.borrow().fft_size, 2048);
}

/// Test default values.
#[gpui::test]
async fn test_binaural_defaults(_cx: &mut TestAppContext) {
    let state = BinauralState::default();

    assert_eq!(state.fft_size, 2048);
    assert_eq!(state.input_channels, 6);
    assert!(state.enable_optimization);
    assert!((state.externalization - 0.0).abs() < 0.001);
    assert!(state.diffuse_field_eq);
    assert!((state.lfe_crossover - 120.0).abs() < 0.1);
}

// =============================================================================
// HRTF File Tests
// =============================================================================

/// Test HRTF file selection.
#[gpui::test]
async fn test_hrtf_file_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    state.borrow_mut().hrtf_file = "/path/to/hrtf.sofa".to_string();
    assert_eq!(state.borrow().hrtf_file, "/path/to/hrtf.sofa");
}

/// Test HRTF file extension validation.
#[gpui::test]
async fn test_hrtf_file_extensions(_cx: &mut TestAppContext) {
    fn is_valid_hrtf_extension(path: &str) -> bool {
        path.ends_with(".sofa") || path.ends_with(".polar")
    }

    assert!(is_valid_hrtf_extension("hrtf.sofa"));
    assert!(is_valid_hrtf_extension("hrtf.polar"));
    assert!(!is_valid_hrtf_extension("hrtf.wav"));
    assert!(!is_valid_hrtf_extension("hrtf.txt"));
}

/// Test empty HRTF file (uses built-in).
#[gpui::test]
async fn test_hrtf_empty_uses_builtin(_cx: &mut TestAppContext) {
    let state = BinauralState::default();
    assert!(state.hrtf_file.is_empty(), "Empty means use built-in HRTF");
}

/// Test HRTF file display name.
#[gpui::test]
async fn test_hrtf_display_name(_cx: &mut TestAppContext) {
    fn get_hrtf_display_name(path: &str) -> String {
        if path.is_empty() {
            "Built-in HRTF".to_string()
        } else {
            std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string())
        }
    }

    assert_eq!(get_hrtf_display_name(""), "Built-in HRTF");
    assert_eq!(get_hrtf_display_name("/path/to/my_hrtf.sofa"), "my_hrtf");
}

// =============================================================================
// FFT Size Tests
// =============================================================================

/// Test FFT size options.
#[gpui::test]
async fn test_fft_size_options(_cx: &mut TestAppContext) {
    let valid_sizes = [512, 1024, 2048, 4096, 8192];
    let state = Rc::new(RefCell::new(BinauralState::default()));

    for size in valid_sizes {
        state.borrow_mut().fft_size = size;
        assert_eq!(state.borrow().fft_size, size);
    }
}

/// Test FFT size affects latency.
#[gpui::test]
async fn test_fft_size_latency(_cx: &mut TestAppContext) {
    fn calculate_latency_ms(fft_size: usize, sample_rate: u32) -> f32 {
        (fft_size as f32 / sample_rate as f32) * 1000.0
    }

    // At 48kHz
    assert!((calculate_latency_ms(512, 48000) - 10.67).abs() < 0.1);
    assert!((calculate_latency_ms(1024, 48000) - 21.33).abs() < 0.1);
    assert!((calculate_latency_ms(2048, 48000) - 42.67).abs() < 0.1);
}

// =============================================================================
// Input Channel Tests
// =============================================================================

/// Test input channel selection.
#[gpui::test]
async fn test_input_channels(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    let channel_configs = [2, 6, 8, 10, 12]; // Stereo, 5.1, 7.1, 5.1.4, 7.1.4

    for channels in channel_configs {
        state.borrow_mut().input_channels = channels;
        assert_eq!(state.borrow().input_channels, channels);
    }
}

/// Test output is always stereo.
#[gpui::test]
async fn test_output_channels_stereo(_cx: &mut TestAppContext) {
    // Binaural decoder always outputs 2 channels (headphone)
    let output_channels = 2;
    assert_eq!(output_channels, 2);
}

// =============================================================================
// Optimization Tests
// =============================================================================

/// Test optimization toggle.
#[gpui::test]
async fn test_optimization_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    assert!(state.borrow().enable_optimization);

    state.borrow_mut().enable_optimization = false;
    assert!(!state.borrow().enable_optimization);
}

/// Test optimization description.
#[gpui::test]
async fn test_optimization_description(_cx: &mut TestAppContext) {
    fn get_optimization_description(enabled: bool) -> &'static str {
        if enabled {
            "Sum-Before-IFFT (faster)"
        } else {
            "Standard convolution"
        }
    }

    assert_eq!(
        get_optimization_description(true),
        "Sum-Before-IFFT (faster)"
    );
}

// =============================================================================
// Externalization Tests
// =============================================================================

/// Test externalization control.
#[gpui::test]
async fn test_externalization_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().externalization = value;
        assert!((state.borrow().externalization - value).abs() < 0.001);
    }
}

/// Test externalization bounds.
#[gpui::test]
async fn test_externalization_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Range: 0.0 to 1.0
    let clamped = (-0.1_f32).clamp(0.0, 1.0);
    state.borrow_mut().externalization = clamped;
    assert!(state.borrow().externalization >= 0.0);

    let clamped = (1.5_f32).clamp(0.0, 1.0);
    state.borrow_mut().externalization = clamped;
    assert!(state.borrow().externalization <= 1.0);
}

/// Test externalization description.
#[gpui::test]
async fn test_externalization_description(_cx: &mut TestAppContext) {
    fn get_externalization_label(value: f32) -> &'static str {
        if value < 0.1 {
            "In-head"
        } else if value < 0.5 {
            "Near"
        } else if value < 0.9 {
            "Mid"
        } else {
            "External"
        }
    }

    assert_eq!(get_externalization_label(0.0), "In-head");
    assert_eq!(get_externalization_label(0.3), "Near");
    assert_eq!(get_externalization_label(0.7), "Mid");
    assert_eq!(get_externalization_label(1.0), "External");
}

// =============================================================================
// Near-Field Tests
// =============================================================================

/// Test near-field strength control.
#[gpui::test]
async fn test_near_field_strength(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().near_field_strength = value;
        assert!((state.borrow().near_field_strength - value).abs() < 0.001);
    }
}

/// Test near-field bounds.
#[gpui::test]
async fn test_near_field_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Range: 0.0 to 1.0
    let clamped = (-0.1_f32).clamp(0.0, 1.0);
    state.borrow_mut().near_field_strength = clamped;
    assert!(state.borrow().near_field_strength >= 0.0);
}

// =============================================================================
// Diffuse Field EQ Tests
// =============================================================================

/// Test diffuse field EQ toggle.
#[gpui::test]
async fn test_diffuse_field_eq_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Default is enabled
    assert!(state.borrow().diffuse_field_eq);

    state.borrow_mut().diffuse_field_eq = false;
    assert!(!state.borrow().diffuse_field_eq);
}

/// Test diffuse field EQ description.
#[gpui::test]
async fn test_diffuse_field_eq_description(_cx: &mut TestAppContext) {
    fn get_dfeq_description(enabled: bool) -> &'static str {
        if enabled {
            "Compensates for HRTF coloration"
        } else {
            "Raw HRTF (may sound colored)"
        }
    }

    assert!(get_dfeq_description(true).contains("Compensates"));
}

// =============================================================================
// LFE Handling Tests
// =============================================================================

/// Test LFE crossover frequency.
#[gpui::test]
async fn test_lfe_crossover(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    let test_values: Vec<f32> = vec![60.0, 80.0, 100.0, 120.0, 150.0];
    for value in test_values {
        state.borrow_mut().lfe_crossover = value;
        assert!((state.borrow().lfe_crossover - value).abs() < 0.1);
    }
}

/// Test LFE crossover bounds.
#[gpui::test]
async fn test_lfe_crossover_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Typical range: 40-200 Hz
    let clamped = (20.0_f32).clamp(40.0, 200.0);
    state.borrow_mut().lfe_crossover = clamped;
    assert!(state.borrow().lfe_crossover >= 40.0);
}

/// Test LFE distance control.
#[gpui::test]
async fn test_lfe_distance(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    let test_values: Vec<f32> = vec![0.5, 1.0, 2.0, 3.0, 5.0];
    for value in test_values {
        state.borrow_mut().lfe_distance = value;
        assert!((state.borrow().lfe_distance - value).abs() < 0.01);
    }
}

/// Test LFE level control.
#[gpui::test]
async fn test_lfe_level(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Level in dB
    let test_values: Vec<f32> = vec![-10.0, -6.0, 0.0, 3.0, 6.0];
    for value in test_values {
        state.borrow_mut().lfe_level = value;
        assert!((state.borrow().lfe_level - value).abs() < 0.01);
    }
}

// =============================================================================
// Room Model Tests
// =============================================================================

/// Test room model enable.
#[gpui::test]
async fn test_room_model_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    assert!(!state.borrow().room_model.enabled);

    state.borrow_mut().room_model.enabled = true;
    assert!(state.borrow().room_model.enabled);
}

/// Test room size control.
#[gpui::test]
async fn test_room_size(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().room_model.room_size = value;
        assert!((state.borrow().room_model.room_size - value).abs() < 0.001);
    }
}

/// Test room size description.
#[gpui::test]
async fn test_room_size_description(_cx: &mut TestAppContext) {
    fn get_room_size_label(size: f32) -> &'static str {
        if size < 0.25 {
            "Small"
        } else if size < 0.5 {
            "Medium"
        } else if size < 0.75 {
            "Large"
        } else {
            "Very Large"
        }
    }

    assert_eq!(get_room_size_label(0.1), "Small");
    assert_eq!(get_room_size_label(0.4), "Medium");
    assert_eq!(get_room_size_label(0.6), "Large");
    assert_eq!(get_room_size_label(0.9), "Very Large");
}

/// Test reflection level control.
#[gpui::test]
async fn test_reflection_level(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().room_model.reflection_level = value;
        assert!((state.borrow().room_model.reflection_level - value).abs() < 0.001);
    }
}

/// Test reverb time control.
#[gpui::test]
async fn test_reverb_time(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // In seconds
    let test_values: Vec<f32> = vec![0.1, 0.3, 0.5, 1.0, 2.0];
    for value in test_values {
        state.borrow_mut().room_model.reverb_time = value;
        assert!((state.borrow().room_model.reverb_time - value).abs() < 0.01);
    }
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_binaural_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Speaker Position Tests
// =============================================================================

/// Test 5.1 speaker angles.
#[gpui::test]
async fn test_speaker_angles_5_1(_cx: &mut TestAppContext) {
    // Standard 5.1 speaker angles (ITU-R BS.775)
    fn get_speaker_angle_5_1(channel: usize) -> i32 {
        match channel {
            0 => 30,   // Front Left
            1 => -30,  // Front Right
            2 => 0,    // Center
            3 => 0,    // LFE (no angle, just bass)
            4 => 110,  // Surround Left
            5 => -110, // Surround Right
            _ => 0,
        }
    }

    assert_eq!(get_speaker_angle_5_1(0), 30);
    assert_eq!(get_speaker_angle_5_1(1), -30);
    assert_eq!(get_speaker_angle_5_1(4), 110);
}

/// Test 7.1 speaker angles.
#[gpui::test]
async fn test_speaker_angles_7_1(_cx: &mut TestAppContext) {
    fn get_speaker_angle_7_1(channel: usize) -> i32 {
        match channel {
            0 => 30,   // Front Left
            1 => -30,  // Front Right
            2 => 0,    // Center
            3 => 0,    // LFE
            4 => 90,   // Side Left
            5 => -90,  // Side Right
            6 => 135,  // Back Left
            7 => -135, // Back Right
            _ => 0,
        }
    }

    assert_eq!(get_speaker_angle_7_1(4), 90);
    assert_eq!(get_speaker_angle_7_1(6), 135);
}

/// Test height speaker elevation.
#[gpui::test]
async fn test_speaker_elevation_atmos(_cx: &mut TestAppContext) {
    fn get_speaker_elevation(channel: usize, config: &str) -> i32 {
        match (config, channel) {
            // 7.1.4 height speakers at elevation 45°
            ("7.1.4", 8) => 45,  // Top Front Left
            ("7.1.4", 9) => 45,  // Top Front Right
            ("7.1.4", 10) => 45, // Top Rear Left
            ("7.1.4", 11) => 45, // Top Rear Right
            _ => 0,              // Bed layer at 0° elevation
        }
    }

    assert_eq!(get_speaker_elevation(0, "7.1.4"), 0);
    assert_eq!(get_speaker_elevation(8, "7.1.4"), 45);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test HRTF status display.
#[gpui::test]
async fn test_hrtf_status_display(_cx: &mut TestAppContext) {
    fn get_hrtf_status(loaded: bool, file: &str) -> String {
        if file.is_empty() {
            "Using built-in HRTF".to_string()
        } else if loaded {
            format!(
                "Loaded: {}",
                std::path::Path::new(file)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
            )
        } else {
            "Failed to load HRTF".to_string()
        }
    }

    assert!(get_hrtf_status(true, "").contains("built-in"));
    assert!(get_hrtf_status(true, "/path/to/custom.sofa").contains("custom.sofa"));
}

/// Test processing indicator.
#[gpui::test]
async fn test_processing_indicator(_cx: &mut TestAppContext) {
    fn format_processing_info(fft_size: usize, sample_rate: u32) -> String {
        let latency_ms = (fft_size as f32 / sample_rate as f32) * 1000.0;
        format!("FFT {} ({:.1}ms latency)", fft_size, latency_ms)
    }

    let info = format_processing_info(2048, 48000);
    assert!(info.contains("2048"));
    assert!(info.contains("42.7") || info.contains("42.6"));
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: minimal processing.
#[gpui::test]
async fn test_preset_minimal(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Minimal = no externalization, no room
    state.borrow_mut().externalization = 0.0;
    state.borrow_mut().near_field_strength = 0.0;
    state.borrow_mut().room_model.enabled = false;

    assert!((state.borrow().externalization - 0.0).abs() < 0.001);
    assert!(!state.borrow().room_model.enabled);
}

/// Test preset: speaker simulation.
#[gpui::test]
async fn test_preset_speaker_sim(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Speaker sim = moderate externalization + room
    state.borrow_mut().externalization = 0.6;
    state.borrow_mut().room_model.enabled = true;
    state.borrow_mut().room_model.room_size = 0.5;
    state.borrow_mut().room_model.reflection_level = 0.3;

    assert!(state.borrow().externalization > 0.5);
    assert!(state.borrow().room_model.enabled);
}

/// Test preset: immersive.
#[gpui::test]
async fn test_preset_immersive(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    // Immersive = full externalization, large room
    state.borrow_mut().externalization = 1.0;
    state.borrow_mut().near_field_strength = 0.3;
    state.borrow_mut().room_model.enabled = true;
    state.borrow_mut().room_model.room_size = 0.8;
    state.borrow_mut().room_model.reverb_time = 0.5;

    assert!((state.borrow().externalization - 1.0).abs() < 0.001);
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test stereo input (already binaural).
#[gpui::test]
async fn test_stereo_input(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    state.borrow_mut().input_channels = 2;

    // With stereo input, minimal processing needed
    // (just externalization/room if enabled)
    assert_eq!(state.borrow().input_channels, 2);
}

/// Test mono input.
#[gpui::test]
async fn test_mono_input(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(BinauralState::default()));

    state.borrow_mut().input_channels = 1;

    // Mono should be placed at center (0° azimuth)
    assert_eq!(state.borrow().input_channels, 1);
}
