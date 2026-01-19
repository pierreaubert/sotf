//! Mesh hierarchy for geometric multigrid
//!
//! Manages a sequence of meshes from fine to coarse for multigrid methods.

use crate::assembly::{
    HelmholtzMatrix, MassMatrix, StiffnessMatrix, assemble_mass, assemble_stiffness,
};
use crate::basis::PolynomialDegree;
use crate::mesh::Mesh;
use num_complex::Complex64;
use std::collections::HashMap;

/// A level in the multigrid hierarchy
#[derive(Debug)]
pub struct MultigridLevel {
    /// Mesh at this level
    pub mesh: Mesh,
    /// Number of degrees of freedom
    pub n_dofs: usize,
    /// Stiffness matrix (optional, assembled on demand)
    pub stiffness: Option<StiffnessMatrix>,
    /// Mass matrix (optional)
    pub mass: Option<MassMatrix>,
    /// System matrix (K - k²M)
    pub system: Option<HelmholtzMatrix>,
    /// Prolongation operator to next finer level (None for finest)
    pub prolongation: Option<TransferMatrix>,
    /// Restriction operator to next coarser level (None for coarsest)
    pub restriction: Option<TransferMatrix>,
}

/// Transfer matrix (prolongation or restriction)
#[derive(Debug, Clone)]
pub struct TransferMatrix {
    /// Row indices
    pub rows: Vec<usize>,
    /// Column indices
    pub cols: Vec<usize>,
    /// Values
    pub values: Vec<f64>,
    /// Number of rows (fine DOFs for prolongation, coarse for restriction)
    pub n_rows: usize,
    /// Number of columns
    pub n_cols: usize,
}

impl TransferMatrix {
    pub fn new(n_rows: usize, n_cols: usize) -> Self {
        Self {
            rows: Vec::new(),
            cols: Vec::new(),
            values: Vec::new(),
            n_rows,
            n_cols,
        }
    }

    /// Add entry (i, j, v)
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        self.rows.push(i);
        self.cols.push(j);
        self.values.push(v);
    }

    /// Apply y = A * x (matrix-vector product)
    pub fn apply(&self, x: &[Complex64]) -> Vec<Complex64> {
        let mut y = vec![Complex64::new(0.0, 0.0); self.n_rows];
        for k in 0..self.rows.len() {
            y[self.rows[k]] += Complex64::new(self.values[k], 0.0) * x[self.cols[k]];
        }
        y
    }

    /// Apply y = A^T * x (transpose)
    pub fn apply_transpose(&self, x: &[Complex64]) -> Vec<Complex64> {
        let mut y = vec![Complex64::new(0.0, 0.0); self.n_cols];
        for k in 0..self.rows.len() {
            y[self.cols[k]] += Complex64::new(self.values[k], 0.0) * x[self.rows[k]];
        }
        y
    }
}

/// Multigrid mesh hierarchy
pub struct MultigridHierarchy {
    /// Levels from finest (0) to coarsest (n-1)
    pub levels: Vec<MultigridLevel>,
    /// Polynomial degree for FEM
    pub degree: PolynomialDegree,
    /// Wavenumber for Helmholtz
    pub wavenumber: Complex64,
}

impl MultigridHierarchy {
    /// Create hierarchy from a sequence of meshes (finest first)
    pub fn from_meshes(meshes: Vec<Mesh>, degree: PolynomialDegree, wavenumber: Complex64) -> Self {
        let n_levels = meshes.len();
        let mut levels = Vec::with_capacity(n_levels);

        for mesh in meshes {
            let n_dofs = mesh.num_nodes();
            levels.push(MultigridLevel {
                mesh,
                n_dofs,
                stiffness: None,
                mass: None,
                system: None,
                prolongation: None,
                restriction: None,
            });
        }

        // Build transfer operators between levels
        for i in 0..n_levels.saturating_sub(1) {
            let (p, r) = build_transfer_operators(&levels[i].mesh, &levels[i + 1].mesh);
            levels[i].restriction = Some(r);
            levels[i + 1].prolongation = Some(p);
        }

        Self {
            levels,
            degree,
            wavenumber,
        }
    }

    /// Create hierarchy by successive coarsening
    pub fn from_fine_mesh(
        fine_mesh: Mesh,
        n_levels: usize,
        degree: PolynomialDegree,
        wavenumber: Complex64,
    ) -> Self {
        let mut meshes = vec![fine_mesh];

        // Generate coarser meshes by selecting every other node
        // This is a simplified coarsening - real implementations use graph coarsening
        for _ in 1..n_levels {
            let coarse = coarsen_mesh(meshes.last().unwrap());
            if coarse.num_nodes() < 4 {
                break; // Too coarse
            }
            meshes.push(coarse);
        }

        Self::from_meshes(meshes, degree, wavenumber)
    }

    /// Number of levels
    pub fn num_levels(&self) -> usize {
        self.levels.len()
    }

    /// Get finest level
    pub fn finest(&self) -> &MultigridLevel {
        &self.levels[0]
    }

    /// Get coarsest level
    pub fn coarsest(&self) -> &MultigridLevel {
        &self.levels[self.levels.len() - 1]
    }

