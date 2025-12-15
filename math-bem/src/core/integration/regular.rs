//! Regular (non-singular) element integration
//!
//! Adaptive integration for elements when source and field elements are different.
//! Direct port of NC_RegularIntegration from NC_EquationSystem.cpp.

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use std::f64::consts::PI;

use crate::core::integration::gauss::{quad_quadrature, triangle_quadrature};
use crate::core::integration::singular::generate_subelements;
use crate::core::mesh::element::{cross_product, normalize};
use crate::core::types::{ElementType, IntegrationResult, PhysicsParams};

/// Perform regular integration over a field element
///
/// This implements NC_RegularIntegration from the C++ code.
/// Uses adaptive subelement subdivision based on distance from source point.
///
/// # Arguments
/// * `source_point` - Source point (collocation point)
/// * `source_normal` - Unit normal at source point
/// * `element_coords` - Node coordinates of the field element (num_nodes × 3)
/// * `element_type` - Triangle or quad element
/// * `element_area` - Area of the field element
/// * `physics` - Physics parameters (wave number, etc.)
/// * `bc_values` - Boundary condition values at element nodes (for RHS)
/// * `bc_type` - Boundary condition type (0=velocity, 1=pressure)
/// * `compute_rhs` - Whether to compute RHS contribution
///
/// # Returns
/// IntegrationResult with G, H, H^T, E integrals and RHS contribution
pub fn regular_integration(
    source_point: &Array1<f64>,
    source_normal: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    element_area: f64,
    physics: &PhysicsParams,
    bc_values: Option<&[Complex64]>,
    bc_type: i32,
    compute_rhs: bool,
) -> IntegrationResult {
    let wavruim = physics.harmonic_factor * physics.wave_number;
    let k2 = physics.wave_number * physics.wave_number;

    let mut result = IntegrationResult::default();

    // Generate adaptive subelements
    let subelements =
        generate_subelements(source_point, element_coords, element_type, element_area);

    // Local coordinates of element vertices (standard vertex mapping)
    // Triangle: N0 at (0,0), N1 at (1,0), N2 at (0,1)
    // Quad: bilinear on [-1,1]²
    let (_csi_nodes, _eta_nodes): (Vec<f64>, Vec<f64>) = match element_type {
        ElementType::Tri3 => (vec![0.0, 1.0, 0.0], vec![0.0, 0.0, 1.0]),
        ElementType::Quad4 => (vec![1.0, -1.0, -1.0, 1.0], vec![1.0, 1.0, -1.0, -1.0]),
    };

    // Process each subelement
    for subel in &subelements {
        let xice = subel.xi_center;
        let etce = subel.eta_center;
        let fase = subel.factor;
        let fase2 = fase * fase;
        let iforie = (fase.abs() - 1.0).abs() < 1e-10; // Not subdivided

        // Get quadrature points for this subelement
        let quad_points = get_quadrature_points(element_type, subel.gauss_order);

        for (csi_gau, eta_gau, wei_gau) in quad_points {
            // Transform quadrature point to original element coordinates
            let (xio, eto, weih2) = if iforie {
                (csi_gau, eta_gau, wei_gau)
            } else if let Some(tri_verts) = &subel.tri_vertices {
                // For triangular subelements: use affine transformation with shape functions
                // Reference triangle point (ξ, η) maps to:
                // xio = v0.0 * (1-ξ-η) + v1.0 * ξ + v2.0 * η
                // eto = v0.1 * (1-ξ-η) + v1.1 * ξ + v2.1 * η
                let l0 = 1.0 - csi_gau - eta_gau; // barycentric coord for v0
                let xio = tri_verts[0].0 * l0 + tri_verts[1].0 * csi_gau + tri_verts[2].0 * eta_gau;
                let eto = tri_verts[0].1 * l0 + tri_verts[1].1 * csi_gau + tri_verts[2].1 * eta_gau;

                // Jacobian of the affine map = 2 * area of subtriangle
                // Area = 0.5 * |det([v1-v0, v2-v0])|
                let dx1 = tri_verts[1].0 - tri_verts[0].0;
                let dy1 = tri_verts[1].1 - tri_verts[0].1;
                let dx2 = tri_verts[2].0 - tri_verts[0].0;
                let dy2 = tri_verts[2].1 - tri_verts[0].1;
                let det = (dx1 * dy2 - dx2 * dy1).abs();
                // det = 2 * area of subtriangle in local coords
                // The reference triangle has area 0.5, so Jacobian = det
                let weih2 = wei_gau * det;

                (xio, eto, weih2)
            } else {
                // For quad subelements: use center + scale transformation
                (
                    xice + csi_gau * fase,
                    etce + eta_gau * fase,
                    wei_gau * fase2,
                )
            };

            // Compute shape functions, Jacobian and position at quadrature point
            let (shape_fn, jacobian, el_norm, crd_poi) =
                compute_parameters(element_coords, element_type, xio, eto);

            let wga = weih2 * jacobian;

            // Compute distance vector and kernels
            let mut diff_fsp = Array1::zeros(3);
            for i in 0..3 {
                diff_fsp[i] = crd_poi[i] - source_point[i];
            }

            let (unit_r, dis_fsp) = normalize(&diff_fsp);

            if dis_fsp < 1e-15 {
                continue;
            }

            // G kernel: exp(ikr)/(4πr)
            let re1 = wavruim * dis_fsp;
            let re2 = wga / (4.0 * PI * dis_fsp);
            let zg = Complex64::new(re1.cos() * re2, re1.sin() * re2);

            // H kernel (double layer): ∂G/∂n_y
            let z1 = Complex64::new(-1.0 / dis_fsp, wavruim);
            let zhh_base = zg * z1;
            let re1_h = unit_r.dot(&el_norm); // (y-x)·n_y / r
            let zhh = zhh_base * re1_h;

            // H^T kernel (adjoint double layer): ∂G/∂n_x
            let re2_h = -unit_r.dot(source_normal); // -(y-x)·n_x / r
            let zht = zhh_base * re2_h;

            // E kernel (hypersingular): ∂²G/(∂n_x ∂n_y)
            let rq = re1_h * re2_h;
            let nx_dot_ny = source_normal.dot(&el_norm);
            let dq = dis_fsp * dis_fsp;

            let ze_factor = Complex64::new(
                (3.0 / dq - k2) * rq + nx_dot_ny / dq,
                -wavruim / dis_fsp * (3.0 * rq + nx_dot_ny),
            );
            let ze = zg * ze_factor;

            // Accumulate integrals
            result.g_integral += zg;
            result.dg_dn_integral += zhh;
            result.dg_dnx_integral += zht;
            result.d2g_dnxdny_integral += ze;

            // RHS contribution
            if compute_rhs && let Some(bc) = bc_values {
                // Interpolate BC at quadrature point
                let mut zbgao = Complex64::new(0.0, 0.0);
                for (i, &n) in shape_fn.iter().enumerate() {
                    if i < bc.len() {
                        zbgao += bc[i] * n;
                    }
                }

                let gamma = physics.gamma();
                let tau = physics.tau;
                let beta = physics.burton_miller_beta();

                if bc_type == 0 {
                    // Velocity BC
                    result.rhs_contribution += (zg * gamma * tau + zht * beta) * zbgao;
                } else if bc_type == 1 {
                    // Pressure BC
                    result.rhs_contribution -= (zhh * gamma * tau + ze * beta) * zbgao;
                }
            }
        }
    }

    result
}

