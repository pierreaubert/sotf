//! Analytical HRTF computation
//!
//! This module provides analytical models for computing Head-Related Transfer Functions (HRTFs)
//! using simplified geometric models of the head.
//!
//! # Analytical Models
//!
//! ## Woodworth-Schlosberg Spherical Head Model
//!
//! This is a classical model that treats the head as a rigid sphere.
//! It computes:
//! - **ITD** (Interaural Time Difference): Time delay between ears
//! - **ILD** (Interaural Level Difference): Level difference due to head shadow
//!
//! ### ITD Formula
//! ```text
//! ITD = (r/c) * (θ + sin(θ))
//! ```
//! where:
//! - r = head radius (cm)
//! - c = speed of sound (343 m/s)
//! - θ = azimuth angle (radians)
//!
//! ### ILD Formula (simplified)
//! ```text
//! ILD = -20 * log10(1 + shadow_factor * |sin(θ)|)
//! ```
//!
//! # References
//! - Woodworth & Schlosberg (1954). Experimental Psychology
//! - Kuhn (1977). "Model for the interaural time differences in the azimuthal plane"

use crate::acoustics::model::AcousticHeadModel;
use crate::error::ScannerResult;
use nalgebra::{Point3, Vector3};
use std::f32::consts::PI;

/// Speed of sound in air at 20°C (cm/s)
const SPEED_OF_SOUND: f32 = 34300.0; // cm/s (343 m/s)

/// Analytical HRTF generator using Woodworth-Schlosberg sphere model
pub struct AnalyticalHRTF {
    model: AcousticHeadModel,
    sample_rate: f32,
}

impl AnalyticalHRTF {
    /// Create a new analytical HRTF generator
    pub fn new(model: AcousticHeadModel, sample_rate: f32) -> Self {
        Self {
            model,
            sample_rate,
        }
    }

    /// Compute HRTF impulse responses for a source position
    ///
    /// Returns (left_ir, right_ir) where each IR is a vector of samples
    pub fn compute_hrtf(&self, source_position: &Point3<f32>) -> (Vec<f32>, Vec<f32>) {
        // Compute azimuth and elevation relative to head center
        let (azimuth, elevation, distance) = self.compute_source_angles(source_position);

        // Compute ITD (Interaural Time Difference) in samples
        let itd_samples = self.compute_itd(azimuth);

        // Compute ILD (Interaural Level Difference) in dB
        let ild_db = self.compute_ild(azimuth, elevation);

        // Generate impulse responses
        self.generate_impulse_responses(itd_samples, ild_db, distance)
    }

    /// Compute source position in spherical coordinates relative to head
    ///
    /// Returns (azimuth, elevation, distance) in degrees/cm
    fn compute_source_angles(&self, source: &Point3<f32>) -> (f32, f32, f32) {
        // Vector from head center to source
        let to_source = source - self.model.head_center;
        let distance = to_source.norm();

        if distance < 0.01 {
            // Source at head center
            return (0.0, 0.0, 0.01);
        }

        // Azimuth: angle in horizontal plane (0° = front, 90° = left, -90° = right)
        // Using atan2(x, z) so that:
        //   z > 0, x = 0 → 0° (front)
        //   z = 0, x > 0 → 90° (left)
        //   z = 0, x < 0 → -90° (right)
        let azimuth = to_source.x.atan2(to_source.z).to_degrees();

        // Elevation: angle from horizontal plane
        let horizontal_dist = (to_source.x.powi(2) + to_source.z.powi(2)).sqrt();
        let elevation = to_source.y.atan2(horizontal_dist).to_degrees();

        (azimuth, elevation, distance)
    }

    /// Compute Interaural Time Difference (ITD) using Woodworth-Schlosberg model
    ///
    /// Returns ITD in samples
    fn compute_itd(&self, azimuth_deg: f32) -> f32 {
        let azimuth_rad = azimuth_deg.to_radians();
        let radius_m = self.model.head_radius / 100.0; // Convert cm to m

        // Woodworth-Schlosberg formula: ITD = (r/c) * (θ + sin(θ))
        let itd_seconds = (radius_m / 343.0) * (azimuth_rad + azimuth_rad.sin());

        // Convert to samples
        itd_seconds * self.sample_rate
    }

