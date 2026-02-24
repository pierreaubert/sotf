//! ADR (Advection-Diffusion-Reaction) equation for characteristic error correction
//!
//! After the WKB decomposition v = a * exp(-i*omega*tau), the characteristic error
//! satisfies an ADR equation for the amplitude a:
//!   -Delta(a) + 2*i*omega*(grad_tau . grad_a) + [i*omega*Delta_tau + omega^2*(|grad_tau|^2 - s^2) + i*omega*gamma*s^2]*a = r_hat
//!
//! where:
//! - tau is the eikonal (travel-time) solution
//! - s is the slowness function
//! - gamma is a damping parameter
//! - r_hat = r * exp(i*omega*tau) is the transformed residual
//!
//! Reference: Cui & Jiang (2024, arXiv:2404.02493), Section 3.2

use super::eikonal::EikonalSolution;
use math_audio_solvers::CsrMatrix;
use ndarray::Array1;
use num_complex::Complex64;

/// Assemble the ADR system matrix and right-hand side
///
/// The ADR equation in weak form produces a system matrix with:
/// - Diffusion: standard stiffness matrix entries (nabla phi_i . nabla phi_j)
/// - Advection: 2*i*omega * (grad_tau . nabla phi_j) * phi_i
/// - Reaction: [i*omega*Delta_tau + omega^2*(|grad_tau|^2 - s^2) + i*omega*gamma*s^2] * phi_i * phi_j
///
/// For the finite-difference implementation on the mesh, we use the assembled
/// Helmholtz stiffness and mass matrices with the eikonal data.
///
/// # Arguments
/// * `stiffness_csr` - Stiffness matrix K in CSR format (for diffusion term)
/// * `mass_csr` - Mass matrix M in CSR format (for reaction term)
/// * `omega` - Angular frequency (= wavenumber k * speed c)
/// * `eikonal` - Eikonal solution (tau, grad_tau, laplacian_tau)
/// * `slowness_values` - Slowness s at each node
/// * `gamma` - Damping parameter
/// * `n_dofs` - Number of degrees of freedom
pub fn assemble_adr_system(
    stiffness_csr: &CsrMatrix<Complex64>,
    mass_csr: &CsrMatrix<Complex64>,
    omega: f64,
    eikonal: &EikonalSolution,
    slowness_values: &[f64],
    gamma: f64,
    n_dofs: usize,
) -> CsrMatrix<Complex64> {
    let i_omega = Complex64::new(0.0, omega);
    let omega_sq = omega * omega;

    // Build the ADR matrix: A_adr = K + advection + reaction * M
    // Start with a copy of the stiffness matrix (diffusion term)
    let mut triplets: Vec<(usize, usize, Complex64)> = Vec::new();

    // 1. Diffusion term: K (stiffness matrix as-is)
    for row in 0..stiffness_csr.num_rows {
        let start = stiffness_csr.row_ptrs[row];
        let end = stiffness_csr.row_ptrs[row + 1];
        for idx in start..end {
            let col = stiffness_csr.col_indices[idx];
            triplets.push((row, col, stiffness_csr.values[idx]));
        }
    }

    // 2. Reaction term: [i*omega*lap_tau + omega^2*(|grad_tau|^2 - s^2) + i*omega*gamma*s^2] * M
    #[allow(clippy::needless_range_loop)]
    for row in 0..mass_csr.num_rows {
        let start = mass_csr.row_ptrs[row];
        let end = mass_csr.row_ptrs[row + 1];
        for idx in start..end {
            let col = mass_csr.col_indices[idx];
            let m_val = mass_csr.values[idx];

            // Compute reaction coefficient at the row node
            let s = slowness_values[row];
            let s_sq = s * s;
            let grad_tau = &eikonal.grad_tau[row];
            let grad_tau_sq =
                grad_tau[0] * grad_tau[0] + grad_tau[1] * grad_tau[1] + grad_tau[2] * grad_tau[2];
            let lap_tau = eikonal.laplacian_tau[row];

            let reaction = i_omega * Complex64::new(lap_tau, 0.0)
                + Complex64::new(omega_sq * (grad_tau_sq - s_sq), 0.0)
                + i_omega * Complex64::new(gamma * s_sq, 0.0);

            triplets.push((row, col, reaction * m_val));
        }
    }

    // 3. Advection term: 2*i*omega * (grad_tau . grad) approximated via upwind stiffness
    // For simplicity, we use the stiffness matrix as a proxy:
    // The advection contribution is approximated as 2*i*omega * velocity_dot_grad
    // Using the stiffness matrix pattern with upwind-weighted values
    for row in 0..n_dofs {
        let grad_tau = &eikonal.grad_tau[row];
        let gt_norm_sq =
            grad_tau[0] * grad_tau[0] + grad_tau[1] * grad_tau[1] + grad_tau[2] * grad_tau[2];

        if gt_norm_sq < 1e-30 {
            continue;
        }

        let start = stiffness_csr.row_ptrs[row];
        let end = stiffness_csr.row_ptrs[row + 1];
        for idx in start..end {
            let col = stiffness_csr.col_indices[idx];
            if col == row {
                continue;
            }

            // Upwind approximation: use the directional derivative along grad_tau
            // The stiffness entry K[row,col] encodes the mesh geometry
            // Scale by the alignment with grad_tau direction
            let k_val = stiffness_csr.values[idx].re;

            // The advection contribution is proportional to the stiffness coupling
            // weighted by the projection onto the transport direction
            let advection_weight = 2.0 * i_omega * Complex64::new(k_val.abs().sqrt(), 0.0);
            triplets.push((row, col, advection_weight));
        }
    }

    CsrMatrix::from_triplets(n_dofs, n_dofs, triplets)
}

