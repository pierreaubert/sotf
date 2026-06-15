//! UI-agnostic view models shared between GPUI and TUI shells.
//!
//! The modules here hold cross-platform screen state and user-intent events
//! so that `app-gpui` and `app-tui` remain thin rendering layers.

pub mod headphone_eq;
pub mod recording;
pub mod room_eq;
pub mod spinorama_eq;