    /// Compute Interaural Level Difference (ILD) using simplified head shadow model
    ///
    /// Returns ILD in dB (negative for right ear when source is on left)
    fn compute_ild(&self, azimuth_deg: f32, elevation_deg: f32) -> f32 {
        let azimuth_rad = azimuth_deg.to_radians().abs();
        let elevation_rad = elevation_deg.to_radians();

        // Shadow factor increases with lateral angle
        let lateral_factor = azimuth_rad.sin();

        // Elevation reduces shadow effect (less shadowing from above/below)
        let elevation_factor = 1.0 - elevation_rad.abs() / (PI / 2.0) * 0.5;

        // Frequency-dependent shadowing (simplified - we use a mid-frequency approximation)
        // At 1 kHz, head shadow can be up to 20 dB for 90° azimuth
        let max_ild = 20.0; // dB

        let ild = -max_ild * lateral_factor * elevation_factor;

        // Sign convention: negative when source is on left (right ear is shadowed)
        if azimuth_deg > 0.0 {
            ild // Left side: right ear shadowed
        } else {
            -ild // Right side: left ear shadowed
        }
    }

    /// Generate impulse responses from ITD and ILD
    ///
    /// Creates simple impulse responses with:
    /// - Time delay (ITD)
    /// - Amplitude difference (ILD)
    /// - Basic head-related filtering
    fn generate_impulse_responses(
        &self,
        itd_samples: f32,
        ild_db: f32,
        distance: f32,
    ) -> (Vec<f32>, Vec<f32>) {
        // IR length: 512 samples (~11.6ms at 44.1kHz)
        let ir_length = 512;

        // Distance attenuation (1/r law)
        let distance_attenuation = 1.0 / (distance / 100.0).max(0.1); // Normalize to 1m

        // Convert ILD from dB to linear scale
        let ild_linear = 10.0f32.powf(ild_db / 20.0);

        // Initialize impulse responses
        let mut left_ir = vec![0.0f32; ir_length];
        let mut right_ir = vec![0.0f32; ir_length];

        // Compute arrival time for each ear
        let left_arrival = (ir_length / 4) as f32; // Base delay
        let right_arrival = left_arrival + itd_samples;

        // Generate impulse with simple exponential decay
        for i in 0..ir_length {
            let t = i as f32;

            // Left ear impulse
            if t >= left_arrival {
                let delta = t - left_arrival;
                let amplitude = distance_attenuation * (-delta / 100.0).exp();
                left_ir[i] = amplitude * self.apply_directional_filter(delta, true);
            }

            // Right ear impulse with ITD and ILD
            if t >= right_arrival {
                let delta = t - right_arrival;
                let amplitude = distance_attenuation * ild_linear * (-delta / 100.0).exp();
                right_ir[i] = amplitude * self.apply_directional_filter(delta, false);
            }
        }

        // Normalize
        let max_val = left_ir
            .iter()
            .chain(right_ir.iter())
            .map(|x| x.abs())
            .fold(0.0f32, f32::max);

        if max_val > 0.0 {
            for i in 0..ir_length {
                left_ir[i] /= max_val;
                right_ir[i] /= max_val;
            }
        }

        (left_ir, right_ir)
    }

    /// Apply basic directional filtering to simulate high-frequency shadowing
    fn apply_directional_filter(&self, sample_index: f32, _is_left: bool) -> f32 {
        // Simple low-pass characteristic for shadowed ear
        // In a full implementation, this would be frequency-dependent
        let decay = (-sample_index / 50.0).exp();
        decay
    }

    /// Compute full HRTF set for standard measurement grid
    ///
    /// Returns (source_positions, impulse_responses)
    /// where impulse_responses[source_idx][ear_idx][sample_idx]
    pub fn compute_hrtf_grid(
        &self,
        azimuth_resolution: usize,
        elevation_resolution: usize,
        distance: f32,
    ) -> (Vec<Point3<f32>>, Vec<Vec<Vec<f32>>>) {
        let mut source_positions = Vec::new();
        let mut impulse_responses = Vec::new();

        // Generate grid in spherical coordinates
        for elev_idx in 0..elevation_resolution {
            let elevation =
                -45.0 + (90.0 * elev_idx as f32 / (elevation_resolution - 1) as f32);

            for az_idx in 0..azimuth_resolution {
                let azimuth = -180.0 + (360.0 * az_idx as f32 / azimuth_resolution as f32);

                // Convert to Cartesian
                let source = self.spherical_to_cartesian(azimuth, elevation, distance);
                source_positions.push(source);

                // Compute HRTF
                let (left_ir, right_ir) = self.compute_hrtf(&source);
                impulse_responses.push(vec![left_ir, right_ir]);
            }
        }

        log::info!(
            "Generated HRTF grid: {} positions ({}az × {}el)",
            source_positions.len(),
            azimuth_resolution,
            elevation_resolution
        );

        (source_positions, impulse_responses)
    }

