//! Stiffness matrix assembly
//!
//! Assembles the element stiffness matrix K where K_ij = ∫ ∇φ_i · ∇φ_j dΩ

use crate::basis::{Jacobian, PolynomialDegree, evaluate_shape};
use crate::mesh::{ElementType, Mesh};
use crate::quadrature::{QuadratureRule, for_stiffness};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Assembled stiffness matrix in triplet format
#[derive(Debug, Clone)]
pub struct StiffnessMatrix {
    /// Row indices
    pub rows: Vec<usize>,
    /// Column indices
    pub cols: Vec<usize>,
    /// Values (real for standard stiffness)
    pub values: Vec<f64>,
    /// Matrix dimension
    pub dim: usize,
}

impl StiffnessMatrix {
    pub fn new(dim: usize) -> Self {
        Self {
            rows: Vec::new(),
            cols: Vec::new(),
            values: Vec::new(),
            dim,
        }
    }

    /// Add a triplet (i, j, value)
    pub fn add(&mut self, i: usize, j: usize, value: f64) {
        self.rows.push(i);
        self.cols.push(j);
        self.values.push(value);
    }

    /// Number of non-zeros
    pub fn nnz(&self) -> usize {
        self.values.len()
    }
}

/// Compute element stiffness matrix for a 2D triangle (P1)
fn element_stiffness_triangle_p1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    // Get physical coordinates
    let coords: Vec<[f64; 2]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
        .collect();

    let mut k_local = vec![vec![0.0; n_nodes]; n_nodes];

    // Gauss quadrature
    for qp in quad.iter() {
        let shape = evaluate_shape(ElementType::Triangle, degree, qp.xi(), qp.eta(), 0.0);

        // Compute Jacobian
        let jac = Jacobian::from_2d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        // Transform gradients to physical space
        let grads_phys: Vec<Vec<f64>> = shape
            .gradients
            .iter()
            .map(|g| jac.transform_gradient(g))
            .collect();

        // Assemble: K_ij += (∇N_i · ∇N_j) * det(J) * weight
        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let dot: f64 = grads_phys[i]
                    .iter()
                    .zip(&grads_phys[j])
                    .map(|(a, b)| a * b)
                    .sum();
                k_local[i][j] += dot * det_j * qp.weight;
            }
        }
    }

    (vertices.to_vec(), k_local)
}

/// Compute element stiffness matrix for a 2D quadrilateral (Q1)
fn element_stiffness_quad_q1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 2]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
        .collect();

    let mut k_local = vec![vec![0.0; n_nodes]; n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(ElementType::Quadrilateral, degree, qp.xi(), qp.eta(), 0.0);
        let jac = Jacobian::from_2d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        let grads_phys: Vec<Vec<f64>> = shape
            .gradients
            .iter()
            .map(|g| jac.transform_gradient(g))
            .collect();

        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let dot: f64 = grads_phys[i]
                    .iter()
                    .zip(&grads_phys[j])
                    .map(|(a, b)| a * b)
                    .sum();
                k_local[i][j] += dot * det_j * qp.weight;
            }
        }
    }

    (vertices.to_vec(), k_local)
}

/// Compute element stiffness matrix for a 3D tetrahedron (P1)
fn element_stiffness_tet_p1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 3]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
        .collect();

    let mut k_local = vec![vec![0.0; n_nodes]; n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(
            ElementType::Tetrahedron,
            degree,
            qp.xi(),
            qp.eta(),
            qp.zeta(),
        );
        let jac = Jacobian::from_3d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        let grads_phys: Vec<Vec<f64>> = shape
            .gradients
            .iter()
            .map(|g| jac.transform_gradient(g))
            .collect();

        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let dot: f64 = grads_phys[i]
                    .iter()
                    .zip(&grads_phys[j])
                    .map(|(a, b)| a * b)
                    .sum();
                k_local[i][j] += dot * det_j * qp.weight;
            }
        }
    }

    (vertices.to_vec(), k_local)
}

/// Compute element stiffness matrix for a 3D hexahedron (Q1)
fn element_stiffness_hex_q1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 3]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
        .collect();

    let mut k_local = vec![vec![0.0; n_nodes]; n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(
            ElementType::Hexahedron,
            degree,
            qp.xi(),
            qp.eta(),
            qp.zeta(),
        );
        let jac = Jacobian::from_3d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        let grads_phys: Vec<Vec<f64>> = shape
            .gradients
            .iter()
            .map(|g| jac.transform_gradient(g))
            .collect();

        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let dot: f64 = grads_phys[i]
                    .iter()
                    .zip(&grads_phys[j])
                    .map(|(a, b)| a * b)
                    .sum();
                k_local[i][j] += dot * det_j * qp.weight;
            }
        }
    }

    (vertices.to_vec(), k_local)
}

