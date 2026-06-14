/// Head radius in meters (typical adult)
pub(super) const HEAD_RADIUS_M: f32 = 0.0875;

/// Speed of sound in m/s
pub(super) const SPEED_OF_SOUND: f32 = 343.0;

/// Compute per-ear ITD delays (ms) from yaw angle (degrees) and static offset.
///
/// Returns `(delay_l, delay_r)` where `delay_l` is the delay on the L→R crossfeed path
/// and `delay_r` is the delay on the R→L crossfeed path.
///
/// Acoustic model: the crossfeed path for the ear *farther* from the source gets the
/// longer delay.  With positive yaw (head turned right) the left ear is farther, so
/// the L→R path (carrying left-channel signal to the right ear) is longer.
///
/// `base = static_itd_ms / 2` so that when yaw = 0 both paths carry equal delay
/// summing to `static_itd_ms`.
pub(super) fn compute_differential_itd_ms(head_yaw_deg: f32, static_itd_ms: f32) -> (f32, f32) {
    let yaw_rad = head_yaw_deg * std::f32::consts::PI / 180.0;
    let dynamic_ms = HEAD_RADIUS_M * yaw_rad.sin() / SPEED_OF_SOUND * 1000.0;
    let base = static_itd_ms * 0.5;
    // Positive yaw → left ear farther → longer L→R crossfeed delay
    let delay_l = (base + dynamic_ms).clamp(0.0, 1.0);
    let delay_r = (base - dynamic_ms).clamp(0.0, 1.0);
    (delay_l, delay_r)
}
