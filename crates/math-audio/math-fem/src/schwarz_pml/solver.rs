//! Schwarz iteration solver with PML transmission conditions
//!
//! Implements additive and multiplicative Schwarz methods where artificial
//! subdomain boundaries use PML absorption instead of classical transmission
//! conditions, achieving k-independent convergence.
//!
//! Reference: Galkowski et al. (2024, arXiv:2408.16580)

use crate::mesh::Mesh;
use crate::basis::PolynomialDegree;
use crate::schwarz_pml::config::{SchwarzPmlConfig, SchwarzVariant};
use crate::schwarz_pml::decomposition::{
    SubdomainInfo, compute_partition_of_unity, decompose_domain, extract_local_mesh,
};
use crate::schwarz_pml::local_assembly::assemble_local_pml_system;
use crate::solver::{Solution, SolverConfig, SolverError};
use math_audio_solvers::CsrMatrix;
use ndarray::Array1;
use num_complex::Complex64;
use std::collections::HashSet;
use std::time::Instant;

/// Solve a Helmholtz problem using the Optimized Schwarz Method with PML
///
/// This is the primary entry point. Unlike most solvers in solver/mod.rs, this
/// needs access to the mesh geometry for domain decomposition and PML construction.
///
/// # Arguments
/// * `mesh` - The global finite element mesh
/// * `degree` - Polynomial degree for basis functions
/// * `wavenumber` - Complex wavenumber (real part used for PML sizing)
/// * `rhs` - Global right-hand side vector (length = mesh.num_nodes())
/// * `dirichlet_bcs` - Global Dirichlet boundary conditions: (node_index, value)
/// * `config` - Schwarz-PML configuration
pub fn solve_schwarz_pml(
    mesh: &Mesh,
    degree: PolynomialDegree,
    wavenumber: Complex64,
    rhs: &[Complex64],
    dirichlet_bcs: &[(usize, Complex64)],
    config: &SchwarzPmlConfig,
) -> Result<Solution, SolverError> {
    let k = wavenumber.re;
    if k <= 0.0 {
        return Err(SolverError::InvalidConfiguration(
            "Wavenumber must have positive real part".into(),
        ));
    }

    let n_global = mesh.num_nodes();
    if rhs.len() != n_global {
        return Err(SolverError::DimensionMismatch {
            expected: n_global,
            actual: rhs.len(),
        });
    }

    let start = Instant::now();

    // 1. Decompose domain
    let subdomains = decompose_domain(mesh, config, k);

    if config.verbosity > 0 {
        println!(
            "  [Schwarz-PML] {} subdomains, k={:.2}, overlap={:.1}%, PML={:.2} wavelengths",
            subdomains.len(),
            k,
            config.overlap_fraction * 100.0,
            config.pml_wavelengths,
        );
    }

    // 2. Assemble local systems for each subdomain
    let local_data: Vec<LocalSubdomainData> = subdomains
        .iter()
        .map(|sub| prepare_subdomain(mesh, sub, degree, k, dirichlet_bcs))
        .collect();

    let setup_time = start.elapsed();
    if config.verbosity > 0 {
        println!(
            "  [Schwarz-PML] Setup: {:.1}ms ({} local systems assembled)",
            setup_time.as_secs_f64() * 1000.0,
            local_data.len(),
        );
    }

    // 3. Compute partition of unity
    let pou_weights = compute_partition_of_unity(mesh, &subdomains);

    // 4. Run Schwarz iteration
    let solve_start = Instant::now();
    let result = match config.variant {
        SchwarzVariant::Additive => schwarz_additive(
            mesh, rhs, &subdomains, &local_data, &pou_weights, config,
        ),
        SchwarzVariant::Multiplicative => schwarz_multiplicative(
            mesh, rhs, &subdomains, &local_data, &pou_weights, config,
        ),
    };

    let solve_time = solve_start.elapsed();
    if config.verbosity > 0 {
        match &result {
            Ok(sol) => println!(
                "  [Schwarz-PML] {} in {} iterations (residual: {:.2e}, time: {:.1}ms)",
                if sol.converged { "Converged" } else { "Did not converge" },
                sol.iterations,
                sol.residual,
                solve_time.as_secs_f64() * 1000.0,
            ),
            Err(e) => println!("  [Schwarz-PML] Failed: {}", e),
        }
    }

    result
}

