//! Sound source definitions for room acoustics simulations

use crate::types::Point3D;
use ndarray::Array2;
use serde::{Deserialize, Serialize};

/// Directivity pattern sampled on a grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectivityPattern {
    /// Horizontal angles (azimuth) in degrees [0, 360) with step 10°
    pub horizontal_angles: Vec<f64>,
    /// Vertical angles (elevation) in degrees [0, 180] with step 10°
    pub vertical_angles: Vec<f64>,
    /// Magnitude at each (horizontal, vertical) angle pair
    /// Shape: [n_vertical, n_horizontal]
    pub magnitude: Array2<f64>,
}

impl DirectivityPattern {
    /// Create omnidirectional pattern (uniform radiation)
    pub fn omnidirectional() -> Self {
        let horizontal_angles: Vec<f64> = (0..36).map(|i| i as f64 * 10.0).collect();
        let vertical_angles: Vec<f64> = (0..19).map(|i| i as f64 * 10.0).collect();

        let magnitude = Array2::ones((vertical_angles.len(), horizontal_angles.len()));

        Self {
            horizontal_angles,
            vertical_angles,
            magnitude,
        }
    }

    /// Create a cardioid directivity pattern
    pub fn cardioid() -> Self {
        let horizontal_angles: Vec<f64> = (0..36).map(|i| i as f64 * 10.0).collect();
        let vertical_angles: Vec<f64> = (0..19).map(|i| i as f64 * 10.0).collect();

        let mut magnitude = Array2::zeros((vertical_angles.len(), horizontal_angles.len()));

        for (v_idx, &v_angle) in vertical_angles.iter().enumerate() {
            for (h_idx, &h_angle) in horizontal_angles.iter().enumerate() {
                // Cardioid pattern: 0.5 * (1 + cos(theta))
                let theta_rad = v_angle.to_radians();
                let phi_rad = h_angle.to_radians();
                // Forward direction is along +Y (phi=90, theta=90)
                let forward_dot = theta_rad.sin() * phi_rad.sin();
                magnitude[[v_idx, h_idx]] = 0.5 * (1.0 + forward_dot).max(0.0);
            }
        }

        Self {
            horizontal_angles,
            vertical_angles,
            magnitude,
        }
    }

    /// Interpolate directivity at arbitrary direction
    pub fn interpolate(&self, theta: f64, phi: f64) -> f64 {
        // Convert spherical to degrees
        let theta_deg = theta.to_degrees();
        let mut phi_deg = phi.to_degrees();

        // Normalize phi to [0, 360)
        while phi_deg < 0.0 {
            phi_deg += 360.0;
        }
        while phi_deg >= 360.0 {
            phi_deg -= 360.0;
        }

        // Find surrounding angles
        let h_idx = (phi_deg / 10.0).floor() as usize;
        let v_idx = (theta_deg / 10.0).floor() as usize;

        let h_idx = h_idx.min(self.horizontal_angles.len() - 1);
        let v_idx = v_idx.min(self.vertical_angles.len() - 1);

        let h_next = (h_idx + 1) % self.horizontal_angles.len();
        let v_next = (v_idx + 1).min(self.vertical_angles.len() - 1);

        // Bilinear interpolation
        let h_frac = (phi_deg / 10.0) - h_idx as f64;
        let v_frac = (theta_deg / 10.0) - v_idx as f64;

        let m00 = self.magnitude[[v_idx, h_idx]];
        let m01 = self.magnitude[[v_idx, h_next]];
        let m10 = self.magnitude[[v_next, h_idx]];
        let m11 = self.magnitude[[v_next, h_next]];

        let m0 = m00 * (1.0 - h_frac) + m01 * h_frac;
        let m1 = m10 * (1.0 - h_frac) + m11 * h_frac;

        m0 * (1.0 - v_frac) + m1 * v_frac
    }
}

/// Crossover filter for frequency-limited sources
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum CrossoverFilter {
    /// Full range (no filter)
    #[default]
    FullRange,
    /// Low-pass filter (Butterworth)
    Lowpass {
        /// Cutoff frequency (Hz)
        cutoff_freq: f64,
        /// Filter order (e.g., 2, 4)
        order: u32,
    },
    /// High-pass filter (Butterworth)
    Highpass {
        /// Cutoff frequency (Hz)
        cutoff_freq: f64,
        /// Filter order (e.g., 2, 4)
        order: u32,
    },
    /// Band-pass filter (combined Lowpass and Highpass)
    Bandpass {
        /// Low cutoff frequency (Hz)
        low_cutoff: f64,
        /// High cutoff frequency (Hz)
        high_cutoff: f64,
        /// Filter order
        order: u32,
    },
}

