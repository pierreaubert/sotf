//! Core types for the TUI application state management
use sotf_audio_player::{Album, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
    Plugins,
    Devices,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    AddPlugin,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    BrowseSofaFile,
    BrowseIrFile,
    ShowHelp,
    ShowError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    Main,   // Main content area (library, queue, etc.)
    Meters, // Right column with level meters
}

/// Matrix plugin editor mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatrixEditMode {
    #[default]
    Header, // Editing input/output channels, preset
    Grid,   // Editing matrix cells
}

/// Tree view mode for library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewMode {
    Flat,     // Original list view
    TreeView, // Hierarchical artist → albums
}

/// Library sort order
pub use sotf_audio_player::library::LibrarySortOrder;

/// Channel filter options
pub use sotf_audio_player::library::ChannelFilter;

/// Artist node in tree view
#[derive(Debug, Clone)]
pub struct ArtistNode {
    pub artist: String,
    pub album_indices: Vec<usize>, // Indices into library.albums
    pub expanded: bool,
}

/// Tree item type for rendering
#[derive(Debug, Clone)]
pub enum TreeItem {
    Artist { name: String, expanded: bool },
    Album { index: usize },
}

/// Channel group for level meter display
#[derive(Debug, Clone)]
pub struct ChannelGroup {
    pub name: String,
    pub channels: Vec<ChannelInfo>,
    pub muted: bool,
    pub soloed: bool,
    pub dimmed: bool,
}

/// Individual channel information
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub index: usize,              // Index in loudness.channel_peaks
    pub name: String,              // e.g., "FL", "FR", "C"
    pub display_name: Vec<String>, // Multi-line display: ["F", "L"] or ["T", "B", "R"]
}

/// Pending parameter update for zero-dropout updates
#[derive(Debug, Clone)]
pub struct PendingParameterUpdate {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub album: Album,
    pub current_track_index: usize,
}

impl QueueItem {
    pub fn new(album: Album) -> Self {
        Self {
            album,
            current_track_index: 0,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.album.tracks.get(self.current_track_index)
    }

    pub fn next_track(&mut self) -> Option<&Track> {
        if self.current_track_index + 1 < self.album.tracks.len() {
            self.current_track_index += 1;
            self.current_track()
        } else {
            None
        }
    }

    pub fn previous_track(&mut self) -> Option<&Track> {
        if self.current_track_index > 0 {
            self.current_track_index -= 1;
            self.current_track()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub item: QueueItem,
    pub expanded: bool,
}

impl QueueEntry {
    pub fn new(item: QueueItem) -> Self {
        Self {
            item,
            expanded: false,
        }
    }
}
