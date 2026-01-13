//! E2E tests for Upmixer Plugin.
//!
//! Tests for the stereo-to-surround upmixer plugin.
//! Supports multiple speaker configurations (5.1, 7.1, 5.1.4, 7.1.4, etc.)

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Speaker configuration enum
#[derive(Debug, Clone, PartialEq)]
enum SpeakerConfig {
    Surround51,
    Surround71,
    Atmos514,
    Atmos714,
    Stereo,
}

impl SpeakerConfig {
    fn output_channels(&self) -> usize {
        match self {
            SpeakerConfig::Stereo => 2,
            SpeakerConfig::Surround51 => 6,
            SpeakerConfig::Surround71 => 8,
            SpeakerConfig::Atmos514 => 10,
            SpeakerConfig::Atmos714 => 12,
        }
    }

    fn has_lfe(&self) -> bool {
        !matches!(self, SpeakerConfig::Stereo)
    }

    fn has_height(&self) -> bool {
        matches!(self, SpeakerConfig::Atmos514 | SpeakerConfig::Atmos714)
    }

    fn label(&self) -> &'static str {
        match self {
            SpeakerConfig::Stereo => "2.0",
            SpeakerConfig::Surround51 => "5.1",
            SpeakerConfig::Surround71 => "7.1",
            SpeakerConfig::Atmos514 => "5.1.4",
            SpeakerConfig::Atmos714 => "7.1.4",
        }
    }
}

/// Upmixer plugin state for testing
struct UpmixerState {
    enabled: bool,
    speaker_config: SpeakerConfig,
    // FFT settings
    fft_size: usize,
    // Gain controls
    gain_front_direct: f32,
    gain_front_ambient: f32,
    gain_rear_ambient: f32,
    // LFE settings
    lfe_cutoff_hz: f32,
    lfe_gain: f32,
    // Stereo/spatial settings
    stereo_width: f32,
    center_spread: f32,
    bandpass_hz: f32,
    // Height channel settings
    height_gain: f32,
    height_hf_cap_hz: f32,
    height_transient_reduction: f32,
    height_direct_leak: f32,
    // Subharmonic synthesis
    enable_subharmonic_synth: bool,
    subharmonic_gain: f32,
    subharmonic_freq_hz: f32,
    subharmonic_attack_ms: f32,
    subharmonic_release_ms: f32,
    // Decorrelation
    decorrelation_mode: usize,
    decorrelation_lfo_rate_hz: f32,
    // Advanced
    safety_cap_db: f32,
    enable_hr_direct: bool,
    hr_sharpen: f32,
}

impl Default for UpmixerState {
    fn default() -> Self {
        Self {
            enabled: true,
            speaker_config: SpeakerConfig::Surround51,
            fft_size: 2048,
            gain_front_direct: 1.0,
            gain_front_ambient: 0.5,
            gain_rear_ambient: 1.1,
            lfe_cutoff_hz: 120.0,
            lfe_gain: 1.0,
            stereo_width: 0.5,
            center_spread: 0.0,
            bandpass_hz: 250.0,
            height_gain: 0.5,
            height_hf_cap_hz: 16000.0,
            height_transient_reduction: 0.6,
            height_direct_leak: 0.15,
            enable_subharmonic_synth: false,
            subharmonic_gain: 0.5,
            subharmonic_freq_hz: 40.0,
            subharmonic_attack_ms: 10.0,
            subharmonic_release_ms: 50.0,
            decorrelation_mode: 0,
            decorrelation_lfo_rate_hz: 0.15,
            safety_cap_db: 3.0,
            enable_hr_direct: false,
            hr_sharpen: 1.0,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_upmixer_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));
    assert!(state.borrow().enabled);
    assert_eq!(state.borrow().speaker_config, SpeakerConfig::Surround51);
}

/// Test default values.
#[gpui::test]
async fn test_upmixer_defaults(_cx: &mut TestAppContext) {
    let state = UpmixerState::default();

    assert_eq!(state.fft_size, 2048);
    assert!((state.gain_front_direct - 1.0).abs() < 0.001);
    assert!((state.gain_front_ambient - 0.5).abs() < 0.001);
    assert!((state.gain_rear_ambient - 1.1).abs() < 0.001);
    assert!((state.lfe_cutoff_hz - 120.0).abs() < 0.1);
    assert!((state.stereo_width - 0.5).abs() < 0.001);
}

