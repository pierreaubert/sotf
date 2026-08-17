use crate::signals::*;

/// Absolute sample magnitude at/above which a captured sample counts as
/// clipped. The output stage hard-clamps to ±1.0, so anything reaching
/// ±0.999 rode the limiter (matches the generator-side `clip()` semantics).
pub const CLIP_THRESHOLD: f32 = 0.999;

/// Overall clipped-sample percentage above which a capture gets a warning.
pub const CLIP_WARN_PERCENT: f32 = 0.1;

/// Per-block clipped-sample percentage above which a capture is refused,
/// mirroring REW's abort rule (>30% clipped samples in a block).
pub const CLIP_ABORT_BLOCK_PERCENT: f32 = 30.0;

/// Block length used by [`analyze_clipping`] for the per-block percentage
/// (≈43 ms at 48 kHz).
pub const CLIP_BLOCK_SAMPLES: usize = 2048;

/// Summary of clipping observed in a captured buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipStats {
    /// Number of samples with `|s| >= CLIP_THRESHOLD`.
    pub clipped_samples: usize,
    /// Clipped samples as a percentage of the whole buffer.
    pub clip_percent: f32,
    /// Highest clipped-sample percentage observed in any single
    /// [`CLIP_BLOCK_SAMPLES`]-sample block.
    pub max_block_clip_percent: f32,
}

/// Count clipped samples in a captured buffer, overall and per block.
pub fn analyze_clipping(samples: &[f32]) -> ClipStats {
    if samples.is_empty() {
        return ClipStats {
            clipped_samples: 0,
            clip_percent: 0.0,
            max_block_clip_percent: 0.0,
        };
    }

    let mut clipped_samples = 0_usize;
    let mut max_block_clip_percent = 0.0_f32;
    for block in samples.chunks(CLIP_BLOCK_SAMPLES) {
        let block_clipped = block.iter().filter(|s| s.abs() >= CLIP_THRESHOLD).count();
        clipped_samples += block_clipped;
        let block_percent = block_clipped as f32 / block.len() as f32 * 100.0;
        max_block_clip_percent = max_block_clip_percent.max(block_percent);
    }

    ClipStats {
        clipped_samples,
        clip_percent: clipped_samples as f32 / samples.len() as f32 * 100.0,
        max_block_clip_percent,
    }
}

/// Inspect a captured buffer for clipping: logs a warning once clipping
/// exceeds [`CLIP_WARN_PERCENT`] overall and returns `Err` when any block
/// exceeds [`CLIP_ABORT_BLOCK_PERCENT`] (REW's abort rule).
#[cfg(not(target_os = "ios"))]
pub(super) fn check_capture_clipping(samples: &[f32], log_tag: &str) -> Result<(), String> {
    let stats = analyze_clipping(samples);
    if stats.max_block_clip_percent > CLIP_ABORT_BLOCK_PERCENT {
        return Err(format!(
            "[{log_tag}] Recording clipped hard: {:.1}% of samples at full scale in the worst block \
             ({} clipped samples, {:.2}% overall). Lower the output level or mic gain and re-run \
             the measurement.",
            stats.max_block_clip_percent, stats.clipped_samples, stats.clip_percent,
        ));
    }
    if stats.clip_percent > CLIP_WARN_PERCENT {
        log::warn!(
            "[{log_tag}] Recording clipped: {} samples at full scale ({:.2}% overall, worst block {:.1}%). \
             Consider lowering the output level or mic gain.",
            stats.clipped_samples,
            stats.clip_percent,
            stats.max_block_clip_percent,
        );
    }
    Ok(())
}

#[cfg(not(target_os = "ios"))]
pub(super) fn capture_capacity(
    sample_rate: u32,
    duration_secs: f64,
    extra_tail_secs: f64,
) -> usize {
    ((duration_secs + extra_tail_secs).max(1.0) * sample_rate as f64).ceil() as usize
}

#[cfg(not(target_os = "ios"))]
pub(super) fn drain_capture<T>(consumer: &mut rtrb::Consumer<T>, expected_len: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(expected_len);
    while let Ok(sample) = consumer.pop() {
        out.push(sample);
    }
    out
}

/// Wrap a low-level audio device/stream error with actionable guidance (C3).
///
/// cpal surfaces permission denials, busy devices and missing devices as
/// host-specific strings, so this classifies the error text and prepends
/// user-facing advice for the common failure modes. The original message is
/// always kept (appended in parentheses) for debugging; when no advice
/// applies, the result is just `"{context}: {err}"`, i.e. identical to the
/// previous formatting.
pub fn actionable_capture_error(context: &str, err: &dyn std::fmt::Display) -> String {
    let raw = err.to_string();
    let lower = raw.to_lowercase();
    let advice = if lower.contains("permission")
        || lower.contains("denied")
        || lower.contains("not authorized")
    {
        #[cfg(target_os = "macos")]
        {
            "Grant microphone permission in System Settings → Privacy & Security → Microphone, \
             then restart the app."
        }
        #[cfg(target_os = "ios")]
        {
            "Grant microphone permission in Settings → Privacy & Security → Microphone, \
             then restart the app."
        }
        #[cfg(target_os = "linux")]
        {
            "Check that your user is in the 'audio' group \
             (e.g. `sudo usermod -aG audio $USER`, then log out and back in)."
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
        {
            "Check the OS microphone privacy settings, then restart the app."
        }
    } else if lower.contains("busy") || lower.contains("in use") || lower.contains("exclusive") {
        "The device is busy — close other apps using it and try again."
    } else if lower.contains("not found")
        || lower.contains("no such device")
        || lower.contains("no default input device")
        || lower.contains("no default output device")
    {
        "Run with --list-devices (or the UI device picker) to see available devices."
    } else {
        ""
    };

    if advice.is_empty() {
        format!("{context}: {raw}")
    } else {
        format!("{context}: {advice} (original error: {raw})")
    }
}

/// Prepare a signal for playback with fades and padding
pub fn prepare_signal(signal: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    const FADE_MS: f32 = 20.0;
    const PADDING_MS: f32 = 250.0;

    prepare_signal_for_playback(signal, sample_rate, FADE_MS, PADDING_MS)
}

#[cfg(not(target_os = "ios"))]
pub(super) fn fill_measurement_output<T>(
    data: &mut [T],
    playback: &[f32],
    cursor: &std::sync::atomic::AtomicUsize,
) where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    use std::sync::atomic::Ordering as AtomicOrdering;

    let start = cursor.fetch_add(data.len(), AtomicOrdering::Relaxed);
    let available = playback.len().saturating_sub(start).min(data.len());
    for i in 0..available {
        data[i] = T::from_sample(playback[start + i].clamp(-1.0, 1.0));
    }
    if available < data.len() {
        data[available..].fill(T::from_sample(0.0));
    }
}

/// Parse comma-separated channel list (0-based indices)
pub fn parse_channel_list(s: &str) -> Result<Vec<u16>, String> {
    let mut channels = Vec::new();

    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let ch: u16 = part
            .parse()
            .map_err(|_| format!("Invalid channel number: {}", part))?;

        if channels.contains(&ch) {
            return Err(format!("Duplicate channel number: {}", ch));
        }

        channels.push(ch);
    }

    if channels.is_empty() {
        return Err("Channel list is empty".to_string());
    }

    Ok(channels)
}
