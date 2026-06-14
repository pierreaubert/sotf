/// Resampler chunk size — the fixed input block size expected by rubato.
pub(super) const RESAMPLER_CHUNK_SIZE: usize = 1024;

pub(super) const PV_FFT_SIZE: usize = 2048;

pub(super) const PV_HOP_SIZE: usize = PV_FFT_SIZE / 4;

/// Smoothing time for correction_strength parameter changes (ms).
/// Prevents audible pitch jumps when tweaking correction strength live.
pub(super) const CORRECTION_STRENGTH_SMOOTH_MS: f32 = 50.0;
