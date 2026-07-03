// ============================================================================
// Chromecast Audio Sender (CASTV2)
// ============================================================================
//
// Chromecast output is intentionally gated until we have a real CASTV2 LOAD
// implementation. The previous local HTTP/WAV sender was not enough to make a
// Chromecast start playback and left callers with a false "connected" state.

use crate::discovery::CastDevice;
use std::net::Ipv4Addr;

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromecastState {
    Disconnected,
    Connecting,
    Connected,
    Launching,
    Streaming,
}

/// Chromecast audio sender.
pub struct ChromecastSender {
    device: CastDevice,
    state: ChromecastState,
    /// Volume (0.0 to 1.0).
    volume: f32,
}

impl ChromecastSender {
    pub fn new(device: CastDevice) -> Self {
        Self {
            device,
            state: ChromecastState::Disconnected,
            volume: 1.0,
        }
    }

    /// Connect to the Chromecast and launch the media receiver.
    pub fn connect(&mut self, _local_ip: Ipv4Addr) -> Result<(), String> {
        self.state = ChromecastState::Disconnected;
        Err("Chromecast CASTV2 LOAD is not implemented; Chromecast output is disabled".to_string())
    }

    /// Start streaming audio. Tells the Chromecast to play from our HTTP server.
    pub fn start_stream(&mut self, _sample_rate: u32, _channels: u16) -> Result<(), String> {
        self.state = ChromecastState::Disconnected;
        Err("Chromecast CASTV2 LOAD is not implemented; Chromecast output is disabled".to_string())
    }

    /// Write audio samples (interleaved f32) to the Chromecast stream.
    pub fn write_audio(&mut self, samples: &[f32]) -> Result<usize, String> {
        let _ = samples;
        Err("Not streaming".to_string())
    }

    /// Set volume (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Stop streaming and disconnect.
    pub fn disconnect(&mut self) {
        self.state = ChromecastState::Disconnected;
        log::info!("[Chromecast] Disconnected from {}", self.device.name);
    }

    /// Current state.
    pub fn state(&self) -> ChromecastState {
        self.state
    }

    /// The target device.
    pub fn device(&self) -> &CastDevice {
        &self.device
    }
}

impl Drop for ChromecastSender {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_device() -> CastDevice {
        CastDevice {
            device_type: crate::CastDeviceType::Chromecast,
            name: "Living Room".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 100),
            port: 8009,
            instance_name: String::new(),
            txt_records: HashMap::new(),
        }
    }

    #[test]
    fn test_chromecast_initial_state() {
        let sender = ChromecastSender::new(test_device());
        assert_eq!(sender.state(), ChromecastState::Disconnected);
        assert_eq!(sender.device().name, "Living Room");
    }

    #[test]
    fn test_chromecast_connect_is_gated_until_castv2_load_exists() {
        let mut sender = ChromecastSender::new(test_device());
        let err = sender.connect(Ipv4Addr::new(127, 0, 0, 1)).unwrap_err();
        assert!(err.contains("CASTV2 LOAD is not implemented"));
        assert_eq!(sender.state(), ChromecastState::Disconnected);
    }

    #[test]
    fn test_chromecast_start_stream_is_gated_until_castv2_load_exists() {
        let mut sender = ChromecastSender::new(test_device());
        let err = sender.start_stream(48_000, 2).unwrap_err();
        assert!(err.contains("CASTV2 LOAD is not implemented"));
        assert_eq!(sender.state(), ChromecastState::Disconnected);
    }

    #[test]
    fn test_write_without_streaming_fails() {
        let mut sender = ChromecastSender::new(test_device());
        let samples = vec![0.0f32; 100];
        assert!(sender.write_audio(&samples).is_err());
    }
}
