//! Acoustic head model for HRTF simulation
//!
//! This module provides acoustic modeling of the head geometry,
//! including ear detection and head shape characterization.

use crate::error::{ScannerError, ScannerResult};
use crate::mesh::{Mesh, Vertex};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};

/// Acoustic head model for HRTF generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticHeadModel {
    /// Original mesh
    mesh: Mesh,

    /// Left ear position (entrance to ear canal)
    pub left_ear: Point3<f32>,

    /// Right ear position (entrance to ear canal)
    pub right_ear: Point3<f32>,

    /// Head center (centroid)
    pub head_center: Point3<f32>,

    /// Head radius (bounding sphere radius)
    pub head_radius: f32,

    /// Head dimensions (width, height, depth) in cm
    pub dimensions: (f32, f32, f32),
}

impl AcousticHeadModel {
    /// Create an acoustic head model from a mesh
    ///
    /// This performs automatic ear detection and head geometry analysis
    pub fn from_mesh(mesh: &Mesh) -> ScannerResult<Self> {
        if mesh.vertex_count() < 100 {
            return Err(ScannerError::InsufficientData(
                "Mesh too small for acoustic modeling (need at least 100 vertices)".to_string(),
            ));
        }

        // Compute head center (centroid)
        let head_center = Self::compute_centroid(mesh);

        // Compute bounding box and radius
        let (bbox_min, bbox_max) = Self::compute_bounding_box(mesh);
        let dimensions = (
            (bbox_max.x - bbox_min.x).abs(),
            (bbox_max.y - bbox_min.y).abs(),
            (bbox_max.z - bbox_min.z).abs(),
        );

        let head_radius = Self::compute_bounding_radius(mesh, &head_center);

        log::info!(
            "Head geometry: center={:.1}, {:.1}, {:.1} radius={:.1}cm dimensions={:.1}x{:.1}x{:.1}cm",
            head_center.x,
            head_center.y,
            head_center.z,
            head_radius,
            dimensions.0,
            dimensions.1,
            dimensions.2
        );

        // Detect ear positions
        log::info!("Detecting ear positions...");
        let (left_ear, right_ear) = Self::detect_ear_positions(mesh, &head_center, head_radius)?;

        log::info!(
            "Ears detected: left=({:.1}, {:.1}, {:.1}) right=({:.1}, {:.1}, {:.1})",
            left_ear.x,
            left_ear.y,
            left_ear.z,
            right_ear.x,
            right_ear.y,
            right_ear.z
        );

        // Validate ear symmetry
        Self::validate_ear_symmetry(&left_ear, &right_ear, &head_center)?;

        Ok(Self {
            mesh: mesh.clone(),
            left_ear,
            right_ear,
            head_center,
            head_radius,
            dimensions,
        })
    }

    /// Compute the centroid of the mesh
    fn compute_centroid(mesh: &Mesh) -> Point3<f32> {
        let mut sum = Vector3::zeros();
        let vertices = mesh.vertices();

        for vertex in vertices {
            sum += vertex.position.coords;
        }

        Point3::from(sum / vertices.len() as f32)
    }

    /// Compute bounding box
    fn compute_bounding_box(mesh: &Mesh) -> (Point3<f32>, Point3<f32>) {
        let vertices = mesh.vertices();
        let first = vertices[0].position;

        let mut min = first;
        let mut max = first;

        for vertex in vertices.iter().skip(1) {
            let p = vertex.position;
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            min.z = min.z.min(p.z);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            max.z = max.z.max(p.z);
        }

        (min, max)
    }

    /// Compute bounding sphere radius
    fn compute_bounding_radius(mesh: &Mesh, center: &Point3<f32>) -> f32 {
        let vertices = mesh.vertices();
        let mut max_dist_sq = 0.0f32;

        for vertex in vertices {
            let dist_sq = (vertex.position - center).norm_squared();
            max_dist_sq = max_dist_sq.max(dist_sq);
        }

        max_dist_sq.sqrt()
    }

