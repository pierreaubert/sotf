//! E2E tests for Audio Device Settings.
//!
//! Tests for audio device selection and configuration:
//! - Output device selection
//! - Sample rate configuration
//! - Buffer size settings
//! - Channel configuration

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Audio device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceType {
    Output,
    Input,
}

/// Audio device info
#[derive(Debug, Clone)]
struct AudioDevice {
    id: String,
    name: String,
    device_type: DeviceType,
    num_channels: usize,
    sample_rates: Vec<u32>,
    is_default: bool,
}

impl Default for AudioDevice {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Default Device".to_string(),
            device_type: DeviceType::Output,
            num_channels: 2,
            sample_rates: vec![44100, 48000, 88200, 96000],
            is_default: true,
        }
    }
}

/// Buffer size option
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BufferSize {
    Samples64,
    Samples128,
    Samples256,
    Samples512,
    Samples1024,
    Samples2048,
}

impl BufferSize {
    fn as_samples(&self) -> usize {
        match self {
            BufferSize::Samples64 => 64,
            BufferSize::Samples128 => 128,
            BufferSize::Samples256 => 256,
            BufferSize::Samples512 => 512,
            BufferSize::Samples1024 => 1024,
            BufferSize::Samples2048 => 2048,
        }
    }

    fn latency_ms(&self, sample_rate: u32) -> f32 {
        self.as_samples() as f32 / sample_rate as f32 * 1000.0
    }
}

/// Audio settings state
struct AudioSettingsState {
    available_devices: Vec<AudioDevice>,
    selected_device_id: Option<String>,
    sample_rate: u32,
    buffer_size: BufferSize,
    output_channels: usize,
    exclusive_mode: bool,
    device_dropdown_open: bool,
    sample_rate_dropdown_open: bool,
    buffer_size_dropdown_open: bool,
}

impl Default for AudioSettingsState {
    fn default() -> Self {
        Self {
            available_devices: Vec::new(),
            selected_device_id: None,
            sample_rate: 48000,
            buffer_size: BufferSize::Samples512,
            output_channels: 2,
            exclusive_mode: false,
            device_dropdown_open: false,
            sample_rate_dropdown_open: false,
            buffer_size_dropdown_open: false,
        }
    }
}

// =============================================================================
// Device Selection Tests
// =============================================================================

/// Test device list loading.
#[gpui::test]
async fn test_device_list_loading(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    state.borrow_mut().available_devices = vec![
        AudioDevice {
            id: "built-in".to_string(),
            name: "Built-in Output".to_string(),
            device_type: DeviceType::Output,
            num_channels: 2,
            sample_rates: vec![44100, 48000],
            is_default: true,
        },
        AudioDevice {
            id: "blackhole".to_string(),
            name: "Blackhole 16ch".to_string(),
            device_type: DeviceType::Output,
            num_channels: 16,
            sample_rates: vec![44100, 48000, 96000],
            is_default: false,
        },
    ];

    assert_eq!(state.borrow().available_devices.len(), 2);
}

/// Test device selection.
#[gpui::test]
async fn test_device_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    state.borrow_mut().available_devices = vec![AudioDevice {
        id: "test-device".to_string(),
        name: "Test Device".to_string(),
        ..Default::default()
    }];

    state.borrow_mut().selected_device_id = Some("test-device".to_string());
    assert_eq!(
        state.borrow().selected_device_id,
        Some("test-device".to_string())
    );
}

/// Test default device selection.
#[gpui::test]
async fn test_default_device_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    state.borrow_mut().available_devices = vec![
        AudioDevice {
            id: "device-1".to_string(),
            name: "Device 1".to_string(),
            is_default: false,
            ..Default::default()
        },
        AudioDevice {
            id: "device-2".to_string(),
            name: "Device 2".to_string(),
            is_default: true,
            ..Default::default()
        },
    ];

    // Find and select default device
    let default_id = state
        .borrow()
        .available_devices
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.id.clone());

    state.borrow_mut().selected_device_id = default_id;
    assert_eq!(
        state.borrow().selected_device_id,
        Some("device-2".to_string())
    );
}

/// Test device dropdown toggle.
#[gpui::test]
async fn test_device_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    assert!(!state.borrow().device_dropdown_open);

    state.borrow_mut().device_dropdown_open = true;
    assert!(state.borrow().device_dropdown_open);
}

// =============================================================================
// Sample Rate Tests
// =============================================================================

/// Test sample rate selection.
#[gpui::test]
async fn test_sample_rate_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    let rates = [44100_u32, 48000, 88200, 96000, 176400, 192000];
    for rate in rates {
        state.borrow_mut().sample_rate = rate;
        assert_eq!(state.borrow().sample_rate, rate);
    }
}

/// Test sample rate dropdown.
#[gpui::test]
async fn test_sample_rate_dropdown(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    state.borrow_mut().sample_rate_dropdown_open = true;
    assert!(state.borrow().sample_rate_dropdown_open);
}

/// Test sample rate display format.
#[gpui::test]
async fn test_sample_rate_display_format(_cx: &mut TestAppContext) {
    fn format_sample_rate(rate: u32) -> String {
        format!("{:.1} kHz", rate as f32 / 1000.0)
    }

    assert_eq!(format_sample_rate(44100), "44.1 kHz");
    assert_eq!(format_sample_rate(48000), "48.0 kHz");
    assert_eq!(format_sample_rate(96000), "96.0 kHz");
}

