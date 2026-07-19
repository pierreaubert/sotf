use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::PluginGraphConfig;

/// Update the processing graph topology.
pub struct UpdatePluginGraphCommand(pub PluginGraphConfig);

impl ManagerCommandHandler for UpdatePluginGraphCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let graph_config = &self.0;
        log::debug!(
            "[Manager Thread] Update plugin graph ({} nodes, {} edges)",
            graph_config.nodes.len(),
            graph_config.edges.len()
        );

        match super::super::apply::apply_plugin_graph_update(
            ctx.processing,
            ctx.playback,
            ctx.state,
            graph_config.clone(),
            ctx.config.output_sample_rate,
            ctx.config.input_channels,
            ctx.config.oversampling_policy,
        ) {
            Ok(()) => ManagerResponse::Ok,
            Err(e) => {
                let message = e.to_string();
                ManagerResponse::Error(message)
            }
        }
    }
}
