use crate::theme::Theme;
use sotf_audio_player::PluginSettings;
use sotf_audio_player::controllers::plugin::{desktop_param_indices, desktop_param_spec};
use sotf_audio_player_midi::MidiMappingEngine;
use sotf_audio_player_midi::auto_map;
use sotf_audio_player_midi::layout::ControllerLayout;
use sotf_audio_player_midi::layouts::{lcxl_layout, xone_k2_layout};
use sotf_audio_player_midi::mapping::MidiMapping;
use sotf_plugins::param_specs::{ParamSpec, ParamType};

/// Strip the inner-chassis from knob/slider widgets used inside the
/// controller-view cells. The cell already provides a rounded card with
/// border and label — switching the inner widget to `LABEL_UNDERLINED`
/// removes the duplicate chassis that otherwise sits inside it (the
/// "misaligned boxes" effect).
pub(super) fn cell_inner_theme(theme: &Theme) -> Theme {
    let mut out = theme.clone();
    out.layout.design_tokens.knob_label_style =
        gpui_audio_kit::audio_design_tokens::AudioDesignTokens::LABEL_UNDERLINED;
    out.layout.design_tokens.meter_label_style =
        gpui_audio_kit::audio_design_tokens::AudioDesignTokens::LABEL_UNDERLINED;
    out
}

/// Resolve a controller id (as exposed by `available_controllers`) to its
/// `ControllerLayout`. Returns `None` for unknown ids.
pub(super) fn layout_for_controller_id(controller_id: &str) -> Option<ControllerLayout> {
    match controller_id {
        "xone_k2" => Some(xone_k2_layout()),
        "lcxl" => Some(lcxl_layout()),
        _ => None,
    }
}

/// Build the `MidiMapping` to display: prefer the engine's live mapping if
/// it targets this controller and plugin, otherwise synthesize one with the
/// auto-mapper so the view is meaningful even without a connected device.
pub(super) fn mapping_for_view(
    layout: &ControllerLayout,
    settings: &PluginSettings,
    plugin_index: usize,
    engine: &MidiMappingEngine,
) -> MidiMapping {
    let plugin_type_name = settings.plugin_type().name().to_string();

    if let Some(mapping) = engine.mapping()
        && mapping.controller_name == layout.name
        && mapping.plugin_type == plugin_type_name
        && mapping
            .bindings
            .iter()
            .all(|binding| binding.plugin_index == plugin_index)
    {
        return mapping.clone();
    }

    desktop_mapping(layout, settings, plugin_index, &plugin_type_name)
}

/// Adapt the MIDI allocator's dense positions to actual desktop parameter addresses.
fn desktop_mapping(
    layout: &ControllerLayout,
    settings: &PluginSettings,
    plugin_index: usize,
    plugin_type_name: &str,
) -> MidiMapping {
    let parameters: Vec<_> = desktop_param_indices(settings)
        .into_iter()
        .filter_map(|index| desktop_param_spec(settings, index).map(|spec| (index, *spec)))
        .collect();
    let specs: Vec<_> = parameters.iter().map(|(_, spec)| *spec).collect();
    let mut mapping = auto_map::auto_map(layout, &specs, plugin_index, plugin_type_name);
    // The allocator indexes the slice it receives. Persisted/live mappings already
    // use their own addresses and are deliberately not reinterpreted here.
    mapping.bindings.retain_mut(|binding| {
        let Some((index, _)) = parameters.get(binding.param_index) else {
            return false;
        };
        binding.param_index = *index;
        true
    });
    mapping
}

#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
pub(super) fn continuous_range(spec: &ParamSpec) -> Option<(f64, f64)> {
    match spec.param_type {
        ParamType::Float { min, max, .. } => Some((min, max)),
        ParamType::Int { min, max, .. } => Some((min as f64, max as f64)),
        ParamType::Bool { .. } => Some((0.0, 1.0)),
        ParamType::Choice { labels, .. } => {
            Some((0.0, labels.len().saturating_sub(1).max(1) as f64))
        }
        ParamType::FilePath => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio_player::PluginType;

    #[test]
    fn synthesized_controller_mapping_keeps_sparse_desktop_addresses() {
        let layout = lcxl_layout();
        for kind in [
            PluginType::EQ,
            PluginType::LinearPhaseEq,
            PluginType::DynamicEq,
            PluginType::MultibandCompressor,
            PluginType::MultibandExpander,
        ] {
            let settings = PluginSettings::default_for(&kind).unwrap();
            let mapping = desktop_mapping(&layout, &settings, 3, kind.name());
            assert!(!mapping.bindings.is_empty(), "{kind:?}");
            for binding in &mapping.bindings {
                assert_eq!(binding.plugin_index, 3);
                assert!(
                    desktop_param_spec(&settings, binding.param_index).is_some(),
                    "{kind:?}: {}",
                    binding.param_index
                );
            }
            if matches!(
                kind,
                PluginType::LinearPhaseEq
                    | PluginType::DynamicEq
                    | PluginType::MultibandCompressor
                    | PluginType::MultibandExpander
            ) {
                assert!(
                    mapping
                        .bindings
                        .iter()
                        .any(|binding| binding.param_index >= 100),
                    "{kind:?}: missing sparse band/global addresses"
                );
            }
        }
    }
}
