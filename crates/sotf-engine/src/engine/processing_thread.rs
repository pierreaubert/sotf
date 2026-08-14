use super::{
    DecoderMessage, ProcessingCommand, ProcessingMessage, ProcessingResponse, ThreadEvent,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender};

mod build;
mod isolated;
mod misc;
mod processing_state;
#[cfg(test)]
mod tests;

pub use build::*;

use processing_state::run_processing_thread;

#[derive(Debug)]
pub(super) struct ProcessingRequest {
    pub(super) id: u64,
    pub(super) command: ProcessingCommand,
    pub(super) ticket: ProcessingCommandTicket,
}

pub(super) struct ProcessingReply {
    pub(super) id: u64,
    pub(super) response: ProcessingResponse,
}

const PROCESSING_REQUEST_PENDING: u8 = 0;
const PROCESSING_REQUEST_CLAIMED: u8 = 1;
const PROCESSING_REQUEST_CANCELLED: u8 = 2;

#[derive(Clone, Debug)]
pub(super) struct ProcessingCommandTicket(Arc<AtomicU8>);

impl ProcessingCommandTicket {
    pub(super) fn new() -> Self {
        Self(Arc::new(AtomicU8::new(PROCESSING_REQUEST_PENDING)))
    }

    pub(super) fn try_claim(&self) -> bool {
        self.0
            .compare_exchange(
                PROCESSING_REQUEST_PENDING,
                PROCESSING_REQUEST_CLAIMED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn cancel(&self) -> bool {
        match self.0.compare_exchange(
            PROCESSING_REQUEST_PENDING,
            PROCESSING_REQUEST_CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) | Err(PROCESSING_REQUEST_CANCELLED) => true,
            Err(PROCESSING_REQUEST_CLAIMED) => false,
            Err(_) => false,
        }
    }
}

struct ProcessingResponseInbox {
    rx: Receiver<ProcessingReply>,
    buffered: HashMap<u64, ProcessingResponse>,
    abandoned: HashSet<u64>,
}

/// Processing thread handle
pub struct ProcessingThread {
    command_tx: Sender<ProcessingRequest>,
    response_inbox: Mutex<ProcessingResponseInbox>,
    request_tickets: Mutex<HashMap<u64, ProcessingCommandTicket>>,
    next_request_id: AtomicU64,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    host_generation: Arc<AtomicU64>,
}

impl ProcessingThread {
    #[cfg(test)]
    pub(in crate::engine) fn command_probe() -> (Self, Receiver<ProcessingRequest>) {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (_response_tx, response_rx) = std::sync::mpsc::channel();
        (
            Self {
                command_tx,
                response_inbox: Mutex::new(ProcessingResponseInbox {
                    rx: response_rx,
                    buffered: HashMap::new(),
                    abandoned: HashSet::new(),
                }),
                request_tickets: Mutex::new(HashMap::new()),
                next_request_id: AtomicU64::new(1),
                thread_handle: None,
                host_generation: Arc::new(AtomicU64::new(0)),
            },
            command_rx,
        )
    }

    /// Create and start the processing thread
    #[allow(clippy::too_many_arguments)] // thread constructor takes many channel endpoints
    pub fn new(
        decoder_rx: Receiver<DecoderMessage>,
        message_tx: SyncSender<ProcessingMessage>,
        event_tx: crossbeam::channel::Sender<ThreadEvent>,
        sample_rate: u32,
        channels: usize,
        plugin_data_cache: super::PluginDataCache,
        gc_tx: super::GcSender,
        recycle_rx: Receiver<Vec<f32>>,
        decoder_recycle_tx: SyncSender<Vec<f32>>,
        #[cfg(feature = "streaming")] network_stream_tap: Option<sotf_streaming::PcmStreamHandle>,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        let host_generation = Arc::new(AtomicU64::new(0));
        let processing_host_generation = Arc::clone(&host_generation);

        let thread_handle = std::thread::Builder::new()
            .name("processing".to_string())
            .spawn(move || {
                let error_tx = event_tx.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_processing_thread(
                        decoder_rx,
                        message_tx,
                        command_rx,
                        response_tx,
                        event_tx,
                        sample_rate,
                        channels,
                        plugin_data_cache,
                        gc_tx,
                        recycle_rx,
                        decoder_recycle_tx,
                        processing_host_generation,
                        #[cfg(feature = "streaming")]
                        network_stream_tap,
                    )
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        log::error!("[Processing Thread] Error: {}", e);
                        let _ = error_tx.try_send(ThreadEvent::ProcessingError(format!(
                            "Processing thread exited with an error: {e}"
                        )));
                    }
                    Err(_) => {
                        log::error!("[Processing Thread] Panicked");
                        let _ =
                            error_tx.try_send(ThreadEvent::ThreadPanic("processing".to_string()));
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn processing thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_inbox: Mutex::new(ProcessingResponseInbox {
                rx: response_rx,
                buffered: HashMap::new(),
                abandoned: HashSet::new(),
            }),
            request_tickets: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(1),
            thread_handle: Some(thread_handle),
            host_generation,
        })
    }