    /// Detect ear positions using geometry analysis
    ///
    /// Algorithm:
    /// 1. Split mesh into left/right sides based on center
    /// 2. Find regions with high concave curvature (ear canals)
    /// 3. Filter by expected ear height
    /// 4. Select best candidates based on curvature and position
    fn detect_ear_positions(
        mesh: &Mesh,
        center: &Point3<f32>,
        radius: f32,
    ) -> ScannerResult<(Point3<f32>, Point3<f32>)> {
        let vertices = mesh.vertices();

        // Expected ear height: between 40-60% of head height from bottom
        // Ears are typically at eye level
        let ear_height_min = center.y - radius * 0.2;
        let ear_height_max = center.y + radius * 0.2;

        // Expected ear lateral position: 70-90% of head radius from center
        let ear_lateral_min = radius * 0.6;

        // Find left ear candidates (x < center.x)
        let mut left_candidates = Vec::new();
        for (i, vertex) in vertices.iter().enumerate() {
            if vertex.position.x < center.x - ear_lateral_min
                && vertex.position.y >= ear_height_min
                && vertex.position.y <= ear_height_max
            {
                if let Some(normal) = &vertex.normal {
                    // Check for inward-pointing normal (concave region)
                    let to_center = (center - vertex.position).normalize();
                    let concavity = normal.dot(&to_center);

                    if concavity > 0.3 {
                        // Pointing somewhat toward center
                        let curvature = Self::estimate_mean_curvature_at_vertex(mesh, i);
                        left_candidates.push((vertex.position, concavity, curvature));
                    }
                }
            }
        }

        // Find right ear candidates (x > center.x)
        let mut right_candidates = Vec::new();
        for (i, vertex) in vertices.iter().enumerate() {
            if vertex.position.x > center.x + ear_lateral_min
                && vertex.position.y >= ear_height_min
                && vertex.position.y <= ear_height_max
            {
                if let Some(normal) = &vertex.normal {
                    let to_center = (center - vertex.position).normalize();
                    let concavity = normal.dot(&to_center);

                    if concavity > 0.3 {
                        let curvature = Self::estimate_mean_curvature_at_vertex(mesh, i);
                        right_candidates.push((vertex.position, concavity, curvature));
                    }
                }
            }
        }

        if left_candidates.is_empty() || right_candidates.is_empty() {
            return Err(ScannerError::InvalidConfig(
                "Could not detect ear positions - insufficient concave regions found".to_string(),
            ));
        }

        // Select best candidates based on curvature
        left_candidates.sort_by(|a, b| {
            let score_a = a.1 + a.2 * 0.5; // Concavity + weighted curvature
            let score_b = b.1 + b.2 * 0.5;
            score_b.partial_cmp(&score_a).unwrap()
        });

        right_candidates.sort_by(|a, b| {
            let score_a = a.1 + a.2 * 0.5;
            let score_b = b.1 + b.2 * 0.5;
            score_b.partial_cmp(&score_a).unwrap()
        });

        let left_ear = left_candidates[0].0;
        let right_ear = right_candidates[0].0;

        Ok((left_ear, right_ear))
    }

    /// Estimate mean curvature at a vertex (simplified)
    ///
    /// Uses local neighborhood analysis to estimate curvature
    fn estimate_mean_curvature_at_vertex(mesh: &Mesh, vertex_index: usize) -> f32 {
        let vertices = mesh.vertices();
        let vertex = &vertices[vertex_index];

        if vertex.normal.is_none() {
            return 0.0;
        }

        let normal = vertex.normal.unwrap();
        let position = vertex.position;

        // Find nearby vertices (simplified - within 2cm radius)
        let search_radius = 2.0;
        let mut curvature_sum = 0.0;
        let mut count = 0;

        for other in vertices {
            let dist = (other.position - position).norm();
            if dist > 0.01 && dist < search_radius {
                if let Some(other_normal) = &other.normal {
                    // Curvature estimate from normal variation
                    let normal_diff = (normal - other_normal).norm();
                    curvature_sum += normal_diff / dist;
                    count += 1;
                }
            }
        }

        if count > 0 {
            curvature_sum / count as f32
        } else {
            0.0
        }
    }