impl CrossoverFilter {
    /// Get amplitude multiplier at a given frequency
    pub fn amplitude_at_frequency(&self, frequency: f64) -> f64 {
        match self {
            CrossoverFilter::FullRange => 1.0,
            CrossoverFilter::Lowpass { cutoff_freq, order } => {
                let ratio = frequency / cutoff_freq;
                1.0 / (1.0 + ratio.powi(*order as i32 * 2)).sqrt()
            }
            CrossoverFilter::Highpass { cutoff_freq, order } => {
                let ratio = cutoff_freq / frequency;
                1.0 / (1.0 + ratio.powi(*order as i32 * 2)).sqrt()
            }
            CrossoverFilter::Bandpass {
                low_cutoff,
                high_cutoff,
                order,
            } => {
                let high_ratio = low_cutoff / frequency;
                let low_ratio = frequency / high_cutoff;
                let hp_response = 1.0 / (1.0 + high_ratio.powi(*order as i32 * 2)).sqrt();
                let lp_response = 1.0 / (1.0 + low_ratio.powi(*order as i32 * 2)).sqrt();
                hp_response * lp_response
            }
        }
    }
}

/// Sound source with position and directivity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Source position
    pub position: Point3D,
    /// Directivity pattern
    pub directivity: DirectivityPattern,
    /// Source amplitude (strength)
    pub amplitude: f64,
    /// Crossover filter
    pub crossover: CrossoverFilter,
    /// Source name (e.g., "Left Main", "Subwoofer")
    pub name: String,
}

impl Source {
    /// Create a new omnidirectional source
    pub fn new(position: Point3D, directivity: DirectivityPattern, amplitude: f64) -> Self {
        Self {
            position,
            directivity,
            amplitude,
            crossover: CrossoverFilter::FullRange,
            name: String::from("Source"),
        }
    }

    /// Create a simple omnidirectional source at position
    pub fn omnidirectional(position: Point3D, amplitude: f64) -> Self {
        Self::new(position, DirectivityPattern::omnidirectional(), amplitude)
    }

    /// Set crossover filter
    pub fn with_crossover(mut self, crossover: CrossoverFilter) -> Self {
        self.crossover = crossover;
        self
    }

    /// Set source name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// Get directional amplitude towards a point at a specific frequency
    pub fn amplitude_towards(&self, point: &Point3D, frequency: f64) -> f64 {
        let dx = point.x - self.position.x;
        let dy = point.y - self.position.y;
        let dz = point.z - self.position.z;

        let r = (dx * dx + dy * dy + dz * dz).sqrt();
        if r < 1e-10 {
            return self.amplitude * self.crossover.amplitude_at_frequency(frequency);
        }

        let theta = (dz / r).acos();
        let phi = dy.atan2(dx);

        let directivity_factor = self.directivity.interpolate(theta, phi);
        let crossover_factor = self.crossover.amplitude_at_frequency(frequency);
        self.amplitude * directivity_factor * crossover_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_omnidirectional_pattern() {
        let pattern = DirectivityPattern::omnidirectional();
        // Should be 1.0 in all directions
        assert!((pattern.interpolate(0.0, 0.0) - 1.0).abs() < 1e-6);
        assert!((pattern.interpolate(PI / 2.0, PI) - 1.0).abs() < 1e-6);
        assert!((pattern.interpolate(PI, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_crossover_lowpass() {
        let crossover = CrossoverFilter::Lowpass {
            cutoff_freq: 100.0,
            order: 2,
        };
        // Well below cutoff
        assert!((crossover.amplitude_at_frequency(10.0) - 1.0).abs() < 0.1);
        // At cutoff (-3dB point)
        let at_cutoff = crossover.amplitude_at_frequency(100.0);
        assert!(at_cutoff > 0.6 && at_cutoff < 0.8);
        // Well above cutoff
        assert!(crossover.amplitude_at_frequency(1000.0) < 0.1);
    }

    #[test]
    fn test_source_amplitude() {
        let source = Source::omnidirectional(Point3D::new(0.0, 0.0, 0.0), 1.0);
        let amp = source.amplitude_towards(&Point3D::new(1.0, 0.0, 0.0), 1000.0);
        assert!((amp - 1.0).abs() < 1e-6);
    }
}
