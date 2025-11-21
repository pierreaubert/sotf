//! Scan guidance system for head scanning
//!
//! Provides real-time feedback to users about:
//! - Which areas need more coverage
//! - How to move the camera
//! - When scan quality is sufficient
//! - Angular coverage completeness

use crate::coverage::CoverageMap;
use crate::reconstruction::CameraPose;
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Viewing angle for head scanning
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewAngle {
    /// Horizontal rotation angle (0-360°)
    pub azimuth: f32,

    /// Vertical angle (-90 to 90°)
    pub elevation: f32,

    /// Distance from head center (cm)
    pub distance: f32,
}

impl ViewAngle {
    /// Create a new view angle
    pub fn new(azimuth: f32, elevation: f32, distance: f32) -> Self {
        Self {
            azimuth,
            elevation,
            distance,
        }
    }

    /// Compute angular distance between two view angles (in degrees)
    pub fn angular_distance(&self, other: &ViewAngle) -> f32 {
        // Spherical distance on unit sphere
        let az1 = self.azimuth.to_radians();
        let el1 = self.elevation.to_radians();
        let az2 = other.azimuth.to_radians();
        let el2 = other.elevation.to_radians();

        // Convert to Cartesian coordinates
        let x1 = el1.cos() * az1.cos();
        let y1 = el1.cos() * az1.sin();
        let z1 = el1.sin();

        let x2 = el2.cos() * az2.cos();
        let y2 = el2.cos() * az2.sin();
        let z2 = el2.sin();

        // Dot product gives cosine of angle
        let dot = x1 * x2 + y1 * y2 + z1 * z2;
        let angle = dot.clamp(-1.0, 1.0).acos();

        angle.to_degrees()
    }

    /// Check if this angle is approximately equal to another (within threshold)
    pub fn is_similar_to(&self, other: &ViewAngle, threshold_degrees: f32) -> bool {
        self.angular_distance(other) < threshold_degrees
    }
}

/// Head region identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeadRegion {
    Front,
    FrontLeft,
    FrontRight,
    Left,
    Right,
    BackLeft,
    BackRight,
    Back,
    Top,
    TopLeft,
    TopRight,
}

impl HeadRegion {
    /// Get the canonical viewing angle for this region
    pub fn canonical_angle(&self) -> ViewAngle {
        let distance = 50.0; // 50cm default distance
        match self {
            HeadRegion::Front => ViewAngle::new(0.0, 0.0, distance),
            HeadRegion::FrontLeft => ViewAngle::new(45.0, 0.0, distance),
            HeadRegion::FrontRight => ViewAngle::new(315.0, 0.0, distance),
            HeadRegion::Left => ViewAngle::new(90.0, 0.0, distance),
            HeadRegion::Right => ViewAngle::new(270.0, 0.0, distance),
            HeadRegion::BackLeft => ViewAngle::new(135.0, 0.0, distance),
            HeadRegion::BackRight => ViewAngle::new(225.0, 0.0, distance),
            HeadRegion::Back => ViewAngle::new(180.0, 0.0, distance),
            HeadRegion::Top => ViewAngle::new(0.0, 45.0, distance),
            HeadRegion::TopLeft => ViewAngle::new(90.0, 30.0, distance),
            HeadRegion::TopRight => ViewAngle::new(270.0, 30.0, distance),
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            HeadRegion::Front => "Front",
            HeadRegion::FrontLeft => "Front-Left (45°)",
            HeadRegion::FrontRight => "Front-Right (315°)",
            HeadRegion::Left => "Left Side (90°)",
            HeadRegion::Right => "Right Side (270°)",
            HeadRegion::BackLeft => "Back-Left (135°)",
            HeadRegion::BackRight => "Back-Right (225°)",
            HeadRegion::Back => "Back",
            HeadRegion::Top => "Top",
            HeadRegion::TopLeft => "Top-Left",
            HeadRegion::TopRight => "Top-Right",
        }
    }

    /// Get priority (0 = highest, higher number = lower priority)
    pub fn priority(&self) -> u8 {
        match self {
            HeadRegion::Front => 0,         // Highest priority
            HeadRegion::FrontLeft => 1,
            HeadRegion::FrontRight => 1,
            HeadRegion::Left => 2,
            HeadRegion::Right => 2,
            HeadRegion::Top => 3,
            HeadRegion::BackLeft => 4,
            HeadRegion::BackRight => 4,
            HeadRegion::Back => 5,          // Lower priority (hard to scan)
            HeadRegion::TopLeft => 6,
            HeadRegion::TopRight => 6,
        }
    }

