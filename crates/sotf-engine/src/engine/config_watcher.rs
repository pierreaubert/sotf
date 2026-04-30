// ============================================================================
// Config Watcher - File Watching and Signal Handling
// ============================================================================
//
// Watches for config file changes and Unix signals, notifying manager thread.
//
// Features:
// - File watching (cross-platform via notify crate)
// - Unix signals: SIGHUP (reload), SIGTERM/SIGINT (shutdown)
// - Windows: File watching only (no signal support)

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

const SPIN_MS_DELAY_WATCHER: u64 = 300;
const DEBOUNCE_MS: u64 = 300; // Wait 300ms after last file change before triggering reload

/// Config watcher events
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Config file changed - reload requested
    ConfigChanged(PathBuf),
    /// Shutdown signal received (SIGTERM, SIGINT, Ctrl-C)
    Shutdown,
    /// Reload signal received (SIGHUP on Unix)
    Reload,
}

/// Config watcher handle
pub struct ConfigWatcher {
    event_rx: Receiver<ConfigEvent>,
    shutdown_tx: Option<Sender<()>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl ConfigWatcher {
    /// Create and start a config watcher
    ///
    /// # Arguments
    /// - `config_path`: Optional path to config file to watch
    /// - `watch_signals`: Whether to watch Unix signals (SIGHUP, SIGTERM, SIGINT)
    pub fn new(config_path: Option<PathBuf>, watch_signals: bool) -> Result<Self, String> {
        let (event_tx, event_rx) = channel();
        let (shutdown_tx, shutdown_rx) = channel();
        let shutdown_tx_thread = shutdown_tx.clone();
        let (startup_tx, startup_rx) = channel();

        let thread_handle = thread::Builder::new()
            .name("config-watcher".to_string())
            .spawn(move || {
                if let Err(e) = run_config_watcher(
                    config_path,
                    watch_signals,
                    event_tx,
                    shutdown_tx_thread,
                    shutdown_rx,
                    startup_tx,
                ) {
                    log::debug!("[Config Watcher] Error: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn config watcher thread: {}", e))?;

        match startup_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                thread_handle.join().ok();
                return Err(e);
            }
            Err(_) => {
                thread_handle.join().ok();
                return Err("Config watcher thread exited before startup completed".to_string());
            }
        }

        Ok(Self {
            event_rx,
            shutdown_tx: Some(shutdown_tx),
            thread_handle: Some(thread_handle),
        })
    }

    /// Try to receive a config event (non-blocking)
    pub fn try_recv(&self) -> Option<ConfigEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Shutdown the watcher
    pub fn shutdown(&mut self) {
        // Signal the thread to exit
        if let Some(tx) = self.shutdown_tx.take() {
            tx.send(()).ok();
        }

        // Wait for thread to exit
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Main config watcher thread function
fn run_config_watcher(
    config_path: Option<PathBuf>,
    watch_signals: bool,
    event_tx: Sender<ConfigEvent>,
    shutdown_tx: Sender<()>,
    shutdown_rx: Receiver<()>,
    startup_tx: Sender<Result<(), String>>,
) -> Result<(), String> {
    log::debug!("[Config Watcher] Starting");
    log::debug!("[Config Watcher]   Config file: {:?}", config_path);
    log::debug!("[Config Watcher]   Watch signals: {}", watch_signals);

    // Setup file watcher if config path provided
    let _file_watcher = if let Some(ref path) = config_path {
        match setup_file_watcher(path.clone(), event_tx.clone()) {
            Ok(watcher) => Some(watcher),
            Err(e) => {
                startup_tx.send(Err(e.clone())).ok();
                return Err(e);
            }
        }
    } else {
        None
    };

    // Setup signal handler if requested (Unix only)
    #[cfg(unix)]
    let signal_flags = if watch_signals {
        match setup_signal_handler() {
            Ok(flags) => Some(flags),
            Err(e) => {
                startup_tx.send(Err(e.clone())).ok();
                return Err(e);
            }
        }
    } else {
        None
    };

    #[cfg(not(unix))]
    if watch_signals {
        log::debug!("[Config Watcher] Warning: Signal watching not supported on this platform");
    }

    log::debug!("[Config Watcher] Ready");
    startup_tx.send(Ok(())).ok();

    // Main loop - check for signals and shutdown requests
    loop {
        // Check for Unix signals (non-blocking)
        #[cfg(unix)]
        if let Some(ref flags) = signal_flags {
            if flags.shutdown.load(Ordering::Relaxed) {
                log::debug!("[Config Watcher] Shutdown signal received (SIGTERM/SIGINT)");
                event_tx.send(ConfigEvent::Shutdown).ok();
                shutdown_tx.send(()).ok();
                break;
            }
            if flags.reload.load(Ordering::Relaxed) {
                log::debug!("[Config Watcher] Reload signal received (SIGHUP)");
                event_tx.send(ConfigEvent::Reload).ok();
                // Reset flag so we can detect future signals
                flags.reload.store(false, Ordering::Relaxed);
            }
        }

        // Check for shutdown request from parent (with short timeout for responsiveness)
        match shutdown_rx.recv_timeout(Duration::from_millis(SPIN_MS_DELAY_WATCHER)) {
            Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::debug!("[Config Watcher] Shutting down");
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Continue waiting
            }
        }
    }

    Ok(())
}

/// Setup file watcher using notify crate with debouncing
fn setup_file_watcher(
    config_path: PathBuf,
    event_tx: Sender<ConfigEvent>,
) -> Result<notify::RecommendedWatcher, String> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

    log::debug!("[Config Watcher] Watching file: {:?}", config_path);

    let event_config_path = config_path.clone();
    let match_config_path = normalized_config_path(&config_path);
    let config_path_clone = match_config_path.clone();

    // Debouncing state: tracks the last event time and if event is pending
    #[derive(Clone, Copy)]
    struct DebounceState {
        last_event_time: Instant,
        event_pending: bool,
        last_sent_time: Instant,
    }

    let debounce_state = Arc::new(Mutex::new(DebounceState {
        last_event_time: Instant::now(),
        event_pending: false,
        last_sent_time: Instant::now(),
    }));

    let debounce_event_tx = event_tx.clone();
    let debounce_config_path = event_config_path;

    // Spawn a debouncing thread that checks periodically
    let debounce_state_clone = Arc::clone(&debounce_state);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(DEBOUNCE_MS / 2));

