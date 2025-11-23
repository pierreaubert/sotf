# manager.rs

update_plugin_chain -> engine.update_plugin_chain()

# engine/audio_engine.rs

update_plugin_chain  -> send ManagerCommand::UpdatePluginChain
set_plugin_parameter -> send ManagerCommand::SetPluginParameter
reload_config        -> send ManagerCommand::ReloadConfig

# engine/manager_thread.rs

handle_config_event -> ConfigEvent::ConfigChanged(_) | ConfigEvent::Reload

handle_command      -> ManagerCommand::UpdatePluginChain -> apply_plugin_update
apply_plugin_update -> ProcessingResponse::PluginChainUpdated


# processing_thread.rs

start_reload
loop
  ProcessingCommand::UpdatePlugin
  ProcessingCommand::SetParameter



