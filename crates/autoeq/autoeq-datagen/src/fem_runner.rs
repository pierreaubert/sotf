//! FEM simulation driver
//!
//! Uses `math_audio_fem` to solve the Helmholtz equation for room acoustics
//! per source per frequency, returning complex pressure at each listening position.
//!
//! Key functions (`create_room_mesh`, `assemble_rhs_parallel`,
//! `find_containing_element_parallel`, `evaluate_solution_at_point_interpolated`)
//! are adapted from `crates/math-fem/bin/room_simulator_fem.rs`.

use anyhow::Result;
use math_audio_fem::assembly::{
    HelmholtzAssembler, assemble_boundary_mass, assemble_mass, assemble_stiffness,
};
use math_audio_fem::basis::{Jacobian, PolynomialDegree, evaluate_shape};
use math_audio_fem::mesh::{BoundaryType, ElementType, Mesh, Point};
use math_audio_fem::quadrature::for_mass;
use math_audio_fem::solver::{self, SolverConfig, SolverType};
use math_audio_xem_common::{Point3D, RoomSimulation, Source};
use num_complex::Complex64;
use rayon::prelude::*;
use std::collections::HashMap;

use crate::SimulationOutput;

/// Default source width in meters (Gaussian sigma)
const DEFAULT_SOURCE_WIDTH: f64 = 0.1;

/// Default mesh resolution (elements per meter).
/// At 500 Hz max (wavelength ~0.69m) this gives ~5 elements per wavelength.
const MESH_RESOLUTION: usize = 3;

/// Boundary markers
const MARKER_FLOOR: i32 = 1;
const MARKER_CEILING: i32 = 2;
const MARKER_FRONT: i32 = 3;
const MARKER_BACK: i32 = 4;
const MARKER_LEFT: i32 = 5;
const MARKER_RIGHT: i32 = 6;

/// Run FEM simulation for each source independently.
///
/// For every source, sweeps all frequencies and computes the complex pressure
/// at each listening position.  Returns `[source_idx][lp_idx][freq_idx]`.
pub fn run_fem(simulation: &RoomSimulation) -> Result<SimulationOutput> {
    let mesh = create_room_mesh(simulation, MESH_RESOLUTION);
    log::info!(
        "FEM mesh: {} nodes, {} elements",
        mesh.num_nodes(),
        mesh.num_elements()
    );

    // Assemble frequency-independent matrices once
    let stiffness = assemble_stiffness(&mesh, PolynomialDegree::P1);
    let mass = assemble_mass(&mesh, PolynomialDegree::P1);

    let markers = [
        MARKER_FLOOR,
        MARKER_CEILING,
        MARKER_FRONT,
        MARKER_BACK,
        MARKER_LEFT,
        MARKER_RIGHT,
    ];
    let mut boundary_matrices = Vec::new();
    for &marker in &markers {
        let b_mass = assemble_boundary_mass(&mesh, PolynomialDegree::P1, marker);
        if b_mass.nnz() > 0 {
            boundary_matrices.push((marker as usize, b_mass));
        }
    }

    let assembler = HelmholtzAssembler::from_matrices(&stiffness, &mass, &boundary_matrices);

    // Pre-locate elements containing listening positions
    let lp_elements: Vec<Option<usize>> = simulation
        .listening_positions
        .par_iter()
        .map(|lp| find_containing_element_parallel(&mesh, *lp))
        .collect();

    // Direct LU: at 900–3600 DOFs the system is small enough that dense
    // factorisation (~1–10 ms per frequency) is faster and unconditionally
    // stable — no convergence issues unlike iterative Helmholtz solvers.
    let solver_config = SolverConfig {
        solver_type: SolverType::Direct,
        verbosity: 0,
        ..Default::default()
    };

    let n_sources = simulation.sources.len();
    let n_lps = simulation.listening_positions.len();
    let n_freqs = simulation.frequencies.len();

    let mut pressures: Vec<Vec<Vec<Complex64>>> =
        vec![vec![vec![Complex64::new(0.0, 0.0); n_freqs]; n_lps]; n_sources];

    for (src_idx, source) in simulation.sources.iter().enumerate() {
        log::info!(
            "FEM: solving source {} of {} ({})",
            src_idx + 1,
            n_sources,
            source.name
        );

        let single_source = vec![source.clone()];

        let freq_results: Vec<(usize, Vec<Complex64>)> = simulation
            .frequencies
            .par_iter()
            .enumerate()
            .map(|(freq_idx, &freq)| {
                let k = simulation.wavenumber(freq);
                let k_complex = Complex64::new(k, 0.0);

                let boundary_coeffs = compute_boundary_coefficients(simulation, freq);
                let csr = assembler.assemble(k_complex, &boundary_coeffs);
                let rhs = assemble_rhs_parallel(&mesh, &single_source, freq, k, DEFAULT_SOURCE_WIDTH);
                let rhs_array = ndarray::Array1::from(rhs);

                let solution =
                    solver::solve_csr_with_guess(&csr, &rhs_array, None, &solver_config)
                        .unwrap_or_else(|e| {
                            log::warn!(
                                "FEM solve failed for source {} freq {:.1} Hz: {}",
                                source.name,
                                freq,
                                e
                            );
                            solver::Solution {
                                values: ndarray::Array1::zeros(mesh.num_nodes()),
                                iterations: 0,
                                residual: f64::NAN,
                                converged: false,
                            }
                        });

                let lp_pressures: Vec<Complex64> = simulation
                    .listening_positions
                    .iter()
                    .zip(lp_elements.iter())
                    .map(|(lp, elem_opt)| {
                        evaluate_solution_at_point_interpolated(
                            &mesh,
                            &solution.values,
                            *lp,
                            *elem_opt,
                        )
                    })
                    .collect();

                (freq_idx, lp_pressures)
            })
            .collect();

        for (freq_idx, lp_pressures) in freq_results {
            for (lp_idx, pressure) in lp_pressures.into_iter().enumerate() {
                pressures[src_idx][lp_idx][freq_idx] = pressure;
            }
        }
    }

    Ok(SimulationOutput {
        frequencies: simulation.frequencies.clone(),
        pressures,
        source_names: simulation
            .sources
            .iter()
            .map(|s| s.name.clone())
            .collect(),
    })
}

