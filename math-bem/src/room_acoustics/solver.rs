//! BEM solver for room acoustics
//!
//! Solves the Helmholtz equation in the room interior with rigid boundary conditions.
//!
//! This module supports three build modes:
//! - `native`: Uses native rayon for parallel processing (fastest)
//! - `wasm`: Uses wasm-bindgen-rayon for Web Worker parallelism
//! - Neither: Falls back to sequential processing

use super::*;
use crate::core::parallel::{parallel_map, parallel_map_indexed};
use ndarray::{Array1, Array2};
use num_complex::Complex64;
use std::f64::consts::PI;

/// Green's function for 3D Helmholtz equation
/// G(r) = exp(ikr) / (4πr)
fn greens_function_3d(r: f64, k: f64) -> Complex64 {
    if r < 1e-10 {
        return Complex64::new(0.0, 0.0);
    }
    let ikr = Complex64::new(0.0, k * r);
    ikr.exp() / (4.0 * PI * r)
}

/// Derivative of Green's function in normal direction
/// ∂G/∂n = (ikr - 1) * exp(ikr) / (4πr²) * cos(angle)
fn greens_function_derivative(r: f64, k: f64, cos_angle: f64) -> Complex64 {
    if r < 1e-10 {
        return Complex64::new(0.0, 0.0);
    }
    let ikr = Complex64::new(0.0, k * r);
    let factor = (ikr - 1.0) * ikr.exp() / (4.0 * PI * r * r);
    factor * cos_angle
}

/// Calculate element center and normal vector
fn element_center_and_normal(nodes: &[Point3D]) -> (Point3D, Point3D) {
    // Assume quadrilateral element
    let center = Point3D::new(
        nodes.iter().map(|n| n.x).sum::<f64>() / nodes.len() as f64,
        nodes.iter().map(|n| n.y).sum::<f64>() / nodes.len() as f64,
        nodes.iter().map(|n| n.z).sum::<f64>() / nodes.len() as f64,
    );

    // Normal from cross product of edges (works for both triangles and quads)
    let v1 = Point3D::new(
        nodes[1].x - nodes[0].x,
        nodes[1].y - nodes[0].y,
        nodes[1].z - nodes[0].z,
    );
    let v2 = Point3D::new(
        nodes[2].x - nodes[0].x,
        nodes[2].y - nodes[0].y,
        nodes[2].z - nodes[0].z,
    );

    // Cross product
    let nx = v1.y * v2.z - v1.z * v2.y;
    let ny = v1.z * v2.x - v1.x * v2.z;
    let nz = v1.x * v2.y - v1.y * v2.x;

    let norm = (nx * nx + ny * ny + nz * nz).sqrt();
    let normal = Point3D::new(nx / norm, ny / norm, nz / norm);

    (center, normal)
}

/// Calculate element area (supports triangles and quads)
fn element_area(nodes: &[Point3D]) -> f64 {
    if nodes.len() == 3 {
        // Triangle: 0.5 * |v1 × v2|
        let v1 = Point3D::new(
            nodes[1].x - nodes[0].x,
            nodes[1].y - nodes[0].y,
            nodes[1].z - nodes[0].z,
        );
        let v2 = Point3D::new(
            nodes[2].x - nodes[0].x,
            nodes[2].y - nodes[0].y,
            nodes[2].z - nodes[0].z,
        );

        let cross_x = v1.y * v2.z - v1.z * v2.y;
        let cross_y = v1.z * v2.x - v1.x * v2.z;
        let cross_z = v1.x * v2.y - v1.y * v2.x;

        0.5 * (cross_x * cross_x + cross_y * cross_y + cross_z * cross_z).sqrt()
    } else if nodes.len() == 4 {
        // Quadrilateral: split into two triangles and sum areas
        let v1 = Point3D::new(
            nodes[1].x - nodes[0].x,
            nodes[1].y - nodes[0].y,
            nodes[1].z - nodes[0].z,
        );
        let v2 = Point3D::new(
            nodes[2].x - nodes[0].x,
            nodes[2].y - nodes[0].y,
            nodes[2].z - nodes[0].z,
        );

        let cross1_x = v1.y * v2.z - v1.z * v2.y;
        let cross1_y = v1.z * v2.x - v1.x * v2.z;
        let cross1_z = v1.x * v2.y - v1.y * v2.x;
        let area1 = 0.5 * (cross1_x * cross1_x + cross1_y * cross1_y + cross1_z * cross1_z).sqrt();

        let v3 = Point3D::new(
            nodes[3].x - nodes[0].x,
            nodes[3].y - nodes[0].y,
            nodes[3].z - nodes[0].z,
        );

        let cross2_x = v2.y * v3.z - v2.z * v3.y;
        let cross2_y = v2.z * v3.x - v2.x * v3.z;
        let cross2_z = v2.x * v3.y - v2.y * v3.x;
        let area2 = 0.5 * (cross2_x * cross2_x + cross2_y * cross2_y + cross2_z * cross2_z).sqrt();

        area1 + area2
    } else {
        0.0
    }
}

