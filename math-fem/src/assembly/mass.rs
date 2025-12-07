//! Mass matrix assembly
//!
//! Assembles the element mass matrix M where M_ij = ∫ φ_i · φ_j dΩ

use crate::basis::{Jacobian, PolynomialDegree, evaluate_shape};
use crate::mesh::{ElementType, Mesh};
use crate::quadrature::{QuadratureRule, for_mass};
use num_complex::Complex64;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Assembled mass matrix in triplet format
#[derive(Debug, Clone)]
pub struct MassMatrix {
    /// Row indices
    pub rows: Vec<usize>,
    /// Column indices
    pub cols: Vec<usize>,
    /// Values (complex for Helmholtz with PML)
    pub values: Vec<Complex64>,
    /// Matrix dimension
    pub dim: usize,
}

impl MassMatrix {
    pub fn new(dim: usize) -> Self {
        Self {
            rows: Vec::new(),
            cols: Vec::new(),
            values: Vec::new(),
            dim,
        }
    }

    pub fn add(&mut self, i: usize, j: usize, value: Complex64) {
        self.rows.push(i);
        self.cols.push(j);
        self.values.push(value);
    }

    pub fn nnz(&self) -> usize {
        self.values.len()
    }
}

/// Compute element mass matrix for a 2D triangle (P1)
fn element_mass_triangle_p1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<Complex64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 2]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
        .collect();

    let mut m_local = vec![vec![Complex64::new(0.0, 0.0); n_nodes]; n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(ElementType::Triangle, degree, qp.xi(), qp.eta(), 0.0);
        let jac = Jacobian::from_2d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        // Assemble: M_ij += N_i * N_j * det(J) * weight
        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                m_local[i][j] += Complex64::new(val, 0.0);
            }
        }
    }

    (vertices.to_vec(), m_local)
}

/// Compute element mass matrix for a 2D quadrilateral (Q1)
fn element_mass_quad_q1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<Complex64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 2]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
        .collect();

    let mut m_local = vec![vec![Complex64::new(0.0, 0.0); n_nodes]; n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(ElementType::Quadrilateral, degree, qp.xi(), qp.eta(), 0.0);
        let jac = Jacobian::from_2d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                m_local[i][j] += Complex64::new(val, 0.0);
            }
        }
    }

    (vertices.to_vec(), m_local)
}

/// Compute element mass matrix for a 3D tetrahedron (P1)
fn element_mass_tet_p1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<Complex64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 3]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
        .collect();

    let mut m_local = vec![vec![Complex64::new(0.0, 0.0); n_nodes]; n_nodes];

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

        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                m_local[i][j] += Complex64::new(val, 0.0);
            }
        }
    }

    (vertices.to_vec(), m_local)
}

/// Compute element mass matrix for a 3D hexahedron (Q1)
fn element_mass_hex_q1(
    mesh: &Mesh,
    elem_idx: usize,
    quad: &QuadratureRule,
    degree: PolynomialDegree,
) -> (Vec<usize>, Vec<Vec<Complex64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 3]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
        .collect();

    let mut m_local = vec![vec![Complex64::new(0.0, 0.0); n_nodes]; n_nodes];

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

        for i in 0..n_nodes {
            for j in 0..n_nodes {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                m_local[i][j] += Complex64::new(val, 0.0);
            }
        }
    }

    (vertices.to_vec(), m_local)
}

/// Compute element mass contributions (returns triplets for one element)
fn compute_element_mass(
    mesh: &Mesh,
    elem_idx: usize,
    degree: PolynomialDegree,
) -> Vec<(usize, usize, Complex64)> {
    let elem_type = mesh.elements[elem_idx].element_type;
    let quad = for_mass(elem_type, degree.degree());

    let (global_nodes, m_local) = match elem_type {
        ElementType::Triangle => element_mass_triangle_p1(mesh, elem_idx, &quad, degree),
        ElementType::Quadrilateral => element_mass_quad_q1(mesh, elem_idx, &quad, degree),
        ElementType::Tetrahedron => element_mass_tet_p1(mesh, elem_idx, &quad, degree),
        ElementType::Hexahedron => element_mass_hex_q1(mesh, elem_idx, &quad, degree),
    };

    let mut triplets = Vec::new();
    for (i, &gi) in global_nodes.iter().enumerate() {
        for (j, &gj) in global_nodes.iter().enumerate() {
            if m_local[i][j].norm() > 1e-15 {
                triplets.push((gi, gj, m_local[i][j]));
            }
        }
    }
    triplets
}

