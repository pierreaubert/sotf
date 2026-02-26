pub fn find_device(name_part: &str, input: bool) -> Option<(cpal::Device, cpal::SupportedStreamConfig)> {
    let host = cpal::default_host();
    let devices = if input { host.input_devices().ok()? } else { host.output_devices().ok()? };
    for device in devices {
        if let Ok(name) = device.name() {
            if name.contains(name_part) {
                if input {
                    if let Ok(configs) = device.supported_input_configs() {
                        for config in configs {
                            if config.channels() >= 2 { return Some((device, config.with_max_sample_rate())); }
                        }
                    }
                } else {
                    if let Ok(configs) = device.supported_output_configs() {
                        for config in configs {
                            if config.channels() >= 2 { return Some((device, config.with_max_sample_rate())); }
                        }
                    }
                }
            }
        }
    }
    None
}

