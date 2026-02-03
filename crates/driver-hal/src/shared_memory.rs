//! Shared memory interface for communication with Swift HAL driver
//!
//! This module provides a Rust interface to the shared memory region
//! created by the Swift HAL driver for audio data exchange.

use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::atomic::{fence, AtomicU32, AtomicU64, Ordering};

use memmap2::MmapMut;

/// Magic number for shared memory header validation: 'SOTF'
const SHARED_MEMORY_MAGIC: u32 = 0x534F5446;

/// Current protocol version
/// Version 2: Added encryption fields (encrypted, key_fingerprint, frame_counter)
/// Version 3: Added config negotiation fields for bidirectional HAL-Daemon sync
const SHARED_MEMORY_VERSION: u32 = 3;

/// Get the shared memory path for the current user
///
/// Security model: each user has their own shared memory region.
/// Path is based on the user's UID to match the Swift HAL driver's path.
///
/// IMPORTANT: This must match the Swift side in SharedMemory.swift which uses:
/// `/tmp/sotf-{uid}/audio.shm`
pub fn get_shared_memory_path() -> std::path::PathBuf {
    // Use UID-based path to match Swift HAL driver
    // Note: Swift HAL driver runs as _coreaudiod but uses the console user's UID
    // via SCDynamicStoreCopyConsoleUser to determine the path
    let uid = unsafe { libc::getuid() };
    std::path::PathBuf::from(format!("/tmp/sotf-{}/audio.shm", uid))
}

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

    // Config negotiation fields (version 3+)
    /// Requested sample rate (set by requester, either HAL or Daemon)
    pub requested_sample_rate: u32,
    /// Requested buffer frames (set by requester)
    pub requested_buffer_frames: u32,
    /// Actual sample rate in use (set by responder after negotiation)
    pub actual_sample_rate: u32,
    /// Actual buffer frames in use (set by responder after negotiation)
    pub actual_buffer_frames: u32,
    /// Config status: 0=pending, 1=accepted, 2=negotiated, 3=error
    pub config_status: AtomicU32,
    /// Config source: 1=HAL initiated, 2=Daemon initiated
    pub config_source: AtomicU32,
    /// Error code if config_status=3
    pub config_error_code: u32,
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

        // Calculate audio capacity: buffer_frames * channel_count * 8 ring buffers
        // Validate to prevent arithmetic overflow and ensure reasonable values
        let buffer_frames = header.buffer_frames as usize;
        let channel_count = header.channel_count as usize;

        if buffer_frames == 0 || channel_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid shared memory configuration: buffer_frames={}, channel_count={}",
                    buffer_frames, channel_count
                ),
            ));
        }

        // Check for reasonable limits (max 16 channels, max 64k frames)
        if channel_count > 16 || buffer_frames > 65536 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Shared memory configuration out of range: buffer_frames={}, channel_count={}",
                    buffer_frames, channel_count
                ),
            ));
        }

        let audio_capacity = buffer_frames * channel_count * 8; // 8 ring buffers

        // Verify mmap is large enough
        let required_size = audio_offset + audio_capacity * std::mem::size_of::<f32>();
        if mmap.len() < required_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Shared memory too small: {} bytes, need {} bytes",
                    mmap.len(),
                    required_size
                ),
            ));
        }

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
    ///
    /// # Thread Safety
    /// This uses an atomic fetch_add which is safe for concurrent use.
    /// Each call is guaranteed to return a unique value.
    ///
    /// # Nonce Safety
    /// The frame counter is used as a nonce for encryption. It must NEVER
    /// be reused with the same key. The atomic operation guarantees uniqueness
    /// even under concurrent access from multiple threads.
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

    // =========================================================================
    // Config negotiation methods (version 3+)
    // =========================================================================

    /// Get the requested sample rate (set by the config change requester)
    ///
    /// # Memory Ordering
    /// Caller should check `config_changed()` first, which performs an Acquire load.
    /// If config_changed is true, the non-atomic fields will be visible due to the
    /// Release fence on the writer side.
    pub fn requested_sample_rate(&self) -> u32 {
        // Acquire fence to ensure we see the latest value after checking config_changed
        fence(Ordering::Acquire);
        self.header().requested_sample_rate
    }

    /// Get the requested buffer frames (set by the config change requester)
    ///
    /// # Memory Ordering
    /// Caller should check `config_changed()` first, which performs an Acquire load.
    pub fn requested_buffer_frames(&self) -> u32 {
        // Acquire fence to ensure we see the latest value after checking config_changed
        fence(Ordering::Acquire);
        self.header().requested_buffer_frames
    }

    /// Set the actual sample rate (response from the handler)
    pub fn set_actual_sample_rate(&mut self, rate: u32) {
        self.header_mut().actual_sample_rate = rate;
    }

    /// Get the actual sample rate
    pub fn actual_sample_rate(&self) -> u32 {
        self.header().actual_sample_rate
    }

    /// Set the actual buffer frames (response from the handler)
    pub fn set_actual_buffer_frames(&mut self, frames: u32) {
        self.header_mut().actual_buffer_frames = frames;
    }

    /// Get the actual buffer frames
    pub fn actual_buffer_frames(&self) -> u32 {
        self.header().actual_buffer_frames
    }

    /// Get the config status
    /// 0=pending, 1=accepted, 2=negotiated, 3=error
    pub fn config_status(&self) -> u32 {
        self.header().config_status.load(Ordering::Acquire)
    }

    /// Set the config status (atomic)
    /// 0=pending, 1=accepted, 2=negotiated, 3=error
    pub fn set_config_status(&self, status: u32) {
        self.header().config_status.store(status, Ordering::Release);
    }

    /// Get the config source
    /// 1=HAL initiated, 2=Daemon initiated
    pub fn config_source(&self) -> u32 {
        self.header().config_source.load(Ordering::Acquire)
    }

    /// Set the config source (atomic)
    /// 1=HAL initiated, 2=Daemon initiated
    pub fn set_config_source(&self, source: u32) {
        self.header().config_source.store(source, Ordering::Release);
    }

    /// Get the config error code (only valid when config_status=3)
    pub fn config_error_code(&self) -> u32 {
        self.header().config_error_code
    }

    /// Set the config error code
    pub fn set_config_error_code(&mut self, code: u32) {
        self.header_mut().config_error_code = code;
    }

    /// Request a config change (called by the requester - HAL or Daemon)
    ///
    /// # Arguments
    /// * `sample_rate` - Requested sample rate in Hz
    /// * `buffer_frames` - Requested buffer frames
    /// * `source` - Who is requesting: 1=HAL, 2=Daemon
    ///
    /// # Memory Ordering
    /// Non-atomic fields are written first, then a Release fence ensures they
    /// are visible before the atomic notification flags are set. This prevents
    /// the responder from reading incomplete data.
    pub fn request_config_change(&mut self, sample_rate: u32, buffer_frames: u32, source: u32) {
        let header = self.header_mut();
        header.requested_sample_rate = sample_rate;
        header.requested_buffer_frames = buffer_frames;
        // Release fence ensures non-atomic writes are visible before atomic flags
        fence(Ordering::Release);
        // Set status to pending before setting config_source and config_changed
        self.header().config_status.store(0, Ordering::Relaxed);
        self.header().config_source.store(source, Ordering::Relaxed);
        // Final Release store acts as the notification point
        self.header().config_changed.store(1, Ordering::Release);
    }

    /// Acknowledge a config change (called by the handler after processing)
    ///
    /// # Arguments
    /// * `actual_rate` - The sample rate that will actually be used
    /// * `actual_frames` - The buffer frames that will actually be used
    /// * `status` - Result status: 1=accepted, 2=negotiated, 3=error
    /// * `error_code` - Error code if status=3, otherwise 0
    ///
    /// # Memory Ordering
    /// Non-atomic fields are written first, then a Release fence ensures they
    /// are visible before the atomic status flag is set. This prevents the
    /// requester from reading incomplete response data.
    pub fn acknowledge_config_change(
        &mut self,
        actual_rate: u32,
        actual_frames: u32,
        status: u32,
        error_code: u32,
    ) {
        let header = self.header_mut();
        header.actual_sample_rate = actual_rate;
        header.actual_buffer_frames = actual_frames;
        header.config_error_code = error_code;
        // Release fence ensures non-atomic writes are visible before atomic status
        fence(Ordering::Release);
        // Set status last (acts as "ready" flag for the requester)
        self.header().config_status.store(status, Ordering::Release);
        // Clear config_changed to signal we've handled it
        self.header().config_changed.store(0, Ordering::Release);
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
        let new_read_pos = read_pos + to_read as u64;
        header
            .read_position
            .store(new_read_pos, Ordering::Release);

        // TRACE: Log frames consumed from shared memory by Rust daemon
        let frames_read = to_read / channel_count;
        if frames_read > 0 {
            log::debug!(
                "[SHM TRACE] Rust read: {} frames, wpos={}, rpos={}",
                frames_read,
                write_pos,
                new_read_pos
            );
        }

        frames_read
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
        let new_write_pos = write_pos + to_write as u64;
        self.header()
            .write_position
            .store(new_write_pos, Ordering::Release);

        // TRACE: Log frames pushed to shared memory by Rust daemon
        let frames_written = to_write / channel_count;
        if frames_written > 0 {
            log::debug!(
                "[SHM TRACE] Rust write: {} frames, wpos={}, rpos={}",
                frames_written,
                new_write_pos,
                read_pos
            );
        }

        frames_written
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
    /// to shared memory. The nonce (frame counter) is prepended to the encrypted
    /// block so the reader can decrypt without external state.
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

        // Prepend nonce (8 bytes big-endian) to ciphertext
        let mut payload = Vec::with_capacity(8 + ciphertext.len());
        payload.extend_from_slice(&frame_counter.to_be_bytes());
        payload.extend_from_slice(&ciphertext);

        // Store as f32 slots in the ring buffer
        let encrypted_samples = crate::encryption::encrypted_to_samples(&payload);

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let used = (write_pos - read_pos) as usize;
        let available = self.audio_capacity - used;
        let to_write = encrypted_samples.len();

        if to_write > available {
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
    /// The nonce (frame counter) is stored at the start of each encrypted block,
    /// so no external tracking is needed.
    ///
    /// # Arguments
    /// * `buffer` - Buffer to fill with decrypted audio samples
    /// * `cipher` - The AudioCipher for decryption
    ///
    /// # Returns
    /// Number of frames read
    pub fn read_audio_encrypted(
        &self,
        buffer: &mut [f32],
        cipher: &crate::encryption::AudioCipher,
    ) -> usize {
        if !self.is_encrypted() {
            // Fall back to unencrypted read
            return self.read_audio(buffer);
        }

        let header = self.header();
        let channel_count = header.channel_count as usize;
        let sample_count = buffer.len();

        // Calculate expected encrypted size in f32 "slots"
        // Format: [8-byte nonce] [ciphertext + 16-byte tag]
        // ciphertext_size returns bytes for samples + tag, add 8 for nonce
        let ciphertext_bytes = crate::encryption::AudioCipher::ciphertext_size(sample_count);
        let total_bytes = 8 + ciphertext_bytes; // 8 bytes for nonce prefix
        let encrypted_size = (total_bytes + std::mem::size_of::<f32>() - 1) / std::mem::size_of::<f32>();

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let available = (write_pos - read_pos) as usize;
        if available < encrypted_size {
            buffer.fill(0.0);
            return 0;
        }

        // Read the encrypted data (includes nonce prefix)
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

        // Convert samples back to bytes
        let all_bytes = crate::encryption::samples_to_encrypted(&encrypted_samples);

        // Extract nonce (first 8 bytes) and ciphertext (rest)
        if all_bytes.len() < 8 {
            log::warn!("Encrypted block too small for nonce");
            buffer.fill(0.0);
            return 0;
        }

        let frame_counter = u64::from_be_bytes(all_bytes[..8].try_into().unwrap());
        let ciphertext = &all_bytes[8..8 + ciphertext_bytes];

        // Decrypt
        match cipher.decrypt(ciphertext, frame_counter) {
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
                log::warn!("Audio decryption failed (frame {})", frame_counter);
                buffer.fill(0.0);
                0
            }
        }
    }

    /// Read audio with decryption using pre-allocated buffers (allocation-free hot path)
    ///
    /// # Arguments
    /// * `buffer` - Buffer to fill with decrypted audio samples
    /// * `cipher` - The AudioCipher for decryption
    /// * `encrypted_buf` - Pre-allocated buffer for encrypted f32 slots
    /// * `ciphertext_buf` - Pre-allocated buffer for ciphertext bytes
    ///
    /// # Returns
    /// Number of frames read
    pub fn read_audio_encrypted_into(
        &self,
        buffer: &mut [f32],
        cipher: &crate::encryption::AudioCipher,
        encrypted_buf: &mut Vec<f32>,
        ciphertext_buf: &mut Vec<u8>,
    ) -> usize {
        if !self.is_encrypted() {
            return self.read_audio(buffer);
        }

        let header = self.header();
        let channel_count = header.channel_count as usize;
        let sample_count = buffer.len();

        // Calculate expected encrypted size
        let ciphertext_bytes = crate::encryption::AudioCipher::ciphertext_size(sample_count);
        let total_bytes = 8 + ciphertext_bytes; // 8 bytes for nonce prefix
        let encrypted_size = (total_bytes + 3) / 4; // Round up to f32 slots

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let available = (write_pos - read_pos) as usize;
        if available < encrypted_size {
            buffer.fill(0.0);
            return 0;
        }

        // Ensure pre-allocated buffers are large enough (resize only if needed)
        if encrypted_buf.len() < encrypted_size {
            encrypted_buf.resize(encrypted_size, 0.0);
        }
        if ciphertext_buf.len() < total_bytes {
            ciphertext_buf.resize(total_bytes, 0);
        }

        // Read encrypted data from ring buffer
        let read_index = (read_pos as usize) % self.audio_capacity;
        let first_part = encrypted_size.min(self.audio_capacity - read_index);
        let second_part = encrypted_size - first_part;

        unsafe {
            let audio_data = self.audio_data();

            std::ptr::copy_nonoverlapping(
                audio_data.add(read_index),
                encrypted_buf.as_mut_ptr(),
                first_part,
            );

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    audio_data,
                    encrypted_buf.as_mut_ptr().add(first_part),
                    second_part,
                );
            }
        }

        // Convert samples to bytes using pre-allocated buffer
        crate::encryption::samples_to_encrypted_into(&encrypted_buf[..encrypted_size], ciphertext_buf);

        // Extract nonce and decrypt
        if total_bytes < 8 {
            log::warn!("Encrypted block too small for nonce");
            buffer.fill(0.0);
            return 0;
        }

        let frame_counter = u64::from_be_bytes(ciphertext_buf[..8].try_into().unwrap());
        let ciphertext = &ciphertext_buf[8..8 + ciphertext_bytes];

        // Decrypt directly into output buffer
        match cipher.decrypt_into(ciphertext, frame_counter, buffer) {
            Some(decrypted_count) => {
                if decrypted_count < sample_count {
                    buffer[decrypted_count..].fill(0.0);
                }

                header
                    .read_position
                    .store(read_pos + encrypted_size as u64, Ordering::Release);

                decrypted_count / channel_count
            }
            None => {
                log::warn!("Audio decryption failed (frame {})", frame_counter);
                buffer.fill(0.0);
                0
            }
        }
    }

    /// Write audio with encryption using pre-allocated buffers (allocation-free hot path)
    ///
    /// # Arguments
    /// * `samples` - Audio samples to write
    /// * `cipher` - The AudioCipher for encryption
    /// * `ciphertext_buf` - Pre-allocated buffer for ciphertext bytes (must be >= encrypted_byte_size + 8)
    /// * `encrypted_buf` - Pre-allocated buffer for encrypted f32 slots
    ///
    /// # Returns
    /// Number of frames written
    pub fn write_audio_encrypted_into(
        &mut self,
        samples: &[f32],
        cipher: &crate::encryption::AudioCipher,
        ciphertext_buf: &mut Vec<u8>,
        encrypted_buf: &mut Vec<f32>,
    ) -> usize {
        if !self.is_encrypted() {
            return self.write_audio(samples);
        }

        let header = self.header();
        let channel_count = header.channel_count as usize;
        let sample_count = samples.len();

        // Calculate sizes
        let ciphertext_size = crate::encryption::encrypted_byte_size(sample_count);
        let total_bytes = 8 + ciphertext_size; // 8 bytes nonce + ciphertext
        let encrypted_slots = (total_bytes + 3) / 4;

        // Ensure pre-allocated buffers are large enough
        if ciphertext_buf.len() < total_bytes {
            ciphertext_buf.resize(total_bytes, 0);
        }
        if encrypted_buf.len() < encrypted_slots {
            encrypted_buf.resize(encrypted_slots, 0.0);
        }

        // Get frame counter and write nonce
        let frame_counter = self.increment_frame_counter();
        ciphertext_buf[..8].copy_from_slice(&frame_counter.to_be_bytes());

        // Encrypt directly into the buffer after nonce
        match cipher.encrypt_into(samples, frame_counter, &mut ciphertext_buf[8..8 + ciphertext_size]) {
            Some(_) => {}
            None => {
                log::error!("Encryption failed - buffer too small");
                return 0;
            }
        }

        // Convert bytes to f32 slots
        crate::encryption::encrypted_to_samples_into(&ciphertext_buf[..total_bytes], encrypted_buf);

        // Write to ring buffer
        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let used = (write_pos - read_pos) as usize;
        let available = self.audio_capacity - used;

        if encrypted_slots > available {
            return 0;
        }

        let write_index = (write_pos as usize) % self.audio_capacity;
        let first_part = encrypted_slots.min(self.audio_capacity - write_index);
        let second_part = encrypted_slots - first_part;

        unsafe {
            let audio_data = self.audio_data_mut();

            std::ptr::copy_nonoverlapping(
                encrypted_buf.as_ptr(),
                audio_data.add(write_index),
                first_part,
            );

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    encrypted_buf.as_ptr().add(first_part),
                    audio_data,
                    second_part,
                );
            }
        }

        self.header()
            .write_position
            .store(write_pos + encrypted_slots as u64, Ordering::Release);

        sample_count / channel_count
    }
}

