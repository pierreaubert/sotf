//! Efficient Helmholtz matrix assembler
//!
//! Provides a mechanism to pre-assemble the sparsity pattern of the Helmholtz system
//! and efficiently update the values for different frequencies (wavenumbers).
//!
//! This avoids rebuilding the CSR topology for every frequency step, which is
//! critical for performance in frequency sweeps.

use super::mass::{MassMatrix, assemble_mass};
use super::stiffness::{StiffnessMatrix, assemble_stiffness};
use crate::basis::PolynomialDegree;
use crate::mesh::Mesh;
use math_audio_solvers::CsrMatrix;
use num_complex::Complex64;
use rayon::prelude::*;
use std::collections::HashMap;

/// Efficient assembler for Helmholtz problems
pub struct HelmholtzAssembler {
    /// Number of rows (DOFs)
    pub num_rows: usize,
    /// CSR row pointers (shared by K, M, and boundaries)
    pub row_ptrs: Vec<usize>,
    /// CSR column indices (shared by K, M, and boundaries)
    pub col_indices: Vec<usize>,
    /// Stiffness matrix values (aligned with col_indices)
    pub k_values: Vec<f64>,
    /// Mass matrix values (aligned with col_indices)
    pub m_values: Vec<f64>,
    /// Boundary mass matrix values (Tag -> Values, aligned with col_indices)
    pub boundary_values: HashMap<usize, Vec<f64>>,
}

impl HelmholtzAssembler {
    /// Create a new assembler from mesh and polynomial degree
    pub fn new(mesh: &Mesh, degree: PolynomialDegree) -> Self {
        let stiffness = assemble_stiffness(mesh, degree);
        let mass = assemble_mass(mesh, degree);
        // Default: no boundary matrices
        Self::from_matrices(&stiffness, &mass, &[])
    }

    /// Create assembler from existing stiffness, mass, and boundary matrices
    pub fn from_matrices(
        stiffness: &StiffnessMatrix,
        mass: &MassMatrix,
        boundaries: &[(usize, MassMatrix)],
    ) -> Self {
        assert_eq!(stiffness.dim, mass.dim);
        let num_rows = stiffness.dim;

        // Collect all triplets from all matrices
        // We use a custom struct to identify source
        #[derive(Debug, Clone, Copy)]
        struct Entry {
            row: usize,
            col: usize,
            source_idx: i32, // -1 for K, -2 for M, >=0 for boundary tag
            value: f64,
        }

        // Estimate total capacity
        let total_nnz =
            stiffness.nnz() + mass.nnz() + boundaries.iter().map(|(_, m)| m.nnz()).sum::<usize>();

        let mut entries = Vec::with_capacity(total_nnz);

        // Stiffness (Source -1)
        for i in 0..stiffness.nnz() {
            entries.push(Entry {
                row: stiffness.rows[i],
                col: stiffness.cols[i],
                source_idx: -1,
                value: stiffness.values[i],
            });
        }

        // Mass (Source -2)
        for i in 0..mass.nnz() {
            entries.push(Entry {
                row: mass.rows[i],
                col: mass.cols[i],
                source_idx: -2,
                value: mass.values[i],
            });
        }

        // Boundaries (Source = tag)
        for (tag, matrix) in boundaries {
            for i in 0..matrix.nnz() {
                entries.push(Entry {
                    row: matrix.rows[i],
                    col: matrix.cols[i],
                    source_idx: *tag as i32,
                    value: matrix.values[i],
                });
            }
        }

        // Sort by row, then col
        entries.par_sort_unstable_by(|a, b| {
            if a.row != b.row {
                a.row.cmp(&b.row)
            } else {
                a.col.cmp(&b.col)
            }
        });

        // Merge duplicates and build CSR structure
        let mut row_ptrs = vec![0; num_rows + 1];
        let mut col_indices = Vec::with_capacity(entries.len());
        let mut k_values = Vec::with_capacity(entries.len());
        let mut m_values = Vec::with_capacity(entries.len());
        let mut boundary_values_map: HashMap<usize, Vec<f64>> = HashMap::new();

        // Initialize boundary value vectors
        for (tag, _) in boundaries {
            boundary_values_map.insert(*tag, Vec::with_capacity(entries.len()));
        }

        if entries.is_empty() {
            return Self {
                num_rows,
                row_ptrs,
                col_indices,
                k_values,
                m_values,
                boundary_values: boundary_values_map,
            };
        }

        let mut last_r = entries[0].row;
        let mut last_c = entries[0].col;

        // Accumulators
        let mut acc_k = 0.0;
        let mut acc_m = 0.0;
        let mut acc_boundaries: HashMap<usize, f64> = HashMap::new();

        // Helper to accumulate
        let accumulate =
            |entry: &Entry, k: &mut f64, m: &mut f64, b_map: &mut HashMap<usize, f64>| match entry
                .source_idx
            {
                -1 => *k += entry.value,
                -2 => *m += entry.value,
                tag => {
                    let t = tag as usize;
                    *b_map.entry(t).or_insert(0.0) += entry.value;
                }
            };

        accumulate(&entries[0], &mut acc_k, &mut acc_m, &mut acc_boundaries);

        // Fix row pointers for empty initial rows
        for r in 0..last_r {
            row_ptrs[r + 1] = 0;
        }

        for entry in entries.iter().skip(1) {
            if entry.row == last_r && entry.col == last_c {
                // Same position, accumulate
                accumulate(entry, &mut acc_k, &mut acc_m, &mut acc_boundaries);
            } else {
                // New position, push previous
                k_values.push(acc_k);
                m_values.push(acc_m);
                for (tag, vec) in boundary_values_map.iter_mut() {
                    vec.push(acc_boundaries.get(tag).copied().unwrap_or(0.0));
                }
                col_indices.push(last_c);

                // Update row pointers
                if entry.row != last_r {
                    row_ptrs[last_r + 1] = k_values.len();
                    for r in (last_r + 1)..entry.row {
                        row_ptrs[r + 1] = k_values.len();
                    }
                }

                // Start new accumulation
                last_r = entry.row;
                last_c = entry.col;
                acc_k = 0.0;
                acc_m = 0.0;
                acc_boundaries.clear();
                accumulate(entry, &mut acc_k, &mut acc_m, &mut acc_boundaries);
            }
        }

        // Push final entry
        k_values.push(acc_k);
        m_values.push(acc_m);
        for (tag, vec) in boundary_values_map.iter_mut() {
            vec.push(acc_boundaries.get(tag).copied().unwrap_or(0.0));
        }
        col_indices.push(last_c);

        // Finish row pointers
        row_ptrs[last_r + 1] = k_values.len();
        for r in (last_r + 1)..num_rows {
            row_ptrs[r + 1] = k_values.len();
        }

        Self {
            num_rows,
            row_ptrs,
            col_indices,
            k_values,
            m_values,
            boundary_values: boundary_values_map,
        }
    }

