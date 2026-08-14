/// Sample capacity for the processing-to-playback ring.
///
/// `work_horizon_frames` is expressed in this stream's output-rate domain.
/// Keeping at least twice that horizon lets the existing half-ring silence
/// prefill reserve one complete horizon before the hardware starts consuming.
pub(in crate::engine) fn playback_buffer_capacity(
    sample_rate: u32,
    channels: usize,
    buffer_ms: u32,
    work_horizon_frames: usize,
) -> usize {
    let samples = sample_rate as u128 * buffer_ms as u128 * channels as u128;
    let configured = samples.div_ceil(1000);
    let horizon_floor = (work_horizon_frames.max(1) as u128)
        .saturating_mul(channels.max(1) as u128)
        .saturating_mul(2);
    configured.max(horizon_floor).min(usize::MAX as u128) as usize
}

pub(super) fn validate_playback_work_horizon(
    channels: usize,
    work_horizon_frames: usize,
) -> Result<(), String> {
    let required = (channels.max(1) as u128)
        .saturating_mul(work_horizon_frames.max(1) as u128)
        .saturating_mul(2);
    if required > super::super::MAX_ENGINE_PLAYBACK_BUFFER_SAMPLES as u128 {
        Err(format!(
            "playback work horizon requires {required} ring samples, exceeding the engine limit of {}",
            super::super::MAX_ENGINE_PLAYBACK_BUFFER_SAMPLES
        ))
    } else {
        Ok(())
    }
}

pub(super) fn convert_work_horizon_frames(
    frames: usize,
    source_rate: u32,
    target_rate: u32,
) -> usize {
    if source_rate == 0 {
        return frames.max(1);
    }
    ((frames.max(1) as u128)
        .saturating_mul(target_rate as u128)
        .div_ceil(source_rate as u128))
    .min(usize::MAX as u128) as usize
}

#[allow(
    clippy::too_many_arguments,
    reason = "recovery diagnostic: one argument per observed stream/callback counter"
)]
pub(super) fn playback_recovery_reason(
    current_stream_errors: u64,
    last_stream_error_count: &mut u64,
    current_callbacks: u64,
    last_callback_count: &mut u64,
    last_callback_check: &mut std::time::Instant,
    callback_stall_timeout: std::time::Duration,
    frames_received: u64,
    frames_written: u64,
    coreaudio_identity_reason: Option<String>,
) -> Option<String> {
    if current_stream_errors != *last_stream_error_count {
        *last_stream_error_count = current_stream_errors;
        Some(format!(
            "stream error reported by CoreAudio ({} total)",
            current_stream_errors
        ))
    } else if current_callbacks != *last_callback_count {
        *last_callback_count = current_callbacks;
        *last_callback_check = std::time::Instant::now();
        None
    } else if let Some(reason) = coreaudio_identity_reason {
        Some(reason)
    } else if last_callback_check.elapsed() > callback_stall_timeout && frames_received > 0 {
        Some(format!(
            "callbacks stalled after {} frames played",
            frames_written
        ))
    } else {
        None
    }
}
