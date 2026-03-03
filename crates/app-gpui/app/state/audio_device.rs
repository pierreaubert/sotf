//! Audio device state management.
//!
//! Contains state related to audio input/output devices and playback source.

use sotf_audio::devices::AudioDevice;

use crate::app::types::PlaybackSource;

/// Common sample rates for audio devices
pub const SAMPLE_RATES: &[u32] = &[44100, 48000, 88200, 96000, 176400, 192000];

/// Common buffer sizes (in frames)
pub const BUFFER_SIZES: &[u32] = &[128, 256, 512, 1024, 2048, 4096];

/// HAL driver configuration
#[derive(Debug, Clone)]
pub struct HalConfig {
    /// Sample rate in Hz (default: 48000)
    pub sample_rate: u32,
    /// Number of audio channels (default: 2)
    pub channel_count: u32,
    /// Buffer size in frames (default: 1024)
    pub buffer_frames: u32,
}

/// State for HAL configuration dropdowns
#[derive(Debug, Clone, Default)]
pub struct HalDropdownState {
    /// Whether sample rate dropdown is open
    pub sample_rate_open: bool,
    /// Whether channel count dropdown is open
    pub channel_count_open: bool,
    /// Whether buffer size dropdown is open
    pub buffer_size_open: bool,
}

impl Default for HalConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channel_count: 2,
            buffer_frames: 1024,
        }
    }
}

impl HalConfig {
    /// Get available sample rates
    pub fn available_sample_rates() -> &'static [u32] {
        SAMPLE_RATES
    }

    /// Get available buffer sizes
    pub fn available_buffer_sizes() -> &'static [u32] {
        BUFFER_SIZES
    }

    /// Get sample rate as display string (e.g., "48 kHz")
    pub fn sample_rate_display(&self) -> String {
        format_sample_rate(self.sample_rate)
    }

    /// Get buffer size as display string with latency (e.g., "1024 (~21ms)")
    pub fn buffer_frames_display(&self) -> String {
        format_buffer_size(self.buffer_frames, self.sample_rate)
    }
}

/// Format sample rate for display (e.g., 48000 -> "48 kHz")
pub fn format_sample_rate(sample_rate: u32) -> String {
    if sample_rate.is_multiple_of(1000) {
        format!("{} kHz", sample_rate / 1000)
    } else {
        format!("{:.1} kHz", sample_rate as f32 / 1000.0)
    }
}

/// Format buffer size for display with latency estimate
pub fn format_buffer_size(buffer_frames: u32, sample_rate: u32) -> String {
    let latency_ms = (buffer_frames as f64 / sample_rate as f64) * 1000.0;
    format!("{} (~{:.1}ms)", buffer_frames, latency_ms)
}

/// Audio device state for input and output device selection
#[derive(Debug, Clone, Default)]
pub struct AudioDeviceState {
    /// Available output devices
    pub output_devices: Vec<AudioDevice>,
    /// Currently selected output device index
    pub selected_output_device_index: usize,
    /// Name of the currently active output device (may differ from selected during transitions)
    pub current_output_device_name: Option<String>,

    /// Available input devices
    pub input_devices: Vec<AudioDevice>,
    /// Currently selected input device index
    pub selected_input_device_index: usize,
    /// Name of the currently active input device
    pub current_input_device_name: Option<String>,

    /// Audio source mode (File player or HAL device input)
    pub playback_source: PlaybackSource,

    /// HAL driver configuration (sample rate, channels, buffer size)
    pub hal_config: HalConfig,

    /// HAL dropdown open states
    pub hal_dropdowns: HalDropdownState,
}

impl AudioDeviceState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently selected output device
    pub fn selected_output_device(&self) -> Option<&AudioDevice> {
        self.output_devices.get(self.selected_output_device_index)
    }

    /// Get the currently selected input device
    pub fn selected_input_device(&self) -> Option<&AudioDevice> {
        self.input_devices.get(self.selected_input_device_index)
    }

    /// Select output device by index, returns true if selection changed
    pub fn select_output_device(&mut self, index: usize) -> bool {
        if index < self.output_devices.len() && index != self.selected_output_device_index {
            self.selected_output_device_index = index;
            true
        } else {
            false
        }
    }

    /// Select input device by index, returns true if selection changed
    pub fn select_input_device(&mut self, index: usize) -> bool {
        if index < self.input_devices.len() && index != self.selected_input_device_index {
            self.selected_input_device_index = index;
            true
        } else {
            false
        }
    }

    /// Update the list of available output devices
    pub fn set_output_devices(&mut self, devices: Vec<AudioDevice>) {
        self.output_devices = devices;
        // Clamp selection to valid range
        if self.selected_output_device_index >= self.output_devices.len() {
            self.selected_output_device_index = 0;
        }
    }

    /// Update the list of available input devices
    pub fn set_input_devices(&mut self, devices: Vec<AudioDevice>) {
        self.input_devices = devices;
        // Clamp selection to valid range
        if self.selected_input_device_index >= self.input_devices.len() {
            self.selected_input_device_index = 0;
        }
    }

    /// Check if playback source is file-based
    pub fn is_file_source(&self) -> bool {
        matches!(self.playback_source, PlaybackSource::File)
    }

    /// Check if a device name is a virtual/passthrough device that shouldn't be default
    pub fn is_virtual_device(name: &str) -> bool {
        sotf_audio_player::is_virtual_device(name)
    }

    /// Find the best default output device index (prefers non-virtual, is_default, then first)
    pub fn find_best_default_device_index(&self) -> usize {
        // First, try to find the system default device that's not virtual
        if let Some((idx, _)) = self
            .output_devices
            .iter()
            .enumerate()
            .find(|(_, d)| d.is_default && !Self::is_virtual_device(&d.name))
        {
            return idx;
        }

        // Then, find any non-virtual device
        if let Some((idx, _)) = self
            .output_devices
            .iter()
            .enumerate()
            .find(|(_, d)| !Self::is_virtual_device(&d.name))
        {
            return idx;
        }

        // Finally, fall back to the system default (even if virtual)
        self.output_devices
            .iter()
            .enumerate()
            .find(|(_, d)| d.is_default)
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    /// Update output devices and select the best default
    pub fn set_output_devices_with_smart_default(&mut self, devices: Vec<AudioDevice>) {
        self.output_devices = devices;
        self.selected_output_device_index = self.find_best_default_device_index();
        if let Some(device) = self.output_devices.get(self.selected_output_device_index) {
            self.current_output_device_name = Some(device.name.clone());
        }
    }

    /// Close all HAL dropdowns
    pub fn close_hal_dropdowns(&mut self) {
        self.hal_dropdowns.sample_rate_open = false;
        self.hal_dropdowns.channel_count_open = false;
        self.hal_dropdowns.buffer_size_open = false;
    }
}
