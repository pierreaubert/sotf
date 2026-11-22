/// Get channel count for a speaker configuration
pub(super) fn get_speaker_config_channels(config: &str) -> Result<usize, String> {
    let supported = sotf_plugins::param_specs::upmixer::SPEAKER_CONFIGS;
    supported
        .contains(&config)
        .then(|| sotf_plugins::speaker_config::get_speaker_config(config))
        .flatten()
        .map(|layout| layout.total_channels)
        .ok_or_else(|| {
            format!(
                "Invalid speaker configuration '{config}'. Valid options: {}",
                supported.join(", ")
            )
        })
}

#[cfg(test)]
mod speaker_config_tests {
    #[test]
    fn cli_layouts_use_the_upmixer_speaker_catalog() {
        for id in sotf_plugins::param_specs::upmixer::SPEAKER_CONFIGS {
            let layout = sotf_plugins::speaker_config::get_speaker_config(id).unwrap();
            assert_eq!(
                super::get_speaker_config_channels(id),
                Ok(layout.total_channels)
            );
        }
        assert_eq!(super::get_speaker_config_channels("5.1.6"), Ok(12));
        assert!(super::get_speaker_config_channels("2.1").is_err());
    }
}

pub(super) fn list_devices() -> Result<(), String> {
    let msg = "Enumerating audio devices...\n";
    log::info!("{msg}");
    println!("{msg}");

    let devices = sotf_audio::devices::get_audio_devices()
        .map_err(|e| format!("Failed to get devices: {e}"))?;

    // Print input devices
    if let Some(input_devices) = devices.get("input") {
        let title = "Input Devices:";
        let separator = "=".repeat(80);
        log::info!("{title}");
        log::info!("{separator}");
        println!("{title}");
        println!("{separator}");

        for (idx, device) in input_devices.iter().enumerate() {
            let default_marker = if device.is_default { " (Default)" } else { "" };

            if let Some(config) = &device.default_config {
                let rate_range = if device.available_sample_rates.is_empty() {
                    "unknown".to_string()
                } else if device.available_sample_rates.len() == 1 {
                    format!("{} Hz", device.available_sample_rates[0])
                } else {
                    {
                        let rates = &device.available_sample_rates;
                        format!("{}-{} Hz", rates[0], rates[rates.len() - 1])
                    }
                };

                let line = format!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx + 1,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
                log::info!("{line}");
                println!("{line}");
            } else {
                let line = format!("  [{}] {}{}", idx + 1, device.name, default_marker);
                log::info!("{line}");
                println!("{line}");
            }
        }
        log::info!("");
        println!();
    }

    // Print output devices
    if let Some(output_devices) = devices.get("output") {
        let title = "Output Devices:";
        let separator = "=".repeat(80);
        log::info!("{title}");
        log::info!("{separator}");
        println!("{title}");
        println!("{separator}");

        for (idx, device) in output_devices.iter().enumerate() {
            let default_marker = if device.is_default { " (Default)" } else { "" };

            if let Some(config) = &device.default_config {
                let rate_range = if device.available_sample_rates.is_empty() {
                    "unknown".to_string()
                } else if device.available_sample_rates.len() == 1 {
                    format!("{} Hz", device.available_sample_rates[0])
                } else {
                    {
                        let rates = &device.available_sample_rates;
                        format!("{}-{} Hz", rates[0], rates[rates.len() - 1])
                    }
                };

                let line = format!(
                    "  [{}] {}{} - {} ch, {} (current: {} Hz), {}",
                    idx + 1,
                    device.name,
                    default_marker,
                    config.channels,
                    rate_range,
                    config.sample_rate,
                    config.sample_format
                );
                log::info!("{line}");
                println!("{line}");
            } else {
                let line = format!("  [{}] {}{}", idx + 1, device.name, default_marker);
                log::info!("{line}");
                println!("{line}");
            }
        }
        log::info!("");
        println!();
    }

    Ok(())
}
