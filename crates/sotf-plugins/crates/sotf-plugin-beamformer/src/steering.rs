// ============================================================================
// Steering Vector Computation
// ============================================================================
//
// Computes steering vectors for microphone arrays. A steering vector encodes
// the phase delays for a plane wave arriving from a specific direction.

use nalgebra::Complex;

/// Microphone array geometry.
#[derive(Debug, Clone)]
pub enum ArrayGeometry {
    /// Linear array with uniform spacing
    Linear {
        /// Number of microphones
        num_mics: usize,
        /// Spacing between adjacent microphones in meters
        spacing_m: f32,
    },
    /// Circular array
    Circular {
        /// Number of microphones
        num_mics: usize,
        /// Radius in meters
        radius_m: f32,
    },
    /// Arbitrary mic positions
    Custom {
        /// (x, y, z) positions in meters
        positions: Vec<(f32, f32, f32)>,
    },
}

impl ArrayGeometry {
    /// Get microphone positions as (x, y, z) tuples.
    pub fn positions(&self) -> Vec<(f32, f32, f32)> {
        match self {
            Self::Linear {
                num_mics,
                spacing_m,
            } => (0..*num_mics)
                .map(|i| (i as f32 * spacing_m, 0.0, 0.0))
                .collect(),
            Self::Circular { num_mics, radius_m } => {
                let two_pi = 2.0 * std::f32::consts::PI;
                (0..*num_mics)
                    .map(|i| {
                        let angle = two_pi * i as f32 / *num_mics as f32;
                        (radius_m * angle.cos(), radius_m * angle.sin(), 0.0)
                    })
                    .collect()
            }
            Self::Custom { positions } => positions.clone(),
        }
    }

    pub fn num_mics(&self) -> usize {
        match self {
            Self::Linear { num_mics, .. } => *num_mics,
            Self::Circular { num_mics, .. } => *num_mics,
            Self::Custom { positions } => positions.len(),
        }
    }
}

const SPEED_OF_SOUND: f32 = 343.0; // m/s at 20°C

/// Compute the steering vector for a plane wave from a given direction.
///
/// # Arguments
/// * `freq_hz` - Frequency in Hz
/// * `geometry` - Microphone array geometry
/// * `azimuth_deg` - Steering azimuth angle in degrees.
///   Convention: for a linear array along the x-axis, 0° is **broadside**
///   (wave arrives perpendicular to the array) and ±90° is **endfire**
///   (wave travels along the array axis).
/// * `elevation_deg` - Steering elevation angle in degrees (0° = horizontal)
///
/// # Returns
/// Complex steering vector of length `num_mics`
pub fn compute_steering_vector(
    freq_hz: f32,
    geometry: &ArrayGeometry,
    azimuth_deg: f32,
    elevation_deg: f32,
) -> Vec<Complex<f32>> {
    let positions = geometry.positions();
    let two_pi = 2.0 * std::f32::consts::PI;
    let az = azimuth_deg.to_radians();
    let el = elevation_deg.to_radians();

    // Unit direction vector: 0° azimuth = broadside (y-axis)
    // 90° azimuth = endfire (x-axis)
    let dx = el.cos() * az.sin();
    let dy = el.cos() * az.cos();
    let dz = el.sin();

    positions
        .iter()
        .map(|&(x, y, z)| {
            // Time delay for this microphone
            let tau = (x * dx + y * dy + z * dz) / SPEED_OF_SOUND;
            // Phase shift
            let phase = -two_pi * freq_hz * tau;
            Complex::new(phase.cos(), phase.sin())
        })
        .collect()
}

/// Compute steering vectors for all frequency bins.
///
/// # Arguments
/// * `geometry` - Microphone array geometry
/// * `azimuth_deg` - Steering direction
/// * `fft_size` - FFT size
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// Steering vectors indexed by [bin][mic]
pub fn compute_all_steering_vectors(
    geometry: &ArrayGeometry,
    azimuth_deg: f32,
    fft_size: usize,
    sample_rate: f32,
) -> Vec<Vec<Complex<f32>>> {
    let spectrum_size = fft_size / 2 + 1;
    (0..spectrum_size)
        .map(|k| {
            let freq = k as f32 * sample_rate / fft_size as f32;
            compute_steering_vector(freq, geometry, azimuth_deg, 0.0)
        })
        .collect()
}

