use super::cpal_playback_state::CpalPlaybackState;
use super::misc::clamp_samples;
use std::sync::atomic::Ordering;

/// Apply volume and mute to f32 scratch buffer without clipping the float path.
#[inline(always)]
pub(super) fn apply_volume(
    scratch: &mut [f32],
    state: &CpalPlaybackState,
    channels: usize,
    sample_rate: u32,
) {
    let volume = f32::from_bits(state.volume.load(Ordering::Relaxed));
    let muted = state.muted.load(Ordering::Relaxed);
    let target = if muted { 0.0 } else { volume };
    state
        .volume_ramp
        .apply(scratch, channels, sample_rate, target);
}

#[inline(always)]
pub(super) fn apply_volume_clamp(
    scratch: &mut [f32],
    state: &CpalPlaybackState,
    channels: usize,
    sample_rate: u32,
) {
    apply_volume(scratch, state, channels, sample_rate);
    clamp_samples(scratch);
}
