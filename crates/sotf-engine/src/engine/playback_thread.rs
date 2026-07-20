use super::{PlaybackCommand, ProcessingMessage, ThreadEvent};
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

pub(in crate::engine) use frame_writer::{
    FrameWriteOutcome, required_conversion_capacity, write_frame_to_ring,
};
use misc::send_playback_event;
use runtime::run_playback_thread;

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
        event_tx: Sender<ThreadEvent>,
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

        let thread_handle = std::thread::Builder::new()
            .name("playback".to_string())
            .spawn(move || {
                let error_tx = event_tx.clone();
                if let Err(e) = run_playback_thread(
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
                ) {
                    log::debug!("[Playback Thread] Error: {}", e);
                    send_playback_event(
                        &error_tx,
                        ThreadEvent::ProcessingError(format!("Playback thread error: {}", e)),
                        "thread error",
                    );
                }
            })
            .map_err(|e| format!("Failed to spawn playback thread: {}", e))?;

        Ok(Self {
            command_tx,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the playback thread
    pub fn send_command(&self, command: PlaybackCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
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
