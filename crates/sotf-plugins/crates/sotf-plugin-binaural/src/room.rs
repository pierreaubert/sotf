use serde::{Deserialize, Serialize};
use sotf_host::sofa::SourcePosition;
use sotf_host::speaker_config::{SpeakerConfig, SpeakerPosition};

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

        // Compute 1st-order image sources and optionally 2nd-order
        add_image_reflections(
            &images,
            &listener,
            direct_dist,
            room,
            sample_rate,
            &mut channel_reflections,
        );

        // 2nd-order reflections: mirror each 1st-order image across the other 5 walls
        if room.max_order >= 2 {
            let mut second_order_images: Vec<([f32; 3], usize, usize)> = Vec::new();
            for &(img_pos, wall_idx) in &images {
                // Mirror this 1st-order image across each wall except the one it was reflected from
                let second_images = [
                    // Front wall (y = bounds[1])
                    (0, [img_pos[0], 2.0 * bounds[1] - img_pos[1], img_pos[2]]),
                    // Back wall (y = 0)
                    (1, [img_pos[0], -img_pos[1], img_pos[2]]),
                    // Left wall (x = 0)
                    (2, [-img_pos[0], img_pos[1], img_pos[2]]),
                    // Right wall (x = bounds[0])
                    (3, [2.0 * bounds[0] - img_pos[0], img_pos[1], img_pos[2]]),
                    // Floor (z = 0)
                    (4, [img_pos[0], img_pos[1], -img_pos[2]]),
                    // Ceiling (z = bounds[2])
                    (5, [img_pos[0], img_pos[1], 2.0 * bounds[2] - img_pos[2]]),
                ];
                for (wall2_idx, pos) in second_images {
                    if wall2_idx != wall_idx {
                        second_order_images.push((pos, wall_idx, wall2_idx));
                    }
                }
            }

            for (img_pos, wall1_idx, wall2_idx) in &second_order_images {
                let dx = img_pos[0] - listener[0];
                let dy = img_pos[1] - listener[1];
                let dz = img_pos[2] - listener[2];
                let img_dist = (dx * dx + dy * dy + dz * dz).sqrt();

                let path_diff = img_dist - direct_dist;
                if path_diff > 0.0 {
                    let delay_sec = path_diff / room.speed_of_sound;
                    let delay_samples = (delay_sec * sample_rate as f32).round() as usize;

                    let dist_att = direct_dist / img_dist;
                    let wall_att1 = 1.0 - room.absorption[*wall1_idx];
                    let wall_att2 = 1.0 - room.absorption[*wall2_idx];
                    let gain = dist_att * wall_att1 * wall_att2;

                    let az = dx.atan2(dy);
                    let p = (az + std::f32::consts::PI / 4.0) * 0.5;
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
        }

        reflections.push(channel_reflections);
    }

    reflections
}

/// Add reflections from image sources to the channel reflection list
fn add_image_reflections(
    images: &[([f32; 3], usize)],
    listener: &[f32; 3],
    direct_dist: f32,
    room: &RoomModel,
    sample_rate: u32,
    channel_reflections: &mut Vec<Reflection>,
) {
    for (img_pos, wall_idx) in images.iter() {
        let dx = img_pos[0] - listener[0];
        let dy = img_pos[1] - listener[1];
        let dz = img_pos[2] - listener[2];
        let img_dist = (dx * dx + dy * dy + dz * dz).sqrt();

        let path_diff = img_dist - direct_dist;

        if path_diff > 0.0 {
            let delay_sec = path_diff / room.speed_of_sound;
            let delay_samples = (delay_sec * sample_rate as f32).round() as usize;

            let dist_att = direct_dist / img_dist;
            let wall_att = 1.0 - room.absorption[*wall_idx];
            let gain = dist_att * wall_att;

            let az = dx.atan2(dy);
            let p = (az + std::f32::consts::PI / 4.0) * 0.5;
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
}
