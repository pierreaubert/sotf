//! Type definitions for the GPUI audio player application.
//!
//! Contains enums and simple structs used throughout the application.

use std::time::{Duration, Instant};

use sotf_audio_player::{Album, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
    Spectrum,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayGainMode {
    Track,
    Album,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    Help,
    KeyboardShortcuts,
    About,
}

/// Active menu dropdown (if any)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMenu {
    None,
    File,
    View,
    Help,
}

/// Layout mode based on window height
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Compact,  // Below 800px - tabs bar visible
    Expanded, // Above 800px - split Library/Queue view
}

/// Settings screen tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Library,
    Appearance,
    AudioDevice,
    Plugins,
    RoomEQ,
    Headphone,
}

/// Toast message type for color coding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Success,
    Error,
    Info,
    Warning,
}

/// Toast message with type and timing
#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub message: String,
    pub toast_type: ToastType,
    pub created_at: Instant,
    pub auto_dismiss_ms: Option<u64>, // None = no auto-dismiss
}

impl ToastMessage {
    pub fn new(message: String, toast_type: ToastType) -> Self {
        Self {
            message,
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: Some(5000), // Default 5 seconds
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Success)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Error)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Info)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Warning)
    }

    pub fn persistent(message: impl Into<String>, toast_type: ToastType) -> Self {
        Self {
            message: message.into(),
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: None, // No auto-dismiss
        }
    }

    pub fn should_dismiss(&self) -> bool {
        if let Some(dismiss_ms) = self.auto_dismiss_ms {
            self.created_at.elapsed() > Duration::from_millis(dismiss_ms)
        } else {
            false
        }
    }
}

// Enums mapped from library
pub use sotf_audio_player::library::{ChannelFilter, LibrarySortOrder};

#[derive(Debug)]
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

/// Context menu state
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub menu_type: ContextMenuType,
    pub position_x: f32,
    pub position_y: f32,
    pub item_index: usize, // Index of the item that was right-clicked
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuType {
    Album,
    QueueItem,
    Plugin,
    Directory,
}
