use super::misc::clamp_samples;
use super::playback_state::PlaybackState;
use std::sync::atomic::Ordering;

/// Apply volume and mute to f32 scratch buffer without clipping the float path.
#[inline(always)]
pub(super) fn apply_volume(
    scratch: &mut [f32],
    state: &PlaybackState,
    channels: usize,
    sample_rate: u32,
) {
    let volume = f32::from_bits(state.volume.load(Ordering::Relaxed));
    let muted = state.muted.load(Ordering::Relaxed);
    let target = if muted { 0.0 } else { volume };
    state
        .volume_ramp
        .apply(scratch, channels, sample_rate, target);

    accumulate_output_meter(scratch, state);
}

/// Apply volume/mute and clamp for integer hardware formats.
#[inline(always)]
pub(in crate::engine) fn apply_volume_clamp(
    scratch: &mut [f32],
    state: &PlaybackState,
    channels: usize,
    sample_rate: u32,
) {
    apply_volume(scratch, state, channels, sample_rate);
    clamp_samples(scratch);
}

/// Accumulate post-volume, pre-clamp output telemetry without allocation or locking.
#[inline(always)]
fn accumulate_output_meter(samples: &[f32], state: &PlaybackState) {
    let mut peak = 0.0f32;
    let mut clipped = 0u64;

    for &sample in samples {
        let magnitude = sample.abs();
        if magnitude.is_finite() {
            peak = peak.max(magnitude);
            clipped += u64::from(magnitude > 1.0);
        } else {
            clipped += 1;
        }
    }

    // Positive finite f32 values preserve numeric ordering in their bit pattern.
    state
        .output_peak_bits
        .fetch_max(peak.to_bits(), Ordering::Relaxed);
    if clipped > 0 {
        state
            .clipped_sample_count
            .fetch_add(clipped, Ordering::Relaxed);
    }
}
