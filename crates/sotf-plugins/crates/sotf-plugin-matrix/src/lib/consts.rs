/// Smoothing time in ms for gain coefficient transitions (~5ms to avoid clicks)
pub(super) const GAIN_SMOOTH_MS: f32 = 5.0;

/// Known routing presets
pub(super) const PRESET_CUSTOM: &str = "custom";

pub(super) const PRESET_STEREO_DOWNMIX: &str = "stereo_downmix";

pub(super) const PRESET_MS_ENCODE: &str = "ms_encode";

pub(super) const PRESET_MS_DECODE: &str = "ms_decode";

pub(super) const PRESET_51_REMAP: &str = "5.1_remap";

pub(super) const PRESET_CHOICES: &[&str] = &[
    PRESET_CUSTOM,
    PRESET_STEREO_DOWNMIX,
    PRESET_MS_ENCODE,
    PRESET_MS_DECODE,
    PRESET_51_REMAP,
];
