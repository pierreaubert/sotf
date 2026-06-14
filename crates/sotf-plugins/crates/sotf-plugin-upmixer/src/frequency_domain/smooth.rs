use super::consts::DIALOGUE_SPATIAL_ATTACK_ALPHA;
use super::consts::DIALOGUE_SPATIAL_DEADBAND;
use super::consts::DIALOGUE_SPATIAL_MAX_FALL;
use super::consts::DIALOGUE_SPATIAL_MAX_RISE;
use super::consts::DIALOGUE_SPATIAL_RELEASE_ALPHA;
use super::consts::DIFFUSENESS_MAX_STEP;
use super::consts::DIFFUSENESS_SMOOTHING_ALPHA;

#[inline(always)]
pub(super) fn smooth_dialogue_spatial_control(previous: f32, target: f32) -> f32 {
    let target = target.clamp(0.0, 1.0);
    let diff = target - previous;
    if diff.abs() <= DIALOGUE_SPATIAL_DEADBAND {
        return previous;
    }

    let alpha = if diff > 0.0 {
        DIALOGUE_SPATIAL_ATTACK_ALPHA
    } else {
        DIALOGUE_SPATIAL_RELEASE_ALPHA
    };
    let smoothed = previous + alpha * diff;
    let limited_diff =
        (smoothed - previous).clamp(-DIALOGUE_SPATIAL_MAX_FALL, DIALOGUE_SPATIAL_MAX_RISE);
    (previous + limited_diff).clamp(0.0, 1.0)
}

#[inline(always)]
pub(super) fn smooth_diffuseness(previous: f32, target: f32, smoothing_scale: f32) -> f32 {
    let target = target.clamp(0.0, 1.0);
    let alpha = (DIFFUSENESS_SMOOTHING_ALPHA * smoothing_scale).max(0.035);
    let max_step = (DIFFUSENESS_MAX_STEP * smoothing_scale).max(0.025);
    let smoothed = previous + alpha * (target - previous);
    let limited_diff = (smoothed - previous).clamp(-max_step, max_step);
    (previous + limited_diff).clamp(0.0, 1.0)
}
