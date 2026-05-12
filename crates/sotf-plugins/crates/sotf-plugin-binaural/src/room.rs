use math_rir::{SsirConfig, SsirResult};
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use sotf_host::sofa::SourcePosition;
use sotf_host::speaker_config::{SpeakerConfig, SpeakerPosition};
use std::collections::HashSet;
use std::path::Path;

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
    /// DOA azimuth in degrees (0 = front, positive = left). Used for per-reflection HRTF lookup.
    pub azimuth_deg: f32,
    /// DOA elevation in degrees. Used for per-reflection HRTF lookup.
    pub elevation_deg: f32,
    /// Pre-computed HRTF FIR for this reflection's DOA: [left_ir, right_ir] in frequency domain.
    /// Populated during initialize() when a SOFA file is loaded. Empty for ISM reflections.
    pub hrtf_filter: Option<ReflectionHrtf>,
}

/// Pre-computed frequency-domain HRTF filter for a single reflection.
#[derive(Debug, Clone)]
pub struct ReflectionHrtf {
    /// Left-ear HRTF in frequency domain
    pub left: Vec<Complex<f32>>,
    /// Right-ear HRTF in frequency domain
    pub right: Vec<Complex<f32>>,
    /// Broadband left-ear gain derived from HRTF energy (for efficient real-time use)
    pub left_gain_broadband: f32,
    /// Broadband right-ear gain derived from HRTF energy (for efficient real-time use)
    pub right_gain_broadband: f32,
}

impl ReflectionHrtf {
    /// Create from frequency-domain HRTF data, computing broadband gains automatically.
    pub fn from_freq_domain(left: Vec<Complex<f32>>, right: Vec<Complex<f32>>) -> Self {
        // Compute broadband gain as RMS of magnitude spectrum
        let left_energy: f32 = left.iter().map(|c| c.norm_sqr()).sum();
        let right_energy: f32 = right.iter().map(|c| c.norm_sqr()).sum();

        let n = left.len().max(1) as f32;
        let left_rms = (left_energy / n).sqrt();
        let right_rms = (right_energy / n).sqrt();

        // Normalize so that the louder ear gets gain 1.0
        let max_rms = left_rms.max(right_rms).max(1e-12);
        let left_gain_broadband = left_rms / max_rms;
        let right_gain_broadband = right_rms / max_rms;

        Self {
            left,
            right,
            left_gain_broadband,
            right_gain_broadband,
        }
    }
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

        // 2nd-order reflections: mirror each 1st-order image across the other 5 walls.
        // Deduplication is required: mirroring wall A then wall B produces the same
        // image as mirroring B then A. Without dedup, orthogonal-wall pairs each
        // contribute a duplicate reflection boosting those paths by 6 dB.
        if room.max_order >= 2 {
            let mut second_order_images: Vec<([f32; 3], usize, usize)> = Vec::new();
            // Track already-seen image positions at 1 cm resolution to skip duplicates.
            let mut seen_positions: HashSet<(i32, i32, i32)> = HashSet::new();

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
                        // Quantize to 1 cm to detect geometrically identical images
                        // produced by reversing the wall-pair order (A→B == B→A).
                        let key = (
                            (pos[0] * 100.0).round() as i32,
                            (pos[1] * 100.0).round() as i32,
                            (pos[2] * 100.0).round() as i32,
                        );
                        if seen_positions.insert(key) {
                            second_order_images.push((pos, wall_idx, wall2_idx));
                        }
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
                    let el = dz.atan2((dx * dx + dy * dy).sqrt());
                    // Standard constant-power sine-law panning.
                    // az convention: 0 = front, π/2 = right, −π/2 = left.
                    // sin(az) = -1 at left, 0 at front/back, +1 at right.
                    let pan = az.sin();
                    let left = ((1.0 - pan) * 0.5).sqrt();
                    let right = ((1.0 + pan) * 0.5).sqrt();

                    channel_reflections.push(Reflection {
                        delay_samples,
                        gain,
                        left_gain: left,
                        right_gain: right,
                        azimuth_deg: az.to_degrees(),
                        elevation_deg: el.to_degrees(),
                        hrtf_filter: None,
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
            let el = dz.atan2((dx * dx + dy * dy).sqrt());
            // Standard constant-power sine-law panning.
            // az convention: 0 = front, π/2 = right, −π/2 = left.
            // sin(az) = -1 at left, 0 at front/back, +1 at right.
            let pan = az.sin();
            let left = ((1.0 - pan) * 0.5).sqrt();
            let right = ((1.0 + pan) * 0.5).sqrt();

            channel_reflections.push(Reflection {
                delay_samples,
                gain,
                left_gain: left,
                right_gain: right,
                azimuth_deg: az.to_degrees(),
                elevation_deg: el.to_degrees(),
                hrtf_filter: None,
            });
        }
    }
}

// ============================================================================
// SSIR-based Measured Room Reflections
// ============================================================================

/// Load a WAV file and return its channels as separate Vec<f32>.
fn load_wav_channels(path: &Path) -> Result<(Vec<Vec<f32>>, u32), String> {
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Failed to open WAV file: {e}"))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let num_channels = spec.channels as usize;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read WAV samples: {e}"))?,
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let scale = 1.0 / (1i64 << (bits - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to read WAV samples: {e}"))?
        }
    };

    // Deinterleave
    let num_frames = samples.len() / num_channels;
    let mut channels = vec![vec![0.0f32; num_frames]; num_channels];
    for (frame_idx, chunk) in samples.chunks_exact(num_channels).enumerate() {
        for (ch, &sample) in chunk.iter().enumerate() {
            channels[ch][frame_idx] = sample;
        }
    }

    Ok((channels, sample_rate))
}

