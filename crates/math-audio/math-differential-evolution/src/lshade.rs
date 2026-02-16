//! L-SHADE (Linear Population Size Reduction SHADE) Configuration
//!
//! L-SHADE extends SHADE with linear population size reduction:
//! - Starts with large population for exploration
//! - Gradually reduces population for focused exploitation

/// L-SHADE specific configuration
#[derive(Debug, Clone)]
pub struct LShadeConfig {
    /// Initial population size multiplier (default: 18)
    /// NP_init = np_init * n_dim
    pub np_init: usize,

    /// Final population size (default: 4)
    pub np_final: usize,

    /// p parameter for pbest selection (default: 0.11)
    /// Selects from top p*100% of population
    pub p: f64,

    /// Archive rate (default: 2.1)
    /// Archive size = arc_rate * NP_current
    pub arc_rate: f64,

    /// Memory size for parameter adaptation (default: 6)
    pub memory_size: usize,
}

impl Default for LShadeConfig {
    fn default() -> Self {
        Self {
            np_init: 18,
            np_final: 4,
            p: 0.11,
            arc_rate: 2.1,
            memory_size: 6,
        }
    }
}

impl LShadeConfig {
    /// Compute initial population size
    pub fn initial_population_size(&self, n_dim: usize) -> usize {
        (self.np_init * n_dim).max(10)
    }

    /// Compute current population size based on progress
    /// Uses linear reduction formula from L-SHADE paper
    pub fn current_population_size(&self, n_dim: usize, nfev: usize, max_nfev: usize) -> usize {
        let np_init = self.initial_population_size(n_dim) as f64;
        let np_final = self.np_final as f64;

        let progress = (nfev as f64 / max_nfev as f64).min(1.0);
        let np = np_final + (np_init - np_final) * (1.0 - progress);

        (np.round() as usize).max(self.np_final)
    }

    /// Compute current archive size
    pub fn current_archive_size(&self, current_np: usize) -> usize {
        (self.arc_rate * current_np as f64).ceil() as usize
    }

    /// Compute pbest size for current population
    pub fn pbest_size(&self, current_np: usize) -> usize {
        ((self.p * current_np as f64).ceil() as usize)
            .max(1)
            .min(current_np)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_population_reduction() {
        let config = LShadeConfig::default();
        let n_dim = 10;

        let np_init = config.initial_population_size(n_dim);
        assert_eq!(np_init, 180);

        let np_start = config.current_population_size(n_dim, 0, 10000);
        assert!(np_start >= np_init - 1);

        let np_end = config.current_population_size(n_dim, 10000, 10000);
        assert_eq!(np_end, config.np_final);
    }

    #[test]
    fn test_archive_size() {
        let config = LShadeConfig::default();
        assert_eq!(config.current_archive_size(100), 210);
    }
}