/// Build BEM system matrix for rigid boundaries
pub fn build_bem_matrix(mesh: &RoomMesh, k: f64) -> Array2<Complex64> {
    let n = mesh.elements.len();
    let mut matrix = Array2::zeros((n, n));

    // Get element centers and normals
    let mut centers = Vec::new();
    let mut normals = Vec::new();
    let mut areas = Vec::new();

    for element in &mesh.elements {
        let nodes: Vec<Point3D> = element.nodes.iter().map(|&i| mesh.nodes[i]).collect();
        let (center, normal) = element_center_and_normal(&nodes);
        let area = element_area(&nodes);

        centers.push(center);
        normals.push(normal);
        areas.push(area);
    }

    // Fill matrix: rigid boundary condition ∂p/∂n = 0
    // This gives: Σ_j (∂G/∂n_i)(r_ij) * p_j * A_j = incident field derivative
    for i in 0..n {
        for j in 0..n {
            let r = centers[i].distance_to(&centers[j]);

            if i == j {
                // Diagonal: use approximation for self-interaction
                // For planar element: ∂G/∂n ≈ -ik/(2π) for small kr
                matrix[[i, j]] = Complex64::new(0.0, -k / (2.0 * PI)) * areas[j];
            } else {
                // Direction from j to i
                let dx = centers[i].x - centers[j].x;
                let dy = centers[i].y - centers[j].y;
                let dz = centers[i].z - centers[j].z;

                // Cosine of angle between (i-j) direction and normal at i
                let cos_angle = (dx * normals[i].x + dy * normals[i].y + dz * normals[i].z) / r;

                matrix[[i, j]] = greens_function_derivative(r, k, cos_angle) * areas[j];
            }
        }
    }

    matrix
}

/// Calculate incident field from sources at element centers
pub fn calculate_incident_field(
    mesh: &RoomMesh,
    sources: &[Source],
    k: f64,
    frequency: f64,
) -> Array1<Complex64> {
    let n = mesh.elements.len();
    let mut incident = Array1::zeros(n);

    for (i, element) in mesh.elements.iter().enumerate() {
        let nodes: Vec<Point3D> = element.nodes.iter().map(|&idx| mesh.nodes[idx]).collect();
        let (center, _normal) = element_center_and_normal(&nodes);

        let mut total_pressure = Complex64::new(0.0, 0.0);

        for source in sources {
            let r = center.distance_to(&source.position);
            let amplitude = source.amplitude_towards(&center, frequency);

            // Incident pressure from monopole source
            total_pressure += greens_function_3d(r, k) * amplitude;
        }

        incident[i] = total_pressure;
    }

    incident
}

/// Calculate pressure at field points using double-layer potential representation
///
/// Uses: p(x) = p_inc(x) + ∫∫ (∂G/∂n)(x, y) * p_surface(y) dS(y)
pub fn calculate_field_pressure(
    mesh: &RoomMesh,
    surface_pressure: &Array1<Complex64>,
    sources: &[Source],
    field_points: &[Point3D],
    k: f64,
    frequency: f64,
) -> Array1<Complex64> {
    let n_points = field_points.len();
    let mut pressures = Array1::zeros(n_points);

    // Get element data
    let mut centers = Vec::new();
    let mut normals = Vec::new();
    let mut areas = Vec::new();

    for element in &mesh.elements {
        let nodes: Vec<Point3D> = element.nodes.iter().map(|&i| mesh.nodes[i]).collect();
        let (center, normal) = element_center_and_normal(&nodes);
        let area = element_area(&nodes);

        centers.push(center);
        normals.push(normal);
        areas.push(area);
    }

    for (ip, point) in field_points.iter().enumerate() {
        // Incident field from sources
        let mut p_incident = Complex64::new(0.0, 0.0);
        for source in sources {
            let r = point.distance_to(&source.position);
            if r < 1e-10 {
                continue;
            }
            let amplitude = source.amplitude_towards(point, frequency);
            p_incident += greens_function_3d(r, k) * amplitude;
        }

        // Scattered field from boundary integral using double-layer potential
        // p_scattered = ∫∫ (∂G/∂n)(x, y) * p_surface(y) dS(y)
        let mut p_scattered = Complex64::new(0.0, 0.0);

        for j in 0..surface_pressure.len() {
            let r = point.distance_to(&centers[j]);
            if r < 1e-10 {
                continue;
            }

            // Direction from element center to field point
            let dx = point.x - centers[j].x;
            let dy = point.y - centers[j].y;
            let dz = point.z - centers[j].z;

            // Cosine of angle between (point - center) direction and outward normal
            let cos_angle = (dx * normals[j].x + dy * normals[j].y + dz * normals[j].z) / r;

            // Normal derivative of Green's function
            let dg_dn = greens_function_derivative(r, k, cos_angle);

            p_scattered += dg_dn * surface_pressure[j] * areas[j];
        }

        pressures[ip] = p_incident + p_scattered;
    }

    pressures
}

// pressure_to_spl is now in xem_common::types

