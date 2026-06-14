use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::decoder::AudioSource;
use crate::engine::{DecoderCommand, PlaybackState};
use std::sync::Arc;

/// Start playback of a source at a specific position (in seconds).
pub struct PlayAtCommand(pub AudioSource, pub f64);

impl ManagerCommandHandler for PlayAtCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let source = &self.0;
        let position = self.1;
        log::debug!(
            "[Manager Thread] PlayAt: {} at {:.2}s",
            source.display_name(),
            position
        );

        if let Err(e) = ctx
            .decoder
            .send_command(DecoderCommand::PlayAt(source.clone(), position))
        {
            return ManagerResponse::Error(e);
        }

        match super::super::wait::wait_for_decoder_ack(
            ctx.decoder,
            std::time::Duration::from_millis(super::super::consts::DECODER_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => {
                let mut new_state = (**ctx.state.load()).clone();
                new_state.current_file = source.as_path().map(|p| p.to_path_buf());
                new_state.current_source = Some(source.clone());
                new_state.playback_state = PlaybackState::Playing;
                new_state.position = position;
                ctx.state.store(Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
