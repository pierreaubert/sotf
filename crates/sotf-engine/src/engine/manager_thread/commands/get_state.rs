use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};

/// Return a snapshot of the current engine state.
pub struct GetStateCommand;

impl ManagerCommandHandler for GetStateCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        ManagerResponse::State((**ctx.state.load()).clone())
    }
}
