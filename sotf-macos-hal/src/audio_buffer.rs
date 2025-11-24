//! Lock-free audio buffers for bidirectional audio flow
//!
//! This module provides two ring buffers:
//! - Input buffer: Audio data coming FROM macOS apps TO the audio player
//! - Output buffer: Audio data going FROM the audio player BACK TO the HAL (loopback)

use crossbeam::channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};

/// Bidirectional audio buffer for HAL driver
pub struct AudioBuffer {
    /// Input channel: macOS apps → audio player
    /// Written by HAL I/O callback, read by audio player
    input_tx: Sender<Vec<f32>>,
    input_rx: Receiver<Vec<f32>>,

    /// Output channel: audio player → HAL (loopback)
    /// Written by audio player, read by HAL I/O callback
    output_tx: Sender<Vec<f32>>,
    output_rx: Receiver<Vec<f32>>,

    /// Sample rate
    sample_rate: u32,

    /// Number of channels
    channels: usize,
}

impl AudioBuffer {
    /// Create a new bidirectional audio buffer
    ///
    /// # Arguments
    /// * `capacity_ms` - Buffer capacity in milliseconds (e.g., 500ms)
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    pub fn new(capacity_ms: usize, sample_rate: u32, channels: usize) -> Self {
        let capacity_frames = (sample_rate as usize * capacity_ms) / 1000;
        // Each message will be a chunk of audio, capacity is in number of chunks
        let channel_capacity = capacity_frames / 512; // Assume ~512 samples per chunk

        // Create bounded channels
        let (input_tx, input_rx) = bounded(channel_capacity);
        let (output_tx, output_rx) = bounded(channel_capacity);

        log::info!(
            "Created audio buffers: {}ms capacity, {} Hz, {} channels ({} frame buffer, {} chunk capacity)",
            capacity_ms, sample_rate, channels, capacity_frames, channel_capacity
        );

        Self {
            input_tx,
            input_rx,
            output_tx,
            output_rx,
            sample_rate,
            channels,
        }
    }

    /// Get a handle to write to input buffer (HAL → player)
    pub fn input_producer(&self) -> AudioBufferProducer {
        AudioBufferProducer {
            tx: self.input_tx.clone(),
        }
    }

    /// Get a handle to read from input buffer (player reads from HAL)
    pub fn input_consumer(&self) -> AudioBufferConsumer {
        AudioBufferConsumer {
            rx: self.input_rx.clone(),
            buffer: Vec::new(),
            read_pos: 0,
        }
    }

    /// Get a handle to write to output buffer (player → HAL loopback)
    pub fn output_producer(&self) -> AudioBufferProducer {
        AudioBufferProducer {
            tx: self.output_tx.clone(),
        }
    }

    /// Get a handle to read from output buffer (HAL reads loopback)
    pub fn output_consumer(&self) -> AudioBufferConsumer {
        AudioBufferConsumer {
            rx: self.output_rx.clone(),
            buffer: Vec::new(),
            read_pos: 0,
        }
    }

    /// Get buffer configuration
    pub fn config(&self) -> AudioBufferConfig {
        AudioBufferConfig {
            sample_rate: self.sample_rate,
            channels: self.channels,
        }
    }
}

/// Configuration information for the audio buffer
#[derive(Debug, Clone, Copy)]
pub struct AudioBufferConfig {
    pub sample_rate: u32,
    pub channels: usize,
}

/// Producer handle for writing to a buffer
pub struct AudioBufferProducer {
    tx: Sender<Vec<f32>>,
}

impl AudioBufferProducer {
    /// Write audio samples to the buffer (non-blocking)
    ///
    /// Returns the number of samples actually written.
    /// If buffer is full, fewer samples may be written.
    pub fn write(&mut self, samples: &[f32]) -> usize {
        match self.tx.try_send(samples.to_vec()) {
            Ok(_) => samples.len(),
            Err(TrySendError::Full(_)) => {
                log::warn!("Audio buffer full, dropping samples");
                0
            }
            Err(TrySendError::Disconnected(_)) => {
                log::error!("Audio buffer disconnected");
                0
            }
        }
    }

    /// Get available space for writing (approximate)
    pub fn available_write(&self) -> usize {
        // Crossbeam doesn't provide exact space, return a large number if not full
        if self.tx.is_full() {
            0
        } else {
            100000 // Approximate
        }
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        self.tx.is_full()
    }
}

/// Consumer handle for reading from a buffer
pub struct AudioBufferConsumer {
    rx: Receiver<Vec<f32>>,
    buffer: Vec<f32>,
    read_pos: usize,
}

