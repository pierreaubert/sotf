use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::{DecoderCommand, PlaybackCommand, PlaybackState};
use std::sync::Arc;

/// Stop playback and clear the current source.
pub struct StopCommand;

impl ManagerCommandHandler for StopCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::debug!("[Manager Thread] Stop");

        if let Err(e) = ctx.decoder.send_command(DecoderCommand::Stop) {
            return ManagerResponse::Error(e);
        }
        if let Err(e) = super::super::wait::wait_for_decoder_ack(
            ctx.decoder,
            std::time::Duration::from_millis(super::super::consts::DECODER_COMMAND_TIMEOUT_MS),
        ) {
            return ManagerResponse::Error(e);
        }

        // Best-effort: the playback thread may have already exited after
        // end-of-stream drain. This is expected during auto-advance.
        if let Err(e) = ctx.playback.send_command(PlaybackCommand::Stop) {
            log::debug!(
                "[Manager Thread] Stop send to playback failed (already exited): {}",
                e
            );
        }

        let mut new_state = (**ctx.state.load()).clone();
        new_state.playback_state = PlaybackState::Stopped;
        new_state.current_file = None;
        new_state.current_source = None;
        new_state.position = 0.0;
        new_state.seeking = false;
        new_state.output_peak_linear = 0.0;
        new_state.output_clipping_detected = false;
        ctx.state.store(Arc::new(new_state));

        ManagerResponse::Ok
    }
}