// ============================================================================
// Functions adapted from crates/math-fem/bin/room_simulator_fem.rs
// ============================================================================

/// Create a tetrahedral mesh for the room using conforming 6-tetrahedron decomposition
fn create_room_mesh(simulation: &RoomSimulation, elements_per_meter: usize) -> Mesh {
    let (width, depth, height) = simulation.room.dimensions();

    let nx = (width * elements_per_meter as f64).ceil() as usize + 1;
    let ny = (depth * elements_per_meter as f64).ceil() as usize + 1;
    let nz = (height * elements_per_meter as f64).ceil() as usize + 1;

    let dx = width / (nx - 1) as f64;
    let dy = depth / (ny - 1) as f64;
    let dz = height / (nz - 1) as f64;

    let mut mesh = Mesh::new(3);

    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                mesh.add_node(Point::new_3d(
                    i as f64 * dx,
                    j as f64 * dy,
                    k as f64 * dz,
                ));
            }
        }
    }

    for k in 0..(nz - 1) {
        for j in 0..(ny - 1) {
            for i in 0..(nx - 1) {
                let v0 = k * ny * nx + j * nx + i;
                let v1 = v0 + 1;
                let v2 = v0 + nx;
                let v3 = v2 + 1;
                let v4 = v0 + ny * nx;
                let v5 = v4 + 1;
                let v6 = v4 + nx;
                let v7 = v6 + 1;

                if (i + j + k) % 2 == 0 {
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v1, v3, v7]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v1, v7, v5]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v5, v7, v4]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v3, v2, v7]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v2, v6, v7]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v6, v4, v7]);
                } else {
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v0, v2, v6]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v0, v6, v4]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v4, v6, v5]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v2, v3, v6]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v3, v7, v6]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v7, v5, v6]);
                }
            }
        }
    }

    mesh.detect_boundaries();

    let eps = 1e-6;

    mesh.set_boundary_condition(BoundaryType::Neumann, MARKER_FLOOR, |pts| {
        pts.iter().all(|p| p.z.abs() < eps)
    });
    mesh.set_boundary_condition(BoundaryType::Neumann, MARKER_CEILING, |pts| {
        pts.iter().all(|p| (p.z - height).abs() < eps)
    });
    mesh.set_boundary_condition(BoundaryType::Neumann, MARKER_FRONT, |pts| {
        pts.iter().all(|p| p.y.abs() < eps)
    });
    mesh.set_boundary_condition(BoundaryType::Neumann, MARKER_BACK, |pts| {
        pts.iter().all(|p| (p.y - depth).abs() < eps)
    });
    mesh.set_boundary_condition(BoundaryType::Neumann, MARKER_LEFT, |pts| {
        pts.iter().all(|p| p.x.abs() < eps)
    });
    mesh.set_boundary_condition(BoundaryType::Neumann, MARKER_RIGHT, |pts| {
        pts.iter().all(|p| (p.x - width).abs() < eps)
    });

    mesh
}