impl AudioBufferConsumer {
    /// Read audio samples from the buffer (non-blocking)
    ///
    /// Returns the number of samples actually read.
    /// If buffer doesn't have enough data, fewer samples may be read (rest filled with zeros).
    pub fn read(&mut self, output: &mut [f32]) -> usize {
        let mut written = 0;

        while written < output.len() {
            // If internal buffer is exhausted, try to get more data
            if self.read_pos >= self.buffer.len() {
                match self.rx.try_recv() {
                    Ok(chunk) => {
                        self.buffer = chunk;
                        self.read_pos = 0;
                    }
                    Err(TryRecvError::Empty) => {
                        // No more data, fill rest with zeros
                        output[written..].fill(0.0);
                        return written;
                    }
                    Err(TryRecvError::Disconnected) => {
                        output[written..].fill(0.0);
                        return written;
                    }
                }
            }

            // Copy from internal buffer to output
            let available = self.buffer.len() - self.read_pos;
            let to_copy = (output.len() - written).min(available);
            output[written..written + to_copy]
                .copy_from_slice(&self.buffer[self.read_pos..self.read_pos + to_copy]);
            self.read_pos += to_copy;
            written += to_copy;
        }

        written
    }

    /// Get available samples for reading (approximate)
    pub fn available_read(&self) -> usize {
        let buffer_remaining = self.buffer.len() - self.read_pos;
        buffer_remaining + self.rx.len() * 512 // Estimate
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.read_pos >= self.buffer.len() && self.rx.is_empty()
    }

    /// Skip/discard samples without reading them
    pub fn skip(&mut self, count: usize) -> usize {
        let mut dummy = vec![0.0f32; count];
        self.read(&mut dummy)
    }
}

/// Global audio buffer shared between HAL driver and audio player
static GLOBAL_AUDIO_BUFFER: OnceLock<Mutex<Option<Arc<AudioBuffer>>>> = OnceLock::new();

/// Initialize the global audio buffer
///
/// This should be called once when the HAL driver initializes.
/// The buffer can then be accessed by both the HAL driver and audio player.
pub fn init_global_buffer(capacity_ms: usize, sample_rate: u32, channels: usize) {
    let lock = GLOBAL_AUDIO_BUFFER.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().unwrap();

    if guard.is_some() {
        log::warn!("Global audio buffer already initialized, replacing...");
    }

    let buffer = Arc::new(AudioBuffer::new(capacity_ms, sample_rate, channels));
    *guard = Some(buffer);
    log::info!("Global audio buffer initialized");
}

/// Get the global audio buffer
///
/// Returns None if buffer hasn't been initialized yet.
pub fn get_global_buffer() -> Option<Arc<AudioBuffer>> {
    GLOBAL_AUDIO_BUFFER
        .get()
        .and_then(|lock| lock.lock().unwrap().clone())
}

/// Shutdown and clear the global audio buffer
pub fn shutdown_global_buffer() {
    if let Some(lock) = GLOBAL_AUDIO_BUFFER.get() {
        let mut guard = lock.lock().unwrap();
        *guard = None;
        log::info!("Global audio buffer shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_creation() {
        let buffer = AudioBuffer::new(500, 48000, 2);
        assert_eq!(buffer.sample_rate, 48000);
        assert_eq!(buffer.channels, 2);
    }

    #[test]
    fn test_buffer_write_read() {
        let buffer = AudioBuffer::new(500, 48000, 2);

        let mut producer = buffer.input_producer();
        let mut consumer = buffer.input_consumer();

        // Write some samples
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let written = producer.write(&input);
        assert_eq!(written, input.len());

        // Read them back
        let mut output = vec![0.0; 5];
        let read = consumer.read(&mut output);
        assert_eq!(read, input.len());
        assert_eq!(output, input);
    }

    #[test]
    fn test_bidirectional_buffers() {
        let buffer = AudioBuffer::new(500, 48000, 2);

        // Test input buffer (HAL → player)
        let mut input_prod = buffer.input_producer();
        let mut input_cons = buffer.input_consumer();

        let input_data = vec![1.0, 2.0, 3.0];
        input_prod.write(&input_data);

        let mut read_data = vec![0.0; 3];
        input_cons.read(&mut read_data);
        assert_eq!(read_data, input_data);

        // Test output buffer (player → HAL)
        let mut output_prod = buffer.output_producer();
        let mut output_cons = buffer.output_consumer();

        let output_data = vec![4.0, 5.0, 6.0];
        output_prod.write(&output_data);

        let mut read_data2 = vec![0.0; 3];
        output_cons.read(&mut read_data2);
        assert_eq!(read_data2, output_data);
    }
}
