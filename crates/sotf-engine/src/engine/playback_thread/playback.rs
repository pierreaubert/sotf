pub(super) fn playback_buffer_capacity(sample_rate: u32, channels: usize, buffer_ms: u32) -> usize {
    let samples = sample_rate as u128 * buffer_ms as u128 * channels as u128;
    samples.div_ceil(1000).min(usize::MAX as u128) as usize
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
