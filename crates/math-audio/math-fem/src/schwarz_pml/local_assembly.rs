//! PML-modified Helmholtz assembly for local subdomain problems
//!
//! Assembles the complex-valued system matrix A_pml = K_pml - k² M_pml
//! where stiffness and mass terms are modified by PML stretching coefficients.

use crate::basis::{Jacobian, PolynomialDegree, evaluate_shape};
use crate::boundary::PmlRegion;
use crate::mesh::{ElementType, Mesh, Point};
use crate::quadrature::for_mass;
use math_audio_solvers::CsrMatrix;
use num_complex::Complex64;
use std::collections::HashSet;

/// Assemble the PML-modified Helmholtz system for a subdomain
///
/// Returns a CSR matrix A = K_pml - k² M_pml where:
/// - In non-PML elements: standard assembly (real stiffness and mass)
/// - In PML elements: complex-valued assembly with PML stretching coefficients
///
/// Also applies homogeneous Dirichlet BCs at specified nodes.
pub fn assemble_local_pml_system(
    local_mesh: &Mesh,
    degree: PolynomialDegree,
    k: f64,
    pml_regions: &[PmlRegion],
    dirichlet_local_nodes: &HashSet<usize>,
) -> CsrMatrix<Complex64> {
    let n_dofs = local_mesh.num_nodes();
    let k_sq = Complex64::new(k * k, 0.0);

    let mut triplets: Vec<(usize, usize, Complex64)> = Vec::new();

    for elem_idx in 0..local_mesh.num_elements() {
        let elem_triplets = assemble_element_pml(local_mesh, elem_idx, degree, k_sq, pml_regions);
        triplets.extend(elem_triplets);
    }

    // Build CSR (from_triplets sums duplicates)
    let mut csr = CsrMatrix::from_triplets(n_dofs, n_dofs, triplets);

    // Apply homogeneous Dirichlet BCs: zero row and column, set diagonal to 1
    apply_homogeneous_dirichlet(&mut csr, dirichlet_local_nodes);

    csr
}

/// Assemble RHS for a local subdomain problem
///
/// Restricts the global RHS to local nodes and applies Dirichlet values.
#[allow(dead_code)]
pub fn assemble_local_rhs(
    global_rhs: &[Complex64],
    local_to_global: &[usize],
    dirichlet_local_nodes: &HashSet<usize>,
    dirichlet_values: &[(usize, Complex64)], // (local_node, value)
) -> Vec<Complex64> {
    let n_local = local_to_global.len();
    let mut rhs = vec![Complex64::new(0.0, 0.0); n_local];

    // Restrict global RHS to local nodes
    for (local, &global) in local_to_global.iter().enumerate() {
        if global < global_rhs.len() {
            rhs[local] = global_rhs[global];
        }
    }

    // Apply Dirichlet BCs
    for &local in dirichlet_local_nodes {
        rhs[local] = Complex64::new(0.0, 0.0);
    }
    for &(local, val) in dirichlet_values {
        rhs[local] = val;
    }

    rhs
}

