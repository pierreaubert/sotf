use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::{DecoderCommand, PlaybackCommand, PlaybackState, ProcessingCommand};
use std::sync::Arc;

/// Shut down all engine threads.
pub struct ShutdownCommand;

impl ManagerCommandHandler for ShutdownCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::debug!("[Manager Thread] Shutdown requested");

        {
            let mut new_state = (**ctx.state.load()).clone();
            new_state.playback_state = PlaybackState::Stopped;
            ctx.state.store(Arc::new(new_state));
        }

        // Signal threads to shutdown
        if let Err(e) = ctx.decoder.send_command(DecoderCommand::Shutdown) {
            log::trace!("[Manager Thread] Decoder shutdown command dropped: {}", e);
        }
        if let Err(e) = ctx.processing.send_command(ProcessingCommand::Shutdown) {
            log::trace!(
                "[Manager Thread] Processing shutdown command dropped: {}",
                e
            );
        }
        if let Err(e) = ctx.playback.send_command(PlaybackCommand::Shutdown) {
            log::trace!("[Manager Thread] Playback shutdown command dropped: {}", e);
        }

        ManagerResponse::Shutdown
    }
}
