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
            HeadRegion::Front => 0, // Highest priority
            HeadRegion::FrontLeft => 1,
            HeadRegion::FrontRight => 1,
            HeadRegion::Left => 2,
            HeadRegion::Right => 2,
            HeadRegion::Top => 3,
            HeadRegion::BackLeft => 4,
            HeadRegion::BackRight => 4,
            HeadRegion::Back => 5, // Lower priority (hard to scan)
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

    /// Reprojection errors from feature tracking (for reconstruction quality)
    /// Stores recent reprojection errors in pixels
    reprojection_errors: std::collections::VecDeque<f32>,
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
            reprojection_errors: std::collections::VecDeque::with_capacity(100), // Track last 100 errors
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
        let distance = to_camera.norm();

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

    /// Record a reprojection error for reconstruction quality tracking
    ///
    /// # Arguments
    /// * `error_px` - Reprojection error in pixels
    ///
    /// This should be called after feature matching/tracking to track reconstruction quality
    pub fn record_reprojection_error(&mut self, error_px: f32) {
        if error_px.is_finite() && error_px >= 0.0 {
            self.reprojection_errors.push_back(error_px);

            // Keep only last 100 errors
            while self.reprojection_errors.len() > 100 {
                self.reprojection_errors.pop_front();
            }
        }
    }

    /// Compute motion blur score from recent camera movement
    ///
    /// Returns a score from 0.0 (no blur) to 1.0 (severe blur)
    ///
    /// Algorithm:
    /// 1. Calculate angular velocity from recent poses
    /// 2. High angular velocity → high blur
    /// 3. Threshold: ~60°/second is acceptable, >180°/second is severe blur
    fn compute_blur_score(&self) -> f32 {
        if self.recent_poses.len() < 2 {
            return 0.0; // Not enough data
        }

        // Calculate angular velocities between consecutive poses
        let mut angular_velocities = Vec::new();

        for i in 1..self.recent_poses.len() {
            let (time1, angle1) = &self.recent_poses[i - 1];
            let (time2, angle2) = &self.recent_poses[i];

            let time_diff = time2.duration_since(*time1).as_secs_f32();

            if time_diff > 0.0 && time_diff < 1.0 {
                // Only consider poses within 1 second
                let angular_dist = angle1.angular_distance(angle2);
                let angular_velocity = angular_dist / time_diff; // degrees per second

                angular_velocities.push(angular_velocity);
            }
        }

        if angular_velocities.is_empty() {
            return 0.0;
        }

        // Filter out NaN/infinity values before sorting to prevent panics
        angular_velocities.retain(|v| v.is_finite());

        if angular_velocities.is_empty() {
            return 0.0; // All values were NaN/infinity
        }

        // Use 90th percentile angular velocity (ignore outliers)
        angular_velocities.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let percentile_90 = angular_velocities
            [(angular_velocities.len() * 9 / 10).min(angular_velocities.len() - 1)];

        // Map angular velocity to blur score
        // 0-60°/s → 0.0 (no blur)
        // 60-180°/s → 0.0-0.8 (moderate blur)
        // >180°/s → 0.8-1.0 (severe blur)
        const LOW_THRESHOLD: f32 = 60.0; // degrees/second - acceptable motion
        const HIGH_THRESHOLD: f32 = 180.0; // degrees/second - severe blur

        if percentile_90 < LOW_THRESHOLD {
            0.0
        } else if percentile_90 < HIGH_THRESHOLD {
            // Linear mapping from 0.0 to 0.8
            (percentile_90 - LOW_THRESHOLD) / (HIGH_THRESHOLD - LOW_THRESHOLD) * 0.8
        } else {
            // Saturate at 1.0 for very high velocities
            ((percentile_90 - HIGH_THRESHOLD) / 120.0 + 0.8).min(1.0)
        }
    }

    /// Compute reconstruction error from reprojection errors
    ///
    /// Returns normalized error score from 0.0 (perfect) to 1.0 (poor quality)
    ///
    /// Algorithm:
    /// 1. Use median reprojection error (robust to outliers)
    /// 2. Good reconstruction: < 1 pixel error
    /// 3. Acceptable: 1-3 pixels
    /// 4. Poor: > 3 pixels
    fn compute_reconstruction_error(&self) -> f32 {
        if self.reprojection_errors.is_empty() {
            return 0.0; // No data yet, assume good
        }

        // Calculate median reprojection error (robust to outliers)
        let mut errors: Vec<f32> = self.reprojection_errors.iter().copied().collect();

        // Filter out NaN/infinity values before sorting to prevent panics
        errors.retain(|e| e.is_finite());

        if errors.is_empty() {
            return 0.0; // All values were NaN/infinity, assume good
        }

        errors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median_error = if errors.len() % 2 == 0 {
            (errors[errors.len() / 2 - 1] + errors[errors.len() / 2]) / 2.0
        } else {
            errors[errors.len() / 2]
        };

        // Map median error to normalized score
        // 0-1 px → 0.0-0.2 (excellent)
        // 1-3 px → 0.2-0.6 (acceptable)
        // >3 px → 0.6-1.0 (poor)
        const EXCELLENT_THRESHOLD: f32 = 1.0; // pixels
        const ACCEPTABLE_THRESHOLD: f32 = 3.0; // pixels
        const POOR_THRESHOLD: f32 = 10.0; // pixels

        if median_error < EXCELLENT_THRESHOLD {
            // Linear mapping from 0.0 to 0.2
            median_error / EXCELLENT_THRESHOLD * 0.2
        } else if median_error < ACCEPTABLE_THRESHOLD {
            // Linear mapping from 0.2 to 0.6
            0.2 + (median_error - EXCELLENT_THRESHOLD)
                / (ACCEPTABLE_THRESHOLD - EXCELLENT_THRESHOLD)
                * 0.4
        } else if median_error < POOR_THRESHOLD {
            // Linear mapping from 0.6 to 1.0
            0.6 + (median_error - ACCEPTABLE_THRESHOLD) / (POOR_THRESHOLD - ACCEPTABLE_THRESHOLD)
                * 0.4
        } else {
            1.0 // Cap at 1.0 for very poor quality
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

        // Blur score: compute from recent camera motion
        // 0.0 = no blur, 1.0 = severe motion blur
        let blur_score = self.compute_blur_score();

        // Reconstruction error: compute from reprojection errors
        // 0.0 = perfect reconstruction, 1.0 = poor quality
        let reconstruction_error = self.compute_reconstruction_error();

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

        format!("Please {} {}", horizontal_dir, vertical_dir)
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
    fn test_blur_score_calculation() {
        use std::time::{Duration, Instant};

        let mut guidance = ScanGuidance::new();

        // Simulate slow camera movement (low blur)
        let start_time = Instant::now();
        for i in 0..10 {
            let angle = ViewAngle::new(i as f32 * 5.0, 0.0, 50.0); // 5° increments
            let time = start_time + Duration::from_millis(i * 100); // 100ms apart
            guidance.recent_poses.push_back((time, angle));
        }

        let blur_score = guidance.compute_blur_score();
        assert!(
            blur_score < 0.3,
            "Slow movement should have low blur score, got {}",
            blur_score
        );

        // Simulate fast camera movement (high blur)
        guidance.recent_poses.clear();
        let start_time = Instant::now();
        for i in 0..10 {
            let angle = ViewAngle::new(i as f32 * 30.0, 0.0, 50.0); // 30° increments
            let time = start_time + Duration::from_millis(i * 50); // 50ms apart = 600°/s
            guidance.recent_poses.push_back((time, angle));
        }

        let blur_score_fast = guidance.compute_blur_score();
        assert!(
            blur_score_fast > 0.8,
            "Fast movement should have high blur score, got {}",
            blur_score_fast
        );
    }

    #[test]
    fn test_blur_score_thresholds() {
        use std::time::{Duration, Instant};

        let mut guidance = ScanGuidance::new();
        let start_time = Instant::now();

        // Test at LOW_THRESHOLD (60°/s) - should be ~0.0
        guidance.recent_poses.clear();
        for i in 0..10 {
            let angle = ViewAngle::new(i as f32 * 6.0, 0.0, 50.0); // 6° per 100ms = 60°/s
            let time = start_time + Duration::from_millis(i * 100);
            guidance.recent_poses.push_back((time, angle));
        }
        let blur_low = guidance.compute_blur_score();
        assert!(
            blur_low < 0.1,
            "At 60°/s threshold, blur should be near 0.0, got {}",
            blur_low
        );

        // Test at HIGH_THRESHOLD (180°/s) - should be ~0.8
        guidance.recent_poses.clear();
        for i in 0..10 {
            let angle = ViewAngle::new(i as f32 * 18.0, 0.0, 50.0); // 18° per 100ms = 180°/s
            let time = start_time + Duration::from_millis(i * 100);
            guidance.recent_poses.push_back((time, angle));
        }
        let blur_high = guidance.compute_blur_score();
        assert!(
            blur_high > 0.7 && blur_high < 0.9,
            "At 180°/s threshold, blur should be ~0.8, got {}",
            blur_high
        );
    }

    #[test]
    fn test_reconstruction_error_calculation() {
        let mut guidance = ScanGuidance::new();

        // Excellent reconstruction (< 1 pixel)
        for _ in 0..20 {
            guidance.record_reprojection_error(0.5);
        }
        let error_excellent = guidance.compute_reconstruction_error();
        assert!(
            error_excellent < 0.2,
            "Excellent reconstruction should have low error, got {}",
            error_excellent
        );

        // Acceptable reconstruction (1-3 pixels)
        guidance.reprojection_errors.clear();
        for _ in 0..20 {
            guidance.record_reprojection_error(2.0);
        }
        let error_acceptable = guidance.compute_reconstruction_error();
        assert!(
            error_acceptable > 0.2 && error_acceptable < 0.6,
            "Acceptable reconstruction should have moderate error, got {}",
            error_acceptable
        );

        // Poor reconstruction (> 3 pixels)
        guidance.reprojection_errors.clear();
        for _ in 0..20 {
            guidance.record_reprojection_error(5.0);
        }
        let error_poor = guidance.compute_reconstruction_error();
        assert!(
            error_poor > 0.6,
            "Poor reconstruction should have high error, got {}",
            error_poor
        );
    }

    #[test]
    fn test_reconstruction_error_median() {
        let mut guidance = ScanGuidance::new();

        // Add errors with outliers - median should be robust
        guidance.record_reprojection_error(0.5);
        guidance.record_reprojection_error(0.6);
        guidance.record_reprojection_error(0.7);
        guidance.record_reprojection_error(0.8);
        guidance.record_reprojection_error(100.0); // Outlier

        let error = guidance.compute_reconstruction_error();
        // Median should be ~0.7, not affected by 100.0 outlier
        assert!(error < 0.2, "Median should ignore outliers, got {}", error);
    }

    #[test]
    fn test_reprojection_error_recording() {
        let mut guidance = ScanGuidance::new();

        // Test normal error recording
        guidance.record_reprojection_error(1.5);
        assert_eq!(guidance.reprojection_errors.len(), 1);

        // Test NaN rejection
        guidance.record_reprojection_error(f32::NAN);
        assert_eq!(
            guidance.reprojection_errors.len(),
            1,
            "NaN should be rejected"
        );

        // Test negative rejection
        guidance.record_reprojection_error(-1.0);
        assert_eq!(
            guidance.reprojection_errors.len(),
            1,
            "Negative should be rejected"
        );

        // Test infinity rejection
        guidance.record_reprojection_error(f32::INFINITY);
        assert_eq!(
            guidance.reprojection_errors.len(),
            1,
            "Infinity should be rejected"
        );

        // Test buffer limit (max 100)
        for i in 0..150 {
            guidance.record_reprojection_error(i as f32);
        }
        assert_eq!(
            guidance.reprojection_errors.len(),
            100,
            "Should cap at 100 errors"
        );
    }

    #[test]
    fn test_blur_score_insufficient_data() {
        let guidance = ScanGuidance::new();

        // No poses - should return 0.0
        let blur = guidance.compute_blur_score();
        assert_eq!(blur, 0.0, "No data should return 0.0 blur");

        // Only 1 pose - should return 0.0
        let mut guidance_one = ScanGuidance::new();
        guidance_one
            .recent_poses
            .push_back((std::time::Instant::now(), ViewAngle::new(0.0, 0.0, 50.0)));
        let blur_one = guidance_one.compute_blur_score();
        assert_eq!(blur_one, 0.0, "One pose should return 0.0 blur");
    }

    #[test]
    fn test_reconstruction_error_no_data() {
        let guidance = ScanGuidance::new();

        // No errors - should return 0.0 (assume good)
        let error = guidance.compute_reconstruction_error();
        assert_eq!(error, 0.0, "No data should return 0.0 error");
    }

    #[test]
    fn test_quality_metrics_integration() {
        use crate::coverage::CoverageMap;
        use crate::reconstruction::CameraPose;
        use nalgebra::{Point3, UnitQuaternion};
        use std::time::{Duration, Instant};

        let mut guidance = ScanGuidance::new();
        let mut coverage = CoverageMap::new(100);

        // Add some coverage
        for i in 0..50 {
            coverage.add_point(&Point3::new(i as f32, 0.0, 0.0));
        }

        // Simulate camera movement with moderate speed
        let start_time = Instant::now();
        for i in 0..20 {
            let angle = ViewAngle::new(i as f32 * 10.0, 0.0, 50.0);
            let time = start_time + Duration::from_millis(i * 100);
            guidance.recent_poses.push_back((time, angle));
        }

        // Add some reprojection errors
        for _ in 0..30 {
            guidance.record_reprojection_error(1.5);
        }

        // Update poses
        let pose = CameraPose {
            position: Point3::new(0.0, 0.0, 50.0),
            rotation: UnitQuaternion::identity(),
        };
        guidance.update_pose(&pose, 50);

        // Compute quality
        let metrics = guidance.compute_quality(&coverage, 50);

        // Verify all metrics are computed
        assert!(metrics.coverage_percentage > 0.0);
        assert!(metrics.blur_score >= 0.0 && metrics.blur_score <= 1.0);
        assert!(metrics.reconstruction_error >= 0.0 && metrics.reconstruction_error <= 1.0);

        println!("Quality metrics:");
        println!("  Coverage: {:.1}%", metrics.coverage_percentage * 100.0);
        println!("  Blur score: {:.2}", metrics.blur_score);
        println!(
            "  Reconstruction error: {:.2}",
            metrics.reconstruction_error
        );
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
