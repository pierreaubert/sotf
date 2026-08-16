use sotf_plugins::param_specs::{self};

pub(super) fn ambisonics_layouts() -> &'static [&'static str] {
    param_specs::find_by_key(param_specs::ambisonics::PARAMS, "target_layout").choice_labels()
}

pub(super) fn ambisonics_layout_to_index(layout: &str) -> f64 {
    ambisonics_layouts()
        .iter()
        .position(|&l| l == layout)
        .unwrap_or(0) as f64
}

pub(super) fn ambisonics_algorithms() -> &'static [&'static str] {
    param_specs::find_by_key(param_specs::ambisonics::PARAMS, "algorithm").choice_labels()
}

pub(super) fn ambisonics_algorithm_to_index(algorithm: &str) -> f64 {
    ambisonics_algorithms()
        .iter()
        .position(|&value| value == algorithm)
        .unwrap_or(0) as f64
}
