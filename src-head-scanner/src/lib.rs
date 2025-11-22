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

pub mod acoustics;
pub mod bundle_adjustment;
pub mod calibration;
pub mod camera;
pub mod convexhull;
pub mod coverage;
pub mod error;
pub mod ffi; // FFI bindings for iOS/Swift
pub mod security; // Security utilities for path validation
pub mod guidance;
pub mod mesh;
pub mod pointcloud;
pub mod reconstruction;
pub mod scanner; // Simplified scanner interface for FFI
pub mod stereo;
pub mod texture;
pub mod vision;

pub use camera::Frame;
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

    /// Allowed base directories for model files (for security)
    /// If empty, any directory is allowed (less secure)
    pub model_base_dirs: Vec<std::path::PathBuf>,

    /// Use Structure-from-Motion for accurate 3D reconstruction
    pub use_sfm: bool,

    /// Number of frames to keep in history for SfM (2-10 recommended)
    pub sfm_frame_count: usize,

    /// Minimum number of inliers for valid essential matrix
    pub sfm_min_inliers: usize,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            camera_index: 0,
            frame_width: 1280,
            frame_height: 720,
            fps: 30,
            min_coverage: 0.85,  // 85% coverage required
            point_density: 50.0, // 50 points per square cm
            use_gpu: true,
            model_path: None,
            model_base_dirs: Vec::new(), // Empty = allow any directory (less secure)
            use_sfm: false,      // Disabled by default for compatibility
            sfm_frame_count: 3,  // Use 3 frames for SfM
            sfm_min_inliers: 20, // Minimum 20 inliers for valid pose
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

    // SfM components
    orb_detector: Arc<RwLock<Option<vision::ORBDetector>>>,
    frame_history: Arc<RwLock<std::collections::VecDeque<SfMFrame>>>,
    pose_history: Arc<RwLock<Vec<reconstruction::CameraPose>>>,
    intrinsics: reconstruction::CameraIntrinsics,

    // Scan guidance system
    guidance: Arc<RwLock<guidance::ScanGuidance>>,

    // K-d tree cache for efficient point deduplication
    // Stores (tree, point_count_when_built) to know when to invalidate
    kd_tree_cache: Arc<RwLock<Option<(kiddo::KdTree<f32, 3>, usize)>>>,
}

/// Frame data for Structure-from-Motion
struct SfMFrame {
    frame: Frame,
    keypoints: opencv::core::Vector<opencv::core::KeyPoint>,
    descriptors: opencv::core::Mat,
}

