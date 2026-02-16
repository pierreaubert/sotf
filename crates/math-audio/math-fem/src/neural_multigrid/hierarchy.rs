//! Wave-specific multigrid hierarchy for the Neural Multigrid solver
//!
//! Extends the standard multigrid hierarchy with wave-specific information:
//! - kh values at each level (product of wavenumber and mesh size)
//! - Identification of the ADR correction level (kh ~ 1)
//! - Per-level Chebyshev alpha parameters
//! - Re-discretized Helmholtz operators at each level
//!
//! Reference: Cui & Jiang (2024, arXiv:2404.02493), Section 2

use crate::basis::PolynomialDegree;
use crate::mesh::Mesh;
use crate::multigrid::{MultigridHierarchy, TransferMatrix};
use math_audio_solvers::CsrMatrix;
use num_complex::Complex64;

use super::chebyshev::estimate_alpha;

/// Wave-specific multigrid hierarchy
pub struct WaveHierarchy {
    /// Underlying multigrid hierarchy (meshes, transfer operators)
    pub hierarchy: MultigridHierarchy,
    /// Helmholtz CSR matrices at each level (re-discretized)
    pub operators: Vec<CsrMatrix<Complex64>>,
    /// Stiffness CSR matrices at each level (for ADR assembly)
    pub stiffness_csrs: Vec<CsrMatrix<Complex64>>,
    /// Mass CSR matrices at each level (for ADR assembly)
    pub mass_csrs: Vec<CsrMatrix<Complex64>>,
    /// kh values at each level
    pub kh_values: Vec<f64>,
    /// Mesh size h at each level
    pub h_values: Vec<f64>,
    /// Per-level Chebyshev alpha parameters
    pub alpha_values: Vec<f64>,
    /// Index of the ADR correction level (where kh ~ 1)
    pub adr_level: usize,
    /// Wavenumber k (real part)
    pub wavenumber: f64,
    /// Angular frequency omega
    pub omega: f64,
    /// Damping parameter gamma for the damped Helmholtz
    pub gamma: f64,
}

impl WaveHierarchy {
    /// Build a wave hierarchy from a fine mesh
    ///
    /// Determines the number of levels from k and h such that the coarsest
    /// level has kh ~ pi (about 2 points per wavelength).
    ///
    /// # Arguments
    /// * `mesh` - The fine mesh
    /// * `degree` - Polynomial degree
    /// * `k` - Real wavenumber
    /// * `gamma` - Damping parameter
    pub fn build(mesh: Mesh, degree: PolynomialDegree, k: f64, gamma: f64) -> Self {
        let h_fine = super::eikonal::mesh_size(&mesh);
        let kh_fine = k * h_fine;

        // Determine number of levels: coarsen until kh ~ pi
        // Each coarsening doubles h, so kh doubles
        let n_levels = if kh_fine > 0.01 {
            let levels_needed = ((std::f64::consts::PI / kh_fine).log2().ceil() as usize).max(2);
            levels_needed.min(8) // Cap at 8 levels
        } else {
            4 // Default for very fine meshes
        };

        // Build standard multigrid hierarchy
        let wavenumber_complex = Complex64::new(k, 0.0);
        let mut hierarchy =
            MultigridHierarchy::from_fine_mesh(mesh, n_levels, degree, wavenumber_complex);
        let actual_levels = hierarchy.num_levels();

        // Assemble matrices at all levels
        hierarchy.assemble_all();

        // Compute per-level information
        let mut operators = Vec::with_capacity(actual_levels);
        let mut stiffness_csrs = Vec::with_capacity(actual_levels);
        let mut mass_csrs = Vec::with_capacity(actual_levels);
        let mut kh_values = Vec::with_capacity(actual_levels);
        let mut h_values = Vec::with_capacity(actual_levels);
        let mut alpha_values = Vec::with_capacity(actual_levels);

        for (level_idx, level) in hierarchy.levels.iter().enumerate() {
            let h = super::eikonal::mesh_size(&level.mesh);
            let kh = k * h;

            // Build CSR operator for this level
            let system = level.system.as_ref().expect("System not assembled");
            let csr = system.to_csr();
            operators.push(csr);

            // Build stiffness and mass CSRs
            let stiffness = level.stiffness.as_ref().expect("Stiffness not assembled");
            let mass = level.mass.as_ref().expect("Mass not assembled");
            stiffness_csrs.push(real_to_complex_csr(stiffness));
            mass_csrs.push(real_to_complex_csr_mass(mass));

            // Chebyshev parameter
            let alpha = estimate_alpha(k, h, level_idx);
            alpha_values.push(alpha);

            kh_values.push(kh);
            h_values.push(h);
        }

        // Find ADR correction level: the level where kh is closest to 1
        let adr_level = kh_values
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (**a - 1.0).abs();
                let db = (**b - 1.0).abs();
                da.partial_cmp(&db).unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(1)
            .max(1); // Never use finest level for ADR

        WaveHierarchy {
            hierarchy,
            operators,
            stiffness_csrs,
            mass_csrs,
            kh_values,
            h_values,
            alpha_values,
            adr_level,
            wavenumber: k,
            omega: k, // For unit speed, omega = k
            gamma,
        }
    }

