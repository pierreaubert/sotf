use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::ProcessingCommand;

/// Set a parameter on a plugin in the processing chain.
pub struct SetPluginParameterCommand {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

impl ManagerCommandHandler for SetPluginParameterCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::info!(
            "[Manager Thread] Set plugin {} parameter {} = {}",
            self.plugin_index,
            self.param_id,
            self.value
        );

        if let Err(e) = ctx.processing.send_command(ProcessingCommand::SetParameter {
            plugin_index: self.plugin_index,
            param_id: self.param_id.clone(),
            value: self.value.clone(),
        }) {
            return ManagerResponse::Error(e);
        }

        match super::super::wait::wait_for_processing_ack(
            ctx.processing,
            std::time::Duration::from_millis(super::super::consts::PROCESSING_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => ManagerResponse::Ok,
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