/// Assemble global mass matrix from mesh
pub fn assemble_mass(mesh: &Mesh, degree: PolynomialDegree) -> MassMatrix {
    #[cfg(feature = "parallel")]
    {
        assemble_mass_parallel(mesh, degree)
    }
    #[cfg(not(feature = "parallel"))]
    {
        assemble_mass_sequential(mesh, degree)
    }
}

/// Sequential mass assembly
#[cfg(not(feature = "parallel"))]
fn assemble_mass_sequential(mesh: &Mesh, degree: PolynomialDegree) -> MassMatrix {
    let n_dofs = mesh.num_nodes();
    let mut matrix = MassMatrix::new(n_dofs);

    for elem_idx in 0..mesh.num_elements() {
        for (gi, gj, val) in compute_element_mass(mesh, elem_idx, degree) {
            matrix.add(gi, gj, val);
        }
    }

    matrix
}

/// Parallel mass assembly using rayon
#[cfg(feature = "parallel")]
pub fn assemble_mass_parallel(mesh: &Mesh, degree: PolynomialDegree) -> MassMatrix {
    let n_dofs = mesh.num_nodes();
    let n_elems = mesh.num_elements();

    // Compute all element contributions in parallel
    let all_triplets: Vec<Vec<(usize, usize, Complex64)>> = (0..n_elems)
        .into_par_iter()
        .map(|elem_idx| compute_element_mass(mesh, elem_idx, degree))
        .collect();

    // Merge all triplets into one matrix
    let total_triplets: usize = all_triplets.iter().map(|t| t.len()).sum();
    let mut matrix = MassMatrix::new(n_dofs);
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

/// Compute lumped mass matrix (row-sum lumping)
pub fn assemble_lumped_mass(mesh: &Mesh, degree: PolynomialDegree) -> Vec<Complex64> {
    let mass = assemble_mass(mesh, degree);
    let mut lumped = vec![Complex64::new(0.0, 0.0); mass.dim];

    for k in 0..mass.nnz() {
        lumped[mass.rows[k]] += mass.values[k];
    }

    lumped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_mass_assembly_triangle() {
        let mesh = unit_square_triangles(2);
        let mass = assemble_mass(&mesh, PolynomialDegree::P1);

        assert_eq!(mass.dim, mesh.num_nodes());
        assert!(mass.nnz() > 0);
    }

    #[test]
    fn test_mass_positivity() {
        let mesh = unit_square_triangles(2);
        let mass = assemble_mass(&mesh, PolynomialDegree::P1);

        // All diagonal entries should be positive
        let n = mass.dim;
        let mut diag = vec![Complex64::new(0.0, 0.0); n];

        for k in 0..mass.nnz() {
            if mass.rows[k] == mass.cols[k] {
                diag[mass.rows[k]] += mass.values[k];
            }
        }

        for (i, &d) in diag.iter().enumerate() {
            assert!(d.re > 0.0, "Diagonal {} is not positive: {}", i, d);
        }
    }

    #[test]
    fn test_mass_integrates_to_area() {
        // For unit square, sum of all mass entries should equal 1 (area)
        let mesh = unit_square_triangles(4);
        let mass = assemble_mass(&mesh, PolynomialDegree::P1);

        let total: f64 = mass.values.iter().map(|v| v.re).sum();
        assert!(
            (total - 1.0).abs() < 1e-10,
            "Total mass {} should be 1.0",
            total
        );
    }

    #[test]
    fn test_lumped_mass() {
        let mesh = unit_square_triangles(4);
        let lumped = assemble_lumped_mass(&mesh, PolynomialDegree::P1);

        assert_eq!(lumped.len(), mesh.num_nodes());

        // Sum should still be area
        let total: f64 = lumped.iter().map(|v| v.re).sum();
        assert!(
            (total - 1.0).abs() < 1e-10,
            "Total lumped mass {} should be 1.0",
            total
        );
    }
}
