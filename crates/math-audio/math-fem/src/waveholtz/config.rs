//! WaveHoltz solver configuration

/// Configuration for the WaveHoltz solver
#[derive(Debug, Clone)]
pub struct WaveHoltzConfig {
    /// Number of time steps per wave period T = 2π/ω (default: 10, minimum 4)
    pub steps_per_period: usize,
    /// Maximum outer GMRES iterations (default: 100)
    pub max_iterations: usize,
    /// GMRES restart parameter (default: 30)
    pub gmres_restart: usize,
    /// Outer GMRES convergence tolerance (default: 1e-10)
    pub tolerance: f64,
    /// Inner CG solver tolerance (default: 1e-12)
    pub inner_tolerance: f64,
    /// Inner CG maximum iterations (default: 500)
    pub inner_max_iterations: usize,
    /// Use AMG preconditioner for inner CG solves (default: true)
    pub use_amg_inner: bool,
    /// Apply dispersion correction to filter weights (default: true)
    pub dispersion_correction: bool,
    /// Verbosity level (0 = quiet, 1 = summary, 2+ = detailed)
    pub verbosity: usize,
}

impl Default for WaveHoltzConfig {
    fn default() -> Self {
        Self {
            steps_per_period: 10,
            max_iterations: 100,
            gmres_restart: 30,
            tolerance: 1e-10,
            inner_tolerance: 1e-12,
            inner_max_iterations: 500,
            use_amg_inner: true,
            dispersion_correction: true,
            verbosity: 0,
        }
    }
}

impl WaveHoltzConfig {
    /// Configuration tuned for a given wavenumber
    ///
    /// Higher wavenumbers need more time steps per period for accuracy.
    pub fn for_wavenumber(k: f64) -> Self {
        let steps = if k > 20.0 {
            16
        } else if k > 10.0 {
            12
        } else {
            10
        };
        Self {
            steps_per_period: steps,
            ..Default::default()
        }
    }

    /// Fast configuration (fewer steps, looser tolerances)
    pub fn fast() -> Self {
        Self {
            steps_per_period: 6,
            max_iterations: 50,
            gmres_restart: 20,
            tolerance: 1e-6,
            inner_tolerance: 1e-8,
            inner_max_iterations: 200,
            use_amg_inner: true,
            dispersion_correction: false,
            verbosity: 0,
        }
    }

    /// High-accuracy configuration
    pub fn accurate() -> Self {
        Self {
            steps_per_period: 16,
            max_iterations: 200,
            gmres_restart: 50,
            tolerance: 1e-12,
            inner_tolerance: 1e-14,
            inner_max_iterations: 1000,
            use_amg_inner: true,
            dispersion_correction: true,
            verbosity: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WaveHoltzConfig::default();
        assert_eq!(config.steps_per_period, 10);
        assert_eq!(config.max_iterations, 100);
        assert!(config.use_amg_inner);
        assert!(config.dispersion_correction);
    }

    #[test]
    fn test_for_wavenumber() {
        let low = WaveHoltzConfig::for_wavenumber(5.0);
        assert_eq!(low.steps_per_period, 10);

        let mid = WaveHoltzConfig::for_wavenumber(15.0);
        assert_eq!(mid.steps_per_period, 12);

        let high = WaveHoltzConfig::for_wavenumber(25.0);
        assert_eq!(high.steps_per_period, 16);
    }
}
