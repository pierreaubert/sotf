/// List ASIO devices available on the system.
///
/// Returns device names prefixed with "ASIO:" for use with `get_host_for_device`.
/// Returns an empty Vec if ASIO is not available.
#[cfg(all(target_os = "windows", feature = "asio"))]
pub fn list_asio_devices() -> Vec<String> {
    match cpal::host_from_id(cpal::HostId::Asio) {
        Ok(host) => {
            let mut devices = Vec::new();
            if let Ok(output_devices) = host.output_devices() {
                for device in output_devices {
                    if let Ok(desc) = device.description() {
                        devices.push(format!("{}{}", ASIO_DEVICE_PREFIX, desc.name()));
                    }
                }
            }
            devices
        }
        Err(e) => {
            log::debug!("[AUDIO] ASIO not available: {}", e);
            Vec::new()
        }
    }
}

/// List ASIO devices (stub when ASIO is not available).
#[cfg(not(all(target_os = "windows", feature = "asio")))]
pub fn list_asio_devices() -> Vec<String> {
    Vec::new()
}
