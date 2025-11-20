//! Point cloud data structure and operations

use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

/// A 3D point with optional color and normal information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    /// 3D position
    pub position: Point3<f32>,

    /// RGB color (0-255)
    pub color: Option<[u8; 3]>,

    /// Surface normal (unit vector)
    pub normal: Option<Vector3<f32>>,

    /// Confidence score (0.0-1.0)
    pub confidence: f32,
}

impl Point {
    /// Create a new point with position only
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: Point3::new(x, y, z),
            color: None,
            normal: None,
            confidence: 1.0,
        }
    }

    /// Create a point with position and color
    pub fn with_color(x: f32, y: f32, z: f32, color: [u8; 3]) -> Self {
        Self {
            position: Point3::new(x, y, z),
            color: Some(color),
            normal: None,
            confidence: 1.0,
        }
    }

    /// Set the normal vector
    pub fn set_normal(&mut self, normal: Vector3<f32>) {
        self.normal = Some(normal.normalize());
    }

    /// Set the confidence score
    pub fn set_confidence(&mut self, confidence: f32) {
        self.confidence = confidence.clamp(0.0, 1.0);
    }
}

/// Collection of 3D points representing a scanned surface
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PointCloud {
    /// All points in the cloud
    points: Vec<Point>,

    /// Bounding box minimum
    bbox_min: Option<Point3<f32>>,

    /// Bounding box maximum
    bbox_max: Option<Point3<f32>>,
}

impl PointCloud {
    /// Create a new empty point cloud
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single point to the cloud
    pub fn add_point(&mut self, point: Point) {
        self.update_bbox(&point.position);
        self.points.push(point);
    }

    /// Add multiple points to the cloud
    pub fn add_points(&mut self, points: &[Point]) {
        for point in points {
            self.update_bbox(&point.position);
        }
        self.points.extend_from_slice(points);
    }

    /// Get the number of points in the cloud
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Check if the point cloud is empty
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Get a slice of all points
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// Get a mutable slice of all points
    pub fn points_mut(&mut self) -> &mut [Point] {
        &mut self.points
    }

    /// Get the bounding box of the point cloud
    pub fn bounding_box(&self) -> Option<(Point3<f32>, Point3<f32>)> {
        self.bbox_min.zip(self.bbox_max)
    }

    /// Get the center of the point cloud
    pub fn center(&self) -> Option<Point3<f32>> {
        if self.is_empty() {
            return None;
        }

        let sum = self
            .points
            .iter()
            .fold(Vector3::zeros(), |acc, p| acc + p.position.coords);

        Some(Point3::from(sum / self.points.len() as f32))
    }

    /// Filter points by confidence threshold
    pub fn filter_by_confidence(&mut self, min_confidence: f32) {
        self.points.retain(|p| p.confidence >= min_confidence);
        self.recompute_bbox();
    }

    /// Remove statistical outliers using distance threshold
    pub fn remove_outliers(&mut self, k_neighbors: usize, std_ratio: f32) {
        if self.points.len() < k_neighbors {
            return;
        }

        // Build k-d tree for nearest neighbor search
        use kiddo::KdTree;

        let mut tree: KdTree<f32, 3> = KdTree::new();
        for (idx, point) in self.points.iter().enumerate() {
            let pos = point.position;
            tree.add(
                &[pos.x, pos.y, pos.z],
                u64::try_from(idx).expect("point index exceeds u64 range"),
            );
        }

        // Compute mean distance to k nearest neighbors for each point
        let mut distances = Vec::with_capacity(self.points.len());
        for point in &self.points {
            let pos = point.position;
            let neighbors =
                tree.nearest_n::<kiddo::SquaredEuclidean>(&[pos.x, pos.y, pos.z], k_neighbors + 1);

            let mean_dist = neighbors
                .iter()
                .skip(1) // Skip the point itself
                .map(|n| n.distance)
                .sum::<f32>()
                / k_neighbors as f32;

            distances.push(mean_dist);
        }

        // Compute mean and standard deviation
        let mean = distances.iter().sum::<f32>() / distances.len() as f32;
        let variance =
            distances.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / distances.len() as f32;
        let std_dev = variance.sqrt();

        let threshold = mean + std_ratio * std_dev;

        // Filter points
        let mut filtered_points = Vec::new();
        for (point, dist) in self.points.iter().zip(distances.iter()) {
            if *dist <= threshold {
                filtered_points.push(point.clone());
            }
        }

        self.points = filtered_points;
        self.recompute_bbox();
    }

