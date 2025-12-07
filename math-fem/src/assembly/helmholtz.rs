//! Helmholtz equation assembly
//!
//! Assembles the Helmholtz system matrix A = K - k²M where:
//! - K is the stiffness matrix (∇φ · ∇φ)
//! - M is the mass matrix (φ · φ)
//! - k is the wavenumber
//!
//! For time-harmonic acoustics: ∇²p + k²p = 0

use super::mass::{MassMatrix, assemble_mass};
use super::stiffness::{StiffnessMatrix, assemble_stiffness};
use crate::basis::PolynomialDegree;
use crate::mesh::Mesh;
use num_complex::Complex64;
use solvers::CsrMatrix;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Helmholtz system matrix in triplet format
#[derive(Debug, Clone)]
pub struct HelmholtzMatrix {
    /// Row indices
    pub rows: Vec<usize>,
    /// Column indices
    pub cols: Vec<usize>,
    /// Values (A = K - k²M)
    pub values: Vec<Complex64>,
    /// Matrix dimension
    pub dim: usize,
    /// Wavenumber k
    pub wavenumber: Complex64,
}

impl HelmholtzMatrix {
    /// Create Helmholtz matrix A = K - k²M
    pub fn new(stiffness: &StiffnessMatrix, mass: &MassMatrix, wavenumber: Complex64) -> Self {
        assert_eq!(stiffness.dim, mass.dim);
        let dim = stiffness.dim;
        let k_sq = wavenumber * wavenumber;

        // Combine entries
        let mut combined_nnz = stiffness.nnz() + mass.nnz();
        let mut rows = Vec::with_capacity(combined_nnz);
        let mut cols = Vec::with_capacity(combined_nnz);
        let mut values = Vec::with_capacity(combined_nnz);

        // Add stiffness entries (K)
        for i in 0..stiffness.nnz() {
            rows.push(stiffness.rows[i]);
            cols.push(stiffness.cols[i]);
            values.push(stiffness.values[i]);
        }

        // Subtract k²M entries (-k²M)
        for i in 0..mass.nnz() {
            rows.push(mass.rows[i]);
            cols.push(mass.cols[i]);
            values.push(-k_sq * mass.values[i]);
        }

        combined_nnz = values.len();
        let _ = combined_nnz; // suppress unused warning

        Self {
            rows,
            cols,
            values,
            dim,
            wavenumber,
        }
    }

    /// Number of non-zeros (may include duplicates)
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Convert to compressed (summing duplicate entries)
    pub fn to_compressed(&self) -> HelmholtzMatrix {
        use std::collections::HashMap;

        let mut entries: HashMap<(usize, usize), Complex64> = HashMap::new();

        for k in 0..self.nnz() {
            let key = (self.rows[k], self.cols[k]);
            *entries.entry(key).or_insert(Complex64::new(0.0, 0.0)) += self.values[k];
        }

        let mut rows = Vec::with_capacity(entries.len());
        let mut cols = Vec::with_capacity(entries.len());
        let mut values = Vec::with_capacity(entries.len());

        for ((r, c), v) in entries {
            if v.norm() > 1e-15 {
                rows.push(r);
                cols.push(c);
                values.push(v);
            }
        }

        HelmholtzMatrix {
            rows,
            cols,
            values,
            dim: self.dim,
            wavenumber: self.wavenumber,
        }
    }

    /// Convert to CSR sparse matrix format
    ///
    /// The triplets are converted to CSR format using `CsrMatrix::from_triplets`,
    /// which handles duplicate entries by summing them.
    pub fn to_csr(&self) -> CsrMatrix<Complex64> {
        let triplets: Vec<(usize, usize, Complex64)> = self
            .rows
            .iter()
            .zip(self.cols.iter())
            .zip(self.values.iter())
            .map(|((&r, &c), &v)| (r, c, v))
            .collect();

        CsrMatrix::from_triplets(self.dim, self.dim, triplets)
    }
}