            let should_send = {
                let state = debounce_state_clone.lock().unwrap();
                let elapsed_since_event = state.last_event_time.elapsed().as_millis() as u64;
                let elapsed_since_sent = state.last_sent_time.elapsed().as_millis() as u64;

                // Send if:
                // 1. There's a pending event
                // 2. Enough time has passed since the last file change
                // 3. We haven't sent an event recently (avoid duplicates)
                state.event_pending
                    && elapsed_since_event >= DEBOUNCE_MS
                    && elapsed_since_sent >= DEBOUNCE_MS
            };

            if should_send {
                log::debug!("[Config Watcher] Debounce period elapsed, triggering reload");
                if debounce_event_tx
                    .send(ConfigEvent::ConfigChanged(debounce_config_path.clone()))
                    .is_err()
                {
                    // Channel closed, exit thread
                    break;
                }

                // Mark event as sent
                let mut state = debounce_state_clone.lock().unwrap();
                state.event_pending = false;
                state.last_sent_time = Instant::now();
            }
        }
    });

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if !event_paths_match_config(&event, &config_path_clone) {
                        return;
                    }

                    if is_config_change_event(event.kind) {
                        log::debug!(
                            "[Config Watcher] File changed: {:?} (debouncing)",
                            config_path_clone
                        );
                        // Update the debounce state
                        let mut state = debounce_state.lock().unwrap();
                        state.last_event_time = Instant::now();
                        state.event_pending = true;
                    }
                }
                Err(e) => {
                    log::debug!("[Config Watcher] Watch error: {}", e);
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("Failed to create file watcher: {}", e))?;

    let Some(parent) = match_config_path.parent() else {
        return Err("Invalid config path".to_string());
    };

    if match_config_path.exists() {
        watcher
            .watch(&match_config_path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch config path: {}", e))?;
    } else {
        log::info!(
            "[Config Watcher] File doesn't exist, watching parent directory: {:?}",
            parent
        );
    }

    // Also watch the parent directory and filter events for the config path.
    // Some editors replace files atomically via parent-directory operations.
    let watch_path = parent.to_path_buf();

    watcher
        .watch(&watch_path, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch path: {}", e))?;

    Ok(watcher)
}

fn normalized_config_path(config_path: &Path) -> PathBuf {
    if let Ok(canonical_path) = config_path.canonicalize() {
        return canonical_path;
    }

    let Some(parent) = config_path.parent() else {
        return config_path.to_path_buf();
    };
    let Some(file_name) = config_path.file_name() else {
        return config_path.to_path_buf();
    };

    parent
        .canonicalize()
        .map(|canonical_parent| canonical_parent.join(file_name))
        .unwrap_or_else(|_| config_path.to_path_buf())
}

fn event_paths_match_config(event: &notify::Event, config_path: &Path) -> bool {
    event.paths.is_empty()
        || event
            .paths
            .iter()
            .any(|path| path_matches_config(path, config_path))
}

fn path_matches_config(path: &Path, config_path: &Path) -> bool {
    if path == config_path
        || path
            .canonicalize()
            .is_ok_and(|canonical_path| canonical_path == config_path)
    {
        return true;
    }

    if path.file_name() != config_path.file_name() {
        return false;
    }

    match (path.parent(), config_path.parent()) {
        (Some(path_parent), Some(config_parent)) => {
            path_parent == config_parent
                || path_parent
                    .canonicalize()
                    .is_ok_and(|canonical_parent| canonical_parent == config_parent)
        }
        _ => false,
    }
}

fn is_config_change_event(kind: notify::EventKind) -> bool {
    use notify::EventKind;
    use notify::event::{AccessKind, AccessMode};

    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Remove(_)
            | EventKind::Access(AccessKind::Close(
                AccessMode::Any | AccessMode::Write | AccessMode::Other
            ))
    )
}

