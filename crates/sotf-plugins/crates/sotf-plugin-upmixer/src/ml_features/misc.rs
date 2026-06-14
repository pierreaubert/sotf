/// Convert mel scale back to Hz.
#[inline]
pub(super) fn mel_to_hz(m: f32) -> f32 {
    700.0 * (10.0_f32.powf(m / 2595.0) - 1.0)
}
