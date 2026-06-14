//! Playlist controller — manages playlist state and operations.
//!
//! Delegates persistence to `MusicDatabase` and provides navigation,
//! CRUD operations, and track resolution for UI layers.

mod playlist_controller;

pub use playlist_controller::*;