/// Signal handler flags
#[cfg(unix)]
struct SignalFlags {
    shutdown: Arc<AtomicBool>,
    reload: Arc<AtomicBool>,
}

/// Setup Unix signal handler using flag-based approach
#[cfg(unix)]
fn setup_signal_handler() -> Result<SignalFlags, String> {
    use signal_hook::consts::signal::*;
    use signal_hook::flag;

    log::debug!("[Config Watcher] Setting up signal handlers (SIGHUP, SIGTERM, SIGINT)");

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Register signal handlers using signal-hook
    flag::register(SIGTERM, Arc::clone(&shutdown))
        .map_err(|e| format!("Failed to register SIGTERM handler: {}", e))?;

    flag::register(SIGINT, Arc::clone(&shutdown))
        .map_err(|e| format!("Failed to register SIGINT handler: {}", e))?;

    flag::register(SIGHUP, Arc::clone(&reload))
        .map_err(|e| format!("Failed to register SIGHUP handler: {}", e))?;

    log::debug!("[Config Watcher] Signal handlers registered successfully");

    Ok(SignalFlags { shutdown, reload })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    const SPIN_MS_INIT_WATCHER: u64 = 100;
    const SPIN_MS_SLEEP_WATCHER: u64 = 1000;

    #[test]
    fn test_file_watcher_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        // Create initial file
        fs::write(&config_path, "initial: value").unwrap();

        // Start watcher
        let watcher = ConfigWatcher::new(Some(config_path.clone()), false).unwrap();

        // Give watcher time to initialize
        thread::sleep(Duration::from_millis(SPIN_MS_INIT_WATCHER));

        // Modify file
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&config_path)
            .unwrap();
        file.write_all(b"updated: value").unwrap();
        file.sync_all().unwrap();
        drop(file);

        let deadline = Instant::now() + Duration::from_millis(SPIN_MS_SLEEP_WATCHER * 5);
        let mut event = None;
        while Instant::now() < deadline {
            if let Some(next_event) = watcher.try_recv() {
                event = Some(next_event);
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(event.is_some());
        match event.unwrap() {
            ConfigEvent::ConfigChanged(path) => {
                assert_eq!(path, config_path);
            }
            _ => panic!("Expected ConfigChanged event"),
        }
    }
}