/// Simple GMRES solver for complex linear systems
/// Solves Ax = b using restarted GMRES
pub fn gmres_solve(
    a: &Array2<Complex64>,
    b: &Array1<Complex64>,
    max_iter: usize,
    restart: usize,
    tol: f64,
) -> Result<Array1<Complex64>, String> {
    let n = b.len();
    if a.nrows() != n || a.ncols() != n {
        return Err("Matrix dimensions mismatch".to_string());
    }

    let mut x = Array1::zeros(n);

    for _cycle in 0..max_iter {
        // Compute initial residual
        let ax = a.dot(&x);
        let r = b - &ax;
        let beta = r.iter().map(|ri| ri.norm_sqr()).sum::<f64>().sqrt();

        if beta < tol {
            return Ok(x);
        }

        // Arnoldi iteration
        let m = restart.min(n);
        let mut v = vec![Array1::zeros(n); m + 1];
        let mut h = Array2::<Complex64>::zeros((m + 1, m));

        v[0] = r.mapv(|ri| ri / Complex64::new(beta, 0.0));

        for j in 0..m {
            // w = A * v[j]
            let w = a.dot(&v[j]);
            let mut w_orth = w.clone();

            // Modified Gram-Schmidt orthogonalization
            for i in 0..=j {
                h[[i, j]] = v[i]
                    .iter()
                    .zip(w_orth.iter())
                    .map(|(vi, wi)| vi.conj() * wi)
                    .sum();

                for k in 0..n {
                    w_orth[k] -= h[[i, j]] * v[i][k];
                }
            }

            let h_norm = w_orth.iter().map(|wi| wi.norm_sqr()).sum::<f64>().sqrt();
            h[[j + 1, j]] = Complex64::new(h_norm, 0.0);

            if h_norm > 1e-12 {
                v[j + 1] = w_orth.mapv(|wi| wi / Complex64::new(h_norm, 0.0));
            } else {
                break;
            }
        }

        // Solve least squares problem: minimize ||β*e1 - H*y||
        let mut e1 = Array1::<Complex64>::zeros(m + 1);
        e1[0] = Complex64::new(beta, 0.0);

        // Use QR decomposition to solve
        let y = solve_least_squares(&h, &e1, m)?;

        // Update solution: x = x + V*y
        for j in 0..m {
            for k in 0..n {
                x[k] += y[j] * v[j][k];
            }
        }

        // Check convergence
        let ax = a.dot(&x);
        let r_final = b - &ax;
        let residual = r_final.iter().map(|ri| ri.norm_sqr()).sum::<f64>().sqrt();

        if residual < tol {
            return Ok(x);
        }
    }

    Ok(x)
}

/// Solve least squares problem using back substitution on upper triangular part
fn solve_least_squares(
    h: &Array2<Complex64>,
    e1: &Array1<Complex64>,
    m: usize,
) -> Result<Array1<Complex64>, String> {
    let mut y = Array1::<Complex64>::zeros(m);
    let mut rhs = e1.slice(ndarray::s![0..m]).to_owned();

    // Apply Givens rotations to make H upper triangular
    let mut h_tri = h.slice(ndarray::s![0..m, 0..m]).to_owned();

    for i in 0..m {
        for j in (i + 1)..m {
            if h_tri[[j, i]].norm() > 1e-12 {
                let a = h_tri[[i, i]];
                let b = h_tri[[j, i]];
                let r = (a.norm_sqr() + b.norm_sqr()).sqrt();
                let c = a.norm() / r;
                let s = b / Complex64::new(r, 0.0);

                // Apply rotation to rows i and j
                for k in i..m {
                    let temp = c * h_tri[[i, k]] + s * h_tri[[j, k]];
                    h_tri[[j, k]] = -s.conj() * h_tri[[i, k]] + c * h_tri[[j, k]];
                    h_tri[[i, k]] = temp;
                }

                let temp = c * rhs[i] + s * rhs[j];
                rhs[j] = -s.conj() * rhs[i] + c * rhs[j];
                rhs[i] = temp;
            }
        }
    }

    // Back substitution
    for i in (0..m).rev() {
        let mut sum = rhs[i];
        for j in (i + 1)..m {
            sum -= h_tri[[i, j]] * y[j];
        }
        if h_tri[[i, i]].norm() > 1e-12 {
            y[i] = sum / h_tri[[i, i]];
        }
    }

    Ok(y)
}

/// Solve BEM system using GMRES with parallel matrix assembly
pub fn solve_bem_system(
    mesh: &RoomMesh,
    sources: &[Source],
    k: f64,
    frequency: f64,
) -> Result<Array1<Complex64>, String> {
    use crate::core::solver::{GmresConfig, solve_gmres};

    // Build BEM matrix using double-layer potential formulation
    let matrix = build_bem_matrix_parallel(mesh, k);

    // Compute RHS from incident field normal derivative
    let rhs = calculate_incident_field_derivative_parallel(mesh, sources, k, frequency);

    // Use the core GMRES solver
    let config = GmresConfig {
        max_iterations: 100,
        restart: 50,
        tolerance: 1e-6,
        print_interval: 0,
    };

    // Create matvec closure for dense matrix (implement LinearOperator or use wrapper)
    // solve_gmres expects a LinearOperator.
    // We can use DenseOperator from core::solver
    use crate::core::solver::DenseOperator;
    let op = DenseOperator::new(matrix);

    let solution = solve_gmres(&op, &rhs, &config);

    Ok(solution.x)
}

