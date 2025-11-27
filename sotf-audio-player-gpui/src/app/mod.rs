//! Application state and logic.
//!
//! This module contains the main application state and business logic,
//! organized into submodules by functionality.
//!
//! Note: Plugin editing and level meter logic have been moved to ui/components:
//! - Plugin editing: ui/components/host/plugin_editing.rs
//! - Level meters: ui/components/plugins/level_meters.rs

mod autocomplete;
mod library;
mod navigation;
mod queue;
mod state;
pub mod types;

// Re-export everything publicly
pub use crate::ui::components::host::get_param_count;
pub use state::{App, AppState};
pub use types::{
    ActiveMenu, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState, ContextMenuType,
    InputMode, LayoutMode, LetterNode, LibrarySortOrder, LibraryViewMode, QueueItem, Screen,
    ToastMessage, ToastType, TreeItem,
};
