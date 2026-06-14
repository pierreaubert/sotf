/// Maximum number of channels supported for pre-allocated buffers.
pub(super) const MAX_CHANNELS: usize = 32;

/// Default pre-allocation size in samples (frames * channels).
/// Covers 48000 frames * 2 channels = 96000. Grown in process_in_place if needed.
pub(super) const DEFAULT_BUF_SIZE: usize = 96000;

/// Soft clip: tanh(input * drive) / tanh(drive)
#[inline(always)]
pub(super) fn soft_clip(x: f32, drive: f32) -> f32 {
    let driven = x * drive;
    let tanh_drive = drive.tanh();
    if tanh_drive < 1e-6 {
        x
    } else {
        driven.tanh() / tanh_drive
    }
}

/// Tube: symmetric polynomial-like saturation x / (1 + |x|^n).
/// f(-x) = -f(x), so it is an odd function. The exponent `n` (tone) controls
/// the character of the saturation knee but does NOT add even harmonics.
#[inline(always)]
pub(super) fn tube(x: f32, drive: f32, n: f32) -> f32 {
    let driven = x * drive;
    driven / (1.0 + driven.abs().powf(n))
}

/// Tape-style exponential saturation (memoryless sigmoid, not true hysteresis).
#[inline(always)]
pub(super) fn tape(x: f32, drive: f32) -> f32 {
    let driven = x * drive;
    driven.signum() * (1.0 - (-driven.abs() * 2.0).exp()) * 0.5
}
