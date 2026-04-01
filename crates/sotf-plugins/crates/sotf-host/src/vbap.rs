// ============================================================================
// Vector Base Amplitude Panning (VBAP)
// ============================================================================
//
// Implements 2D and 3D VBAP for object-based audio rendering.
//
// Azimuth convention (matches SpeakerConfig):
//   0° = front, +90° = left, -90° = right, ±180° = back
//
// References:
//   Pulkki, V. (1997). Virtual sound source positioning using vector base
//   amplitude panning. JAES, 45(6), 456–466.

use crate::speaker_config::SpeakerPosition;

/// Pre-computed VBAP panner for a fixed speaker layout.
///
/// Precomputes the speaker adjacency structure at construction time to avoid
/// per-frame work. The `pan()` method writes gain coefficients into a
/// caller-provided slice (no allocation).
pub struct VbapPanner {
    /// Number of output channels (including LFE which always gets gain 0)
    num_speakers: usize,
    /// Whether the layout has any height speakers (elevation ≠ 0)
    has_height: bool,
    /// Pre-sorted non-LFE speaker list for 2D panning (sorted by azimuth desc)
    speakers_2d: Vec<Speaker2d>,
    /// Full speaker list (index = channel index) for 3D panning
    speakers_3d: Vec<Speaker3d>,
    /// Reusable output buffer (avoids per-call allocation)
    gains: Vec<f32>,
}

/// A horizontal-plane speaker entry for 2D VBAP
#[derive(Clone)]
struct Speaker2d {
    azimuth_deg: f32,
    channel: usize,
}

/// A 3D speaker entry (unit vector on the sphere)
#[derive(Clone)]
struct Speaker3d {
    x: f32,
    y: f32,
    z: f32,
    channel: usize,
    is_lfe: bool,
}

impl VbapPanner {
    /// Construct a VBAP panner for the given speaker set.
    ///
    /// `speakers`: non-empty slice of `SpeakerPosition` values (from `SpeakerConfig`).
    /// `num_channels`: total output channel count (must equal max(channel)+1).
    pub fn new(speakers: &[SpeakerPosition], num_channels: usize) -> Self {
        // Check if any speaker has non-zero elevation
        let has_height = speakers
            .iter()
            .any(|s| !s.is_lfe && s.elevation.abs() > 1.0);

        // Build 2D speaker list: non-LFE speakers only, sorted by azimuth descending
        // (largest azimuth first = leftmost first for our convention)
        let mut speakers_2d: Vec<Speaker2d> = speakers
            .iter()
            .filter(|s| !s.is_lfe)
            .map(|s| Speaker2d {
                azimuth_deg: s.azimuth,
                channel: s.channel,
            })
            .collect();
        // Sort by azimuth descending so we iterate left → right
        speakers_2d.sort_by(|a, b| b.azimuth_deg.partial_cmp(&a.azimuth_deg).unwrap());

        // Build 3D speaker list: spherical to Cartesian, azimuth/elevation in radians.
        // VBAP Cartesian convention:
        //   az=0,el=0 → (0, 0, 1) (front)
        //   az=90,el=0 → (1, 0, 0) (left)
        //   el=90 → (0, 1, 0) (above)
        let speakers_3d: Vec<Speaker3d> = speakers
            .iter()
            .map(|s| {
                let az = s.azimuth.to_radians();
                let el = s.elevation.to_radians();
                Speaker3d {
                    x: el.cos() * az.sin(),
                    y: el.sin(),
                    z: el.cos() * az.cos(),
                    channel: s.channel,
                    is_lfe: s.is_lfe,
                }
            })
            .collect();

        Self {
            num_speakers: num_channels,
            has_height,
            speakers_2d,
            speakers_3d,
            gains: vec![0.0_f32; num_channels],
        }
    }

    /// Compute VBAP gains for a source at the given azimuth and elevation.
    ///
    /// Returns a slice of length `num_channels`. LFE channels always receive 0.
    /// The caller must not hold onto the reference across subsequent `pan()` calls.
    ///
    /// This method contains no heap allocations.
    pub fn pan(&mut self, azimuth_deg: f32, elevation_deg: f32) -> &[f32] {
        // Clear all gains
        self.gains.fill(0.0);

        if self.has_height && elevation_deg.abs() > 5.0 {
            self.pan_3d(azimuth_deg, elevation_deg);
        } else {
            self.pan_2d(azimuth_deg);
        }

        &self.gains
    }