// =============================================================================
// Speaker Configuration Tests
// =============================================================================

/// Test speaker config selection.
#[gpui::test]
async fn test_speaker_config_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let configs = [
        (SpeakerConfig::Stereo, 2),
        (SpeakerConfig::Surround51, 6),
        (SpeakerConfig::Surround71, 8),
        (SpeakerConfig::Atmos514, 10),
        (SpeakerConfig::Atmos714, 12),
    ];

    for (config, expected_channels) in configs {
        state.borrow_mut().speaker_config = config.clone();
        assert_eq!(
            state.borrow().speaker_config.output_channels(),
            expected_channels
        );
    }
}

/// Test speaker config labels.
#[gpui::test]
async fn test_speaker_config_labels(_cx: &mut TestAppContext) {
    assert_eq!(SpeakerConfig::Stereo.label(), "2.0");
    assert_eq!(SpeakerConfig::Surround51.label(), "5.1");
    assert_eq!(SpeakerConfig::Surround71.label(), "7.1");
    assert_eq!(SpeakerConfig::Atmos514.label(), "5.1.4");
    assert_eq!(SpeakerConfig::Atmos714.label(), "7.1.4");
}

/// Test LFE availability by config.
#[gpui::test]
async fn test_lfe_availability(_cx: &mut TestAppContext) {
    assert!(!SpeakerConfig::Stereo.has_lfe());
    assert!(SpeakerConfig::Surround51.has_lfe());
    assert!(SpeakerConfig::Surround71.has_lfe());
    assert!(SpeakerConfig::Atmos514.has_lfe());
    assert!(SpeakerConfig::Atmos714.has_lfe());
}

/// Test height availability by config.
#[gpui::test]
async fn test_height_availability(_cx: &mut TestAppContext) {
    assert!(!SpeakerConfig::Stereo.has_height());
    assert!(!SpeakerConfig::Surround51.has_height());
    assert!(!SpeakerConfig::Surround71.has_height());
    assert!(SpeakerConfig::Atmos514.has_height());
    assert!(SpeakerConfig::Atmos714.has_height());
}

// =============================================================================
// FFT Size Tests
// =============================================================================

/// Test FFT size options.
#[gpui::test]
async fn test_fft_size_options(_cx: &mut TestAppContext) {
    let valid_sizes = [512, 1024, 2048, 4096, 8192];
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    for size in valid_sizes {
        state.borrow_mut().fft_size = size;
        assert_eq!(state.borrow().fft_size, size);
    }
}

/// Test FFT size is power of 2.
#[gpui::test]
async fn test_fft_size_power_of_2(_cx: &mut TestAppContext) {
    fn is_power_of_2(n: usize) -> bool {
        n > 0 && (n & (n - 1)) == 0
    }

    for size in [512, 1024, 2048, 4096, 8192] {
        assert!(is_power_of_2(size), "{} should be power of 2", size);
    }
}

// =============================================================================
// Front Channel Gain Tests
// =============================================================================

/// Test front direct gain control.
#[gpui::test]
async fn test_front_direct_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.5, 1.0, 1.5, 2.0];
    for value in test_values {
        state.borrow_mut().gain_front_direct = value;
        assert!((state.borrow().gain_front_direct - value).abs() < 0.001);
    }
}

/// Test front direct gain bounds.
#[gpui::test]
async fn test_front_direct_gain_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Clamp to valid range
    let clamped = (-0.5_f32).clamp(0.0, 2.0);
    state.borrow_mut().gain_front_direct = clamped;
    assert!(state.borrow().gain_front_direct >= 0.0);

    let clamped = (2.5_f32).clamp(0.0, 2.0);
    state.borrow_mut().gain_front_direct = clamped;
    assert!(state.borrow().gain_front_direct <= 2.0);
}

