use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::PlaybackCommand;
use std::sync::Arc;

/// Set the playback volume.
pub struct SetVolumeCommand(pub f32);

impl ManagerCommandHandler for SetVolumeCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let volume = self.0;
        log::debug!("[Manager Thread] Set volume: {:.2}", volume);

        {
            let mut new_state = (**ctx.state.load()).clone();
            new_state.volume = volume;
            ctx.state.store(Arc::new(new_state));
        }

        // Best-effort: the playback thread may have already exited after
        // end-of-stream drain. The volume is stored in state and will be
        // applied when the next engine starts.
        if let Err(e) = ctx.playback.send_command(PlaybackCommand::SetVolume(volume)) {
            log::debug!("[Manager Thread] SetVolume send failed (playback ended): {}", e);
        }

        ManagerResponse::Ok
    }
}