impl HeadScanner {
    /// Create a new head scanner with the given configuration
    pub fn new(config: ScannerConfig) -> ScannerResult<Self> {
        // Default camera intrinsics (will be updated if calibration is available)
        let intrinsics = reconstruction::CameraIntrinsics {
            fx: config.frame_width as f32 * 0.8,  // Typical focal length
            fy: config.frame_width as f32 * 0.8,
            cx: config.frame_width as f32 / 2.0,
            cy: config.frame_height as f32 / 2.0,
            distortion: Some([0.0, 0.0, 0.0, 0.0, 0.0]),
        };
        
        Ok(Self {
            config,
            state: Arc::new(RwLock::new(ScanState::Idle)),
            camera: Arc::new(RwLock::new(None)),
            point_cloud: Arc::new(RwLock::new(PointCloud::new())),
            coverage: Arc::new(RwLock::new(coverage::CoverageMap::new())),
            vision_model: Arc::new(RwLock::new(None)),
            orb_detector: Arc::new(RwLock::new(None)),
            frame_history: Arc::new(RwLock::new(std::collections::VecDeque::new())),
            pose_history: Arc::new(RwLock::new(Vec::new())),
            intrinsics,
            guidance: Arc::new(RwLock::new(guidance::ScanGuidance::new())),
            kd_tree_cache: Arc::new(RwLock::new(None)),
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
            // Validate model path for security (prevents path traversal, symlink attacks, etc.)
            let validated_path = if self.config.model_base_dirs.is_empty() {
                // No base directories specified - just validate the path exists and is safe
                security::validate_model_path(model_path, &[])?
            } else {
                // Validate path is within one of the allowed base directories
                security::validate_model_path(model_path, &self.config.model_base_dirs)?
            };

            let model = vision::VisionModel::load_with_gpu(
                validated_path.to_str().unwrap(),
                self.config.use_gpu,
            )?;
            *self.vision_model.write() = Some(model);
        }

        // Initialize ORB detector if SfM is enabled
        if self.config.use_sfm {
            let orb = vision::ORBDetector::new()?;
            *self.orb_detector.write() = Some(orb);
            log::info!("SfM mode enabled with {} frame history", self.config.sfm_frame_count);
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
                    let camera = camera_guard
                        .as_ref()
                        .ok_or(ScannerError::CameraNotInitialized)?;
                    camera.capture_frame()?
                };

                // Choose reconstruction method based on config
                if self.config.use_sfm {
                    // SfM mode: detect ORB features and triangulate
                    self.process_frame_sfm(frame, state).await?;
                } else {
                    // Classical mode: detect features and estimate depth
                    self.process_frame_classical(frame, state).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Process frame using classical feature detection with estimated depth
    async fn process_frame_classical(&mut self, frame: Frame, state: ScanState) -> ScannerResult<()> {
        // Detect head features
        let features = if let Some(ref mut model) = *self.vision_model.write() {
            model.detect_features(&frame)?
        } else {
            vision::detect_features_classical(&frame)?
        };

        log::debug!("Detected {} features in frame", features.len());

        // If we detected a head, start scanning
        if state == ScanState::DetectingHead && !features.is_empty() {
            *self.state.write() = ScanState::Scanning;
            log::info!("Head detected with {} features, starting scan", features.len());
        }

        // Add points to the point cloud
        if state == ScanState::Scanning {
            let points = reconstruction::features_to_points(&features, &frame)?;
            log::debug!("Converted {} features to {} 3D points", features.len(), points.len());

            // Deduplicate and filter points before adding
            let mut point_cloud = self.point_cloud.write();
            let initial_count = point_cloud.len();
            let filtered_points = self.filter_new_points(&point_cloud, points);

            if !filtered_points.is_empty() {
                point_cloud.add_points(&filtered_points);
                let new_count = point_cloud.len();
                log::debug!("Added {} new points (total: {} -> {})", filtered_points.len(), initial_count, new_count);

                // Periodically downsample to control memory usage
                if point_cloud.len() % 1000 == 0 {
                    point_cloud.voxel_downsample(self.config.point_density / 100.0);
                }
            } else {
                log::debug!("No new points added (all filtered out)");
            }

            drop(point_cloud); // Release lock before updating coverage

            // Update coverage map
            self.coverage.write().update(&filtered_points);
        }

        Ok(())
    }

    /// Process frame using Structure-from-Motion with triangulation
    async fn process_frame_sfm(&mut self, frame: Frame, state: ScanState) -> ScannerResult<()> {
        // Detect ORB features
        let (keypoints, descriptors) = {
            let mut orb_guard = self.orb_detector.write();
            let orb = orb_guard.as_mut()
                .ok_or(ScannerError::InvalidConfig("ORB detector not initialized".to_string()))?;
            orb.detect_and_compute(&frame)?
        };

        log::debug!("Detected {} ORB keypoints", keypoints.len());

        // If we detected features, start scanning
        if state == ScanState::DetectingHead && keypoints.len() > 0 {
            *self.state.write() = ScanState::Scanning;
            log::info!("Head detected with {} ORB features, starting SfM scan", keypoints.len());
        }

        // Add frame to history
        if state == ScanState::Scanning {
            let mut history = self.frame_history.write();
            history.push_back(SfMFrame {
                frame: frame.clone(),
                keypoints,
                descriptors,
            });

            // Keep only last N frames
            while history.len() > self.config.sfm_frame_count {
                history.pop_front();
            }

            // If we have at least 2 frames, do SfM reconstruction
            if history.len() >= 2 {
                drop(history); // Release lock before reconstruction
                self.reconstruct_sfm().await?;
            }
        }

        Ok(())
    }

    /// Perform Structure-from-Motion reconstruction from frame history
    async fn reconstruct_sfm(&mut self) -> ScannerResult<()> {
        let history = self.frame_history.read();
        let n = history.len();
        
        if n < 2 {
            return Ok(());
        }

        // Use last two frames
        let frame1 = &history[n - 2];
        let frame2 = &history[n - 1];

        // Match features using descriptor-based matching with ratio test
        let matches = vision::ORBDetector::match_features(
            &frame1.descriptors,
            &frame2.descriptors,
            0.75,  // Lowe's ratio test threshold
        )?;
        
        if matches.len() < 8 {
            log::debug!("Not enough matched features for SfM ({} < 8)", matches.len());
            return Ok(());
        }

        log::debug!("Matched {} feature pairs between frames", matches.len());

        // Convert keypoints to features
        let features1 = vision::ORBDetector::keypoints_to_features(&frame1.keypoints);
        let features2 = vision::ORBDetector::keypoints_to_features(&frame2.keypoints);

        // Extract matched point positions
        let mut points1 = Vec::new();
        let mut points2 = Vec::new();
        
        for (idx1, idx2) in matches.iter() {
            if *idx1 < features1.len() && *idx2 < features2.len() {
                points1.push((features1[*idx1].position.x, features1[*idx1].position.y));
                points2.push((features2[*idx2].position.x, features2[*idx2].position.y));
            }
        }

        // Estimate essential matrix
        let (essential, inliers) = match reconstruction::estimate_essential_matrix(
            &points1,
            &points2,
            &self.intrinsics,
        ) {
            Ok(result) => result,
            Err(e) => {
                log::debug!("Essential matrix estimation failed: {}", e);
                return Ok(());
            }
        };

        let inlier_count = inliers.iter().filter(|&&x| x).count();
        
        if inlier_count < self.config.sfm_min_inliers {
            log::debug!("Not enough inliers for SfM ({} < {})", inlier_count, self.config.sfm_min_inliers);
            return Ok(());
        }

        log::debug!("Essential matrix: {} inliers of {} points", inlier_count, points1.len());

        // Recover camera pose
        let pose = match reconstruction::recover_pose_from_essential(
            &essential,
            &points1,
            &points2,
            &self.intrinsics,
            &inliers,
        ) {
            Ok(p) => p,
            Err(e) => {
                log::debug!("Pose recovery failed: {}", e);
                return Ok(());
            }
        };

        // Add pose to history
        self.pose_history.write().push(pose.clone());

        // Update scan guidance with new pose
        let point_count = self.point_cloud.read().len();
        self.guidance.write().update_pose(&pose, point_count);

        // Get previous pose (or identity for first frame)
        let pose1 = if self.pose_history.read().len() > 1 {
            let poses = self.pose_history.read();
            poses[poses.len() - 2].clone()
        } else {
            reconstruction::CameraPose {
                position: nalgebra::Point3::new(0.0, 0.0, 0.0),
                rotation: nalgebra::Matrix3::identity(),
            }
        };

        // Triangulate inlier points
        let mut new_points = Vec::new();
        for (i, &is_inlier) in inliers.iter().enumerate() {
            if is_inlier {
                let pt1 = nalgebra::Point2::new(points1[i].0, points1[i].1);
                let pt2 = nalgebra::Point2::new(points2[i].0, points2[i].1);

                if let Ok(point_3d) = reconstruction::triangulate_point(&pt1, &pt2, &pose1, &pose, &self.intrinsics) {
                    // Filter out points that are too far or behind camera
                    let depth = point_3d.coords.norm();
                    if depth > 10.0 && depth < 200.0 {  // 10cm to 2m range
                        new_points.push(pointcloud::Point {
                            position: point_3d,
                            normal: None,
                            color: None,
                            confidence: 1.0,  // High confidence for triangulated points
                        });
                    }
                }
            }
        }

        if !new_points.is_empty() {
            // Add to point cloud
            let mut point_cloud = self.point_cloud.write();
            let initial_count = point_cloud.len();
            let filtered_points = self.filter_new_points(&point_cloud, new_points);
            
            if !filtered_points.is_empty() {
                point_cloud.add_points(&filtered_points);
                log::info!("SfM: Triangulated {} new 3D points (total: {} -> {})", 
                    filtered_points.len(), initial_count, point_cloud.len());

                // Update coverage
                drop(point_cloud);
                self.coverage.write().update(&filtered_points);
            }
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
        *self.kd_tree_cache.write() = None; // Invalidate K-d tree cache
    }

    /// Stop the scanner and release resources
    pub async fn stop(&mut self) -> ScannerResult<()> {
        *self.camera.write() = None;
        *self.state.write() = ScanState::Idle;
        Ok(())
    }

    /// Capture and return the current frame from the camera
    ///
    /// This is useful for displaying the camera feed in real-time
    pub fn capture_current_frame(&self) -> ScannerResult<camera::Frame> {
        let camera_guard = self.camera.read();
        let camera = camera_guard
            .as_ref()
            .ok_or(ScannerError::CameraNotInitialized)?;
        camera.capture_frame()
    }

    /// Check if GPU acceleration is enabled for ML inference
    pub fn is_using_gpu(&self) -> bool {
        let model_guard = self.vision_model.read();
        if let Some(model) = model_guard.as_ref() {
            model.is_using_gpu()
        } else {
            false
        }
    }

    /// Get the current size of the point cloud
    pub fn get_point_cloud_size(&self) -> usize {
        self.point_cloud.read().len()
    }

    /// Get the scan guidance system (for reading guidance instructions)
    pub fn get_guidance(&self) -> guidance::ScanGuidance {
        self.guidance.read().clone()
    }

    /// Get quality metrics for the current scan
    pub fn get_quality_metrics(&self) -> guidance::QualityMetrics {
        let point_count = self.get_point_cloud_size();
        let coverage_map = self.coverage.read();
        self.guidance
            .write()
            .compute_quality(&coverage_map, point_count)
    }

    /// Get next scan guidance instruction
    pub fn get_next_guidance(&self) -> Option<guidance::GuidanceInstruction> {
        self.guidance.read().compute_next_target()
    }

    /// Get quality improvement suggestions
    pub fn get_suggestions(&self) -> Vec<String> {
        self.guidance.read().suggest_improvements()
    }

    /// Filter new points to remove duplicates and points too close to existing ones
    ///
    /// This prevents the point cloud from growing unbounded with redundant data
    /// Uses a cached K-d tree to avoid O(n log n) rebuild overhead every frame
    ///
    /// Cache is invalidated when point cloud exceeds MAX_CACHE_SIZE to prevent
    /// excessive memory usage in long scanning sessions.
    fn filter_new_points(
        &self,
        existing_cloud: &PointCloud,
        new_points: Vec<pointcloud::Point>,
    ) -> Vec<pointcloud::Point> {
        use kiddo::{KdTree, SquaredEuclidean};

        // Maximum point count for K-d tree caching (prevents unbounded memory growth)
        // 100K points ≈ 2-4 MB tree, reasonable for head scan detail
        const MAX_CACHE_SIZE: usize = 100_000;

        if existing_cloud.is_empty() {
            return new_points;
        }

        let current_point_count = existing_cloud.len();

        // Check if we can use cached K-d tree or need to rebuild
        let tree = {
            let mut cache = self.kd_tree_cache.write();

            // Check if cache is valid (point count matches AND within size limit)
            let needs_rebuild = match cache.as_ref() {
                Some((_, cached_count)) => {
                    // Invalidate if point count changed OR exceeded max cache size
                    *cached_count != current_point_count || current_point_count > MAX_CACHE_SIZE
                }
                None => true,
            };

            if needs_rebuild {
                // If point cloud is too large, disable caching to prevent memory bloat
                if current_point_count > MAX_CACHE_SIZE {
                    log::debug!(
                        "K-d tree cache disabled: point cloud size {} exceeds limit {}",
                        current_point_count,
                        MAX_CACHE_SIZE
                    );
                    *cache = None; // Clear cache

                    // Build tree directly without caching
                    let mut new_tree: KdTree<f32, 3> = KdTree::new();
                    for (idx, point) in existing_cloud.points().iter().enumerate() {
                        let pos = point.position;
                        new_tree.add(&[pos.x, pos.y, pos.z], idx as u64);
                    }
                    new_tree
                } else {
                    // Build new k-d tree from existing points
                    let mut new_tree: KdTree<f32, 3> = KdTree::new();
                    for (idx, point) in existing_cloud.points().iter().enumerate() {
                        let pos = point.position;
                        new_tree.add(&[pos.x, pos.y, pos.z], idx as u64);
                    }

                    log::debug!(
                        "Rebuilt K-d tree with {} points (cache was {})",
                        current_point_count,
                        if cache.is_some() { "stale" } else { "empty" }
                    );

                    // Update cache
                    *cache = Some((new_tree.clone(), current_point_count));
                    new_tree
                }
            } else {
                // Clone the tree for use (kiddo::KdTree is relatively lightweight to clone)
                cache.as_ref().unwrap().0.clone()
            }
        };

        // Minimum distance threshold (in cm) - points closer than this are considered duplicates
        // Relaxed threshold to allow more points through for faster coverage
        let min_distance = self.config.point_density / 50.0; // e.g., 1mm for 50 points/cm²
        let min_distance_sq = min_distance * min_distance;

        // Filter out points that are too close to existing points
        let mut filtered = Vec::new();
        for point in new_points {
            let pos = point.position;

            // Find nearest existing point
            let nearest = tree.nearest_one::<SquaredEuclidean>(&[pos.x, pos.y, pos.z]);

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

    #[test]
    fn test_kd_tree_cache_initialization() {
        // K-d tree cache should start as None
        let config = ScannerConfig::default();
        let scanner = HeadScanner::new(config).unwrap();

        let cache = scanner.kd_tree_cache.read();
        assert!(cache.is_none(), "K-d tree cache should be None initially");
    }

    #[test]
    fn test_kd_tree_cache_builds_on_first_use() {
        let config = ScannerConfig::default();
        let scanner = HeadScanner::new(config).unwrap();

        // Add some points to the point cloud
        let mut point_cloud = scanner.point_cloud.write();
        point_cloud.add_point(pointcloud::Point::new(0.0, 0.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(1.0, 0.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(0.0, 1.0, 0.0));
        drop(point_cloud); // Release lock

        // Filter new points (this should build the cache)
        let new_points = vec![pointcloud::Point::new(0.5, 0.5, 0.0)];
        let point_cloud_read = scanner.point_cloud.read();
        let _filtered = scanner.filter_new_points(&point_cloud_read, new_points);
        drop(point_cloud_read);

        // Cache should now exist with correct point count
        let cache = scanner.kd_tree_cache.read();
        assert!(cache.is_some(), "K-d tree cache should be built after first use");
        let (_, cached_count) = cache.as_ref().unwrap();
        assert_eq!(*cached_count, 3, "Cached point count should match point cloud size");
    }

    #[test]
    fn test_kd_tree_cache_reuses_on_same_size() {
        let config = ScannerConfig::default();
        let scanner = HeadScanner::new(config).unwrap();

        // Add points
        let mut point_cloud = scanner.point_cloud.write();
        point_cloud.add_point(pointcloud::Point::new(0.0, 0.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(1.0, 0.0, 0.0));
        drop(point_cloud);

        // First filter - builds cache
        let new_points1 = vec![pointcloud::Point::new(0.5, 0.5, 0.0)];
        let point_cloud_read = scanner.point_cloud.read();
        let _filtered1 = scanner.filter_new_points(&point_cloud_read, new_points1);
        drop(point_cloud_read);

        // Get cache reference
        let cache_ptr = {
            let cache = scanner.kd_tree_cache.read();
            cache.as_ref().unwrap().0.size()
        };

        // Second filter with same point cloud size - should reuse cache
        let new_points2 = vec![pointcloud::Point::new(0.3, 0.3, 0.0)];
        let point_cloud_read = scanner.point_cloud.read();
        let _filtered2 = scanner.filter_new_points(&point_cloud_read, new_points2);
        drop(point_cloud_read);

        // Cache should still have same size (was reused)
        let cache = scanner.kd_tree_cache.read();
        assert_eq!(cache.as_ref().unwrap().0.size(), cache_ptr, "Cache should be reused");
    }

    #[test]
    fn test_kd_tree_cache_rebuilds_on_size_change() {
        let config = ScannerConfig::default();
        let scanner = HeadScanner::new(config).unwrap();

        // Add initial points
        let mut point_cloud = scanner.point_cloud.write();
        point_cloud.add_point(pointcloud::Point::new(0.0, 0.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(1.0, 0.0, 0.0));
        drop(point_cloud);

        // First filter - builds cache with 2 points
        let new_points1 = vec![pointcloud::Point::new(0.5, 0.5, 0.0)];
        let point_cloud_read = scanner.point_cloud.read();
        let _filtered1 = scanner.filter_new_points(&point_cloud_read, new_points1);
        drop(point_cloud_read);

        let initial_cached_count = {
            let cache = scanner.kd_tree_cache.read();
            cache.as_ref().unwrap().1
        };
        assert_eq!(initial_cached_count, 2);

        // Add more points to change size
        let mut point_cloud = scanner.point_cloud.write();
        point_cloud.add_point(pointcloud::Point::new(0.0, 1.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(1.0, 1.0, 0.0));
        drop(point_cloud);

        // Second filter - should rebuild cache with 4 points
        let new_points2 = vec![pointcloud::Point::new(0.3, 0.3, 0.0)];
        let point_cloud_read = scanner.point_cloud.read();
        let _filtered2 = scanner.filter_new_points(&point_cloud_read, new_points2);
        drop(point_cloud_read);

        // Cache should reflect new point count
        let cache = scanner.kd_tree_cache.read();
        let (_, cached_count) = cache.as_ref().unwrap();
        assert_eq!(*cached_count, 4, "Cache should be rebuilt with new point count");
    }

    #[test]
    fn test_kd_tree_cache_invalidates_on_reset() {
        let config = ScannerConfig::default();
        let mut scanner = HeadScanner::new(config).unwrap();

        // Add points and build cache
        let mut point_cloud = scanner.point_cloud.write();
        point_cloud.add_point(pointcloud::Point::new(0.0, 0.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(1.0, 0.0, 0.0));
        drop(point_cloud);

        let new_points = vec![pointcloud::Point::new(0.5, 0.5, 0.0)];
        let point_cloud_read = scanner.point_cloud.read();
        let _filtered = scanner.filter_new_points(&point_cloud_read, new_points);
        drop(point_cloud_read);

        // Verify cache exists
        assert!(scanner.kd_tree_cache.read().is_some());

        // Reset scanner
        scanner.reset();

        // Cache should be invalidated
        let cache = scanner.kd_tree_cache.read();
        assert!(cache.is_none(), "K-d tree cache should be None after reset");
    }

    #[test]
    fn test_kd_tree_cache_filter_correctness() {
        // Verify that cached filtering produces same results as uncached
        let config = ScannerConfig::default();
        let scanner = HeadScanner::new(config).unwrap();

        // Add points with specific spacing
        let mut point_cloud = scanner.point_cloud.write();
        point_cloud.add_point(pointcloud::Point::new(0.0, 0.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(10.0, 0.0, 0.0));
        point_cloud.add_point(pointcloud::Point::new(0.0, 10.0, 0.0));
        drop(point_cloud);

        // Points to filter: one close (should be filtered), one far (should pass)
        let new_points = vec![
            pointcloud::Point::new(0.01, 0.01, 0.0), // Very close to (0,0,0) - should be filtered
            pointcloud::Point::new(5.0, 5.0, 0.0),   // Far from all points - should pass
        ];

        let point_cloud_read = scanner.point_cloud.read();
        let filtered = scanner.filter_new_points(&point_cloud_read, new_points);
        drop(point_cloud_read);

        // Should only have the far point
        assert_eq!(filtered.len(), 1, "Should filter out close point");
        assert!((filtered[0].position.x - 5.0).abs() < 0.01);
        assert!((filtered[0].position.y - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_kd_tree_cache_with_empty_cloud() {
        // Verify cache behavior with empty point cloud
        let config = ScannerConfig::default();
        let scanner = HeadScanner::new(config).unwrap();

        let new_points = vec![
            pointcloud::Point::new(1.0, 1.0, 1.0),
            pointcloud::Point::new(2.0, 2.0, 2.0),
        ];

        let point_cloud_read = scanner.point_cloud.read();
        let filtered = scanner.filter_new_points(&point_cloud_read, new_points.clone());
        drop(point_cloud_read);

        // All points should pass through (no existing points to compare)
        assert_eq!(filtered.len(), 2, "All points should pass with empty cloud");

        // Cache should still be None (early return for empty cloud)
        let cache = scanner.kd_tree_cache.read();
        assert!(cache.is_none(), "Cache should remain None for empty cloud");
    }
}
