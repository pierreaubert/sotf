/// Maximum number of channels supported for pre-allocated buffers.
pub(super) const MAX_CHANNELS: usize = 32;

/// Default pre-allocation size in samples. Initialization guarantees at least
/// 16,384 frames per configured channel; larger callbacks fail without growing
/// on the audio thread.
pub(super) const DEFAULT_BUF_SIZE: usize = 96000;
pub(super) const MAX_BLOCK_FRAMES: usize = 65_536;

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

/// Explicit asymmetric, memoryless saturation family.
///
/// This is a DC-centred, rail-normalized, bias-shifted tanh curve rather than a
/// physical diode or triode circuit model. `tone` maps to a fixed bias in
/// `[0.08, 0.40]`; the unequal positive and negative small-signal slopes create
/// controlled even harmonics. Both rails converge to exactly +/-1 and zero
/// input maps to zero, leaving any programme-dependent DC for the optional DC
/// blocker to remove.
#[inline(always)]
pub(super) fn asymmetric(x: f32, drive: f32, tone: f32) -> f32 {
    let bias = 0.08 + 0.16 * (tone - 1.0).clamp(0.0, 2.0);
    let bias_tanh = bias.tanh();
    let centered = (x * drive + bias).tanh() - bias_tanh;
    if centered >= 0.0 {
        centered / (1.0 - bias_tanh)
    } else {
        centered / (1.0 + bias_tanh)
    }
}