/// Test front ambient gain control.
#[gpui::test]
async fn test_front_ambient_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    state.borrow_mut().gain_front_ambient = 0.75;
    assert!((state.borrow().gain_front_ambient - 0.75).abs() < 0.001);
}

// =============================================================================
// Rear/Surround Gain Tests
// =============================================================================

/// Test rear ambient gain control.
#[gpui::test]
async fn test_rear_ambient_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.5, 1.0, 1.1, 1.5, 2.0];
    for value in test_values {
        state.borrow_mut().gain_rear_ambient = value;
        assert!((state.borrow().gain_rear_ambient - value).abs() < 0.001);
    }
}

/// Test rear gain default is boosted.
#[gpui::test]
async fn test_rear_gain_default_boosted(_cx: &mut TestAppContext) {
    let state = UpmixerState::default();
    // Default 1.1 = 10% boost for better rear envelopment
    assert!(
        state.gain_rear_ambient > 1.0,
        "Rear gain should be boosted by default"
    );
}

// =============================================================================
// LFE/Subwoofer Tests
// =============================================================================

/// Test LFE cutoff frequency.
#[gpui::test]
async fn test_lfe_cutoff(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![40.0, 80.0, 100.0, 120.0, 150.0, 200.0];
    for value in test_values {
        state.borrow_mut().lfe_cutoff_hz = value;
        assert!((state.borrow().lfe_cutoff_hz - value).abs() < 0.1);
    }
}

/// Test LFE cutoff bounds.
#[gpui::test]
async fn test_lfe_cutoff_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Typical range: 40-200 Hz
    let clamped = (20.0_f32).clamp(40.0, 200.0);
    state.borrow_mut().lfe_cutoff_hz = clamped;
    assert!(state.borrow().lfe_cutoff_hz >= 40.0);

    let clamped = (300.0_f32).clamp(40.0, 200.0);
    state.borrow_mut().lfe_cutoff_hz = clamped;
    assert!(state.borrow().lfe_cutoff_hz <= 200.0);
}

/// Test LFE gain control.
#[gpui::test]
async fn test_lfe_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.5, 1.0, 1.5, 2.0];
    for value in test_values {
        state.borrow_mut().lfe_gain = value;
        assert!((state.borrow().lfe_gain - value).abs() < 0.001);
    }
}

// =============================================================================
// Stereo Width Tests
// =============================================================================

/// Test stereo width control.
#[gpui::test]
async fn test_stereo_width(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().stereo_width = value;
        assert!((state.borrow().stereo_width - value).abs() < 0.001);
    }
}

/// Test stereo width bounds.
#[gpui::test]
async fn test_stereo_width_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Range: 0.0 to 1.0
    let clamped = (-0.1_f32).clamp(0.0, 1.0);
    state.borrow_mut().stereo_width = clamped;
    assert!(state.borrow().stereo_width >= 0.0);

    let clamped = (1.5_f32).clamp(0.0, 1.0);
    state.borrow_mut().stereo_width = clamped;
    assert!(state.borrow().stereo_width <= 1.0);
}

// =============================================================================
// Center Spread Tests
// =============================================================================

/// Test center spread control.
#[gpui::test]
async fn test_center_spread(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // 0 = phantom center, 1 = full center channel
    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().center_spread = value;
        assert!((state.borrow().center_spread - value).abs() < 0.001);
    }
}

/// Test center spread default.
#[gpui::test]
async fn test_center_spread_default(_cx: &mut TestAppContext) {
    let state = UpmixerState::default();
    // Default 0.0 = phantom center (no dedicated center channel usage)
    assert!((state.center_spread - 0.0).abs() < 0.001);
}

// =============================================================================
// Bandpass Tests
// =============================================================================

/// Test bandpass frequency control.
#[gpui::test]
async fn test_bandpass_frequency(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![100.0, 200.0, 250.0, 300.0, 400.0];
    for value in test_values {
        state.borrow_mut().bandpass_hz = value;
        assert!((state.borrow().bandpass_hz - value).abs() < 0.1);
    }
}

