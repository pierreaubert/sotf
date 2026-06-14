use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};

/// Return the current playback position.
pub struct GetPositionCommand;

impl ManagerCommandHandler for GetPositionCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        ManagerResponse::Position(ctx.state.load().position)
    }
}
