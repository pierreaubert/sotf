use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::DecoderCommand;

/// Cancel a previously queued next source.
pub struct CancelNextCommand;

impl ManagerCommandHandler for CancelNextCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::debug!("[Manager Thread] CancelNext");

        let request_id = match ctx.decoder.send_command(DecoderCommand::CancelNext) {
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
