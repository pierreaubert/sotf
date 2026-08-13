use super::super::ThreadEvent;
use crate::OutputAccessStatus;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, SampleFormat};
use rtrb::{CopyToUninit, Producer, chunks::WriteChunkUninit};
use std::sync::mpsc::SyncSender;

pub(super) const SPIN_MS_RINGBUFFER: u64 = 5;

/// Max input channels for the stack-allocated downmix coefficient arrays.
pub(super) const MAX_DOWNMIX_CH: usize = 32;

/// Bulk-copy a slice into a ring buffer chunk using memcpy instead of per-element iteration.
/// For 96K f32 samples this is ~2× faster than `fill_from_iter`.
pub(in crate::engine) fn write_chunk_bulk(mut chunk: WriteChunkUninit<'_, f32>, data: &[f32]) {
    let (first, second) = chunk.as_mut_slices();
    let first_len = first.len().min(data.len());
    data[..first_len].copy_to_uninit(&mut first[..first_len]);
    let remaining = data.len() - first_len;
    if remaining > 0 {
        let second_len = second.len().min(remaining);
        data[first_len..first_len + second_len].copy_to_uninit(&mut second[..second_len]);
    }
    // Safety: we've initialized exactly data.len() elements via copy_to_uninit (memcpy).
    unsafe { chunk.commit(data.len()) };
}

pub(super) fn send_playback_event(
    event_tx: &crossbeam::channel::Sender<ThreadEvent>,
    event: ThreadEvent,
    context: &str,
) {
    if let Err(e) = event_tx.try_send(event) {
        crate::rate_limited_log!(
            trace,
            5,
            "[Playback Thread] Dropped event in {}: {}",
            context,
            e
        );
    }
}

pub(super) fn recycle_frame_data(recycle_tx: &SyncSender<Vec<f32>>, data: Vec<f32>, context: &str) {
    if let Err(e) = recycle_tx.try_send(data) {
        crate::rate_limited_log!(
            trace,
            5,
            "[Playback Thread] Dropped recycled frame buffer in {}: {}",
            context,
            e
        );
    }
}

pub(super) fn is_virtual_output_device_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("sotf")
        || lower.contains("blackhole")
        || lower.contains("zoomaudio")
        || lower.contains("loopback")
        || lower.contains("virtual")
        || lower.contains("soundflower")
        || lower.contains("background music")
        || lower.contains("audio bridge")
        || crate::devices::is_null_device(name)
}

/// Push `samples` zeros into the producer to give the cpal callback a cushion
/// before real audio arrives. Silently truncates if the ring has less free
/// space (newly-created ring is fully empty so this only happens after a
/// rebuild that races with cpal startup).
pub(super) fn prefill_silence(producer: &mut Producer<f32>, samples: usize) {
    let to_write = samples.min(producer.slots());
    if to_write == 0 {
        return;
    }
    let Ok(mut chunk) = producer.write_chunk_uninit(to_write) else {
        return;
    };
    let (first, second) = chunk.as_mut_slices();
    for slot in first.iter_mut() {
        slot.write(0.0);
    }
    for slot in second.iter_mut() {
        slot.write(0.0);
    }
    // Safety: we initialized exactly `to_write` elements across the two slices.
    unsafe { chunk.commit(to_write) };
}

pub(super) fn select_playback_device(
    host: &cpal::Host,
    output_device: Option<&str>,
    allow_virtual_output: bool,
) -> Result<Device, String> {
    let get_name = |d: &Device| -> String {
        d.description()
            .map(|desc| desc.name().to_string())
            .unwrap_or_else(|_| "Unknown".to_string())
    };

    let find_physical_output = || -> Result<Device, String> {
        let devices = host.output_devices().map_err(|e| e.to_string())?;
        let physical = devices.into_iter().find(|d| {
            let name = get_name(d);
            !is_virtual_output_device_name(&name)
        });

        if let Some(dev) = physical {
            log::info!(
                "[Playback Thread] Using fallback physical device: {}",
                get_name(&dev)
            );
            Ok(dev)
        } else {
            Err("No physical output device found".to_string())
        }
    };

    if let Some(device_identifier) = output_device {
        log::debug!(
            "[Playback Thread] Looking for device: '{}'",
            device_identifier
        );

        if is_virtual_output_device_name(device_identifier) && !allow_virtual_output {
            log::info!(
                "[Playback Thread] Explicit virtual output device '{}' requested; honoring selection",
                device_identifier
            );
        }

        match crate::devices::find_device(host, device_identifier, false) {
            Ok(dev) => {
                log::debug!("[Playback Thread] Using device: '{}'", get_name(&dev));
                Ok(dev)
            }
            Err(e) => {
                log::warn!(
                    "[Playback Thread] Explicit output device '{}' was not found: {}",
                    device_identifier,
                    e
                );
                Err(format!(
                    "Selected output device '{}' is not available: {}",
                    device_identifier, e
                ))
            }
        }
    } else if !allow_virtual_output {
        // In systemwide mode the macOS default output is the SotF virtual
        // device. Do not open it even transiently: CoreAudio can keep the
        // daemon registered as an active virtual-device client, which starves
        // the app-audio ingress path.
        find_physical_output()
    } else {
        let default_dev = host
            .default_output_device()
            .ok_or("No output device available")?;
        Ok(default_dev)
    }
}

#[cfg(target_os = "macos")]
pub(super) fn set_output_access_status(
    event_tx: &crossbeam::channel::Sender<ThreadEvent>,
    status: &mut OutputAccessStatus,
    new_status: OutputAccessStatus,
    context: &str,
) {
    if *status == new_status {
        return;
    }
    *status = new_status;
    send_playback_event(
        event_tx,
        ThreadEvent::PlaybackOutputAccessChanged(new_status),
        context,
    );
}

pub(super) fn initial_buffer_size(
    status: OutputAccessStatus,
    frame_size: usize,
) -> cpal::BufferSize {
    if status == OutputAccessStatus::ExclusiveActive {
        cpal::BufferSize::Fixed(frame_size.clamp(1, u32::MAX as usize) as u32)
    } else {
        cpal::BufferSize::Default
    }
}

pub(super) fn fallback_output_format(
    default_format_and_channels: Option<(SampleFormat, u16)>,
    requested_channels: u16,
) -> (SampleFormat, u16) {
    default_format_and_channels.unwrap_or((SampleFormat::F32, requested_channels))
}

#[inline(always)]
pub(super) fn clamp_samples(scratch: &mut [f32]) {
    for sample in scratch.iter_mut() {
        *sample = sample.clamp(-1.0, 1.0);
    }
}
