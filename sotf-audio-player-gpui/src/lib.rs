// ============================================================================
// SOTF GPUI Audio Player Library
// ============================================================================
//
// This library module exposes internal types and functions for testing.
// The main application binary is in main.rs.

pub mod actions;
pub mod config;
pub mod i18n;
pub mod keybindings;
pub mod optimization_params;
pub mod theme;
// Note: ui must be loaded before app because app re-exports from ui::components::host
pub mod ui;
pub mod app;

// Re-export commonly used types for testing
pub use app::{
    App, AppState, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState,
    ContextMenuType, InputMode, LetterNode, LibrarySortOrder, LibraryViewMode, QueueItem, Screen,
    ToastMessage, ToastType, TreeItem, get_param_count,
};
pub use i18n::{Language, Translations};
pub use keybindings::{KeymapPreset, get_keybindings, get_documented_keybindings, DocumentedKeybinding, KeybindingCategory};
pub use theme::{Theme, ThemeId};
