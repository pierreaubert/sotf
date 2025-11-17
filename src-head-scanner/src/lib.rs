//! # Head Scanner
//!
//! A 3D head scanning system using computer vision for HRTF (Head-Related Transfer Function) optimization.
//!
//! ## Architecture
//!
//! The head scanner consists of several key components:
//!
//! - **Camera Capture**: Interfaces with device cameras (webcam, phone) to capture frames
//! - **Feature Detection**: Uses computer vision to detect facial landmarks and head features
//! - **3D Reconstruction**: Converts 2D images into 3D point clouds using structure-from-motion
//! - **Coverage Tracking**: Monitors which parts of the head have been scanned
//! - **Surface Reconstruction**: Builds a triangulated mesh from the point cloud
//! - **Convex Hull**: Computes the 3D convex hull for surface optimization
//! - **Mesh Export**: Exports the final head model in standard formats (OBJ, PLY, STL)
//!
//! ## Usage
//!
//! ```no_run
//! use head_scanner::{HeadScanner, ScannerConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create scanner with default configuration
//! let config = ScannerConfig::default();
//! let mut scanner = HeadScanner::new(config)?;
//!
//! // Start scanning
//! scanner.start().await?;
//!
//! // Process frames until scan is complete
//! while !scanner.is_scan_complete() {
//!     scanner.process_frame().await?;
//!     let coverage = scanner.get_coverage();
//!     println!("Scan coverage: {:.1}%", coverage * 100.0);
//! }
//!
//! // Generate final mesh
//! let mesh = scanner.generate_mesh()?;
//! mesh.export("head_model.obj")?;
//! # Ok(())
//! # }
//! ```

pub mod bundle_adjustment;
pub mod camera;
pub mod convexhull;
pub mod coverage;
pub mod error;
pub mod mesh;
pub mod pointcloud;
pub mod reconstruction;
pub mod stereo;
pub mod texture;
pub mod vision;

pub use error::{ScannerError, ScannerResult};
pub use mesh::{Mesh, Triangle, Vertex};
pub use pointcloud::PointCloud;

use parking_lot::RwLock;
use std::sync::Arc;

/// Configuration for the head scanner
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScannerConfig {
    /// Camera device index (0 for default camera)
    pub camera_index: u32,

    /// Target resolution width
    pub frame_width: u32,

    /// Target resolution height
    pub frame_height: u32,

    /// Frame rate for capture
    pub fps: u32,

    /// Minimum coverage percentage to consider scan complete (0.0-1.0)
    pub min_coverage: f32,

    /// Point cloud density target (points per square cm)
    pub point_density: f32,

    /// Enable GPU acceleration if available
    pub use_gpu: bool,

    /// Path to the vision model (ONNX format)
    pub model_path: Option<String>,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            camera_index: 0,
            frame_width: 1280,
            frame_height: 720,
            fps: 30,
            min_coverage: 0.85, // 85% coverage required
            point_density: 50.0, // 50 points per square cm
            use_gpu: true,
            model_path: None,
        }
    }
}

/// State of the scanning process
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScanState {
    /// Scanner is idle, not yet started
    Idle,

    /// Scanner is initializing camera and models
    Initializing,

    /// Waiting for head detection
    DetectingHead,

    /// Actively scanning and collecting points
    Scanning,

    /// Scan is paused
    Paused,

    /// Processing and building final mesh
    Processing,

    /// Scan is complete
    Complete,

    /// Error occurred
    Error,
}

/// Main head scanner interface
pub struct HeadScanner {
    config: ScannerConfig,
    state: Arc<RwLock<ScanState>>,
    camera: Arc<RwLock<Option<camera::Camera>>>,
    point_cloud: Arc<RwLock<PointCloud>>,
    coverage: Arc<RwLock<coverage::CoverageMap>>,
    vision_model: Arc<RwLock<Option<vision::VisionModel>>>,
}

