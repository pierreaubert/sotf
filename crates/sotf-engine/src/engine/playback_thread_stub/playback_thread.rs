use super::super::{
    HostUpdateTicket, PlaybackCommand, PlaybackConfiguration, PlaybackReconfigureRequest,
    ProcessingMessage, ThreadEvent,
};
use super::audio_unit_handle::run_playback_ios;
use std::sync::mpsc::{Receiver, Sender, SyncSender};

pub struct PlaybackThread {
    pub(super) command_tx: Sender<PlaybackCommand>,
    pub(super) thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl PlaybackThread {
    pub fn new(
        message_rx: Receiver<ProcessingMessage>,
        event_tx: crossbeam::channel::Sender<ThreadEvent>,
        sample_rate: u32,
        buffer_ms: u32,
        channels: usize,
        frame_size: usize,
        _output_device: Option<String>,
        recycle_tx: SyncSender<Vec<f32>>,
        _allow_virtual_output: bool,
        _output_access: crate::OutputAccessMode,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();

        let thread_handle = std::thread::Builder::new()
            .name("playback-ios".to_string())
            .spawn(move || {
                let error_tx = event_tx.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_playback_ios(
                        message_rx,
                        command_rx,
                        event_tx,
                        sample_rate,
                        buffer_ms,
                        channels,
                        frame_size,
                        recycle_tx,
                    )
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        log::error!("[Playback Thread iOS] Error: {error}");
                        error_tx
                            .try_send(ThreadEvent::ProcessingError(format!(
                                "iOS playback error: {error}"
                            )))
                            .ok();
                    }
                    Err(_) => {
                        log::error!("[Playback Thread iOS] Panicked");
                        error_tx
                            .try_send(ThreadEvent::ThreadPanic("playback-ios".to_string()))
                            .ok();
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn playback thread: {}", e))?;

        Ok(Self {
            command_tx,
            thread_handle: Some(thread_handle),
        })
    }

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
        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
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
        match reply_rx.recv_timeout(TIMEOUT) {
            Ok(result) => result,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("iOS playback reconfiguration reply channel disconnected".to_string())
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) if ticket.cancel() => Err(
                "iOS playback reconfiguration timed out and was cancelled before completion"
                    .to_string(),
            ),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => loop {
                match reply_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(result) => break result,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break Err("iOS playback worker exited before replying".to_string());
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) if self.is_finished() => {
                        break Err("iOS playback worker stopped before replying".to_string());
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
            },
        }
    }

    pub fn is_finished(&self) -> bool {
        self.thread_handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
    }

    pub fn shutdown(&mut self) {
        self.send_command(PlaybackCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take()
            && super::super::join_timeout(handle, std::time::Duration::from_secs(5)).is_err()
        {
            log::warn!("[Playback Thread iOS] Shutdown join timed out; thread left detached");
        }
    }
}

impl Drop for PlaybackThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}