/// Get quadrature points for element type and order
fn get_quadrature_points(element_type: ElementType, order: usize) -> Vec<(f64, f64, f64)> {
    match element_type {
        ElementType::Tri3 => triangle_quadrature(order),
        ElementType::Quad4 => quad_quadrature(order),
    }
}

/// Compute shape functions, Jacobian, normal and position at local coordinates
fn compute_parameters(
    element_coords: &Array2<f64>,
    element_type: ElementType,
    s: f64,
    t: f64,
) -> (Vec<f64>, f64, Array1<f64>, Array1<f64>) {
    let num_nodes = element_type.num_nodes();

    let (shape_fn, shape_ds, shape_dt) = match element_type {
        ElementType::Tri3 => {
            // Triangle area coordinates: standard vertex mapping
            // N0 = 1-s-t (vertex at (0,0))
            // N1 = s (vertex at (1,0))
            // N2 = t (vertex at (0,1))
            let shape_fn = vec![1.0 - s - t, s, t];
            let shape_ds = vec![-1.0, 1.0, 0.0];
            let shape_dt = vec![-1.0, 0.0, 1.0];
            (shape_fn, shape_ds, shape_dt)
        }
        ElementType::Quad4 => {
            // Bilinear quad on [-1,1]²
            let s1 = 0.25 * (s + 1.0);
            let s2 = 0.25 * (s - 1.0);
            let t1 = t + 1.0;
            let t2 = t - 1.0;

            let shape_fn = vec![s1 * t1, -s2 * t1, s2 * t2, -s1 * t2];
            let shape_ds = vec![
                0.25 * (t + 1.0),
                -0.25 * (t + 1.0),
                0.25 * (t - 1.0),
                -0.25 * (t - 1.0),
            ];
            let shape_dt = vec![
                0.25 * (s + 1.0),
                0.25 * (1.0 - s),
                0.25 * (s - 1.0),
                -0.25 * (s + 1.0),
            ];
            (shape_fn, shape_ds, shape_dt)
        }
    };

    // Compute global position and tangent vectors
    let mut crd_poi = Array1::zeros(3);
    let mut dx_ds = Array1::zeros(3);
    let mut dx_dt = Array1::zeros(3);

    for i in 0..num_nodes {
        for j in 0..3 {
            crd_poi[j] += shape_fn[i] * element_coords[[i, j]];
            dx_ds[j] += shape_ds[i] * element_coords[[i, j]];
            dx_dt[j] += shape_dt[i] * element_coords[[i, j]];
        }
    }

    // Normal = dx_ds × dx_dt
    let normal = cross_product(&dx_ds, &dx_dt);
    let jacobian = normal.dot(&normal).sqrt();

    let el_norm = if jacobian > 1e-15 {
        normal / jacobian
    } else {
        Array1::zeros(3)
    };

    (shape_fn, jacobian, el_norm, crd_poi)
}

