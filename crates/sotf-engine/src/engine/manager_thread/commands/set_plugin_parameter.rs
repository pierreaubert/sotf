use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::ProcessingCommand;

/// Set a parameter on a plugin in the processing chain.
pub struct SetPluginParameterCommand {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

impl ManagerCommandHandler for SetPluginParameterCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::trace!(
            "[Manager Thread] Set plugin {} parameter {} = {}",
            self.plugin_index,
            self.param_id,
            self.value
        );

        let request_id = match ctx
            .processing
            .send_command(ProcessingCommand::SetParameter {
                plugin_index: self.plugin_index,
                param_id: self.param_id.clone(),
                value: self.value.clone(),
            }) {
            Ok(request_id) => request_id,
            Err(e) => return ManagerResponse::Error(e),
        };

        match super::super::wait::wait_for_parameter_update(
            ctx.processing,
            request_id,
            std::time::Duration::from_millis(super::super::consts::PROCESSING_COMMAND_TIMEOUT_MS),
        ) {
            Ok((output_channels, output_sample_rate, latency_samples)) => {
                let current = ctx.state.load();
                let old_playback_channels = current.playback_channels;
                let old_sample_rate = current.sample_rate;
                let playback_channels = if ctx.config.output_channels > 0 {
                    output_channels.min(ctx.config.output_channels)
                } else {
                    output_channels
                };
                let mut new_state = (**current).clone();
                drop(current);
                new_state.num_channels = output_channels;
                let (actual_playback_channels, actual_playback_sample_rate) = if playback_channels
                    != old_playback_channels
                    || output_sample_rate != old_sample_rate
                {
                    match ctx
                        .playback
                        .reconfigure(output_sample_rate, playback_channels)
                    {
                        Ok(actual) => (actual.channels, actual.sample_rate),
                        Err(error) => {
                            let reason = format!(
                                "Parameter update committed, but playback output reconfiguration failed: {error}"
                            );
                            new_state.num_channels = output_channels;
                            new_state.sample_rate = output_sample_rate;
                            new_state.plugin_latency_samples = latency_samples;
                            new_state.last_error = Some(reason.clone());
                            new_state.playback_state = crate::PlaybackState::Stopped;
                            ctx.state.store(std::sync::Arc::new(new_state));
                            return ManagerResponse::Error(reason);
                        }
                    }
                } else {
                    (old_playback_channels, old_sample_rate)
                };
                new_state.playback_channels = actual_playback_channels;
                new_state.sample_rate = actual_playback_sample_rate;
                new_state.plugin_latency_samples = latency_samples;
                ctx.state.store(std::sync::Arc::new(new_state));
                ManagerResponse::Ok
            }
            Err(e) => ManagerResponse::Error(e),
        }
    }
}
