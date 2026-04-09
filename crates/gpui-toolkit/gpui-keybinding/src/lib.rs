//! gpui-keybinding — Reusable keybinding framework for GPUI applications.
//!
//! Provides:
//! - [`KeymapPreset`] — preset identifiers (Default, Vim, Emacs, VSCode)
//! - [`KeybindingCategory`] — categories for organizing bindings in help UI
//! - [`DocumentedKeybinding`] — human-readable binding descriptions
//! - [`KeybindingProvider`] — trait for apps to register bindings per preset
//! - [`KeybindingRegistry`] — collects bindings from multiple providers
//! - [`NavigationAction`] + [`navigation_key`] — generic navigation presets
//! - [`platform`] — platform-aware key label formatting
//! - [`conflict`] — conflict detection for duplicate key+context bindings

mod conflict;
mod platform;
mod preset;
mod provider;
mod registry;

pub mod presets;

pub use conflict::{detect_conflicts, KeyConflict};
pub use platform::{format_key_label, platform_modifier, platform_modifier_symbol};
pub use preset::KeymapPreset;
pub use presets::{navigation_key, navigation_mappings, NavigationAction, NavigationMapping};
pub use provider::{DocumentedKeybinding, KeybindingCategory, KeybindingProvider};
pub use registry::KeybindingRegistry;