/// Assemble a single element's contribution to the PML-modified system
///
/// Returns triplets (local_i, local_j, value) for A = K_pml - k² M_pml
fn assemble_element_pml(
    mesh: &Mesh,
    elem_idx: usize,
    degree: PolynomialDegree,
    k_sq: Complex64,
    pml_regions: &[PmlRegion],
) -> Vec<(usize, usize, Complex64)> {
    let elem = &mesh.elements[elem_idx];
    let elem_type = elem.element_type;
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    // Check if element centroid is in any PML region
    let centroid = mesh.element_centroid(elem_idx);
    let in_pml: Option<&PmlRegion> = pml_regions.iter().find(|pml| pml.contains(&centroid));

    // Use higher-order quadrature (mass-accurate) for both K and M
    // to ensure complex PML coefficients are properly integrated
    let quad = for_mass(elem_type, degree.degree());

    let is_3d = matches!(
        elem_type,
        ElementType::Tetrahedron | ElementType::Hexahedron
    );

    let mut k_local = vec![vec![Complex64::new(0.0, 0.0); n_nodes]; n_nodes];
    let mut m_local = vec![vec![Complex64::new(0.0, 0.0); n_nodes]; n_nodes];

    if is_3d {
        let coords: Vec<[f64; 3]> = vertices
            .iter()
            .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
            .collect();

        for qp in quad.iter() {
            let shape = evaluate_shape(elem_type, degree, qp.xi(), qp.eta(), qp.zeta());
            let jac = Jacobian::from_3d(&shape.gradients, &coords);
            let det_j = jac.det.abs();

            // Map to physical coordinates
            let phys = map_to_physical_3d(&shape.values, &coords);
            let point = Point::new_3d(phys[0], phys[1], phys[2]);

            // Transform gradients to physical space
            let grads_phys: Vec<Vec<f64>> = shape
                .gradients
                .iter()
                .map(|g| jac.transform_gradient(g))
                .collect();

            // PML stretching coefficients
            let (coeff_x, coeff_y, coeff_z, mass_coeff) = if let Some(pml) = in_pml {
                (
                    pml.stiffness_coefficient_x(&point),
                    pml.stiffness_coefficient_y(&point),
                    pml.stiffness_coefficient_z(&point),
                    pml.mass_coefficient(&point),
                )
            } else {
                (
                    Complex64::new(1.0, 0.0),
                    Complex64::new(1.0, 0.0),
                    Complex64::new(1.0, 0.0),
                    Complex64::new(1.0, 0.0),
                )
            };

            let w_det = det_j * qp.weight;

            for i in 0..n_nodes {
                for j in 0..n_nodes {
                    // K_ij = (coeff_x * dN_i/dx * dN_j/dx + coeff_y * dN_i/dy * dN_j/dy + coeff_z * dN_i/dz * dN_j/dz) * det_J * w
                    let stiff_val = coeff_x * grads_phys[i][0] * grads_phys[j][0]
                        + coeff_y * grads_phys[i][1] * grads_phys[j][1]
                        + coeff_z * grads_phys[i][2] * grads_phys[j][2];
                    k_local[i][j] += stiff_val * w_det;

                    // M_ij = mass_coeff * N_i * N_j * det_J * w
                    m_local[i][j] += mass_coeff * shape.values[i] * shape.values[j] * w_det;
                }
            }
        }
    } else {
        // 2D case
        let coords: Vec<[f64; 2]> = vertices
            .iter()
            .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
            .collect();

        for qp in quad.iter() {
            let shape = evaluate_shape(elem_type, degree, qp.xi(), qp.eta(), 0.0);
            let jac = Jacobian::from_2d(&shape.gradients, &coords);
            let det_j = jac.det.abs();

            // Map to physical coordinates
            let phys = map_to_physical_2d(&shape.values, &coords);
            let point = Point::new_2d(phys[0], phys[1]);

            // Transform gradients to physical space
            let grads_phys: Vec<Vec<f64>> = shape
                .gradients
                .iter()
                .map(|g| jac.transform_gradient(g))
                .collect();

            // PML stretching coefficients (for 2D, sz = 1)
            let (coeff_x, coeff_y, mass_coeff) = if let Some(pml) = in_pml {
                (
                    pml.stiffness_coefficient_x(&point),
                    pml.stiffness_coefficient_y(&point),
                    pml.mass_coefficient(&point),
                )
            } else {
                (
                    Complex64::new(1.0, 0.0),
                    Complex64::new(1.0, 0.0),
                    Complex64::new(1.0, 0.0),
                )
            };

            let w_det = det_j * qp.weight;

            for i in 0..n_nodes {
                for j in 0..n_nodes {
                    // K_ij = (coeff_x * dN_i/dx * dN_j/dx + coeff_y * dN_i/dy * dN_j/dy) * det_J * w
                    let stiff_val = coeff_x * grads_phys[i][0] * grads_phys[j][0]
                        + coeff_y * grads_phys[i][1] * grads_phys[j][1];
                    k_local[i][j] += stiff_val * w_det;

                    // M_ij = mass_coeff * N_i * N_j * det_J * w
                    m_local[i][j] += mass_coeff * shape.values[i] * shape.values[j] * w_det;
                }
            }
        }
    }

    // Combine: A = K - k² M → triplets
    let mut triplets = Vec::with_capacity(n_nodes * n_nodes);
    for (i, &gi) in vertices.iter().enumerate() {
        for (j, &gj) in vertices.iter().enumerate() {
            let val = k_local[i][j] - k_sq * m_local[i][j];
            if val.norm() > 1e-15 {
                triplets.push((gi, gj, val));
            }
        }
    }

    triplets
}