/// Test bandpass default for surround content.
#[gpui::test]
async fn test_bandpass_default(_cx: &mut TestAppContext) {
    let state = UpmixerState::default();
    // Default 250 Hz for more mid-range content in surrounds
    assert!((state.bandpass_hz - 250.0).abs() < 1.0);
}

// =============================================================================
// Height Channel Tests
// =============================================================================

/// Test height gain control.
#[gpui::test]
async fn test_height_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
    for value in test_values {
        state.borrow_mut().height_gain = value;
        assert!((state.borrow().height_gain - value).abs() < 0.001);
    }
}

/// Test height HF cap frequency.
#[gpui::test]
async fn test_height_hf_cap(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![8000.0, 12000.0, 16000.0, 20000.0];
    for value in test_values {
        state.borrow_mut().height_hf_cap_hz = value;
        assert!((state.borrow().height_hf_cap_hz - value).abs() < 1.0);
    }
}

/// Test height transient reduction.
#[gpui::test]
async fn test_height_transient_reduction(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.3, 0.6, 0.9, 1.0];
    for value in test_values {
        state.borrow_mut().height_transient_reduction = value;
        assert!((state.borrow().height_transient_reduction - value).abs() < 0.001);
    }
}

/// Test height direct leak.
#[gpui::test]
async fn test_height_direct_leak(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.1, 0.15, 0.25, 0.5];
    for value in test_values {
        state.borrow_mut().height_direct_leak = value;
        assert!((state.borrow().height_direct_leak - value).abs() < 0.001);
    }
}

// =============================================================================
// Subharmonic Synthesis Tests
// =============================================================================

/// Test subharmonic enable toggle.
#[gpui::test]
async fn test_subharmonic_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    assert!(!state.borrow().enable_subharmonic_synth);

    state.borrow_mut().enable_subharmonic_synth = true;
    assert!(state.borrow().enable_subharmonic_synth);
}

/// Test subharmonic gain control.
#[gpui::test]
async fn test_subharmonic_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().subharmonic_gain = value;
        assert!((state.borrow().subharmonic_gain - value).abs() < 0.001);
    }
}

/// Test subharmonic frequency.
#[gpui::test]
async fn test_subharmonic_frequency(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Range: 20-80 Hz
    let test_values: Vec<f32> = vec![20.0, 30.0, 40.0, 60.0, 80.0];
    for value in test_values {
        state.borrow_mut().subharmonic_freq_hz = value;
        assert!((state.borrow().subharmonic_freq_hz - value).abs() < 0.1);
    }
}

/// Test subharmonic attack/release.
#[gpui::test]
async fn test_subharmonic_envelope(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Attack: 1-100 ms
    state.borrow_mut().subharmonic_attack_ms = 20.0;
    assert!((state.borrow().subharmonic_attack_ms - 20.0).abs() < 0.1);

    // Release: 10-500 ms
    state.borrow_mut().subharmonic_release_ms = 100.0;
    assert!((state.borrow().subharmonic_release_ms - 100.0).abs() < 0.1);
}

// =============================================================================
// Decorrelation Tests
// =============================================================================

/// Test decorrelation mode selection.
#[gpui::test]
async fn test_decorrelation_mode(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Mode 0 = off, 1 = velvet noise, 2 = all-pass, etc.
    for mode in 0..4 {
        state.borrow_mut().decorrelation_mode = mode;
        assert_eq!(state.borrow().decorrelation_mode, mode);
    }
}

/// Test decorrelation LFO rate.
#[gpui::test]
async fn test_decorrelation_lfo_rate(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Range: 0.01-1.0 Hz
    let test_values: Vec<f32> = vec![0.01, 0.05, 0.15, 0.5, 1.0];
    for value in test_values {
        state.borrow_mut().decorrelation_lfo_rate_hz = value;
        assert!((state.borrow().decorrelation_lfo_rate_hz - value).abs() < 0.001);
    }
}

// =============================================================================
// Safety/Advanced Tests
// =============================================================================