// =============================================================================
// Adapter types for compatibility with existing code
// =============================================================================

/// Reader adapter for HAL input (compatible with old HalInputReader API)
pub struct HalInputReader {
    buffer: Option<SharedAudioBuffer>,
    cipher: Option<crate::encryption::AudioCipher>,
    /// Pre-allocated buffer for reading encrypted f32 slots (avoids allocation in hot path)
    encrypted_samples_buf: Vec<f32>,
    /// Pre-allocated buffer for ciphertext bytes (avoids allocation in hot path)
    ciphertext_buf: Vec<u8>,
}

impl HalInputReader {
    /// Create a new HAL input reader
    pub fn new() -> Option<Self> {
        let path = get_shared_memory_path();
        log::info!("[HAL INPUT] Attempting to open SharedMemory at: {:?}", path);

        match SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                log::info!(
                    "[HAL INPUT] SharedMemory opened successfully: sample_rate={}, buffer_frames={}, channels={}, driver_ready={}, active={}",
                    buffer.sample_rate(),
                    buffer.buffer_frames(),
                    buffer.channel_count(),
                    buffer.driver_ready(),
                    buffer.is_active()
                );
                // Pre-allocate buffers for typical frame size (2048 samples * channels)
                // Will be resized if needed
                let typical_samples = 2048 * buffer.channel_count() as usize;
                let encrypted_slots = crate::encryption::encrypted_sample_slots(typical_samples);
                Some(Self {
                    buffer: Some(buffer),
                    cipher: None,
                    encrypted_samples_buf: Vec::with_capacity(encrypted_slots),
                    ciphertext_buf: Vec::with_capacity(crate::encryption::encrypted_byte_size(typical_samples) + 8),
                })
            }
            Err(e) => {
                log::error!("[HAL INPUT] Failed to open SharedMemory: {}", e);
                None
            }
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
    pub fn read(&mut self, buffer: &mut [f32]) -> usize {
        // Static counter for periodic logging
        static READ_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = READ_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if let Some(buf) = &self.buffer {
            // Log state every 100 reads (~2 seconds)
            if count % 100 == 0 {
                let header = buf.header();
                let write_pos = header.write_position.load(std::sync::atomic::Ordering::Acquire);
                let read_pos = header.read_position.load(std::sync::atomic::Ordering::Acquire);
                let available = (write_pos - read_pos) as usize;
                let channel_count = header.channel_count as usize;
                let available_frames = if channel_count > 0 { available / channel_count } else { 0 };

                log::info!(
                    "[HAL INPUT] State: wpos={}, rpos={}, available={} frames, driver_ready={}, engine_ready={}, active={}",
                    write_pos,
                    read_pos,
                    available_frames,
                    header.driver_ready.load(std::sync::atomic::Ordering::Acquire) != 0,
                    header.engine_ready.load(std::sync::atomic::Ordering::Acquire) != 0,
                    header.active.load(std::sync::atomic::Ordering::Acquire) != 0
                );
            }

            if buf.is_encrypted() {
                // Check if we need to load/reload cipher
                let header_fingerprint = buf.key_fingerprint();
                let need_reload = self.cipher.as_ref().map_or(true, |c| c.fingerprint() != &header_fingerprint);

                if need_reload {
                    log::debug!("Encryption enabled/changed, loading key...");
                    match crate::encryption::load_session_key() {
                        Ok(key) => {
                            let cipher = crate::encryption::AudioCipher::new(&key);
                            if cipher.fingerprint() == &header_fingerprint {
                                self.cipher = Some(cipher);
                                log::debug!("Loaded encryption key, fingerprint matches");
                            } else {
                                log::error!("Loaded key fingerprint mismatch! Expected {:?}, got {:?}", header_fingerprint, cipher.fingerprint());
                                self.cipher = None;
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to load encryption key: {}", e);
                            self.cipher = None;
                        }
                    }
                }

                if let Some(cipher) = &self.cipher {
                    // Use allocation-free version with pre-allocated buffers
                    return buf.read_audio_encrypted_into(
                        buffer,
                        cipher,
                        &mut self.encrypted_samples_buf,
                        &mut self.ciphertext_buf,
                    );
                } else {
                    // Encrypted but no key -> return silence
                    buffer.fill(0.0);
                    return 0;
                }
            }

            // Not encrypted
            buf.read_audio(buffer)
        } else {
            if count % 100 == 0 {
                log::warn!("[HAL INPUT] No buffer available for read");
            }
            0
        }
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

    /// Get available frames to read
    pub fn available_read_frames(&self) -> usize {
        self.buffer
            .as_ref()
            .map(|b| b.available_read_frames())
            .unwrap_or(0)
    }
}

impl Default for HalInputReader {
    fn default() -> Self {
        Self {
            buffer: None,
            cipher: None,
            encrypted_samples_buf: Vec::new(),
            ciphertext_buf: Vec::new(),
        }
    }
}

/// Writer adapter for HAL output (compatible with old HalOutputWriter API)
///
/// Also provides configuration methods for setting sample rate, channel count,
/// and buffer frames.
pub struct HalOutputWriter {
    buffer: Option<SharedAudioBuffer>,
    cipher: Option<crate::encryption::AudioCipher>,
    /// Pre-allocated buffer for ciphertext bytes (avoids allocation in hot path)
    ciphertext_buf: Vec<u8>,
    /// Pre-allocated buffer for encrypted f32 slots (avoids allocation in hot path)
    encrypted_buf: Vec<f32>,
}

impl HalOutputWriter {
    /// Create a new HAL output writer
    pub fn new() -> Option<Self> {
        match SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                // Pre-allocate buffers for typical frame size
                let typical_samples = 2048 * buffer.channel_count() as usize;
                let ciphertext_size = crate::encryption::encrypted_byte_size(typical_samples) + 8;
                let encrypted_slots = crate::encryption::encrypted_sample_slots(typical_samples);
                Some(Self {
                    buffer: Some(buffer),
                    cipher: None,
                    ciphertext_buf: Vec::with_capacity(ciphertext_size),
                    encrypted_buf: Vec::with_capacity(encrypted_slots),
                })
            }
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
        // We need to split the borrow to access both buffer and other fields
        let is_encrypted = self.buffer.as_ref().map_or(false, |b| b.is_encrypted());

        if is_encrypted {
            // Check if we need to load/reload cipher
            let header_fingerprint = self.buffer.as_ref().map(|b| b.key_fingerprint());
            let need_reload = match (&self.cipher, header_fingerprint) {
                (Some(c), Some(fp)) => c.fingerprint() != &fp,
                (None, Some(_)) => true,
                _ => false,
            };

            if need_reload {
                match crate::encryption::load_session_key() {
                    Ok(key) => {
                        let cipher = crate::encryption::AudioCipher::new(&key);
                        if let Some(fp) = header_fingerprint {
                            if cipher.fingerprint() == &fp {
                                self.cipher = Some(cipher);
                            } else {
                                self.cipher = None;
                            }
                        }
                    }
                    Err(_) => {
                        self.cipher = None;
                    }
                }
            }

            if self.cipher.is_some() {
                // Use allocation-free version with pre-allocated buffers
                if let Some(buf) = &mut self.buffer {
                    let cipher = self.cipher.as_ref().unwrap();
                    return buf.write_audio_encrypted_into(
                        buffer,
                        cipher,
                        &mut self.ciphertext_buf,
                        &mut self.encrypted_buf,
                    );
                }
            }
            // Encrypted but no key -> don't write
            return 0;
        }

        // Not encrypted
        if let Some(buf) = &mut self.buffer {
            buf.write_audio(buffer)
        } else {
            0
        }
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
        Self {
            buffer: None,
            cipher: None,
            ciphertext_buf: Vec::new(),
            encrypted_buf: Vec::new(),
        }
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
        // Version 3 added config negotiation fields, so header grew
        assert!(std::mem::size_of::<SharedAudioHeader>() <= 192);
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
            // Config negotiation fields (version 3+)
            requested_sample_rate: 0,
            requested_buffer_frames: 0,
            actual_sample_rate: sample_rate,
            actual_buffer_frames: buffer_frames,
            config_status: AtomicU32::new(0),
            config_source: AtomicU32::new(0),
            config_error_code: 0,
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

    // ==========================================================================
    // Validation Tests - catch invalid configurations and race conditions
    // ==========================================================================

    #[test]
    fn test_invalid_zero_buffer_frames() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        // Create header with buffer_frames = 0
        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let mut buffer = vec![0u8; header_size + 4096];

        // Write valid magic and version
        buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
        buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
        // sample_rate
        buffer[8..12].copy_from_slice(&48000u32.to_ne_bytes());
        // buffer_frames = 0 (INVALID)
        buffer[12..16].copy_from_slice(&0u32.to_ne_bytes());
        // channel_count
        buffer[16..20].copy_from_slice(&2u32.to_ne_bytes());

        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        assert!(
            result.is_err(),
            "Should reject shared memory with buffer_frames=0"
        );
        let err = result.err().expect("Expected error");
        assert!(
            err.to_string().contains("Invalid shared memory configuration"),
            "Expected 'Invalid shared memory configuration' error, got: {}",
            err
        );
    }

    #[test]
    fn test_invalid_zero_channel_count() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let mut buffer = vec![0u8; header_size + 4096];

        buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
        buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
        buffer[8..12].copy_from_slice(&48000u32.to_ne_bytes());
        buffer[12..16].copy_from_slice(&1024u32.to_ne_bytes());
        // channel_count = 0 (INVALID)
        buffer[16..20].copy_from_slice(&0u32.to_ne_bytes());

        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        assert!(
            result.is_err(),
            "Should reject shared memory with channel_count=0"
        );
        let err = result.err().expect("Expected error");
        assert!(
            err.to_string().contains("Invalid shared memory configuration"),
            "Expected 'Invalid shared memory configuration' error, got: {}",
            err
        );
    }