/// Threshold for quasi-singular integration (distance/element_size ratio)
/// NumCalc uses approximately 3.0
pub const QUASI_SINGULAR_THRESHOLD: f64 = 3.0;

/// High-accuracy threshold requiring 13-point quadrature
pub const HIGH_ACCURACY_THRESHOLD: f64 = 2.0;

/// Determine the optimal quadrature order based on distance to element
///
/// Returns the recommended Gauss order for triangle quadrature.
/// - ratio < 2.0: 13-point (very near-singular)
/// - ratio < 3.0: 7-point (quasi-singular)
/// - ratio >= 3.0: 4-point (regular)
pub fn optimal_quadrature_order(distance: f64, element_size: f64) -> usize {
    let ratio = distance / element_size;
    if ratio < HIGH_ACCURACY_THRESHOLD {
        4 // Maps to 13-point in triangle_quadrature
    } else if ratio < QUASI_SINGULAR_THRESHOLD {
        3 // Maps to 7-point in triangle_quadrature
    } else {
        2 // Maps to 4-point in triangle_quadrature
    }
}

/// Perform quasi-singular integration with adaptive quadrature order
///
/// This function determines the appropriate quadrature order based on the
/// distance between source point and element, using higher-order rules
/// for near-singular cases.
///
/// # Arguments
/// * `source_point` - Source point (collocation point)
/// * `source_normal` - Unit normal at source point
/// * `element_coords` - Node coordinates of the field element (num_nodes × 3)
/// * `element_type` - Triangle or quad element
/// * `element_center` - Center of the element
/// * `element_area` - Area of the field element
/// * `physics` - Physics parameters (wave number, etc.)
///
/// # Returns
/// IntegrationResult with G, H, H^T, E integrals
pub fn quasi_singular_integration(
    source_point: &Array1<f64>,
    source_normal: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    element_center: &Array1<f64>,
    element_area: f64,
    physics: &PhysicsParams,
) -> IntegrationResult {
    // Compute distance from source to element center
    let mut dist_sq = 0.0;
    for i in 0..3 {
        let d = element_center[i] - source_point[i];
        dist_sq += d * d;
    }
    let distance = dist_sq.sqrt();
    let element_size = element_area.sqrt();
    let ratio = distance / element_size;

    // Choose integration strategy based on distance ratio
    if ratio < HIGH_ACCURACY_THRESHOLD {
        // Very close: use adaptive subdivision with high-order quadrature
        regular_integration(
            source_point,
            source_normal,
            element_coords,
            element_type,
            element_area,
            physics,
            None,
            0,
            false,
        )
    } else if ratio < QUASI_SINGULAR_THRESHOLD {
        // Quasi-singular: use high-order quadrature without subdivision
        let order = optimal_quadrature_order(distance, element_size);
        regular_integration_fixed_order(
            source_point,
            source_normal,
            element_coords,
            element_type,
            physics,
            order,
        )
    } else {
        // Regular far-field: use standard 4-point quadrature
        regular_integration_fixed_order(
            source_point,
            source_normal,
            element_coords,
            element_type,
            physics,
            2, // 4-point quadrature
        )
    }
}