impl HeadScanner {
    /// Create a new head scanner with the given configuration
    pub fn new(config: ScannerConfig) -> ScannerResult<Self> {
        Ok(Self {
            config,
            state: Arc::new(RwLock::new(ScanState::Idle)),
            camera: Arc::new(RwLock::new(None)),
            point_cloud: Arc::new(RwLock::new(PointCloud::new())),
            coverage: Arc::new(RwLock::new(coverage::CoverageMap::new())),
            vision_model: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize and start the scanner
    pub async fn start(&mut self) -> ScannerResult<()> {
        *self.state.write() = ScanState::Initializing;

        // Initialize camera
        let camera = camera::Camera::new(
            self.config.camera_index,
            self.config.frame_width,
            self.config.frame_height,
            self.config.fps,
        )?;
        *self.camera.write() = Some(camera);

        // Initialize vision model if path provided
        if let Some(ref model_path) = self.config.model_path {
            // Validate model path for security
            use std::path::Path;
            let path = Path::new(model_path);
            if model_path.contains("..") {
                return Err(ScannerError::InvalidConfig(
                    "Path traversal detected in model path".to_string(),
                ));
            }
            if !path.exists() {
                return Err(ScannerError::InvalidConfig(format!(
                    "Model file does not exist: {}",
                    model_path
                )));
            }

            let model = vision::VisionModel::load(model_path)?;
            *self.vision_model.write() = Some(model);
        }

        *self.state.write() = ScanState::DetectingHead;
        Ok(())
    }

    /// Process a single frame from the camera
    pub async fn process_frame(&mut self) -> ScannerResult<()> {
        let state = *self.state.read();

        match state {
            ScanState::DetectingHead | ScanState::Scanning => {
                // Capture frame from camera
                let frame = {
                    let camera_guard = self.camera.read();
                    let camera = camera_guard.as_ref()
                        .ok_or(ScannerError::CameraNotInitialized)?;
                    camera.capture_frame()?
                };

                // Detect head features
                let features = if let Some(ref model) = *self.vision_model.read() {
                    model.detect_features(&frame)?
                } else {
                    vision::detect_features_classical(&frame)?
                };

                // If we detected a head, start scanning
                if state == ScanState::DetectingHead && !features.is_empty() {
                    *self.state.write() = ScanState::Scanning;
                }

                // Add points to the point cloud
                if state == ScanState::Scanning {
                    let points = reconstruction::features_to_points(&features, &frame)?;

                    // Deduplicate and filter points before adding
                    let mut point_cloud = self.point_cloud.write();
                    let filtered_points = self.filter_new_points(&point_cloud, points);

                    if !filtered_points.is_empty() {
                        point_cloud.add_points(&filtered_points);

                        // Periodically downsample to control memory usage
                        if point_cloud.len() % 1000 == 0 {
                            point_cloud.voxel_downsample(self.config.point_density / 100.0);
                        }
                    }

                    drop(point_cloud); // Release lock before updating coverage

                    // Update coverage map
                    self.coverage.write().update(&filtered_points);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Check if the scan is complete based on coverage
    pub fn is_scan_complete(&self) -> bool {
        let coverage = self.coverage.read();
        coverage.get_coverage_percentage() >= self.config.min_coverage
    }

    /// Get current coverage percentage (0.0 to 1.0)
    pub fn get_coverage(&self) -> f32 {
        self.coverage.read().get_coverage_percentage()
    }

    /// Get coverage map for visualization
    pub fn get_coverage_map(&self) -> coverage::CoverageMap {
        self.coverage.read().clone()
    }

    /// Get current scan state
    pub fn get_state(&self) -> ScanState {
        *self.state.read()
    }

    /// Pause the scanning process
    pub fn pause(&mut self) {
        *self.state.write() = ScanState::Paused;
    }

    /// Resume the scanning process
    pub fn resume(&mut self) {
        *self.state.write() = ScanState::Scanning;
    }

    /// Generate the final triangulated mesh from the point cloud
    pub fn generate_mesh(&mut self) -> ScannerResult<Mesh> {
        *self.state.write() = ScanState::Processing;

        let point_cloud = self.point_cloud.read();

        // Build convex hull
        let hull = convexhull::compute_convex_hull_3d(&point_cloud)?;

        // Convert hull to mesh
        let mesh = Mesh::from_convex_hull(&hull);

        *self.state.write() = ScanState::Complete;
        Ok(mesh)
    }

    /// Reset the scanner to initial state
    pub fn reset(&mut self) {
        *self.state.write() = ScanState::Idle;
        *self.point_cloud.write() = PointCloud::new();
        *self.coverage.write() = coverage::CoverageMap::new();
    }

    /// Stop the scanner and release resources
    pub async fn stop(&mut self) -> ScannerResult<()> {
        *self.camera.write() = None;
        *self.state.write() = ScanState::Idle;
        Ok(())
    }

    /// Filter new points to remove duplicates and points too close to existing ones
    ///
    /// This prevents the point cloud from growing unbounded with redundant data
    fn filter_new_points(&self, existing_cloud: &PointCloud, new_points: Vec<pointcloud::Point>) -> Vec<pointcloud::Point> {
        use kiddo::KdTree;

        if existing_cloud.is_empty() {
            return new_points;
        }

        // Build k-d tree from existing points for efficient nearest neighbor search
        let mut tree: KdTree<f32, 3> = KdTree::new();
        for (idx, point) in existing_cloud.points().iter().enumerate() {
            let pos = point.position;
            tree.add(&[pos.x, pos.y, pos.z], idx);
        }

        // Minimum distance threshold (in cm) - points closer than this are considered duplicates
        let min_distance = self.config.point_density / 20.0; // e.g., 2.5mm for 50 points/cm²
        let min_distance_sq = min_distance * min_distance;

        // Filter out points that are too close to existing points
        let mut filtered = Vec::new();
        for point in new_points {
            let pos = point.position;

            // Find nearest existing point
            let nearest = tree.nearest_one(&[pos.x, pos.y, pos.z]);

            // Only add if far enough from existing points
            if nearest.distance > min_distance_sq {
                filtered.push(point);
            }
        }

        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_config_default() {
        let config = ScannerConfig::default();
        assert_eq!(config.camera_index, 0);
        assert_eq!(config.frame_width, 1280);
        assert_eq!(config.frame_height, 720);
        assert_eq!(config.fps, 30);
        assert!(config.min_coverage > 0.0 && config.min_coverage <= 1.0);
    }

    #[test]
    fn test_scanner_creation() {
        let config = ScannerConfig::default();
        let scanner = HeadScanner::new(config);
        assert!(scanner.is_ok());

        let scanner = scanner.unwrap();
        assert_eq!(scanner.get_state(), ScanState::Idle);
        assert_eq!(scanner.get_coverage(), 0.0);
    }
}
