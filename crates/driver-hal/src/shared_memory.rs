//! Shared memory interface for communication with Swift HAL driver
//!
//! This module provides a Rust interface to the shared memory region
//! created by the Swift HAL driver for audio data exchange.

use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use memmap2::MmapMut;

/// Magic number for shared memory header validation: 'SOTF'
const SHARED_MEMORY_MAGIC: u32 = 0x534F5446;

/// Current protocol version
const SHARED_MEMORY_VERSION: u32 = 1;

/// Default shared memory file path
pub const SHARED_MEMORY_PATH: &str = "/tmp/sotf-audio-shm";

/// Header structure for shared memory region
/// Must match the Swift side exactly
#[repr(C)]
pub struct SharedAudioHeader {
    /// Magic number for validation (0x534F5446 = 'SOTF')
    pub magic: u32,
    /// Protocol version
    pub version: u32,
    /// Current sample rate in Hz
    pub sample_rate: u32,
    /// Frames per buffer
    pub buffer_frames: u32,
    /// Number of audio channels
    pub channel_count: u32,

    // Ring buffer state (atomic)
    /// Write position in samples
    pub write_position: AtomicU64,
    /// Read position in samples
    pub read_position: AtomicU64,

    // Control flags (atomic)
    /// IO is running
    pub active: AtomicU32,
    /// Configuration changed (engine should reload)
    pub config_changed: AtomicU32,
    /// Driver is initialized and ready
    pub driver_ready: AtomicU32,
    /// Rust engine is connected and ready
    pub engine_ready: AtomicU32,

    /// Reserved for future use
    pub reserved: [u32; 4],
}

/// Shared audio buffer for communication with Swift HAL driver
pub struct SharedAudioBuffer {
    mmap: MmapMut,
    audio_offset: usize,
    audio_capacity: usize,
}

impl SharedAudioBuffer {
    /// Open an existing shared memory region created by the Swift HAL driver
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        let mmap = unsafe { MmapMut::map_mut(&file)? };

        // Calculate audio data offset (64-byte aligned after header)
        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let audio_offset = (header_size + 63) & !63;

        // Get audio capacity from header
        let header = unsafe { &*(mmap.as_ptr() as *const SharedAudioHeader) };

        if header.magic != SHARED_MEMORY_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid shared memory magic number",
            ));
        }

        if header.version != SHARED_MEMORY_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Incompatible shared memory version: {} (expected {})",
                    header.version, SHARED_MEMORY_VERSION
                ),
            ));
        }

        let audio_capacity =
            (header.buffer_frames as usize) * (header.channel_count as usize) * 8; // 8 buffers

        Ok(Self {
            mmap,
            audio_offset,
            audio_capacity,
        })
    }

    /// Open the default shared memory path
    pub fn open_default() -> io::Result<Self> {
        Self::open(SHARED_MEMORY_PATH)
    }

    /// Get a reference to the header
    pub fn header(&self) -> &SharedAudioHeader {
        unsafe { &*(self.mmap.as_ptr() as *const SharedAudioHeader) }
    }

    /// Get a mutable reference to the header
    #[allow(dead_code)]
    fn header_mut(&mut self) -> &mut SharedAudioHeader {
        unsafe { &mut *(self.mmap.as_mut_ptr() as *mut SharedAudioHeader) }
    }

    /// Check if the driver is ready
    pub fn driver_ready(&self) -> bool {
        self.header().driver_ready.load(Ordering::Acquire) != 0
    }

    /// Check if IO is active
    pub fn is_active(&self) -> bool {
        self.header().active.load(Ordering::Acquire) != 0
    }

    /// Check if configuration has changed
    pub fn config_changed(&self) -> bool {
        self.header().config_changed.load(Ordering::Acquire) != 0
    }

    /// Clear the configuration changed flag
    pub fn clear_config_changed(&self) {
        self.header().config_changed.store(0, Ordering::Release);
    }

    /// Set engine ready flag
    pub fn set_engine_ready(&self, ready: bool) {
        self.header()
            .engine_ready
            .store(if ready { 1 } else { 0 }, Ordering::Release);
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.header().sample_rate
    }

    /// Get buffer frame size
    pub fn buffer_frames(&self) -> u32 {
        self.header().buffer_frames
    }

    /// Get channel count
    pub fn channel_count(&self) -> u32 {
        self.header().channel_count
    }

    /// Get pointer to audio data
    fn audio_data(&self) -> *const f32 {
        unsafe { self.mmap.as_ptr().add(self.audio_offset) as *const f32 }
    }

    /// Get mutable pointer to audio data
    fn audio_data_mut(&mut self) -> *mut f32 {
        unsafe { self.mmap.as_mut_ptr().add(self.audio_offset) as *mut f32 }
    }

    /// Read audio from the shared memory ring buffer
    /// Returns the number of frames actually read
    pub fn read_audio(&self, buffer: &mut [f32]) -> usize {
        let header = self.header();
        let channel_count = header.channel_count as usize;
        let sample_count = buffer.len();

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let available = (write_pos - read_pos) as usize;
        let to_read = sample_count.min(available);

        if to_read == 0 {
            buffer.fill(0.0);
            return 0;
        }

        let read_index = (read_pos as usize) % self.audio_capacity;
        let first_part = to_read.min(self.audio_capacity - read_index);
        let second_part = to_read - first_part;

        unsafe {
            let audio_data = self.audio_data();

            // Copy first part
            std::ptr::copy_nonoverlapping(
                audio_data.add(read_index),
                buffer.as_mut_ptr(),
                first_part,
            );

            // Copy second part (wrap around)
            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    audio_data,
                    buffer.as_mut_ptr().add(first_part),
                    second_part,
                );
            }
        }

        // Fill remaining with silence
        if to_read < sample_count {
            buffer[to_read..].fill(0.0);
        }

        // Update read position
        header
            .read_position
            .store(read_pos + to_read as u64, Ordering::Release);

        to_read / channel_count
    }

    /// Write audio to the shared memory ring buffer
    /// Returns the number of frames actually written
    pub fn write_audio(&mut self, buffer: &[f32]) -> usize {
        let header = self.header();
        let channel_count = header.channel_count as usize;
        let sample_count = buffer.len();

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let used = (write_pos - read_pos) as usize;
        let available = self.audio_capacity - used;
        let to_write = sample_count.min(available);

        if to_write == 0 {
            return 0;
        }

        let write_index = (write_pos as usize) % self.audio_capacity;
        let first_part = to_write.min(self.audio_capacity - write_index);
        let second_part = to_write - first_part;

        unsafe {
            let audio_data = self.audio_data_mut();

            // Copy first part
            std::ptr::copy_nonoverlapping(
                buffer.as_ptr(),
                audio_data.add(write_index),
                first_part,
            );

            // Copy second part (wrap around)
            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    buffer.as_ptr().add(first_part),
                    audio_data,
                    second_part,
                );
            }
        }

        // Update write position
        self.header()
            .write_position
            .store(write_pos + to_write as u64, Ordering::Release);

        to_write / channel_count
    }

    /// Get available frames to read
    pub fn available_read_frames(&self) -> usize {
        let header = self.header();
        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);
        let channel_count = header.channel_count as usize;

        ((write_pos - read_pos) as usize) / channel_count
    }

    /// Get available space to write (in frames)
    pub fn available_write_frames(&self) -> usize {
        let header = self.header();
        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);
        let channel_count = header.channel_count as usize;

        let used = (write_pos - read_pos) as usize;
        (self.audio_capacity - used) / channel_count
    }
}

