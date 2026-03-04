//! TUI application state management and related types

pub(crate) mod app_autocomplete;
mod app_image;
mod app_impl;
mod app_level_meter;
mod app_library;
mod app_plugins;
mod app_scanner;
mod app_tree;
mod app_volume;
mod parameters;
#[cfg(test)]
#[path = "../tests/test_app.rs"]
mod test;
mod types;

pub use app_impl::App;
pub use parameters::{TuiEditablePlugin, TuiParamDescriptor, TuiParamSpec, TuiParamType};
pub use types::{
    ArtistNode, ChannelConflictChoice, ChannelFilter, ChannelGroup, ChannelInfo,
    ConfigureSubScreen, FilePickerMode, FilePickerOrigin, HEADPHONE_TARGET_PRESETS,
    HeadphoneEqStep, HeadphoneEqTuiState, InputMode, LibrarySortOrder, LibraryViewMode,
    MatrixEditMode, PendingParameterUpdate, QueueEntry, QueueItem, RecordingTuiState,
    ReplayGainMode, RoomEqTuiState, Screen, SpinUpdateSubStep, SpinoramaEqTuiState, SpinoramaStep,
    TreeItem,
};
