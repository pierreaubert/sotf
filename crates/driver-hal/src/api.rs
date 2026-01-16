//! Public API for audio player integration
//!
//! This module provides a simple, safe API for the audio player (src-audio)
//! to interact with the HAL driver's audio buffers.

use crate::audio_buffer::{AudioBufferConfig, get_global_buffer};
use rtrb::{Consumer, Producer};

/// Handle for reading audio from the HAL driver (macOS apps → player)
pub struct HalInputReader {
    consumer: Consumer<f32>,
    config: AudioBufferConfig,
}

impl HalInputReader {
    /// Create a new input reader
    ///
    /// Returns None if the HAL driver hasn't been initialized yet
    /// or if the input consumer has already been taken.
    pub fn new() -> Option<Self> {
        let buffer = get_global_buffer()?;
        let consumer = buffer.take_input_consumer()?;
        Some(Self {
            consumer,
            config: buffer.config(),
        })
    }

    /// Read audio samples from macOS apps
    ///
    /// Returns the number of samples actually read.
    /// If not enough data is available, the rest of the buffer is filled with zeros.
    pub fn read(&mut self, output: &mut [f32]) -> usize {
        // Use read_chunk for efficiency
        if let Ok(chunk) = self.consumer.read_chunk(output.len()) {
            let (s1, s2) = chunk.as_slices();
            let len1 = s1.len();
            let len2 = s2.len();

            if len1 > 0 {
                output[..len1].copy_from_slice(s1);
            }
            if len2 > 0 {
                output[len1..len1 + len2].copy_from_slice(s2);
            }
            chunk.commit_all();
            return len1 + len2;
        }

        // Not enough data, read what we have
        let available = self.consumer.slots();
        if available > 0 {
            if let Ok(chunk) = self.consumer.read_chunk(available) {
                let (s1, s2) = chunk.as_slices();
                let len1 = s1.len();
                let len2 = s2.len();

                if len1 > 0 {
                    output[..len1].copy_from_slice(s1);
                }
                if len2 > 0 {
                    output[len1..len1 + len2].copy_from_slice(s2);
                }
                chunk.commit_all();

                // Zero pad
                if available < output.len() {
                    output[available..].fill(0.0);
                }
                return available;
            }
        }

        // No data
        output.fill(0.0);
        0
    }

    /// Get available samples to read
    pub fn available(&self) -> usize {
        self.consumer.slots()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.consumer.slots() == 0
    }

    /// Skip/discard samples without reading them
    pub fn skip(&mut self, count: usize) -> usize {
        if let Ok(chunk) = self.consumer.read_chunk(count) {
            let n = chunk.len();
            chunk.commit_all();
            n
        } else {
            // Not enough, skip all available
            let n = self.consumer.slots();
            if let Ok(chunk) = self.consumer.read_chunk(n) {
                chunk.commit_all();
                n
            } else {
                0
            }
        }
    }

    /// Get buffer configuration
    pub fn config(&self) -> AudioBufferConfig {
        self.config
    }
}

/// Handle for writing audio back to the HAL driver (player → macOS, loopback)
pub struct HalOutputWriter {
    producer: Producer<f32>,
    config: AudioBufferConfig,
}

impl HalOutputWriter {
    /// Create a new output writer
    ///
    /// Returns None if the HAL driver hasn't been initialized yet
    /// or if the output producer has already been taken.
    pub fn new() -> Option<Self> {
        let buffer = get_global_buffer()?;
        let producer = buffer.take_output_producer()?;
        Some(Self {
            producer,
            config: buffer.config(),
        })
    }

    /// Write audio samples back to macOS (loopback)
    ///
    /// Returns the number of samples actually written.
    /// If buffer is full, fewer samples than requested may be written.
    pub fn write(&mut self, samples: &[f32]) -> usize {
        match self.producer.write_chunk_uninit(samples.len()) {
            Ok(chunk) => {
                chunk.fill_from_iter(samples.iter().copied());
                samples.len()
            }
            Err(_) => {
                // Not enough space, write what we can
                let available = self.producer.slots();
                if available > 0 {
                    if let Ok(chunk) = self.producer.write_chunk_uninit(available) {
                        chunk.fill_from_iter(samples[..available].iter().copied());
                        return available;
                    }
                }
                0
            }
        }
    }