    /// Validate that detected ears are reasonably symmetric
    fn validate_ear_symmetry(
        left_ear: &Point3<f32>,
        right_ear: &Point3<f32>,
        center: &Point3<f32>,
    ) -> ScannerResult<()> {
        // Check that ears are on opposite sides
        if (left_ear.x - center.x) * (right_ear.x - center.x) > 0.0 {
            return Err(ScannerError::InvalidConfig(
                "Ears detected on same side of head".to_string(),
            ));
        }

        // Check Y symmetry (should be at similar heights)
        let y_diff = (left_ear.y - right_ear.y).abs();
        if y_diff > 5.0 {
            // More than 5cm difference
            log::warn!("Ears have significant height asymmetry: {:.1}cm", y_diff);
        }

        // Check Z symmetry (should be at similar depth)
        let z_diff = (left_ear.z - right_ear.z).abs();
        if z_diff > 5.0 {
            log::warn!("Ears have significant depth asymmetry: {:.1}cm", z_diff);
        }

        // Check approximate symmetry in distance from center
        let left_dist = (left_ear - center).norm();
        let right_dist = (right_ear - center).norm();
        let dist_ratio = left_dist / right_dist;

        if dist_ratio < 0.8 || dist_ratio > 1.2 {
            log::warn!(
                "Ears have asymmetric distance from center: left={:.1}cm right={:.1}cm",
                left_dist,
                right_dist
            );
        }

        Ok(())
    }

    /// Get the mesh
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    /// Get interaural distance (distance between ears)
    pub fn interaural_distance(&self) -> f32 {
        (self.left_ear - self.right_ear).norm()
    }

    /// Get ear positions as array for SOFA export
    pub fn ear_positions_array(&self) -> [[f32; 3]; 2] {
        [
            [self.left_ear.x, self.left_ear.y, self.left_ear.z],
            [self.right_ear.x, self.right_ear.y, self.right_ear.z],
        ]
    }

    /// Project a point onto the head surface (nearest point on mesh)
    ///
    /// Used for visualizing source positions relative to head
    pub fn project_to_surface(&self, point: &Point3<f32>) -> Point3<f32> {
        let vertices = self.mesh.vertices();
        let mut min_dist = f32::MAX;
        let mut nearest = vertices[0].position;

        for vertex in vertices {
            let dist = (vertex.position - point).norm_squared();
            if dist < min_dist {
                min_dist = dist;
                nearest = vertex.position;
            }
        }

        nearest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Triangle;

    #[test]
    fn test_centroid() {
        // Create a simple cube mesh
        let vertices = vec![
            Vertex::new(0.0, 0.0, 0.0),
            Vertex::new(10.0, 0.0, 0.0),
            Vertex::new(0.0, 10.0, 0.0),
            Vertex::new(10.0, 10.0, 0.0),
        ];

        let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(1, 3, 2)];

        let mesh = Mesh::from_parts(vertices, triangles);
        let centroid = AcousticHeadModel::compute_centroid(&mesh);

        // Centroid should be at (5, 5, 0)
        assert!((centroid.x - 5.0).abs() < 0.1);
        assert!((centroid.y - 5.0).abs() < 0.1);
        assert!((centroid.z - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_bounding_box() {
        let vertices = vec![
            Vertex::new(-5.0, -5.0, -5.0),
            Vertex::new(5.0, 10.0, 3.0),
            Vertex::new(0.0, 0.0, 0.0),
        ];

        let triangles = vec![Triangle::new(0, 1, 2)];
        let mesh = Mesh::from_parts(vertices, triangles);

        let (min, max) = AcousticHeadModel::compute_bounding_box(&mesh);

        assert_eq!(min.x, -5.0);
        assert_eq!(min.y, -5.0);
        assert_eq!(min.z, -5.0);
        assert_eq!(max.x, 5.0);
        assert_eq!(max.y, 10.0);
        assert_eq!(max.z, 3.0);
    }
}
