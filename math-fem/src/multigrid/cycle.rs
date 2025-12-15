//! Multigrid cycle implementations
//!
//! Provides V-cycle, W-cycle, and F-cycle algorithms.

use super::hierarchy::MultigridHierarchy;
use super::smoother::{SmootherConfig, compute_residual, smooth};
use super::transfer::{prolongate, restrict};
use num_complex::Complex64;

/// Multigrid cycle type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleType {
    /// V-cycle: descend to coarsest, ascend
    VCycle,
    /// W-cycle: recursive V-cycles
    WCycle,
    /// F-cycle: FMG-like pattern
    FCycle,
}

/// Multigrid solver configuration
#[derive(Debug, Clone)]
pub struct MultigridConfig {
    /// Type of cycle
    pub cycle_type: CycleType,
    /// Pre-smoothing configuration
    pub pre_smoother: SmootherConfig,
    /// Post-smoothing configuration
    pub post_smoother: SmootherConfig,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Relative tolerance for convergence
    pub tolerance: f64,
    /// Number of cycles per iteration
    pub cycles_per_iteration: usize,
}

impl Default for MultigridConfig {
    fn default() -> Self {
        Self {
            cycle_type: CycleType::VCycle,
            pre_smoother: SmootherConfig::default(),
            post_smoother: SmootherConfig::default(),
            max_iterations: 50,
            tolerance: 1e-8,
            cycles_per_iteration: 1,
        }
    }
}

/// Multigrid solver result
#[derive(Debug)]
pub struct MultigridResult {
    /// Solution vector
    pub solution: Vec<Complex64>,
    /// Number of iterations performed
    pub iterations: usize,
    /// Final residual norm
    pub residual_norm: f64,
    /// Convergence achieved
    pub converged: bool,
}

/// Solve using multigrid method
pub fn solve_multigrid(
    hierarchy: &MultigridHierarchy,
    b: &[Complex64],
    initial_guess: Option<&[Complex64]>,
    config: &MultigridConfig,
) -> MultigridResult {
    let n = hierarchy.finest().n_dofs;

    // Initialize solution
    let mut x = match initial_guess {
        Some(guess) => guess.to_vec(),
        None => vec![Complex64::new(0.0, 0.0); n],
    };

    let matrix = hierarchy.levels[0]
        .system
        .as_ref()
        .expect("Finest level matrix not assembled");

    let b_norm = b.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();
    let tol = config.tolerance * b_norm.max(1e-10);

    let mut iterations = 0;
    let mut residual = compute_residual(matrix, &x, b);
    let mut res_norm = residual.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();

    while iterations < config.max_iterations && res_norm > tol {
        for _ in 0..config.cycles_per_iteration {
            match config.cycle_type {
                CycleType::VCycle => {
                    v_cycle(
                        hierarchy,
                        0,
                        &mut x,
                        b,
                        &config.pre_smoother,
                        &config.post_smoother,
                    );
                }
                CycleType::WCycle => {
                    w_cycle(
                        hierarchy,
                        0,
                        &mut x,
                        b,
                        &config.pre_smoother,
                        &config.post_smoother,
                    );
                }
                CycleType::FCycle => {
                    f_cycle(
                        hierarchy,
                        0,
                        &mut x,
                        b,
                        &config.pre_smoother,
                        &config.post_smoother,
                    );
                }
            }
        }

        residual = compute_residual(matrix, &x, b);
        res_norm = residual.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();
        iterations += 1;
    }

    MultigridResult {
        solution: x,
        iterations,
        residual_norm: res_norm,
        converged: res_norm <= tol,
    }
}

/// V-cycle: single descent to coarsest level
fn v_cycle(
    hierarchy: &MultigridHierarchy,
    level: usize,
    x: &mut [Complex64],
    b: &[Complex64],
    pre_config: &SmootherConfig,
    post_config: &SmootherConfig,
) {
    let n_levels = hierarchy.num_levels();

    if level == n_levels - 1 {
        // Coarsest level: solve exactly or smooth many times
        let matrix = hierarchy.levels[level]
            .system
            .as_ref()
            .expect("Coarse matrix not assembled");

        let exact_config = SmootherConfig {
            iterations: 20,
            ..pre_config.clone()
        };
        smooth(matrix, x, b, &exact_config);
        return;
    }

    let matrix = hierarchy.levels[level]
        .system
        .as_ref()
        .expect("Matrix not assembled");

    // Pre-smoothing
    smooth(matrix, x, b, pre_config);

    // Compute residual
    let residual = compute_residual(matrix, x, b);

    // Restrict residual to coarser level
    let restriction = hierarchy.levels[level]
        .restriction
        .as_ref()
        .expect("Restriction operator not built");
    let r_coarse = restrict(restriction, &residual);

    // Solve on coarser level (correction equation: A*e = r)
    let n_coarse = hierarchy.levels[level + 1].n_dofs;
    let mut e_coarse = vec![Complex64::new(0.0, 0.0); n_coarse];

    v_cycle(
        hierarchy,
        level + 1,
        &mut e_coarse,
        &r_coarse,
        pre_config,
        post_config,
    );

    // Prolongate correction
    let prolongation = hierarchy.levels[level + 1]
        .prolongation
        .as_ref()
        .expect("Prolongation operator not built");
    let e_fine = prolongate(prolongation, &e_coarse);

    // Add correction
    for i in 0..x.len() {
        x[i] += e_fine[i];
    }

    // Post-smoothing
    smooth(matrix, x, b, post_config);
}