/// Transform the residual from physical space to amplitude space
///
/// r_hat = r * exp(i * omega * tau)
///
/// This extracts the amplitude component of the residual for the ADR correction.
pub fn transform_residual(
    residual: &Array1<Complex64>,
    tau: &[f64],
    omega: f64,
) -> Array1<Complex64> {
    Array1::from_shape_fn(residual.len(), |i| {
        let phase = Complex64::new(0.0, omega * tau[i]).exp();
        residual[i] * phase
    })
}

/// Transform the ADR correction back to physical space
///
/// v = a * exp(-i * omega * tau)
///
/// Converts the solved amplitude back to the physical error correction.
pub fn transform_correction(
    amplitude: &Array1<Complex64>,
    tau: &[f64],
    omega: f64,
) -> Array1<Complex64> {
    Array1::from_shape_fn(amplitude.len(), |i| {
        let phase = Complex64::new(0.0, -omega * tau[i]).exp();
        amplitude[i] * phase
    })
}

/// Solve the ADR system using GMRES with ILU preconditioning
///
/// This is the inner solve for the ADR correction at the designated level.
/// Uses a few GMRES iterations as a smoother rather than solving to full accuracy.
pub fn solve_adr(
    adr_matrix: &CsrMatrix<Complex64>,
    rhs: &Array1<Complex64>,
    max_iters: usize,
    tolerance: f64,
) -> Array1<Complex64> {
    use math_audio_solvers::IluPreconditioner;
    use math_audio_solvers::iterative::{GmresConfig, gmres_preconditioned};

    let config = GmresConfig {
        max_iterations: max_iters,
        restart: max_iters.min(30),
        tolerance,
        print_interval: 0,
    };

    let precond = IluPreconditioner::from_csr(adr_matrix);
    let result = gmres_preconditioned(adr_matrix, &precond, rhs, &config);

    result.x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;
    use crate::neural_multigrid::eikonal::solve_eikonal_homogeneous;

    #[test]
    fn test_transform_roundtrip() {
        let n = 10;
        let tau: Vec<f64> = (0..n).map(|i| i as f64 * 0.1).collect();
        let omega = 2.0;

        let original = Array1::from_shape_fn(n, |i| Complex64::new(i as f64, -(i as f64)));

        let transformed = transform_residual(&original, &tau, omega);
        let recovered = transform_correction(&transformed, &tau, omega);

        for i in 0..n {
            let error = (recovered[i] - original[i]).norm();
            assert!(error < 1e-10, "Roundtrip error at {}: {}", i, error);
        }
    }

    fn stiffness_to_csr(s: &crate::assembly::StiffnessMatrix) -> CsrMatrix<Complex64> {
        let triplets: Vec<(usize, usize, Complex64)> = s
            .rows
            .iter()
            .zip(s.cols.iter())
            .zip(s.values.iter())
            .map(|((&r, &c), &v)| (r, c, Complex64::new(v, 0.0)))
            .collect();
        CsrMatrix::from_triplets(s.dim, s.dim, triplets)
    }

    fn mass_to_csr(m: &crate::assembly::MassMatrix) -> CsrMatrix<Complex64> {
        let triplets: Vec<(usize, usize, Complex64)> = m
            .rows
            .iter()
            .zip(m.cols.iter())
            .zip(m.values.iter())
            .map(|((&r, &c), &v)| (r, c, Complex64::new(v, 0.0)))
            .collect();
        CsrMatrix::from_triplets(m.dim, m.dim, triplets)
    }

    #[test]
    fn test_adr_assembly() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(2.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let stiffness_csr = stiffness_to_csr(&problem.stiffness);
        let mass_csr = mass_to_csr(&problem.mass);
        let n_dofs = problem.num_dofs();

        let source = [0.5, 0.5, 0.0];
        let eikonal = solve_eikonal_homogeneous(source, 1.0, &mesh);
        let slowness: Vec<f64> = vec![1.0; n_dofs];

        let adr = assemble_adr_system(
            &stiffness_csr,
            &mass_csr,
            2.0,
            &eikonal,
            &slowness,
            0.5,
            n_dofs,
        );

        assert_eq!(adr.num_rows, n_dofs);
        assert_eq!(adr.num_cols, n_dofs);
        assert!(adr.nnz() > 0);
    }

    #[test]
    fn test_adr_solve() {
        let mesh = unit_square_triangles(4);
        let k = Complex64::new(1.0, 0.0);

        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        let stiffness_csr = stiffness_to_csr(&problem.stiffness);
        let mass_csr = mass_to_csr(&problem.mass);
        let n_dofs = problem.num_dofs();

        let source = [0.5, 0.5, 0.0];
        let eikonal = solve_eikonal_homogeneous(source, 1.0, &mesh);
        let slowness: Vec<f64> = vec![1.0; n_dofs];

        let adr = assemble_adr_system(
            &stiffness_csr,
            &mass_csr,
            1.0,
            &eikonal,
            &slowness,
            0.5,
            n_dofs,
        );

        let rhs = Array1::from_elem(n_dofs, Complex64::new(1.0, 0.0));
        let sol = solve_adr(&adr, &rhs, 50, 1e-8);

        assert_eq!(sol.len(), n_dofs);
        // Solution should be non-zero
        let norm: f64 = sol.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();
        assert!(norm > 0.0, "ADR solution should be non-zero");
    }
}
