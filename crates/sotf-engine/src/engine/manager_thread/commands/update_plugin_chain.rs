use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};
use crate::engine::PluginConfig;

/// Replace the active plugin chain.
pub struct UpdatePluginChainCommand(pub Vec<PluginConfig>);

impl ManagerCommandHandler for UpdatePluginChainCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        let plugins = &self.0;
        log::debug!(
            "[Manager Thread] Update plugin chain ({} plugins)",
            plugins.len()
        );
        log::trace!(
            "[Manager Thread] UpdatePluginChain: Validating configuration with {} plugins",
            plugins.len()
        );

        // Validate config before processing
        if let Err(e) = super::super::validate::validate_plugin_configs(plugins) {
            log::warn!(
                "[Manager Thread] Plugin configuration validation failed: {}",
                e
            );
            ctx.config_queue.metrics.record_rejection();
            return ManagerResponse::Error(e.to_string());
        }

        log::trace!("[Manager Thread] UpdatePluginChain: Configuration validated successfully");

        // If a config update is already in progress, enqueue this one
        if ctx.config_queue.is_processing() {
            log::debug!(
                "[Manager Thread] Config update ALREADY IN PROGRESS - queuing (this may cause channel mismatch!)"
            );
            log::trace!(
                "[Manager Thread] UpdatePluginChain: Queueing update (queue size before: {})",
                ctx.config_queue.queue.len()
            );
            let queued = ctx.config_queue.enqueue(
                plugins.clone(),
                super::super::types::ConfigUpdatePriority::UserDirect,
            );
            if queued {
                log::debug!(
                    "[Manager Thread] UpdatePluginChain: Update QUEUED (not applied immediately)"
                );
                return ManagerResponse::Ok;
            } else {
                log::warn!(
                    "[Manager Thread] UpdatePluginChain: Failed to queue update (queue full)"
                );
                return ManagerResponse::Error("Plugin update queue is full".to_string());
            }
        }

        log::debug!("[Manager Thread] UpdatePluginChain: Applying update immediately (not queued)");

        // Otherwise, apply immediately using the synchronized apply function
        match super::super::apply::apply_plugin_update(
            ctx.processing,
            ctx.playback,
            ctx.state,
            ctx.config_queue,
            plugins.clone(),
            ctx.config.output_sample_rate,
            ctx.config.input_channels,
            ctx.config.oversampling_policy,
        ) {
            Ok(()) => {
                log::trace!("[Manager Thread] UpdatePluginChain: Update applied successfully");
                ManagerResponse::Ok
            }
            Err(e) => {
                let message = e.to_string();
                log::trace!("[Manager Thread] UpdatePluginChain: Update failed: {}", e);
                ManagerResponse::Error(message)
            }
        }
    }
}