/// Compute element integration with fixed quadrature order (no adaptation)
///
/// This is a simpler version for far-field elements where adaptation is not needed.
pub fn regular_integration_fixed_order(
    source_point: &Array1<f64>,
    source_normal: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    physics: &PhysicsParams,
    gauss_order: usize,
) -> IntegrationResult {
    let wavruim = physics.harmonic_factor * physics.wave_number;
    let k2 = physics.wave_number * physics.wave_number;

    let mut result = IntegrationResult::default();

    // Get quadrature points
    let quad_points = get_quadrature_points(element_type, gauss_order);

    for (xi, eta, weight) in quad_points {
        // Compute shape functions, Jacobian and position
        let (_, jacobian, el_norm, crd_poi) =
            compute_parameters(element_coords, element_type, xi, eta);

        let wga = weight * jacobian;

        // Distance vector
        let mut diff_fsp = Array1::zeros(3);
        for i in 0..3 {
            diff_fsp[i] = crd_poi[i] - source_point[i];
        }

        let (unit_r, dis_fsp) = normalize(&diff_fsp);

        if dis_fsp < 1e-15 {
            continue;
        }

        // G kernel
        let kr = wavruim * dis_fsp;
        let g_scaled = wga / (4.0 * PI * dis_fsp);
        let zg = Complex64::new(kr.cos() * g_scaled, kr.sin() * g_scaled);

        // H and H^T kernels
        let z1 = Complex64::new(-1.0 / dis_fsp, wavruim);
        let zhh_base = zg * z1;
        let r_dot_ny = unit_r.dot(&el_norm);
        let r_dot_nx = -unit_r.dot(source_normal);
        let zhh = zhh_base * r_dot_ny;
        let zht = zhh_base * r_dot_nx;

        // E kernel
        let rq = r_dot_ny * r_dot_nx;
        let nx_dot_ny = source_normal.dot(&el_norm);
        let dq = dis_fsp * dis_fsp;

        let ze_factor = Complex64::new(
            (3.0 / dq - k2) * rq + nx_dot_ny / dq,
            -wavruim / dis_fsp * (3.0 * rq + nx_dot_ny),
        );
        let ze = zg * ze_factor;

        // Accumulate
        result.g_integral += zg;
        result.dg_dn_integral += zhh;
        result.dg_dnx_integral += zht;
        result.d2g_dnxdny_integral += ze;
    }

    result
}

/// Compute G kernel only (single layer potential) for efficiency
///
/// Used when only G is needed (e.g., for FMM far-field approximation).
pub fn integrate_g_only(
    source_point: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    physics: &PhysicsParams,
    gauss_order: usize,
) -> Complex64 {
    let wavruim = physics.harmonic_factor * physics.wave_number;
    let mut result = Complex64::new(0.0, 0.0);

    let quad_points = get_quadrature_points(element_type, gauss_order);

    for (xi, eta, weight) in quad_points {
        let (_, jacobian, _, crd_poi) = compute_parameters(element_coords, element_type, xi, eta);

        let wga = weight * jacobian;

        let mut diff = Array1::zeros(3);
        for i in 0..3 {
            diff[i] = crd_poi[i] - source_point[i];
        }
        let r = diff.dot(&diff).sqrt();

        if r < 1e-15 {
            continue;
        }

        let kr = wavruim * r;
        let g_scaled = wga / (4.0 * PI * r);
        result += Complex64::new(kr.cos() * g_scaled, kr.sin() * g_scaled);
    }

    result
}

