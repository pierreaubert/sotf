//! Bundle Adjustment - Full Jacobian Implementation

use crate::error::ScannerResult;
use crate::reconstruction::{CameraIntrinsics, CameraPose};
use nalgebra::{
    DMatrix, DVector, Matrix2x3, Matrix2x6, Matrix3, Matrix3x6, Point2, Point3, Vector3, Vector6,
};
use rayon::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Point3DWithObservations {
    pub position: Point3<f32>,
    pub observations: Vec<(usize, Point2<f32>)>,
}

pub struct BundleAdjuster {
    intrinsics: CameraIntrinsics,
    max_iterations: usize,
    convergence_threshold: f32,
    initial_lambda: f32,
}

fn skew_symmetric(v: &Vector3<f32>) -> Matrix3<f32> {
    Matrix3::new(0.0, -v.z, v.y, v.z, 0.0, -v.x, -v.y, v.x, 0.0)
}

fn exp_map(omega: &Vector3<f32>) -> Matrix3<f32> {
    let theta = omega.norm();
    if theta < 1e-8 {
        return Matrix3::identity() + skew_symmetric(omega);
    }
    let omega_normalized = omega / theta;
    let k = skew_symmetric(&omega_normalized);
    let k_sq = k * k;
    Matrix3::identity() + k * theta.sin() + k_sq * (1.0 - theta.cos())
}

impl BundleAdjuster {
    pub fn new(intrinsics: CameraIntrinsics) -> Self {
        Self {
            intrinsics,
            max_iterations: 50,
            convergence_threshold: 1e-6,
            initial_lambda: 1e-3,
        }
    }

    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    pub fn with_convergence_threshold(mut self, threshold: f32) -> Self {
        self.convergence_threshold = threshold;
        self
    }

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
            let (camera_jacs, point_jacs, residuals) =
                self.compute_jacobian_and_residuals(&current_poses, points, &current_points);

            let delta = self.solve_normal_equations(
                &camera_jacs,
                &point_jacs,
                &residuals,
                current_poses.len(),
                current_points.len(),
                lambda,
            )?;

            let (new_poses, new_points) =
                self.update_parameters(&current_poses, &current_points, &delta, poses.len());

            let new_cost = self.compute_cost(&new_poses, points, &new_points);

