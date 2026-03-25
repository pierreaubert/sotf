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
    ConfigureSubScreen, FederationEditState, FederationMode, FederationTuiState,
    FilePickerMode, FilePickerOrigin, HEADPHONE_TARGET_PRESETS,
    HeadphoneEqStep, HeadphoneEqTuiState, InputMode, LibrarySortOrder, LibraryViewMode,
    MatrixEditMode, PendingParameterUpdate, PlaylistMode, QueueEntry, QueueItem,
    RecordingTuiState, ReplayGainMode, RoomEqTuiState, Screen, ServerSection, ServersTuiState,
    SpinUpdateSubStep, SpinoramaEqTuiState, SpinoramaStep, TreeItem,
};
// Allow access to types submodule for full detail (SOURCE_TYPE_NAMES, etc.)
pub(crate) use types::{ADD_SOURCE_TYPE_IDX, SOURCE_TYPE_NAMES};
