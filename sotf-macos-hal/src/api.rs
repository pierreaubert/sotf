//! Public API for audio player integration
//!
//! This module provides a simple, safe API for the audio player (src-audio)
//! to interact with the HAL driver's audio buffers.

use crate::audio_buffer::{get_global_buffer, AudioBuffer, AudioBufferConfig};
use std::sync::Arc;

/// Handle for reading audio from the HAL driver (macOS apps → player)
pub struct HalInputReader {
    buffer: Arc<AudioBuffer>,
}

impl HalInputReader {
    /// Create a new input reader
    ///
    /// Returns None if the HAL driver hasn't been initialized yet.
    pub fn new() -> Option<Self> {
        let buffer = get_global_buffer()?;
        Some(Self { buffer })
    }

    /// Read audio samples from macOS apps
    ///
    /// Returns the number of samples actually read.
    /// If not enough data is available, the rest of the buffer is filled with zeros.
    pub fn read(&mut self, output: &mut [f32]) -> usize {
        let mut consumer = self.buffer.input_consumer();
        consumer.read(output)
    }

    /// Get available samples to read
    pub fn available(&self) -> usize {
        let consumer = self.buffer.input_consumer();
        consumer.available_read()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        let consumer = self.buffer.input_consumer();
        consumer.is_empty()
    }

    /// Skip/discard samples without reading them
    pub fn skip(&mut self, count: usize) -> usize {
        let mut consumer = self.buffer.input_consumer();
        consumer.skip(count)
    }

    /// Get buffer configuration
    pub fn config(&self) -> AudioBufferConfig {
        self.buffer.config()
    }
}

/// Handle for writing audio back to the HAL driver (player → macOS, loopback)
pub struct HalOutputWriter {
    buffer: Arc<AudioBuffer>,
}

impl HalOutputWriter {
    /// Create a new output writer
    ///
    /// Returns None if the HAL driver hasn't been initialized yet.
    pub fn new() -> Option<Self> {
        let buffer = get_global_buffer()?;
        Some(Self { buffer })
    }

    /// Write audio samples back to macOS (loopback)
    ///
    /// Returns the number of samples actually written.
    /// If buffer is full, fewer samples than requested may be written.
    pub fn write(&mut self, samples: &[f32]) -> usize {
        let mut producer = self.buffer.output_producer();
        producer.write(samples)
    }

    /// Get available space for writing
    pub fn available_write(&self) -> usize {
        let producer = self.buffer.output_producer();
        producer.available_write()
    }

    /// Check if buffer is full
    pub fn is_full(&self) -> bool {
        let producer = self.buffer.output_producer();
        producer.is_full()
    }

    /// Get buffer configuration
    pub fn config(&self) -> AudioBufferConfig {
        self.buffer.config()
    }
}

/// Combined handle for bidirectional audio
///
/// This is a convenience wrapper that provides both input reading and output writing.
pub struct HalAudioHandle {
    buffer: Arc<AudioBuffer>,
}

impl HalAudioHandle {
    /// Create a new audio handle
    ///
    /// Returns None if the HAL driver hasn't been initialized yet.
    pub fn new() -> Option<Self> {
        let buffer = get_global_buffer()?;
        Some(Self { buffer })
    }

    /// Read audio from macOS apps
    pub fn read_input(&mut self, output: &mut [f32]) -> usize {
        let mut consumer = self.buffer.input_consumer();
        consumer.read(output)
    }

    /// Write audio back to macOS (loopback)
    pub fn write_output(&mut self, samples: &[f32]) -> usize {
        let mut producer = self.buffer.output_producer();
        producer.write(samples)
    }

    /// Get input buffer statistics
    pub fn input_stats(&self) -> BufferStats {
        let consumer = self.buffer.input_consumer();
        BufferStats {
            available: consumer.available_read(),
            is_empty: consumer.is_empty(),
        }
    }

    /// Get output buffer statistics
    pub fn output_stats(&self) -> BufferStats {
        let producer = self.buffer.output_producer();
        BufferStats {
            available: producer.available_write(),
            is_empty: producer.is_full(), // Note: for output, we check if full
        }
    }

    /// Get buffer configuration
    pub fn config(&self) -> AudioBufferConfig {
        self.buffer.config()
    }
}

/// Buffer statistics
#[derive(Debug, Clone, Copy)]
pub struct BufferStats {
    pub available: usize,
    pub is_empty: bool,
}

// C API for potential C/C++ integration
// This allows src-audio or other C code to access the HAL buffers

use std::os::raw::{c_float, c_int};

/// C API: Read audio from HAL input buffer
///
/// Returns number of samples read, or -1 on error.
///
/// # Safety
/// Caller must ensure:
/// - `output` points to a valid buffer of at least `length` floats
/// - `output` is properly aligned for f32
/// - `output` buffer remains valid for the duration of this call
#[no_mangle]
pub unsafe extern "C" fn hal_read_input(output: *mut c_float, length: c_int) -> c_int {
    if output.is_null() || length <= 0 {
        return -1;
    }

    let mut reader = match HalInputReader::new() {
        Some(r) => r,
        None => return -1,
    };

    let slice = std::slice::from_raw_parts_mut(output, length as usize);
    reader.read(slice) as c_int
}

/// C API: Write audio to HAL output buffer (loopback)
///
/// Returns number of samples written, or -1 on error.
///
/// # Safety
/// Caller must ensure:
/// - `input` points to a valid buffer of at least `length` floats
/// - `input` is properly aligned for f32
/// - `input` buffer remains valid for the duration of this call
#[no_mangle]
pub unsafe extern "C" fn hal_write_output(input: *const c_float, length: c_int) -> c_int {
    if input.is_null() || length <= 0 {
        return -1;
    }

    let mut writer = match HalOutputWriter::new() {
        Some(w) => w,
        None => return -1,
    };

    let slice = std::slice::from_raw_parts(input, length as usize);
    writer.write(slice) as c_int
}

/// C API: Get available input samples
///
/// Returns number of available samples, or -1 on error.
#[no_mangle]
pub extern "C" fn hal_input_available() -> c_int {
    let reader = match HalInputReader::new() {
        Some(r) => r,
        None => return -1,
    };

    reader.available() as c_int
}

/// C API: Get available output space
///
/// Returns number of available sample slots, or -1 on error.
#[no_mangle]
pub extern "C" fn hal_output_available() -> c_int {
    let writer = match HalOutputWriter::new() {
        Some(w) => w,
        None => return -1,
    };

    writer.available_write() as c_int
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
        let mut reader = HalInputReader::new().expect("Failed to create reader");
        let config = reader.config();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);

        // Test output writer
        let mut writer = HalOutputWriter::new().expect("Failed to create writer");
        let config = writer.config();
        assert_eq!(config.sample_rate, 48000);

        // Test combined handle
        let handle = HalAudioHandle::new().expect("Failed to create handle");
        let config = handle.config();
        assert_eq!(config.sample_rate, 48000);
    }

    #[test]
    fn test_read_write() {
        init_global_buffer(500, 48000, 2);

        let mut writer = HalOutputWriter::new().unwrap();
        let mut reader = HalInputReader::new().unwrap();

        // Note: These are separate buffers (input vs output)
        // So writing to output won't appear in input

        let data = vec![1.0, 2.0, 3.0, 4.0];
        let written = writer.write(&data);
        assert_eq!(written, data.len());
    }
}
