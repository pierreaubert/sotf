//! Cast device discovery and management.

use crate::app::App;
use crate::app::state::audio_device::CastDeviceInfo;

impl App {
    /// Start scanning for Cast devices (Chromecast + AirPlay) on the local network.
    /// Spawns a background thread; poll with `update_cast_discovery()`.
    pub fn start_cast_discovery(&mut self) {
        if self.audio_device_state.cast_discovery_running {
            return;
        }

        self.audio_device_state.cast_discovery_running = true;

        let (tx, rx) = std::sync::mpsc::channel();
        self.cast_discovery_receiver = Some(rx);

        std::thread::Builder::new()
            .name("cast-discovery".into())
            .spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                let devices = rt.block_on(async {
                    let timeout = std::time::Duration::from_secs(3);
                    match sotf_cast::CastDiscovery::discover_all(timeout).await {
                        Ok(devices) => devices
                            .into_iter()
                            .map(|d| CastDeviceInfo {
                                name: d.name.clone(),
                                device_type: match d.device_type {
                                    sotf_cast::CastDeviceType::AirPlay => "AirPlay".to_string(),
                                    sotf_cast::CastDeviceType::Chromecast => {
                                        "Chromecast".to_string()
                                    }
                                },
                                address: d.address.to_string(),
                                port: d.port,
                            })
                            .collect(),
                        Err(e) => {
                            log::warn!("Cast discovery failed: {e}");
                            Vec::new()
                        }
                    }
                });
                let _ = tx.send(devices);
            })
            .expect("spawn cast discovery thread");
    }

    /// Poll for Cast discovery results. Call from the UI update loop.
    pub fn update_cast_discovery(&mut self) {
        let rx = match &self.cast_discovery_receiver {
            Some(rx) => rx,
            None => return,
        };

        match rx.try_recv() {
            Ok(devices) => {
                log::info!("Cast discovery found {} devices", devices.len());
                self.audio_device_state.cast_devices = devices;
                self.audio_device_state.cast_discovery_running = false;
                self.cast_discovery_receiver = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.audio_device_state.cast_discovery_running = false;
                self.cast_discovery_receiver = None;
            }
        }
    }

    /// Select a Cast device for audio output.
    pub fn select_cast_device(&mut self, index: usize) {
        if index < self.audio_device_state.cast_devices.len() {
            self.audio_device_state.selected_cast_device = Some(index);
        }
    }

    /// Deselect Cast device, revert to local output.
    pub fn deselect_cast_device(&mut self) {
        self.audio_device_state.selected_cast_device = None;
    }
}
