//! TUI application state management and related types

mod app_impl;
mod parameters;
mod types;

pub use app_impl::App;
pub use parameters::{TuiEditablePlugin, TuiParamDescriptor, TuiParamSpec, TuiParamType};
pub use types::{
    ArtistNode, ChannelConflictChoice, ChannelFilter, ChannelGroup, ChannelInfo,
    ConfigureSubScreen, FocusedPane, HEADPHONE_TARGET_PRESETS, HeadphoneEqStep,
    HeadphoneEqTuiState, InputMode, LibrarySortOrder, LibraryViewMode, MatrixEditMode,
    PendingParameterUpdate, QueueEntry, QueueItem, RecordingTuiState, ReplayGainMode,
    RoomEqTuiState, Screen, SpinUpdateSubStep, SpinoramaEqTuiState, SpinoramaStep, TreeItem,
};