/// Build BEM matrix with parallel assembly
///
/// Uses portable parallel iteration that works with native rayon, WASM, or sequential fallback.
pub fn build_bem_matrix_parallel(mesh: &RoomMesh, k: f64) -> Array2<Complex64> {
    let n = mesh.elements.len();

    // Precompute element data (parallel when available)
    let element_data: Vec<_> = parallel_map(&mesh.elements, |element| {
        let nodes: Vec<Point3D> = element.nodes.iter().map(|&i| mesh.nodes[i]).collect();
        let (center, normal) = element_center_and_normal(&nodes);
        let area = element_area(&nodes);
        (center, normal, area)
    });

    // Build matrix rows (parallel when available)
    let rows: Vec<_> = parallel_map_indexed(n, |i| {
        let mut row = vec![Complex64::new(0.0, 0.0); n];
        let (center_i, normal_i, _area_i) = &element_data[i];

        for j in 0..n {
            let (center_j, _normal_j, area_j) = &element_data[j];
            let r = center_i.distance_to(center_j);

            if i == j {
                // Diagonal: self-interaction
                row[j] = Complex64::new(0.0, -k / (2.0 * PI)) * area_j;
            } else {
                // Off-diagonal
                let dx = center_i.x - center_j.x;
                let dy = center_i.y - center_j.y;
                let dz = center_i.z - center_j.z;

                let cos_angle = (dx * normal_i.x + dy * normal_i.y + dz * normal_i.z) / r;
                row[j] = greens_function_derivative(r, k, cos_angle) * area_j;
            }
        }
        row
    });

    // Convert to ndarray
    let mut matrix = Array2::zeros((n, n));
    for (i, row) in rows.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            matrix[[i, j]] = val;
        }
    }

    matrix
}

/// Build BEM matrix with adaptive integration for near-singular elements
///
/// Uses adaptive subdivision for self-interaction and nearby elements to improve accuracy.
/// This is especially important at higher frequencies where standard point collocation
/// becomes inaccurate.
pub fn build_bem_matrix_adaptive(mesh: &RoomMesh, k: f64, use_adaptive: bool) -> Array2<Complex64> {
    use crate::core::integration::singular::QuadratureParams;
    use crate::core::types::{ElementType, PhysicsParams};

    let n = mesh.elements.len();

    // Physics parameters for singular integration
    let speed_of_sound = 343.0;
    let frequency = k * speed_of_sound / (2.0 * PI);
    let omega = 2.0 * PI * frequency;

    let physics = PhysicsParams {
        wave_number: k,
        density: 1.0,
        speed_of_sound,
        frequency,
        omega,
        wave_length: speed_of_sound / frequency,
        harmonic_factor: 1.0, // exp(+ikr) convention
        pressure_factor: 1.0 * omega * 1.0,
        tau: -1.0, // internal problem (room interior)
    };

    // Precompute element data (parallel when available)
    let element_data: Vec<_> = parallel_map(&mesh.elements, |element| {
        let nodes: Vec<Point3D> = element.nodes.iter().map(|&i| mesh.nodes[i]).collect();
        let (center, normal) = element_center_and_normal(&nodes);
        let area = element_area(&nodes);
        let char_length = element_characteristic_length(&nodes);
        (center, normal, area, char_length, nodes)
    });

    // Build matrix rows (parallel when available)
    let rows: Vec<_> = parallel_map_indexed(n, |i| {
        let mut row = vec![Complex64::new(0.0, 0.0); n];
        let (center_i, normal_i, _area_i, char_length_i, _nodes_i) = &element_data[i];

        for j in 0..n {
            let (center_j, _normal_j, area_j, char_length_j, nodes_j) = &element_data[j];
            let r = center_i.distance_to(center_j);

            // Criterion for near-singular: distance < 2 * characteristic length
            let near_threshold = 2.0 * (char_length_i + char_length_j);
            let is_near = r < near_threshold || i == j;

            if use_adaptive && is_near {
                // Use adaptive singular integration for near-field
                let element_coords = nodes_to_array2(nodes_j);
                let source_point = point_to_array1(center_i);
                let source_normal = normal_to_array1(normal_i);

                // Use frequency-adaptive quadrature
                let ka = k * char_length_j;
                let quad_params = QuadratureParams::for_ka(ka);

                let result = crate::core::integration::singular::singular_integration_with_params(
                    &source_point,
                    &source_normal,
                    &element_coords,
                    ElementType::Tri3,
                    &physics,
                    None,
                    0,     // Dirichlet BC type
                    false, // don't compute RHS
                    &quad_params,
                );

                // H matrix coefficient (∂G/∂n term) - double layer potential
                row[j] = result.dg_dn_integral;
            } else {
                // Use point collocation for far-field
                if i == j {
                    // Diagonal: self-interaction approximation
                    row[j] = Complex64::new(0.0, -k / (2.0 * PI)) * area_j;
                } else {
                    // Off-diagonal: standard collocation
                    let dx = center_i.x - center_j.x;
                    let dy = center_i.y - center_j.y;
                    let dz = center_i.z - center_j.z;

                    let cos_angle = (dx * normal_i.x + dy * normal_i.y + dz * normal_i.z) / r;
                    row[j] = greens_function_derivative(r, k, cos_angle) * area_j;
                }
            }
        }
        row
    });

    // Convert to ndarray
    let mut matrix = Array2::zeros((n, n));
    for (i, row) in rows.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            matrix[[i, j]] = val;
        }
    }

    matrix
}

