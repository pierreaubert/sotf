//! Coverage tracking for head scanning
//!
//! This module tracks which parts of the head have been scanned by maintaining
//! a 3D voxel grid. It provides feedback to the user about areas that need more coverage.

use crate::pointcloud::Point;
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Voxel grid for tracking scan coverage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageMap {
    /// Voxel size in world units (cm)
    voxel_size: f32,

    /// Set of occupied voxels (using voxel coordinates)
    occupied: HashSet<VoxelCoord>,

    /// Bounding box minimum
    bbox_min: Option<Point3<f32>>,

    /// Bounding box maximum
    bbox_max: Option<Point3<f32>>,

    /// Target number of voxels for complete coverage
    target_voxel_count: usize,
}

/// 3D voxel coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct VoxelCoord {
    x: i32,
    y: i32,
    z: i32,
}

impl VoxelCoord {
    fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    fn from_point(point: &Point3<f32>, voxel_size: f32) -> Self {
        Self {
            x: (point.x / voxel_size).floor() as i32,
            y: (point.y / voxel_size).floor() as i32,
            z: (point.z / voxel_size).floor() as i32,
        }
    }

    fn to_point(&self, voxel_size: f32) -> Point3<f32> {
        Point3::new(
            self.x as f32 * voxel_size,
            self.y as f32 * voxel_size,
            self.z as f32 * voxel_size,
        )
    }
}

impl CoverageMap {
    /// Create a new coverage map with default voxel size (0.5 cm)
    pub fn new() -> Self {
        Self::with_voxel_size(0.5)
    }

    /// Create a new coverage map with specified voxel size
    pub fn with_voxel_size(voxel_size: f32) -> Self {
        Self {
            voxel_size,
            occupied: HashSet::new(),
            bbox_min: None,
            bbox_max: None,
            target_voxel_count: 10000, // Estimated for average head
        }
    }

    /// Update coverage with new points
    pub fn update(&mut self, points: &[Point]) {
        for point in points {
            self.add_point(&point.position);
        }
    }

    /// Add a single point to the coverage map
    pub fn add_point(&mut self, point: &Point3<f32>) {
        let voxel = VoxelCoord::from_point(point, self.voxel_size);
        self.occupied.insert(voxel);

        // Update bounding box
        self.update_bbox(point);
    }

    /// Get the coverage percentage (0.0 to 1.0)
    pub fn get_coverage_percentage(&self) -> f32 {
        if self.target_voxel_count == 0 {
            return 0.0;
        }

        (self.occupied.len() as f32 / self.target_voxel_count as f32).min(1.0)
    }

    /// Get the number of covered voxels
    pub fn covered_voxel_count(&self) -> usize {
        self.occupied.len()
    }

    /// Set the target voxel count for complete coverage
    pub fn set_target_voxel_count(&mut self, count: usize) {
        self.target_voxel_count = count;
    }

    /// Check if a point is covered
    pub fn is_covered(&self, point: &Point3<f32>) -> bool {
        let voxel = VoxelCoord::from_point(point, self.voxel_size);
        self.occupied.contains(&voxel)
    }

    /// Get regions that need more coverage
    ///
    /// Returns a list of points representing centers of uncovered regions
    pub fn get_uncovered_regions(&self) -> Vec<Point3<f32>> {
        if self.bbox_min.is_none() || self.bbox_max.is_none() {
            return Vec::new();
        }

        let bbox_min = self.bbox_min.unwrap();
        let bbox_max = self.bbox_max.unwrap();

        let mut uncovered = Vec::new();

        // Create a spherical region representing the head
        let center = Point3::new(
            (bbox_min.x + bbox_max.x) / 2.0,
            (bbox_min.y + bbox_max.y) / 2.0,
            (bbox_min.z + bbox_max.z) / 2.0,
        );

        let radius = ((bbox_max.x - bbox_min.x).max(bbox_max.y - bbox_min.y).max(bbox_max.z - bbox_min.z)) / 2.0;

        // Sample points on a sphere and check coverage
        let samples = 100;
        for i in 0..samples {
            let phi = std::f32::consts::PI * i as f32 / samples as f32;
            for j in 0..samples {
                let theta = 2.0 * std::f32::consts::PI * j as f32 / samples as f32;

                let x = center.x + radius * phi.sin() * theta.cos();
                let y = center.y + radius * phi.sin() * theta.sin();
                let z = center.z + radius * phi.cos();

                let point = Point3::new(x, y, z);

                if !self.is_covered(&point) {
                    uncovered.push(point);
                }
            }
        }

        uncovered
    }