/// Compute element stiffness contributions (returns triplets for one element)
fn compute_element_stiffness(
    mesh: &Mesh,
    elem_idx: usize,
    degree: PolynomialDegree,
) -> Vec<(usize, usize, f64)> {
    let elem_type = mesh.elements[elem_idx].element_type;
    let quad = for_stiffness(elem_type, degree.degree());

    let (global_nodes, k_local) = match elem_type {
        ElementType::Triangle => element_stiffness_triangle_p1(mesh, elem_idx, &quad, degree),
        ElementType::Quadrilateral => element_stiffness_quad_q1(mesh, elem_idx, &quad, degree),
        ElementType::Tetrahedron => element_stiffness_tet_p1(mesh, elem_idx, &quad, degree),
        ElementType::Hexahedron => element_stiffness_hex_q1(mesh, elem_idx, &quad, degree),
    };

    let mut triplets = Vec::new();
    for (i, &gi) in global_nodes.iter().enumerate() {
        for (j, &gj) in global_nodes.iter().enumerate() {
            if k_local[i][j].abs() > 1e-15 {
                triplets.push((gi, gj, k_local[i][j]));
            }
        }
    }
    triplets
}

/// Assemble global stiffness matrix from mesh
pub fn assemble_stiffness(mesh: &Mesh, degree: PolynomialDegree) -> StiffnessMatrix {
    #[cfg(feature = "parallel")]
    {
        assemble_stiffness_parallel(mesh, degree)
    }
    #[cfg(not(feature = "parallel"))]
    {
        assemble_stiffness_sequential(mesh, degree)
    }
}

/// Sequential stiffness assembly
#[cfg(not(feature = "parallel"))]
fn assemble_stiffness_sequential(mesh: &Mesh, degree: PolynomialDegree) -> StiffnessMatrix {
    let n_dofs = mesh.num_nodes();
    let mut matrix = StiffnessMatrix::new(n_dofs);

    for elem_idx in 0..mesh.num_elements() {
        for (gi, gj, val) in compute_element_stiffness(mesh, elem_idx, degree) {
            matrix.add(gi, gj, val);
        }
    }

    matrix
}

/// Parallel stiffness assembly using rayon
#[cfg(feature = "parallel")]
pub fn assemble_stiffness_parallel(mesh: &Mesh, degree: PolynomialDegree) -> StiffnessMatrix {
    let n_dofs = mesh.num_nodes();
    let n_elems = mesh.num_elements();

    // Compute all element contributions in parallel
    let all_triplets: Vec<Vec<(usize, usize, f64)>> = (0..n_elems)
        .into_par_iter()
        .map(|elem_idx| compute_element_stiffness(mesh, elem_idx, degree))
        .collect();

    // Merge all triplets into one matrix
    let total_triplets: usize = all_triplets.iter().map(|t| t.len()).sum();
    let mut matrix = StiffnessMatrix::new(n_dofs);
    matrix.rows.reserve(total_triplets);
    matrix.cols.reserve(total_triplets);
    matrix.values.reserve(total_triplets);

    for triplets in all_triplets {
        for (i, j, v) in triplets {
            matrix.add(i, j, v);
        }
    }

    matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_stiffness_assembly_triangle() {
        let mesh = unit_square_triangles(2);
        let stiffness = assemble_stiffness(&mesh, PolynomialDegree::P1);

        assert_eq!(stiffness.dim, mesh.num_nodes());
        assert!(stiffness.nnz() > 0);
    }

    #[test]
    fn test_stiffness_symmetry() {
        let mesh = unit_square_triangles(2);
        let stiffness = assemble_stiffness(&mesh, PolynomialDegree::P1);

        // Accumulate into dense matrix for testing
        let n = stiffness.dim;
        let mut dense = vec![vec![0.0; n]; n];

        for k in 0..stiffness.nnz() {
            dense[stiffness.rows[k]][stiffness.cols[k]] += stiffness.values[k];
        }

        // Check symmetry
        for i in 0..n {
            for j in 0..n {
                let diff = (dense[i][j] - dense[j][i]).abs();
                assert!(
                    diff < 1e-10,
                    "Asymmetric at ({}, {}): diff = {}",
                    i,
                    j,
                    diff
                );
            }
        }
    }
}
