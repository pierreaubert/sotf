//! TUI application state management and related types

mod types;
mod parameters;
mod app_impl;

pub use types::{
    ArtistNode, ChannelConflictChoice, ChannelGroup, ChannelInfo, ChannelFilter,
    ConfigureSubScreen, FocusedPane, HeadphoneEqStep, HeadphoneEqTuiState,
    InputMode, LibraryViewMode, LibrarySortOrder, MatrixEditMode,
    PendingParameterUpdate, QueueEntry, QueueItem, RecordingTuiState, ReplayGainMode,
    RoomEqTuiState, Screen, SpinoramaEqTuiState, SpinoramaStep, TreeItem,
    HEADPHONE_TARGET_PRESETS,
};
pub use parameters::{TuiEditablePlugin, TuiParamDescriptor, TuiParamSpec, TuiParamType};
pub use app_impl::App;