    /// Downsample the point cloud using voxel grid filtering
    pub fn voxel_downsample(&mut self, voxel_size: f32) {
        use std::collections::HashMap;

        if self.is_empty() {
            return;
        }

        let mut voxel_map: HashMap<(i32, i32, i32), Vec<Point>> = HashMap::new();

        // Group points by voxel
        for point in &self.points {
            let voxel = (
                (point.position.x / voxel_size).floor() as i32,
                (point.position.y / voxel_size).floor() as i32,
                (point.position.z / voxel_size).floor() as i32,
            );

            voxel_map.entry(voxel).or_default().push(point.clone());
        }

        // Average points in each voxel
        let mut downsampled = Vec::new();
        for points_in_voxel in voxel_map.values() {
            let n = points_in_voxel.len() as f32;

            let avg_pos = points_in_voxel
                .iter()
                .fold(Vector3::zeros(), |acc, p| acc + p.position.coords)
                / n;

            let avg_confidence = points_in_voxel.iter().map(|p| p.confidence).sum::<f32>() / n;

            let avg_color = if points_in_voxel[0].color.is_some() {
                let sum_r = points_in_voxel
                    .iter()
                    .filter_map(|p| p.color.map(|c| c[0] as u32))
                    .sum::<u32>();
                let sum_g = points_in_voxel
                    .iter()
                    .filter_map(|p| p.color.map(|c| c[1] as u32))
                    .sum::<u32>();
                let sum_b = points_in_voxel
                    .iter()
                    .filter_map(|p| p.color.map(|c| c[2] as u32))
                    .sum::<u32>();

                Some([
                    (sum_r / n as u32) as u8,
                    (sum_g / n as u32) as u8,
                    (sum_b / n as u32) as u8,
                ])
            } else {
                None
            };

            let mut point = Point::new(avg_pos.x, avg_pos.y, avg_pos.z);
            point.color = avg_color;
            point.confidence = avg_confidence;

            downsampled.push(point);
        }

        self.points = downsampled;
        self.recompute_bbox();
    }

    /// Estimate normals for all points using k nearest neighbors
    pub fn estimate_normals(&mut self, k_neighbors: usize) {
        use kiddo::KdTree;

        if self.points.len() < k_neighbors {
            return;
        }

        // Build k-d tree
        let mut tree: KdTree<f32, 3> = KdTree::new();
        for (idx, point) in self.points.iter().enumerate() {
            let pos = point.position;
            tree.add(
                &[pos.x, pos.y, pos.z],
                u64::try_from(idx).expect("point index exceeds u64 range"),
            );
        }

        // Estimate normal for each point
        for i in 0..self.points.len() {
            let pos = self.points[i].position;
            let neighbors =
                tree.nearest_n::<kiddo::SquaredEuclidean>(&[pos.x, pos.y, pos.z], k_neighbors);

            // Collect neighbor positions
            let neighbor_points: Vec<Point3<f32>> = neighbors
                .iter()
                .filter_map(|n| usize::try_from(n.item).ok())
                .filter_map(|index| self.points.get(index).map(|p| p.position))
                .collect();

            // Compute covariance matrix and find normal via PCA
            if let Some(normal) = compute_normal_pca(&neighbor_points) {
                self.points[i].set_normal(normal);
            }
        }
    }

