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
#[doc(hidden)]
pub fn extract_file_paths(
    params: &[ParamSpec],
    settings: &PluginSettings,
) -> HashMap<usize, String> {
    let mut file_paths = HashMap::new();
    for (i, spec) in params.iter().enumerate() {
        if matches!(spec.param_type, ParamType::FilePath) {
            // Keep the shared accessor as the source of truth. These optional
            // or directory-valued paths are legacy gaps; fixing their engine
            // accessor is intentionally deferred because engine changes
            // require a dedicated PR in this repository.
            let path = settings
                .param_value_string(i)
                .or_else(|| match settings {
                    PluginSettings::XTC { room_ir_file, .. }
                        if spec.engine_key == "room_ir_file" =>
                    {
                        room_ir_file.clone()
                    }
                    PluginSettings::BinauralDecoder {
                        hrtf_database_dir, ..
                    } if spec.engine_key == "hrtf_database_dir" => Some(hrtf_database_dir.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            file_paths.insert(i, path);
        }
    }
    file_paths
}
