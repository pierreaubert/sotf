use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::{DecoderCommand, PlaybackState};
use std::sync::Arc;

/// Resume paused playback.
pub struct ResumeCommand;

impl ManagerCommandHandler for ResumeCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::debug!("[Manager Thread] Resume");

        if let Err(e) = ctx.decoder.send_command(DecoderCommand::Resume) {
            return ManagerResponse::Error(e);
        }

        match super::super::wait::wait_for_decoder_ack(
            ctx.decoder,
            std::time::Duration::from_millis(super::super::consts::DECODER_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => {
                let mut new_state = (**ctx.state.load()).clone();
                new_state.playback_state = PlaybackState::Playing;
                ctx.state.store(Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
