use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::{DecoderCommand, PlaybackCommand, PlaybackState};
use std::sync::Arc;

/// Resume paused playback.
pub struct ResumeCommand;

impl ManagerCommandHandler for ResumeCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::debug!("[Manager Thread] Resume");

        let request_id = match ctx.decoder.send_command(DecoderCommand::Resume) {
            Ok(request_id) => request_id,
            Err(e) => return ManagerResponse::Error(e),
        };

        match super::super::wait::wait_for_decoder_ack(
            ctx.decoder,
            request_id,
            std::time::Duration::from_millis(super::super::consts::DECODER_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => {
                if let Err(e) = ctx.playback.send_command(PlaybackCommand::Resume) {
                    return ManagerResponse::Error(e);
                }
                let mut new_state = (**ctx.state.load()).clone();
                new_state.playback_state = PlaybackState::Playing;
                ctx.state.store(Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
