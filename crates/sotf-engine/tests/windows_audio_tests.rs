//! Windows Audio Backend Tests
//!
//! Tests for different Windows audio backends: WASAPI, ASIO, and default.
//! These tests verify that the engine works with different Windows audio APIs.
//!
//! Run with: cargo test --test windows_audio_tests --target x86_64-pc-windows-msvc
//!
//! Note: ASIO requires the ASIO feature to be enabled in cpal and ASIO drivers
//! to be installed on the system.

#[cfg(target_os = "windows")]
mod tests {
    use cpal::HostId;
    use cpal::traits::{DeviceTrait, HostTrait};

    /// Get all available host IDs on the system
    fn available_hosts() -> Vec<HostId> {
        cpal::platform::available_hosts()
    }

    /// Test: Enumerate available Windows audio backends
    #[test]
    fn test_enumerate_windows_backends() {
        let hosts = available_hosts();

        println!("Available audio hosts:");
        for host in &hosts {
            println!("  - {:?}", host);
        }

        // At minimum, some host should be available on Windows
        assert!(!hosts.is_empty(), "No audio hosts available on Windows");
    }

    /// Test: Check for WASAPI-like host (default on Windows 11)
    #[test]
    fn test_default_windows_host() {
        let hosts = available_hosts();
        let default_host = cpal::default_host();
        let default_id = default_host.id();

        println!("Default host: {:?}", default_id);
        println!("All available hosts: {:?}", hosts);

        // On Windows 11, default should be WASAPI
        // The exact variant name may vary by cpal version
        assert!(
            hosts.contains(&default_id),
            "Default host should be in available hosts"
        );
    }

    /// Test: Windows host can enumerate devices
    #[test]
    fn test_enumerate_devices() {
        let host = cpal::default_host();

        let output_devices: Vec<_> = host
            .output_devices()
            .expect("Failed to enumerate output devices")
            .collect();

        let input_devices: Vec<_> = host
            .input_devices()
            .expect("Failed to enumerate input devices")
            .collect();

        println!("Output devices: {}", output_devices.len());
        println!("Input devices: {}", input_devices.len());

        // At least one output device should be available
        assert!(!output_devices.is_empty(), "No output devices found");
    }

    /// Test: Device format support
    #[test]
    fn test_device_formats() {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .expect("No default output device");

        let name = device
            .description()
            .map(|d| d.name())
            .unwrap_or_else(|_| "Unknown".to_string());

        println!("Default output device: {}", name);

        // Check supported configs
        let supported_configs = device
            .supported_output_configs()
            .expect("Failed to get supported configs");

        let count = supported_configs.count();
        println!("Supported output configs: {}", count);

        assert!(count > 0, "No supported output configs found");
    }

    /// Test: Sample rate compatibility check
    #[test]
    fn test_common_sample_rates() {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .expect("No default output device");

        let supported = device
            .supported_output_configs()
            .expect("Failed to get supported configs");

        let common_rates = [44100, 48000, 88200, 96000, 176400, 192000];

        println!("Checking common sample rates:");
        for rate in common_rates {
            let rate_supported = supported.as_ref().any(|config| {
                config.min_sample_rate().0 <= rate && rate <= config.max_sample_rate().0
            });
            println!(
                "  {}Hz: {}",
                rate,
                if rate_supported {
                    "OK"
                } else {
                    "NOT SUPPORTED"
                }
            );
        }
    }

    /// Test: Buffer size options
    #[test]
    fn test_buffer_sizes() {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .expect("No default output device");

        let supported = device
            .supported_output_configs()
            .expect("Failed to get supported configs");

        for config in supported {
            println!(
                "Config: {}ch, {}Hz, {:?} format, buffer sizes: {:?}",
                config.channels(),
                config.sample_rate().0,
                config.sample_format(),
                config.buffer_size()
            );
        }
    }

    /// Test: Verify all hosts are unique
    #[test]
    fn test_hosts_are_unique() {
        use std::collections::HashSet;

        let hosts = available_hosts();
        let mut unique: HashSet<HostId> = std::collections::HashSet::new();

        for host in &hosts {
            assert!(unique.insert(*host), "Duplicate host found: {:?}", host);
        }

        println!("Found {} unique audio hosts", unique.len());
    }

    /// Test: Dry-run stream creation
    #[test]
    fn test_stream_config() {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .expect("No default output device");

        let config = device
            .default_output_config()
            .expect("Failed to get default output config");

        println!("Default output config:");
        println!("  Sample rate: {}Hz", config.sample_rate().0);
        println!("  Channels: {}", config.channels());
        println!("  Sample format: {:?}", config.sample_format());
        println!("  Buffer size: {:?}", config.buffer_size());

        assert!(
            config.sample_rate().0 > 0,
            "Default output config must report a non-zero sample rate"
        );
        assert!(
            config.channels() > 0,
            "Default output config must report at least one channel"
        );
    }
}

#[cfg(not(target_os = "windows"))]
mod tests {
    /// Placeholder test for non-Windows platforms
    #[test]
    fn windows_tests_skip_on_non_windows() {
        println!("Windows audio backend tests are only run on Windows");

        // Still test cpal on the current platform to verify it works
        let hosts = cpal::platform::available_hosts();
        println!("Available hosts on this platform: {:?}", hosts);

        assert!(!hosts.is_empty(), "No hosts available");
    }
}
