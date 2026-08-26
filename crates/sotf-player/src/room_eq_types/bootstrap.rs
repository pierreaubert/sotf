use super::bootstrap_uncertainty_ui_config::BootstrapUncertaintyUiConfig;

pub(super) fn bootstrap_uncertainty_from_backend(
    b: &autoeq::roomeq::BootstrapUncertaintyConfig,
) -> BootstrapUncertaintyUiConfig {
    BootstrapUncertaintyUiConfig {
        num_resamples: b.num_resamples,
        alpha: b.alpha,
        seed: b.seed,
        scalarisation: match b.scalarisation {
            autoeq::roomeq::BootstrapScalarisation::WorstCase => "worst_case".to_string(),
            autoeq::roomeq::BootstrapScalarisation::Cvar => "cvar".to_string(),
        },
        cvar_alpha: b.cvar_alpha,
    }
}

pub(super) fn bootstrap_uncertainty_to_backend(
    ui: &BootstrapUncertaintyUiConfig,
) -> autoeq::roomeq::BootstrapUncertaintyConfig {
    autoeq::roomeq::BootstrapUncertaintyConfig {
        num_resamples: ui.num_resamples,
        alpha: ui.alpha,
        seed: ui.seed,
        scalarisation: match ui.scalarisation.as_str() {
            "cvar" => autoeq::roomeq::BootstrapScalarisation::Cvar,
            _ => autoeq::roomeq::BootstrapScalarisation::WorstCase,
        },
        cvar_alpha: ui.cvar_alpha,
        // These nuisance and correlation adjustments are not yet editable in
        // the UI. Keep them absent rather than manufacturing measurement
        // certainty from a UI default.
        effective_spatial_sample_size: None,
        repeat_sweep_noise_std_db: None,
        calibration_uncertainty_std_db: None,
    }
}
