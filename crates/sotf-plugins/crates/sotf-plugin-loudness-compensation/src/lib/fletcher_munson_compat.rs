use super::default::default_fm_compat_reference;
use super::types::LoudnessCompensationPluginParams;
use serde::{Deserialize, Serialize};

/// Backward-compatible deserialization of old FletcherMunson configs.
/// When the factory receives a `FletcherMunson` plugin type, it deserializes
/// into this struct and then converts to `LoudnessCompensationPluginParams`
/// with `mode = 2` (Auto).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FletcherMunsonCompat {
    #[serde(default)]
    pub playback_volume_db: f32,
    #[serde(default = "default_fm_compat_reference")]
    pub reference_level_db: f32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_gain_enabled: bool,
    #[serde(default)]
    pub smoothing_ms: f32,
}

impl FletcherMunsonCompat {
    /// Convert this backward-compat struct to a LoudnessCompensation params in Auto mode.
    pub fn into_loudness_compensation_params(self) -> LoudnessCompensationPluginParams {
        let smoothing_ms = if self.smoothing_ms <= 0.0 {
            // The legacy zero value meant an immediate transition. The new
            // smoother's minimum is 1 ms, which is the closest safe mapping.
            1.0
        } else {
            self.smoothing_ms.clamp(1.0, 1000.0)
        };
        LoudnessCompensationPluginParams {
            mode: if self.enabled { 2 } else { 0 },
            playback_volume_db: self.playback_volume_db,
            // Convert relative reference_level_db to absolute SPL estimate for ISO 226.
            // Old FM used relative dB (e.g. -14). Map to SPL: 83 + reference_level_db.
            reference_level_db: 83.0 + self.reference_level_db,
            auto_gain_enabled: self.auto_gain_enabled,
            auto_gain_position: if self.auto_gain_enabled {
                "post"
            } else {
                "disabled"
            }
            .into(),
            auto_gain_smoothing_ms: smoothing_ms,
            auto_calibrated: true,
            ..Default::default()
        }
    }
}
