pub(super) const FFT_SIZE: usize = 2048;

pub(super) const HOP_SIZE: usize = FFT_SIZE / 2;

pub(super) const PARAM_SMOOTH_MS: f32 = 20.0;

#[cfg(test)]
pub(super) const ALLPASS_FC_HZ: [f32; 2] = [100.0, 132.0];
