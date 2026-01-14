//! Domain-separated state modules.
//!
//! This module contains focused state structs that can be used as GPUI Entities.
//! The goal is to separate concerns and allow independent observation/subscription.

pub mod app;
pub mod audio_device;
pub mod input;
pub mod library;
pub mod measurement;
pub mod playback;
pub mod playback_events;
pub mod plugin;
pub mod shared;
pub mod ui;

pub use app::{App, AppState, DividerDragState, DividerType, WorkflowNodeMapping};
pub use audio_device::AudioDeviceState;
pub use input::InputState;
pub use playback_events::{PlaybackEvent, PlaybackEventStore, TrackChangeTrigger};
pub use library::LibraryState;
pub use measurement::MeasurementState;
pub use playback::PlaybackState;
pub use plugin::{PluginState, PluginViewMode};
pub use shared::SharedState;
pub use ui::UIState;


