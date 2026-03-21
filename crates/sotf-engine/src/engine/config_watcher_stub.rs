// ============================================================================
// Config Watcher - iOS Stub
// ============================================================================
//
// On iOS, there's no file watching or Unix signal handling.
// This stub provides the same API surface so the engine compiles.

use std::sync::mpsc::{Receiver, channel};

/// Config watcher events (same as desktop)
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    ConfigChanged(std::path::PathBuf),
    Shutdown,
    Reload,
}

/// Config watcher handle (iOS no-op)
pub struct ConfigWatcher {
    event_rx: Receiver<ConfigEvent>,
}

impl ConfigWatcher {
    pub fn new(
        _config_path: Option<std::path::PathBuf>,
        _watch_signals: bool,
    ) -> Result<Self, String> {
        let (_event_tx, event_rx) = channel();
        log::debug!("[Config Watcher iOS] No-op watcher created (file watching not available on iOS)");
        Ok(Self { event_rx })
    }

    pub fn try_recv(&self) -> Option<ConfigEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn shutdown(&mut self) {
        // No-op
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}
