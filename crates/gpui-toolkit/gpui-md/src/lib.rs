//! gpui-md — Markdown editor with live preview for GPUI applications.
//!
//! A full-featured markdown editor demonstrating the gpui-toolkit ecosystem:
//! - GitHub Flavored Markdown (GFM) via comrak
//! - Split-pane editor + live preview
//! - Ropey-backed document buffer with undo/redo
//! - Source map for click-to-locate (preview → editor)
//! - Platform-specific keybindings via gpui-keybinding
//! - Theme support via gpui-themes
//! - Import/export: Word (.docx), PDF, Google Docs

pub mod actions;
pub mod commands;
pub mod dired;
pub mod document;
pub mod export;
pub mod import;
pub mod keybindings;
pub mod macros;
pub mod markdown;
pub mod minibuffer;
pub mod state;
pub mod views;

pub use keybindings::MdKeybindingProvider;
pub use state::MdAppState;
pub use views::main_view::MainView;
