use super::{DecoderCommand, DecoderMessage, DecoderResponse, ThreadEvent};
use sotf_types::DsdOutputMode;
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

/// Decoder thread handle
pub struct DecoderThread {
    command_tx: Sender<DecoderCommand>,
    response_rx: Receiver<DecoderResponse>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl DecoderThread {
    /// Create and start the decoder thread
    pub fn new(
        message_tx: SyncSender<DecoderMessage>,
        event_tx: Sender<ThreadEvent>,
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
                if let Err(e) = run_decoder_thread(
                    message_tx,
                    command_rx,
                    response_tx,
                    event_tx,
                    target_sample_rate,
                    frame_size,
                    recycle_rx,
                    dsd_output,
                ) {
                    log::error!("[Decoder Thread] Error: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn decoder thread: {}", e))?;

        Ok(Self {
            command_tx,
            response_rx,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the decoder thread
    pub fn send_command(&self, command: DecoderCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    pub fn try_recv_response(&self) -> Option<DecoderResponse> {
        self.response_rx.try_recv().ok()
    }

    /// Shutdown the decoder thread
    pub fn shutdown(&mut self) {
        self.send_command(DecoderCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for DecoderThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}
