//! Bundle Adjustment for Structure-from-Motion
//!
//! Bundle adjustment is a refinement technique that simultaneously optimizes
//! camera poses and 3D point positions to minimize reprojection error across
//! all observations.
//!
//! # ⚠️ IMPORTANT: Placeholder Implementation
//!
//! This is a **simplified educational implementation** with known limitations:
//!
//! - **Jacobian**: Only computes partial derivatives, not full 2x6 camera Jacobian
//! - **Solver**: Uses diagonal approximation instead of proper sparse solver
//! - **Rotation**: Camera rotations are NOT optimized (translation only)
//! - **Accuracy**: Will NOT produce production-quality results
//!
//! For production use, consider:
//! - [ceres-solver](https://crates.io/crates/ceres-solver) bindings
//! - [g2o](https://github.com/RainerKuemmerle/g2o) via FFI
//! - Full Jacobian implementation with proper sparse linear solver
//!
//! See: Triggs et al., "Bundle Adjustment — A Modern Synthesis" (1999)

use crate::error::{ScannerError, ScannerResult};
use crate::reconstruction::{CameraIntrinsics, CameraPose};
use crate::vision::Feature;
use nalgebra::{Matrix3, Point2, Point3, Vector3};
use rayon::prelude::*;

/// A 3D point with observations from multiple cameras
#[derive(Debug, Clone)]
pub struct Point3DWithObservations {
    /// 3D position of the point
    pub position: Point3<f32>,

    /// List of (camera_index, observed_2d_position) pairs
    pub observations: Vec<(usize, Point2<f32>)>,
}

/// Bundle adjustment optimizer
///
/// Uses Levenberg-Marquardt algorithm to minimize reprojection error
pub struct BundleAdjuster {
    /// Camera intrinsics (assumed constant)
    intrinsics: CameraIntrinsics,

    /// Maximum number of iterations
    max_iterations: usize,

    /// Convergence threshold for cost function
    convergence_threshold: f32,

    /// Initial damping parameter for Levenberg-Marquardt
    initial_lambda: f32,
}

impl BundleAdjuster {
    /// Create a new bundle adjuster
    pub fn new(intrinsics: CameraIntrinsics) -> Self {
        Self {
            intrinsics,
            max_iterations: 50,
            convergence_threshold: 1e-6,
            initial_lambda: 1e-3,
        }
    }

    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set convergence threshold
    pub fn with_convergence_threshold(mut self, threshold: f32) -> Self {
        self.convergence_threshold = threshold;
        self
    }

    /// Optimize camera poses and 3D points
    ///
    /// Returns (refined_poses, refined_points)
    pub fn optimize(
        &self,
        poses: &[CameraPose],
        points: &[Point3DWithObservations],
    ) -> ScannerResult<(Vec<CameraPose>, Vec<Point3<f32>>)> {
        if poses.is_empty() || points.is_empty() {
            return Ok((poses.to_vec(), Vec::new()));
        }

        let mut current_poses = poses.to_vec();
        let mut current_points: Vec<Point3<f32>> = points.iter().map(|p| p.position).collect();

        let mut lambda = self.initial_lambda;
        let mut prev_cost = self.compute_cost(&current_poses, points, &current_points);

        for iteration in 0..self.max_iterations {
            // Compute Jacobian and residuals
            let (jacobian, residuals) =
                self.compute_jacobian_and_residuals(&current_poses, points, &current_points);

            // Solve the normal equations (J^T * J + λI) * δ = -J^T * r
            let delta = self.solve_normal_equations(&jacobian, &residuals, lambda)?;

            // Update parameters
            let (new_poses, new_points) =
                self.update_parameters(&current_poses, &current_points, &delta, poses.len());

            let new_cost = self.compute_cost(&new_poses, points, &new_points);

            // Check if update improved the cost
            if new_cost < prev_cost {
                // Accept update and decrease damping
                current_poses = new_poses;
                current_points = new_points;
                lambda *= 0.1;

                // Check convergence
                let cost_change = (prev_cost - new_cost).abs() / prev_cost;
                if cost_change < self.convergence_threshold {
                    log::info!(
                        "Bundle adjustment converged after {} iterations (cost: {:.6})",
                        iteration + 1,
                        new_cost
                    );
                    break;
                }

                prev_cost = new_cost;
            } else {
                // Reject update and increase damping
                lambda *= 10.0;

                // Prevent lambda from growing too large
                if lambda > 1e10 {
                    log::warn!("Bundle adjustment damping factor too large, stopping");
                    break;
                }
            }
        }

        log::info!("Bundle adjustment final cost: {:.6}", prev_cost);
        Ok((current_poses, current_points))
    }

