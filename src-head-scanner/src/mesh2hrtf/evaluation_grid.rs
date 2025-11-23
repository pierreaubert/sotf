//! Evaluation grid generation for HRTF measurements
//!
//! This module provides functions to generate evaluation grids where the
//! sound pressure field will be computed during BEM simulation.
//!
//! # Grid Types
//!
//! - **Spherical grids**: Points distributed uniformly on a sphere around the head
//! - **Horizontal plane**: Circular grid at ear height (z=0)
//! - **Vertical plane**: Semicircular grid in the median plane
//!
//! # Usage
//!
//! ```rust,no_run
//! use head_scanner::mesh2hrtf::{GridGenerator, GridType};
//!
//! // Generate spherical grid with 72 points at 1.5m radius
//! let grid = GridGenerator::generate_sphere(1.5, 72)?;
//!
//! // Generate horizontal plane grid
//! let grid = GridGenerator::generate_horizontal_plane(1.5, 0.0, 36)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use super::types::{EvaluationGrid, GridType, Node, Point};
use anyhow::Result;
use std::f64::consts::PI;

/// Grid generator for evaluation grids
pub struct GridGenerator;

impl GridGenerator {
    /// Generate a spherical evaluation grid
    ///
    /// Creates points uniformly distributed on a sphere using the Fibonacci
    /// sphere algorithm, which provides good uniform distribution.
    ///
    /// # Arguments
    ///
    /// * `radius` - Radius of the sphere (meters)
    /// * `num_points` - Number of evaluation points
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use head_scanner::mesh2hrtf::GridGenerator;
    ///
    /// // Generate 72 points on a 1.5m radius sphere
    /// let grid = GridGenerator::generate_sphere(1.5, 72)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn generate_sphere(radius: f64, num_points: usize) -> Result<EvaluationGrid> {
        if radius <= 0.0 {
            anyhow::bail!("Radius must be positive");
        }
        if num_points == 0 {
            anyhow::bail!("Number of points must be positive");
        }

        let mut nodes = Vec::with_capacity(num_points);

        // Fibonacci sphere algorithm for uniform distribution
        let golden_ratio = (1.0 + 5.0_f64.sqrt()) / 2.0;
        let angle_increment = 2.0 * PI * golden_ratio;

        for i in 0..num_points {
            // Vertical position: uniform distribution from -1 to 1
            let y = 1.0 - (2.0 * i as f64) / (num_points - 1) as f64;

            // Radius at this height
            let r_at_height = (1.0 - y * y).sqrt();

            // Azimuthal angle
            let theta = angle_increment * i as f64;

            // Convert to Cartesian coordinates
            let x = r_at_height * theta.cos() * radius;
            let z = r_at_height * theta.sin() * radius;
            let y = y * radius;

            nodes.push(Node::new(i, Point::new(x, y, z)));
        }