    /// Assemble matrices at all levels
    pub fn assemble_all(&mut self) {
        for level in &mut self.levels {
            let stiffness = assemble_stiffness(&level.mesh, self.degree);
            let mass = assemble_mass(&level.mesh, self.degree);
            let system = HelmholtzMatrix::new(&stiffness, &mass, self.wavenumber);

            level.stiffness = Some(stiffness);
            level.mass = Some(mass);
            level.system = Some(system);
        }
    }

    /// Assemble matrix at specific level
    pub fn assemble_level(&mut self, level_idx: usize) {
        let level = &mut self.levels[level_idx];
        let stiffness = assemble_stiffness(&level.mesh, self.degree);
        let mass = assemble_mass(&level.mesh, self.degree);
        let system = HelmholtzMatrix::new(&stiffness, &mass, self.wavenumber);

        level.stiffness = Some(stiffness);
        level.mass = Some(mass);
        level.system = Some(system);
    }
}

/// Build prolongation and restriction operators between two mesh levels
fn build_transfer_operators(
    fine_mesh: &Mesh,
    coarse_mesh: &Mesh,
) -> (TransferMatrix, TransferMatrix) {
    let n_fine = fine_mesh.num_nodes();
    let n_coarse = coarse_mesh.num_nodes();

    let mut prolongation = TransferMatrix::new(n_fine, n_coarse);
    let mut restriction = TransferMatrix::new(n_coarse, n_fine);

    // Build node mapping from coarse to fine
    // For geometric multigrid, coarse nodes are a subset of fine nodes
    let mut coarse_to_fine: HashMap<usize, usize> = HashMap::new();

    // Find matching nodes by position
    let tol = 1e-10;
    for (ci, cp) in coarse_mesh.nodes.iter().enumerate() {
        for (fi, fp) in fine_mesh.nodes.iter().enumerate() {
            let dist =
                ((cp.x - fp.x).powi(2) + (cp.y - fp.y).powi(2) + (cp.z - fp.z).powi(2)).sqrt();
            if dist < tol {
                coarse_to_fine.insert(ci, fi);
                break;
            }
        }
    }

    // Prolongation: inject coarse values to fine, interpolate others
    // Simplified: only inject for matching nodes
    for (ci, &fi) in &coarse_to_fine {
        prolongation.add(fi, *ci, 1.0);
    }

    // For non-matching fine nodes, find nearest coarse nodes and interpolate
    // This is a simplified linear interpolation
    for fi in 0..n_fine {
        let is_coarse = coarse_to_fine.values().any(|&v| v == fi);
        if !is_coarse {
            // Find nearest coarse nodes
            let fp = &fine_mesh.nodes[fi];
            let mut distances: Vec<(usize, f64)> = coarse_mesh
                .nodes
                .iter()
                .enumerate()
                .map(|(ci, cp)| {
                    let d = ((cp.x - fp.x).powi(2) + (cp.y - fp.y).powi(2) + (cp.z - fp.z).powi(2))
                        .sqrt();
                    (ci, d)
                })
                .collect();
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            // Use inverse distance weighting for nearest neighbors
            let n_neighbors = distances.len().min(3);
            let total_inv_dist: f64 = distances[..n_neighbors]
                .iter()
                .map(|(_, d)| if *d > 1e-15 { 1.0 / d } else { 1e15 })
                .sum();

            for (ci, d) in distances.iter().take(n_neighbors) {
                let weight = if *d > 1e-15 { 1.0 / d } else { 1e15 };
                prolongation.add(fi, *ci, weight / total_inv_dist);
            }
        }
    }

    // Restriction: transpose of prolongation scaled by 2 (for standard scaling)
    // R = P^T (or scaled version)
    for k in 0..prolongation.rows.len() {
        restriction.add(
            prolongation.cols[k],
            prolongation.rows[k],
            prolongation.values[k],
        );
    }

    (prolongation, restriction)
}

/// Create a coarser mesh by skipping nodes
/// This is a very simplified coarsening for demonstration
fn coarsen_mesh(fine_mesh: &Mesh) -> Mesh {
    use crate::mesh::{unit_cube_tetrahedra, unit_square_triangles};

    // For structured meshes, create a coarser version
    // This is a placeholder - real implementations use graph coarsening
    let n_fine = fine_mesh.num_nodes();
    let n_coarse = ((n_fine as f64).sqrt() / 2.0).ceil() as usize;

    if fine_mesh.dimension == 2 {
        unit_square_triangles(n_coarse.max(1))
    } else {
        unit_cube_tetrahedra(n_coarse.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_hierarchy_creation() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let hierarchy = MultigridHierarchy::from_fine_mesh(mesh, 3, PolynomialDegree::P1, k);

        assert!(hierarchy.num_levels() >= 2);
        assert!(hierarchy.finest().n_dofs > hierarchy.coarsest().n_dofs);
    }

    #[test]
    fn test_transfer_matrix_apply() {
        let mut p = TransferMatrix::new(4, 2);
        p.add(0, 0, 1.0);
        p.add(1, 0, 0.5);
        p.add(1, 1, 0.5);
        p.add(2, 1, 1.0);
        p.add(3, 0, 0.5);
        p.add(3, 1, 0.5);

        let x = vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)];
        let y = p.apply(&x);

        assert_eq!(y.len(), 4);
        assert!((y[0].re - 1.0).abs() < 1e-10); // 1*1
        assert!((y[1].re - 1.5).abs() < 1e-10); // 0.5*1 + 0.5*2
        assert!((y[2].re - 2.0).abs() < 1e-10); // 1*2
    }
}
