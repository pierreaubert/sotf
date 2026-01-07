// ============================================================================
// SOTF GPUI Audio Player Library
// ============================================================================
//
// This library module exposes internal types and functions for testing.
// The main application binary is in main.rs.

#![recursion_limit = "2048"]

pub mod components;

// Note: ui must be loaded before app because app re-exports from ui::components::host
pub mod app;
pub mod ui;

// Re-export modules at crate root for simpler imports
pub use app::config;
pub use app::i18n;
pub use app::keybindings;
pub use app::theme;

// Re-export commonly used types for testing
pub use app::{
    App, AppState, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState, ContextMenuType,
    InputMode, LibrarySortOrder, QueueItem, Screen, SettingsTab, ToastMessage, ToastType,
    get_param_count,
};
