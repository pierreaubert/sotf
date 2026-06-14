#![allow(dead_code)]
/// Default smoothing time in ms for mute/solo/dim transitions (~5ms fade to avoid clicks)
pub(super) const DEFAULT_FADE_MS: f32 = 5.0;

/// Default dim gain in dB (-20dB)
pub(super) const DEFAULT_DIM_GAIN_DB: f32 = -20.0;
