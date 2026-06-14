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

mod play;
mod play_at;
mod pause;
mod resume;
mod seek;
mod stop;
mod update_plugin_chain;

pub use play::PlayCommand;
pub use play_at::PlayAtCommand;
pub use pause::PauseCommand;
pub use resume::ResumeCommand;
pub use seek::SeekCommand;
pub use stop::StopCommand;
pub use update_plugin_chain::UpdatePluginChainCommand;

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
