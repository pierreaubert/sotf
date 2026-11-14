use crate::app::types::PluginUpdateType;

/// Convert a `PluginUpdateEffect` from the controller to GPUI's `PluginUpdateType`.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Preserve owned commands in the application state and plugin update interfaces"
)]
pub(super) fn effect_to_update_type(
    effect: sotf_audio_player::PluginUpdateEffect,
) -> Option<PluginUpdateType> {
    match effect {
        sotf_audio_player::PluginUpdateEffect::None => None,
        sotf_audio_player::PluginUpdateEffect::Structural => Some(PluginUpdateType::Structural),
        sotf_audio_player::PluginUpdateEffect::Parameter {
            plugin_index,
            param_index,
        } => Some(PluginUpdateType::Parameter {
            plugin_index,
            param_index,
        }),
        sotf_audio_player::PluginUpdateEffect::ParameterByNodeId {
            node_id,
            param_index,
        } => Some(PluginUpdateType::ParameterByNodeId {
            node_id,
            param_index,
        }),
        sotf_audio_player::PluginUpdateEffect::Action {
            plugin_index,
            node_id,
            param_index,
            key,
            value,
        } => Some(PluginUpdateType::Action {
            plugin_index,
            node_id,
            param_index,
            key,
            value,
            request_id: 0,
        }),
    }
}