    /// Get all required regions for complete head scan
    pub fn all_regions() -> Vec<HeadRegion> {
        vec![
            HeadRegion::Front,
            HeadRegion::FrontLeft,
            HeadRegion::FrontRight,
            HeadRegion::Left,
            HeadRegion::Right,
            HeadRegion::BackLeft,
            HeadRegion::BackRight,
            HeadRegion::Back,
            HeadRegion::Top,
            HeadRegion::TopLeft,
            HeadRegion::TopRight,
        ]
    }
}

/// Scan quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Overall coverage percentage (0.0 to 1.0)
    pub coverage_percentage: f32,

    /// Angular coverage percentage (0.0 to 1.0)
    pub angular_coverage: f32,

    /// Average point density (points per cm²)
    pub point_density: f32,

    /// Motion blur score (0.0 = no blur, 1.0 = severe blur)
    pub blur_score: f32,

    /// Number of unique viewing angles captured
    pub unique_angles: usize,

    /// Estimated reconstruction error (reprojection error in pixels)
    pub reconstruction_error: f32,
}

impl QualityMetrics {
    /// Create quality metrics with default values
    pub fn default() -> Self {
        Self {
            coverage_percentage: 0.0,
            angular_coverage: 0.0,
            point_density: 0.0,
            blur_score: 0.0,
            unique_angles: 0,
            reconstruction_error: 0.0,
        }
    }

    /// Compute overall quality score (0.0 to 1.0)
    pub fn overall_score(&self) -> f32 {
        // Weighted average of metrics
        let coverage_weight = 0.4;
        let angular_weight = 0.3;
        let density_weight = 0.2;
        let blur_penalty = 0.1;

        let coverage_score = self.coverage_percentage;
        let angular_score = self.angular_coverage;
        let density_score = (self.point_density / 50.0).min(1.0); // Normalize to 50 points/cm²
        let blur_score = 1.0 - self.blur_score; // Invert: less blur = better

        (coverage_score * coverage_weight
            + angular_score * angular_weight
            + density_score * density_weight
            + blur_score * blur_penalty)
            .clamp(0.0, 1.0)
    }

    /// Check if scan quality is acceptable
    pub fn is_acceptable(&self) -> bool {
        self.coverage_percentage >= 0.85
            && self.angular_coverage >= 0.70
            && self.point_density >= 30.0
            && self.blur_score < 0.3
    }

    /// Check if scan quality is excellent
    pub fn is_excellent(&self) -> bool {
        self.coverage_percentage >= 0.95
            && self.angular_coverage >= 0.90
            && self.point_density >= 50.0
            && self.blur_score < 0.15
    }
}

/// User instruction for next scan action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceInstruction {
    /// Direction to move/rotate (e.g., "Turn head 45° right")
    pub direction: String,

    /// Target region name (e.g., "Right ear area")
    pub region_name: String,

    /// Current coverage before this action
    pub coverage_before: f32,

    /// Estimated coverage after this action
    pub coverage_after_estimate: f32,

    /// Target viewing angle
    pub target_angle: ViewAngle,

    /// Priority (0 = highest)
    pub priority: u8,
}

/// Scan guidance system
#[derive(Clone)]
pub struct ScanGuidance {
    /// Head center position (estimated)
    head_center: Option<Point3<f32>>,

    /// Head radius (estimated from bounding sphere)
    head_radius: f32,

    /// Captured viewing angles
    captured_angles: Vec<ViewAngle>,

    /// Covered regions
    covered_regions: HashSet<HeadRegion>,

    /// Current quality metrics
    quality_metrics: QualityMetrics,

    /// Angular similarity threshold (degrees)
    /// This determines when two viewing angles are considered too similar
    angle_threshold: f32,

    /// Recent pose updates (for motion blur estimation)
    /// Stores last N (timestamp, ViewAngle) pairs
    recent_poses: std::collections::VecDeque<(std::time::Instant, ViewAngle)>,
}

impl ScanGuidance {
    /// Create a new scan guidance system
    pub fn new() -> Self {
        Self {
            head_center: None,
            head_radius: 25.0, // Default ~25cm head radius (typical adult head)
            captured_angles: Vec::new(),
            covered_regions: HashSet::new(),
            quality_metrics: QualityMetrics::default(),
            angle_threshold: 20.0, // 20° threshold for "similar" angles
            recent_poses: std::collections::VecDeque::with_capacity(30), // Track last 30 poses
        }
    }

