//! Application state and logic.
//!
//! This module contains the main application state and business logic,
//! organized into submodules by functionality.
//!
//! Note: Plugin editing and level meter logic have been moved to ui/components:
//! - Plugin editing: ui/components/host/plugin_editing.rs
//! - Level meters: ui/components/plugins/level_meters.rs

pub mod actions;
mod autocomplete;
pub mod cast;
pub mod config;
pub mod constants;
pub mod debug;
#[cfg(feature = "dev-api")]
pub mod dev_api;
pub mod federation;
pub mod i18n;
pub mod keybindings;
pub mod library;
pub mod manager;
pub mod midi_input;
pub mod navigation;
pub mod queue;
pub mod remote;
pub mod state;
pub mod theme;
pub mod types;

pub use i18n::{Language, Translations};
pub use keybindings::{
    DocumentedKeybinding, KeybindingCategory, KeymapPreset, get_documented_keybindings,
    get_keybindings,
};
pub use state::{App, AppState};
pub use theme::{CommunityThemeId, Theme, ThemeId};

// Re-export everything publicly
pub use crate::components::plugins::get_param_count;
pub use types::{
    ActiveMenu, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState, ContextMenuType,
    InputMode, LayoutMode, LayoutOrientation, LibrarySortOrder, MetadataEditorFields,
    MetadataEditorScope, MetadataEditorState, MeterDisplayMode, PhoneHomeShelf, PlatformStyle,
    PluginViewMode, QueueItem, RackDisplayMode, ReplayGainMode, Screen, SettingsTab, ToastAction,
    ToastMessage, ToastType,
};