// =============================================================================
// Adapter types for compatibility with existing code
// =============================================================================

/// Reader adapter for HAL input (compatible with old HalInputReader API)
pub struct HalInputReader {
    buffer: Option<SharedAudioBuffer>,
}

impl HalInputReader {
    /// Create a new HAL input reader
    pub fn new() -> Option<Self> {
        match SharedAudioBuffer::open_default() {
            Ok(buffer) => Some(Self {
                buffer: Some(buffer),
            }),
            Err(_) => None,
        }
    }

    /// Check if connected to the HAL driver
    pub fn is_connected(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.driver_ready())
            .unwrap_or(false)
    }

    /// Read audio samples from the HAL
    pub fn read(&self, buffer: &mut [f32]) -> usize {
        self.buffer
            .as_ref()
            .map(|b| b.read_audio(buffer))
            .unwrap_or(0)
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(48000)
    }

    /// Get channel count
    pub fn channel_count(&self) -> u32 {
        self.buffer
            .as_ref()
            .map(|b| b.channel_count())
            .unwrap_or(2)
    }
}

impl Default for HalInputReader {
    fn default() -> Self {
        Self { buffer: None }
    }
}

/// Writer adapter for HAL output (compatible with old HalOutputWriter API)
pub struct HalOutputWriter {
    buffer: Option<SharedAudioBuffer>,
}

impl HalOutputWriter {
    /// Create a new HAL output writer
    pub fn new() -> Option<Self> {
        match SharedAudioBuffer::open_default() {
            Ok(buffer) => Some(Self {
                buffer: Some(buffer),
            }),
            Err(_) => None,
        }
    }

    /// Check if connected to the HAL driver
    pub fn is_connected(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.driver_ready())
            .unwrap_or(false)
    }

    /// Write audio samples to the HAL
    pub fn write(&mut self, buffer: &[f32]) -> usize {
        self.buffer
            .as_mut()
            .map(|b| b.write_audio(buffer))
            .unwrap_or(0)
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(48000)
    }

    /// Get channel count
    pub fn channel_count(&self) -> u32 {
        self.buffer
            .as_ref()
            .map(|b| b.channel_count())
            .unwrap_or(2)
    }

    /// Set engine ready flag
    pub fn set_engine_ready(&self, ready: bool) {
        if let Some(buffer) = &self.buffer {
            buffer.set_engine_ready(ready);
        }
    }
}

impl Default for HalOutputWriter {
    fn default() -> Self {
        Self { buffer: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        // Ensure header is packed correctly
        assert!(std::mem::size_of::<SharedAudioHeader>() <= 128);
    }
}
