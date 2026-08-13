# manager.rs

update_plugin_chain -> engine.update_plugin_chain()

# engine/audio_engine.rs

update_plugin_chain  -> send ManagerCommand::UpdatePluginChain
set_plugin_parameter -> send ManagerCommand::SetPluginParameter
reload_config        -> send ManagerCommand::ReloadConfig

# engine/manager_thread.rs

handle_config_event -> ConfigEvent::ConfigChanged(_) | ConfigEvent::Reload

handle_command      -> ManagerCommand::UpdatePluginChain -> apply_plugin_update
apply_plugin_update -> PreparedHostUpdate::prepare -> ProcessingCommand::CommitHostUpdate
                    -> ProcessingResponse::PluginChainUpdated


# processing_thread.rs

start_reload
loop
  ProcessingCommand::CommitHostUpdate
  ProcessingCommand::SetParameter

Structural host updates are built and fully allocation-prepared on the
manager/control side. The processing thread validates the candidate against
the output-channel/latency snapshot it was based on, then commits it at a block
boundary. A stale candidate is retired without touching the active host.

For equal output rates, old and new hosts run concurrently during a 50 ms
crossfade and the shorter-latency path passes through its preallocated delay
line before mixing. Rate-changing replacements transition through silence.
Completed and rolled-back host states are sent to the bounded GC queue, so the
processing path neither allocates transition storage nor destroys plugin state.