    /// Update with new camera pose
    pub fn update_pose(&mut self, pose: &CameraPose, point_count: usize) {
        // Estimate head center if not set
        if self.head_center.is_none() {
            self.head_center = Some(pose.position.clone());
        }

        // Compute viewing angle from pose
        let angle = self.estimate_viewing_angle(pose);

        // Track recent poses for motion blur estimation
        let now = std::time::Instant::now();
        self.recent_poses.push_back((now, angle));

        // Keep only last 30 poses (approx 1 second at 30fps)
        while self.recent_poses.len() > 30 {
            self.recent_poses.pop_front();
        }

        // Check if this is a new unique angle
        let is_new_angle = !self
            .captured_angles
            .iter()
            .any(|a| a.is_similar_to(&angle, self.angle_threshold));

        if is_new_angle {
            self.captured_angles.push(angle);
            log::debug!(
                "New viewing angle captured: azimuth={:.1}°, elevation={:.1}° (total: {})",
                angle.azimuth,
                angle.elevation,
                self.captured_angles.len()
            );

            // Update covered regions
            self.update_covered_regions();
        }
    }

    /// Estimate viewing angle from camera pose
    fn estimate_viewing_angle(&self, pose: &CameraPose) -> ViewAngle {
        let head_center = self.head_center.unwrap_or(Point3::origin());

        // Vector from head center to camera
        let to_camera = pose.position - head_center;
        let distance = to_camera.coords.norm();

        // Compute azimuth (horizontal angle)
        let azimuth = to_camera.z.atan2(to_camera.x).to_degrees();

        // Compute elevation (vertical angle)
        let horizontal_dist = (to_camera.x.powi(2) + to_camera.z.powi(2)).sqrt();
        let elevation = to_camera.y.atan2(horizontal_dist).to_degrees();

        ViewAngle::new(azimuth, elevation, distance)
    }

    /// Update which regions have been covered
    fn update_covered_regions(&mut self) {
        for region in HeadRegion::all_regions() {
            let canonical = region.canonical_angle();

            // Check if we have an angle close to this region's canonical angle
            let is_covered = self
                .captured_angles
                .iter()
                .any(|a| a.is_similar_to(&canonical, self.angle_threshold));

            if is_covered {
                self.covered_regions.insert(region);
            }
        }
    }

    /// Compute quality metrics
    pub fn compute_quality(
        &mut self,
        coverage_map: &CoverageMap,
        point_count: usize,
    ) -> QualityMetrics {
        // Coverage percentage from coverage map
        let coverage_percentage = coverage_map.get_coverage_percentage();

        // Angular coverage: what fraction of required regions are covered?
        let total_regions = HeadRegion::all_regions().len() as f32;
        let covered_count = self.covered_regions.len() as f32;
        let angular_coverage = covered_count / total_regions;

        // Point density estimation (rough approximation)
        // Assume head surface area ≈ 4πr² ≈ 4 * 3.14 * 25² ≈ 7850 cm²
        let estimated_surface_area = 4.0 * std::f32::consts::PI * self.head_radius.powi(2);
        let point_density = if estimated_surface_area > 0.0 {
            point_count as f32 / estimated_surface_area
        } else {
            0.0
        };

        // Blur score: placeholder for now (would need actual image analysis)
        let blur_score = 0.0; // TODO: Implement motion blur detection

        // Reconstruction error: placeholder
        let reconstruction_error = 0.0; // TODO: Implement from bundle adjustment

        self.quality_metrics = QualityMetrics {
            coverage_percentage,
            angular_coverage,
            point_density,
            blur_score,
            unique_angles: self.captured_angles.len(),
            reconstruction_error,
        };

        self.quality_metrics.clone()
    }

    /// Get the next recommended scan action
    pub fn compute_next_target(&self) -> Option<GuidanceInstruction> {
        // Find uncovered regions sorted by priority
        let mut uncovered: Vec<HeadRegion> = HeadRegion::all_regions()
            .into_iter()
            .filter(|r| !self.covered_regions.contains(r))
            .collect();

        if uncovered.is_empty() {
            return None; // All regions covered!
        }

        // Sort by priority
        uncovered.sort_by_key(|r| r.priority());

        // Take highest priority uncovered region
        let target_region = uncovered[0];
        let target_angle = target_region.canonical_angle();

        // Generate instruction text
        let direction = self.generate_direction_text(&target_angle);

        // Estimate coverage improvement (rough estimate)
        let coverage_improvement = 1.0 / HeadRegion::all_regions().len() as f32;

        Some(GuidanceInstruction {
            direction,
            region_name: target_region.name().to_string(),
            coverage_before: self.quality_metrics.angular_coverage,
            coverage_after_estimate: self.quality_metrics.angular_coverage + coverage_improvement,
            target_angle,
            priority: target_region.priority(),
        })
    }