/// Assembled Helmholtz problem including RHS
#[derive(Debug, Clone)]
pub struct HelmholtzProblem {
    /// System matrix A = K - k²M
    pub matrix: HelmholtzMatrix,
    /// Right-hand side vector
    pub rhs: Vec<Complex64>,
    /// Stiffness matrix (for reference)
    pub stiffness: StiffnessMatrix,
    /// Mass matrix (for reference)
    pub mass: MassMatrix,
}

impl HelmholtzProblem {
    /// Assemble Helmholtz problem from mesh
    ///
    /// # Arguments
    /// * `mesh` - The finite element mesh
    /// * `degree` - Polynomial degree for basis functions
    /// * `wavenumber` - Complex wavenumber k (can be complex for damping)
    /// * `source` - Source function f(x, y, z) for RHS
    pub fn assemble<F>(
        mesh: &Mesh,
        degree: PolynomialDegree,
        wavenumber: Complex64,
        source: F,
    ) -> Self
    where
        F: Fn(f64, f64, f64) -> Complex64 + Sync,
    {
        let stiffness = assemble_stiffness(mesh, degree);
        let mass = assemble_mass(mesh, degree);
        let matrix = HelmholtzMatrix::new(&stiffness, &mass, wavenumber);

        // Assemble RHS: b_i = ∫ f * φ_i dΩ
        let rhs = assemble_rhs(mesh, degree, &source);

        Self {
            matrix,
            rhs,
            stiffness,
            mass,
        }
    }

    /// Number of degrees of freedom
    pub fn num_dofs(&self) -> usize {
        self.matrix.dim
    }
}

/// Compute element RHS contributions (returns (node_index, value) pairs)
fn compute_element_rhs<F>(
    mesh: &Mesh,
    elem_idx: usize,
    degree: PolynomialDegree,
    source: &F,
) -> Vec<(usize, Complex64)>
where
    F: Fn(f64, f64, f64) -> Complex64,
{
    use crate::basis::{Jacobian, evaluate_shape};
    use crate::mesh::ElementType;
    use crate::quadrature::for_mass;

    let elem = &mesh.elements[elem_idx];
    let elem_type = elem.element_type;
    let vertices = elem.vertices();
    let n_nodes = vertices.len();

    let quad = for_mass(elem_type, degree.degree());

    // Get coordinates based on dimension
    let is_3d = matches!(
        elem_type,
        ElementType::Tetrahedron | ElementType::Hexahedron
    );

    let mut f_local = vec![Complex64::new(0.0, 0.0); n_nodes];

    for qp in quad.iter() {
        let shape = evaluate_shape(elem_type, degree, qp.xi(), qp.eta(), qp.zeta());

        // Map reference coordinates to physical coordinates
        let (x, y, z, det_j) = if is_3d {
            let coords: Vec<[f64; 3]> = vertices
                .iter()
                .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
                .collect();
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

            (x, y, z, jac.det.abs())
        } else {
            let coords: Vec<[f64; 2]> = vertices
                .iter()
                .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y])
                .collect();
            let jac = Jacobian::from_2d(&shape.gradients, &coords);

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

            (x, y, 0.0, jac.det.abs())
        };

        let f_val = source(x, y, z);

        for i in 0..n_nodes {
            f_local[i] += f_val * Complex64::new(shape.values[i] * det_j * qp.weight, 0.0);
        }
    }

    // Return (global_node_index, value) pairs
    vertices
        .iter()
        .enumerate()
        .map(|(i, &gi)| (gi, f_local[i]))
        .collect()
}

/// Assemble right-hand side vector
fn assemble_rhs<F>(mesh: &Mesh, degree: PolynomialDegree, source: &F) -> Vec<Complex64>
where
    F: Fn(f64, f64, f64) -> Complex64 + Sync,
{
    #[cfg(feature = "parallel")]
    {
        assemble_rhs_parallel(mesh, degree, source)
    }
    #[cfg(not(feature = "parallel"))]
    {
        assemble_rhs_sequential(mesh, degree, source)
    }
}