    /// Get coverage heatmap data for visualization
    ///
    /// Returns (voxel_centers, coverage_values) where coverage_values indicate
    /// the density of points in each voxel region
    pub fn get_heatmap(&self, resolution: usize) -> (Vec<Point3<f32>>, Vec<f32>) {
        if self.bbox_min.is_none() || self.bbox_max.is_none() {
            return (Vec::new(), Vec::new());
        }

        let bbox_min = self.bbox_min.unwrap();
        let bbox_max = self.bbox_max.unwrap();

        let mut centers = Vec::new();
        let mut values = Vec::new();

        let step_x = (bbox_max.x - bbox_min.x) / resolution as f32;
        let step_y = (bbox_max.y - bbox_min.y) / resolution as f32;
        let step_z = (bbox_max.z - bbox_min.z) / resolution as f32;

        for i in 0..resolution {
            for j in 0..resolution {
                for k in 0..resolution {
                    let x = bbox_min.x + i as f32 * step_x;
                    let y = bbox_min.y + j as f32 * step_y;
                    let z = bbox_min.z + k as f32 * step_z;

                    let point = Point3::new(x, y, z);
                    let coverage = if self.is_covered(&point) { 1.0 } else { 0.0 };

                    centers.push(point);
                    values.push(coverage);
                }
            }
        }

        (centers, values)
    }

    /// Reset the coverage map
    pub fn reset(&mut self) {
        self.occupied.clear();
        self.bbox_min = None;
        self.bbox_max = None;
    }

    /// Export coverage data to JSON
    pub fn export_json(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import coverage data from JSON
    pub fn import_json(path: &str) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let coverage: Self = serde_json::from_str(&json)?;
        Ok(coverage)
    }

    // Private helper methods

    fn update_bbox(&mut self, point: &Point3<f32>) {
        match (&mut self.bbox_min, &mut self.bbox_max) {
            (Some(min), Some(max)) => {
                min.x = min.x.min(point.x);
                min.y = min.y.min(point.y);
                min.z = min.z.min(point.z);
                max.x = max.x.max(point.x);
                max.y = max.y.max(point.y);
                max.z = max.z.max(point.z);
            }
            _ => {
                self.bbox_min = Some(*point);
                self.bbox_max = Some(*point);
            }
        }
    }
}

impl Default for CoverageMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Coverage statistics for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageStats {
    /// Total coverage percentage (0.0 to 1.0)
    pub total_coverage: f32,

    /// Number of covered voxels
    pub covered_voxels: usize,

    /// Target number of voxels
    pub target_voxels: usize,

    /// Number of uncovered regions
    pub uncovered_regions: usize,

    /// Bounding box dimensions (width, height, depth) in cm
    pub dimensions: Option<(f32, f32, f32)>,
}

impl CoverageStats {
    /// Compute statistics from a coverage map
    pub fn from_coverage_map(map: &CoverageMap) -> Self {
        let dimensions = map.bbox_min.zip(map.bbox_max).map(|(min, max)| {
            (
                (max.x - min.x).abs(),
                (max.y - min.y).abs(),
                (max.z - min.z).abs(),
            )
        });

        Self {
            total_coverage: map.get_coverage_percentage(),
            covered_voxels: map.covered_voxel_count(),
            target_voxels: map.target_voxel_count,
            uncovered_regions: map.get_uncovered_regions().len(),
            dimensions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voxel_coord() {
        let voxel = VoxelCoord::new(1, 2, 3);
        assert_eq!(voxel.x, 1);
        assert_eq!(voxel.y, 2);
        assert_eq!(voxel.z, 3);

        let point = Point3::new(1.5, 2.5, 3.5);
        let voxel = VoxelCoord::from_point(&point, 1.0);
        assert_eq!(voxel, VoxelCoord::new(1, 2, 3));
    }

    #[test]
    fn test_coverage_map_basic() {
        let mut map = CoverageMap::new();
        assert_eq!(map.get_coverage_percentage(), 0.0);

        map.add_point(&Point3::new(0.0, 0.0, 0.0));
        assert!(map.covered_voxel_count() > 0);

        map.add_point(&Point3::new(0.1, 0.1, 0.1));
        // Should be in the same voxel
        assert_eq!(map.covered_voxel_count(), 1);

        map.add_point(&Point3::new(1.0, 1.0, 1.0));
        // Should be in a different voxel
        assert_eq!(map.covered_voxel_count(), 2);
    }

    #[test]
    fn test_coverage_percentage() {
        let mut map = CoverageMap::new();
        map.set_target_voxel_count(100);

        for i in 0..50 {
            map.add_point(&Point3::new(i as f32, 0.0, 0.0));
        }

        assert!(map.get_coverage_percentage() >= 0.45);
        assert!(map.get_coverage_percentage() <= 0.55);
    }
}