    /// Compute the total reprojection error cost
    fn compute_cost(
        &self,
        poses: &[CameraPose],
        points_with_obs: &[Point3DWithObservations],
        points_3d: &[Point3<f32>],
    ) -> f32 {
        points_with_obs
            .par_iter()
            .enumerate()
            .map(|(point_idx, point_obs)| {
                let point_3d = &points_3d[point_idx];

                point_obs
                    .observations
                    .iter()
                    .map(|(cam_idx, observed_2d)| {
                        let pose = &poses[*cam_idx];

                        // Transform point to camera space
                        let point_cam = pose.to_camera(point_3d);

                        // Project to image plane
                        let projected = self.intrinsics.project(&point_cam);
                        let projected_2d = Point2::new(projected.0, projected.1);

                        // Compute reprojection error
                        let error = observed_2d - projected_2d;
                        error.norm_squared()
                    })
                    .sum::<f32>()
            })
            .sum::<f32>()
            / 2.0 // Standard formulation uses 1/2 * sum(errors^2)
    }

    /// Compute Jacobian matrix and residuals
    ///
    /// Returns (Jacobian, residuals)
    fn compute_jacobian_and_residuals(
        &self,
        poses: &[CameraPose],
        points_with_obs: &[Point3DWithObservations],
        points_3d: &[Point3<f32>],
    ) -> (Vec<Vec<f32>>, Vec<f32>) {
        let num_observations: usize = points_with_obs.iter().map(|p| p.observations.len()).sum();

        let num_params = poses.len() * 6 + points_3d.len() * 3; // 6 DOF per camera, 3 per point
        let num_residuals = num_observations * 2; // 2 residuals per observation (x, y)

        let mut jacobian = vec![vec![0.0; num_params]; num_residuals];
        let mut residuals = vec![0.0; num_residuals];

        let mut residual_idx = 0;

        for (point_idx, point_obs) in points_with_obs.iter().enumerate() {
            let point_3d = &points_3d[point_idx];

            for (cam_idx, observed_2d) in &point_obs.observations {
                let pose = &poses[*cam_idx];

                // Transform point to camera space
                let point_cam = pose.to_camera(point_3d);

                // Project to image plane
                let projected = self.intrinsics.project(&point_cam);
                let projected_2d = Point2::new(projected.0, projected.1);

                // Residual
                let error = observed_2d - projected_2d;
                residuals[residual_idx] = error.x;
                residuals[residual_idx + 1] = error.y;

                // Compute Jacobian with respect to camera parameters (simplified)
                // In practice, this would use automatic differentiation or analytical derivatives
                let cam_param_offset = cam_idx * 6;
                jacobian[residual_idx][cam_param_offset] = self.compute_cam_jacobian_x(&point_cam);
                jacobian[residual_idx + 1][cam_param_offset] =
                    self.compute_cam_jacobian_y(&point_cam);

                // Compute Jacobian with respect to 3D point
                let point_param_offset = poses.len() * 6 + point_idx * 3;
                self.compute_point_jacobian(
                    &mut jacobian[residual_idx],
                    &mut jacobian[residual_idx + 1],
                    point_param_offset,
                    &point_cam,
                );

                residual_idx += 2;
            }
        }

        (jacobian, residuals)
    }

    /// Compute camera jacobian for x coordinate (simplified)
    ///
    /// FIXME: This only computes a single scalar instead of the full 2x6 Jacobian matrix.
    /// A proper implementation needs:
    /// - ∂u/∂[tx, ty, tz, rx, ry, rz] (6 partial derivatives for x projection)
    /// - ∂v/∂[tx, ty, tz, rx, ry, rz] (6 partial derivatives for y projection)
    fn compute_cam_jacobian_x(&self, point_cam: &Point3<f32>) -> f32 {
        // PLACEHOLDER: Only partial derivative w.r.t. translation
        // Real implementation needs rotation derivatives using Lie algebra
        self.intrinsics.fx / point_cam.z
    }

    /// Compute camera jacobian for y coordinate (simplified)
    ///
    /// FIXME: See compute_cam_jacobian_x - same issue applies here
    fn compute_cam_jacobian_y(&self, point_cam: &Point3<f32>) -> f32 {
        self.intrinsics.fy / point_cam.z
    }

    /// Compute 3D point jacobian
    fn compute_point_jacobian(
        &self,
        jac_x: &mut [f32],
        jac_y: &mut [f32],
        offset: usize,
        point_cam: &Point3<f32>,
    ) {
        // Derivative of projection w.r.t. 3D point coordinates
        // d(u)/d(X) = fx/Z
        // d(u)/d(Y) = 0
        // d(u)/d(Z) = -fx*X/Z^2
        let z_sq = point_cam.z * point_cam.z;

        jac_x[offset] = self.intrinsics.fx / point_cam.z; // d(u)/d(X)
        jac_x[offset + 1] = 0.0; // d(u)/d(Y)
        jac_x[offset + 2] = -self.intrinsics.fx * point_cam.x / z_sq; // d(u)/d(Z)

        jac_y[offset] = 0.0; // d(v)/d(X)
        jac_y[offset + 1] = self.intrinsics.fy / point_cam.z; // d(v)/d(Y)
        jac_y[offset + 2] = -self.intrinsics.fy * point_cam.y / z_sq; // d(v)/d(Z)
    }