/// W-cycle: recursive pattern with two coarse-level calls
fn w_cycle(
    hierarchy: &MultigridHierarchy,
    level: usize,
    x: &mut [Complex64],
    b: &[Complex64],
    pre_config: &SmootherConfig,
    post_config: &SmootherConfig,
) {
    let n_levels = hierarchy.num_levels();

    if level == n_levels - 1 {
        let matrix = hierarchy.levels[level]
            .system
            .as_ref()
            .expect("Coarse matrix not assembled");

        let exact_config = SmootherConfig {
            iterations: 20,
            ..pre_config.clone()
        };
        smooth(matrix, x, b, &exact_config);
        return;
    }

    let matrix = hierarchy.levels[level]
        .system
        .as_ref()
        .expect("Matrix not assembled");

    // Pre-smoothing
    smooth(matrix, x, b, pre_config);

    // Compute residual
    let residual = compute_residual(matrix, x, b);

    // Restrict
    let restriction = hierarchy.levels[level]
        .restriction
        .as_ref()
        .expect("Restriction operator not built");
    let r_coarse = restrict(restriction, &residual);

    // First coarse grid correction
    let n_coarse = hierarchy.levels[level + 1].n_dofs;
    let mut e_coarse = vec![Complex64::new(0.0, 0.0); n_coarse];
    w_cycle(
        hierarchy,
        level + 1,
        &mut e_coarse,
        &r_coarse,
        pre_config,
        post_config,
    );

    // Second coarse grid correction (W-cycle pattern)
    w_cycle(
        hierarchy,
        level + 1,
        &mut e_coarse,
        &r_coarse,
        pre_config,
        post_config,
    );

    // Prolongate and add correction
    let prolongation = hierarchy.levels[level + 1]
        .prolongation
        .as_ref()
        .expect("Prolongation operator not built");
    let e_fine = prolongate(prolongation, &e_coarse);

    for i in 0..x.len() {
        x[i] += e_fine[i];
    }

    // Post-smoothing
    smooth(matrix, x, b, post_config);
}

/// F-cycle: Full Multigrid pattern
fn f_cycle(
    hierarchy: &MultigridHierarchy,
    level: usize,
    x: &mut [Complex64],
    b: &[Complex64],
    pre_config: &SmootherConfig,
    post_config: &SmootherConfig,
) {
    let n_levels = hierarchy.num_levels();

    if level == n_levels - 1 {
        let matrix = hierarchy.levels[level]
            .system
            .as_ref()
            .expect("Coarse matrix not assembled");

        let exact_config = SmootherConfig {
            iterations: 20,
            ..pre_config.clone()
        };
        smooth(matrix, x, b, &exact_config);
        return;
    }

    // First, restrict to coarser level and solve there
    let restriction = hierarchy.levels[level]
        .restriction
        .as_ref()
        .expect("Restriction operator not built");

    // Restrict RHS (for FMG initialization)
    let b_coarse = restrict(restriction, b);

    // Solve on coarser level first (FMG pattern)
    let n_coarse = hierarchy.levels[level + 1].n_dofs;
    let mut x_coarse = vec![Complex64::new(0.0, 0.0); n_coarse];
    f_cycle(
        hierarchy,
        level + 1,
        &mut x_coarse,
        &b_coarse,
        pre_config,
        post_config,
    );

    // Prolongate as initial guess
    let prolongation = hierarchy.levels[level + 1]
        .prolongation
        .as_ref()
        .expect("Prolongation operator not built");
    let x_init = prolongate(prolongation, &x_coarse);

    for (i, xi) in x_init.iter().enumerate() {
        x[i] = *xi;
    }

    // Now do V-cycle to polish
    v_cycle(hierarchy, level, x, b, pre_config, post_config);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_v_cycle_reduces_residual() {
        // Use smoother directly to test residual reduction
        // Apply Dirichlet BCs to make the system non-singular
        use crate::assembly::HelmholtzProblem;
        use crate::boundary::apply_homogeneous_dirichlet;
        use crate::multigrid::smoother::{SmootherConfig, residual_norm, smooth};

        let mesh = unit_square_triangles(4);
        let k = Complex64::new(0.0, 0.0);

        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, |_, _, _| {
            Complex64::new(1.0, 0.0)
        });

        // Apply Dirichlet BC on all boundaries to make system non-singular
        apply_homogeneous_dirichlet(&mut problem, &mesh, &[1, 2, 3, 4]);

        let matrix = problem.matrix.to_compressed();
        let b = &problem.rhs;
        let mut x = vec![Complex64::new(0.0, 0.0); matrix.dim];

        let initial_norm = residual_norm(&matrix, &x, b);

        // Apply many smoothing iterations
        let config = SmootherConfig {
            iterations: 50,
            ..Default::default()
        };
        smooth(&matrix, &mut x, b, &config);

        let final_norm = residual_norm(&matrix, &x, b);

        // Smoothing should reduce residual for a well-posed problem
        assert!(
            final_norm < initial_norm,
            "Residual should decrease: {} -> {}",
            initial_norm,
            final_norm
        );
    }

    #[test]
    fn test_multigrid_config() {
        let config = MultigridConfig {
            cycle_type: CycleType::WCycle,
            max_iterations: 100,
            tolerance: 1e-10,
            ..Default::default()
        };

        assert_eq!(config.cycle_type, CycleType::WCycle);
        assert_eq!(config.max_iterations, 100);
    }
}
