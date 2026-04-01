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
/// * `azimuth_deg` - Steering azimuth angle in degrees (0° = broadside)
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

    // Unit direction vector
    let dx = el.cos() * az.cos();
    let dy = el.cos() * az.sin();
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
        let sv = compute_steering_vector(1000.0, &geom, 90.0, 0.0);
        // At 90°, wave travels along array axis → different delays
        assert_eq!(sv.len(), 2);
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
}