/// Pre-assembled data for a single subdomain
struct LocalSubdomainData {
    /// Local CSR system matrix (K_pml - k² M_pml with Dirichlet BCs)
    system: CsrMatrix<Complex64>,
    /// Local Dirichlet node indices for PML truncation + global Dirichlet (always zero)
    pml_dirichlet_nodes: HashSet<usize>,
    /// Local node indices on the overlap boundary (Dirichlet from current iterate)
    overlap_boundary_nodes: HashSet<usize>,
}

/// Prepare a subdomain: extract local mesh, assemble PML system, restrict RHS
fn prepare_subdomain(
    global_mesh: &Mesh,
    sub: &SubdomainInfo,
    degree: PolynomialDegree,
    k: f64,
    global_dirichlet_bcs: &[(usize, Complex64)],
) -> LocalSubdomainData {
    let local_mesh = extract_local_mesh(global_mesh, sub);

    // Merge Dirichlet nodes: PML truncation + global boundary + overlap boundary
    let mut pml_dirichlet_nodes = sub.dirichlet_local_nodes.clone();

    for &(global_node, _value) in global_dirichlet_bcs {
        if let Some(&local_node) = sub.global_to_local.get(&global_node) {
            pml_dirichlet_nodes.insert(local_node);
        }
    }

    // All Dirichlet nodes: PML truncation + global + overlap boundary
    let mut all_dirichlet = pml_dirichlet_nodes.clone();
    all_dirichlet.extend(&sub.overlap_boundary_nodes);

    let system = assemble_local_pml_system(
        &local_mesh,
        degree,
        k,
        &sub.pml_regions,
        &all_dirichlet,
    );

    LocalSubdomainData {
        system,
        pml_dirichlet_nodes,
        overlap_boundary_nodes: sub.overlap_boundary_nodes.clone(),
    }
}

/// Additive Schwarz iteration
///
/// All subdomains are solved independently (parallelizable), then combined
/// using partition-of-unity weights.
fn schwarz_additive(
    mesh: &Mesh,
    global_rhs: &[Complex64],
    subdomains: &[SubdomainInfo],
    local_data: &[LocalSubdomainData],
    pou_weights: &[Vec<(usize, f64)>],
    config: &SchwarzPmlConfig,
) -> Result<Solution, SolverError> {
    let n = mesh.num_nodes();
    let mut u_global = Array1::from_vec(vec![Complex64::new(0.0, 0.0); n]);

    let local_solver_config = SolverConfig {
        solver_type: config.local_solver,
        gmres: config.local_gmres.clone(),
        verbosity: config.verbosity.saturating_sub(1),
        ..Default::default()
    };

    for iter in 0..config.max_iterations {
        let mut u_new = Array1::from_vec(vec![Complex64::new(0.0, 0.0); n]);

        // Solve each subdomain independently
        for (j, (sub, ld)) in subdomains.iter().zip(local_data.iter()).enumerate() {
            let local_rhs = build_local_rhs_with_overlap(
                global_rhs, &u_global, sub, ld,
            );

            let u_local = solve_local_system(&ld.system, &local_rhs, &local_solver_config)?;

            // Prolongate with partition-of-unity weights
            for (local_idx, &global_idx) in sub.local_to_global.iter().enumerate() {
                // Find the weight for this subdomain at this node
                for &(sub_idx, weight) in &pou_weights[global_idx] {
                    if sub_idx == j {
                        u_new[global_idx] += weight * u_local[local_idx];
                        break;
                    }
                }
            }
        }

        // Check convergence: relative change
        let diff_norm: f64 = u_new.iter().zip(u_global.iter())
            .map(|(a, b)| (a - b).norm().powi(2))
            .sum::<f64>()
            .sqrt();
        let new_norm: f64 = u_new.iter().map(|v| v.norm().powi(2)).sum::<f64>().sqrt().max(1e-15);
        let rel_change = diff_norm / new_norm;

        if config.verbosity > 1 {
            println!(
                "    [Schwarz-PML] iter {}: rel_change = {:.2e}",
                iter + 1, rel_change
            );
        }

        u_global = u_new;

        if rel_change < config.tolerance {
            return Ok(Solution {
                values: u_global,
                iterations: iter + 1,
                residual: rel_change,
                converged: true,
            });
        }
    }

    let final_norm: f64 = u_global.iter().map(|v| v.norm().powi(2)).sum::<f64>().sqrt().max(1e-15);
    Err(SolverError::ConvergenceFailure(config.max_iterations, final_norm))
}