    /// Export point cloud to PLY format
    pub fn export_ply(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(path)?;

        // Write PLY header
        writeln!(file, "ply")?;
        writeln!(file, "format ascii 1.0")?;
        writeln!(file, "element vertex {}", self.points.len())?;
        writeln!(file, "property float x")?;
        writeln!(file, "property float y")?;
        writeln!(file, "property float z")?;

        if self.points.iter().any(|p| p.color.is_some()) {
            writeln!(file, "property uchar red")?;
            writeln!(file, "property uchar green")?;
            writeln!(file, "property uchar blue")?;
        }

        if self.points.iter().any(|p| p.normal.is_some()) {
            writeln!(file, "property float nx")?;
            writeln!(file, "property float ny")?;
            writeln!(file, "property float nz")?;
        }

        writeln!(file, "end_header")?;

        // Write points
        for point in &self.points {
            write!(
                file,
                "{} {} {}",
                point.position.x, point.position.y, point.position.z
            )?;

            if let Some(color) = point.color {
                write!(file, " {} {} {}", color[0], color[1], color[2])?;
            }

            if let Some(normal) = point.normal {
                write!(file, " {} {} {}", normal.x, normal.y, normal.z)?;
            }

            writeln!(file)?;
        }

        Ok(())
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

    fn recompute_bbox(&mut self) {
        self.bbox_min = None;
        self.bbox_max = None;

        // Collect positions first to avoid borrow checker issues
        let positions: Vec<_> = self.points.iter().map(|p| p.position).collect();
        for position in &positions {
            self.update_bbox(position);
        }
    }
}

/// Compute normal vector using PCA on a set of points
fn compute_normal_pca(points: &[Point3<f32>]) -> Option<Vector3<f32>> {
    if points.len() < 3 {
        return None;
    }

    // Compute centroid
    let centroid = points
        .iter()
        .fold(Vector3::zeros(), |acc, p| acc + p.coords)
        / points.len() as f32;

    // Compute covariance matrix
    let mut cov = nalgebra::Matrix3::zeros();
    for point in points {
        let centered = point.coords - centroid;
        cov += centered * centered.transpose();
    }
    cov /= points.len() as f32;

    // Find eigenvector corresponding to smallest eigenvalue
    // This is the normal direction
    let eigen = cov.symmetric_eigen();
    let normal = eigen.eigenvectors.column(0).into_owned();

    Some(normal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let p1 = Point::new(1.0, 2.0, 3.0);
        assert_eq!(p1.position.x, 1.0);
        assert_eq!(p1.position.y, 2.0);
        assert_eq!(p1.position.z, 3.0);
        assert_eq!(p1.confidence, 1.0);

        let p2 = Point::with_color(1.0, 2.0, 3.0, [255, 128, 64]);
        assert_eq!(p2.color, Some([255, 128, 64]));
    }

    #[test]
    fn test_point_cloud_basic() {
        let mut cloud = PointCloud::new();
        assert!(cloud.is_empty());

        cloud.add_point(Point::new(0.0, 0.0, 0.0));
        cloud.add_point(Point::new(1.0, 1.0, 1.0));

        assert_eq!(cloud.len(), 2);
        assert!(!cloud.is_empty());

        let center = cloud.center().unwrap();
        assert!((center.x - 0.5).abs() < 1e-6);
        assert!((center.y - 0.5).abs() < 1e-6);
        assert!((center.z - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_bounding_box() {
        let mut cloud = PointCloud::new();
        cloud.add_point(Point::new(-1.0, -2.0, -3.0));
        cloud.add_point(Point::new(4.0, 5.0, 6.0));

        let (min, max) = cloud.bounding_box().unwrap();
        assert_eq!(min, Point3::new(-1.0, -2.0, -3.0));
        assert_eq!(max, Point3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn test_kd_tree_index_conversion() {
        let mut cloud = PointCloud::new();
        cloud.add_point(Point::new(0.0, 0.0, 0.0));
        cloud.add_point(Point::new(1.0, 0.0, 0.0));
        cloud.add_point(Point::new(0.0, 1.0, 0.0));
        cloud.add_point(Point::new(1.0, 1.0, 0.0));

        cloud.remove_outliers(3, 1.0);
        cloud.estimate_normals(3);

        assert!(cloud.points().iter().all(|point| point.normal.is_some()));
    }
}
