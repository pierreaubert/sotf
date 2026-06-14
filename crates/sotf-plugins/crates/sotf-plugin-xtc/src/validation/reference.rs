use super::super::filters::{SPEED_OF_SOUND, head_shadowing_woodworth};
use std::f32::consts::PI;

/// Reference ITD using Woodworth formula.
///
/// ITD = (r/c) * (θ + sin(θ)) for spherical head model.
/// Returns ITD in milliseconds.
#[inline]
pub fn reference_itd_ms(speaker_angle_deg: f32, head_radius_m: f32) -> f32 {
    let theta = speaker_angle_deg * PI / 180.0;
    let itd_seconds = (head_radius_m / SPEED_OF_SOUND) * (theta + theta.sin());
    itd_seconds * 1000.0
}

/// Reference ILD in dB from head shadowing model.
#[inline]
pub fn reference_ild_db(freq_hz: f32, source_angle_deg: f32, head_radius_m: f32) -> f32 {
    let shadow_angle = (90.0 + source_angle_deg).min(180.0);
    let shadow = head_shadowing_woodworth(freq_hz, shadow_angle * PI / 180.0, head_radius_m);

    if shadow < 1e-6 {
        return 60.0;
    }

    20.0 * (1.0 / shadow).log10()
}
