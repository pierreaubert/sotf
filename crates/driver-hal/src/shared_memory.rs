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
/// Version 2: Added encryption fields (encrypted, key_fingerprint, frame_counter)
const SHARED_MEMORY_VERSION: u32 = 2;

/// Legacy shared memory file path (for backwards compatibility)
pub const LEGACY_SHARED_MEMORY_PATH: &str = "/tmp/sotf-audio-shm";

/// Get the secure shared memory path for the current user
///
/// Security model: each user has their own shared memory region.
/// Path is based on the user's UID or TMPDIR environment variable.
pub fn get_secure_shm_path() -> std::path::PathBuf {
    // Try macOS per-user temp directory (already secured)
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        return std::path::PathBuf::from(tmpdir).join("sotf-audio.shm");
    }

    // Fallback: use UID-based path
    let uid = unsafe { libc::getuid() };
    std::path::PathBuf::from(format!("/tmp/sotf-{}/audio.shm", uid))
}

/// Get the shared memory path to use
///
/// Tries secure path first, then falls back to legacy path if it exists
pub fn get_shared_memory_path() -> std::path::PathBuf {
    let secure_path = get_secure_shm_path();

    // If secure path exists, use it
    if secure_path.exists() {
        return secure_path;
    }

    // If legacy path exists, use it (backwards compatibility)
    let legacy_path = std::path::Path::new(LEGACY_SHARED_MEMORY_PATH);
    if legacy_path.exists() {
        return legacy_path.to_path_buf();
    }

    // Default to secure path (will be created when HAL driver initializes)
    secure_path
}

