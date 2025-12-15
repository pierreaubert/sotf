// ============================================================================
// SOTF GPUI Audio Player Library
// ============================================================================
//
// This library module exposes internal types and functions for testing.
// The main application binary is in main.rs.

#![recursion_limit = "1024"]

pub mod actions;
pub mod components;
pub mod config;
pub mod i18n;
pub mod keybindings;
pub mod plugins;
pub mod screens;
pub mod state;
pub mod theme;

// Note: ui must be loaded before app because app re-exports from ui::components::host
pub mod app;
pub mod ui;

// Re-export commonly used types for testing
pub use app::{
    App, AppState, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState, ContextMenuType,
    InputMode, LibrarySortOrder, QueueItem, Screen, SettingsTab, ToastMessage, ToastType,
    get_param_count,
};
pub use i18n::{Language, Translations};
pub use keybindings::{
    DocumentedKeybinding, KeybindingCategory, KeymapPreset, get_documented_keybindings,
    get_keybindings,
};
pub use theme::{Theme, ThemeId};

// Re-export autoeq and optimization_params at crate level for convenience
pub use components::autoeq;
pub use components::optimization_params;