            if new_cost < prev_cost {
                current_poses = new_poses;
                current_points = new_points;
                lambda *= 0.1;

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
                lambda *= 10.0;
                if lambda > 1e10 {
                    log::warn!("Bundle adjustment damping factor too large, stopping");
                    break;
                }
            }
        }

        log::info!("Bundle adjustment final cost: {:.6}", prev_cost);
        Ok((current_poses, current_points))
    }

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
                        let point_cam = pose.to_camera(point_3d);
                        let projected = self.intrinsics.project(&point_cam);
                        let projected_2d = Point2::new(projected.0, projected.1);
                        let error = observed_2d - projected_2d;
                        error.norm_squared()
                    })
                    .sum::<f32>()
            })
            .sum::<f32>()
            / 2.0
    }

    fn compute_jacobian_and_residuals(
        &self,
        poses: &[CameraPose],
        points_with_obs: &[Point3DWithObservations],
        points_3d: &[Point3<f32>],
    ) -> (
        Vec<(usize, usize, Matrix2x6<f32>)>,
        Vec<(usize, usize, Matrix2x3<f32>)>,
        Vec<f32>,
    ) {
        let num_observations: usize = points_with_obs.iter().map(|p| p.observations.len()).sum();
        let num_residuals = num_observations * 2;

        let mut camera_jacobians = Vec::new();
        let mut point_jacobians = Vec::new();
        let mut residuals = vec![0.0; num_residuals];

        let mut residual_idx = 0;

        for (point_idx, point_obs) in points_with_obs.iter().enumerate() {
            let point_3d = &points_3d[point_idx];

            for (cam_idx, observed_2d) in &point_obs.observations {
                let pose = &poses[*cam_idx];
                let point_cam = pose.to_camera(point_3d);
                let projected = self.intrinsics.project(&point_cam);
                let projected_2d = Point2::new(projected.0, projected.1);

                let error = observed_2d - projected_2d;
                residuals[residual_idx] = error.x;
                residuals[residual_idx + 1] = error.y;

                let j_cam = self.compute_camera_jacobian(point_3d, &point_cam, pose);
                camera_jacobians.push((residual_idx, *cam_idx, j_cam));

                let j_point = self.compute_point_jacobian_matrix(&point_cam, pose);
                point_jacobians.push((residual_idx, point_idx, j_point));

                residual_idx += 2;
            }
        }

        (camera_jacobians, point_jacobians, residuals)
    }

    fn compute_camera_jacobian(
        &self,
        point_world: &Point3<f32>,
        point_cam: &Point3<f32>,
        pose: &CameraPose,
    ) -> Matrix2x6<f32> {
        let fx = self.intrinsics.fx;
        let fy = self.intrinsics.fy;
        let x = point_cam.x;
        let y = point_cam.y;
        let z = point_cam.z;
        let z_inv = 1.0 / z;
        let z_inv_sq = z_inv * z_inv;

        let j_proj = Matrix2x3::new(
            fx * z_inv,
            0.0,
            -fx * x * z_inv_sq,
            0.0,
            fy * z_inv,
            -fy * y * z_inv_sq,
        );

        let r_t = pose.rotation.transpose();
        let j_trans = -r_t;

        let p_diff = point_world - pose.position;
        let p_diff_vec = Vector3::new(p_diff.x, p_diff.y, p_diff.z);
        let skew = skew_symmetric(&p_diff_vec);
        let j_rot = -r_t * skew;

        let mut j_cam_3x6 = Matrix3x6::zeros();
        j_cam_3x6.fixed_view_mut::<3, 3>(0, 0).copy_from(&j_trans);
        j_cam_3x6.fixed_view_mut::<3, 3>(0, 3).copy_from(&j_rot);

        j_proj * j_cam_3x6
    }

    fn compute_point_jacobian_matrix(
        &self,
        point_cam: &Point3<f32>,
        pose: &CameraPose,
    ) -> Matrix2x3<f32> {
        let fx = self.intrinsics.fx;
        let fy = self.intrinsics.fy;
        let x = point_cam.x;
        let y = point_cam.y;
        let z = point_cam.z;
        let z_inv = 1.0 / z;
        let z_inv_sq = z_inv * z_inv;

        let j_proj = Matrix2x3::new(
            fx * z_inv,
            0.0,
            -fx * x * z_inv_sq,
            0.0,
            fy * z_inv,
            -fy * y * z_inv_sq,
        );

        let r_t = pose.rotation.transpose();
        j_proj * r_t
    }

    #[allow(clippy::too_many_arguments)]
    fn solve_normal_equations(
        &self,
        camera_jacs: &[(usize, usize, Matrix2x6<f32>)],
        point_jacs: &[(usize, usize, Matrix2x3<f32>)],
        residuals: &[f32],
        num_cameras: usize,
        num_points: usize,
        lambda: f32,
    ) -> ScannerResult<Vec<f32>> {
        if camera_jacs.is_empty() || point_jacs.is_empty() {
            return Ok(vec![0.0; num_cameras * 6 + num_points * 3]);
        }

        let cam_params = num_cameras * 6;
        let point_params = num_points * 3;

        let mut c_blocks: Vec<Matrix3<f32>> = vec![Matrix3::zeros(); num_points];
        let mut jtp_r: Vec<Vector3<f32>> = vec![Vector3::zeros(); num_points];

        for (res_idx, point_idx, j_point) in point_jacs {
            let r_2d = nalgebra::Vector2::new(residuals[*res_idx], residuals[*res_idx + 1]);
            c_blocks[*point_idx] += j_point.transpose() * j_point;
            jtp_r[*point_idx] += j_point.transpose() * r_2d;
        }

        let mut c_inv_blocks: Vec<Matrix3<f32>> = Vec::with_capacity(num_points);
        for c_block in &c_blocks {
            let c_damped = c_block + Matrix3::identity() * lambda;
            match c_damped.try_inverse() {
                Some(inv) => c_inv_blocks.push(inv),
                None => {
                    let c_more_damped = c_block + Matrix3::identity() * (lambda * 10.0);
                    c_inv_blocks.push(
                        c_more_damped
                            .try_inverse()
                            .unwrap_or_else(|| Matrix3::identity() * 0.001),
                    );
                }
            }
        }

        let mut a_matrix = DMatrix::zeros(cam_params, cam_params);
        let mut jtc_r = DVector::zeros(cam_params);

        for (res_idx, cam_idx, j_cam) in camera_jacs {
            let r_2d = nalgebra::Vector2::new(residuals[*res_idx], residuals[*res_idx + 1]);

            let cam_offset = cam_idx * 6;
            let jtj = j_cam.transpose() * j_cam;
            for i in 0..6 {
                for j in 0..6 {
                    a_matrix[(cam_offset + i, cam_offset + j)] += jtj[(i, j)];
                }
            }

            let jtr = j_cam.transpose() * r_2d;
            for i in 0..6 {
                jtc_r[cam_offset + i] += jtr[i];
            }
        }

        let mut schur_rhs = jtc_r.clone();

        for ((res_idx_c, cam_idx, j_cam), (res_idx_p, point_idx, j_point)) in
            camera_jacs.iter().zip(point_jacs.iter())
        {
            if res_idx_c != res_idx_p {
                continue;
            }

            let b = j_cam.transpose() * j_point;

            let c_inv_jtp_r = c_inv_blocks[*point_idx] * jtp_r[*point_idx];
            let b_c_inv_jtp_r = b * c_inv_jtp_r;
            let cam_offset = cam_idx * 6;
            for i in 0..6 {
                schur_rhs[cam_offset + i] += b_c_inv_jtp_r[i];
            }

            let b_c_inv = b * c_inv_blocks[*point_idx];
            let b_c_inv_bt = b_c_inv * b.transpose();
            for i in 0..6 {
                for j in 0..6 {
                    a_matrix[(cam_offset + i, cam_offset + j)] -= b_c_inv_bt[(i, j)];
                }
            }
        }

        for i in 0..cam_params {
            a_matrix[(i, i)] += lambda;
        }

        let delta_cam = match a_matrix.clone().cholesky() {
            Some(chol) => chol.solve(&schur_rhs),
            None => match a_matrix.clone().lu().solve(&schur_rhs) {
                Some(sol) => sol,
                None => {
                    log::warn!("Failed to solve camera system, using zero update");
                    DVector::zeros(cam_params)
                }
            },
        };

        let mut delta_points = vec![0.0; point_params];

        let mut point_observations: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
        for ((res_idx, cam_idx, _), (_, point_idx, _)) in camera_jacs.iter().zip(point_jacs.iter())
        {
            point_observations
                .entry(*point_idx)
                .or_default()
                .push((*res_idx, *cam_idx));
        }

        for point_idx in 0..num_points {
            let mut rhs = -jtp_r[point_idx];

            if let Some(observations) = point_observations.get(&point_idx) {
                for (res_idx, cam_idx) in observations {
                    let j_cam = camera_jacs
                        .iter()
                        .find(|(r, c, _)| r == res_idx && c == cam_idx)
                        .map(|(_, _, j)| j)
                        .unwrap();
                    let j_point = point_jacs
                        .iter()
                        .find(|(r, p, _)| r == res_idx && p == &point_idx)
                        .map(|(_, _, j)| j)
                        .unwrap();

                    let b = j_cam.transpose() * j_point;
                    let cam_offset = cam_idx * 6;
                    let delta_c = Vector6::from_iterator((0..6).map(|i| delta_cam[cam_offset + i]));
                    rhs -= b.transpose() * delta_c;
                }
            }

            let delta_p = c_inv_blocks[point_idx] * rhs;
            let point_offset = point_idx * 3;
            delta_points[point_offset] = delta_p[0];
            delta_points[point_offset + 1] = delta_p[1];
            delta_points[point_offset + 2] = delta_p[2];
        }

        let mut delta = vec![0.0; cam_params + point_params];
        for i in 0..cam_params {
            delta[i] = delta_cam[i];
        }
        for i in 0..point_params {
            delta[cam_params + i] = delta_points[i];
        }

        Ok(delta)
    }

    fn update_parameters(
        &self,
        poses: &[CameraPose],
        points: &[Point3<f32>],
        delta: &[f32],
        num_poses: usize,
    ) -> (Vec<CameraPose>, Vec<Point3<f32>>) {
        let mut new_poses = poses.to_vec();
        let mut new_points = points.to_vec();

        for (i, pose) in new_poses.iter_mut().enumerate() {
            let offset = i * 6;
            if offset + 5 < delta.len() {
                pose.position.x += delta[offset];
                pose.position.y += delta[offset + 1];
                pose.position.z += delta[offset + 2];

                let omega = Vector3::new(delta[offset + 3], delta[offset + 4], delta[offset + 5]);
                let delta_r = exp_map(&omega);
                pose.rotation *= delta_r;
            }
        }

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
        assert!(cost >= 0.0);
    }

    #[test]
    fn test_skew_symmetric() {
        let v = Vector3::new(1.0, 2.0, 3.0);
        let skew = skew_symmetric(&v);
        assert!((skew + skew.transpose()).norm() < 1e-6);
        assert_eq!(skew[(0, 1)], -3.0);
        assert_eq!(skew[(1, 0)], 3.0);
    }

    #[test]
    fn test_exp_map_identity() {
        let omega = Vector3::zeros();
        let r = exp_map(&omega);
        assert!((r - Matrix3::identity()).norm() < 1e-6);
    }

    #[test]
    fn test_exp_map_orthogonal() {
        let omega = Vector3::new(0.5, 1.0, 0.3);
        let r = exp_map(&omega);
        let should_be_identity = r.transpose() * r;
        assert!((should_be_identity - Matrix3::identity()).norm() < 1e-5);
        assert!((r.determinant() - 1.0).abs() < 1e-5);
    }
}
