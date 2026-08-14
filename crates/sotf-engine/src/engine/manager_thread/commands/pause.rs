use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::{DecoderCommand, PlaybackCommand, PlaybackState};
use std::sync::Arc;

/// Pause playback.
pub struct PauseCommand;

impl ManagerCommandHandler for PauseCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::debug!("[Manager Thread] Pause");

        // Flush callback-visible audio first so transport pause is immediate;
        // the decoder ACK can take substantially longer than one callback.
        if let Err(e) = ctx.playback.send_command(PlaybackCommand::Pause) {
            return ManagerResponse::Error(e);
        }

        let request_id = match ctx.decoder.send_command(DecoderCommand::Pause) {
            Ok(request_id) => request_id,
            Err(e) => {
                ctx.playback.send_command(PlaybackCommand::Resume).ok();
                return ManagerResponse::Error(e);
            }
        };

        match super::super::wait::wait_for_decoder_ack(
            ctx.decoder,
            request_id,
            std::time::Duration::from_millis(super::super::consts::DECODER_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => {
                let mut new_state = (**ctx.state.load()).clone();
                new_state.playback_state = PlaybackState::Paused;
                ctx.state.store(Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) => {
                ctx.playback.send_command(PlaybackCommand::Resume).ok();
                ManagerResponse::Error(e)
            }
        }
    }
}