    /// Generate human-readable direction text
    fn generate_direction_text(&self, target: &ViewAngle) -> String {
        let azimuth = target.azimuth;
        let elevation = target.elevation;

        let horizontal_dir = if azimuth >= 315.0 || azimuth < 45.0 {
            "face forward"
        } else if azimuth >= 45.0 && azimuth < 135.0 {
            "turn left"
        } else if azimuth >= 135.0 && azimuth < 225.0 {
            "turn around (back view)"
        } else {
            "turn right"
        };

        let vertical_dir = if elevation > 20.0 {
            ", tilt head down (camera above)"
        } else if elevation < -20.0 {
            ", tilt head up (camera below)"
        } else {
            ""
        };

        format!(
            "Please {} {}",
            horizontal_dir,
            vertical_dir
        )
    }

    /// Get suggestions for improving scan quality
    pub fn suggest_improvements(&self) -> Vec<String> {
        let mut suggestions = Vec::new();

        if self.quality_metrics.angular_coverage < 0.7 {
            suggestions.push(format!(
                "Move around the head more - only {}/{} angles captured",
                self.covered_regions.len(),
                HeadRegion::all_regions().len()
            ));
        }

        if self.quality_metrics.coverage_percentage < 0.85 {
            suggestions.push(format!(
                "Increase coverage - currently at {:.0}%",
                self.quality_metrics.coverage_percentage * 100.0
            ));
        }

        if self.quality_metrics.point_density < 30.0 {
            suggestions.push(format!(
                "Move closer to capture more detail (current density: {:.1} points/cm²)",
                self.quality_metrics.point_density
            ));
        }

        if self.quality_metrics.blur_score > 0.3 {
            suggestions.push("Move slower to reduce motion blur".to_string());
        }

        if suggestions.is_empty() {
            suggestions.push("Scan quality is good! Keep going.".to_string());
        }

        suggestions
    }

    /// Get current quality metrics
    pub fn get_quality_metrics(&self) -> &QualityMetrics {
        &self.quality_metrics
    }

    /// Get list of covered regions
    pub fn get_covered_regions(&self) -> Vec<HeadRegion> {
        self.covered_regions.iter().copied().collect()
    }

    /// Get list of uncovered regions
    pub fn get_uncovered_regions(&self) -> Vec<HeadRegion> {
        HeadRegion::all_regions()
            .into_iter()
            .filter(|r| !self.covered_regions.contains(r))
            .collect()
    }

    /// Get number of unique viewing angles captured
    pub fn get_unique_angle_count(&self) -> usize {
        self.captured_angles.len()
    }
}

impl Default for ScanGuidance {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_angle_distance() {
        let angle1 = ViewAngle::new(0.0, 0.0, 50.0);
        let angle2 = ViewAngle::new(90.0, 0.0, 50.0);

        let dist = angle1.angular_distance(&angle2);
        assert!(dist > 85.0 && dist < 95.0); // Should be ~90°
    }

    #[test]
    fn test_head_regions() {
        let regions = HeadRegion::all_regions();
        assert_eq!(regions.len(), 11);

        let front = HeadRegion::Front;
        assert_eq!(front.priority(), 0);
        assert_eq!(front.name(), "Front");
    }

    #[test]
    fn test_quality_metrics() {
        let metrics = QualityMetrics {
            coverage_percentage: 0.9,
            angular_coverage: 0.8,
            point_density: 45.0,
            blur_score: 0.1,
            unique_angles: 8,
            reconstruction_error: 0.5,
        };

        assert!(metrics.is_acceptable());
        let score = metrics.overall_score();
        assert!(score > 0.8);
    }

    #[test]
    fn test_guidance_system() {
        let mut guidance = ScanGuidance::new();

        // Simulate capturing front view
        let pose_front = CameraPose {
            position: Point3::new(0.0, 0.0, 50.0),
            rotation: nalgebra::Matrix3::identity(),
        };

        guidance.update_pose(&pose_front, 100);
        assert!(guidance.get_unique_angle_count() > 0);

        // Should suggest next uncovered region
        let instruction = guidance.compute_next_target();
        assert!(instruction.is_some());
    }
}
