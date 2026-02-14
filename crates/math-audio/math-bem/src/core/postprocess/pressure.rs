//! Field pressure evaluation at exterior points
//!
//! Computes the acoustic pressure field at evaluation points using
//! the BEM representation formula:
//!
//! p(x) = p_inc(x) + ∫_Γ [p(y) ∂G/∂n_y - (∂p/∂n)(y) G(x,y)] dS_y
//!
//! For a rigid scatterer (Neumann BC), the normal velocity is zero,
//! so the formula simplifies to:
//!
//! p(x) = p_inc(x) + ∫_Γ p(y) ∂G/∂n_y dS_y

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use std::f64::consts::PI;

// Note: Using inline Green's function computation for performance
use crate::core::incident::IncidentField;
use crate::core::integration::gauss::triangle_quadrature;
use crate::core::types::{Element, ElementType, PhysicsParams};

/// Field evaluation result at a single point
#[derive(Debug, Clone)]
pub struct FieldPoint {
    /// Position coordinates [x, y, z]
    pub position: [f64; 3],
    /// Incident pressure
    pub p_incident: Complex64,
    /// Scattered pressure
    pub p_scattered: Complex64,
    /// Total pressure (incident + scattered)
    pub p_total: Complex64,
}

impl FieldPoint {
    /// Create new field point
    pub fn new(position: [f64; 3], p_incident: Complex64, p_scattered: Complex64) -> Self {
        Self {
            position,
            p_incident,
            p_scattered,
            p_total: p_incident + p_scattered,
        }
    }

    /// Get pressure magnitude in dB SPL (re: 20 μPa)
    pub fn spl_db(&self) -> f64 {
        let p_ref = 20e-6; // Reference pressure for dB SPL
        20.0 * (self.p_total.norm() / p_ref).log10()
    }

    /// Get pressure magnitude (absolute value)
    pub fn magnitude(&self) -> f64 {
        self.p_total.norm()
    }

    /// Get pressure phase in radians
    pub fn phase(&self) -> f64 {
        self.p_total.arg()
    }
}

/// Compute scattered field at evaluation points using surface solution
///
/// Uses the Kirchhoff-Helmholtz integral equation for exterior problems:
/// p_scat(x) = ∫_Γ [p(y) ∂G/∂n_y(x,y) - v_n(y) G(x,y)] dS_y
///
/// This implementation uses proper Gauss quadrature over each element,
/// matching the C++ NumCalc NC_IntegrationTBEM function for accuracy.
///
/// # Arguments
/// * `eval_points` - Evaluation points (N × 3 array)
/// * `elements` - Mesh elements with surface solution
/// * `nodes` - Node coordinates
/// * `surface_pressure` - Solved surface pressure values (one per element)
/// * `surface_velocity` - Surface normal velocity (usually from BC, can be zero for rigid)
/// * `physics` - Physical parameters
///
/// # Returns
/// Scattered pressure at each evaluation point
pub fn compute_scattered_field(
    eval_points: &Array2<f64>,
    elements: &[Element],
    nodes: &Array2<f64>,
    surface_pressure: &Array1<Complex64>,
    surface_velocity: Option<&Array1<Complex64>>,
    physics: &PhysicsParams,
) -> Array1<Complex64> {
    let n_eval = eval_points.nrows();
    let k = physics.wave_number;
    let harmonic_factor = physics.harmonic_factor;
    let wavruim = k * harmonic_factor;

    let mut p_scattered = Array1::zeros(n_eval);

    // Get boundary elements (exclude evaluation-only elements)
    let boundary_elements: Vec<_> = elements
        .iter()
        .filter(|e| !e.property.is_evaluation())
        .collect();

    for i in 0..n_eval {
        let x = Array1::from_vec(vec![
            eval_points[[i, 0]],
            eval_points[[i, 1]],
            eval_points[[i, 2]],
        ]);

        let mut p_scat = Complex64::new(0.0, 0.0);

        for (j, elem) in boundary_elements.iter().enumerate() {
            // Get surface values (constant over element)
            let p_surf = surface_pressure[j];
            let v_surf = surface_velocity.map_or(Complex64::new(0.0, 0.0), |v| v[j]);

            // Get element node coordinates
            let elem_coords = get_element_coords(elem, nodes);

            // Integrate using Gauss quadrature (matching NumCalc approach)
            let contribution = integrate_element_field(
                &x,
                &elem_coords,
                elem.element_type,
                p_surf,
                v_surf,
                k,
                wavruim,
            );

            p_scat += contribution;
        }

        p_scattered[i] = p_scat;
    }

    p_scattered
}

