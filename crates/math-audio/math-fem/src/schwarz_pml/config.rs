//! Configuration for Optimized Schwarz Methods with PML transmission conditions
//!
//! Reference: Galkowski et al. (2024, arXiv:2408.16580)

use crate::solver::{GmresConfigF64, SolverType};

/// Schwarz iteration variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchwarzVariant {
    /// Additive Schwarz: all subdomains solved in parallel, combined with partition of unity
    Additive,
    /// Multiplicative Schwarz: subdomains solved sequentially, each sees latest updates
    Multiplicative,
}

/// Configuration for the Optimized Schwarz Method with PML transmission conditions
#[derive(Debug, Clone)]
pub struct SchwarzPmlConfig {
    /// Schwarz iteration variant (additive or multiplicative)
    pub variant: SchwarzVariant,
    /// Number of subdomains (strips along x-axis)
    pub num_subdomains: usize,
    /// Overlap as fraction of strip width
    pub overlap_fraction: f64,
    /// PML width in wavelength units (lambda = 2*pi/k)
    pub pml_wavelengths: f64,
    /// Polynomial profile power for PML absorption
    pub pml_power: usize,
    /// Target reflection coefficient for optimal sigma_max
    pub pml_target_reflection: f64,
    /// Maximum outer Schwarz iterations
    pub max_iterations: usize,
    /// Convergence tolerance for relative residual
    pub tolerance: f64,
    /// Solver type for local subdomain problems
    pub local_solver: SolverType,
    /// GMRES parameters for local solves
    pub local_gmres: GmresConfigF64,
    /// Verbosity level (0 = quiet, 1 = summary, 2+ = detailed)
    pub verbosity: usize,
}

impl Default for SchwarzPmlConfig {
    fn default() -> Self {
        Self {
            variant: SchwarzVariant::Additive,
            num_subdomains: 4,
            overlap_fraction: 0.1,
            pml_wavelengths: 1.0,
            pml_power: 2,
            pml_target_reflection: 1e-6,
            max_iterations: 50,
            tolerance: 1e-8,
            local_solver: SolverType::GmresIlu,
            local_gmres: GmresConfigF64 {
                max_iterations: 500,
                restart: 30,
                tolerance: 1e-10,
                print_interval: 0,
            },
            verbosity: 0,
        }
    }
}

impl SchwarzPmlConfig {
    /// Configuration tuned for a given wavenumber
    ///
    /// Higher wavenumbers use more subdomains and tighter PML.
    pub fn for_wavenumber(k: f64) -> Self {
        let num_subdomains = if k > 30.0 {
            8
        } else if k > 15.0 {
            6
        } else {
            4
        };
        Self {
            num_subdomains,
            ..Default::default()
        }
    }

    /// Fast configuration (fewer iterations, looser tolerances)
    pub fn fast() -> Self {
        Self {
            max_iterations: 20,
            tolerance: 1e-6,
            local_gmres: GmresConfigF64 {
                max_iterations: 200,
                restart: 20,
                tolerance: 1e-8,
                print_interval: 0,
            },
            ..Default::default()
        }
    }

    /// High-accuracy configuration
    pub fn accurate() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-10,
            pml_wavelengths: 1.5,
            pml_target_reflection: 1e-8,
            local_gmres: GmresConfigF64 {
                max_iterations: 1000,
                restart: 50,
                tolerance: 1e-12,
                print_interval: 0,
            },
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SchwarzPmlConfig::default();
        assert_eq!(config.num_subdomains, 4);
        assert_eq!(config.variant, SchwarzVariant::Additive);
        assert!(config.tolerance > 0.0);
    }

    #[test]
    fn test_for_wavenumber() {
        let low = SchwarzPmlConfig::for_wavenumber(5.0);
        assert_eq!(low.num_subdomains, 4);

        let mid = SchwarzPmlConfig::for_wavenumber(20.0);
        assert_eq!(mid.num_subdomains, 6);

        let high = SchwarzPmlConfig::for_wavenumber(40.0);
        assert_eq!(high.num_subdomains, 8);
    }
}
