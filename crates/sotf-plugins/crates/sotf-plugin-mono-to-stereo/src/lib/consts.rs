pub(super) const FFT_SIZE: usize = 2048;

pub(super) const HOP_SIZE: usize = FFT_SIZE / 4; // 75% overlap

pub(super) const PARAM_SMOOTH_MS: f32 = 20.0;

/// Maximum Haas delay in samples at 192kHz (30ms * 192000 / 1000 = 5760)
/// Round up to next power of two for masking.
pub(super) const HAAS_DELAY_BUF_SIZE: usize = 8192;
