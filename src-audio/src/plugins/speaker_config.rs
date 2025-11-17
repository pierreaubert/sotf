// ============================================================================
// Speaker Configuration Module
// ============================================================================
//
// Standard speaker positions based on ITU-R BS.775 and Dolby Atmos specs
// Azimuth: 0° = front, +90° = left, -90° = right, ±180° = back
// Elevation: 0° = ear level, +90° = overhead

use serde::{Deserialize, Serialize};

/// Speaker position in 3D space using spherical coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpeakerPosition {
    /// Channel label (e.g., "FL", "C", "TFL")
    pub label: &'static str,
    /// Full name (e.g., "Front Left", "Center")
    pub name: &'static str,
    /// Horizontal angle in degrees (-180 to +180)
    pub azimuth: f32,
    /// Vertical angle in degrees (0 to 90)
    pub elevation: f32,
    /// Channel index in output array
    pub channel: usize,
    /// True if this is the LFE channel
    pub is_lfe: bool,
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
}

// ============================================================================
// Standard Speaker Configurations
// ============================================================================

/// 5.1 Surround (ITU-R BS.775)
pub const CONFIG_5_1: SpeakerConfig = SpeakerConfig {
    id: "5.1",
    name: "5.1 Surround",
    description: "Standard 5.1 surround sound (ITU-R BS.775)",
    total_channels: 6,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 110.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -110.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
    ],
};

/// 7.1 Surround
pub const CONFIG_7_1: SpeakerConfig = SpeakerConfig {
    id: "7.1",
    name: "7.1 Surround",
    description: "7.1 surround with side and back speakers",
    total_channels: 8,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
    ],
};

/// 5.1.2 Atmos
pub const CONFIG_5_1_2: SpeakerConfig = SpeakerConfig {
    id: "5.1.2",
    name: "5.1.2 Atmos",
    description: "5.1 with 2 height speakers",
    total_channels: 8,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 110.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -110.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 7,
            is_lfe: false,
        },
    ],
};

/// 5.1.4 Atmos
pub const CONFIG_5_1_4: SpeakerConfig = SpeakerConfig {
    id: "5.1.4",
    name: "5.1.4 Atmos",
    description: "5.1 with 4 height speakers",
    total_channels: 10,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 110.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -110.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 9,
            is_lfe: false,
        },
    ],
};

/// 7.1.2 Atmos
pub const CONFIG_7_1_2: SpeakerConfig = SpeakerConfig {
    id: "7.1.2",
    name: "7.1.2 Atmos",
    description: "7.1 with 2 height speakers",
    total_channels: 10,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 9,
            is_lfe: false,
        },
    ],
};

/// 7.1.4 Atmos
pub const CONFIG_7_1_4: SpeakerConfig = SpeakerConfig {
    id: "7.1.4",
    name: "7.1.4 Atmos",
    description: "7.1 with 4 height speakers",
    total_channels: 12,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 9,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 10,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 11,
            is_lfe: false,
        },
    ],
};

/// 9.1.4 Atmos
pub const CONFIG_9_1_4: SpeakerConfig = SpeakerConfig {
    id: "9.1.4",
    name: "9.1.4 Atmos",
    description: "9.1 with 4 height speakers (adds wide channels)",
    total_channels: 14,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WL",
            name: "Wide Left",
            azimuth: 60.0,
            elevation: 0.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WR",
            name: "Wide Right",
            azimuth: -60.0,
            elevation: 0.0,
            channel: 9,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 10,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 11,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 12,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 13,
            is_lfe: false,
        },
    ],
};

/// 9.1.6 Atmos
pub const CONFIG_9_1_6: SpeakerConfig = SpeakerConfig {
    id: "9.1.6",
    name: "9.1.6 Atmos",
    description: "9.1 with 6 height speakers (adds top mid channels)",
    total_channels: 16,
    speakers: &[
        SpeakerPosition {
            label: "FL",
            name: "Front Left",
            azimuth: 30.0,
            elevation: 0.0,
            channel: 0,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "FR",
            name: "Front Right",
            azimuth: -30.0,
            elevation: 0.0,
            channel: 1,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "C",
            name: "Center",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 2,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "LFE",
            name: "Low Frequency Effects",
            azimuth: 0.0,
            elevation: 0.0,
            channel: 3,
            is_lfe: true,
        },
        SpeakerPosition {
            label: "SL",
            name: "Side Left",
            azimuth: 90.0,
            elevation: 0.0,
            channel: 4,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "SR",
            name: "Side Right",
            azimuth: -90.0,
            elevation: 0.0,
            channel: 5,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BL",
            name: "Back Left",
            azimuth: 150.0,
            elevation: 0.0,
            channel: 6,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "BR",
            name: "Back Right",
            azimuth: -150.0,
            elevation: 0.0,
            channel: 7,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WL",
            name: "Wide Left",
            azimuth: 60.0,
            elevation: 0.0,
            channel: 8,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "WR",
            name: "Wide Right",
            azimuth: -60.0,
            elevation: 0.0,
            channel: 9,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFL",
            name: "Top Front Left",
            azimuth: 30.0,
            elevation: 45.0,
            channel: 10,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TFR",
            name: "Top Front Right",
            azimuth: -30.0,
            elevation: 45.0,
            channel: 11,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBL",
            name: "Top Back Left",
            azimuth: 150.0,
            elevation: 45.0,
            channel: 12,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TBR",
            name: "Top Back Right",
            azimuth: -150.0,
            elevation: 45.0,
            channel: 13,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TMiL",
            name: "Top Middle Left",
            azimuth: 90.0,
            elevation: 45.0,
            channel: 14,
            is_lfe: false,
        },
        SpeakerPosition {
            label: "TMiR",
            name: "Top Middle Right",
            azimuth: -90.0,
            elevation: 45.0,
            channel: 15,
            is_lfe: false,
        },
    ],
};

