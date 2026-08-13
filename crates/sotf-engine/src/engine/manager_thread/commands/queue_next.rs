use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::decoder::AudioSource;
use crate::engine::DecoderCommand;

/// Queue the next source for gapless playback.
pub struct QueueNextCommand(pub AudioSource);

impl ManagerCommandHandler for QueueNextCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let source = self.0.clone();
        log::debug!("[Manager Thread] QueueNext: {}", source.display_name());

        if let Err(e) = super::super::validate::validate_gapless_source_compatible(
            &source,
            ctx.config.input_channels,
        ) {
            return ManagerResponse::Error(e);
        }

        let request_id = match ctx.decoder.send_command(DecoderCommand::QueueNext(source)) {
            Ok(request_id) => request_id,
            Err(e) => return ManagerResponse::Error(e),
        };

        match super::super::wait::wait_for_decoder_ack(
            ctx.decoder,
            request_id,
            std::time::Duration::from_millis(super::super::consts::DECODER_COMMAND_TIMEOUT_MS),
        ) {
            Ok(()) => ManagerResponse::Ok,
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
