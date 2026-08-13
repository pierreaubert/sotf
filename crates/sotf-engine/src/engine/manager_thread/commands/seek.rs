use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::{DecoderCommand, PlaybackState};
use std::sync::Arc;

/// Seek to a position (in seconds) in the current source.
pub struct SeekCommand(pub f64);

impl ManagerCommandHandler for SeekCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let position = self.0;
        log::debug!("[Manager Thread] Seek to {:.2}s", position);

        let request_id = match ctx.decoder.send_command(DecoderCommand::Seek(position)) {
            Ok(request_id) => request_id,
            Err(e) => return ManagerResponse::Error(e),
        };

        match super::super::wait::wait_for_decoder_ack(
            ctx.decoder,
            request_id,
            std::time::Duration::from_millis(super::super::consts::DECODER_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => {
                let mut new_state = (**ctx.state.load()).clone();
                new_state.position = position;
                new_state.seeking = true;
                ctx.state.store(Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) if e == "No decoder" => {
                let current = ctx.state.load();
                let Some(source) = current.current_source.clone() else {
                    return ManagerResponse::Error(e);
                };
                if current.playback_state == PlaybackState::Stopped {
                    return ManagerResponse::Error(e);
                }
                drop(current);

                log::debug!(
                    "[Manager Thread] Seek found no active decoder; reopening current source at {:.2}s",
                    position
                );
                let play_request_id = match ctx
                    .decoder
                    .send_command(DecoderCommand::PlayAt(source.clone(), position))
                {
                    Ok(request_id) => request_id,
                    Err(play_err) => return ManagerResponse::Error(play_err),
                };

                match super::super::wait::wait_for_decoder_ack(
                    ctx.decoder,
                    play_request_id,
                    std::time::Duration::from_millis(
                        super::super::consts::DECODER_COMMAND_TIMEOUT_MS,
                    ),
                ) {
                    Ok(()) => {
                        let mut new_state = (**ctx.state.load()).clone();
                        new_state.current_file = source.as_path().map(|p| p.to_path_buf());
                        new_state.current_source = Some(source);
                        new_state.playback_state = PlaybackState::Playing;
                        new_state.position = position;
                        new_state.seeking = true;
                        ctx.state.store(Arc::new(new_state));
                        ManagerResponse::Ok
                    }
                    Err(play_err) => ManagerResponse::Error(play_err),
                }
            }
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
