//! Audio device selection logic.
//!
//! Shared between all app frontends (GPUI, TUI, etc.)

use sotf_audio::devices::AudioDevice;

/// Core audio output device state: devices list, selected index, and current device name.
///
/// Frontends may embed this struct and add their own UI-specific fields
/// (HAL config, dropdown open states, input devices, etc.).
#[derive(Debug, Clone, Default)]
pub struct AudioOutputDeviceState {
    /// Available output devices
    pub devices: Vec<AudioDevice>,
    /// Currently selected output device index
    pub selected_index: usize,
    /// Name of the currently active output device
    pub current_device_name: Option<String>,
}

impl AudioOutputDeviceState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently selected output device.
    pub fn selected_device(&self) -> Option<&AudioDevice> {
        self.devices.get(self.selected_index)
    }

    /// Select output device by index. Returns true if selection changed.
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.devices.len() && index != self.selected_index {
            self.selected_index = index;
            true
        } else {
            false
        }
    }

    /// Select next output device (wraps around).
    pub fn select_next(&mut self) {
        if !self.devices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.devices.len();
        }
    }

    /// Select previous output device (wraps around).
    pub fn select_previous(&mut self) {
        if !self.devices.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.devices.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Update the list of available output devices. Clamps selection to valid range.
    pub fn set_devices(&mut self, devices: Vec<AudioDevice>) {
        self.devices = devices;
        if self.selected_index >= self.devices.len() {
            self.selected_index = 0;
        }
    }

    /// Update output devices and select the best default (prefers non-virtual, non-HAL).
    pub fn set_devices_with_smart_default(&mut self, devices: Vec<AudioDevice>) {
        self.devices = devices;
        self.selected_index = self.find_best_default_index();
        if let Some(device) = self.devices.get(self.selected_index) {
            self.current_device_name = Some(device.name.clone());
        }
    }

    /// Get the max channels supported by the selected device.
    pub fn max_channels(&self) -> Option<usize> {
        self.selected_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.channels as usize)
    }

    /// Get the current device sample rate, or 48 kHz as fallback.
    pub fn current_sample_rate(&self) -> f64 {
        self.selected_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.sample_rate as f64)
            .unwrap_or(48000.0)
    }

    /// Find the best default output device index.
    /// Priority: system default non-virtual > any non-virtual > system default > first.
    pub fn find_best_default_index(&self) -> usize {
        // Prefer system default that's not virtual
        if let Some((idx, _)) = self
            .devices
            .iter()
            .enumerate()
            .find(|(_, d)| d.is_default && !is_virtual_device(&d.name))
        {
            return idx;
        }

        // Any non-virtual device
        if let Some((idx, _)) = self
            .devices
            .iter()
            .enumerate()
            .find(|(_, d)| !is_virtual_device(&d.name))
        {
            return idx;
        }

        // System default (even if virtual)
        self.devices
            .iter()
            .enumerate()
            .find(|(_, d)| d.is_default)
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }
}

/// Check if a device name is a virtual/passthrough device that shouldn't be default.
pub fn is_virtual_device(name: &str) -> bool {
    let lower_name = name.to_lowercase();
    lower_name.contains("sotf")
        || lower_name.contains("hal")
        || lower_name.contains("blackhole")
        || lower_name.contains("zoomaudio")
        || lower_name.contains("soundflower")
        || lower_name.contains("loopback")
        || lower_name.contains("virtual")
        || lower_name.contains("background music")
        || lower_name.contains("audio bridge")
        || name.trim().eq_ignore_ascii_case("null")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, is_default: bool) -> AudioDevice {
        AudioDevice {
            device_id: Some(name.to_string()),
            name: name.to_string(),
            display_info: None,
            is_input: false,
            is_default,
            supported_configs: Vec::new(),
            default_config: None,
            available_sample_rates: Vec::new(),
        }
    }

    #[test]
    fn virtual_device_detector_matches_systemwide_virtual_output() {
        assert!(is_virtual_device("SotF Virtual Device"));
        assert!(is_virtual_device("SotF Virtual Output"));
        assert!(is_virtual_device("BlackHole 2ch"));
        assert!(is_virtual_device("Loopback Audio"));
        assert!(is_virtual_device("Generic Virtual Device"));
    }

    #[test]
    fn smart_default_skips_virtual_system_default() {
        let mut state = AudioOutputDeviceState::new();
        state.set_devices(vec![
            device("SotF Virtual Device", true),
            device("Built-in Output", false),
        ]);

        assert_eq!(state.find_best_default_index(), 1);
    }
}
