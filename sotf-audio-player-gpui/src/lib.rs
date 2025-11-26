// ============================================================================
// SOTF GPUI Audio Player Library
// ============================================================================
//
// This library module exposes internal types and functions for testing.
// The main application binary is in main.rs.

pub mod app;
pub mod config;

// Re-export commonly used types for testing
pub use app::{
    App, AppState, ArtistNode, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState,
    ContextMenuType, InputMode, LibrarySortOrder, LibraryViewMode, QueueItem, Screen, ToastMessage,
    ToastType, TreeItem, get_param_count,
};
