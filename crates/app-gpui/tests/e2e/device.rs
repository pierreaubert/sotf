//! Device detection helpers for GPUI E2E tests.
//!
//! CPAL device enumeration and default-device queries can block on headless
//! machines (e.g. waiting for a permission dialog or a stuck virtual driver).
//! All helpers here run the CPAL work on a temporary thread and return `None`
//! if it does not complete within a short timeout.

use cpal::traits::{DeviceTrait, HostTrait};
use std::time::Duration;

const VIRTUAL_OUTPUT_CANDIDATES: &[&str] = &[
    "BlackHole 2ch",
    "BlackHole 16ch",
    "BlackHole 64ch",
    "SotF Virtual Audio",
];

const LOOPBACK_CANDIDATES: &[&str] = &[
    "BlackHole",
    "Loopback",
    "VB-Cable",
    "Cable",
    "Null",
    "Dummy",
    "Stereo Mix",
];

/// Find a virtual output device suitable for E2E playback.
///
/// Honors `AEQ_E2E_DEVICE` if it names a device visible to CPAL, otherwise
/// falls back to the known virtual-device candidates. Returns `None` if the
/// lookup takes longer than `timeout` or no suitable device is found.
pub fn find_virtual_output_device() -> Option<String> {
    call_with_timeout(Duration::from_secs(3), || {
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

        if let Ok(requested) = std::env::var("AEQ_E2E_DEVICE")
            && !requested.is_empty()
        {
            if let Some(name) = devices
                .iter()
                .find(|name| *name == &requested || name.contains(&requested))
            {
                return Some(name.clone());
            }
        }

        VIRTUAL_OUTPUT_CANDIDATES
            .iter()
            .find_map(|candidate| {
                devices
                    .iter()
                    .find(|name| name.contains(candidate))
                    .cloned()
            })
    })
    .unwrap_or(None)
}

/// Find a loopback device that appears as both an input and an output.
///
/// Returns `None` if the lookup takes longer than `timeout` or no suitable
/// device is found.
pub fn find_loopback_device() -> Option<String> {
    call_with_timeout(Duration::from_secs(3), || {
        let host = cpal::default_host();
        let output_names: Vec<String> = host
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

        let input_names: Vec<String> = host
            .input_devices()
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

        for candidate in LOOPBACK_CANDIDATES {
            let in_match = input_names.iter().find(|name| name.contains(candidate));
            let out_match = output_names.iter().find(|name| name.contains(candidate));
            if let (Some(in_name), Some(_)) = (in_match, out_match) {
                return Some(in_name.clone());
            }
        }

        None
    })
    .unwrap_or(None)
}

/// Run `f` on a temporary thread and return its result, or `None` on timeout.
fn call_with_timeout<T: Send + 'static, F: FnOnce() -> T + Send + 'static>(
    timeout: Duration,
    f: F,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).ok()
}
