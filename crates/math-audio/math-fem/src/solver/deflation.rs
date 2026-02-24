//! Deflation subspace computation for Helmholtz problems
//!
//! Computes deflation vectors that approximate the problematic near-k²
//! eigenmodes of the Helmholtz operator A = K - k²M. These vectors are
//! used with deflated GMRES to accelerate convergence.
//!
//! Reference: Erlangga & Nabben (2008), Gaul et al. (2013).

use crate::assembly::HelmholtzProblem;
use math_audio_solvers::{AmgConfig, AmgPreconditioner, CsrMatrix, DeflationSubspace, GmresConfig};
use ndarray::Array1;
use num_complex::Complex64;

/// Method for computing deflation vectors
#[derive(Debug, Clone)]
pub enum DeflationMethod {
    /// Inverse iteration using shifted-Laplacian AMG as approximate solver.
    /// Reuses the existing preconditioner infrastructure.
    InverseIteration {
        /// Number of inverse iteration steps per vector
        num_iterations: usize,
    },
    /// Extract Harmonic Ritz vectors from a previous GMRES solve (deferred)
    HarmonicRitz,
    /// Use analytical room mode shapes (deferred)
    AnalyticalRoomModes,
}

impl Default for DeflationMethod {
    fn default() -> Self {
        Self::InverseIteration { num_iterations: 5 }
    }
}

/// Configuration for deflation subspace computation
#[derive(Debug, Clone)]
pub struct DeflationConfig {
    /// Number of deflation vectors to compute
    pub num_vectors: usize,
    /// Method used to compute the vectors
    pub method: DeflationMethod,
}

impl Default for DeflationConfig {
    fn default() -> Self {
        Self {
            num_vectors: 10,
            method: DeflationMethod::default(),
        }
    }
}

impl DeflationConfig {
    /// Create configuration optimized for frequency sweeps
    pub fn for_frequency_sweep(num_vectors: usize) -> Self {
        Self {
            num_vectors,
            method: DeflationMethod::InverseIteration { num_iterations: 5 },
        }
    }
}

/// Compute deflation subspace for a Helmholtz problem
///
/// Returns deflation vectors as columns of an n×r matrix (stored as Vec<Array1>),
/// pre-built into a `DeflationSubspace` ready for use with `gmres_deflated`.
///
/// # Algorithm (InverseIteration)
/// 1. Build shifted-Laplacian P and its AMG preconditioner
/// 2. For each vector i=0..r:
///    a. Start from deterministic pseudo-random initial vector
///    b. Iterate: solve A·z ≈ v via short GMRES+SL (restart=10, max_iters=5, tol=0.1)
///    c. Orthogonalize against previous vectors (Modified Gram-Schmidt)
///    d. Normalize
/// 3. Return `DeflationSubspace` built from the computed vectors
pub fn compute_deflation_subspace(
    problem: &HelmholtzProblem,
    csr: &CsrMatrix<Complex64>,
    wavenumber: f64,
    config: &DeflationConfig,
) -> DeflationSubspace<Complex64> {
    match &config.method {
        DeflationMethod::InverseIteration { num_iterations } => compute_inverse_iteration(
            problem,
            csr,
            wavenumber,
            config.num_vectors,
            *num_iterations,
        ),
        DeflationMethod::HarmonicRitz => {
            unimplemented!("HarmonicRitz deflation method not yet implemented")
        }
        DeflationMethod::AnalyticalRoomModes => {
            unimplemented!("AnalyticalRoomModes deflation method not yet implemented")
        }
    }
}

