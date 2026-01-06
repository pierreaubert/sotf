//! Integration tests.
//!
//! End-to-end workflows that combine multiple components:
//! - `full_playback`: Load track -> play -> seek -> pause -> stop
//! - `plugin_workflow`: Add plugins, adjust parameters, save
//! - `library_workflow`: Scan library -> browse -> select -> play

pub mod full_playback;
