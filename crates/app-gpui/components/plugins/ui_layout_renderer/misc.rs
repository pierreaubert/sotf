use sotf_audio_player::PluginSettings;
use sotf_plugins::layout_solver::KnobSize;
use sotf_plugins::param_specs::{ParamSpec, ParamType};
use sotf_plugins::plugin_layout::*;
use std::collections::HashMap;

pub(super) const AUTO_COLUMN_MIN_SIDE_WIDTH: f32 = 180.0;

pub(super) fn control_column_width(knob_size: KnobSize) -> f32 {
    match knob_size {
        KnobSize::Xs => 130.0,
        KnobSize::Sm => 150.0,
        KnobSize::Md => 170.0,
    }
}

pub(super) fn visible_control_count(group: &ControlGroup) -> usize {
    group.controls.iter().filter(|c| !c.hidden).count()
}

/// Extract file path strings from PluginSettings for FilePath params.
pub(super) fn extract_file_paths(
    params: &[ParamSpec],
    settings: &PluginSettings,
) -> HashMap<usize, String> {
    let mut file_paths = HashMap::new();
    for (i, spec) in params.iter().enumerate() {
        if matches!(spec.param_type, ParamType::FilePath) {
            let path = match settings {
                PluginSettings::BinauralDecoder { sofa_file, .. }
                    if spec.engine_key == "sofa_file" =>
                {
                    sofa_file.clone()
                }
                PluginSettings::Convolution { ir_file, .. } if spec.engine_key == "ir_file" => {
                    ir_file.clone()
                }
                PluginSettings::XTC { room_ir_file, .. } if spec.engine_key == "room_ir_file" => {
                    room_ir_file.clone().unwrap_or_default()
                }
                PluginSettings::ABCompare {
                    path_a_config,
                    path_b_config,
                    ..
                } => {
                    if spec.engine_key == "path_a_config" {
                        path_a_config.clone()
                    } else if spec.engine_key == "path_b_config" {
                        path_b_config.clone()
                    } else {
                        String::new()
                    }
                }
                _ => String::new(),
            };
            file_paths.insert(i, path);
        }
    }
    file_paths
}