/// Map reference coordinates to physical coordinates (2D)
fn map_to_physical_2d(shape_values: &[f64], coords: &[[f64; 2]]) -> [f64; 2] {
    let x: f64 = shape_values.iter().zip(coords).map(|(n, c)| n * c[0]).sum();
    let y: f64 = shape_values.iter().zip(coords).map(|(n, c)| n * c[1]).sum();
    [x, y]
}

/// Map reference coordinates to physical coordinates (3D)
fn map_to_physical_3d(shape_values: &[f64], coords: &[[f64; 3]]) -> [f64; 3] {
    let x: f64 = shape_values.iter().zip(coords).map(|(n, c)| n * c[0]).sum();
    let y: f64 = shape_values.iter().zip(coords).map(|(n, c)| n * c[1]).sum();
    let z: f64 = shape_values.iter().zip(coords).map(|(n, c)| n * c[2]).sum();
    [x, y, z]
}

/// Apply homogeneous Dirichlet BCs to a CSR matrix
///
/// For each Dirichlet node: zero out the row, zero out the column, set diagonal to 1.
fn apply_homogeneous_dirichlet(csr: &mut CsrMatrix<Complex64>, dirichlet_nodes: &HashSet<usize>) {
    if dirichlet_nodes.is_empty() {
        return;
    }

    let n = csr.num_rows;
    let one = Complex64::new(1.0, 0.0);
    let zero = Complex64::new(0.0, 0.0);

    // Zero rows and set diagonal for Dirichlet nodes
    for &node in dirichlet_nodes {
        if node >= n {
            continue;
        }
        let row_start = csr.row_ptrs[node];
        let row_end = csr.row_ptrs[node + 1];
        for idx in row_start..row_end {
            if csr.col_indices[idx] == node {
                csr.values[idx] = one;
            } else {
                csr.values[idx] = zero;
            }
        }
    }

    // Zero columns for Dirichlet nodes
    for row in 0..n {
        if dirichlet_nodes.contains(&row) {
            continue;
        }
        let row_start = csr.row_ptrs[row];
        let row_end = csr.row_ptrs[row + 1];
        for idx in row_start..row_end {
            if dirichlet_nodes.contains(&csr.col_indices[idx]) {
                csr.values[idx] = zero;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_assemble_local_no_pml() {
        let mesh = unit_square_triangles(4);
        let dirichlet = HashSet::new();

        let csr = assemble_local_pml_system(&mesh, PolynomialDegree::P1, 1.0, &[], &dirichlet);

        assert_eq!(csr.num_rows, mesh.num_nodes());
        assert!(csr.nnz() > 0);
    }

    #[test]
    fn test_assemble_local_with_pml() {
        let mesh = unit_square_triangles(8);
        let pml = PmlRegion::x_positive(0.8, 0.2, 10.0, 5.0);
        let dirichlet = HashSet::new();

        let csr = assemble_local_pml_system(&mesh, PolynomialDegree::P1, 5.0, &[pml], &dirichlet);

        // PML should introduce complex entries
        let has_complex = csr.values.iter().any(|v| v.im.abs() > 1e-15);
        assert!(has_complex, "PML should introduce imaginary components");
    }

    #[test]
    fn test_homogeneous_dirichlet() {
        let mesh = unit_square_triangles(4);
        let mut dirichlet = HashSet::new();
        dirichlet.insert(0);
        dirichlet.insert(1);

        let csr = assemble_local_pml_system(&mesh, PolynomialDegree::P1, 1.0, &[], &dirichlet);

        // Dirichlet rows should have diagonal = 1 and all else zero
        for &node in &dirichlet {
            let row_start = csr.row_ptrs[node];
            let row_end = csr.row_ptrs[node + 1];
            for idx in row_start..row_end {
                if csr.col_indices[idx] == node {
                    assert!(
                        (csr.values[idx] - Complex64::new(1.0, 0.0)).norm() < 1e-12,
                        "Dirichlet diagonal should be 1"
                    );
                } else {
                    assert!(
                        csr.values[idx].norm() < 1e-12,
                        "Dirichlet off-diagonal should be 0"
                    );
                }
            }
        }
    }
}