    /// Number of levels in the hierarchy
    pub fn num_levels(&self) -> usize {
        self.hierarchy.num_levels()
    }

    /// Get the restriction operator from level to level+1 (fine to coarse)
    pub fn restriction(&self, level: usize) -> Option<&TransferMatrix> {
        self.hierarchy.levels[level].restriction.as_ref()
    }

    /// Get the prolongation operator from level+1 to level (coarse to fine)
    pub fn prolongation(&self, level: usize) -> Option<&TransferMatrix> {
        if level + 1 < self.num_levels() {
            self.hierarchy.levels[level + 1].prolongation.as_ref()
        } else {
            None
        }
    }

    /// Number of DOFs at a given level
    pub fn n_dofs(&self, level: usize) -> usize {
        self.hierarchy.levels[level].n_dofs
    }

    /// Get mesh at a given level
    pub fn mesh(&self, level: usize) -> &Mesh {
        &self.hierarchy.levels[level].mesh
    }
}

/// Convert a real StiffnessMatrix (triplet format) to Complex64 CSR
fn real_to_complex_csr(stiffness: &crate::assembly::StiffnessMatrix) -> CsrMatrix<Complex64> {
    let triplets: Vec<(usize, usize, Complex64)> = stiffness
        .rows
        .iter()
        .zip(stiffness.cols.iter())
        .zip(stiffness.values.iter())
        .map(|((&r, &c), &v)| (r, c, Complex64::new(v, 0.0)))
        .collect();

    CsrMatrix::from_triplets(stiffness.dim, stiffness.dim, triplets)
}

/// Convert a real MassMatrix (triplet format) to Complex64 CSR
fn real_to_complex_csr_mass(mass: &crate::assembly::MassMatrix) -> CsrMatrix<Complex64> {
    let triplets: Vec<(usize, usize, Complex64)> = mass
        .rows
        .iter()
        .zip(mass.cols.iter())
        .zip(mass.values.iter())
        .map(|((&r, &c), &v)| (r, c, Complex64::new(v, 0.0)))
        .collect();

    CsrMatrix::from_triplets(mass.dim, mass.dim, triplets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_wave_hierarchy_build() {
        let mesh = unit_square_triangles(8);
        let k = 5.0;
        let gamma = 0.5;

        let wh = WaveHierarchy::build(mesh, PolynomialDegree::P1, k, gamma);

        assert!(wh.num_levels() >= 2);
        assert!(wh.adr_level >= 1);
        assert!(wh.adr_level < wh.num_levels());

        // kh should increase with coarsening
        for i in 1..wh.num_levels() {
            assert!(
                wh.kh_values[i] >= wh.kh_values[i - 1] * 0.9,
                "kh should generally increase: level {} kh={}, level {} kh={}",
                i - 1,
                wh.kh_values[i - 1],
                i,
                wh.kh_values[i]
            );
        }
    }

    #[test]
    fn test_wave_hierarchy_operators() {
        let mesh = unit_square_triangles(4);
        let k = 2.0;
        let gamma = 0.5;

        let wh = WaveHierarchy::build(mesh, PolynomialDegree::P1, k, gamma);

        // All operators should be square and non-empty
        for (i, op) in wh.operators.iter().enumerate() {
            assert_eq!(op.num_rows, op.num_cols);
            assert!(op.nnz() > 0, "Level {} operator should have entries", i);
            assert_eq!(op.num_rows, wh.n_dofs(i));
        }
    }

    #[test]
    fn test_wave_hierarchy_adr_level() {
        let mesh = unit_square_triangles(16);
        let k = 10.0;
        let gamma = 0.5;

        let wh = WaveHierarchy::build(mesh, PolynomialDegree::P1, k, gamma);

        // ADR level should have kh near 1
        assert!(
            wh.kh_values[wh.adr_level] < 3.0,
            "ADR level kh should be moderate, got {}",
            wh.kh_values[wh.adr_level]
        );
    }
}
