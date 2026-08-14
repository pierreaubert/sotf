use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::decoder::AudioSource;
use crate::engine::{DecoderCommand, PlaybackCommand, PlaybackState};
use std::sync::Arc;

/// Start playback of a source from the beginning.
pub struct PlayCommand(pub AudioSource);

impl ManagerCommandHandler for PlayCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let source = &self.0;
        log::debug!("[Manager Thread] Play: {}", source.display_name());

        let request_id = match ctx
            .decoder
            .send_command(DecoderCommand::Play(source.clone()))
        {
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
                new_state.current_file = source.as_path().map(|p| p.to_path_buf());
                new_state.current_source = Some(source.clone());
                new_state.playback_state = PlaybackState::Playing;
                new_state.position = 0.0;
                new_state.last_error = None;
                ctx.state.store(Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
