//! Library controller — owns library state and encapsulates filtering, sorting,
//! pagination, navigation, directory management, and scanning.
//!
//! Lifted from app-gpui's `LibraryState` with GPUI-specific patterns removed.

mod library_controller;
mod misc;

pub use library_controller::*;