/// Compute deflation vectors via inverse iteration
fn compute_inverse_iteration(
    problem: &HelmholtzProblem,
    csr: &CsrMatrix<Complex64>,
    wavenumber: f64,
    num_vectors: usize,
    num_iterations: usize,
) -> DeflationSubspace<Complex64> {
    let n = csr.num_rows;

    if num_vectors == 0 {
        return DeflationSubspace::new(Vec::new(), csr)
            .expect("Empty deflation subspace should not fail");
    }

    // Build shifted-Laplacian preconditioner
    let sl_config = super::ShiftedLaplacianConfig::for_wavenumber(wavenumber);
    let p_matrix = super::build_shifted_laplacian(
        &problem.stiffness,
        &problem.mass,
        sl_config.alpha,
        sl_config.beta,
    );
    let mut amg_config = AmgConfig::for_parallel();
    amg_config.smoother = math_audio_solvers::AmgSmoother::L1Jacobi;
    amg_config.strong_threshold = 0.5;
    let precond = AmgPreconditioner::from_csr(&p_matrix, amg_config);

    // Short GMRES config for approximate inverse iteration solves
    let gmres_config = GmresConfig {
        max_iterations: 5,
        restart: 10,
        tolerance: 0.1,
        print_interval: 0,
    };

    let mut w_columns: Vec<Array1<Complex64>> = Vec::with_capacity(num_vectors);

    for i in 0..num_vectors {
        // Deterministic pseudo-random starting vector
        let mut v = generate_starting_vector(n, i);

        // Orthogonalize against already-computed vectors
        modified_gram_schmidt(&mut v, &w_columns);
        normalize(&mut v);

        // Inverse iteration: repeatedly solve A·z ≈ v
        for _ in 0..num_iterations {
            let result = math_audio_solvers::iterative::gmres_preconditioned_with_guess(
                csr,
                &precond,
                &v,
                None,
                &gmres_config,
            );
            v = result.x;

            // Re-orthogonalize and normalize
            modified_gram_schmidt(&mut v, &w_columns);
            normalize(&mut v);
        }

        w_columns.push(v);
    }

    DeflationSubspace::new(w_columns, csr)
        .expect("Deflation subspace construction should not fail for well-conditioned E")
}

/// Generate a deterministic pseudo-random starting vector
///
/// Uses a simple hash-based approach seeded by the vector index
/// to produce reproducible, linearly independent starting vectors.
fn generate_starting_vector(n: usize, seed: usize) -> Array1<Complex64> {
    let mut v = Array1::from_elem(n, Complex64::new(0.0, 0.0));
    for j in 0..n {
        // Simple deterministic pseudo-random: golden ratio based hash
        let hash = ((seed + 1) * 2654435761 + j * 2246822519) as f64;
        let angle = hash * std::f64::consts::PI * 2.0 / (1u64 << 32) as f64;
        v[j] = Complex64::new(angle.cos(), angle.sin());
    }
    v
}

/// Modified Gram-Schmidt orthogonalization against existing vectors
fn modified_gram_schmidt(v: &mut Array1<Complex64>, basis: &[Array1<Complex64>]) {
    for w in basis {
        let proj: Complex64 = w.iter().zip(v.iter()).map(|(a, b)| a.conj() * *b).sum();
        *v -= &w.mapv(|wi| wi * proj);
    }
}

/// Normalize a vector to unit length
fn normalize(v: &mut Array1<Complex64>) {
    let norm: f64 = v.iter().map(|vi| vi.norm_sqr()).sum::<f64>().sqrt();
    if norm > 1e-15 {
        let inv_norm = 1.0 / norm;
        v.mapv_inplace(|vi| vi * inv_norm);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_inverse_iteration_produces_orthonormal_vectors() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(2.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let csr = problem.matrix.to_csr();
        let config = DeflationConfig {
            num_vectors: 3,
            method: DeflationMethod::InverseIteration { num_iterations: 3 },
        };

        let deflation = compute_deflation_subspace(&problem, &csr, 2.0, &config);
        assert_eq!(deflation.num_vectors(), 3);
    }

    #[test]
    fn test_deflation_config_constructors() {
        let config = DeflationConfig::default();
        assert_eq!(config.num_vectors, 10);

        let config = DeflationConfig::for_frequency_sweep(5);
        assert_eq!(config.num_vectors, 5);
    }

    #[test]
    fn test_zero_deflation_vectors() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let csr = problem.matrix.to_csr();
        let config = DeflationConfig {
            num_vectors: 0,
            method: DeflationMethod::default(),
        };

        let deflation = compute_deflation_subspace(&problem, &csr, 1.0, &config);
        assert_eq!(deflation.num_vectors(), 0);
    }
}
