use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::ProcessingCommand;

/// Poll isolated external plugin worker status without starting or restarting workers.
pub struct MaintainIsolatedExternalPluginWorkersCommand;

impl ManagerCommandHandler for MaintainIsolatedExternalPluginWorkersCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::trace!("[Manager Thread] Manual external plugin worker status poll requested");

        if let Err(e) = ctx
            .processing
            .send_command(ProcessingCommand::PollIsolatedExternalPluginWorkers)
        {
            return ManagerResponse::Error(e);
        }
        ManagerResponse::Ok
    }
}