    /// Assemble Helmholtz matrix A = K - k²M + Σ c_i M_boundary_i
    pub fn assemble(
        &self,
        wavenumber: Complex64,
        boundary_coeffs: &HashMap<usize, Complex64>,
    ) -> CsrMatrix<Complex64> {
        let k_sq = wavenumber * wavenumber;

        // We use parallel iterator if we have enough elements
        // Since we have multiple vectors to zip, standard zip might be tedious.
        // We will index into vectors directly in parallel.

        let nnz = self.k_values.len();

        let values: Vec<Complex64> = (0..nnz)
            .into_par_iter()
            .map(|i| {
                let mut val = Complex64::new(self.k_values[i], 0.0)
                    - k_sq * Complex64::new(self.m_values[i], 0.0);

                // Add boundary terms
                if !self.boundary_values.is_empty() {
                    #[allow(clippy::collapsible_if)]
                    for (tag, coeffs) in boundary_coeffs {
                        if let Some(b_vals) = self.boundary_values.get(tag) {
                            if b_vals[i] != 0.0 {
                                val += coeffs * Complex64::new(b_vals[i], 0.0);
                            }
                        }
                    }
                }
                val
            })
            .collect();

        CsrMatrix::from_raw_parts(
            self.num_rows,
            self.num_rows, // Square matrix
            self.row_ptrs.clone(),
            self.col_indices.clone(),
            values,
        )
    }

    /// Estimate memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        let usize_size = std::mem::size_of::<usize>();
        let f64_size = std::mem::size_of::<f64>();

        let mut mem = self.row_ptrs.len() * usize_size
            + self.col_indices.len() * usize_size
            + self.k_values.len() * f64_size
            + self.m_values.len() * f64_size;

        for v in self.boundary_values.values() {
            mem += v.len() * f64_size;
        }
        mem
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_assembler_simple() {
        let mesh = unit_square_triangles(2);
        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);

        let k = Complex64::new(1.0, 0.0);
        let coeffs = HashMap::new();
        let matrix = assembler.assemble(k, &coeffs);

        assert_eq!(matrix.num_rows, mesh.num_nodes());
        assert!(matrix.nnz() > 0);
    }

    #[test]
    fn test_assembler_with_boundary() {
        use crate::assembly::mass::assemble_boundary_mass;

        let mut mesh = unit_square_triangles(2);
        mesh.detect_boundaries(); // sets marker 0 for all boundaries

        let stiffness = assemble_stiffness(&mesh, PolynomialDegree::P1);
        let mass = assemble_mass(&mesh, PolynomialDegree::P1);
        let b_mass = assemble_boundary_mass(&mesh, PolynomialDegree::P1, 0);

        let boundaries = vec![(0, b_mass)];
        let assembler = HelmholtzAssembler::from_matrices(&stiffness, &mass, &boundaries);

        let k = Complex64::new(1.0, 0.0);
        let mut coeffs = HashMap::new();
        coeffs.insert(0, Complex64::new(0.5, 0.0)); // Add 0.5 * BoundaryMass

        let matrix = assembler.assemble(k, &coeffs);

        // Check if values are different from base K-M
        let assembler_base = HelmholtzAssembler::from_matrices(&stiffness, &mass, &[]);
        let matrix_base = assembler_base.assemble(k, &HashMap::new());

        // Sum of values should differ
        let sum: Complex64 = matrix.values.iter().sum();
        let sum_base: Complex64 = matrix_base.values.iter().sum();

        assert!((sum - sum_base).norm() > 1e-10);
    }
}