/// Compute H kernel only (double layer potential)
///
/// Used when only the double layer is needed.
pub fn integrate_h_only(
    source_point: &Array1<f64>,
    element_coords: &Array2<f64>,
    element_type: ElementType,
    physics: &PhysicsParams,
    gauss_order: usize,
) -> Complex64 {
    let wavruim = physics.harmonic_factor * physics.wave_number;
    let mut result = Complex64::new(0.0, 0.0);

    let quad_points = get_quadrature_points(element_type, gauss_order);

    for (xi, eta, weight) in quad_points {
        let (_, jacobian, el_norm, crd_poi) =
            compute_parameters(element_coords, element_type, xi, eta);

        let wga = weight * jacobian;

        let mut diff = Array1::zeros(3);
        for i in 0..3 {
            diff[i] = crd_poi[i] - source_point[i];
        }
        let r = diff.dot(&diff).sqrt();

        if r < 1e-15 {
            continue;
        }

        let kr = wavruim * r;
        let g_scaled = wga / (4.0 * PI * r);
        let zg = Complex64::new(kr.cos() * g_scaled, kr.sin() * g_scaled);

        let z1 = Complex64::new(-1.0 / r, wavruim);
        let r_dot_n: f64 = diff
            .iter()
            .zip(el_norm.iter())
            .map(|(d, n)| d * n)
            .sum::<f64>()
            / r;

        result += zg * z1 * r_dot_n;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array2, array};

    fn make_test_triangle() -> Array2<f64> {
        Array2::from_shape_vec((3, 3), vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
    }

    fn make_test_quad() -> Array2<f64> {
        Array2::from_shape_vec(
            (4, 3),
            vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        )
        .unwrap()
    }

    #[test]
    fn test_regular_integration_far_field() {
        let coords = make_test_triangle();
        let source = array![10.0, 0.0, 0.0]; // Far from element
        let normal = array![0.0, 0.0, 1.0];

        let physics = PhysicsParams::new(1000.0, 343.0, 1.21, false);

        let result = regular_integration(
            &source,
            &normal,
            &coords,
            ElementType::Tri3,
            0.5,
            &physics,
            None,
            0,
            false,
        );

        // G integral should be finite and small (far field)
        assert!(result.g_integral.norm().is_finite());
        assert!(result.g_integral.norm() < 0.1);
    }

    #[test]
    fn test_regular_integration_fixed_order() {
        let coords = make_test_triangle();
        let source = array![5.0, 5.0, 0.0];
        let normal = array![0.0, 0.0, 1.0];

        let physics = PhysicsParams::new(1000.0, 343.0, 1.21, false);

        let result = regular_integration_fixed_order(
            &source,
            &normal,
            &coords,
            ElementType::Tri3,
            &physics,
            4,
        );

        assert!(result.g_integral.norm().is_finite());
    }

    #[test]
    fn test_integrate_g_only() {
        let coords = make_test_triangle();
        let source = array![2.0, 2.0, 1.0];

        let physics = PhysicsParams::new(1000.0, 343.0, 1.21, false);

        let g = integrate_g_only(&source, &coords, ElementType::Tri3, &physics, 4);

        assert!(g.norm().is_finite());
        assert!(g.norm() > 0.0);
    }

    #[test]
    fn test_quad_element_integration() {
        let coords = make_test_quad();
        let source = array![5.0, 0.5, 0.0];
        let normal = array![0.0, 0.0, 1.0];

        let physics = PhysicsParams::new(1000.0, 343.0, 1.21, false);

        let result = regular_integration_fixed_order(
            &source,
            &normal,
            &coords,
            ElementType::Quad4,
            &physics,
            4,
        );

        assert!(result.g_integral.norm().is_finite());
    }

    #[test]
    fn test_integration_symmetry() {
        // For a symmetric setup, H^T at point A should equal H at symmetric point
        let coords = make_test_triangle();
        let source1 = array![0.5, 0.5, 1.0];
        let normal1 = array![0.0, 0.0, 1.0];
        let source2 = array![0.5, 0.5, -1.0];
        let normal2 = array![0.0, 0.0, -1.0];

        let physics = PhysicsParams::new(1000.0, 343.0, 1.21, false);

        let result1 = regular_integration_fixed_order(
            &source1,
            &normal1,
            &coords,
            ElementType::Tri3,
            &physics,
            4,
        );

        let result2 = regular_integration_fixed_order(
            &source2,
            &normal2,
            &coords,
            ElementType::Tri3,
            &physics,
            4,
        );

        // G should be equal (symmetric sources)
        assert!((result1.g_integral.norm() - result2.g_integral.norm()).abs() < 1e-10);
    }

    #[test]
    fn test_compute_parameters_triangle() {
        let coords = make_test_triangle();

        let (shape, jac, normal, pos) = compute_parameters(&coords, ElementType::Tri3, 0.5, 0.25);

        // Shape functions should sum to 1
        let sum: f64 = shape.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // Jacobian should be 2 * area = 1
        assert!((jac - 1.0).abs() < 1e-10);

        // Normal should be (0, 0, 1)
        assert!(normal[2].abs() > 0.99);

        // Position check
        assert!((pos[0] - 0.5).abs() < 1e-10);
        assert!((pos[1] - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_compute_parameters_quad() {
        let coords = make_test_quad();

        let (shape, _jac, normal, _pos) = compute_parameters(&coords, ElementType::Quad4, 0.0, 0.0);

        // Shape functions should sum to 1
        let sum: f64 = shape.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // Normal should be (0, 0, 1)
        assert!(normal[2].abs() > 0.99);
    }
}
