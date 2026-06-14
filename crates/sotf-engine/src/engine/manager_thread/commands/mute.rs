use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::PlaybackCommand;
use std::sync::Arc;

/// Mute or unmute playback.
pub struct MuteCommand(pub bool);

impl ManagerCommandHandler for MuteCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let muted = self.0;
        log::debug!("[Manager Thread] Mute: {}", muted);

        {
            let mut new_state = (**ctx.state.load()).clone();
            new_state.muted = muted;
            ctx.state.store(Arc::new(new_state));
        }

        if let Err(e) = ctx.playback.send_command(PlaybackCommand::Mute(muted)) {
            log::debug!("[Manager Thread] Mute send failed (playback ended): {}", e);
        }

        ManagerResponse::Ok
    }
}
