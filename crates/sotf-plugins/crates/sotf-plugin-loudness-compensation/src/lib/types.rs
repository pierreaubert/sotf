use super::channel_loudness_params::ChannelLoudnessParams;
use super::default::default_auto_calibrated;
use super::default::default_auto_gain_enabled;
use super::default::default_auto_gain_max_db;
use super::default::default_auto_gain_position;
use super::default::default_auto_gain_smoothing_ms;
use super::default::default_headroom_normalized;
use super::default::default_high_freq;
use super::default::default_high_gain;
use super::default::default_low_freq;
use super::default::default_low_gain;
use super::default::default_mid_enabled;
use super::default::default_mid_freq;
use super::default::default_mid_gain;
use super::default::default_mid_q;
use super::default::default_playback_level_db;
use super::default::default_playback_volume_db;
use super::default::default_reference_level_db;
use super::loudness_compensation_plugin::LoudnessCompensationPlugin;
use crate::params::MODE_LABELS;
use serde::{Deserialize, Deserializer, Serialize};
use sotf_host::define_choice_index_deserializer;

define_choice_index_deserializer!(deserialize_mode, MODE_LABELS);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessCompensationPluginParams {
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default = "default_low_gain")]
    pub low_gain: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_high_gain")]
    pub high_gain: f32,
    #[serde(default = "default_mid_enabled")]
    pub mid_enabled: bool,
    #[serde(default = "default_mid_freq")]
    pub mid_freq: f32,
    #[serde(default = "default_mid_gain")]
    pub mid_gain: f32,
    #[serde(default = "default_mid_q")]
    pub mid_q: f32,
    #[serde(default)]
    pub channel_params: Vec<ChannelLoudnessParams>,
    #[serde(default = "default_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "default_auto_gain_max_db")]
    pub auto_gain_max_db: f32,
    #[serde(default = "default_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f32,
    /// Auto-gain position: "pre", "post" (default), or "disabled".
    ///
    /// Accepts the canonical label or the UI choice index used on the wire
    /// (`0` = disabled, `1` = pre, `2` = post), matching the engine's own
    /// index-to-label mapping. Unknown values are a hard error, never a
    /// silent default.
    #[serde(
        default = "default_auto_gain_position",
        deserialize_with = "deserialize_auto_gain_position"
    )]
    pub auto_gain_position: String,
    /// 0 = Manual (default), 1 = ISO 226, 2 = Auto
    #[serde(default, deserialize_with = "deserialize_mode")]
    pub mode: usize,
    #[serde(default = "default_playback_level_db")]
    pub playback_level_db: f32,
    #[serde(default = "default_reference_level_db")]
    pub reference_level_db: f32,
    /// Engine playback volume in dB (used in Auto mode)
    #[serde(default = "default_playback_volume_db")]
    pub playback_volume_db: f32,
    /// Apply broadband attenuation equal to the positive peak of the realized
    /// filter cascade. Disabled by default so the ISO 226 1 kHz reference is
    /// preserved; users selecting this policy accept the visible level shift.
    #[serde(default = "default_headroom_normalized")]
    pub headroom_normalized: bool,
    /// Confirms that `reference_level_db` is a measured SPL at digital 0 dB
    /// volume for the actual playback chain. Required by Auto mode.
    #[serde(default = "default_auto_calibrated")]
    pub auto_calibrated: bool,
}

impl Default for LoudnessCompensationPluginParams {
    fn default() -> Self {
        Self {
            low_freq: default_low_freq(),
            low_gain: default_low_gain(),
            high_freq: default_high_freq(),
            high_gain: default_high_gain(),
            mid_enabled: default_mid_enabled(),
            mid_freq: default_mid_freq(),
            mid_gain: default_mid_gain(),
            mid_q: default_mid_q(),
            channel_params: Vec::new(),
            auto_gain_enabled: default_auto_gain_enabled(),
            auto_gain_max_db: default_auto_gain_max_db(),
            auto_gain_smoothing_ms: default_auto_gain_smoothing_ms(),
            auto_gain_position: default_auto_gain_position(),
            mode: 0,
            playback_level_db: default_playback_level_db(),
            reference_level_db: default_reference_level_db(),
            playback_volume_db: default_playback_volume_db(),
            headroom_normalized: default_headroom_normalized(),
            auto_calibrated: default_auto_calibrated(),
        }
    }
}

/// Deserialize `auto_gain_position` from either its canonical string label
/// or the UI choice index (`0` = disabled, `1` = pre, `2` = post).
fn deserialize_auto_gain_position<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct PositionVisitor;

    impl Visitor<'_> for PositionVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter
                .write_str("a position label (\"disabled\", \"pre\", \"post\") or index (0, 1, 2)")
        }

        fn visit_str<E>(self, value: &str) -> Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_owned())
        }

        fn visit_u64<E>(self, value: u64) -> Result<String, E>
        where
            E: de::Error,
        {
            match value {
                0 => Ok("disabled".to_owned()),
                1 => Ok("pre".to_owned()),
                2 => Ok("post".to_owned()),
                other => Err(E::custom(format!(
                    "invalid auto_gain_position index {other}; expected 0, 1, or 2"
                ))),
            }
        }

        fn visit_i64<E>(self, value: i64) -> Result<String, E>
        where
            E: de::Error,
        {
            u64::try_from(value)
                .map_err(|_| {
                    E::custom(format!(
                        "invalid auto_gain_position index {value}; expected 0, 1, or 2"
                    ))
                })
                .and_then(|index| self.visit_u64(index))
        }
    }

    deserializer.deserialize_any(PositionVisitor)
}

/// Type alias for backward compatibility.
pub type FletcherMunsonPlugin = LoudnessCompensationPlugin;

/// Type alias for backward compatibility.
pub type FletcherMunsonPluginParams = LoudnessCompensationPluginParams;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn position_of(parameters: serde_json::Value) -> Result<String, String> {
        serde_json::from_value::<LoudnessCompensationPluginParams>(parameters)
            .map(|params| params.auto_gain_position)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn wire_indices_map_to_canonical_labels() {
        assert_eq!(
            position_of(json!({ "auto_gain_position": 0 })).unwrap(),
            "disabled"
        );
        assert_eq!(
            position_of(json!({ "auto_gain_position": 1 })).unwrap(),
            "pre"
        );
        assert_eq!(
            position_of(json!({ "auto_gain_position": 2 })).unwrap(),
            "post"
        );
    }

    #[test]
    fn string_labels_still_accepted() {
        assert_eq!(
            position_of(json!({ "auto_gain_position": "pre" })).unwrap(),
            "pre"
        );
        assert_eq!(
            position_of(json!({ "auto_gain_position": "post" })).unwrap(),
            "post"
        );
        assert_eq!(
            position_of(json!({ "auto_gain_position": "disabled" })).unwrap(),
            "disabled"
        );
    }

    #[test]
    fn missing_position_uses_default() {
        assert_eq!(
            position_of(json!({})).unwrap(),
            default_auto_gain_position()
        );
    }

    #[test]
    fn unknown_values_are_hard_errors() {
        assert!(position_of(json!({ "auto_gain_position": 7 })).is_err());
        assert!(position_of(json!({ "auto_gain_position": -1 })).is_err());
        assert!(position_of(json!({ "auto_gain_position": true })).is_err());
    }
}