/// Get element node coordinates from connectivity
fn get_element_coords(elem: &Element, nodes: &Array2<f64>) -> Array2<f64> {
    let n_nodes = elem.connectivity.len();
    let mut coords = Array2::zeros((n_nodes, 3));
    for (i, &node_idx) in elem.connectivity.iter().enumerate() {
        for j in 0..3 {
            coords[[i, j]] = nodes[[node_idx, j]];
        }
    }
    coords
}

/// Integrate element contribution using Gauss quadrature
///
/// This matches the NC_IntegrationTBEM function from NumCalc.
fn integrate_element_field(
    x: &Array1<f64>,
    elem_coords: &Array2<f64>,
    elem_type: ElementType,
    p_surf: Complex64,
    v_surf: Complex64,
    _k: f64,
    wavruim: f64,
) -> Complex64 {
    let mut result = Complex64::new(0.0, 0.0);

    // Use order 3 quadrature (7-point for triangles) for good accuracy
    let gauss_order = 3;
    let quad_points = match elem_type {
        ElementType::Tri3 => triangle_quadrature(gauss_order),
        ElementType::Quad4 => {
            // For quads, use tensor product rule (approximate)
            triangle_quadrature(gauss_order)
        }
    };

    for (xi, eta, weight) in quad_points {
        // Compute shape functions and derivatives
        let (shape_fn, shape_ds, shape_dt) = match elem_type {
            ElementType::Tri3 => {
                let shape_fn = vec![1.0 - xi - eta, xi, eta];
                let shape_ds = vec![-1.0, 1.0, 0.0];
                let shape_dt = vec![-1.0, 0.0, 1.0];
                (shape_fn, shape_ds, shape_dt)
            }
            ElementType::Quad4 => {
                // Use triangle approximation for quads
                let shape_fn = vec![1.0 - xi - eta, xi, eta];
                let shape_ds = vec![-1.0, 1.0, 0.0];
                let shape_dt = vec![-1.0, 0.0, 1.0];
                (shape_fn, shape_ds, shape_dt)
            }
        };

        // Compute position at quadrature point
        let mut crd_poi: Array1<f64> = Array1::zeros(3);
        let mut dx_ds: Array1<f64> = Array1::zeros(3);
        let mut dx_dt: Array1<f64> = Array1::zeros(3);

        let n_nodes = elem_coords.nrows().min(3); // Limit to 3 for tri3
        for n in 0..n_nodes {
            for d in 0..3 {
                crd_poi[d] += shape_fn[n] * elem_coords[[n, d]];
                dx_ds[d] += shape_ds[n] * elem_coords[[n, d]];
                dx_dt[d] += shape_dt[n] * elem_coords[[n, d]];
            }
        }

        // Compute normal and Jacobian
        let normal: Array1<f64> = Array1::from_vec(vec![
            dx_ds[1] * dx_dt[2] - dx_ds[2] * dx_dt[1],
            dx_ds[2] * dx_dt[0] - dx_ds[0] * dx_dt[2],
            dx_ds[0] * dx_dt[1] - dx_ds[1] * dx_dt[0],
        ]);
        let jacobian = normal.dot(&normal).sqrt();

        if jacobian < 1e-15 {
            continue;
        }

        let el_norm = &normal / jacobian;

        // Distance from evaluation point to quadrature point
        let mut r_vec: Array1<f64> = Array1::zeros(3);
        for d in 0..3 {
            r_vec[d] = crd_poi[d] - x[d];
        }
        let r = r_vec.dot(&r_vec).sqrt();

        if r < 1e-15 {
            continue;
        }

        // Weight factor
        let vjacwe = jacobian * weight;

        // Green's function: G = exp(ikr) / (4πr)
        let kr = wavruim * r;
        let re1 = 4.0 * PI * r;
        let zgrfu = Complex64::new(kr.cos() / re1, kr.sin() / re1);

        // Derivative factor: z1 = -1/r + ik
        let z1 = Complex64::new(-1.0 / r, wavruim);
        let zgikr = zgrfu * z1;

        // ∂r/∂n_y = (y-x)·n_y / r
        let drdn_y = r_vec.dot(&el_norm) / r;

        // Double layer contribution: p * ∂G/∂n_y * dS
        let zdgrdn = zgikr * drdn_y;
        result += p_surf * zdgrdn * vjacwe;

        // Single layer contribution: -v * G * dS
        // (v_surf is normally zero for rigid scatterer)
        if v_surf.norm() > 1e-15 {
            result -= v_surf * zgrfu * vjacwe;
        }
    }

    result
}

