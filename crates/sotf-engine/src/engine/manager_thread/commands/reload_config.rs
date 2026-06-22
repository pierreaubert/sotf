use super::{ManagerCommandHandler, ManagerContext, ManagerResponse};

/// Reload the engine configuration from disk, if a config path is configured.
pub struct ReloadConfigCommand;

impl ManagerCommandHandler for ReloadConfigCommand {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse {
        log::debug!("[Manager Thread] Reload config requested");

        let Some(config_path) = ctx.config.config_path.as_ref() else {
            log::debug!("[Manager Thread] No config path set, cannot reload config");
            return ManagerResponse::Error("No config path configured".to_string());
        };

        log::debug!("[Manager Thread] Reloading config from: {:?}", config_path);

        let new_config = match super::super::config_error::load_config_file(config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                log::warn!("[Manager Thread] Config parse failed: {}", e);
                return ManagerResponse::Error(format!("Config parse failed: {}", e));
            }
        };

        match super::super::validate::validate_plugin_configs(&new_config.plugins) {
            Ok(_) => {
                log::debug!("[Manager Thread] Config validated, enqueuing plugin update");
                ctx.config_queue.enqueue(
                    new_config.plugins,
                    super::super::types::ConfigUpdatePriority::UserDirect,
                );
                ManagerResponse::Ok
            }
            Err(e) => {
                log::warn!("[Manager Thread] Config validation failed: {}", e);
                ctx.config_queue.metrics.record_rejection();
                ManagerResponse::Error(format!("Config validation failed: {}", e))
            }
        }
    }
}