/// Helper: compute characteristic length of an element
fn element_characteristic_length(nodes: &[Point3D]) -> f64 {
    if nodes.len() < 3 {
        return 0.0;
    }

    // For triangles: use average edge length
    let d01 = nodes[0].distance_to(&nodes[1]);
    let d12 = nodes[1].distance_to(&nodes[2]);
    let d20 = nodes[2].distance_to(&nodes[0]);

    (d01 + d12 + d20) / 3.0
}

/// Helper: convert Point3D to Array1<f64>
fn point_to_array1(p: &Point3D) -> Array1<f64> {
    use ndarray::array;
    array![p.x, p.y, p.z]
}

/// Helper: convert normal vector to Array1<f64>
fn normal_to_array1(n: &Point3D) -> Array1<f64> {
    use ndarray::array;
    array![n.x, n.y, n.z]
}

/// Helper: convert nodes to Array2<f64> for singular integration
fn nodes_to_array2(nodes: &[Point3D]) -> Array2<f64> {
    let n = nodes.len();
    let mut coords = Array2::zeros((n, 3));
    for (i, node) in nodes.iter().enumerate() {
        coords[[i, 0]] = node.x;
        coords[[i, 1]] = node.y;
        coords[[i, 2]] = node.z;
    }
    coords
}

/// Calculate incident field normal derivative in parallel
pub fn calculate_incident_field_derivative_parallel(
    mesh: &RoomMesh,
    sources: &[Source],
    k: f64,
    frequency: f64,
) -> Array1<Complex64> {
    let element_data: Vec<_> = parallel_map(&mesh.elements, |element| {
        let nodes: Vec<Point3D> = element.nodes.iter().map(|&idx| mesh.nodes[idx]).collect();
        let (center, normal) = element_center_and_normal(&nodes);

        // Compute incident field derivative
        let mut dpdn_inc = Complex64::new(0.0, 0.0);

        for source in sources {
            let r = center.distance_to(&source.position);
            if r < 1e-10 {
                continue;
            }

            let amplitude = source.amplitude_towards(&center, frequency);

            // Direction from source to point
            let dx = center.x - source.position.x;
            let dy = center.y - source.position.y;
            let dz = center.z - source.position.z;

            // Normal derivative: ∂G/∂n = ∇G · n
            let cos_angle = (dx * normal.x + dy * normal.y + dz * normal.z) / r;

            dpdn_inc += greens_function_derivative(r, k, cos_angle) * amplitude;
        }

        // For rigid boundary condition: ∂p/∂n = 0
        // The BEM formulation gives: H * q = G * p_inc
        // where q = ∂p/∂n is the unknown (should be zero for rigid)
        // We solve: H * q = -∂p_inc/∂n  (to get total field with zero normal derivative)
        -dpdn_inc
    });

    Array1::from_vec(element_data)
}

/// Calculate pressure at field points using BEM solution (parallel version)
///
/// Uses the double-layer potential (DLP) representation for field evaluation:
/// p(x) = p_inc(x) + ∫∫ (∂G/∂n)(x, y) * p_surface(y) dS(y)
///
/// This matches the BEM formulation which solves for surface pressure using
/// the H matrix (double-layer potential operator).
pub fn calculate_field_pressure_bem_parallel(
    mesh: &RoomMesh,
    surface_pressure: &Array1<Complex64>,
    sources: &[Source],
    field_points: &[Point3D],
    k: f64,
    frequency: f64,
) -> Array1<Complex64> {
    // Precompute element data including normals for DLP evaluation
    let element_data: Vec<_> = mesh
        .elements
        .iter()
        .map(|element| {
            let nodes: Vec<Point3D> = element.nodes.iter().map(|&i| mesh.nodes[i]).collect();
            let (center, normal) = element_center_and_normal(&nodes);
            let area = element_area(&nodes);
            (center, normal, area)
        })
        .collect();

    // Calculate pressure at each field point (parallel when available)
    let pressures: Vec<_> = parallel_map(field_points, |point| {
        // Incident field from sources
        let mut p_incident = Complex64::new(0.0, 0.0);
        for source in sources {
            let r = point.distance_to(&source.position);
            if r < 1e-10 {
                continue;
            }
            let amplitude = source.amplitude_towards(point, frequency);
            p_incident += greens_function_3d(r, k) * amplitude;
        }

        // Scattered field from boundary integral using double-layer potential
        // p_scattered = ∫∫ (∂G/∂n)(x, y) * p_surface(y) dS(y)
        // where ∂G/∂n is the normal derivative at the surface element (pointing outward)
        let mut p_scattered = Complex64::new(0.0, 0.0);
        for (j, (center_j, normal_j, area_j)) in element_data.iter().enumerate() {
            let r = point.distance_to(center_j);
            if r < 1e-10 {
                continue;
            }

            // Direction from element center to field point
            let dx = point.x - center_j.x;
            let dy = point.y - center_j.y;
            let dz = point.z - center_j.z;

            // Cosine of angle between (point - center) direction and outward normal
            let cos_angle = (dx * normal_j.x + dy * normal_j.y + dz * normal_j.z) / r;

            // Normal derivative of Green's function: ∂G/∂n = (ikr-1) * exp(ikr) / (4πr²) * cos_angle
            let dg_dn = greens_function_derivative(r, k, cos_angle);

            p_scattered += dg_dn * surface_pressure[j] * area_j;
        }

        p_incident + p_scattered
    });

    Array1::from_vec(pressures)
}