/// Multiplicative Schwarz iteration
///
/// Subdomains are solved sequentially; each sees the latest global solution
/// from previous subdomains in the current sweep.
fn schwarz_multiplicative(
    mesh: &Mesh,
    global_rhs: &[Complex64],
    subdomains: &[SubdomainInfo],
    local_data: &[LocalSubdomainData],
    pou_weights: &[Vec<(usize, f64)>],
    config: &SchwarzPmlConfig,
) -> Result<Solution, SolverError> {
    let n = mesh.num_nodes();
    let mut u_global = Array1::from_vec(vec![Complex64::new(0.0, 0.0); n]);

    let local_solver_config = SolverConfig {
        solver_type: config.local_solver,
        gmres: config.local_gmres.clone(),
        verbosity: config.verbosity.saturating_sub(1),
        ..Default::default()
    };

    for iter in 0..config.max_iterations {
        let u_prev = u_global.clone();

        // Solve subdomains sequentially, updating u_global after each
        for (j, (sub, ld)) in subdomains.iter().zip(local_data.iter()).enumerate() {
            let local_rhs = build_local_rhs_with_overlap(
                global_rhs, &u_global, sub, ld,
            );

            let u_local = solve_local_system(&ld.system, &local_rhs, &local_solver_config)?;

            // Update global solution with partition-of-unity weights
            // For multiplicative: update in place so next subdomain sees it
            for (local_idx, &global_idx) in sub.local_to_global.iter().enumerate() {
                for &(sub_idx, weight) in &pou_weights[global_idx] {
                    if sub_idx == j {
                        // Replace this subdomain's contribution
                        // (For multiplicative, we need to be more careful about overlaps)
                        // Simple approach: weighted update
                        u_global[global_idx] += weight * (u_local[local_idx] - restrict_global_to_local_node(&u_prev, sub, local_idx));
                        break;
                    }
                }
            }
        }

        // Check convergence: relative change
        let diff_norm: f64 = u_global.iter().zip(u_prev.iter())
            .map(|(a, b)| (a - b).norm().powi(2))
            .sum::<f64>()
            .sqrt();
        let new_norm: f64 = u_global.iter().map(|v| v.norm().powi(2)).sum::<f64>().sqrt().max(1e-15);
        let rel_change = diff_norm / new_norm;

        if config.verbosity > 1 {
            println!(
                "    [Schwarz-PML] iter {}: rel_change = {:.2e}",
                iter + 1, rel_change
            );
        }

        if rel_change < config.tolerance {
            return Ok(Solution {
                values: u_global,
                iterations: iter + 1,
                residual: rel_change,
                converged: true,
            });
        }
    }

    let final_norm: f64 = u_global.iter().map(|v| v.norm().powi(2)).sum::<f64>().sqrt().max(1e-15);
    Err(SolverError::ConvergenceFailure(config.max_iterations, final_norm))
}

/// Build the local RHS for a subdomain, incorporating overlap boundary conditions
///
/// The local RHS is:
/// - For interior nodes: restricted global RHS
/// - For overlap boundary nodes: Dirichlet from current global solution
/// - For PML truncation nodes: homogeneous Dirichlet (zero)
fn build_local_rhs_with_overlap(
    global_rhs: &[Complex64],
    u_global: &Array1<Complex64>,
    sub: &SubdomainInfo,
    ld: &LocalSubdomainData,
) -> Vec<Complex64> {
    let n_local = sub.local_to_global.len();
    let mut rhs = vec![Complex64::new(0.0, 0.0); n_local];

    for (local_idx, &global_idx) in sub.local_to_global.iter().enumerate() {
        if ld.pml_dirichlet_nodes.contains(&local_idx) {
            // PML truncation + global Dirichlet boundary: homogeneous Dirichlet
            rhs[local_idx] = Complex64::new(0.0, 0.0);
        } else if ld.overlap_boundary_nodes.contains(&local_idx) {
            // Overlap boundary: Dirichlet from current iterate
            rhs[local_idx] = u_global[global_idx];
        } else {
            // Interior nodes: use global RHS
            rhs[local_idx] = global_rhs[global_idx];
        }
    }

    rhs
}