/// Compute total field (incident + scattered) at evaluation points
///
/// # Arguments
/// * `eval_points` - Evaluation points (N × 3 array)
/// * `elements` - Mesh elements
/// * `nodes` - Node coordinates
/// * `surface_pressure` - Solved surface pressure
/// * `incident_field` - Incident field specification
/// * `physics` - Physical parameters
///
/// # Returns
/// Vector of FieldPoint with all pressure components
pub fn compute_total_field(
    eval_points: &Array2<f64>,
    elements: &[Element],
    nodes: &Array2<f64>,
    surface_pressure: &Array1<Complex64>,
    surface_velocity: Option<&Array1<Complex64>>,
    incident_field: &IncidentField,
    physics: &PhysicsParams,
) -> Vec<FieldPoint> {
    // Compute incident field
    let p_incident = incident_field.evaluate_pressure(eval_points, physics);

    // Compute scattered field
    let p_scattered = compute_scattered_field(
        eval_points,
        elements,
        nodes,
        surface_pressure,
        surface_velocity,
        physics,
    );

    // Combine results
    let n_eval = eval_points.nrows();
    let mut results = Vec::with_capacity(n_eval);

    for i in 0..n_eval {
        let position = [
            eval_points[[i, 0]],
            eval_points[[i, 1]],
            eval_points[[i, 2]],
        ];
        results.push(FieldPoint::new(position, p_incident[i], p_scattered[i]));
    }

    results
}

/// Generate evaluation points on a sphere around the origin
///
/// # Arguments
/// * `radius` - Sphere radius
/// * `n_theta` - Number of polar divisions
/// * `n_phi` - Number of azimuthal divisions
///
/// # Returns
/// Array of points on the sphere (N × 3)
pub fn generate_sphere_eval_points(radius: f64, n_theta: usize, n_phi: usize) -> Array2<f64> {
    let mut points = Vec::new();

    for i in 0..n_theta {
        let theta = PI * (i as f64 + 0.5) / n_theta as f64;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for j in 0..n_phi {
            let phi = 2.0 * PI * j as f64 / n_phi as f64;

            points.push(radius * sin_theta * phi.cos());
            points.push(radius * sin_theta * phi.sin());
            points.push(radius * cos_theta);
        }
    }

    let n_points = n_theta * n_phi;
    Array2::from_shape_vec((n_points, 3), points).unwrap()
}

