use sotf_plugins::param_specs::{self};

pub(super) fn crossover_types() -> &'static [&'static str] {
    param_specs::find_by_key(param_specs::band_split::PARAMS, "crossover_type").choice_labels()
}

pub(super) fn crossover_type_to_index(ct: &str) -> f64 {
    crossover_types()
        .iter()
        .position(|&c| c.eq_ignore_ascii_case(ct))
        .unwrap_or(0) as f64
}

pub(super) fn crossover_plugin_type_to_index(ct: &str) -> f64 {
    if ct.eq_ignore_ascii_case("linearphase")
        || ct.eq_ignore_ascii_case("linear_phase")
        || ct.eq_ignore_ascii_case("linear-phase")
    {
        1.0
    } else {
        0.0
    }
}

pub(super) fn index_to_crossover_plugin_type(index: f64) -> String {
    match index as usize {
        1 => "LinearPhase",
        _ => "LR24",
    }
    .to_string()
}

pub(super) fn crossover_output_to_index(output: &str) -> f64 {
    if output.eq_ignore_ascii_case("high") || output.eq_ignore_ascii_case("highpass") {
        1.0
    } else if output.eq_ignore_ascii_case("both") {
        2.0
    } else {
        0.0
    }
}
