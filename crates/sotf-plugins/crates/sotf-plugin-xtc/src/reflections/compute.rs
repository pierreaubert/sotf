use super::super::filters::SPEED_OF_SOUND;
use super::types::ReflectionPath;
use super::types::RoomGeometry;
use std::f32::consts::PI;

/// Compute image source positions for first-order reflections.
///
/// Six image sources per speaker: one for each wall surface (left, right, front, back, floor, ceiling).
/// Each image is the speaker position reflected across the respective surface.
pub(crate) fn compute_image_sources(
    speaker_pos: [f32; 3],
    ear_pos: [f32; 3],
    direct_dist: f32,
    room: &RoomGeometry,
) -> Vec<ReflectionPath> {
    let half_w = room.width / 2.0;
    let half_d = room.depth / 2.0;

    // Six image source positions: mirror speaker across each surface
    let images = [
        // Left wall (x = -half_w): reflect x across x=-half_w
        [
            -half_w - (speaker_pos[0] - (-half_w)),
            speaker_pos[1],
            speaker_pos[2],
        ],
        // Right wall (x = +half_w): reflect x across x=+half_w
        [
            half_w + (half_w - speaker_pos[0]),
            speaker_pos[1],
            speaker_pos[2],
        ],
        // Front wall (z = +half_d): reflect z across z=+half_d
        [
            speaker_pos[0],
            speaker_pos[1],
            half_d + (half_d - speaker_pos[2]),
        ],
        // Back wall (z = -half_d): reflect z across z=-half_d
        [
            speaker_pos[0],
            speaker_pos[1],
            -half_d - (speaker_pos[2] - (-half_d)),
        ],
        // Floor (y = 0): reflect y across y=0
        [speaker_pos[0], -speaker_pos[1], speaker_pos[2]],
        // Ceiling (y = height): reflect y across y=height
        [
            speaker_pos[0],
            2.0 * room.height - speaker_pos[1],
            speaker_pos[2],
        ],
    ];

    let mut paths = Vec::with_capacity(6);

    for image_pos in &images {
        let dx = image_pos[0] - ear_pos[0];
        let dy = image_pos[1] - ear_pos[1];
        let dz = image_pos[2] - ear_pos[2];
        let image_dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if image_dist < 1e-6 {
            continue;
        }

        // wall_absorption is a Sabine energy coefficient (0 = reflective, 1 = absorptive).
        // Pressure reflection coefficient = sqrt(1 - α), not (1 - α).
        let amplitude = (1.0 - room.wall_absorption).sqrt() * (direct_dist / image_dist);
        let delay_s = image_dist / SPEED_OF_SOUND;

        // Shadow angle: azimuth from head center to image source (in horizontal plane)
        let azimuth = (dx).atan2(dz).abs();
        let shadow_angle = (PI / 2.0 + azimuth).min(PI);

        paths.push(ReflectionPath {
            delay_s,
            amplitude,
            shadow_angle,
        });
    }

    paths
}

/// Detect comb filter nulls and compute per-bin beta boost factors.
///
/// Smooths the magnitude envelope, then boosts beta at bins where magnitude
/// drops significantly below the smoothed envelope (comb filter nulls).
pub(crate) fn compute_reflection_beta_boost(
    h_total_magnitude: &[f32],
    num_bins: usize,
    boost_factor: f32,
) -> Vec<f32> {
    if num_bins < 3 {
        return vec![1.0; num_bins];
    }

    // Step 1: ~1/6 octave smoothing via moving average with frequency-proportional window
    let mut smoothed = vec![0.0_f32; num_bins];
    smoothed[0] = h_total_magnitude[0];
    for (bin, smoothed_val) in smoothed.iter_mut().enumerate().skip(1) {
        // Window width: ~1/6 octave in bins
        // 1/6 octave at bin b spans b * (2^(1/12) - 1) bins on each side
        let half_width = ((bin as f32 * 0.06) as usize).max(1).min(num_bins / 4);
        let start = bin.saturating_sub(half_width);
        let end = (bin + half_width + 1).min(num_bins);
        let count = (end - start) as f32;
        let sum: f32 = h_total_magnitude[start..end].iter().sum();
        *smoothed_val = sum / count;
    }

    // Step 2: Compute raw boost where magnitude is >10 dB below smoothed envelope
    let threshold_db = 10.0;
    let threshold_ratio = 10.0_f32.powf(-threshold_db / 20.0); // ~0.316

    let mut raw_boost = vec![1.0_f32; num_bins];
    for bin in 1..num_bins - 1 {
        if smoothed[bin] > 1e-10 {
            let ratio = h_total_magnitude[bin] / smoothed[bin];
            if ratio < threshold_ratio {
                // Null depth in dB (positive value)
                let null_depth_db = -20.0 * ratio.max(1e-6).log10();
                // Proportional boost, capped at boost_factor × base
                let boost =
                    (1.0 + (null_depth_db / threshold_db) * (boost_factor - 1.0)).min(boost_factor);
                raw_boost[bin] = boost;
            }
        }
    }

    // Step 3: 3-bin smoothing to avoid sharp transitions
    let mut final_boost = vec![1.0_f32; num_bins];
    for bin in 1..num_bins - 1 {
        final_boost[bin] = (raw_boost[bin - 1] + raw_boost[bin] + raw_boost[bin + 1]) / 3.0;
    }
    final_boost[0] = raw_boost[0];
    if num_bins > 1 {
        final_boost[num_bins - 1] = raw_boost[num_bins - 1];
    }

    final_boost
}