/// Generate evaluation points along a line
///
/// # Arguments
/// * `start` - Start point
/// * `end` - End point
/// * `n_points` - Number of points
///
/// # Returns
/// Array of points along the line (N × 3)
pub fn generate_line_eval_points(start: [f64; 3], end: [f64; 3], n_points: usize) -> Array2<f64> {
    let mut points = Vec::new();

    for i in 0..n_points {
        let t = i as f64 / (n_points - 1).max(1) as f64;
        points.push(start[0] + t * (end[0] - start[0]));
        points.push(start[1] + t * (end[1] - start[1]));
        points.push(start[2] + t * (end[2] - start[2]));
    }

    Array2::from_shape_vec((n_points, 3), points).unwrap()
}

/// Generate evaluation points on a plane (for field maps)
///
/// # Arguments
/// * `center` - Center of the plane
/// * `normal` - Normal to the plane (determines orientation)
/// * `extent` - Half-size of the plane in each direction
/// * `n_points` - Number of points in each direction
///
/// # Returns
/// Array of points on the plane (N² × 3)
pub fn generate_plane_eval_points(
    center: [f64; 3],
    normal: [f64; 3],
    extent: f64,
    n_points: usize,
) -> Array2<f64> {
    // Find two vectors perpendicular to normal
    let n = Array1::from_vec(vec![normal[0], normal[1], normal[2]]);
    let n_norm = n.dot(&n).sqrt();
    let n = &n / n_norm;

    // Choose an arbitrary vector not parallel to n
    let arbitrary = if n[0].abs() < 0.9 {
        Array1::from_vec(vec![1.0, 0.0, 0.0])
    } else {
        Array1::from_vec(vec![0.0, 1.0, 0.0])
    };

    // First basis vector (perpendicular to n)
    let u = {
        let cross = Array1::from_vec(vec![
            n[1] * arbitrary[2] - n[2] * arbitrary[1],
            n[2] * arbitrary[0] - n[0] * arbitrary[2],
            n[0] * arbitrary[1] - n[1] * arbitrary[0],
        ]);
        let norm = cross.dot(&cross).sqrt();
        cross / norm
    };

    // Second basis vector (perpendicular to both n and u)
    let v = Array1::from_vec(vec![
        n[1] * u[2] - n[2] * u[1],
        n[2] * u[0] - n[0] * u[2],
        n[0] * u[1] - n[1] * u[0],
    ]);

    let mut points = Vec::new();

    for i in 0..n_points {
        let s = -extent + 2.0 * extent * i as f64 / (n_points - 1).max(1) as f64;
        for j in 0..n_points {
            let t = -extent + 2.0 * extent * j as f64 / (n_points - 1).max(1) as f64;

            points.push(center[0] + s * u[0] + t * v[0]);
            points.push(center[1] + s * u[1] + t * v[1]);
            points.push(center[2] + s * u[2] + t * v[2]);
        }
    }

    Array2::from_shape_vec((n_points * n_points, 3), points).unwrap()
}

