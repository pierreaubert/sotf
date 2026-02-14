//! Mass matrix assembly
//!
//! Assembles the element mass matrix M where M_ij = ∫ φ_i · φ_j dΩ

use crate::basis::{Jacobian, PolynomialDegree, evaluate_shape};
use crate::mesh::{ElementType, Mesh};
use crate::quadrature::{QuadratureRule, for_mass};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Assembled mass matrix in triplet format
#[derive(Debug, Clone)]
pub struct MassMatrix {
    /// Row indices
    pub rows: Vec<usize>,
    /// Column indices
    pub cols: Vec<usize>,
    /// Values (real for standard mass)
    pub values: Vec<f64>,
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

    pub fn add(&mut self, i: usize, j: usize, value: f64) {
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
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 2]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
        .collect();

    let mut m_local = vec![vec![0.0; n_nodes]; n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(ElementType::Triangle, degree, qp.xi(), qp.eta(), 0.0);
        let jac = Jacobian::from_2d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        // Assemble: M_ij += N_i * N_j * det(J) * weight
        for (i, row) in m_local.iter_mut().enumerate() {
            for (j, m_ij) in row.iter_mut().enumerate() {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                *m_ij += val;
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
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 2]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
        .collect();

    let mut m_local = vec![vec![0.0; n_nodes]; n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(ElementType::Quadrilateral, degree, qp.xi(), qp.eta(), 0.0);
        let jac = Jacobian::from_2d(&shape.gradients, &coords);
        let det_j = jac.det.abs();

        for (i, row) in m_local.iter_mut().enumerate() {
            for (j, m_ij) in row.iter_mut().enumerate() {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                *m_ij += val;
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
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 3]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
        .collect();

    let mut m_local = vec![vec![0.0; n_nodes]; n_nodes];

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

        for (i, row) in m_local.iter_mut().enumerate() {
            for (j, m_ij) in row.iter_mut().enumerate() {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                *m_ij += val;
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
) -> (Vec<usize>, Vec<Vec<f64>>) {
    let elem = &mesh.elements[elem_idx];
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let coords: Vec<[f64; 3]> = vertices
        .iter()
        .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
        .collect();

    let mut m_local = vec![vec![0.0; n_nodes]; n_nodes];

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

        for (i, row) in m_local.iter_mut().enumerate() {
            for (j, m_ij) in row.iter_mut().enumerate() {
                let val = shape.values[i] * shape.values[j] * det_j * qp.weight;
                *m_ij += val;
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
) -> Vec<(usize, usize, f64)> {
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
            if m_local[i][j].abs() > 1e-15 {
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
    let all_triplets: Vec<Vec<(usize, usize, f64)>> = (0..n_elems)
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
pub fn assemble_lumped_mass(mesh: &Mesh, degree: PolynomialDegree) -> Vec<f64> {
    let mass = assemble_mass(mesh, degree);
    let mut lumped = vec![0.0; mass.dim];

    for k in 0..mass.nnz() {
        lumped[mass.rows[k]] += mass.values[k];
    }

    lumped
}

/// Assemble boundary mass matrix for a specific boundary marker
///
/// Assembles M_gamma = ∫_Γ φ_i φ_j dΓ for boundaries with the given marker
pub fn assemble_boundary_mass(mesh: &Mesh, _degree: PolynomialDegree, marker: i32) -> MassMatrix {
    let n_dofs = mesh.num_nodes();
    let mut matrix = MassMatrix::new(n_dofs);

    for boundary in &mesh.boundaries {
        if boundary.marker != marker {
            continue;
        }

        let nodes = &boundary.nodes;

        if nodes.len() == 3 {
            // Triangle face
            let p0 = &mesh.nodes[nodes[0]];
            let p1 = &mesh.nodes[nodes[1]];
            let p2 = &mesh.nodes[nodes[2]];

            // Compute area
            let v1 = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let v2 = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];

            // Cross product
            let cx = v1[1] * v2[2] - v1[2] * v2[1];
            let cy = v1[2] * v2[0] - v1[0] * v2[2];
            let cz = v1[0] * v2[1] - v1[1] * v2[0];

            let area = 0.5 * (cx * cx + cy * cy + cz * cz).sqrt();

            // P1 triangle mass matrix
            // M_loc = (Area / 12) * [2 1 1; 1 2 1; 1 1 2]
            let factor = area / 12.0;

            for i in 0..3 {
                for j in 0..3 {
                    let val = if i == j { 2.0 * factor } else { factor };
                    matrix.add(nodes[i], nodes[j], val);
                }
            }
        } else if nodes.len() == 2 {
            // Edge (2D mesh)
            let p0 = &mesh.nodes[nodes[0]];
            let p1 = &mesh.nodes[nodes[1]];

            let dx = p1.x - p0.x;
            let dy = p1.y - p0.y;
            let dz = p1.z - p0.z;
            let len = (dx * dx + dy * dy + dz * dz).sqrt();

            // P1 edge mass matrix
            // M_loc = (Length / 6) * [2 1; 1 2]
            let factor = len / 6.0;

            for i in 0..2 {
                for j in 0..2 {
                    let val = if i == j { 2.0 * factor } else { factor };
                    matrix.add(nodes[i], nodes[j], val);
                }
            }
        } else if nodes.len() == 4 {
            // Quad face - split into two triangles
            let p0 = &mesh.nodes[nodes[0]];
            let p1 = &mesh.nodes[nodes[1]];
            let p2 = &mesh.nodes[nodes[2]];
            let p3 = &mesh.nodes[nodes[3]];

            // Triangle 1: 0-1-2
            let v1 = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
            let v2 = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
            let cx1 = v1[1] * v2[2] - v1[2] * v2[1];
            let cy1 = v1[2] * v2[0] - v1[0] * v2[2];
            let cz1 = v1[0] * v2[1] - v1[1] * v2[0];
            let area1 = 0.5 * (cx1 * cx1 + cy1 * cy1 + cz1 * cz1).sqrt();
            let factor1 = area1 / 12.0;

            let t1 = [nodes[0], nodes[1], nodes[2]];
            for i in 0..3 {
                for j in 0..3 {
                    let val = if i == j { 2.0 * factor1 } else { factor1 };
                    matrix.add(t1[i], t1[j], val);
                }
            }

            // Triangle 2: 0-2-3
            let v3 = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
            let v4 = [p3.x - p0.x, p3.y - p0.y, p3.z - p0.z];
            let cx2 = v3[1] * v4[2] - v3[2] * v4[1];
            let cy2 = v3[2] * v4[0] - v3[0] * v4[2];
            let cz2 = v3[0] * v4[1] - v3[1] * v4[0];
            let area2 = 0.5 * (cx2 * cx2 + cy2 * cy2 + cz2 * cz2).sqrt();
            let factor2 = area2 / 12.0;

            let t2 = [nodes[0], nodes[2], nodes[3]];
            for i in 0..3 {
                for j in 0..3 {
                    let val = if i == j { 2.0 * factor2 } else { factor2 };
                    matrix.add(t2[i], t2[j], val);
                }
            }
        }
    }

    matrix
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
        let mut diag = vec![0.0; n];

        for k in 0..mass.nnz() {
            if mass.rows[k] == mass.cols[k] {
                diag[mass.rows[k]] += mass.values[k];
            }
        }

        for (i, &d) in diag.iter().enumerate() {
            assert!(d > 0.0, "Diagonal {} is not positive: {}", i, d);
        }
    }

    #[test]
    fn test_mass_integrates_to_area() {
        // For unit square, sum of all mass entries should equal 1 (area)
        let mesh = unit_square_triangles(4);
        let mass = assemble_mass(&mesh, PolynomialDegree::P1);

        let total: f64 = mass.values.iter().sum();
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
        let total: f64 = lumped.iter().sum();
        assert!(
            (total - 1.0).abs() < 1e-10,
            "Total lumped mass {} should be 1.0",
            total
        );
    }

    #[test]
    fn test_boundary_mass() {
        let mesh = unit_square_triangles(2);
        // Unit square: boundaries are length 4
        // assemble_boundary_mass assumes some markers are set.
        // We need to set them first? mesh.detect_boundaries sets everything to 0.
        // So assembling marker 0 should get full perimeter.

        let mut mesh = mesh;
        mesh.detect_boundaries();

        let b_mass = assemble_boundary_mass(&mesh, PolynomialDegree::P1, 0);

        // Sum of boundary mass should be perimeter length (4.0)
        let total: f64 = b_mass.values.iter().sum();
        assert!((total - 4.0).abs() < 1e-10, "Boundary mass sum: {}", total);
    }
}