/// Load a measured Room Impulse Response (mono or multi-channel WAV)
/// and analyze it with SSIR to produce a flat list of reflections.
///
/// For multi-channel (4+ ch B-format) input: full SSIR with DOA estimation.
/// For mono/stereo: energy-based detection only, DOA defaults to (0, 0).
pub fn calculate_reflections_from_srir(
    srir_path: &Path,
    sample_rate: u32,
) -> Result<Vec<Reflection>, String> {
    let (channels, wav_sr) = load_wav_channels(srir_path)?;
    if channels.is_empty() || channels[0].is_empty() {
        return Err("SRIR file is empty".to_string());
    }

    // Use the WAV file's sample rate for analysis, then convert delays to engine sample rate
    let config = SsirConfig::new(wav_sr as f64);

    let result = if channels.len() >= 4 {
        // B-format: full SSIR with DOA
        let refs: Vec<&[f32]> = channels.iter().map(|ch| ch.as_slice()).collect();
        math_rir::analyze_srir(&refs, &config)
    } else {
        // Mono or stereo: use first channel, energy-based detection only
        math_rir::analyze_rir(&channels[0], &config)
    };

    Ok(ssir_result_to_reflections(
        &result,
        &channels[0],
        wav_sr,
        sample_rate,
    ))
}

/// Convert SSIR analysis result into a list of Reflections for the binaural plugin.
fn ssir_result_to_reflections(
    result: &SsirResult,
    omni_rir: &[f32],
    wav_sample_rate: u32,
    engine_sample_rate: u32,
) -> Vec<Reflection> {
    let mut reflections = Vec::new();

    // Skip the direct sound segment (index 0) — it's handled by the main HRTF path.
    // Convert each early reflection segment into a Reflection.
    let direct_toa = result.direct_sound().map(|ds| ds.toa_sample).unwrap_or(0);

    let rate_ratio = engine_sample_rate as f64 / wav_sample_rate as f64;

    for segment in result.reflections() {
        // Delay relative to direct sound, converted to engine sample rate
        let delay_samples_wav = segment.toa_sample.saturating_sub(direct_toa);
        let delay_samples = (delay_samples_wav as f64 * rate_ratio).round() as usize;

        if delay_samples == 0 {
            continue;
        }

        // Gain: peak amplitude of this reflection relative to direct sound
        let direct_amp = omni_rir
            .get(direct_toa)
            .map(|&s| s.abs())
            .unwrap_or(1.0)
            .max(1e-12);
        let reflection_amp = omni_rir
            .get(segment.toa_sample)
            .map(|&s| s.abs())
            .unwrap_or(0.0);
        let gain = (reflection_amp / direct_amp).min(1.0);

        // DOA: from SSIR analysis or default to front
        let (azimuth_deg, elevation_deg) = match segment.doa {
            Some(doa) => {
                let az = doa[1].atan2(doa[0]).to_degrees();
                let el = doa[2]
                    .atan2((doa[0] * doa[0] + doa[1] * doa[1]).sqrt())
                    .to_degrees();
                (az, el)
            }
            None => (0.0, 0.0),
        };

        // Standard constant-power sine-law panning (same formula as ISM).
        // az convention: 0 = front, π/2 = right, −π/2 = left.
        let az_rad = azimuth_deg.to_radians();
        let pan = az_rad.sin();
        let left_gain = ((1.0 - pan) * 0.5).sqrt();
        let right_gain = ((1.0 + pan) * 0.5).sqrt();

        reflections.push(Reflection {
            delay_samples,
            gain,
            left_gain,
            right_gain,
            azimuth_deg,
            elevation_deg,
            hrtf_filter: None, // Populated during initialize() when SOFA is loaded
        });
    }

    reflections
}
