use super::default::default_area_cvar_alpha;
use super::default::default_area_gauss_legendre_points_per_axis;
use super::default::default_area_inner_maxiter;
use super::default::default_area_prior_kind;
use super::default::default_area_quadrature_kind;
use super::default::default_area_quadrature_num_points;
use super::default::default_area_quadrature_seed;
use super::default::default_area_scalarisation_kind;
use super::default::default_gaussian_truncation_sigmas;
use super::default::default_idw_power;
use serde::{Deserialize, Serialize};

/// Flat UI configuration mirror of `autoeq::roomeq::ContinuousListeningAreaConfig`.
///
/// Strings are used in place of tagged enums for ergonomic UI binding; conversion
/// happens at `to_optimizer_config()` time and is permissive on unknown values
/// (falls back to defaults rather than panicking on a stale UI string).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuousListeningAreaUiConfig {
    /// Spatial dimensions (1, 2, or 3).
    pub dimensions: usize,
    /// Per-axis bounding-box bounds `[lo, hi]`. Length must equal `dimensions`.
    pub bounds: Vec<[f64; 2]>,
    /// Spatial coordinates of each calibration seat. Outer length = number of
    /// seats, inner length = `dimensions`. Order must match seat index in the
    /// measurements array.
    pub seat_positions: Vec<Vec<f64>>,
    /// Prior kind: "uniform" or "gaussian".
    #[serde(default = "default_area_prior_kind")]
    pub prior_kind: String,
    /// Per-axis means for Gaussian prior (length must equal `dimensions`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaussian_mean: Vec<f64>,
    /// Per-axis variances for Gaussian prior (length must equal `dimensions`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaussian_cov_diag: Vec<f64>,
    /// Truncation in standard deviations for Gaussian prior.
    #[serde(default = "default_gaussian_truncation_sigmas")]
    pub gaussian_truncation_sigmas: f64,
    /// Quadrature kind: "sobol", "latin_hypercube", or "gauss_legendre".
    #[serde(default = "default_area_quadrature_kind")]
    pub quadrature_kind: String,
    /// Number of quadrature points (Sobol / Latin-Hypercube).
    #[serde(default = "default_area_quadrature_num_points")]
    pub quadrature_num_points: usize,
    /// PRNG seed for sampling-based quadratures.
    #[serde(default = "default_area_quadrature_seed")]
    pub quadrature_seed: u64,
    /// Nodes per axis for Gauss-Legendre.
    #[serde(default = "default_area_gauss_legendre_points_per_axis")]
    pub gauss_legendre_points_per_axis: usize,
    /// Scalarisation kind: "expected_value", "worst_case", or "cvar".
    #[serde(default = "default_area_scalarisation_kind")]
    pub scalarisation_kind: String,
    /// Inner-search budget for the worst-case scalarisation.
    #[serde(default = "default_area_inner_maxiter")]
    pub worst_case_inner_maxiter: usize,
    /// Inner-search seed for the worst-case scalarisation.
    #[serde(default)]
    pub worst_case_inner_seed: u64,
    /// Tail fraction for CVaR scalarisation.
    #[serde(default = "default_area_cvar_alpha")]
    pub cvar_alpha: f64,
    /// IDW power exponent for the spatial measurement interpolator.
    #[serde(default = "default_idw_power")]
    pub idw_power: f64,
}

impl Default for ContinuousListeningAreaUiConfig {
    fn default() -> Self {
        Self {
            dimensions: 2,
            bounds: vec![[0.0, 1.0], [0.0, 1.0]],
            seat_positions: Vec::new(),
            prior_kind: default_area_prior_kind(),
            gaussian_mean: Vec::new(),
            gaussian_cov_diag: Vec::new(),
            gaussian_truncation_sigmas: default_gaussian_truncation_sigmas(),
            quadrature_kind: default_area_quadrature_kind(),
            quadrature_num_points: default_area_quadrature_num_points(),
            quadrature_seed: default_area_quadrature_seed(),
            gauss_legendre_points_per_axis: default_area_gauss_legendre_points_per_axis(),
            scalarisation_kind: default_area_scalarisation_kind(),
            worst_case_inner_maxiter: default_area_inner_maxiter(),
            worst_case_inner_seed: 0,
            cvar_alpha: default_area_cvar_alpha(),
            idw_power: default_idw_power(),
        }
    }
}