    /// Convert spherical coordinates to Cartesian relative to head center
    fn spherical_to_cartesian(&self, azimuth_deg: f32, elevation_deg: f32, radius: f32) -> Point3<f32> {
        let az_rad = azimuth_deg.to_radians();
        let el_rad = elevation_deg.to_radians();

        let x = radius * el_rad.cos() * az_rad.sin();
        let y = radius * el_rad.sin();
        let z = radius * el_rad.cos() * az_rad.cos();

        Point3::new(
            self.model.head_center.x + x,
            self.model.head_center.y + y,
            self.model.head_center.z + z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{Mesh, Triangle, Vertex};

    fn create_test_model() -> AcousticHeadModel {
        // Create a simple spherical head mesh
        let mut vertices = Vec::new();
        let radius = 9.0; // 9cm radius

        // Create vertices on a sphere
        for i in 0..20 {
            let theta = 2.0 * PI * i as f32 / 20.0;
            for j in 0..10 {
                let phi = PI * j as f32 / 10.0;
                let x = radius * phi.sin() * theta.cos();
                let y = radius * phi.cos();
                let z = radius * phi.sin() * theta.sin();

                let pos = Point3::new(x, y, z);
                let normal = Vector3::new(x, y, z).normalize();

                vertices.push(Vertex::from_point(pos).with_normal(normal));
            }
        }

        let triangles = vec![Triangle::new(0, 1, 2)]; // Dummy triangles

        let mesh = Mesh::from_parts(vertices, triangles);

        // Create model with known ear positions
        AcousticHeadModel {
            mesh,
            left_ear: Point3::new(-7.0, 0.0, 0.0),
            right_ear: Point3::new(7.0, 0.0, 0.0),
            head_center: Point3::origin(),
            head_radius: 9.0,
            dimensions: (18.0, 18.0, 18.0),
        }
    }

    #[test]
    fn test_itd_computation() {
        let model = create_test_model();
        let hrtf = AnalyticalHRTF::new(model, 44100.0);

        // Source at 90° left should produce maximum ITD
        let itd_90 = hrtf.compute_itd(90.0);
        assert!(itd_90 > 0.0);
        assert!(itd_90 < 44100.0 * 0.001); // Less than 1ms

        // Source at 0° (front) should have zero ITD
        let itd_0 = hrtf.compute_itd(0.0);
        assert!(itd_0.abs() < 0.1);

        // Source at -90° (right) should have negative ITD
        let itd_neg90 = hrtf.compute_itd(-90.0);
        assert!(itd_neg90 < 0.0);
    }

    #[test]
    fn test_ild_computation() {
        let model = create_test_model();
        let hrtf = AnalyticalHRTF::new(model, 44100.0);

        // Source at 90° left should produce positive ILD (right ear shadowed)
        let ild_90 = hrtf.compute_ild(90.0, 0.0);
        assert!(ild_90 < 0.0); // Negative because right ear is attenuated

        // Source at 0° (front) should have zero ILD
        let ild_0 = hrtf.compute_ild(0.0, 0.0);
        assert!(ild_0.abs() < 1.0); // Nearly zero

        // Source at -90° (right) should have negative ILD (left ear shadowed)
        let ild_neg90 = hrtf.compute_ild(-90.0, 0.0);
        assert!(ild_neg90 > 0.0); // Positive because left ear is attenuated
    }

    #[test]
    fn test_impulse_response_generation() {
        let model = create_test_model();
        let hrtf = AnalyticalHRTF::new(model, 44100.0);

        let source = Point3::new(0.0, 0.0, 100.0); // 1m in front
        let (left_ir, right_ir) = hrtf.compute_hrtf(&source);

        assert_eq!(left_ir.len(), 512);
        assert_eq!(right_ir.len(), 512);

        // Check normalization
        let max_left = left_ir.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let max_right = right_ir.iter().map(|x| x.abs()).fold(0.0f32, f32::max);

        assert!(max_left > 0.0 && max_left <= 1.0);
        assert!(max_right > 0.0 && max_right <= 1.0);
    }

    #[test]
    fn test_hrtf_grid_generation() {
        let model = create_test_model();
        let hrtf = AnalyticalHRTF::new(model, 44100.0);

        let (positions, irs) = hrtf.compute_hrtf_grid(36, 19, 100.0);

        // Should have 36 azimuth × 19 elevation = 684 positions
        assert_eq!(positions.len(), 36 * 19);
        assert_eq!(irs.len(), 36 * 19);

        // Each position should have 2 ears (left, right)
        assert_eq!(irs[0].len(), 2);

        // Each IR should be 512 samples
        assert_eq!(irs[0][0].len(), 512);
        assert_eq!(irs[0][1].len(), 512);
    }
}
