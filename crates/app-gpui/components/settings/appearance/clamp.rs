#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Hour and minute inputs are rounded and clamped to their clock ranges"
)]
pub(super) fn clamp_hour(value: f64) -> u8 {
    value.round().clamp(0.0, 23.0) as u8
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Hour and minute inputs are rounded and clamped to their clock ranges"
)]
pub(super) fn clamp_minute(value: f64) -> u8 {
    value.round().clamp(0.0, 59.0) as u8
}