/// Compute boundary impedance coefficients for a given frequency
fn compute_boundary_coefficients(
    simulation: &RoomSimulation,
    frequency: f64,
) -> HashMap<usize, Complex64> {
    use math_audio_xem_common::SurfaceConfig;

    let mut coeffs = HashMap::new();
    let k = simulation.wavenumber(frequency);
    let rho_c = 1.21 * simulation.speed_of_sound;

    let get_coeff = |config: &SurfaceConfig| -> Complex64 {
        match config {
            SurfaceConfig::Rigid => Complex64::new(0.0, 0.0),
            SurfaceConfig::Absorption { coefficient } => {
                let alpha = coefficient.clamp(0.0, 0.999);
                let sqrt_1_minus_alpha = (1.0 - alpha).sqrt();
                let z_norm = (1.0 + sqrt_1_minus_alpha) / (1.0 - sqrt_1_minus_alpha);
                let z = z_norm * rho_c;
                Complex64::new(0.0, k * rho_c) / Complex64::new(z, 0.0)
            }
            SurfaceConfig::Impedance { real, imag } => {
                let z = Complex64::new(*real, *imag);
                Complex64::new(0.0, k * rho_c) / z
            }
        }
    };

    let b = &simulation.boundaries;
    let wall_coeff = get_coeff(&b.walls);

    coeffs.insert(MARKER_FLOOR as usize, get_coeff(&b.floor));
    coeffs.insert(MARKER_CEILING as usize, get_coeff(&b.ceiling));
    coeffs.insert(
        MARKER_FRONT as usize,
        b.front_wall.as_ref().map(&get_coeff).unwrap_or(wall_coeff),
    );
    coeffs.insert(
        MARKER_BACK as usize,
        b.back_wall.as_ref().map(&get_coeff).unwrap_or(wall_coeff),
    );
    coeffs.insert(
        MARKER_LEFT as usize,
        b.left_wall.as_ref().map(&get_coeff).unwrap_or(wall_coeff),
    );
    coeffs.insert(
        MARKER_RIGHT as usize,
        b.right_wall.as_ref().map(&get_coeff).unwrap_or(wall_coeff),
    );

    coeffs
}

/// Assemble RHS vector with parallel element processing
fn assemble_rhs_parallel(
    mesh: &Mesh,
    sources: &[Source],
    frequency: f64,
    k: f64,
    source_width: f64,
) -> Vec<Complex64> {
    let n_dofs = mesh.num_nodes();
    let n_elems = mesh.num_elements();

    let element_contribs: Vec<Vec<(usize, Complex64)>> = (0..n_elems)
        .into_par_iter()
        .map(|elem_idx| {
            let elem = &mesh.elements[elem_idx];
            let elem_type = elem.element_type;
            let vertices = elem.vertices();
            let n_nodes = vertices.len();

            let quad = for_mass(elem_type, 1);

            let coords: Vec<[f64; 3]> = vertices
                .iter()
                .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
                .collect();

            let mut local_contribs = Vec::with_capacity(n_nodes);

            for qp in quad.iter() {
                let shape = evaluate_shape(
                    elem_type,
                    PolynomialDegree::P1,
                    qp.xi(),
                    qp.eta(),
                    qp.zeta(),
                );
                let jac = Jacobian::from_3d(&shape.gradients, &coords);

                let x: f64 = shape
                    .values
                    .iter()
                    .zip(&coords)
                    .map(|(n, c)| n * c[0])
                    .sum();
                let y: f64 = shape
                    .values
                    .iter()
                    .zip(&coords)
                    .map(|(n, c)| n * c[1])
                    .sum();
                let z: f64 = shape
                    .values
                    .iter()
                    .zip(&coords)
                    .map(|(n, c)| n * c[2])
                    .sum();

                let f_val = compute_source_term(x, y, z, sources, frequency, k, source_width);
                let det_j = jac.det.abs();

                for (i, &vertex) in vertices.iter().enumerate().take(n_nodes) {
                    let contrib = f_val * Complex64::new(shape.values[i] * det_j * qp.weight, 0.0);
                    if contrib.norm() > 1e-15 {
                        local_contribs.push((vertex, contrib));
                    }
                }
            }

            local_contribs
        })
        .collect();

    let mut rhs = vec![Complex64::new(0.0, 0.0); n_dofs];
    for contribs in element_contribs {
        for (node_idx, val) in contribs {
            rhs[node_idx] += val;
        }
    }

    rhs
}

fn compute_source_term(
    x: f64,
    y: f64,
    z: f64,
    sources: &[Source],
    frequency: f64,
    k: f64,
    source_width: f64,
) -> Complex64 {
    let point = Point3D::new(x, y, z);
    let mut total = Complex64::new(0.0, 0.0);

    for source in sources {
        let r = source.position.distance_to(&point);
        let envelope = (-r * r / (2.0 * source_width * source_width)).exp();
        let amplitude = source.amplitude_towards(&point, frequency);
        // Include propagation phase: the source radiates as e^{-ikr}
        total += Complex64::from_polar(amplitude * envelope, -k * r);
    }

    total
}