/// Sequential RHS assembly
#[cfg(not(feature = "parallel"))]
fn assemble_rhs_sequential<F>(mesh: &Mesh, degree: PolynomialDegree, source: &F) -> Vec<Complex64>
where
    F: Fn(f64, f64, f64) -> Complex64,
{
    let n_dofs = mesh.num_nodes();
    let mut rhs = vec![Complex64::new(0.0, 0.0); n_dofs];

    for elem_idx in 0..mesh.num_elements() {
        for (gi, val) in compute_element_rhs(mesh, elem_idx, degree, source) {
            rhs[gi] += val;
        }
    }

    rhs
}

/// Parallel RHS assembly using rayon
#[cfg(feature = "parallel")]
fn assemble_rhs_parallel<F>(mesh: &Mesh, degree: PolynomialDegree, source: &F) -> Vec<Complex64>
where
    F: Fn(f64, f64, f64) -> Complex64 + Sync,
{
    let n_dofs = mesh.num_nodes();
    let n_elems = mesh.num_elements();

    // Compute all element contributions in parallel
    let all_contribs: Vec<Vec<(usize, Complex64)>> = (0..n_elems)
        .into_par_iter()
        .map(|elem_idx| compute_element_rhs(mesh, elem_idx, degree, source))
        .collect();

    // Merge contributions into final RHS vector
    let mut rhs = vec![Complex64::new(0.0, 0.0); n_dofs];
    for contribs in all_contribs {
        for (gi, val) in contribs {
            rhs[gi] += val;
        }
    }

    rhs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_helmholtz_assembly() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        assert_eq!(problem.num_dofs(), mesh.num_nodes());
        assert!(problem.matrix.nnz() > 0);
    }

    #[test]
    fn test_helmholtz_k_zero_equals_laplacian() {
        // When k=0, Helmholtz reduces to Laplace: -∇²u = f
        use std::collections::HashMap;

        let mesh = unit_square_triangles(4);
        let k = Complex64::new(0.0, 0.0);

        let stiffness = assemble_stiffness(&mesh, PolynomialDegree::P1);
        let mass = assemble_mass(&mesh, PolynomialDegree::P1);
        let helmholtz = HelmholtzMatrix::new(&stiffness, &mass, k).to_compressed();

        // Compress stiffness for fair comparison
        let mut stiff_map: HashMap<(usize, usize), Complex64> = HashMap::new();
        for i in 0..stiffness.nnz() {
            *stiff_map
                .entry((stiffness.rows[i], stiffness.cols[i]))
                .or_insert(Complex64::new(0.0, 0.0)) += stiffness.values[i];
        }

        // Check that Helmholtz values match stiffness values
        for i in 0..helmholtz.nnz() {
            let key = (helmholtz.rows[i], helmholtz.cols[i]);
            let stiff_val = stiff_map
                .get(&key)
                .copied()
                .unwrap_or(Complex64::new(0.0, 0.0));
            let diff = (helmholtz.values[i] - stiff_val).norm();
            assert!(
                diff < 1e-10,
                "Mismatch at {:?}: {} vs {}",
                key,
                helmholtz.values[i],
                stiff_val
            );
        }
    }

    #[test]
    fn test_helmholtz_rhs() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        // Constant source f=1 should give non-zero RHS
        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        // Sum of RHS should be approximately 1 (integral of 1 over unit square)
        let rhs_sum: Complex64 = problem.rhs.iter().sum();
        assert!(
            (rhs_sum.re - 1.0).abs() < 1e-10,
            "RHS sum {} should be 1.0",
            rhs_sum
        );
    }

    #[test]
    fn test_helmholtz_complex_wavenumber() {
        // Complex wavenumber for damped Helmholtz
        let mesh = unit_square_triangles(2);
        let k = Complex64::new(1.0, 0.1); // Slightly damped

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        // Matrix should have complex entries
        let has_imag = problem.matrix.values.iter().any(|v| v.im.abs() > 1e-15);
        assert!(
            has_imag,
            "Complex wavenumber should produce complex matrix entries"
        );
    }
}
