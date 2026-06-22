use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::ProcessingCommand;
use std::sync::Arc;

/// Bypass or re-enable the processing chain.
pub struct BypassProcessingCommand(pub bool);

impl ManagerCommandHandler for BypassProcessingCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let bypass = self.0;
        log::debug!("[Manager Thread] Bypass processing: {}", bypass);

        if let Err(e) = ctx
            .processing
            .send_command(ProcessingCommand::Bypass(bypass))
        {
            return ManagerResponse::Error(e);
        }

        match super::super::wait::wait_for_processing_ack(
            ctx.processing,
            std::time::Duration::from_millis(super::super::consts::PROCESSING_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => {
                let mut new_state = (**ctx.state.load()).clone();
                new_state.processing_bypassed = bypass;
                ctx.state.store(Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
