//! Configuration for the Neural Multigrid (Wave-ADR-NS) solver
//!
//! Reference: Cui & Jiang (2024, arXiv:2404.02493)

/// Configuration for the Neural Multigrid (Wave-ADR-NS) solver
#[derive(Debug, Clone)]
pub struct NeuralMultigridConfig {
    /// Maximum outer cycle iterations (default: 20)
    pub max_iterations: usize,
    /// Convergence tolerance on relative residual (default: 1e-8)
    pub tolerance: f64,
    /// Per-level Chebyshev smoothing iterations (default: 5)
    pub chebyshev_iterations: usize,
    /// Coarsest-level Chebyshev iterations (default: 10)
    pub coarsest_chebyshev_iters: usize,
    /// ADR inner solve tolerance (default: 0.1)
    pub adr_tolerance: f64,
    /// ADR GMRES smoother iterations (default: 10)
    pub adr_gmres_iters: usize,
    /// Helmholtz damping parameter gamma (default: 0.5)
    pub damping_gamma: f64,
    /// Source point for eikonal equation [x, y, z] (default: domain center)
    pub source_point: Option<[f64; 3]>,
    /// Verbosity level (0 = quiet, 1 = summary, 2+ = detailed)
    pub verbosity: usize,
}

impl Default for NeuralMultigridConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            tolerance: 1e-8,
            chebyshev_iterations: 5,
            coarsest_chebyshev_iters: 10,
            adr_tolerance: 0.1,
            adr_gmres_iters: 10,
            damping_gamma: 0.5,
            source_point: None,
            verbosity: 0,
        }
    }
}

impl NeuralMultigridConfig {
    /// Configuration tuned for a given wavenumber
    ///
    /// Higher wavenumbers need more smoothing steps and tighter ADR tolerances.
    pub fn for_wavenumber(k: f64) -> Self {
        let chebyshev_iterations = if k > 50.0 {
            8
        } else if k > 20.0 {
            6
        } else {
            5
        };

        let coarsest_chebyshev_iters = if k > 50.0 {
            15
        } else if k > 20.0 {
            12
        } else {
            10
        };

        Self {
            chebyshev_iterations,
            coarsest_chebyshev_iters,
            ..Default::default()
        }
    }

    /// Fast configuration (fewer iterations, looser tolerances)
    pub fn fast() -> Self {
        Self {
            max_iterations: 10,
            tolerance: 1e-6,
            chebyshev_iterations: 3,
            coarsest_chebyshev_iters: 6,
            adr_tolerance: 0.2,
            adr_gmres_iters: 5,
            ..Default::default()
        }
    }

    /// High-accuracy configuration
    pub fn accurate() -> Self {
        Self {
            max_iterations: 50,
            tolerance: 1e-10,
            chebyshev_iterations: 8,
            coarsest_chebyshev_iters: 15,
            adr_tolerance: 0.05,
            adr_gmres_iters: 20,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NeuralMultigridConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.chebyshev_iterations, 5);
        assert!(config.tolerance > 0.0);
        assert!(config.damping_gamma > 0.0);
    }

    #[test]
    fn test_for_wavenumber() {
        let low = NeuralMultigridConfig::for_wavenumber(5.0);
        assert_eq!(low.chebyshev_iterations, 5);

        let mid = NeuralMultigridConfig::for_wavenumber(25.0);
        assert_eq!(mid.chebyshev_iterations, 6);

        let high = NeuralMultigridConfig::for_wavenumber(60.0);
        assert_eq!(high.chebyshev_iterations, 8);
    }
}
