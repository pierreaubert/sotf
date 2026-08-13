use super::speaker_position::SpeakerPosition;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Semantic role of one interleaved audio channel.
///
/// Unlike a channel count, a role is sufficient to apply layout-dependent
/// algorithms such as ITU-R BS.1770 loudness weighting without assuming a
/// physical channel order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelRole {
    Mono,
    FrontLeft,
    FrontRight,
    FrontCenter,
    Lfe,
    SideLeft,
    SideRight,
    BackLeft,
    BackRight,
    WideLeft,
    WideRight,
    TopFrontLeft,
    TopFrontRight,
    TopMiddleLeft,
    TopMiddleRight,
    TopBackLeft,
    TopBackRight,
}

impl ChannelRole {
    /// Linear energy multiplier from ITU-R BS.1770.
    ///
    /// LFE is excluded. Front L/R/C and mono use unity. Other spatial roles
    /// use the +1.5 dB surround coefficient represented by the reference
    /// implementation as 1.41.
    pub fn bs1770_weight(self) -> f32 {
        match self {
            Self::Lfe => 0.0,
            Self::SideLeft | Self::SideRight | Self::BackLeft | Self::BackRight => 1.41,
            Self::Mono
            | Self::FrontLeft
            | Self::FrontRight
            | Self::FrontCenter
            | Self::WideLeft
            | Self::WideRight
            | Self::TopFrontLeft
            | Self::TopFrontRight
            | Self::TopMiddleLeft
            | Self::TopMiddleRight
            | Self::TopBackLeft
            | Self::TopBackRight => 1.0,
        }
    }

    fn from_speaker(speaker: &SpeakerPosition) -> Result<Self, String> {
        if speaker.is_lfe {
            if speaker.label != "LFE" {
                return Err(format!(
                    "speaker {} is marked LFE but has role label {}",
                    speaker.channel, speaker.label
                ));
            }
            return Ok(Self::Lfe);
        }
        match speaker.label {
            "M" => Ok(Self::Mono),
            "L" | "FL" => Ok(Self::FrontLeft),
            "R" | "FR" => Ok(Self::FrontRight),
            "C" => Ok(Self::FrontCenter),
            "SL" => Ok(Self::SideLeft),
            "SR" => Ok(Self::SideRight),
            "BL" => Ok(Self::BackLeft),
            "BR" => Ok(Self::BackRight),
            "WL" => Ok(Self::WideLeft),
            "WR" => Ok(Self::WideRight),
            "TFL" => Ok(Self::TopFrontLeft),
            "TFR" => Ok(Self::TopFrontRight),
            "TML" | "TMiL" => Ok(Self::TopMiddleLeft),
            "TMR" | "TMiR" => Ok(Self::TopMiddleRight),
            "TBL" => Ok(Self::TopBackLeft),
            "TBR" => Ok(Self::TopBackRight),
            label => Err(format!("unknown speaker role label {label:?}")),
        }
    }
}

/// One role assignment in an interleaved channel layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelAssignment {
    pub index: usize,
    pub role: ChannelRole,
}

/// Explicit, serializable channel layout.
///
/// `channels` may appear in any JSON order; `index` is authoritative. A valid
/// layout assigns each index in `0..width` and each semantic role exactly once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelLayout {
    pub channels: Vec<ChannelAssignment>,
}

impl ChannelLayout {
    pub fn new(channels: Vec<ChannelAssignment>) -> Result<Self, String> {
        let layout = Self { channels };
        layout.validate_for_width(layout.channels.len())?;
        Ok(layout)
    }

    pub fn from_speaker_config(config: &SpeakerConfig) -> Result<Self, String> {
        if config.speakers.len() != config.total_channels {
            return Err(format!(
                "speaker config {} declares {} channels but contains {} speakers",
                config.id,
                config.total_channels,
                config.speakers.len()
            ));
        }
        let channels = config
            .speakers
            .iter()
            .map(|speaker| {
                Ok(ChannelAssignment {
                    index: speaker.channel,
                    role: ChannelRole::from_speaker(speaker)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let layout = Self { channels };
        layout.validate_for_width(config.total_channels)?;
        Ok(layout)
    }

    pub fn validate_for_width(&self, width: usize) -> Result<(), String> {
        if width == 0 {
            return Err("channel layout requires at least one channel".into());
        }
        if self.channels.len() != width {
            return Err(format!(
                "channel layout has {} assignments but audio width is {width}",
                self.channels.len()
            ));
        }
        let mut indices = vec![false; width];
        let mut roles = HashSet::with_capacity(width);
        for assignment in &self.channels {
            if assignment.index >= width {
                return Err(format!(
                    "channel layout index {} is outside width {width}",
                    assignment.index
                ));
            }
            if std::mem::replace(&mut indices[assignment.index], true) {
                return Err(format!(
                    "channel layout assigns index {} more than once",
                    assignment.index
                ));
            }
            if !roles.insert(assignment.role) {
                return Err(format!(
                    "channel layout assigns role {:?} more than once",
                    assignment.role
                ));
            }
        }
        if let Some(index) = indices.iter().position(|assigned| !assigned) {
            return Err(format!("channel layout does not assign index {index}"));
        }
        Ok(())
    }

    pub fn role_at(&self, index: usize) -> Option<ChannelRole> {
        self.channels
            .iter()
            .find(|assignment| assignment.index == index)
            .map(|assignment| assignment.role)
    }
}

/// Speaker configuration preset
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerConfig {
    /// Configuration ID (e.g., "5.1", "7.1.4")
    pub id: &'static str,
    /// Display name
    pub name: &'static str,
    /// Description
    pub description: &'static str,
    /// Total number of channels including LFE
    pub total_channels: usize,
    /// Speaker positions
    pub speakers: &'static [SpeakerPosition],
    /// Channel groupings for level meters
    pub meter_groups: &'static [MeterGroupSpec],
}

/// Channel info for meter display (static definition)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterChannelSpec {
    /// Channel index in output array
    pub index: usize,
    /// Short label (e.g., "L", "R", "C")
    pub label: &'static str,
    /// Display characters for vertical rendering (e.g., ["S", "L"] for "SL")
    pub display_chars: &'static [&'static str],
}

/// Meter group specification (static definition)
#[derive(Debug, Clone, PartialEq)]
pub struct MeterGroupSpec {
    /// Group name (e.g., "L/R", "Center", "Surrounds")
    pub name: &'static str,
    /// Channels in this group
    pub channels: &'static [MeterChannelSpec],
}

/// Generate a fallback meter channel spec for a given channel index
/// Returns a heap-allocated MeterChannelSpec for runtime use
pub fn make_fallback_channel(index: usize) -> MeterChannelSpec {
    // Use a static string for the label since we can't allocate at compile time
    // The caller should handle display names separately for fallback channels
    static FALLBACK_LABELS: &[&str] = &[
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
    ];
    static FALLBACK_CHARS: &[&[&str]] = &[
        &["1"],
        &["2"],
        &["3"],
        &["4"],
        &["5"],
        &["6"],
        &["7"],
        &["8"],
        &["9"],
        &["1", "0"],
        &["1", "1"],
        &["1", "2"],
        &["1", "3"],
        &["1", "4"],
        &["1", "5"],
        &["1", "6"],
    ];

    if index < FALLBACK_LABELS.len() {
        MeterChannelSpec {
            index,
            label: FALLBACK_LABELS[index],
            display_chars: FALLBACK_CHARS[index],
        }
    } else {
        // For channels beyond 16, just use the first entry as placeholder
        MeterChannelSpec {
            index,
            label: "?",
            display_chars: &["?"],
        }
    }
}
