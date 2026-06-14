use super::continuous_listening_area_ui_config::ContinuousListeningAreaUiConfig;
use super::default::default_area_cvar_alpha;
use super::default::default_area_gauss_legendre_points_per_axis;
use super::default::default_area_inner_maxiter;
use super::default::default_area_quadrature_num_points;
use super::default::default_area_quadrature_seed;
use super::default::default_gaussian_truncation_sigmas;

pub(super) fn continuous_area_from_backend(
    a: &autoeq::roomeq::ContinuousListeningAreaConfig,
) -> ContinuousListeningAreaUiConfig {
    let (prior_kind, gaussian_mean, gaussian_cov_diag, gaussian_truncation_sigmas) = match &a.prior
    {
        autoeq::roomeq::AreaPriorKind::Uniform => (
            "uniform".to_string(),
            Vec::new(),
            Vec::new(),
            default_gaussian_truncation_sigmas(),
        ),
        autoeq::roomeq::AreaPriorKind::Gaussian {
            mean,
            cov_diag,
            truncation_sigmas,
        } => (
            "gaussian".to_string(),
            mean.clone(),
            cov_diag.clone(),
            *truncation_sigmas,
        ),
    };

    let (quadrature_kind, quadrature_num_points, quadrature_seed, gauss_legendre_points_per_axis) =
        match &a.quadrature {
            autoeq::roomeq::AreaQuadratureKind::Sobol { num_points, seed } => (
                "sobol".to_string(),
                *num_points,
                *seed,
                default_area_gauss_legendre_points_per_axis(),
            ),
            autoeq::roomeq::AreaQuadratureKind::LatinHypercube { num_points, seed } => (
                "latin_hypercube".to_string(),
                *num_points,
                *seed,
                default_area_gauss_legendre_points_per_axis(),
            ),
            autoeq::roomeq::AreaQuadratureKind::GaussLegendre { points_per_axis } => (
                "gauss_legendre".to_string(),
                default_area_quadrature_num_points(),
                default_area_quadrature_seed(),
                *points_per_axis,
            ),
        };

    let (scalarisation_kind, worst_case_inner_maxiter, worst_case_inner_seed, cvar_alpha) =
        match &a.scalarisation {
            autoeq::roomeq::AreaScalarisationKind::ExpectedValue => (
                "expected_value".to_string(),
                default_area_inner_maxiter(),
                0,
                default_area_cvar_alpha(),
            ),
            autoeq::roomeq::AreaScalarisationKind::WorstCase {
                inner_maxiter,
                inner_seed,
            } => (
                "worst_case".to_string(),
                *inner_maxiter,
                *inner_seed,
                default_area_cvar_alpha(),
            ),
            autoeq::roomeq::AreaScalarisationKind::Cvar { alpha } => {
                ("cvar".to_string(), default_area_inner_maxiter(), 0, *alpha)
            }
        };

    ContinuousListeningAreaUiConfig {
        dimensions: a.dimensions,
        bounds: a.bounds.iter().map(|(lo, hi)| [*lo, *hi]).collect(),
        seat_positions: a.seat_positions.clone(),
        prior_kind,
        gaussian_mean,
        gaussian_cov_diag,
        gaussian_truncation_sigmas,
        quadrature_kind,
        quadrature_num_points,
        quadrature_seed,
        gauss_legendre_points_per_axis,
        scalarisation_kind,
        worst_case_inner_maxiter,
        worst_case_inner_seed,
        cvar_alpha,
        idw_power: a.idw_power,
    }
}

pub(super) fn continuous_area_to_backend(
    ui: &ContinuousListeningAreaUiConfig,
) -> autoeq::roomeq::ContinuousListeningAreaConfig {
    let prior = match ui.prior_kind.as_str() {
        "gaussian" => autoeq::roomeq::AreaPriorKind::Gaussian {
            mean: ui.gaussian_mean.clone(),
            cov_diag: ui.gaussian_cov_diag.clone(),
            truncation_sigmas: ui.gaussian_truncation_sigmas,
        },
        _ => autoeq::roomeq::AreaPriorKind::Uniform,
    };

    let quadrature = match ui.quadrature_kind.as_str() {
        "latin_hypercube" => autoeq::roomeq::AreaQuadratureKind::LatinHypercube {
            num_points: ui.quadrature_num_points,
            seed: ui.quadrature_seed,
        },
        "gauss_legendre" => autoeq::roomeq::AreaQuadratureKind::GaussLegendre {
            points_per_axis: ui.gauss_legendre_points_per_axis,
        },
        _ => autoeq::roomeq::AreaQuadratureKind::Sobol {
            num_points: ui.quadrature_num_points,
            seed: ui.quadrature_seed,
        },
    };

    let scalarisation = match ui.scalarisation_kind.as_str() {
        "worst_case" => autoeq::roomeq::AreaScalarisationKind::WorstCase {
            inner_maxiter: ui.worst_case_inner_maxiter,
            inner_seed: ui.worst_case_inner_seed,
        },
        "cvar" => autoeq::roomeq::AreaScalarisationKind::Cvar {
            alpha: ui.cvar_alpha,
        },
        _ => autoeq::roomeq::AreaScalarisationKind::ExpectedValue,
    };

    autoeq::roomeq::ContinuousListeningAreaConfig {
        dimensions: ui.dimensions,
        bounds: ui.bounds.iter().map(|b| (b[0], b[1])).collect(),
        seat_positions: ui.seat_positions.clone(),
        prior,
        quadrature,
        scalarisation,
        idw_power: ui.idw_power,
    }
}