// ============================================================================
// FMM Integration for Room Acoustics
// ============================================================================

use crate::core::assembly::slfmm::{SlfmmSystem, build_slfmm_system};
use crate::core::mesh::octree::Octree;
use crate::core::solver::{
    GmresConfig, GmresSolution, SlfmmOperator, gmres_solve_with_ilu_operator,
};
use crate::core::types::{
    BoundaryCondition, Cluster, Element, ElementProperty, ElementType, PhysicsParams,
};

/// FMM solver configuration
pub struct FmmSolverConfig {
    /// Maximum elements per octree leaf (affects cluster size)
    pub max_elements_per_leaf: usize,
    /// Maximum octree depth
    pub max_tree_depth: usize,
    /// Number of theta integration points on unit sphere
    pub n_theta: usize,
    /// Number of phi integration points on unit sphere
    pub n_phi: usize,
    /// Number of expansion terms
    pub n_terms: usize,
    /// Separation ratio for near/far field classification
    pub separation_ratio: f64,
}

impl Default for FmmSolverConfig {
    fn default() -> Self {
        Self {
            max_elements_per_leaf: 50,
            max_tree_depth: 8,
            n_theta: 6,
            n_phi: 12,
            n_terms: 6,
            separation_ratio: 1.5, // Standard FMM separation: 2/sqrt(3) ≈ 1.155
        }
    }
}

/// Convert RoomMesh to core Element and nodes arrays for FMM
pub fn room_mesh_to_core_elements(mesh: &RoomMesh, _k: f64) -> (Vec<Element>, Array2<f64>) {
    let n_nodes = mesh.nodes.len();
    let n_elements = mesh.elements.len();

    // Convert nodes to Array2
    let mut nodes = Array2::zeros((n_nodes, 3));
    for (i, node) in mesh.nodes.iter().enumerate() {
        nodes[[i, 0]] = node.x;
        nodes[[i, 1]] = node.y;
        nodes[[i, 2]] = node.z;
    }

    // Convert elements
    let mut elements = Vec::with_capacity(n_elements);
    for (elem_idx, surface_elem) in mesh.elements.iter().enumerate() {
        let elem_nodes: Vec<Point3D> = surface_elem.nodes.iter().map(|&i| mesh.nodes[i]).collect();

        let (center, normal) = element_center_and_normal(&elem_nodes);
        let area = element_area(&elem_nodes);

        // Determine element type
        let elem_type = if surface_elem.nodes.len() == 3 {
            ElementType::Tri3
        } else {
            ElementType::Quad4
        };

        let element = Element {
            connectivity: surface_elem.nodes.clone(),
            element_type: elem_type,
            property: ElementProperty::Surface,
            normal: Array1::from_vec(vec![normal.x, normal.y, normal.z]),
            node_normals: Array2::zeros((surface_elem.nodes.len(), 3)),
            center: Array1::from_vec(vec![center.x, center.y, center.z]),
            area,
            boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]),
            group: 0,
            dof_addresses: vec![elem_idx],
        };

        elements.push(element);
    }

    (elements, nodes)
}

/// Build clusters from octree for FMM
pub fn build_clusters_from_octree(octree: &Octree, elements: &[Element]) -> Vec<Cluster> {
    let leaves = octree.leaves();
    let mut clusters = Vec::with_capacity(leaves.len());

    for &leaf_idx in &leaves {
        let node = &octree.nodes[leaf_idx];
        if node.element_indices.is_empty() {
            continue;
        }

        let mut cluster = Cluster::new(node.center.clone());
        cluster.element_indices = node.element_indices.clone();
        cluster.num_elements = node.element_indices.len();
        cluster.element_property = ElementProperty::Surface;
        cluster.radius = node.radius();
        cluster.level = node.level;

        // Count DOFs
        cluster.num_dofs = node
            .element_indices
            .iter()
            .filter(|&&i| !elements[i].property.is_evaluation())
            .count();
        cluster.dofs_per_element = 1;

        clusters.push(cluster);
    }

    // Build near/far lists using octree's computed lists
    // Map from octree leaf indices to cluster indices
    let leaf_to_cluster: std::collections::HashMap<usize, usize> = leaves
        .iter()
        .filter(|&&i| !octree.nodes[i].element_indices.is_empty())
        .enumerate()
        .map(|(cluster_idx, &leaf_idx)| (leaf_idx, cluster_idx))
        .collect();

    // Assign near/far cluster indices
    for (cluster_idx, &leaf_idx) in leaves.iter().enumerate() {
        if octree.nodes[leaf_idx].element_indices.is_empty() {
            continue;
        }

        let octree_node = &octree.nodes[leaf_idx];

        // Map near clusters
        let near: Vec<usize> = octree_node
            .near_clusters
            .iter()
            .filter_map(|&near_leaf| leaf_to_cluster.get(&near_leaf).copied())
            .collect();

        // Map far clusters
        let far: Vec<usize> = octree_node
            .far_clusters
            .iter()
            .filter_map(|&far_leaf| leaf_to_cluster.get(&far_leaf).copied())
            .collect();

        if let Some(cluster) = clusters.get_mut(cluster_idx) {
            cluster.near_clusters = near;
            cluster.far_clusters = far;
        }
    }

    clusters
}

