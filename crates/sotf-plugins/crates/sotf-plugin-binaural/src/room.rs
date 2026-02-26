use sotf_host::sofa::SourcePosition;
use sotf_host::speaker_config::{SpeakerConfig, SpeakerPosition};
use serde::{Deserialize, Serialize};

// ============================================================================
// Room Model Configuration
// ============================================================================

/// Room dimensions and acoustic properties for externalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomModel {
    /// Room dimensions in meters [width, depth, height]
    #[serde(default = "default_room_dimensions")]
    pub dimensions: [f32; 3],

    /// Listener position in room [x, y, z] in meters from corner (0,0,0)
    #[serde(default = "default_listener_position")]
    pub listener_position: [f32; 3],

    /// Wall absorption coefficients [front, back, left, right, floor, ceiling]
    /// Range 0.0 (perfect reflection) to 1.0 (complete absorption)
    #[serde(default = "default_absorption_coefficients")]
    pub absorption: [f32; 6],

    /// Maximum reflection order (0 = direct only, 1 = first-order reflections, etc.)
    #[serde(default = "default_max_reflection_order")]
    pub max_order: usize,

    /// Speed of sound in m/s (typically 343.0 at 20°C)
    #[serde(default = "default_speed_of_sound")]
    pub speed_of_sound: f32,
}

fn default_room_dimensions() -> [f32; 3] {
    [4.0, 5.0, 2.5] // Small listening room: 4m wide × 5m deep × 2.5m high
}

fn default_listener_position() -> [f32; 3] {
    [2.0, 2.0, 1.2] // Center of room, seated height
}

fn default_absorption_coefficients() -> [f32; 6] {
    [0.15, 0.15, 0.20, 0.20, 0.30, 0.25] // Typical living room
}

fn default_max_reflection_order() -> usize {
    1 // First-order reflections only (early reflections)
}

fn default_speed_of_sound() -> f32 {
    343.0 // m/s at 20°C
}

impl Default for RoomModel {
    fn default() -> Self {
        Self {
            dimensions: default_room_dimensions(),
            listener_position: default_listener_position(),
            absorption: default_absorption_coefficients(),
            max_order: default_max_reflection_order(),
            speed_of_sound: default_speed_of_sound(),
        }
    }
}

/// Represents a single reflection path from source to listener
#[derive(Debug, Clone)]
pub struct Reflection {
    /// Delay in samples
    pub delay_samples: usize,
    /// Linear gain (after absorption and distance attenuation)
    pub gain: f32,
    /// Left/right channel multipliers for asymmetric reflections
    pub left_gain: f32,
    pub right_gain: f32,
}

/// Helper to convert SpeakerPosition to SourcePosition
pub fn speaker_to_source_position(speaker: &SpeakerPosition) -> SourcePosition {
    // Use a fixed distance of 1.0 for all speakers
    SourcePosition::new(speaker.azimuth, speaker.elevation, 1.0)
}

#[allow(dead_code)]
pub fn calculate_reflections(
    room: &RoomModel,
    speaker_config: &SpeakerConfig,
    sample_rate: u32,
) -> Vec<Vec<Reflection>> {
    let mut reflections = Vec::with_capacity(speaker_config.speakers.len());

    if room.max_order == 0 {
        // Return empty reflections if disabled
        for _ in 0..speaker_config.speakers.len() {
            reflections.push(Vec::new());
        }
        return reflections;
    }

    // Simple Image Source Method for early reflections
    // We only consider 1st order reflections for now as per default

    // Room boundaries relative to origin (0,0,0)
    let bounds = room.dimensions;
    let listener = room.listener_position;

    for speaker in speaker_config.speakers {
        let mut channel_reflections = Vec::new();

        // Convert speaker position (azimuth/elevation) to Cartesian coordinates relative to listener
        // Assume speaker is at 1.5m distance (typical near-field monitor)
        let dist = 1.5;
        let az_rad = speaker.azimuth.to_radians();
        let el_rad = speaker.elevation.to_radians();

        // Speaker position relative to listener
        let spk_rel_x = dist * az_rad.sin() * el_rad.cos();
        let spk_rel_y = dist * az_rad.cos() * el_rad.cos();
        let spk_rel_z = dist * el_rad.sin();

        // Absolute speaker position in room
        let spk_pos = [
            listener[0] + spk_rel_x,
            listener[1] + spk_rel_y,
            listener[2] + spk_rel_z,
        ];

        // Direct sound distance (for reference)
        let direct_dist = dist;

        // 1st order images
        // 6 walls: Front(y+), Back(y-), Left(x-), Right(x+), Floor(z-), Ceiling(z+)
        // Indices in absorption array: [front, back, left, right, floor, ceiling]

        let images = [
            // Front wall (y = bounds[1])
            ([spk_pos[0], 2.0 * bounds[1] - spk_pos[1], spk_pos[2]], 0),
            // Back wall (y = 0)
            ([spk_pos[0], -spk_pos[1], spk_pos[2]], 1),
            // Left wall (x = 0)
            ([-spk_pos[0], spk_pos[1], spk_pos[2]], 2),
            // Right wall (x = bounds[0])
            ([2.0 * bounds[0] - spk_pos[0], spk_pos[1], spk_pos[2]], 3),
            // Floor (z = 0)
            ([spk_pos[0], spk_pos[1], -spk_pos[2]], 4),
            // Ceiling (z = bounds[2])
            ([spk_pos[0], spk_pos[1], 2.0 * bounds[2] - spk_pos[2]], 5),
        ];

        for (img_pos, wall_idx) in images.iter() {
            // Calculate distance from image to listener
            let dx = img_pos[0] - listener[0];
            let dy = img_pos[1] - listener[1];
            let dz = img_pos[2] - listener[2];
            let img_dist = (dx * dx + dy * dy + dz * dz).sqrt();

            // Path difference
            let path_diff = img_dist - direct_dist;

            if path_diff > 0.0 {
                // Delay
                let delay_sec = path_diff / room.speed_of_sound;
                let delay_samples = (delay_sec * sample_rate as f32).round() as usize;

                // Attenuation
                // 1. Distance attenuation (1/r law)
                let dist_att = direct_dist / img_dist;

                // 2. Wall absorption
                let wall_att = 1.0 - room.absorption[*wall_idx];

                let gain = dist_att * wall_att;

                // Simple panning for reflections based on direction
                // Calculate azimuth of reflection
                let az = dx.atan2(dy); // -pi to pi

                // Pan law (constant power)
                let p = (az + std::f32::consts::PI / 4.0) * 0.5; // Shifted for simple panning
                let left = p.cos().abs();
                let right = p.sin().abs();

                channel_reflections.push(Reflection {
                    delay_samples,
                    gain,
                    left_gain: left,
                    right_gain: right,
                });
            }
        }

        reflections.push(channel_reflections);
    }

    reflections
}
