// ============================================================================
// SOTF GPUI Audio Player Library
// ============================================================================
//
// This library module exposes internal types and functions for testing.
// The main application binary is in main.rs.

#![recursion_limit = "8192"]

pub mod components;

// Note: ui must be loaded before app because app re-exports from ui::components::host
pub mod app;
pub mod ui;

// Re-export modules at crate root for simpler imports
pub use app::config;
pub use app::i18n;
pub use app::keybindings;
pub use app::theme;

// Re-export commonly used types for testing
pub use app::{
    App, AppState, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState, ContextMenuType,
    InputMode, LayoutMode, LibrarySortOrder, QueueItem, Screen, SettingsTab, ToastMessage,
    ToastType, get_param_count,
};

// Re-export additional types for testing
pub use app::types::{
    CalibrationData, ChannelMapping, ChannelMeasurement, ChannelRecording, ChannelRecordingState,
    CrossoverType, HeadphoneEqStep, LibraryStats, MeasureState, MeterDisplayMode, MultiSpeakerMode,
    PlaybackDeviceConfig, PlotSmoothing, PluginViewMode, RecordingDeviceConfig, RecordingResult,
    RecordingSignalType, RecordingState, RecordingStep, ReplayGainMode, RoomEqAlgorithm,
    RoomEqMeasurementsFile, RoomEqOptimizationMode, RoomEqOptimizerConfig, RoomEqSpeakerConfig,
    RoomEqState, RoomEqStep, SpeakerConfigType, SpeakerConfiguration,
};

// Re-export config types for testing
pub use app::config::{Config, PanelLayout, RecordingConfigState, WindowGeometry};

// Re-export state types for testing
pub use app::state::playback::PlaybackState;

// Re-export component types for testing
pub use components::home::image_cache::{ImageAccessTracker, TrackerStats};
pub use components::icons::{Icon, IconName, IconSize};
pub use components::plugins::ticks::{ScaleType, TickConfig, TickMark};
