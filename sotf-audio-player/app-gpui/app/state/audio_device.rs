//! Audio device state management.
//!
//! Contains state related to audio input/output devices and playback source.

use sotf_audio::devices::AudioDevice;

use crate::app::types::PlaybackSource;

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
}
