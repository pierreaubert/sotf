//! TUI application state management and related types

mod types;
mod parameters;
mod app_impl;

pub use types::{
    ArtistNode, ChannelConflictChoice, ChannelGroup, ChannelInfo, ChannelFilter,
    ConfigureSubScreen, FocusedPane, InputMode, LibraryViewMode, LibrarySortOrder, MatrixEditMode,
    PendingParameterUpdate, QueueEntry, QueueItem, ReplayGainMode, Screen, SpinoramaEqTuiState,
    SpinoramaFilter, SpinoramaOptStatus, SpinoramaStep, TreeItem,
};
pub use parameters::{TuiEditablePlugin, TuiParamDescriptor, TuiParamSpec, TuiParamType};
pub use app_impl::App;