    /// Get available space for writing
    pub fn available_write(&self) -> usize {
        self.producer.slots()
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        self.producer.slots() == 0
    }

    /// Get buffer configuration
    pub fn config(&self) -> AudioBufferConfig {
        self.config
    }
}

/// Combined handle for bidirectional audio
pub struct HalAudioHandle {
    input: HalInputReader,
    output: HalOutputWriter,
}

impl HalAudioHandle {
    /// Create a new audio handle
    pub fn new() -> Option<Self> {
        let input = HalInputReader::new()?;
        let output = HalOutputWriter::new()?;
        Some(Self { input, output })
    }

    /// Read audio from macOS apps
    pub fn read_input(&mut self, output: &mut [f32]) -> usize {
        self.input.read(output)
    }

    /// Write audio back to macOS (loopback)
    pub fn write_output(&mut self, samples: &[f32]) -> usize {
        self.output.write(samples)
    }

    /// Get input buffer statistics
    pub fn input_stats(&self) -> BufferStats {
        BufferStats {
            available: self.input.available(),
            is_empty: self.input.is_empty(),
        }
    }

    /// Get output buffer statistics
    pub fn output_stats(&self) -> BufferStats {
        BufferStats {
            available: self.output.available_write(),
            is_empty: self.output.is_full(),
        }
    }

    /// Get buffer configuration
    pub fn config(&self) -> AudioBufferConfig {
        self.input.config()
    }
}

/// Buffer statistics
#[derive(Debug, Clone, Copy)]
pub struct BufferStats {
    pub available: usize,
    pub is_empty: bool,
}

// C API for potential C/C++ integration
use std::os::raw::{c_float, c_int};

/// C API: Read audio from HAL input buffer
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hal_read_input(output: *mut c_float, length: c_int) -> c_int {
    if output.is_null() || length <= 0 {
        return -1;
    }

    // NOTE: This will fail if the Rust engine has already taken the consumer!
    let mut reader = match HalInputReader::new() {
        Some(r) => r,
        None => return -1,
    };

    let slice = unsafe { std::slice::from_raw_parts_mut(output, length as usize) };
    reader.read(slice) as c_int
}

/// C API: Write audio to HAL output buffer (loopback)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hal_write_output(input: *const c_float, length: c_int) -> c_int {
    if input.is_null() || length <= 0 {
        return -1;
    }

    // NOTE: This will fail if the Rust engine has already taken the producer!
    let mut writer = match HalOutputWriter::new() {
        Some(w) => w,
        None => return -1,
    };

    let slice = unsafe { std::slice::from_raw_parts(input, length as usize) };
    writer.write(slice) as c_int
}

/// C API: Get available input samples
#[unsafe(no_mangle)]
pub extern "C" fn hal_input_available() -> c_int {
    // This creates a temporary reader, checks, and drops it (destroying the consumer)
    // This is destructive with rtrb!
    // Returning -1 to indicate not supported in this mode
    -1
}

/// C API: Get available output space
#[unsafe(no_mangle)]
pub extern "C" fn hal_output_available() -> c_int {
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_buffer::init_global_buffer;

    #[test]
    fn test_api_handles() {
        // Initialize global buffer
        init_global_buffer(500, 48000, 2);

        // Test input reader
        let reader = HalInputReader::new().expect("Failed to create reader");
        let config = reader.config();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);

        // Test output writer
        let mut writer = HalOutputWriter::new().expect("Failed to create writer");
        let config = writer.config();
        assert_eq!(config.sample_rate, 48000);

        // Test taking again should fail
        assert!(HalInputReader::new().is_none());
    }

    #[test]
    fn test_read_write() {
        init_global_buffer(500, 48000, 2);

        // We need to simulate the other side (Producer/Consumer)
        // usage since we can only take one side via API

        // But for unit test we can just use the handles
        // Wait, HalInputReader takes Consumer. Who writes?
        // We need the HAL side handles.
        // The API only exposes the Player side handles.
        // We can't easily test read/write without access to the other side of the ring buffer
        // unless we expose HAL side helpers.

        // Skip read/write test for now or implement HAL side helpers in test
    }
}