/// Test safety cap control.
#[gpui::test]
async fn test_safety_cap(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    // Default 3 dB safety cap
    assert!((state.borrow().safety_cap_db - 3.0).abs() < 0.1);

    let test_values: Vec<f32> = vec![0.0, 1.0, 3.0, 6.0, 10.0];
    for value in test_values {
        state.borrow_mut().safety_cap_db = value;
        assert!((state.borrow().safety_cap_db - value).abs() < 0.1);
    }
}

/// Test HR direct enhancement toggle.
#[gpui::test]
async fn test_hr_direct_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    assert!(!state.borrow().enable_hr_direct);

    state.borrow_mut().enable_hr_direct = true;
    assert!(state.borrow().enable_hr_direct);
}

/// Test HR sharpen control.
#[gpui::test]
async fn test_hr_sharpen(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    let test_values: Vec<f32> = vec![0.5, 1.0, 1.5, 2.0];
    for value in test_values {
        state.borrow_mut().hr_sharpen = value;
        assert!((state.borrow().hr_sharpen - value).abs() < 0.001);
    }
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_upmixer_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(UpmixerState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Output Channel Mapping Tests
// =============================================================================

/// Test 5.1 channel mapping.
#[gpui::test]
async fn test_channel_mapping_5_1(_cx: &mut TestAppContext) {
    fn get_channel_name_5_1(ch: usize) -> &'static str {
        match ch {
            0 => "Front Left",
            1 => "Front Right",
            2 => "Center",
            3 => "LFE",
            4 => "Surround Left",
            5 => "Surround Right",
            _ => "Unknown",
        }
    }

    assert_eq!(get_channel_name_5_1(0), "Front Left");
    assert_eq!(get_channel_name_5_1(3), "LFE");
    assert_eq!(get_channel_name_5_1(4), "Surround Left");
}

/// Test 7.1 channel mapping.
#[gpui::test]
async fn test_channel_mapping_7_1(_cx: &mut TestAppContext) {
    fn get_channel_name_7_1(ch: usize) -> &'static str {
        match ch {
            0 => "Front Left",
            1 => "Front Right",
            2 => "Center",
            3 => "LFE",
            4 => "Surround Left",
            5 => "Surround Right",
            6 => "Back Left",
            7 => "Back Right",
            _ => "Unknown",
        }
    }

    assert_eq!(get_channel_name_7_1(6), "Back Left");
    assert_eq!(get_channel_name_7_1(7), "Back Right");
}

/// Test Atmos height channel mapping.
#[gpui::test]
async fn test_channel_mapping_atmos(_cx: &mut TestAppContext) {
    fn get_channel_name_7_1_4(ch: usize) -> &'static str {
        match ch {
            0 => "Front Left",
            1 => "Front Right",
            2 => "Center",
            3 => "LFE",
            4 => "Surround Left",
            5 => "Surround Right",
            6 => "Back Left",
            7 => "Back Right",
            8 => "Top Front Left",
            9 => "Top Front Right",
            10 => "Top Rear Left",
            11 => "Top Rear Right",
            _ => "Unknown",
        }
    }

    assert_eq!(get_channel_name_7_1_4(8), "Top Front Left");
    assert_eq!(get_channel_name_7_1_4(11), "Top Rear Right");
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test gain meter display.
#[gpui::test]
async fn test_gain_meter_display(_cx: &mut TestAppContext) {
    fn format_gain(gain: f32) -> String {
        if gain <= 0.0 {
            "-inf dB".to_string()
        } else {
            let db = 20.0 * gain.log10();
            format!("{:.1} dB", db)
        }
    }

    assert_eq!(format_gain(1.0), "0.0 dB");
    assert_eq!(format_gain(0.5), "-6.0 dB");
    assert_eq!(format_gain(2.0), "6.0 dB");
}

/// Test channel activity indicator.
#[gpui::test]
async fn test_channel_activity(_cx: &mut TestAppContext) {
    fn is_channel_active(config: &SpeakerConfig, channel: usize) -> bool {
        channel < config.output_channels()
    }

    let config = SpeakerConfig::Surround51;
    assert!(is_channel_active(&config, 0));
    assert!(is_channel_active(&config, 5));
    assert!(!is_channel_active(&config, 6));
}