        Ok(EvaluationGrid {
            name: format!("Sphere_r{:.2}_n{}", radius, num_points),
            grid_type: GridType::Sphere {
                radius,
                points: num_points,
            },
            nodes,
            elements: Vec::new(), // Spherical grids are point clouds
        })
    }

    /// Generate a spherical grid with specific angular resolution
    ///
    /// Creates points on a sphere with specified azimuth and elevation steps.
    ///
    /// # Arguments
    ///
    /// * `radius` - Radius of the sphere (meters)
    /// * `azimuth_steps` - Number of steps in azimuth (0-360°)
    /// * `elevation_steps` - Number of steps in elevation (-90° to +90°)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use head_scanner::mesh2hrtf::GridGenerator;
    ///
    /// // 36 azimuth steps (10° each), 19 elevation steps (~10° each)
    /// let grid = GridGenerator::generate_sphere_angular(1.5, 36, 19)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn generate_sphere_angular(
        radius: f64,
        azimuth_steps: usize,
        elevation_steps: usize,
    ) -> Result<EvaluationGrid> {
        if radius <= 0.0 {
            anyhow::bail!("Radius must be positive");
        }
        if azimuth_steps == 0 || elevation_steps == 0 {
            anyhow::bail!("Number of steps must be positive");
        }

        let mut nodes = Vec::new();
        let mut node_id = 0;

        // Elevation from -90° to +90°
        for elev_idx in 0..elevation_steps {
            let elevation = -PI / 2.0 + (elev_idx as f64 * PI) / (elevation_steps - 1) as f64;
            let cos_elev = elevation.cos();
            let sin_elev = elevation.sin();

            // Azimuth from 0° to 360° (excluding 360° as it's same as 0°)
            let azimuth_count = if elev_idx == 0 || elev_idx == elevation_steps - 1 {
                // Poles: only one point
                1
            } else {
                azimuth_steps
            };

            for azim_idx in 0..azimuth_count {
                let azimuth = (azim_idx as f64 * 2.0 * PI) / azimuth_steps as f64;

                // Spherical to Cartesian: (r, θ, φ) → (x, y, z)
                // x = r cos(elevation) cos(azimuth)
                // y = r sin(elevation)
                // z = r cos(elevation) sin(azimuth)
                let x = radius * cos_elev * azimuth.cos();
                let y = radius * sin_elev;
                let z = radius * cos_elev * azimuth.sin();

                nodes.push(Node::new(node_id, Point::new(x, y, z)));
                node_id += 1;
            }
        }

        Ok(EvaluationGrid {
            name: format!(
                "Sphere_r{:.2}_az{}_el{}",
                radius, azimuth_steps, elevation_steps
            ),
            grid_type: GridType::Sphere {
                radius,
                points: nodes.len(),
            },
            nodes,
            elements: Vec::new(),
        })
    }

    /// Generate a horizontal plane evaluation grid
    ///
    /// Creates a circular grid at a specified height, typically at ear level (z=0).
    ///
    /// # Arguments
    ///
    /// * `radius` - Maximum radius of the grid (meters)
    /// * `z_height` - Height of the plane (meters, typically 0 for ear level)
    /// * `num_points` - Number of points around the circle
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use head_scanner::mesh2hrtf::GridGenerator;
    ///
    /// // 36 points in a circle at ear level
    /// let grid = GridGenerator::generate_horizontal_plane(1.5, 0.0, 36)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn generate_horizontal_plane(
        radius: f64,
        z_height: f64,
        num_points: usize,
    ) -> Result<EvaluationGrid> {
        if radius <= 0.0 {
            anyhow::bail!("Radius must be positive");
        }
        if num_points == 0 {
            anyhow::bail!("Number of points must be positive");
        }

        let mut nodes = Vec::with_capacity(num_points);

        for i in 0..num_points {
            let angle = (i as f64 * 2.0 * PI) / num_points as f64;
            let x = radius * angle.cos();
            let y = radius * angle.sin();
            let z = z_height;

            nodes.push(Node::new(i, Point::new(x, y, z)));
        }

        Ok(EvaluationGrid {
            name: format!(
                "HorizontalPlane_r{:.2}_z{:.2}_n{}",
                radius, z_height, num_points
            ),
            grid_type: GridType::HorizontalPlane {
                z: z_height,
                radius,
                points: num_points,
            },
            nodes,
            elements: Vec::new(),
        })
    }

    /// Generate a vertical plane evaluation grid
    ///
    /// Creates a semicircular grid in a vertical plane at a specified azimuth angle.
    /// Typically used for median plane (0°) or lateral plane (90°) measurements.
    ///
    /// # Arguments
    ///
    /// * `radius` - Radius of the semicircle (meters)
    /// * `azimuth` - Azimuth angle of the plane (radians, 0 = front, π/2 = right)
    /// * `num_points` - Number of points along the semicircle
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use head_scanner::mesh2hrtf::GridGenerator;
    /// use std::f64::consts::PI;
    ///
    /// // Median plane (front-back), 19 points
    /// let grid = GridGenerator::generate_vertical_plane(1.5, 0.0, 19)?;
    ///
    /// // Lateral plane (left-right), 19 points
    /// let grid = GridGenerator::generate_vertical_plane(1.5, PI/2.0, 19)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn generate_vertical_plane(
        radius: f64,
        azimuth: f64,
        num_points: usize,
    ) -> Result<EvaluationGrid> {
        if radius <= 0.0 {
            anyhow::bail!("Radius must be positive");
        }
        if num_points < 2 {
            anyhow::bail!("Need at least 2 points for a semicircle");
        }

        let mut nodes = Vec::with_capacity(num_points);

        // Elevation from -90° to +90° (semicircle)
        for i in 0..num_points {
            let elevation = -PI / 2.0 + (i as f64 * PI) / (num_points - 1) as f64;

            // Position in the vertical plane
            let horizontal_dist = radius * elevation.cos();
            let z_pos = radius * elevation.sin();

            // Rotate around z-axis by azimuth angle
            let x = horizontal_dist * azimuth.cos();
            let y = horizontal_dist * azimuth.sin();
            let z = z_pos;

            nodes.push(Node::new(i, Point::new(x, y, z)));
        }

        Ok(EvaluationGrid {
            name: format!(
                "VerticalPlane_r{:.2}_az{:.1}_n{}",
                radius,
                azimuth.to_degrees(),
                num_points
            ),
            grid_type: GridType::VerticalPlane {
                angle: azimuth,
                radius,
                points: num_points,
            },
            nodes,
            elements: Vec::new(),
        })
    }

    /// Generate a multi-ring horizontal grid
    ///
    /// Creates concentric circles at different radii, useful for near-field measurements.
    ///
    /// # Arguments
    ///
    /// * `radii` - Vector of radii for each ring (meters)
    /// * `z_height` - Height of the plane (meters)
    /// * `points_per_ring` - Number of points per ring
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use head_scanner::mesh2hrtf::GridGenerator;
    ///
    /// // 3 rings at different distances
    /// let grid = GridGenerator::generate_multi_ring_plane(
    ///     vec![0.5, 1.0, 1.5],
    ///     0.0,
    ///     36
    /// )?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn generate_multi_ring_plane(
        radii: Vec<f64>,
        z_height: f64,
        points_per_ring: usize,
    ) -> Result<EvaluationGrid> {
        if radii.is_empty() {
            anyhow::bail!("Must specify at least one radius");
        }
        if points_per_ring == 0 {
            anyhow::bail!("Points per ring must be positive");
        }

        let mut nodes = Vec::new();
        let mut node_id = 0;

        for &radius in &radii {
            if radius <= 0.0 {
                anyhow::bail!("All radii must be positive");
            }

            for i in 0..points_per_ring {
                let angle = (i as f64 * 2.0 * PI) / points_per_ring as f64;
                let x = radius * angle.cos();
                let y = radius * angle.sin();
                let z = z_height;

                nodes.push(Node::new(node_id, Point::new(x, y, z)));
                node_id += 1;
            }
        }

        Ok(EvaluationGrid {
            name: format!("MultiRing_{}_rings_z{:.2}", radii.len(), z_height),
            grid_type: GridType::HorizontalPlane {
                z: z_height,
                radius: *radii
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap(),
                points: nodes.len(),
            },
            nodes,
            elements: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sphere() {
        let grid = GridGenerator::generate_sphere(1.0, 100).unwrap();
        assert_eq!(grid.nodes.len(), 100);

        // All points should be approximately on the sphere
        for node in &grid.nodes {
            let r = (node.position.x.powi(2) + node.position.y.powi(2) + node.position.z.powi(2))
                .sqrt();
            assert!((r - 1.0).abs() < 1e-10, "Point not on sphere: r={}", r);
        }
    }

    #[test]
    fn test_generate_sphere_angular() {
        let grid = GridGenerator::generate_sphere_angular(1.5, 36, 19).unwrap();

        // Check all points are on the sphere
        for node in &grid.nodes {
            let r = (node.position.x.powi(2) + node.position.y.powi(2) + node.position.z.powi(2))
                .sqrt();
            assert!((r - 1.5).abs() < 1e-10, "Point not on sphere: r={}", r);
        }

        // Should have poles + intermediate rings
        // Poles: 2 points (1 at each)
        // Intermediate: 17 rings * 36 points
        let expected = 2 + (17 * 36);
        assert_eq!(grid.nodes.len(), expected);
    }

    #[test]
    fn test_generate_horizontal_plane() {
        let grid = GridGenerator::generate_horizontal_plane(1.5, 0.0, 36).unwrap();
        assert_eq!(grid.nodes.len(), 36);

        // All points should be at z=0 and radius=1.5
        for node in &grid.nodes {
            assert!((node.position.z - 0.0).abs() < 1e-10);
            let r = (node.position.x.powi(2) + node.position.y.powi(2)).sqrt();
            assert!((r - 1.5).abs() < 1e-10);
        }
    }

    #[test]
    fn test_generate_vertical_plane() {
        let grid = GridGenerator::generate_vertical_plane(1.0, 0.0, 19).unwrap();
        assert_eq!(grid.nodes.len(), 19);

        // All points should be in the x-z plane (y≈0) and on sphere
        for node in &grid.nodes {
            assert!(node.position.y.abs() < 1e-10, "Not in x-z plane");
            let r = (node.position.x.powi(2) + node.position.y.powi(2) + node.position.z.powi(2))
                .sqrt();
            assert!((r - 1.0).abs() < 1e-10, "Not on sphere");
        }

        // Should span from bottom to top
        let z_values: Vec<f64> = grid.nodes.iter().map(|n| n.position.z).collect();
        let min_z = z_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_z = z_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        assert!((min_z + 1.0).abs() < 1e-10, "Bottom not at -1");
        assert!((max_z - 1.0).abs() < 1e-10, "Top not at +1");
    }

    #[test]
    fn test_multi_ring_plane() {
        let grid = GridGenerator::generate_multi_ring_plane(vec![0.5, 1.0, 1.5], 0.0, 12).unwrap();
        assert_eq!(grid.nodes.len(), 3 * 12); // 3 rings, 12 points each

        // All points at z=0
        for node in &grid.nodes {
            assert!((node.position.z - 0.0).abs() < 1e-10);
        }

        // Count points at each radius
        let mut count_r05 = 0;
        let mut count_r10 = 0;
        let mut count_r15 = 0;

        for node in &grid.nodes {
            let r = (node.position.x.powi(2) + node.position.y.powi(2)).sqrt();
            if (r - 0.5).abs() < 1e-10 {
                count_r05 += 1;
            } else if (r - 1.0).abs() < 1e-10 {
                count_r10 += 1;
            } else if (r - 1.5).abs() < 1e-10 {
                count_r15 += 1;
            }
        }

        assert_eq!(count_r05, 12);
        assert_eq!(count_r10, 12);
        assert_eq!(count_r15, 12);
    }

    #[test]
    fn test_invalid_parameters() {
        assert!(GridGenerator::generate_sphere(-1.0, 10).is_err());
        assert!(GridGenerator::generate_sphere(1.0, 0).is_err());
        assert!(GridGenerator::generate_horizontal_plane(0.0, 0.0, 10).is_err());
        assert!(GridGenerator::generate_vertical_plane(1.0, 0.0, 1).is_err());
    }
}
