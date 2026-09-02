use std::collections::HashMap;
use std::time::{Duration, Instant};

use cpal::traits::DeviceTrait;
use serde_json::Value;

use super::misc::list_audio_devices;

const DEVICE_CACHE_TTL: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct Cached<T> {
    observed_at: Instant,
    value: T,
}

impl<T> Cached<T> {
    fn is_fresh(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.observed_at) <= DEVICE_CACHE_TTL
    }
}

/// Bounded daemon-owned view of CoreAudio/CPAL device capabilities.
///
/// CoreAudio enumeration and `supported_output_configs` probing are
/// synchronous and can each take hundreds of milliseconds. ConfigBar polls
/// device recovery every second, and RoomEQ validation used to repeat the
/// same probe for every file. This registry coalesces those reads while a
/// short TTL guarantees hot-plug changes become visible without preserving
/// stale capabilities indefinitely.
#[derive(Debug, Default)]
pub(super) struct DeviceRegistry {
    generation: u64,
    devices: Option<Cached<Vec<Value>>>,
    max_output_channels: HashMap<(String, u32), Cached<usize>>,
}

impl DeviceRegistry {
    pub(super) fn list_devices(&mut self) -> Result<(u64, Vec<Value>), String> {
        let now = Instant::now();
        if let Some(cached) = &self.devices
            && cached.is_fresh(now)
        {
            return Ok((self.generation, cached.value.clone()));
        }

        let devices = list_audio_devices()?;
        self.generation = self.generation.saturating_add(1);
        self.devices = Some(Cached {
            observed_at: now,
            value: devices.clone(),
        });
        Ok((self.generation, devices))
    }

    pub(super) fn max_output_channels(
        &mut self,
        selected_device: &str,
        sample_rate: u32,
    ) -> Result<usize, String> {
        let now = Instant::now();
        let key = (selected_device.to_string(), sample_rate);
        if let Some(cached) = self.max_output_channels.get(&key)
            && cached.is_fresh(now)
        {
            return Ok(cached.value);
        }

        let host = sotf_audio::devices::get_host_for_device(Some(selected_device));
        let device_name = sotf_audio::devices::strip_asio_prefix(selected_device);
        let device = sotf_audio::devices::find_device(&host, device_name, false)
            .map_err(|error| format!("Cannot inspect output device '{device_name}': {error}"))?;
        let max_channels = device
            .supported_output_configs()
            .map_err(|error| format!("Cannot inspect output formats for '{device_name}': {error}"))?
            .filter(|config| {
                config.min_sample_rate() <= sample_rate && sample_rate <= config.max_sample_rate()
            })
            .map(|config| usize::from(config.channels()))
            .max()
            .or_else(|| {
                device
                    .default_output_config()
                    .ok()
                    .map(|config| usize::from(config.channels()))
            })
            .ok_or_else(|| {
                format!("Output device '{device_name}' does not report a usable channel layout")
            })?;

        self.max_output_channels.insert(
            key,
            Cached {
                observed_at: now,
                value: max_channels,
            },
        );
        Ok(max_channels)
    }

    pub(super) fn invalidate(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.devices = None;
        self.max_output_channels.clear();
    }

    #[cfg(test)]
    pub(super) fn seed_max_output_channels(
        &mut self,
        selected_device: &str,
        sample_rate: u32,
        channels: usize,
    ) {
        self.max_output_channels.insert(
            (selected_device.to_string(), sample_rate),
            Cached {
                observed_at: Instant::now(),
                value: channels,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_observation_expires_at_the_device_ttl() {
        let now = Instant::now();
        let fresh = Cached {
            observed_at: now,
            value: 2,
        };
        let stale = Cached {
            observed_at: now - DEVICE_CACHE_TTL - Duration::from_millis(1),
            value: 2,
        };

        assert!(fresh.is_fresh(now));
        assert!(!stale.is_fresh(now));
    }
}