/// Default shared memory file path (for backwards compatibility)
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

    // Encryption fields (version 2+)
    /// Encryption enabled flag: 0 = disabled, 1 = enabled
    pub encrypted: AtomicU32,
    /// First 8 bytes of SHA256 hash of the encryption key (for key mismatch detection)
    pub key_fingerprint: [u8; 8],
    /// Frame counter for nonce generation (monotonically increasing, never reuse!)
    pub frame_counter: AtomicU64,
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
    ///
    /// Tries secure per-user path first, then falls back to legacy path
    pub fn open_default() -> io::Result<Self> {
        Self::open(get_shared_memory_path())
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

    /// Set sample rate
    ///
    /// This updates the sample rate in the shared memory header and sets the
    /// config_changed flag to notify the Swift HAL driver. The driver should
    /// read the new sample rate and reconfigure accordingly.
    ///
    /// # Arguments
    /// * `sample_rate` - New sample rate in Hz (e.g., 44100, 48000, 96000)
    ///
    /// # Note
    /// The actual sample rate change only takes effect after the driver
    /// processes the config_changed flag. Audio processing should be stopped
    /// before changing sample rate to avoid glitches.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        // Write sample rate directly (non-atomic, but coordinated via config_changed)
        self.header_mut().sample_rate = sample_rate;
        // Signal config change to the driver
        self.set_config_changed();
    }

    /// Get buffer frame size
    pub fn buffer_frames(&self) -> u32 {
        self.header().buffer_frames
    }

    /// Set buffer frame size
    ///
    /// # Note
    /// Changing buffer frames affects latency. Smaller = lower latency but higher CPU.
    /// Common values: 256, 512, 1024, 2048
    pub fn set_buffer_frames(&mut self, buffer_frames: u32) {
        self.header_mut().buffer_frames = buffer_frames;
        // Recalculate audio capacity
        let channel_count = self.header().channel_count as usize;
        self.audio_capacity = (buffer_frames as usize) * channel_count * 8;
        self.set_config_changed();
    }

    /// Get channel count
    pub fn channel_count(&self) -> u32 {
        self.header().channel_count
    }

    /// Set channel count
    ///
    /// # Arguments
    /// * `channel_count` - Number of audio channels (e.g., 2 for stereo, 6 for 5.1)
    pub fn set_channel_count(&mut self, channel_count: u32) {
        self.header_mut().channel_count = channel_count;
        // Recalculate audio capacity
        let buffer_frames = self.header().buffer_frames as usize;
        self.audio_capacity = buffer_frames * (channel_count as usize) * 8;
        self.set_config_changed();
    }

    // =========================================================================
    // Encryption methods
    // =========================================================================

    /// Check if encryption is enabled
    pub fn is_encrypted(&self) -> bool {
        self.header().encrypted.load(Ordering::Acquire) != 0
    }

    /// Enable or disable encryption
    pub fn set_encrypted(&self, enabled: bool) {
        self.header()
            .encrypted
            .store(if enabled { 1 } else { 0 }, Ordering::Release);
    }

    /// Get the key fingerprint
    pub fn key_fingerprint(&self) -> [u8; 8] {
        self.header().key_fingerprint
    }

    /// Set the key fingerprint
    pub fn set_key_fingerprint(&mut self, fingerprint: [u8; 8]) {
        self.header_mut().key_fingerprint = fingerprint;
    }

    /// Get the current frame counter (used as nonce base)
    pub fn frame_counter(&self) -> u64 {
        self.header().frame_counter.load(Ordering::Acquire)
    }

    /// Increment the frame counter and return the new value
    pub fn increment_frame_counter(&self) -> u64 {
        self.header().frame_counter.fetch_add(1, Ordering::AcqRel) + 1
    }

    // =========================================================================
    // Configuration methods
    // =========================================================================

    /// Set the configuration changed flag
    ///
    /// This signals the Swift HAL driver that configuration has changed
    /// and it should re-read the header values.
    pub fn set_config_changed(&self) {
        self.header().config_changed.store(1, Ordering::Release);
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

    // =========================================================================
    // Encrypted audio I/O
    // =========================================================================

    /// Write audio with encryption
    ///
    /// When encryption is enabled, this encrypts the audio data before writing
    /// to shared memory. The cipher and current frame counter are used for
    /// authenticated encryption.
    ///
    /// # Arguments
    /// * `buffer` - Audio samples to write
    /// * `cipher` - The AudioCipher for encryption
    ///
    /// # Returns
    /// Number of frames written
    pub fn write_audio_encrypted(
        &mut self,
        buffer: &[f32],
        cipher: &crate::encryption::AudioCipher,
    ) -> usize {
        if !self.is_encrypted() {
            // Fall back to unencrypted write
            return self.write_audio(buffer);
        }

        let header = self.header();
        let channel_count = header.channel_count as usize;
        let sample_count = buffer.len();

        // Encrypt the samples
        let frame_counter = self.increment_frame_counter();
        let ciphertext = cipher.encrypt(buffer, frame_counter);

        // Store encrypted data as raw bytes in the ring buffer
        // We treat the f32 buffer as raw bytes for encrypted data
        let encrypted_samples = crate::encryption::encrypted_to_samples(&ciphertext);

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let used = (write_pos - read_pos) as usize;
        let available = self.audio_capacity - used;
        let to_write = encrypted_samples.len().min(available);

        if to_write == 0 || to_write < encrypted_samples.len() {
            // Not enough space for the full encrypted block
            return 0;
        }

        let write_index = (write_pos as usize) % self.audio_capacity;
        let first_part = to_write.min(self.audio_capacity - write_index);
        let second_part = to_write - first_part;

        unsafe {
            let audio_data = self.audio_data_mut();

            std::ptr::copy_nonoverlapping(
                encrypted_samples.as_ptr(),
                audio_data.add(write_index),
                first_part,
            );

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    encrypted_samples.as_ptr().add(first_part),
                    audio_data,
                    second_part,
                );
            }
        }

        self.header()
            .write_position
            .store(write_pos + to_write as u64, Ordering::Release);

        sample_count / channel_count
    }

    /// Read audio with decryption
    ///
    /// When encryption is enabled, this reads encrypted data from shared memory
    /// and decrypts it. Returns silence if decryption fails (tampered data).
    ///
    /// # Arguments
    /// * `buffer` - Buffer to fill with decrypted audio samples
    /// * `cipher` - The AudioCipher for decryption
    /// * `expected_frame_counter` - The expected frame counter for this block
    ///
    /// # Returns
    /// Number of frames read
    pub fn read_audio_encrypted(
        &self,
        buffer: &mut [f32],
        cipher: &crate::encryption::AudioCipher,
        expected_frame_counter: u64,
    ) -> usize {
        if !self.is_encrypted() {
            // Fall back to unencrypted read
            return self.read_audio(buffer);
        }

        let header = self.header();
        let channel_count = header.channel_count as usize;
        let sample_count = buffer.len();

        // Calculate expected encrypted size
        let encrypted_size =
            crate::encryption::AudioCipher::ciphertext_size(sample_count) / std::mem::size_of::<f32>() + 1;

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let available = (write_pos - read_pos) as usize;
        if available < encrypted_size {
            buffer.fill(0.0);
            return 0;
        }

        // Read the encrypted data
        let read_index = (read_pos as usize) % self.audio_capacity;
        let first_part = encrypted_size.min(self.audio_capacity - read_index);
        let second_part = encrypted_size - first_part;

        let mut encrypted_samples = vec![0.0f32; encrypted_size];

        unsafe {
            let audio_data = self.audio_data();

            std::ptr::copy_nonoverlapping(
                audio_data.add(read_index),
                encrypted_samples.as_mut_ptr(),
                first_part,
            );

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    audio_data,
                    encrypted_samples.as_mut_ptr().add(first_part),
                    second_part,
                );
            }
        }

        // Convert samples back to ciphertext bytes
        let ciphertext = crate::encryption::samples_to_encrypted(&encrypted_samples);

        // Decrypt
        match cipher.decrypt(&ciphertext, expected_frame_counter) {
            Some(decrypted) => {
                let to_copy = decrypted.len().min(sample_count);
                buffer[..to_copy].copy_from_slice(&decrypted[..to_copy]);
                if to_copy < sample_count {
                    buffer[to_copy..].fill(0.0);
                }

                // Update read position
                header
                    .read_position
                    .store(read_pos + encrypted_size as u64, Ordering::Release);

                to_copy / channel_count
            }
            None => {
                // Decryption failed - return silence
                log::warn!("Audio decryption failed (frame {})", expected_frame_counter);
                buffer.fill(0.0);
                0
            }
        }
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
///
/// Also provides configuration methods for setting sample rate, channel count,
/// and buffer frames.
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

    /// Set sample rate
    ///
    /// Updates the sample rate in the shared memory header and notifies the
    /// Swift HAL driver via the config_changed flag.
    ///
    /// # Arguments
    /// * `sample_rate` - New sample rate in Hz (e.g., 44100, 48000, 96000)
    ///
    /// # Returns
    /// `true` if the sample rate was set, `false` if not connected
    pub fn set_sample_rate(&mut self, sample_rate: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_sample_rate(sample_rate);
            true
        } else {
            false
        }
    }

    /// Get channel count
    pub fn channel_count(&self) -> u32 {
        self.buffer
            .as_ref()
            .map(|b| b.channel_count())
            .unwrap_or(2)
    }

    /// Set channel count
    ///
    /// # Arguments
    /// * `channel_count` - Number of audio channels (e.g., 2 for stereo, 6 for 5.1)
    ///
    /// # Returns
    /// `true` if the channel count was set, `false` if not connected
    pub fn set_channel_count(&mut self, channel_count: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_channel_count(channel_count);
            true
        } else {
            false
        }
    }

    /// Get buffer frame size
    pub fn buffer_frames(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.buffer_frames()).unwrap_or(1024)
    }

    /// Set buffer frame size
    ///
    /// # Arguments
    /// * `buffer_frames` - Frames per buffer (e.g., 256, 512, 1024, 2048)
    ///
    /// # Returns
    /// `true` if the buffer frames was set, `false` if not connected
    pub fn set_buffer_frames(&mut self, buffer_frames: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_buffer_frames(buffer_frames);
            true
        } else {
            false
        }
    }

    /// Set engine ready flag
    pub fn set_engine_ready(&self, ready: bool) {
        if let Some(buffer) = &self.buffer {
            buffer.set_engine_ready(ready);
        }
    }

    /// Check if configuration has changed (signaled by Swift driver)
    pub fn config_changed(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.config_changed())
            .unwrap_or(false)
    }

    /// Clear the configuration changed flag
    pub fn clear_config_changed(&self) {
        if let Some(buffer) = &self.buffer {
            buffer.clear_config_changed();
        }
    }

    /// Signal configuration change to the Swift driver
    pub fn set_config_changed(&self) {
        if let Some(buffer) = &self.buffer {
            buffer.set_config_changed();
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_header_size() {
        // Ensure header is packed correctly
        assert!(std::mem::size_of::<SharedAudioHeader>() <= 128);
    }

    /// Create a mock shared memory file for testing
    /// Returns the file path
    fn create_mock_shared_memory(
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
    ) -> NamedTempFile {
        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let audio_offset = (header_size + 63) & !63; // 64-byte aligned
        let audio_capacity = (buffer_frames as usize) * (channel_count as usize) * 8; // 8 buffers
        let total_size = audio_offset + audio_capacity * std::mem::size_of::<f32>();

        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Write header
        let header = SharedAudioHeader {
            magic: SHARED_MEMORY_MAGIC,
            version: SHARED_MEMORY_VERSION,
            sample_rate,
            buffer_frames,
            channel_count,
            write_position: AtomicU64::new(0),
            read_position: AtomicU64::new(0),
            active: AtomicU32::new(1),
            config_changed: AtomicU32::new(0),
            driver_ready: AtomicU32::new(1),
            engine_ready: AtomicU32::new(0),
            encrypted: AtomicU32::new(0),
            key_fingerprint: [0; 8],
            frame_counter: AtomicU64::new(0),
        };

        // Create buffer with header bytes
        let header_bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(&header as *const _ as *const u8, header_size) };

        // Write header + padding + audio data space
        let mut buffer = vec![0u8; total_size];
        buffer[..header_size].copy_from_slice(header_bytes);

        file.write_all(&buffer).expect("Failed to write to file");
        file.flush().expect("Failed to flush file");

        file
    }

    #[test]
    fn test_shared_memory_roundtrip_bit_exact() {
        // Create mock shared memory
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        // Open the shared memory buffer
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Verify header values
        assert_eq!(buffer.sample_rate(), sample_rate);
        assert_eq!(buffer.buffer_frames(), buffer_frames);
        assert_eq!(buffer.channel_count(), channel_count);
        assert!(buffer.driver_ready());

        // Create test audio data with known values
        let num_samples = buffer_frames as usize * channel_count as usize;
        let input_audio: Vec<f32> = (0..num_samples)
            .map(|i| {
                // Use a mix of values to test precision
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
            })
            .collect();

        // Write audio to shared memory
        let frames_written = buffer.write_audio(&input_audio);
        assert_eq!(
            frames_written,
            buffer_frames as usize,
            "Should write all frames"
        );

        // Read audio back
        let mut output_audio = vec![0.0f32; num_samples];
        let frames_read = buffer.read_audio(&mut output_audio);
        assert_eq!(
            frames_read,
            buffer_frames as usize,
            "Should read all frames"
        );

        // Verify bit-for-bit accuracy
        for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
            assert_eq!(
                input.to_bits(),
                output.to_bits(),
                "Sample {} mismatch: input={} (bits={:#x}), output={} (bits={:#x})",
                i,
                input,
                input.to_bits(),
                output,
                output.to_bits()
            );
        }
    }

    #[test]
    fn test_shared_memory_roundtrip_multiple_blocks() {
        let sample_rate = 48000;
        let buffer_frames = 256;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Write and read multiple blocks to test ring buffer wrap-around
        let num_blocks = 10;
        let samples_per_block = buffer_frames as usize * channel_count as usize;

        for block_idx in 0..num_blocks {
            // Create unique audio for each block
            let input_audio: Vec<f32> = (0..samples_per_block)
                .map(|i| {
                    let sample_idx = block_idx * samples_per_block + i;
                    let t = sample_idx as f32 / sample_rate as f32;
                    (2.0 * std::f32::consts::PI * (440.0 + block_idx as f32 * 100.0) * t).sin()
                        * 0.5
                })
                .collect();

            // Write
            let frames_written = buffer.write_audio(&input_audio);
            assert_eq!(
                frames_written,
                buffer_frames as usize,
                "Block {}: Should write all frames",
                block_idx
            );

            // Read
            let mut output_audio = vec![0.0f32; samples_per_block];
            let frames_read = buffer.read_audio(&mut output_audio);
            assert_eq!(
                frames_read,
                buffer_frames as usize,
                "Block {}: Should read all frames",
                block_idx
            );

            // Verify
            for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
                assert_eq!(
                    input.to_bits(),
                    output.to_bits(),
                    "Block {} Sample {}: mismatch",
                    block_idx,
                    i
                );
            }
        }
    }

    #[test]
    fn test_shared_memory_roundtrip_special_values() {
        let sample_rate = 48000;
        let buffer_frames = 32;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Test special float values
        let special_values: Vec<f32> = vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.5,
            -0.5,
            f32::MIN_POSITIVE,
            -f32::MIN_POSITIVE,
            f32::EPSILON,
            -f32::EPSILON,
            0.999999,
            -0.999999,
            // Typical audio values
            0.707,   // -3dB
            0.5012,  // -6dB
            0.251,   // -12dB
            0.1,     // -20dB
            0.0316,  // -30dB
            0.01,    // -40dB
            0.00316, // -50dB
            0.001,   // -60dB
        ];

        // Pad to fill buffer
        let num_samples = buffer_frames as usize * channel_count as usize;
        let mut input_audio = special_values.clone();
        while input_audio.len() < num_samples {
            input_audio.push(0.0);
        }
        input_audio.truncate(num_samples);

        // Write
        buffer.write_audio(&input_audio);

        // Read
        let mut output_audio = vec![0.0f32; num_samples];
        buffer.read_audio(&mut output_audio);

        // Verify bit-for-bit
        for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
            assert_eq!(
                input.to_bits(),
                output.to_bits(),
                "Sample {} ({}) mismatch",
                i,
                input
            );
        }
    }

    #[test]
    fn test_shared_memory_stereo_channel_separation() {
        let sample_rate = 48000;
        let buffer_frames = 128;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Create stereo audio with distinct left and right content
        let num_samples = buffer_frames as usize * channel_count as usize;
        let input_audio: Vec<f32> = (0..buffer_frames as usize)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let left = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // 440Hz on left
                let right = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.3; // 880Hz on right
                [left, right]
            })
            .collect();

        // Write and read
        buffer.write_audio(&input_audio);
        let mut output_audio = vec![0.0f32; num_samples];
        buffer.read_audio(&mut output_audio);

        // Verify channels are preserved correctly
        for i in 0..buffer_frames as usize {
            let left_in = input_audio[i * 2];
            let right_in = input_audio[i * 2 + 1];
            let left_out = output_audio[i * 2];
            let right_out = output_audio[i * 2 + 1];

            assert_eq!(
                left_in.to_bits(),
                left_out.to_bits(),
                "Frame {}: Left channel mismatch",
                i
            );
            assert_eq!(
                right_in.to_bits(),
                right_out.to_bits(),
                "Frame {}: Right channel mismatch",
                i
            );

            // Also verify left and right are different (sanity check)
            if i > 0 {
                assert_ne!(
                    left_in.to_bits(),
                    right_in.to_bits(),
                    "Frame {}: Left and right should be different",
                    i
                );
            }
        }
    }

    #[test]
    fn test_invalid_magic_number() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Write invalid header (wrong magic)
        let mut buffer = vec![0u8; 4096];

        // Write wrong magic number
        buffer[0..4].copy_from_slice(&0x12345678u32.to_ne_bytes());
        buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());

        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        match result {
            Err(e) => assert!(
                e.to_string().contains("Invalid shared memory magic"),
                "Expected 'Invalid shared memory magic' error, got: {}",
                e
            ),
            Ok(_) => panic!("Expected error for invalid magic number"),
        }
    }

    #[test]
    fn test_invalid_version() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Write header with wrong version
        let mut buffer = vec![0u8; 4096];

        // Correct magic, wrong version
        buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
        buffer[4..8].copy_from_slice(&99u32.to_ne_bytes()); // Invalid version

        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        match result {
            Err(e) => assert!(
                e.to_string().contains("Incompatible shared memory version"),
                "Expected 'Incompatible shared memory version' error, got: {}",
                e
            ),
            Ok(_) => panic!("Expected error for invalid version"),
        }
    }
}
