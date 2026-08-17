//! TUI application state management and related types

mod app_ab_testing;
pub(crate) mod app_autocomplete;
mod app_ear_training;
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

pub use app_impl::{App, FederationScanResult};
pub use parameters::{TuiEditablePlugin, TuiParamDescriptor, TuiParamSpec, TuiParamType};
pub use types::{
    AbTestingStep, AbTestingTuiState, ArtistNode, CastDeviceInfo, ChannelConflictChoice,
    ChannelFilter, ChannelGroup, ChannelInfo, ConfigureSubScreen, EarTrainingTab,
    EarTrainingTuiState, FederationEditState, FederationMode, FederationTuiState, FilePickerMode,
    FilePickerOrigin, HEADPHONE_TARGET_PRESETS, HeadphoneEqStep, HeadphoneEqTuiState, InputMode,
    LibrarySortOrder, LibraryViewMode, MatrixEditMode, MetadataEditorFields, MetadataEditorScope,
    MetadataEditorState, PendingParameterUpdate, PlaylistMode, QueueEntry, QueueItem,
    RecordingTuiState, ReplayGainMode, RoomEqTuiState, Screen, ServerSection, ServersTuiState,
    ServiceLoginEvent, ServiceLoginState, ServiceLoginStatus, SpinUpdateSubStep,
    SpinoramaEqTuiState, SpinoramaStep, Tool, TreeItem,
};
// Allow access to types submodule for full detail (SOURCE_TYPE_NAMES, etc.)
pub(crate) use types::{
    ADD_SOURCE_TYPE_IDX, RecordingField, SOURCE_TYPE_NAMES, recording_field_at,
    recording_field_count,
};
