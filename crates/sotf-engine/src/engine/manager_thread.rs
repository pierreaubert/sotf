use super::{
    AudioEngineState, EngineConfig, ManagerCommand, ManagerResponse, PlaybackState, PluginDataCache,
};
use arc_swap::ArcSwap;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

pub(super) struct ManagerRequest {
    pub(super) id: u64,
    pub(super) command: ManagerCommand,
}

pub(super) struct ManagerReply {
    pub(super) id: u64,
    pub(super) response: ManagerResponse,
}

mod apply;
mod commands;
mod config_error;
mod config_update_metrics;
mod config_update_queue;
mod consts;
mod error;
mod estimate;
mod handle;
mod misc;
mod state_helpers;
#[cfg(test)]
mod tests;
mod thread_event_visitor;
mod types;
mod validate;
mod wait;

use config_update_queue::run_manager_thread;

/// Manager thread handle
pub struct ManagerThread {
    command_tx: Sender<ManagerRequest>,
    response_inbox: Mutex<ManagerResponseInbox>,
    next_request_id: AtomicU64,
    state: Arc<ArcSwap<AudioEngineState>>,
    plugin_data_cache: PluginDataCache,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

struct ManagerResponseInbox {
    rx: Receiver<ManagerReply>,
    buffered: HashMap<u64, ManagerResponse>,
    abandoned: HashSet<u64>,
}

impl ManagerThread {
    /// Create and start the manager thread
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = channel();
        let (response_tx, response_rx) = channel();

        let state = Arc::new(ArcSwap::from_pointee(AudioEngineState::default()));
        let state_clone = Arc::clone(&state);

        let plugin_data_cache: PluginDataCache =
            Arc::new(arc_swap::ArcSwap::from_pointee(Vec::new()));
        let cache_clone = Arc::clone(&plugin_data_cache);

        let thread_handle = std::thread::Builder::new()
            .name("manager".to_string())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_manager_thread(
                        config,
                        command_rx,
                        response_tx,
                        state_clone.clone(),
                        cache_clone,
                    )
                }));
                let failure = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(format!("Engine manager exited with an error: {error}")),
                    Err(_) => Some("Engine manager panicked".to_string()),
                };
                if let Some(failure) = failure {
                    log::error!("[Manager Thread] {failure}");
                    state_helpers::update_engine_state(&state_clone, |new_state| {
                        new_state.last_error = Some(failure);
                        new_state.playback_state = PlaybackState::Stopped;
                    });
                }
            })
            .map_err(|e| format!("Failed to spawn manager thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_inbox: Mutex::new(ManagerResponseInbox {
                rx: response_rx,
                buffered: HashMap::new(),
                abandoned: HashSet::new(),
            }),
            next_request_id: AtomicU64::new(1),
            state,
            plugin_data_cache,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the manager
    pub fn send_command(&self, command: ManagerCommand) -> Result<u64, String> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        self.command_tx
            .send(ManagerRequest { id, command })
            .map_err(|e| format!("Failed to send command: {}", e))?;
        Ok(id)
    }

    pub fn recv_response_for(
        &self,
        request_id: u64,
        timeout: std::time::Duration,
    ) -> Result<ManagerResponse, String> {
        let deadline = std::time::Instant::now() + timeout;
        let mut inbox = self
            .response_inbox
            .lock()
            .map_err(|e| format!("Failed to lock response inbox: {e}"))?;
        if let Some(response) = inbox.buffered.remove(&request_id) {
            return Ok(response);
        }
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                inbox.abandoned.insert(request_id);
                return Err(format!(
                    "Timed out waiting for manager request {request_id}"
                ));
            }
            let reply = match inbox.rx.recv_timeout(remaining) {
                Ok(reply) => reply,
                Err(error) => {
                    inbox.abandoned.insert(request_id);
                    return Err(format!("Failed to receive response: {error}"));
                }
            };
            if inbox.abandoned.remove(&reply.id) {
                continue;
            }
            if reply.id == request_id {
                return Ok(reply.response);
            }
            inbox.buffered.insert(reply.id, reply.response);
        }
    }

    pub(in crate::engine) fn wait_for_exit(
        &self,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self
                .thread_handle
                .as_ref()
                .is_none_or(std::thread::JoinHandle::is_finished)
            {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err("Timed out waiting for engine manager cleanup".to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Get current state (lock-free)
    pub fn get_state(&self) -> AudioEngineState {
        (**self.state.load()).clone()
    }

    /// Get just the current playback state without cloning the full state.
    pub fn get_playback_state(&self) -> PlaybackState {
        self.state.load().playback_state.clone()
    }

    /// Get cached plugin data directly (no command round-trip).
    /// The processing thread updates this cache after every frame.
    pub fn get_cached_plugin_data(&self, index: usize) -> Option<Arc<dyn Any + Send + Sync>> {
        let cache = self.plugin_data_cache.load();
        cache.get(index).and_then(|slot| slot.clone())
    }

    /// Shutdown the manager thread
    pub fn shutdown(&mut self) {
        let already_finished = self
            .thread_handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished);
        if !already_finished && let Err(e) = self.send_command(ManagerCommand::Shutdown) {
            log::trace!("[Manager Thread] Shutdown command receiver dropped: {e}");
        }
        if let Some(handle) = self.thread_handle.take()
            && super::join_timeout(handle, std::time::Duration::from_secs(10)).is_err()
        {
            log::warn!("[Manager Thread] Shutdown join timed out; thread left detached");
        }
    }
}

impl Drop for ManagerThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}
