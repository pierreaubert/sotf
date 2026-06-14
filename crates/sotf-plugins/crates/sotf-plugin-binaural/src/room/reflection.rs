use super::room_model::RoomModel;
use super::types::Reflection;
use math_rir::SsirResult;

/// Add reflections from image sources to the channel reflection list
pub(super) fn add_image_reflections(
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

/// Convert SSIR analysis result into a list of Reflections for the binaural plugin.
pub(super) fn ssir_result_to_reflections(
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
