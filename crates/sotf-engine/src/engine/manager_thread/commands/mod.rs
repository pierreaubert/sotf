//! Command objects for the engine manager thread.
//!
//! Each variant of [`ManagerCommand`](crate::engine::ManagerCommand) is handled by a small
//! struct implementing [`ManagerCommandHandler`]. This keeps `handle_command` short and makes
//! individual commands unit-testable.

use crate::engine::{
    AudioEngineState, DecoderThread, EngineConfig, ManagerResponse, PlaybackThread,
    ProcessingThread,
};
use arc_swap::ArcSwap;
use std::sync::Arc;

use super::config_update_queue::ConfigUpdateQueue;

mod bypass_processing;
mod cancel_next;
mod get_plugin_data;
mod get_position;
mod get_state;
mod maintain_isolated_external_plugin_workers;
mod mute;
mod play;
mod play_at;
mod pause;
mod queue_next;
mod reload_config;
mod resume;
mod seek;
mod set_plugin_parameter;
mod set_volume;
mod shutdown;
mod stop;
mod update_plugin_chain;
mod update_plugin_graph;

pub use bypass_processing::BypassProcessingCommand;
pub use cancel_next::CancelNextCommand;
pub use get_plugin_data::GetPluginDataCommand;
pub use get_position::GetPositionCommand;
pub use get_state::GetStateCommand;
pub use maintain_isolated_external_plugin_workers::MaintainIsolatedExternalPluginWorkersCommand;
pub use mute::MuteCommand;
pub use play::PlayCommand;
pub use play_at::PlayAtCommand;
pub use pause::PauseCommand;
pub use queue_next::QueueNextCommand;
pub use reload_config::ReloadConfigCommand;
pub use resume::ResumeCommand;
pub use seek::SeekCommand;
pub use set_plugin_parameter::SetPluginParameterCommand;
pub use set_volume::SetVolumeCommand;
pub use shutdown::ShutdownCommand;
pub use stop::StopCommand;
pub use update_plugin_chain::UpdatePluginChainCommand;
pub use update_plugin_graph::UpdatePluginGraphCommand;

/// Mutable context passed to every command handler.
pub struct ManagerContext<'a> {
    pub decoder: &'a mut DecoderThread,
    pub processing: &'a mut ProcessingThread,
    pub playback: &'a mut PlaybackThread,
    pub state: &'a Arc<ArcSwap<AudioEngineState>>,
    pub config: &'a EngineConfig,
    pub config_queue: &'a mut ConfigUpdateQueue,
}

/// Trait implemented by every manager command.
pub trait ManagerCommandHandler {
    fn execute(&self, ctx: &mut ManagerContext) -> ManagerResponse;
}