    #[test]
    fn test_excessive_channel_count() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let mut buffer = vec![0u8; header_size + 4096];

        buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
        buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
        buffer[8..12].copy_from_slice(&48000u32.to_ne_bytes());
        buffer[12..16].copy_from_slice(&1024u32.to_ne_bytes());
        // channel_count = 100 (exceeds max of 16)
        buffer[16..20].copy_from_slice(&100u32.to_ne_bytes());

        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        assert!(
            result.is_err(),
            "Should reject shared memory with excessive channel_count"
        );
        let err = result.err().expect("Expected error");
        assert!(
            err.to_string().contains("out of range"),
            "Expected 'out of range' error, got: {}",
            err
        );
    }

    #[test]
    fn test_excessive_buffer_frames() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let mut buffer = vec![0u8; header_size + 4096];

        buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
        buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
        buffer[8..12].copy_from_slice(&48000u32.to_ne_bytes());
        // buffer_frames = 1000000 (exceeds max of 65536)
        buffer[12..16].copy_from_slice(&1000000u32.to_ne_bytes());
        buffer[16..20].copy_from_slice(&2u32.to_ne_bytes());

        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        assert!(
            result.is_err(),
            "Should reject shared memory with excessive buffer_frames"
        );
        let err = result.err().expect("Expected error");
        assert!(
            err.to_string().contains("out of range"),
            "Expected 'out of range' error, got: {}",
            err
        );
    }

    #[test]
    fn test_mmap_too_small() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");

        let header_size = std::mem::size_of::<SharedAudioHeader>();
        // Create buffer that's too small for the claimed configuration
        let mut buffer = vec![0u8; header_size + 100]; // Way too small

        buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
        buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
        buffer[8..12].copy_from_slice(&48000u32.to_ne_bytes());
        buffer[12..16].copy_from_slice(&1024u32.to_ne_bytes()); // Claims 1024 frames
        buffer[16..20].copy_from_slice(&2u32.to_ne_bytes()); // 2 channels

        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        assert!(
            result.is_err(),
            "Should reject shared memory that's too small for claimed configuration"
        );
        let err = result.err().expect("Expected error");
        assert!(
            err.to_string().contains("too small"),
            "Expected 'too small' error, got: {}",
            err
        );
    }

    #[test]
    fn test_config_negotiation_round_trip() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Simulate config request from HAL driver
        let new_sample_rate = 96000;
        let new_buffer_frames = 512;
        buffer.request_config_change(new_sample_rate, new_buffer_frames, 1); // source=1 (HAL)

        // Verify request is visible
        assert!(buffer.config_changed(), "Config change should be flagged");
        assert_eq!(buffer.config_source(), 1, "Source should be HAL");
        assert_eq!(
            buffer.requested_sample_rate(),
            new_sample_rate,
            "Requested sample rate should be set"
        );
        assert_eq!(
            buffer.requested_buffer_frames(),
            new_buffer_frames,
            "Requested buffer frames should be set"
        );

        // Simulate daemon acknowledging with negotiated values
        let actual_rate = 96000;
        let actual_frames = 512;
        buffer.acknowledge_config_change(actual_rate, actual_frames, 1, 0); // status=1 (accepted)

        // Verify acknowledgment
        assert!(!buffer.config_changed(), "Config change flag should be cleared");
        assert_eq!(buffer.config_status(), 1, "Status should be accepted");
        assert_eq!(buffer.actual_sample_rate(), actual_rate);
        assert_eq!(buffer.actual_buffer_frames(), actual_frames);
    }

    #[test]
    fn test_config_negotiation_error() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Request an invalid sample rate
        buffer.request_config_change(999999, 512, 1);

        // Daemon rejects with error
        buffer.acknowledge_config_change(0, 0, 3, 42); // status=3 (error), error_code=42

        assert_eq!(buffer.config_status(), 3, "Status should be error");
        assert_eq!(buffer.config_error_code(), 42, "Error code should be set");
    }

    #[test]
    fn test_frame_counter_increment() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Frame counter should start at 0
        assert_eq!(buffer.frame_counter(), 0, "Frame counter should start at 0");

        // Increment and verify
        let new_counter = buffer.increment_frame_counter();
        assert_eq!(new_counter, 1, "First increment should return 1");
        assert_eq!(buffer.frame_counter(), 1, "Frame counter should be 1");

        // Multiple increments should be monotonic
        for expected in 2..=100 {
            let counter = buffer.increment_frame_counter();
            assert_eq!(counter, expected, "Counter should be monotonically increasing");
        }
    }

    #[test]
    fn test_engine_ready_flag() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Engine ready flag can be set (used by daemon to signal readiness to HAL driver)
        // We can verify the underlying atomic value directly
        let engine_ready = buffer.header().engine_ready.load(Ordering::Acquire);
        assert_eq!(engine_ready, 0, "Engine should start not ready");

        // Set engine ready
        buffer.set_engine_ready(true);
        let engine_ready = buffer.header().engine_ready.load(Ordering::Acquire);
        assert_eq!(engine_ready, 1, "Engine should now be ready");

        // Clear engine ready
        buffer.set_engine_ready(false);
        let engine_ready = buffer.header().engine_ready.load(Ordering::Acquire);
        assert_eq!(engine_ready, 0, "Engine should now be not ready");
    }

    #[test]
    fn test_encryption_flag() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Should start unencrypted
        assert!(!buffer.is_encrypted(), "Should start unencrypted");

        // Set encrypted with fingerprint
        let fingerprint = [1, 2, 3, 4, 5, 6, 7, 8];
        buffer.set_encrypted(true);
        buffer.set_key_fingerprint(fingerprint);

        assert!(buffer.is_encrypted(), "Should now be encrypted");
        assert_eq!(buffer.key_fingerprint(), fingerprint, "Fingerprint should match");

        // Disable encryption
        buffer.set_encrypted(false);
        assert!(!buffer.is_encrypted(), "Should now be unencrypted");
    }

    #[test]
    fn test_key_fingerprint_mismatch_detection() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Set encryption with specific fingerprint
        let fingerprint = [0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44];
        buffer.set_encrypted(true);
        buffer.set_key_fingerprint(fingerprint);

        // Verify we can detect a different fingerprint
        let different_fingerprint = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        assert_ne!(
            buffer.key_fingerprint(),
            different_fingerprint,
            "Should detect fingerprint mismatch"
        );
    }

    #[test]
    fn test_active_flag() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        // Should start active (set in create_mock_shared_memory)
        assert!(buffer.is_active(), "Should start active");

        // Note: There's no set_active method - the active flag is controlled by the HAL driver
        // We can only read it from the Rust side
    }

    #[test]
    fn test_multichannel_configurations() {
        // Test various channel configurations (stereo, 5.1, 7.1, etc.)
        let configurations = vec![
            (2, "Stereo"),
            (6, "5.1 Surround"),
            (8, "7.1 Surround"),
            (16, "Maximum supported"),
        ];

        for (channel_count, name) in configurations {
            let temp_file = create_mock_shared_memory(48000, 256, channel_count);
            let buffer = SharedAudioBuffer::open(temp_file.path())
                .unwrap_or_else(|_| panic!("Failed to open {} buffer", name));

            assert_eq!(
                buffer.channel_count(),
                channel_count,
                "{} channel count mismatch",
                name
            );

            // Write and read test data
            let samples = vec![0.5f32; 256 * channel_count as usize];
            let mut output = vec![0.0f32; 256 * channel_count as usize];

            let mut buffer = buffer;
            buffer.write_audio(&samples);
            buffer.read_audio(&mut output);

            // Verify all samples match
            for (i, (input, output)) in samples.iter().zip(output.iter()).enumerate() {
                assert_eq!(
                    input.to_bits(),
                    output.to_bits(),
                    "{}: Sample {} mismatch",
                    name,
                    i
                );
            }
        }
    }
}