/// Build FMM system for room acoustics
pub fn build_fmm_system(
    mesh: &RoomMesh,
    sources: &[Source],
    k: f64,
    frequency: f64,
    fmm_config: &FmmSolverConfig,
) -> Result<(SlfmmSystem, Vec<Element>, Array2<f64>), String> {
    println!("  Converting room mesh to core elements...");
    let (elements, nodes) = room_mesh_to_core_elements(mesh, k);
    println!("    {} elements, {} nodes", elements.len(), nodes.nrows());

    // Compute element centers for octree
    println!("  Building octree...");
    let centers: Vec<Array1<f64>> = elements.iter().map(|e| e.center.clone()).collect();

    let mut octree = Octree::build(
        &centers,
        fmm_config.max_elements_per_leaf,
        fmm_config.max_tree_depth,
    );

    // Compute interaction lists
    octree.compute_interaction_lists(fmm_config.separation_ratio);

    let stats = octree.stats();
    println!(
        "    {} leaves, {} levels, avg {:.1} elements/leaf",
        stats.num_leaves, stats.num_levels, stats.avg_elements_per_leaf
    );

    // Build clusters from octree
    println!("  Building clusters...");
    let clusters = build_clusters_from_octree(&octree, &elements);
    println!("    {} clusters", clusters.len());

    // Create physics parameters
    let speed_of_sound = 343.0;
    let physics = PhysicsParams::new(frequency, speed_of_sound, 1.21, true);

    // Build SLFMM system
    println!("  Assembling SLFMM system...");
    let mut system = build_slfmm_system(
        &elements,
        &nodes,
        &clusters,
        &physics,
        fmm_config.n_theta,
        fmm_config.n_phi,
        fmm_config.n_terms,
    );

    // Compute RHS
    println!("  Computing RHS...");
    let rhs = calculate_incident_field_derivative_parallel(mesh, sources, k, frequency);
    system.rhs = rhs;

    Ok((system, elements, nodes))
}

/// Solve BEM system using FMM + GMRES + ILU
///
/// This is the recommended solver for large meshes (>1000 elements).
/// Complexity: O(N log N) per iteration vs O(N²) for dense GMRES.
///
/// The near-field matrix for ILU preconditioning is extracted directly
/// from the SLFMM system, avoiding the O(N²) dense matrix assembly.
pub fn solve_bem_fmm_gmres_ilu(
    mesh: &RoomMesh,
    sources: &[Source],
    k: f64,
    frequency: f64,
    fmm_config: &FmmSolverConfig,
    gmres_max_iter: usize,
    gmres_restart: usize,
    gmres_tolerance: f64,
) -> Result<Array1<Complex64>, String> {
    // Build FMM system
    let (system, _elements, _nodes) = build_fmm_system(mesh, sources, k, frequency, fmm_config)?;

    // Extract near-field matrix for ILU preconditioning BEFORE moving system to operator
    // This uses only the already-computed near-field blocks, not a full O(N²) assembly
    println!("  Extracting near-field matrix for ILU...");
    let nearfield_matrix = system.extract_near_field_matrix();
    println!(
        "    Near-field matrix: {}x{}",
        nearfield_matrix.nrows(),
        nearfield_matrix.ncols()
    );

    // Create FMM operator (takes ownership of system)
    let fmm_operator = SlfmmOperator::new(system);

    // GMRES configuration
    let gmres_config = GmresConfig {
        max_iterations: gmres_max_iter,
        restart: gmres_restart,
        tolerance: gmres_tolerance,
        print_interval: 0,
    };

    // Get RHS from operator
    let rhs = fmm_operator.rhs().clone();

    // Solve with FMM-accelerated GMRES + ILU preconditioning
    println!("  Solving with FMM + GMRES + ILU...");
    let result =
        gmres_solve_with_ilu_operator(&fmm_operator, &nearfield_matrix, &rhs, &gmres_config);

    if result.converged {
        println!(
            "    Converged in {} iterations, residual: {:.2e}",
            result.iterations, result.residual
        );
    } else {
        println!(
            "    Warning: Did not converge after {} iterations, residual: {:.2e}",
            result.iterations, result.residual
        );
    }

    Ok(result.x)
}