    /// 2D VBAP: find the speaker pair that brackets the source azimuth.
    ///
    /// The speakers_2d list is sorted by azimuth descending. We look for the
    /// adjacent pair (spk[i], spk[i+1]) such that
    ///   spk[i].azimuth >= source_az > spk[i+1].azimuth,
    /// then apply the sin-based constant-power pan law between them.
    /// If the source falls outside all pairs (wrap-around), we use the last
    /// speaker and the first (the pair that wraps from ±180°).
    fn pan_2d(&mut self, source_az: f32) {
        let n = self.speakers_2d.len();
        if n == 0 {
            return;
        }
        if n == 1 {
            let ch = self.speakers_2d[0].channel;
            if ch < self.num_speakers {
                self.gains[ch] = 1.0;
            }
            return;
        }

        // Find the pair that brackets source_az.
        // speakers_2d is sorted descending: [left ... right]
        let mut pair_idx: Option<usize> = None;
        for i in 0..n - 1 {
            let az_hi = self.speakers_2d[i].azimuth_deg;
            let az_lo = self.speakers_2d[i + 1].azimuth_deg;
            if source_az <= az_hi && source_az >= az_lo {
                pair_idx = Some(i);
                break;
            }
        }

        let (i, j) = match pair_idx {
            Some(i) => (i, i + 1),
            None => {
                // Wrap-around: source is between the last speaker and first speaker
                // (crossing the ±180° discontinuity).
                (n - 1, 0)
            }
        };

        let az1 = self.speakers_2d[i].azimuth_deg;
        let az2 = self.speakers_2d[j].azimuth_deg;
        let ch1 = self.speakers_2d[i].channel;
        let ch2 = self.speakers_2d[j].channel;

        // For the wrap-around case, az2 is actually at ~+30° (left of front)
        // and az1 is at ~-110° (right-rear). We need to compute the angular
        // span correctly. Remap az2 to be "below" az1 in the wrap direction.
        let (effective_az1, effective_az2, effective_src) = if pair_idx.is_none() {
            // Span wraps through ±180°. Shift az2 by 360° so it is < az1.
            let az2_wrapped = if az2 > az1 { az2 - 360.0 } else { az2 };
            // Shift source if needed
            let src = if source_az > az1 {
                source_az - 360.0
            } else {
                source_az
            };
            (az1, az2_wrapped, src)
        } else {
            (az1, az2, source_az)
        };

        let span = (effective_az1 - effective_az2).to_radians();
        let offset = (effective_az1 - effective_src).to_radians();

        // Constant-power gains using sin decomposition:
        //   g1 = sin(az2 - src) / sin(az2 - az1)  (gain for speaker at az1)
        //   g2 = sin(src - az1) / sin(az2 - az1)  (gain for speaker at az2)
        // Equivalent (after sign algebra for descending sort):
        //   g1 = sin(span - offset) / sin(span)
        //   g2 = sin(offset) / sin(span)
        let sin_span = span.sin();
        if sin_span.abs() < 1e-6 {
            // Degenerate: both speakers at same azimuth → equal contribution
            if ch1 < self.num_speakers {
                self.gains[ch1] = std::f32::consts::FRAC_1_SQRT_2;
            }
            if ch2 < self.num_speakers {
                self.gains[ch2] = std::f32::consts::FRAC_1_SQRT_2;
            }
            return;
        }

        let g1 = (span - offset).sin() / sin_span;
        let g2 = offset.sin() / sin_span;

        // Normalize for constant power
        let norm = (g1 * g1 + g2 * g2).sqrt().max(1e-8);
        let g1_norm = g1 / norm;
        let g2_norm = g2 / norm;

        // Clamp to [0, 1]: negative gains can appear numerically (outside the
        // expected bracket) and should not produce negative-phase contributions.
        if ch1 < self.num_speakers {
            self.gains[ch1] = g1_norm.clamp(0.0, 1.0);
        }
        if ch2 < self.num_speakers {
            self.gains[ch2] = g2_norm.clamp(0.0, 1.0);
        }
    }

