use super::{DecoderCommand, DecoderMessage, DecoderResponse, ThreadEvent};
use crate::DsdOutputMode;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender};

mod consts;
mod decoder_state;
mod hal_input_guard_trip;
mod misc;
mod sample_queue;
#[cfg(test)]
mod tests;
mod types;

use types::run_decoder_thread;

const DECODER_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Decoder thread handle
pub struct DecoderThread {
    command_tx: Sender<DecoderCommand>,
    response_inbox: Mutex<DecoderResponseInbox>,
    next_request_id: AtomicU64,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

struct DecoderResponseInbox {
    rx: Receiver<DecoderResponse>,
    pending: VecDeque<u64>,
    buffered: HashMap<u64, DecoderResponse>,
    abandoned: HashSet<u64>,
}

impl DecoderThread {
    /// Create and start the decoder thread
    pub fn new(
        message_tx: SyncSender<DecoderMessage>,
        event_tx: crossbeam::channel::Sender<ThreadEvent>,
        target_sample_rate: u32,
        frame_size: usize,
        recycle_rx: Receiver<Vec<f32>>,
        dsd_output: DsdOutputMode,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (response_tx, response_rx) = std::sync::mpsc::channel();

        let thread_handle = std::thread::Builder::new()
            .name("decoder".to_string())
            .spawn(move || {
                let error_tx = event_tx.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_decoder_thread(
                        message_tx,
                        command_rx,
                        response_tx,
                        event_tx,
                        target_sample_rate,
                        frame_size,
                        recycle_rx,
                        dsd_output,
                    )
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        log::error!("[Decoder Thread] Error: {}", e);
                        let _ = error_tx.try_send(ThreadEvent::DecoderError(format!(
                            "Decoder thread exited with an error: {e}"
                        )));
                    }
                    Err(_) => {
                        log::error!("[Decoder Thread] Panicked");
                        let _ = error_tx.try_send(ThreadEvent::ThreadPanic("decoder".to_string()));
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn decoder thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_inbox: Mutex::new(DecoderResponseInbox {
                rx: response_rx,
                pending: VecDeque::new(),
                buffered: HashMap::new(),
                abandoned: HashSet::new(),
            }),
            next_request_id: AtomicU64::new(1),
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the decoder thread
    pub fn send_command(&self, command: DecoderCommand) -> Result<u64, String> {
        let expects_response = !matches!(
            command,
            DecoderCommand::StartSilentSource(_) | DecoderCommand::Shutdown
        );
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let mut inbox = self
            .response_inbox
            .lock()
            .map_err(|e| format!("Failed to lock decoder response inbox: {e}"))?;
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {e}"))?;
        if expects_response {
            inbox.pending.push_back(request_id);
        }
        Ok(request_id)
    }

    pub fn try_recv_response_for(&self, request_id: u64) -> Option<DecoderResponse> {
        let mut inbox = self.response_inbox.lock().ok()?;
        if let Some(response) = inbox.buffered.remove(&request_id) {
            return Some(response);
        }

        while let Ok(response) = inbox.rx.try_recv() {
            let Some(response_id) = inbox.pending.pop_front() else {
                log::warn!("[Decoder Thread] Received an acknowledgement with no pending request");
                continue;
            };
            if inbox.abandoned.remove(&response_id) {
                continue;
            }
            if response_id == request_id {
                return Some(response);
            }
            inbox.buffered.insert(response_id, response);
        }
        None
    }

    pub fn abandon_request(&self, request_id: u64) {
        if let Ok(mut inbox) = self.response_inbox.lock() {
            inbox.buffered.remove(&request_id);
            if inbox.pending.contains(&request_id) {
                inbox.abandoned.insert(request_id);
            }
        }
    }

    /// Whether the worker has stopped before the manager requested shutdown.
    pub fn is_finished(&self) -> bool {
        self.thread_handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
    }

    /// Shutdown the decoder thread
    pub fn shutdown(&mut self) {
        self.shutdown_with_timeout(DECODER_SHUTDOWN_TIMEOUT);
    }

    fn shutdown_with_timeout(&mut self, timeout: std::time::Duration) {
        self.send_command(DecoderCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take()
            && super::join_timeout(handle, timeout).is_err()
        {
            log::warn!(
                "[Decoder Thread] Shutdown join timed out after {:?}; thread left detached",
                timeout
            );
        }
    }
}

impl Drop for DecoderThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}