    /// Send a command to the processing thread
    pub fn send_command(&self, command: ProcessingCommand) -> Result<u64, String> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let expects_response = matches!(
            command,
            ProcessingCommand::CommitHostUpdate(_)
                | ProcessingCommand::SetParameter { .. }
                | ProcessingCommand::Bypass(_)
                | ProcessingCommand::GetPluginData(_)
        );
        let ticket = ProcessingCommandTicket::new();
        if expects_response {
            self.request_tickets
                .lock()
                .map_err(|e| format!("Failed to lock processing request tickets: {e}"))?
                .insert(id, ticket.clone());
        }
        if let Err(error) = self.command_tx.send(ProcessingRequest {
            id,
            command,
            ticket,
        }) {
            if expects_response && let Ok(mut tickets) = self.request_tickets.lock() {
                tickets.remove(&id);
            }
            return Err(format!("Failed to send command: {error}"));
        }
        Ok(id)
    }

    pub(in crate::engine) fn send_host_update(
        &self,
        update: super::PreparedHostUpdate,
    ) -> Result<(u64, u64, super::HostUpdateTicket), String> {
        let generation = self.host_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let update = update.with_generation(generation);
        let ticket = update.ticket();
        let request_id = self.send_command(ProcessingCommand::CommitHostUpdate(update))?;
        Ok((generation, request_id, ticket))
    }

    pub fn invalidate_host_update(&self, generation: u64) {
        let _ = self.host_generation.compare_exchange(
            generation,
            generation.saturating_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub fn try_recv_response_for(&self, request_id: u64) -> Option<ProcessingResponse> {
        let mut inbox = self.response_inbox.lock().ok()?;
        if let Some(response) = inbox.buffered.remove(&request_id) {
            if let Ok(mut tickets) = self.request_tickets.lock() {
                tickets.remove(&request_id);
            }
            return Some(response);
        }
        while let Ok(reply) = inbox.rx.try_recv() {
            if let Ok(mut tickets) = self.request_tickets.lock() {
                tickets.remove(&reply.id);
            }
            if inbox.abandoned.remove(&reply.id) {
                continue;
            }
            if reply.id == request_id {
                return Some(reply.response);
            }
            inbox.buffered.insert(reply.id, reply.response);
        }
        None
    }

    /// Cancel a request that processing has not claimed. Returns false once
    /// execution has started, in which case the caller must await its reply.
    pub fn cancel_request(&self, request_id: u64) -> bool {
        let cancelled = self
            .request_tickets
            .lock()
            .ok()
            .and_then(|tickets| tickets.get(&request_id).cloned())
            .is_none_or(|ticket| ticket.cancel());
        if cancelled && let Ok(mut inbox) = self.response_inbox.lock() {
            inbox.buffered.remove(&request_id);
            inbox.abandoned.insert(request_id);
        }
        cancelled
    }

    pub fn abandon_request(&self, request_id: u64) {
        if let Ok(mut inbox) = self.response_inbox.lock() {
            inbox.buffered.remove(&request_id);
            inbox.abandoned.insert(request_id);
        }
    }

    /// Whether the worker has stopped before the manager requested shutdown.
    pub fn is_finished(&self) -> bool {
        self.thread_handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
    }

    /// Shutdown the processing thread
    pub fn shutdown(&mut self) {
        self.send_command(ProcessingCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take()
            && super::join_timeout(handle, std::time::Duration::from_secs(5)).is_err()
        {
            log::warn!("[Processing Thread] Shutdown join timed out; thread left detached");
        }
    }
}

impl Drop for ProcessingThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}
