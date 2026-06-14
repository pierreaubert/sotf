use super::default::default_bootstrap_alpha;
use super::default::default_bootstrap_cvar_alpha;
use super::default::default_bootstrap_num_resamples;
use super::default::default_bootstrap_scalarisation;
use super::default::default_bootstrap_seed;
use serde::{Deserialize, Serialize};

/// Flat UI mirror of `autoeq::roomeq::BootstrapUncertaintyConfig`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BootstrapUncertaintyUiConfig {
    /// Number of case-bootstrap resamples B.
    #[serde(default = "default_bootstrap_num_resamples")]
    pub num_resamples: usize,
    /// Two-sided confidence level α (used for diagnostic plots; the optimizer
    /// uses all B resamples).
    #[serde(default = "default_bootstrap_alpha")]
    pub alpha: f64,
    /// PRNG seed.
    #[serde(default = "default_bootstrap_seed")]
    pub seed: u64,
    /// Scalarisation kind: "worst_case" or "cvar".
    #[serde(default = "default_bootstrap_scalarisation")]
    pub scalarisation: String,
    /// Tail fraction for CVaR scalarisation.
    #[serde(default = "default_bootstrap_cvar_alpha")]
    pub cvar_alpha: f64,
}

impl Default for BootstrapUncertaintyUiConfig {
    fn default() -> Self {
        Self {
            num_resamples: default_bootstrap_num_resamples(),
            alpha: default_bootstrap_alpha(),
            seed: default_bootstrap_seed(),
            scalarisation: default_bootstrap_scalarisation(),
            cvar_alpha: default_bootstrap_cvar_alpha(),
        }
    }
}
