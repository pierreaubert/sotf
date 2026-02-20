//! TUI application state management and related types

mod types;
mod parameters;
mod app_impl;

pub use types::{
    ArtistNode, ChannelGroup, ChannelInfo, ChannelFilter, FocusedPane, InputMode, LibraryViewMode,
    LibrarySortOrder, MatrixEditMode, PendingParameterUpdate, QueueEntry, QueueItem, Screen,
    TreeItem,
};
pub use parameters::{TuiEditablePlugin, TuiParamDescriptor, TuiParamSpec, TuiParamType};
pub use app_impl::App;
