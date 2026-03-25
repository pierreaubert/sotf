//! Common GPUI rendering infrastructure for SOTF audio plugin UIs.
//!
//! This crate provides the shared abstractions and rendering helpers that enable
//! the same plugin UI code to work in both:
//! - The full GPUI player app (app-gpui)
//! - Standalone macOS Audio Unit plugin views (plugins-au)
//!
//! # Architecture
//!
//! Plugin UIs are generic over the [`PluginViewHost`] trait, which abstracts
//! parameter access and UI state management. Each host environment implements
//! this trait (e.g., `AppState` in app-gpui, `AuHostState` in plugins-ffi).
//!
//! Shared rendering helpers (knobs, sliders, toggles, meters) live here.
//! Plugin-specific UIs live in each plugin crate's `ui` module behind the
//! `gpui-ui` feature flag.

pub mod common;
pub mod design_tokens;
mod host;
pub mod meter_theme;
mod theme;
pub mod ticks;

pub use design_tokens::audio_tokens_from_ds;
pub use host::PluginViewHost;
pub use meter_theme::{LufsConfig, MeterTheme, TruePeakConfig};
pub use theme::*;
pub use ticks::{ScaleType, TickConfig, render_tick_row};
