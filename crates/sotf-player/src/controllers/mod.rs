//! Domain controllers that encapsulate shared business logic.
//!
//! These controllers own state and orchestrate operations so that UIs
//! (GPUI, TUI, etc.) become thin wrappers that delegate business operations
//! and only manage UI-specific state.

pub mod library;
pub mod playback;
pub mod playlist;
pub mod plugin;
pub mod plugin_param_map;
pub mod queue;
pub mod scan;

pub use library::LibraryController;
pub use playback::PlaybackController;
pub use playlist::PlaylistController;
pub use plugin::{PluginController, PluginUpdateEffect, get_param_count};
pub use plugin_param_map::param_index_to_engine_param;
pub use queue::{QueueController, QueuePlaybackEffect};
pub use scan::ScanController;