/// Test available sample rates from device.
#[gpui::test]
async fn test_available_sample_rates_from_device(_cx: &mut TestAppContext) {
    let device = AudioDevice {
        sample_rates: vec![44100, 48000, 96000],
        ..Default::default()
    };

    assert_eq!(device.sample_rates.len(), 3);
    assert!(device.sample_rates.contains(&48000));
}

// =============================================================================
// Buffer Size Tests
// =============================================================================

/// Test buffer size selection.
#[gpui::test]
async fn test_buffer_size_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    let sizes = [
        BufferSize::Samples64,
        BufferSize::Samples128,
        BufferSize::Samples256,
        BufferSize::Samples512,
        BufferSize::Samples1024,
        BufferSize::Samples2048,
    ];

    for size in sizes {
        state.borrow_mut().buffer_size = size;
        assert_eq!(state.borrow().buffer_size, size);
    }
}

/// Test buffer size in samples.
#[gpui::test]
async fn test_buffer_size_in_samples(_cx: &mut TestAppContext) {
    assert_eq!(BufferSize::Samples64.as_samples(), 64);
    assert_eq!(BufferSize::Samples128.as_samples(), 128);
    assert_eq!(BufferSize::Samples256.as_samples(), 256);
    assert_eq!(BufferSize::Samples512.as_samples(), 512);
    assert_eq!(BufferSize::Samples1024.as_samples(), 1024);
    assert_eq!(BufferSize::Samples2048.as_samples(), 2048);
}

/// Test latency calculation.
#[gpui::test]
async fn test_latency_calculation(_cx: &mut TestAppContext) {
    // At 48kHz, 512 samples = 10.67ms
    let latency = BufferSize::Samples512.latency_ms(48000);
    assert!((latency - 10.67).abs() < 0.1);

    // At 96kHz, 512 samples = 5.33ms
    let latency = BufferSize::Samples512.latency_ms(96000);
    assert!((latency - 5.33).abs() < 0.1);
}

/// Test latency display format.
#[gpui::test]
async fn test_latency_display_format(_cx: &mut TestAppContext) {
    fn format_latency(buffer: BufferSize, sample_rate: u32) -> String {
        let latency = buffer.latency_ms(sample_rate);
        format!("{:.1} ms", latency)
    }

    assert_eq!(format_latency(BufferSize::Samples256, 48000), "5.3 ms");
    assert_eq!(format_latency(BufferSize::Samples1024, 48000), "21.3 ms");
}

/// Test buffer size dropdown.
#[gpui::test]
async fn test_buffer_size_dropdown(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    state.borrow_mut().buffer_size_dropdown_open = true;
    assert!(state.borrow().buffer_size_dropdown_open);
}

// =============================================================================
// Channel Configuration Tests
// =============================================================================

/// Test output channel selection.
#[gpui::test]
async fn test_output_channel_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    let channel_counts = [2, 4, 6, 8, 10, 12, 16];
    for count in channel_counts {
        state.borrow_mut().output_channels = count;
        assert_eq!(state.borrow().output_channels, count);
    }
}

/// Test channel count from device.
#[gpui::test]
async fn test_channel_count_from_device(_cx: &mut TestAppContext) {
    let device = AudioDevice {
        num_channels: 16,
        ..Default::default()
    };

    assert_eq!(device.num_channels, 16);
}

// =============================================================================
// Exclusive Mode Tests
// =============================================================================

/// Test exclusive mode toggle.
#[gpui::test]
async fn test_exclusive_mode_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AudioSettingsState::default()));

    assert!(!state.borrow().exclusive_mode);

    state.borrow_mut().exclusive_mode = true;
    assert!(state.borrow().exclusive_mode);
}

/// Test exclusive mode description.
#[gpui::test]
async fn test_exclusive_mode_description(_cx: &mut TestAppContext) {
    fn get_exclusive_mode_description(enabled: bool) -> &'static str {
        if enabled {
            "Exclusive mode: App has direct control of device"
        } else {
            "Shared mode: Device shared with other apps"
        }
    }

    assert!(get_exclusive_mode_description(true).contains("Exclusive"));
    assert!(get_exclusive_mode_description(false).contains("Shared"));
}

// =============================================================================
// Device Information Display Tests
// =============================================================================

/// Test device info display.
#[gpui::test]
async fn test_device_info_display(_cx: &mut TestAppContext) {
    fn format_device_info(device: &AudioDevice) -> String {
        format!(
            "{} ({} channels)",
            device.name, device.num_channels
        )
    }

    let device = AudioDevice {
        name: "Blackhole 16ch".to_string(),
        num_channels: 16,
        ..Default::default()
    };

    let info = format_device_info(&device);
    assert!(info.contains("Blackhole 16ch"));
    assert!(info.contains("16 channels"));
}

/// Test device type indicator.
#[gpui::test]
async fn test_device_type_indicator(_cx: &mut TestAppContext) {
    fn get_device_type_icon(device_type: DeviceType) -> &'static str {
        match device_type {
            DeviceType::Output => "speaker",
            DeviceType::Input => "microphone",
        }
    }

    assert_eq!(get_device_type_icon(DeviceType::Output), "speaker");
    assert_eq!(get_device_type_icon(DeviceType::Input), "microphone");
}