/// Compute RCS (Radar Cross Section) from far-field pattern
///
/// For acoustic scattering, the RCS is defined as:
/// σ = lim_{r→∞} 4πr² |p_scat/p_inc|²
///
/// # Arguments
/// * `surface_pressure` - Solved surface pressure
/// * `elements` - Mesh elements
/// * `direction` - Far-field direction (unit vector)
/// * `physics` - Physical parameters
///
/// # Returns
/// RCS value
pub fn compute_rcs(
    surface_pressure: &Array1<Complex64>,
    elements: &[Element],
    direction: [f64; 3],
    physics: &PhysicsParams,
) -> f64 {
    let k = physics.wave_number;

    // Far-field pattern is computed using the stationary phase approximation
    // For constant elements: F(θ,φ) = Σ p_j * exp(-ik r_j · d) * A_j * (n_j · d)

    let mut far_field = Complex64::new(0.0, 0.0);

    let boundary_elements: Vec<_> = elements
        .iter()
        .filter(|e| !e.property.is_evaluation())
        .collect();

    let d = Array1::from_vec(vec![direction[0], direction[1], direction[2]]);

    for (j, elem) in boundary_elements.iter().enumerate() {
        let center = &elem.center;
        let normal = &elem.normal;
        let area = elem.area;

        // Phase term: exp(-ik r · d)
        let phase = -k * center.dot(&d);
        let exp_phase = Complex64::new(phase.cos(), phase.sin());

        // Normal factor: n · d (double layer contribution)
        let n_dot_d = normal.dot(&d);

        // Contribution
        let ik = Complex64::new(0.0, k);
        far_field += surface_pressure[j] * exp_phase * area * ik * n_dot_d;
    }

    // RCS = |F|² / |p_inc|²
    // For unit amplitude incident wave, RCS = |F|²
    4.0 * PI * far_field.norm_sqr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::ElementProperty;
    use ndarray::array;

    fn make_physics(k: f64) -> PhysicsParams {
        let c = 343.0;
        let freq = k * c / (2.0 * PI);
        PhysicsParams::new(freq, c, 1.21, false)
    }

    #[test]
    fn test_field_point_creation() {
        let p_inc = Complex64::new(1.0, 0.0);
        let p_scat = Complex64::new(0.5, 0.3);

        let fp = FieldPoint::new([1.0, 0.0, 0.0], p_inc, p_scat);

        assert!((fp.p_total - (p_inc + p_scat)).norm() < 1e-10);
        assert!(fp.magnitude() > 0.0);
    }

    #[test]
    fn test_generate_sphere_eval_points() {
        let points = generate_sphere_eval_points(2.0, 10, 20);

        assert_eq!(points.nrows(), 200);
        assert_eq!(points.ncols(), 3);

        // Check all points are at radius 2.0
        for i in 0..points.nrows() {
            let r =
                (points[[i, 0]].powi(2) + points[[i, 1]].powi(2) + points[[i, 2]].powi(2)).sqrt();
            assert!((r - 2.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_generate_line_eval_points() {
        let points = generate_line_eval_points([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 11);

        assert_eq!(points.nrows(), 11);

        // First point should be at origin
        assert!(points[[0, 0]].abs() < 1e-10);

        // Last point should be at (1, 0, 0)
        assert!((points[[10, 0]] - 1.0).abs() < 1e-10);

        // Point spacing should be uniform
        let spacing = points[[1, 0]] - points[[0, 0]];
        assert!((spacing - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_generate_plane_eval_points() {
        let points = generate_plane_eval_points([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0, 5);

        assert_eq!(points.nrows(), 25);
        assert_eq!(points.ncols(), 3);

        // All points should have z = 0 (on the xy plane)
        for i in 0..points.nrows() {
            assert!(points[[i, 2]].abs() < 1e-10);
        }
    }

    #[test]
    fn test_scattered_field_basic() {
        // Simple test with a single element
        use crate::core::types::BoundaryCondition;

        let physics = make_physics(1.0);

        // Single triangular element
        let nodes =
            Array2::from_shape_vec((3, 3), vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 1.0, 0.0])
                .unwrap();

        let elem = Element {
            connectivity: vec![0, 1, 2],
            element_type: crate::core::types::ElementType::Tri3,
            property: ElementProperty::Surface,
            normal: array![0.0, 0.0, 1.0],
            node_normals: Array2::zeros((3, 3)),
            center: array![0.5, 1.0 / 3.0, 0.0],
            area: 0.5,
            boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]),
            group: 0,
            dof_addresses: vec![0],
        };

        let elements = vec![elem];
        let surface_pressure = Array1::from_vec(vec![Complex64::new(1.0, 0.0)]);

        // Evaluation point above the element
        let eval_points = Array2::from_shape_vec((1, 3), vec![0.5, 0.5, 1.0]).unwrap();

        let p_scat = compute_scattered_field(
            &eval_points,
            &elements,
            &nodes,
            &surface_pressure,
            None,
            &physics,
        );

        // Scattered field should be non-zero
        assert!(p_scat[0].norm() > 0.0);
    }
}
