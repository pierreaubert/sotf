//! Lock-free audio buffers for bidirectional audio flow
//!
//! This module provides two ring buffers:
//! - Input buffer: Audio data coming FROM macOS apps TO the audio player
//! - Output buffer: Audio data going FROM the audio player BACK TO the HAL (loopback)

use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{Arc, Mutex, OnceLock};

/// Bidirectional audio buffer for HAL driver
pub struct AudioBuffer {
    /// Input channel: macOS apps → audio player
    /// Written by HAL I/O callback, read by audio player
    pub input_producer: Mutex<Option<Producer<f32>>>,
    pub input_consumer: Mutex<Option<Consumer<f32>>>,

    /// Output channel: audio player → HAL (loopback)
    /// Written by audio player, read by HAL I/O callback
    pub output_producer: Mutex<Option<Producer<f32>>>,
    pub output_consumer: Mutex<Option<Consumer<f32>>>,

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
        let total_capacity = capacity_frames * channels;

        // Create ring buffers
        let (in_prod, in_cons) = RingBuffer::<f32>::new(total_capacity);
        let (out_prod, out_cons) = RingBuffer::<f32>::new(total_capacity);

        log::info!(
            "Created audio buffers: {}ms capacity, {} Hz, {} channels ({} samples capacity)",
            capacity_ms,
            sample_rate,
            channels,
            total_capacity
        );

        Self {
            input_producer: Mutex::new(Some(in_prod)),
            input_consumer: Mutex::new(Some(in_cons)),
            output_producer: Mutex::new(Some(out_prod)),
            output_consumer: Mutex::new(Some(out_cons)),
            sample_rate,
            channels,
        }
    }

    /// Take the input producer (HAL → player)
    /// Called by HAL Driver
    pub fn take_input_producer(&self) -> Option<Producer<f32>> {
        self.input_producer.lock().unwrap().take()
    }

    /// Take the input consumer (HAL → player)
    /// Called by Audio Player
    pub fn take_input_consumer(&self) -> Option<Consumer<f32>> {
        self.input_consumer.lock().unwrap().take()
    }

    /// Take the output producer (player → HAL loopback)
    /// Called by Audio Player
    pub fn take_output_producer(&self) -> Option<Producer<f32>> {
        self.output_producer.lock().unwrap().take()
    }

    /// Take the output consumer (player → HAL loopback)
    /// Called by HAL Driver
    pub fn take_output_consumer(&self) -> Option<Consumer<f32>> {
        self.output_consumer.lock().unwrap().take()
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

        let mut producer = buffer.take_input_producer().unwrap();
        let mut consumer = buffer.take_input_consumer().unwrap();

        // Write some samples
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let written = producer
            .write_chunk_uninit(input.len())
            .unwrap()
            .fill_from_iter(input.iter().copied());
        assert_eq!(written, input.len());

        // Read them back
        let mut output = vec![0.0; 5];
        let chunk = consumer.read_chunk(5).unwrap();
        let (s1, s2) = chunk.as_slices();
        let len1 = s1.len();
        output[..len1].copy_from_slice(s1);
        output[len1..].copy_from_slice(s2);
        chunk.commit_all();

        assert_eq!(output, input);
    }
}
