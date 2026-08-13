pub(super) const PV_FFT_SIZE: usize = 2048;

pub(super) const PV_HOP_SIZE: usize = PV_FFT_SIZE / 4;

/// Zero history loaded before the first programme sample. This gives the first
/// real sample all four Hann/WOLA contributions instead of placing it at the
/// zero-valued edge of the first analysis window.
pub(super) const PV_PREFILL_FRAMES: usize = PV_FFT_SIZE - PV_HOP_SIZE;

/// Fixed end-to-end delay of the causal prefilled WOLA pitch shifter. An input
/// sample reaches the same synthesis-window position after `FFT_SIZE - 1`
/// output samples, independent of host callback partitioning.
pub(super) const PV_LATENCY_FRAMES: usize = PV_FFT_SIZE - 1;

/// Smoothing time for correction_strength parameter changes (ms).
/// Prevents audible pitch jumps when tweaking correction strength live.
pub(super) const CORRECTION_STRENGTH_SMOOTH_MS: f32 = 50.0;
