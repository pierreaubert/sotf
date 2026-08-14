use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::ProcessingCommand;

/// Request data from a plugin in the processing chain.
pub struct GetPluginDataCommand(pub usize);

impl ManagerCommandHandler for GetPluginDataCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let index = self.0;
        let request_id = match ctx
            .processing
            .send_command(ProcessingCommand::GetPluginData(index))
        {
            Ok(request_id) => request_id,
            Err(e) => return ManagerResponse::Error(e),
        };

        // Wait for response from processing thread with timeout.
        // GetPluginData is time-sensitive for UI, so we wait briefly.
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(100);

        loop {
            if let Some(response) = ctx.processing.try_recv_response_for(request_id) {
                match response {
                    super::super::super::ProcessingResponse::PluginData(data) => {
                        return ManagerResponse::PluginData(data);
                    }
                    super::super::super::ProcessingResponse::Error(e) => {
                        return ManagerResponse::Error(e);
                    }
                    _ => {
                        // Ignore unexpected responses (e.g. from previous timed out requests)
                        continue;
                    }
                }
            }

            if start.elapsed() > timeout {
                ctx.processing.cancel_request(request_id);
                ctx.processing.abandon_request(request_id);
                return ManagerResponse::Error("Timeout waiting for plugin data".to_string());
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
