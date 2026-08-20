//! Helpers for audio-engine integration tests.
//!
//! Requires the `engine` feature.

use sotf_audio::engine::EngineConfig;
use std::sync::OnceLock;

/// Virtual audio device names to try (in order of preference)
const VIRTUAL_DEVICES: &[&str] = &[
    "BlackHole 2ch",
    "BlackHole 16ch",
    "BlackHole 64ch",
    "SotF Virtual Audio",
];

/// Cached virtual device name (checked once per test run)
static VIRTUAL_DEVICE: OnceLock<Option<String>> = OnceLock::new();

/// Find an available virtual audio device.
///
/// Checks `AEQ_E2E_DEVICE` env var first, then auto-detects BlackHole or SotF HAL driver.
pub fn find_virtual_device() -> Option<String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let devices: Vec<String> = host
        .output_devices()
        .map(|devices| {
            devices
                .filter_map(|device| {
                    device
                        .description()
                        .ok()
                        .map(|description| description.name().to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    if let Ok(device) = std::env::var("AEQ_E2E_DEVICE")
        && !device.is_empty()
    {
        if let Some(actual_name) = devices
            .iter()
            .find(|name| *name == &device || name.contains(&device))
        {
            return Some(actual_name.clone());
        }

        eprintln!(
            "AEQ_E2E_DEVICE='{}' is not available to cpal; available output devices: {}",
            device,
            if devices.is_empty() {
                "<none>".to_string()
            } else {
                devices.join(", ")
            }
        );
        return None;
    }

    for virtual_name in VIRTUAL_DEVICES {
        for name in &devices {
            if name.contains(virtual_name) {
                return Some(name.clone());
            }
        }
    }

    None
}

/// Get the cached virtual device name, or `None` if unavailable.
pub fn get_virtual_device() -> Option<String> {
    VIRTUAL_DEVICE.get_or_init(find_virtual_device).clone()
}

/// Panic if no virtual device is available.
pub fn require_virtual_device() -> String {
    get_virtual_device().expect(
        "\n\n\
        ╔═══════════════════════════════════════════════════════════════════════╗\n\
        ║  AUDIO ENGINE TESTS REQUIRE A VIRTUAL AUDIO DEVICE                    ║\n\
        ╠═══════════════════════════════════════════════════════════════════════╣\n\
        ║  No virtual audio device found (BlackHole or SotF HAL).               ║\n\
        ║                                                                       ║\n\
        ║  Tests use virtual devices to avoid playing sound on real speakers.   ║\n\
        ║                                                                       ║\n\
        ║  Options:                                                             ║\n\
        ║  1. Install BlackHole: brew install blackhole-2ch                     ║\n\
        ║     or from: https://existential.audio/blackhole/                     ║\n\
        ║  2. Install the SotF HAL driver                                       ║\n\
        ║  3. Set AEQ_E2E_DEVICE='Your Device Name' to use a specific device   ║\n\
        ╚═══════════════════════════════════════════════════════════════════════╝\n\n",
    )
}

/// Create an `EngineConfig` configured for testing with a virtual audio device.
pub fn test_engine_config() -> EngineConfig {
    EngineConfig {
        output_device: Some(require_virtual_device()),
        allow_virtual_output: true,
        ..Default::default()
    }
}

/// Create an `EngineConfig` with specific settings, using a virtual audio device.
pub fn test_engine_config_with<F>(configure: F) -> EngineConfig
where
    F: FnOnce(&mut EngineConfig),
{
    let mut config = test_engine_config();
    configure(&mut config);
    config
}

/// Skip the current test if no virtual audio device is available.
#[macro_export]
macro_rules! skip_without_device {
    () => {
        match $crate::engine::get_virtual_device() {
            Some(_) => {}
            None => {
                eprintln!(
                    "SKIPPED: {} — no virtual audio device found (install BlackHole or set AEQ_E2E_DEVICE)",
                    module_path!()
                );
                return;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // This test just ensures the helper compiles and runs without panicking
    // when no device is present.
    #[test]
    fn get_virtual_device_does_not_panic() {
        let _ = get_virtual_device();
    }

    #[test]
    fn skip_without_device_returns_only_when_device_is_missing() {
        fn guarded_test(reached_body: &mut bool) {
            crate::skip_without_device!();
            *reached_body = true;
        }

        let mut reached_body = false;
        guarded_test(&mut reached_body);

        assert_eq!(reached_body, get_virtual_device().is_some());
    }
}