/// Solve BEM system using FMM + GMRES with hierarchical preconditioner
///
/// This is an alternative to `solve_bem_fmm_gmres_ilu` that uses a hierarchical
/// block-diagonal preconditioner based on the FMM near-field blocks.
///
/// ## Advantages
/// - O(N) preconditioner setup (vs O(N²) for ILU on extracted dense matrix)
/// - Parallel LU factorization of each cluster block
/// - No dense matrix extraction needed
///
/// ## When to use
/// - For very large problems where ILU setup time dominates
/// - When memory is constrained (no dense matrix extraction)
pub fn solve_bem_fmm_gmres_hierarchical(
    mesh: &RoomMesh,
    sources: &[Source],
    k: f64,
    frequency: f64,
    fmm_config: &FmmSolverConfig,
    gmres_max_iter: usize,
    gmres_restart: usize,
    gmres_tolerance: f64,
) -> Result<Array1<Complex64>, String> {
    use crate::core::solver::gmres_solve_with_hierarchical_precond;

    // Build FMM system
    let (system, _elements, _nodes) = build_fmm_system(mesh, sources, k, frequency, fmm_config)?;

    // GMRES configuration
    let gmres_config = GmresConfig {
        max_iterations: gmres_max_iter,
        restart: gmres_restart,
        tolerance: gmres_tolerance,
        print_interval: 0,
    };

    // Get RHS
    let rhs = system.rhs.clone();

    // Solve with hierarchical preconditioner
    println!("  Solving with FMM + GMRES + Hierarchical Preconditioner...");
    let result = gmres_solve_with_hierarchical_precond(&system, &rhs, &gmres_config);

    if result.converged {
        println!(
            "    Converged in {} iterations, residual: {:.2e}",
            result.iterations, result.residual
        );
    } else {
        println!(
            "    Warning: Did not converge after {} iterations, residual: {:.2e}",
            result.iterations, result.residual
        );
    }

    Ok(result.x)
}

/// Solve BEM system using FMM + GMRES + ILU with full result
///
/// Same as `solve_bem_fmm_gmres_ilu` but returns the full GmresSolution with
/// convergence info.
pub fn solve_bem_fmm_gmres_ilu_with_result(
    mesh: &RoomMesh,
    sources: &[Source],
    k: f64,
    frequency: f64,
    fmm_config: &FmmSolverConfig,
    gmres_max_iter: usize,
    gmres_restart: usize,
    gmres_tolerance: f64,
) -> Result<GmresSolution<Complex64>, String> {
    // Build FMM system
    let (system, _elements, _nodes) = build_fmm_system(mesh, sources, k, frequency, fmm_config)?;

    // Extract near-field matrix for ILU preconditioning BEFORE moving system to operator
    println!("  Extracting near-field matrix for ILU...");
    let nearfield_matrix = system.extract_near_field_matrix();
    println!(
        "    Near-field matrix: {}x{}",
        nearfield_matrix.nrows(),
        nearfield_matrix.ncols()
    );

    // Create FMM operator (takes ownership of system)
    let fmm_operator = SlfmmOperator::new(system);

    // GMRES configuration
    let gmres_config = GmresConfig {
        max_iterations: gmres_max_iter,
        restart: gmres_restart,
        tolerance: gmres_tolerance,
        print_interval: 0,
    };

    // Get RHS from operator
    let rhs = fmm_operator.rhs().clone();

    // Solve with FMM-accelerated GMRES + ILU preconditioning
    println!("  Solving with FMM + GMRES + ILU...");
    let result =
        gmres_solve_with_ilu_operator(&fmm_operator, &nearfield_matrix, &rhs, &gmres_config);

    if result.converged {
        println!(
            "    Converged in {} iterations, residual: {:.2e}",
            result.iterations, result.residual
        );
    } else {
        println!(
            "    Warning: Did not converge after {} iterations, residual: {:.2e}",
            result.iterations, result.residual
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greens_function() {
        let k = 2.0 * PI * 1000.0 / 343.0;
        let r = 1.0;
        let g = greens_function_3d(r, k);
        // Should have magnitude approximately 1/(4πr)
        assert!((g.norm() - 1.0 / (4.0 * PI)).abs() < 0.1);
    }

    #[test]
    fn test_pressure_to_spl() {
        let p = Complex64::new(1.0, 0.0); // 1 Pa
        let spl = pressure_to_spl(p);
        // 1 Pa = 94 dB SPL
        assert!((spl - 94.0).abs() < 1.0);
    }

    #[test]
    fn test_room_mesh_to_core_elements() {
        // Create a simple test mesh
        let nodes = vec![
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(1.0, 0.0, 0.0),
            Point3D::new(0.5, 1.0, 0.0),
        ];
        let elements = vec![SurfaceElement {
            nodes: vec![0, 1, 2],
        }];
        let mesh = RoomMesh { nodes, elements };

        let k = 2.0 * PI * 100.0 / 343.0;
        let (core_elements, core_nodes) = room_mesh_to_core_elements(&mesh, k);

        assert_eq!(core_elements.len(), 1);
        assert_eq!(core_nodes.nrows(), 3);
        assert_eq!(core_elements[0].element_type, ElementType::Tri3);
    }
}