    /// 3D VBAP approximation using soft-knee cos(angle) gain per speaker.
    ///
    /// For each non-LFE speaker, gain = max(0, cos(angle_between_source_and_spk)).
    /// This avoids the full Delaunay triangulation on the sphere while giving
    /// perceptually convincing results for typical object positions.
    ///
    /// After computing raw gains, they are normalized for constant power.
    fn pan_3d(&mut self, azimuth_deg: f32, elevation_deg: f32) {
        // Source unit vector
        let az = azimuth_deg.to_radians();
        let el = elevation_deg.to_radians();
        let sx = el.cos() * az.sin();
        let sy = el.sin();
        let sz = el.cos() * az.cos();

        let mut energy = 0.0_f32;

        for spk in &self.speakers_3d {
            if spk.is_lfe {
                continue;
            }
            // cos(angle) = dot product of unit vectors
            let dot = sx * spk.x + sy * spk.y + sz * spk.z;
            let g = dot.max(0.0);
            if spk.channel < self.num_speakers {
                self.gains[spk.channel] = g;
                energy += g * g;
            }
        }

        // Normalize for constant power
        if energy > 1e-12 {
            let norm = energy.sqrt();
            for g in &mut self.gains {
                *g /= norm;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speaker_config::get_speaker_config;

    fn sum_gain_energy(gains: &[f32]) -> f32 {
        gains.iter().map(|&g| g * g).sum()
    }

    #[test]
    fn test_center_speaker_at_zero_azimuth() {
        // 5.1 has a center speaker at azimuth=0
        let config = get_speaker_config("5.1").unwrap();
        let mut panner = VbapPanner::new(config.speakers, config.total_channels);

        let gains = panner.pan(0.0, 0.0);

        // Center speaker is channel 2 (azimuth=0)
        let center_gain = gains[2];
        assert!(
            center_gain > 0.9,
            "Expected center speaker (ch2) to dominate at az=0, got {center_gain:.3}"
        );

        // Energy should be ~1.0 (constant power)
        let energy = sum_gain_energy(gains);
        assert!(
            (energy - 1.0).abs() < 0.1,
            "Expected unit energy, got {energy:.3}"
        );
    }

    #[test]
    fn test_front_left_at_plus30() {
        // 5.1: FL is at azimuth=+30
        let config = get_speaker_config("5.1").unwrap();
        let mut panner = VbapPanner::new(config.speakers, config.total_channels);

        let gains = panner.pan(30.0, 0.0);

        // FL is channel 0 (azimuth=30)
        let fl_gain = gains[0];
        assert!(
            fl_gain > 0.9,
            "Expected FL (ch0) to dominate at az=+30, got {fl_gain:.3}"
        );

        let energy = sum_gain_energy(gains);
        assert!(
            (energy - 1.0).abs() < 0.1,
            "Expected unit energy, got {energy:.3}"
        );
    }

    #[test]
    fn test_front_right_at_minus30() {
        // 5.1: FR is at azimuth=-30
        let config = get_speaker_config("5.1").unwrap();
        let mut panner = VbapPanner::new(config.speakers, config.total_channels);

        let gains = panner.pan(-30.0, 0.0);

        // FR is channel 1 (azimuth=-30)
        let fr_gain = gains[1];
        assert!(
            fr_gain > 0.9,
            "Expected FR (ch1) to dominate at az=-30, got {fr_gain:.3}"
        );

        let energy = sum_gain_energy(gains);
        assert!(
            (energy - 1.0).abs() < 0.1,
            "Expected unit energy, got {energy:.3}"
        );
    }

    #[test]
    fn test_surround_right_at_minus110() {
        // 5.1: SR is at azimuth=-110
        let config = get_speaker_config("5.1").unwrap();
        let mut panner = VbapPanner::new(config.speakers, config.total_channels);

        let gains = panner.pan(-110.0, 0.0);

        // SR is channel 5 (azimuth=-110)
        let sr_gain = gains[5];
        assert!(
            sr_gain > 0.9,
            "Expected SR (ch5) to dominate at az=-110, got {sr_gain:.3}"
        );

        let energy = sum_gain_energy(gains);
        assert!(
            (energy - 1.0).abs() < 0.1,
            "Expected unit energy, got {energy:.3}"
        );
    }

    #[test]
    fn test_lfe_always_zero() {
        let config = get_speaker_config("5.1").unwrap();
        let mut panner = VbapPanner::new(config.speakers, config.total_channels);

        // LFE is channel 3 in 5.1
        for az in [-180.0_f32, -90.0, 0.0, 90.0, 180.0] {
            let gains = panner.pan(az, 0.0);
            assert_eq!(
                gains[3], 0.0,
                "LFE (ch3) should always be 0, got {} at az={az}",
                gains[3]
            );
        }
    }

    #[test]
    fn test_stereo_phantom_center() {
        // 2.0: L at +30, R at -30. Source at 0 should pan evenly.
        let config = get_speaker_config("2.0").unwrap();
        let mut panner = VbapPanner::new(config.speakers, config.total_channels);

        let gains = panner.pan(0.0, 0.0);

        // Both speakers should receive roughly equal gain
        let diff = (gains[0] - gains[1]).abs();
        assert!(
            diff < 0.05,
            "Expected phantom center (equal L/R), diff={diff:.3}"
        );

        let energy = sum_gain_energy(gains);
        assert!(
            (energy - 1.0).abs() < 0.1,
            "Expected unit energy, got {energy:.3}"
        );
    }

    #[test]
    fn test_3d_height_speaker_routing() {
        // 7.1.4 has height speakers
        let config = get_speaker_config("7.1.4").unwrap();
        let mut panner = VbapPanner::new(config.speakers, config.total_channels);

        // Source directly above (elevation=90)
        let gains = panner.pan(0.0, 90.0);

        // At least some height speaker should have non-zero gain
        let height_energy: f32 = config
            .speakers
            .iter()
            .filter(|s| s.elevation > 30.0 && !s.is_lfe)
            .map(|s| gains[s.channel] * gains[s.channel])
            .sum();
        assert!(
            height_energy > 0.5,
            "Expected height speakers to dominate at el=90, height_energy={height_energy:.3}"
        );
    }
}