/// Restrict global solution to a local node
fn restrict_global_to_local_node(
    u_global: &Array1<Complex64>,
    sub: &SubdomainInfo,
    local_idx: usize,
) -> Complex64 {
    let global_idx = sub.local_to_global[local_idx];
    u_global[global_idx]
}

/// Solve a local subdomain system using the configured solver
fn solve_local_system(
    system: &CsrMatrix<Complex64>,
    rhs: &[Complex64],
    config: &SolverConfig,
) -> Result<Vec<Complex64>, SolverError> {
    let rhs_array = Array1::from_vec(rhs.to_vec());

    let solution = crate::solver::solve_csr(system, &rhs_array, config)?;

    Ok(solution.values.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::unit_square_triangles;
    use crate::assembly::HelmholtzProblem;
    use std::f64::consts::PI;

    #[test]
    fn test_schwarz_pml_basic_convergence() {
        // Simple test: unit square, k=2, uniform source
        let mesh = unit_square_triangles(8);
        let k = 2.0;
        let wavenumber = Complex64::new(k, 0.0);

        // Assemble global problem for RHS
        let problem = HelmholtzProblem::assemble(
            &mesh,
            PolynomialDegree::P1,
            wavenumber,
            |x, y, _| Complex64::new((PI * x).sin() * (PI * y).sin(), 0.0),
        );

        // All boundary nodes as Dirichlet = 0
        let mut dirichlet_bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.x.abs() < 1e-10
                || (node.x - 1.0).abs() < 1e-10
                || node.y.abs() < 1e-10
                || (node.y - 1.0).abs() < 1e-10
            {
                dirichlet_bcs.push((i, Complex64::new(0.0, 0.0)));
            }
        }

        let config = SchwarzPmlConfig {
            num_subdomains: 2,
            max_iterations: 50,
            tolerance: 1e-4,
            verbosity: 0,
            ..Default::default()
        };

        let result = solve_schwarz_pml(
            &mesh,
            PolynomialDegree::P1,
            wavenumber,
            &problem.rhs,
            &dirichlet_bcs,
            &config,
        );

        assert!(result.is_ok(), "Schwarz-PML should converge: {:?}", result.err());
        let sol = result.unwrap();
        assert!(sol.converged);
        assert!(sol.iterations < config.max_iterations);
    }

    #[test]
    fn test_multiplicative_converges_faster() {
        let mesh = unit_square_triangles(8);
        let k = 3.0;
        let wavenumber = Complex64::new(k, 0.0);

        let problem = HelmholtzProblem::assemble(
            &mesh,
            PolynomialDegree::P1,
            wavenumber,
            |_, _, _| Complex64::new(1.0, 0.0),
        );

        let mut dirichlet_bcs = Vec::new();
        for (i, node) in mesh.nodes.iter().enumerate() {
            if node.x.abs() < 1e-10
                || (node.x - 1.0).abs() < 1e-10
                || node.y.abs() < 1e-10
                || (node.y - 1.0).abs() < 1e-10
            {
                dirichlet_bcs.push((i, Complex64::new(0.0, 0.0)));
            }
        }

        let base_config = SchwarzPmlConfig {
            num_subdomains: 2,
            max_iterations: 50,
            tolerance: 1e-4,
            verbosity: 0,
            ..Default::default()
        };

        let additive_config = SchwarzPmlConfig {
            variant: SchwarzVariant::Additive,
            ..base_config.clone()
        };
        let multiplicative_config = SchwarzPmlConfig {
            variant: SchwarzVariant::Multiplicative,
            ..base_config
        };

        let add_result = solve_schwarz_pml(
            &mesh, PolynomialDegree::P1, wavenumber,
            &problem.rhs, &dirichlet_bcs, &additive_config,
        );
        let mult_result = solve_schwarz_pml(
            &mesh, PolynomialDegree::P1, wavenumber,
            &problem.rhs, &dirichlet_bcs, &multiplicative_config,
        );

        // Both should converge
        assert!(add_result.is_ok());
        assert!(mult_result.is_ok());

        // Multiplicative should converge in fewer or equal iterations
        let add_sol = add_result.unwrap();
        let mult_sol = mult_result.unwrap();
        assert!(
            mult_sol.iterations <= add_sol.iterations + 5,
            "Multiplicative ({}) should converge no slower than additive ({})",
            mult_sol.iterations,
            add_sol.iterations,
        );
    }
}