    /// Solve normal equations using simplified method
    ///
    /// FIXME: This uses diagonal approximation which ignores off-diagonal terms
    /// in J^T*J. This is mathematically incorrect and produces suboptimal results.
    ///
    /// Proper implementation should use:
    /// - Sparse Cholesky decomposition (nalgebra, faer, or sprs crate)
    /// - Conjugate Gradient for large problems
    /// - Schur complement trick to exploit problem structure
    ///
    /// The diagonal approximation assumes parameters are independent, which is
    /// violated in bundle adjustment where camera poses affect multiple points.
    fn solve_normal_equations(
        &self,
        jacobian: &[Vec<f32>],
        residuals: &[f32],
        lambda: f32,
    ) -> ScannerResult<Vec<f32>> {
        if jacobian.is_empty() || residuals.is_empty() {
            return Ok(Vec::new());
        }

        let num_params = jacobian[0].len();

        // FIXME: Only computing diagonal of J^T*J, ignoring off-diagonal correlation
        let mut jtj_diag = vec![lambda; num_params]; // Initialize with damping

        // Compute J^T * r
        let mut jtr = vec![0.0; num_params];
        for (row_idx, jac_row) in jacobian.iter().enumerate() {
            let r = residuals[row_idx];
            for (col_idx, &jac_val) in jac_row.iter().enumerate() {
                jtr[col_idx] -= jac_val * r;
                jtj_diag[col_idx] += jac_val * jac_val;
            }
        }

        // FIXME: Diagonal solve - mathematically incorrect but fast
        // Should use: nalgebra::Cholesky::new(jtj_full).solve(&jtr)
        let mut delta = vec![0.0; num_params];
        for i in 0..num_params {
            if jtj_diag[i].abs() > 1e-10 {
                delta[i] = jtr[i] / jtj_diag[i];
            }
        }

        Ok(delta)
    }

    /// Update camera poses and 3D points with delta
    ///
    /// FIXME: Rotation updates are completely skipped! Camera orientations are never optimized.
    /// Proper implementation needs to:
    /// - Use SO(3) Lie algebra to update rotations
    /// - Apply exponential map: R_new = R_old * exp(skew([rx, ry, rz]))
    /// - See "A tutorial on SE(3) transformation parameterizations" for details
    fn update_parameters(
        &self,
        poses: &[CameraPose],
        points: &[Point3<f32>],
        delta: &[f32],
        num_poses: usize,
    ) -> (Vec<CameraPose>, Vec<Point3<f32>>) {
        let mut new_poses = poses.to_vec();
        let mut new_points = points.to_vec();

        // Update camera poses (ONLY translation, rotation ignored)
        for (i, pose) in new_poses.iter_mut().enumerate() {
            let offset = i * 6;
            if offset + 5 < delta.len() {
                pose.position.x += delta[offset];
                pose.position.y += delta[offset + 1];
                pose.position.z += delta[offset + 2];

                // FIXME: Rotation updates (offset+3..offset+6) completely skipped!
                // This means camera orientations are NEVER optimized.
                // Should use: pose.rotation = pose.rotation * exp_map(delta[offset+3..offset+6])
            }
        }

        // Update 3D points
        let point_offset = num_poses * 6;
        for (i, point) in new_points.iter_mut().enumerate() {
            let offset = point_offset + i * 3;
            if offset + 2 < delta.len() {
                point.x += delta[offset];
                point.y += delta[offset + 1];
                point.z += delta[offset + 2];
            }
        }

        (new_poses, new_points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_adjuster_creation() {
        let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
        let adjuster = BundleAdjuster::new(intrinsics);

        assert_eq!(adjuster.max_iterations, 50);
        assert!(adjuster.convergence_threshold > 0.0);
    }

    #[test]
    fn test_bundle_adjustment_empty_data() {
        let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
        let adjuster = BundleAdjuster::new(intrinsics);

        let poses = vec![];
        let points = vec![];

        let result = adjuster.optimize(&poses, &points);
        assert!(result.is_ok());

        let (optimized_poses, optimized_points) = result.unwrap();
        assert!(optimized_poses.is_empty());
        assert!(optimized_points.is_empty());
    }

    #[test]
    fn test_cost_computation() {
        let intrinsics = CameraIntrinsics::default_webcam(1280, 720);
        let adjuster = BundleAdjuster::new(intrinsics);

        let poses = vec![CameraPose::identity()];
        let points_3d = vec![Point3::new(0.0, 0.0, 10.0)];

        let point_with_obs = Point3DWithObservations {
            position: points_3d[0],
            observations: vec![(0, Point2::new(640.0, 360.0))],
        };

        let cost = adjuster.compute_cost(&poses, &[point_with_obs], &points_3d);
        assert!(cost >= 0.0); // Cost should be non-negative
    }
}
