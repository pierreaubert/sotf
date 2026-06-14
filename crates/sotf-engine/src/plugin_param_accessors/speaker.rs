use sotf_plugins::param_specs::{self};

pub(super) fn speaker_configs() -> &'static [&'static str] {
    param_specs::find_by_key(param_specs::upmixer::PARAMS, "speaker_config").choice_labels()
}

pub(super) fn speaker_config_to_index(config: &str) -> f64 {
    speaker_configs()
        .iter()
        .position(|&c| c == config)
        .unwrap_or(2) as f64 // default to 5.1
}
