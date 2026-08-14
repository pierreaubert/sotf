use super::{
    HostUpdateTicket, PlaybackCommand, PlaybackConfiguration, PlaybackReconfigureRequest,
    ProcessingMessage, ThreadEvent,
};
use crate::OutputAccessMode;
use std::sync::mpsc::{Receiver, Sender, SyncSender};

mod apply;
mod build;
mod core_audio_exclusive_mode_guard;
mod coreaudio_mod;
mod frame_writer;
mod misc;
mod pick;
mod playback;
mod playback_state;
mod runtime;
#[cfg(test)]
mod tests;
mod types;

#[cfg(feature = "playback-runtime-harness")]
pub(in crate::engine) use apply::apply_volume_clamp;
#[allow(unused_imports)]
#[cfg(any(test, feature = "playback-runtime-harness"))]
pub(in crate::engine) use frame_writer::{
    FrameWriteOutcome, required_conversion_capacity, write_frame_to_ring,
};
use misc::send_playback_event;
#[cfg(feature = "playback-runtime-harness")]
pub(in crate::engine) use misc::write_chunk_bulk;
#[cfg(feature = "playback-runtime-harness")]
pub(in crate::engine) use playback_state::{PlaybackState, read_ring_buffer};
use runtime::run_playback_thread;

const PLAYBACK_RECONFIGURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Playback thread handle
pub struct PlaybackThread {
    command_tx: Sender<PlaybackCommand>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl PlaybackThread {
    #[cfg(test)]
    pub(in crate::engine) fn command_probe() -> (Self, Receiver<PlaybackCommand>) {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        (
            Self {
                command_tx,
                thread_handle: None,
            },
            command_rx,
        )
    }

    /// Create and start the playback thread
    #[allow(
        clippy::too_many_arguments,
        reason = "constructor mirrors run_playback_thread argument list"
    )]
    pub fn new(
        message_rx: Receiver<ProcessingMessage>,
        event_tx: crossbeam::channel::Sender<ThreadEvent>,
        sample_rate: u32,
        buffer_ms: u32,
        channels: usize,
        frame_size: usize,
        output_device: Option<String>,
        recycle_tx: SyncSender<Vec<f32>>,
        allow_virtual_output: bool,
        output_access: OutputAccessMode,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let (startup_tx, startup_rx) = std::sync::mpsc::sync_channel(1);

        let thread_handle = std::thread::Builder::new()
            .name("playback".to_string())
            .spawn(move || {
                let error_tx = event_tx.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_playback_thread(
                        message_rx,
                        command_rx,
                        event_tx,
                        sample_rate,
                        buffer_ms,
                        channels,
                        frame_size,
                        output_device,
                        recycle_tx,
                        allow_virtual_output,
                        output_access,
                        startup_tx,
                    )
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        log::error!("[Playback Thread] Error: {}", e);
                        send_playback_event(
                            &error_tx,
                            ThreadEvent::ProcessingError(format!("Playback thread error: {e}")),
                            "thread error",
                        );
                    }
                    Err(_) => {
                        log::error!("[Playback Thread] Panicked");
                        send_playback_event(
                            &error_tx,
                            ThreadEvent::ThreadPanic("playback".to_string()),
                            "thread panic",
                        );
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn playback thread: {}", e))?;

        match startup_rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(Self {
                command_tx,
                thread_handle: Some(thread_handle),
            }),
            Ok(Err(err)) => {
                let _ = thread_handle.join();
                Err(err)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let _ = thread_handle.join();
                Err("Playback thread exited during startup".to_string())
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                drop(command_tx);
                if super::join_timeout(thread_handle, std::time::Duration::from_secs(1)).is_err() {
                    log::warn!(
                        "[Playback Thread] Startup timed out and worker did not exit within 1s; leaving it detached"
                    );
                }
                Err("Playback thread startup timed out after 10s".to_string())
            }
        }
    }

    /// Send a command to the playback thread
    pub fn send_command(&self, command: PlaybackCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    pub(in crate::engine) fn reconfigure(
        &self,
        sample_rate: u32,
        channels: usize,
    ) -> Result<PlaybackConfiguration, String> {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        let ticket = HostUpdateTicket::new();
        self.send_command(PlaybackCommand::Reconfigure(PlaybackReconfigureRequest {
            requested: PlaybackConfiguration {
                sample_rate,
                channels,
            },
            ticket: ticket.clone(),
            reply_tx,
        }))?;

        match reply_rx.recv_timeout(PLAYBACK_RECONFIGURE_TIMEOUT) {
            Ok(result) => result,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("Playback reconfiguration reply channel disconnected".to_string())
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) if ticket.cancel() => Err(format!(
                "Playback reconfiguration timed out and was cancelled before installation after {}ms",
                PLAYBACK_RECONFIGURE_TIMEOUT.as_millis()
            )),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                loop {
                    match reply_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                        Ok(result) => break result,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            break Err("Playback reconfiguration worker exited before replying"
                                .to_string());
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) if self.is_finished() => {
                            break Err("Playback reconfiguration worker stopped before replying"
                                .to_string());
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            }
        }
    }

    /// Whether the worker has stopped before the manager requested shutdown.
    pub fn is_finished(&self) -> bool {
        self.thread_handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
    }

    /// Shutdown the playback thread
    pub fn shutdown(&mut self) {
        if let Err(e) = self.send_command(PlaybackCommand::Shutdown) {
            log::trace!("[Playback Thread] Shutdown command receiver dropped: {}", e);
        }
        if let Some(handle) = self.thread_handle.take()
            && super::join_timeout(handle, std::time::Duration::from_secs(5)).is_err()
        {
            log::warn!("[Playback Thread] Shutdown join timed out; thread left detached");
        }
    }
}

impl Drop for PlaybackThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}
