//! Application state and logic.
//!
//! This module contains the main application state and business logic,
//! organized into submodules by functionality.

mod autocomplete;
mod level_meters;
mod library;
mod navigation;
mod plugins;
mod queue;
mod state;
mod types;

// Re-export everything publicly
pub use plugins::get_param_count;
pub use state::{App, AppState};
pub use types::{
    ArtistNode, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState, ContextMenuType,
    InputMode, LibrarySortOrder, LibraryViewMode, QueueItem, Screen, ToastMessage, ToastType,
    TreeItem,
};