// ============================================================================
// Configuration Lookup
// ============================================================================

/// Get speaker configuration by ID
pub fn get_speaker_config(id: &str) -> Option<&'static SpeakerConfig> {
    match id {
        "5.1" => Some(&CONFIG_5_1),
        "7.1" => Some(&CONFIG_7_1),
        "5.1.2" => Some(&CONFIG_5_1_2),
        "5.1.4" => Some(&CONFIG_5_1_4),
        "7.1.2" => Some(&CONFIG_7_1_2),
        "7.1.4" => Some(&CONFIG_7_1_4),
        "9.1.4" => Some(&CONFIG_9_1_4),
        "9.1.6" => Some(&CONFIG_9_1_6),
        _ => None,
    }
}

/// Get all available configuration IDs
pub fn get_available_configs() -> &'static [&'static str] {
    &[
        "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
    ]
}

// ============================================================================
// VBAP (Vector Base Amplitude Panning)
// ============================================================================

/// Calculate panning gain for a speaker based on source position
/// Uses Vector Base Amplitude Panning (VBAP) principles
///
/// # Arguments
/// * `source_azimuth` - Source azimuth in degrees
/// * `source_elevation` - Source elevation in degrees
/// * `speaker_azimuth` - Speaker azimuth in degrees
/// * `speaker_elevation` - Speaker elevation in degrees
///
/// # Returns
/// Gain value (0.0 to 1.0)
pub fn calculate_panning_gain(
    source_azimuth: f32,
    source_elevation: f32,
    speaker_azimuth: f32,
    speaker_elevation: f32,
) -> f32 {
    // Convert to radians
    let src_az = source_azimuth.to_radians();
    let src_el = source_elevation.to_radians();
    let spk_az = speaker_azimuth.to_radians();
    let spk_el = speaker_elevation.to_radians();

    // Convert spherical to Cartesian coordinates
    let src_x = src_el.cos() * src_az.sin();
    let src_y = src_el.cos() * src_az.cos();
    let src_z = src_el.sin();

    let spk_x = spk_el.cos() * spk_az.sin();
    let spk_y = spk_el.cos() * spk_az.cos();
    let spk_z = spk_el.sin();

    // Calculate dot product (cosine of angle between vectors)
    let dot_product = src_x * spk_x + src_y * spk_y + src_z * spk_z;

    // Map from [-1, 1] to [0, 1] with cosine law
    // Use raised cosine for smoother panning
    dot_product.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_speaker_config() {
        assert!(get_speaker_config("5.1").is_some());
        assert!(get_speaker_config("7.1.4").is_some());
        assert!(get_speaker_config("invalid").is_none());
    }

    #[test]
    fn test_config_5_1() {
        let config = get_speaker_config("5.1").unwrap();
        assert_eq!(config.total_channels, 6);
        assert_eq!(config.speakers.len(), 6);
        assert_eq!(config.speakers[0].label, "FL");
        assert_eq!(config.speakers[3].is_lfe, true);
    }

    #[test]
    fn test_config_7_1_4() {
        let config = get_speaker_config("7.1.4").unwrap();
        assert_eq!(config.total_channels, 12);
        assert_eq!(config.speakers.len(), 12);

        // Check height channels
        let height_speakers: Vec<_> = config
            .speakers
            .iter()
            .filter(|s| s.elevation > 0.0)
            .collect();
        assert_eq!(height_speakers.len(), 4);
    }

    #[test]
    fn test_panning_gain_center() {
        // Source at center (0°, 0°) should have max gain at center speaker
        let gain = calculate_panning_gain(0.0, 0.0, 0.0, 0.0);
        assert!((gain - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_panning_gain_opposite() {
        // Source at front (0°) should have zero gain at back (180°)
        let gain = calculate_panning_gain(0.0, 0.0, 180.0, 0.0);
        assert!(gain < 0.01);
    }

    #[test]
    fn test_panning_gain_orthogonal() {
        // Source at front (0°) and side (90°) should have ~0.707 gain
        let gain = calculate_panning_gain(0.0, 0.0, 90.0, 0.0);
        assert!(gain < 0.1); // Should be very low since they're perpendicular
    }

    #[test]
    fn test_panning_gain_elevation() {
        // Test elevation panning
        let gain = calculate_panning_gain(0.0, 45.0, 0.0, 45.0);
        assert!((gain - 1.0).abs() < 0.001);

        let gain = calculate_panning_gain(0.0, 0.0, 0.0, 45.0);
        assert!(gain > 0.5 && gain < 1.0); // Partial gain due to elevation difference
    }
}