/// Compute per-microphone compensation delays for a given steering direction.
///
/// Returns the delay in **samples** that each microphone signal should be
/// delayed so that signals from the look direction are time-aligned.
///
/// # Arguments
/// * `geometry` - Microphone array geometry
/// * `azimuth_deg` - Steering direction in degrees (0° = broadside)
/// * `elevation_deg` - Elevation in degrees (0° = horizontal)
/// * `sample_rate` - Sample rate in Hz
pub fn compute_steering_delays(
    geometry: &ArrayGeometry,
    azimuth_deg: f32,
    elevation_deg: f32,
    sample_rate: f32,
) -> Vec<f32> {
    let positions = geometry.positions();
    let az = azimuth_deg.to_radians();
    let el = elevation_deg.to_radians();

    let dx = el.cos() * az.sin();
    let dy = el.cos() * az.cos();
    let dz = el.sin();

    let propagation_delays: Vec<f32> = positions
        .iter()
        .map(|&(x, y, z)| (x * dx + y * dy + z * dz) / SPEED_OF_SOUND * sample_rate)
        .collect();

    let max_delay = propagation_delays
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    propagation_delays.iter().map(|d| max_delay - d).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_array_positions() {
        let geom = ArrayGeometry::Linear {
            num_mics: 4,
            spacing_m: 0.05,
        };
        let pos = geom.positions();
        assert_eq!(pos.len(), 4);
        assert!((pos[0].0 - 0.0).abs() < 1e-6);
        assert!((pos[1].0 - 0.05).abs() < 1e-6);
        assert!((pos[3].0 - 0.15).abs() < 1e-6);
    }

    #[test]
    fn test_broadside_steering() {
        let geom = ArrayGeometry::Linear {
            num_mics: 2,
            spacing_m: 0.05,
        };

        // At broadside (0°), both mics receive the wave simultaneously
        // → all steering vector phases should be zero (unit magnitude)
        let sv = compute_steering_vector(1000.0, &geom, 0.0, 0.0);
        // At 90°, wave travels along array axis → different delays
        assert_eq!(sv.len(), 2);
        // Both elements should be real (zero phase delay at broadside)
        for c in &sv {
            assert!(
                c.im.abs() < 1e-4,
                "Expected near-zero imag at broadside, got {c}"
            );
        }
    }

    #[test]
    fn test_endfire_steering() {
        let geom = ArrayGeometry::Linear {
            num_mics: 2,
            spacing_m: 0.05,
        };

        // At endfire (90°), the wave travels along the array axis, so mics
        // receive it at different times → phases should differ.
        let sv = compute_steering_vector(1000.0, &geom, 90.0, 0.0);
        assert_eq!(sv.len(), 2);
        // The first mic is at origin → zero phase; second mic has non-zero phase
        assert!(
            sv[0].im.abs() < 1e-6,
            "mic 0 at origin should have zero phase"
        );
        // At non-trivial frequency the second mic should have a different phase
        let phase_diff = (sv[1].re - sv[0].re).abs() + (sv[1].im - sv[0].im).abs();
        assert!(
            phase_diff > 0.01,
            "Endfire steering should produce phase delay: {phase_diff}"
        );
    }

    #[test]
    fn test_steering_vector_unit_magnitude() {
        let geom = ArrayGeometry::Linear {
            num_mics: 4,
            spacing_m: 0.04,
        };

        let sv = compute_steering_vector(2000.0, &geom, 45.0, 0.0);
        for (i, c) in sv.iter().enumerate() {
            let mag = (c.re * c.re + c.im * c.im).sqrt();
            assert!(
                (mag - 1.0).abs() < 1e-5,
                "Steering vector at mic {i} should have unit magnitude, got {mag}"
            );
        }
    }

    #[test]
    fn test_angle_convention_broadside() {
        let geom = ArrayGeometry::Linear {
            num_mics: 2,
            spacing_m: 0.05,
        };

        // 0° should be broadside: both mics in phase
        let sv_broadside = compute_steering_vector(1000.0, &geom, 0.0, 0.0);
        let phase_diff = (sv_broadside[1].im.atan2(sv_broadside[1].re)
            - sv_broadside[0].im.atan2(sv_broadside[0].re))
        .abs();
        assert!(
            phase_diff < 1e-5,
            "0° should be broadside (phase_diff={phase_diff})"
        );

        // 90° should be endfire: phase difference proportional to spacing
        let sv_endfire = compute_steering_vector(1000.0, &geom, 90.0, 0.0);
        let phase_diff_endfire = (sv_endfire[1].im.atan2(sv_endfire[1].re)
            - sv_endfire[0].im.atan2(sv_endfire[0].re))
        .abs();
        let expected = 2.0 * std::f32::consts::PI * 1000.0 * 0.05 / SPEED_OF_SOUND;
        assert!(
            (phase_diff_endfire - expected).abs() < 1e-4,
            "90° should be endfire (phase_diff={phase_diff_endfire}, expected={expected})"
        );
    }

    #[test]
    fn test_steering_delays() {
        let geom = ArrayGeometry::Linear {
            num_mics: 2,
            spacing_m: 0.05,
        };

        // Broadside: zero delay difference
        let delays = compute_steering_delays(&geom, 0.0, 0.0, 48000.0);
        assert_eq!(delays.len(), 2);
        assert!(
            (delays[0]).abs() < 1e-4 && (delays[1]).abs() < 1e-4,
            "Broadside delays should be near zero: {:?}",
            delays
        );

        // Endfire: one mic delayed relative to the other
        let delays_endfire = compute_steering_delays(&geom, 90.0, 0.0, 48000.0);
        let expected = (0.05 / SPEED_OF_SOUND * 48000.0).abs();
        assert!(
            (delays_endfire[0] - expected).abs() < 1e-3,
            "Endfire delay mismatch: expected {expected}, got {}",
            delays_endfire[0]
        );
        assert!(
            delays_endfire[1].abs() < 1e-4,
            "Endfire second mic delay should be near zero: {}",
            delays_endfire[1]
        );
    }
}
