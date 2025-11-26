//! Abstract operations for state-behavior equivalence testing.
//!
//! These operations represent user actions that should produce equivalent
//! state changes in both TUI and GPUI implementations.

use super::comparable_state::{
    ChannelFilterId, InputModeId, PluginTypeId, ScreenId, SortOrderId, ViewModeId,
};

/// Abstract operation that can be applied to both TUI and GPUI apps
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    // Navigation
    SwitchScreen(ScreenId),
    SetInputMode(InputModeId),
    ExitInputMode, // Returns to Normal mode

    // Library navigation
    SelectNextAlbum,
    SelectPreviousAlbum,
    SelectAlbumAtIndex(usize),
    PageDown,
    PageUp,

    // Library configuration
    SetSearchQuery(String),
    ClearSearch,
    CycleSortOrder,
    SetSortOrder(SortOrderId),
    CycleChannelFilter,
    SetChannelFilter(ChannelFilterId),
    ToggleViewMode,
    SetViewMode(ViewModeId),

    // Queue management
    AddSelectedAlbumToQueue,
    AddAlbumToQueueAtIndex(usize),
    RemoveFromQueue(usize),
    ClearQueue,
    SelectNextQueueItem,
    SelectPreviousQueueItem,
    MoveQueueItemUp,
    MoveQueueItemDown,

    // Playback (state-only, not actual audio)
    Play,
    Pause,
    TogglePlayback,
    Stop,
    NextTrack,
    PreviousTrack,
    SetVolume(f32),
    VolumeUp,
    VolumeDown,

    // Plugin management
    AddPlugin(PluginTypeId),
    RemovePlugin(usize),
    TogglePlugin(usize),
    SelectNextPlugin,
    SelectPreviousPlugin,
    EnterPluginEdit,
    ExitPluginEdit,
    MovePluginUp,
    MovePluginDown,

    // Directory management
    SelectNextDirectory,
    SelectPreviousDirectory,
    RemoveSelectedDirectory,

    // Device management
    SelectNextDevice,
    SelectPreviousDevice,
    SelectDevice(usize),
}

/// Sequence of operations for testing
#[derive(Debug, Clone)]
pub struct OperationSequence {
    pub name: String,
    pub operations: Vec<Operation>,
}

impl OperationSequence {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            operations: Vec::new(),
        }
    }

    pub fn then(mut self, op: Operation) -> Self {
        self.operations.push(op);
        self
    }

    pub fn then_many(mut self, ops: impl IntoIterator<Item = Operation>) -> Self {
        self.operations.extend(ops);
        self
    }
}

/// Pre-defined test sequences for common workflows
pub mod sequences {
    use super::*;

    /// Basic library browsing: navigate through albums
    pub fn library_browsing() -> OperationSequence {
        OperationSequence::new("library_browsing")
            .then(Operation::SwitchScreen(ScreenId::Library))
            .then(Operation::SelectNextAlbum)
            .then(Operation::SelectNextAlbum)
            .then(Operation::SelectNextAlbum)
            .then(Operation::SelectPreviousAlbum)
    }

    /// Search workflow: enter search mode, type query, exit
    pub fn search_workflow() -> OperationSequence {
        OperationSequence::new("search_workflow")
            .then(Operation::SwitchScreen(ScreenId::Library))
            .then(Operation::SetInputMode(InputModeId::Search))
            .then(Operation::SetSearchQuery("test".into()))
            .then(Operation::ExitInputMode)
    }

    /// Queue management: add items, navigate, remove
    pub fn queue_management() -> OperationSequence {
        OperationSequence::new("queue_management")
            .then(Operation::SwitchScreen(ScreenId::Library))
            .then(Operation::AddSelectedAlbumToQueue)
            .then(Operation::SelectNextAlbum)
            .then(Operation::AddSelectedAlbumToQueue)
            .then(Operation::SwitchScreen(ScreenId::Queue))
            .then(Operation::SelectNextQueueItem)
            .then(Operation::RemoveFromQueue(0))
    }

    /// Plugin chain: add plugins, toggle, reorder
    pub fn plugin_chain_management() -> OperationSequence {
        OperationSequence::new("plugin_chain_management")
            .then(Operation::SwitchScreen(ScreenId::Plugins))
            .then(Operation::AddPlugin(PluginTypeId::Gain))
            .then(Operation::AddPlugin(PluginTypeId::EQ))
            .then(Operation::AddPlugin(PluginTypeId::Compressor))
            .then(Operation::SelectNextPlugin)
            .then(Operation::TogglePlugin(1))
            .then(Operation::MovePluginUp)
    }

    /// Sort order cycling
    pub fn sort_order_cycling() -> OperationSequence {
        OperationSequence::new("sort_order_cycling")
            .then(Operation::SwitchScreen(ScreenId::Library))
            .then(Operation::CycleSortOrder)
            .then(Operation::CycleSortOrder)
            .then(Operation::CycleSortOrder)
    }

    /// Channel filter cycling
    pub fn channel_filter_cycling() -> OperationSequence {
        OperationSequence::new("channel_filter_cycling")
            .then(Operation::SwitchScreen(ScreenId::Library))
            .then(Operation::CycleChannelFilter)
            .then(Operation::CycleChannelFilter)
            .then(Operation::CycleChannelFilter)
    }

    /// Screen navigation: visit all screens
    pub fn screen_navigation() -> OperationSequence {
        OperationSequence::new("screen_navigation")
            .then(Operation::SwitchScreen(ScreenId::Library))
            .then(Operation::SwitchScreen(ScreenId::Queue))
            .then(Operation::SwitchScreen(ScreenId::Plugins))
            .then(Operation::SwitchScreen(ScreenId::DirectoryManager))
            .then(Operation::SwitchScreen(ScreenId::Devices))
            .then(Operation::SwitchScreen(ScreenId::Library))
    }

    /// Volume control
    pub fn volume_control() -> OperationSequence {
        OperationSequence::new("volume_control")
            .then(Operation::SetVolume(0.5))
            .then(Operation::VolumeUp)
            .then(Operation::VolumeUp)
            .then(Operation::VolumeDown)
    }
}
