use super::cpal_playback_state::CpalPlaybackState;
use super::misc::clamp_samples;
use std::sync::atomic::Ordering;

/// Apply volume and mute to f32 scratch buffer without clipping the float path.
#[inline(always)]
pub(super) fn apply_volume(scratch: &mut [f32], state: &CpalPlaybackState) {
    let volume = f32::from_bits(state.volume.load(Ordering::Relaxed));
    let muted = state.muted.load(Ordering::Relaxed);

    if muted {
        scratch.fill(0.0);
    } else if (volume - 1.0).abs() > 0.001 {
        for sample in scratch.iter_mut() {
            *sample *= volume;
        }
    }
}

#[inline(always)]
pub(super) fn apply_volume_clamp(scratch: &mut [f32], state: &CpalPlaybackState) {
    apply_volume(scratch, state);
    clamp_samples(scratch);
}