fn find_containing_element_parallel(mesh: &Mesh, point: Point3D) -> Option<usize> {
    (0..mesh.elements.len())
        .into_par_iter()
        .find_map_any(|elem_idx| {
            let elem = &mesh.elements[elem_idx];
            if elem.element_type != ElementType::Tetrahedron {
                return None;
            }

            let vertices = elem.vertices();
            if vertices.len() != 4 {
                return None;
            }

            let p0 = &mesh.nodes[vertices[0]];
            let p1 = &mesh.nodes[vertices[1]];
            let p2 = &mesh.nodes[vertices[2]];
            let p3 = &mesh.nodes[vertices[3]];

            if compute_barycentric_coords(point, p0, p1, p2, p3).is_some() {
                Some(elem_idx)
            } else {
                None
            }
        })
}

fn compute_barycentric_coords(
    p: Point3D,
    v0: &Point,
    v1: &Point,
    v2: &Point,
    v3: &Point,
) -> Option<[f64; 4]> {
    let v0p = [p.x - v0.x, p.y - v0.y, p.z - v0.z];
    let v01 = [v1.x - v0.x, v1.y - v0.y, v1.z - v0.z];
    let v02 = [v2.x - v0.x, v2.y - v0.y, v2.z - v0.z];
    let v03 = [v3.x - v0.x, v3.y - v0.y, v3.z - v0.z];

    let det = v01[0] * (v02[1] * v03[2] - v02[2] * v03[1])
        - v01[1] * (v02[0] * v03[2] - v02[2] * v03[0])
        + v01[2] * (v02[0] * v03[1] - v02[1] * v03[0]);

    if det.abs() < 1e-15 {
        return None;
    }

    let inv_det = 1.0 / det;

    let c00 = v02[1] * v03[2] - v02[2] * v03[1];
    let c10 = -(v01[1] * v03[2] - v01[2] * v03[1]);
    let c20 = v01[1] * v02[2] - v01[2] * v02[1];

    let c01 = -(v02[0] * v03[2] - v02[2] * v03[0]);
    let c11 = v01[0] * v03[2] - v01[2] * v03[0];
    let c21 = -(v01[0] * v02[2] - v01[2] * v02[0]);

    let c02 = v02[0] * v03[1] - v02[1] * v03[0];
    let c12 = -(v01[0] * v03[1] - v01[1] * v03[0]);
    let c22 = v01[0] * v02[1] - v01[1] * v02[0];

    let lambda1 = (c00 * v0p[0] + c10 * v0p[1] + c20 * v0p[2]) * inv_det;
    let lambda2 = (c01 * v0p[0] + c11 * v0p[1] + c21 * v0p[2]) * inv_det;
    let lambda3 = (c02 * v0p[0] + c12 * v0p[1] + c22 * v0p[2]) * inv_det;
    let lambda0 = 1.0 - lambda1 - lambda2 - lambda3;

    let tol = -1e-10;
    if lambda0 >= tol && lambda1 >= tol && lambda2 >= tol && lambda3 >= tol {
        Some([lambda0, lambda1, lambda2, lambda3])
    } else {
        None
    }
}

fn evaluate_solution_at_point_interpolated(
    mesh: &Mesh,
    solution: &ndarray::Array1<Complex64>,
    point: Point3D,
    containing_element: Option<usize>,
) -> Complex64 {
    if let Some(elem_idx) = containing_element {
        let elem = &mesh.elements[elem_idx];
        let vertices = elem.vertices();

        if vertices.len() == 4 {
            let v0 = &mesh.nodes[vertices[0]];
            let v1 = &mesh.nodes[vertices[1]];
            let v2 = &mesh.nodes[vertices[2]];
            let v3 = &mesh.nodes[vertices[3]];

            if let Some(bary) = compute_barycentric_coords(point, v0, v1, v2, v3) {
                return solution[vertices[0]] * bary[0]
                    + solution[vertices[1]] * bary[1]
                    + solution[vertices[2]] * bary[2]
                    + solution[vertices[3]] * bary[3];
            }
        }
    }

    // Fallback: nearest node
    let mut min_dist = f64::MAX;
    let mut nearest_node = 0;

    for (i, node) in mesh.nodes.iter().enumerate() {
        let dist =
            (node.x - point.x).powi(2) + (node.y - point.y).powi(2) + (node.z - point.z).powi(2);
        if dist < min_dist {
            min_dist = dist;
            nearest_node = i;
        }
    }

    solution[nearest_node]
}
