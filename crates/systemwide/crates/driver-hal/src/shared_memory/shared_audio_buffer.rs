use super::SharedAudioHeader;
use super::consts::ENCRYPTED_RECORD_HEADER_BYTES;
use super::consts::ENCRYPTED_RECORD_HEADER_SLOTS;
use super::consts::MAX_HAL_BUFFER_FRAMES;
use super::consts::MAX_HAL_CHANNEL_COUNT;
use super::consts::RECONFIG_QUIESCE_TIMEOUT_NS;
use super::consts::SHARED_MEMORY_MAGIC;
use super::consts::SHARED_MEMORY_VERSION;
use super::consts::parse_encrypted_record_header;
use super::consts::write_encrypted_record_header;
use super::current::current_unix_millis;
use super::encrypted::encrypted_record_slots;
use super::encrypted::encrypted_record_total_bytes;
#[cfg(not(unix))]
use super::ensure::ensure_secure_parent_dir;
#[cfg(unix)]
use super::ensure::ensure_secure_parent_dir;
#[cfg(not(target_os = "macos"))]
use super::grant::grant_coreaudiod_shared_memory_access;
#[cfg(target_os = "macos")]
use super::grant::grant_coreaudiod_shared_memory_access;
use super::misc::fingerprint_to_u64;
use super::misc::get_shared_memory_path;
use super::misc::u64_to_fingerprint;
use super::types::EncryptedRecordHeader;
use super::types::EncryptedRecordRead;
use super::validate::{open_existing_shared_memory_file, open_shared_memory_file};
use memmap2::MmapMut;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{Ordering, fence};

/// Shared audio buffer for communication with Swift HAL driver
pub struct SharedAudioBuffer {
    pub(super) mmap: MmapMut,
    pub(super) path: PathBuf,
    pub(super) audio_offset: usize,
    /// Maximum audio capacity based on original mmap size (for validation)
    pub(super) max_audio_capacity: usize,
}

impl SharedAudioBuffer {
    pub(super) fn audio_layout(
        buffer_frames: u32,
        channel_count: u32,
    ) -> io::Result<(usize, usize, usize)> {
        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let audio_offset = (header_size + 63) & !63;
        let buffer_frames = buffer_frames as usize;
        let channel_count = channel_count as usize;

        if buffer_frames == 0 || channel_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Invalid shared memory configuration: buffer_frames={}, channel_count={}",
                    buffer_frames, channel_count
                ),
            ));
        }

        if channel_count > 128 || buffer_frames > 65536 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Shared memory configuration out of range: buffer_frames={}, channel_count={}",
                    buffer_frames, channel_count
                ),
            ));
        }

        let audio_capacity = buffer_frames
            .checked_mul(channel_count)
            .and_then(|v| v.checked_mul(8))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Shared memory audio capacity overflow",
                )
            })?;
        let total_size = audio_offset
            .checked_add(
                audio_capacity
                    .checked_mul(std::mem::size_of::<f32>())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Shared memory byte size overflow",
                        )
                    })?,
            )
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Shared memory total size overflow",
                )
            })?;

        Ok((audio_offset, audio_capacity, total_size))
    }

    pub(super) fn max_audio_capacity_from_len(audio_offset: usize, mmap_len: usize) -> usize {
        mmap_len.saturating_sub(audio_offset) / std::mem::size_of::<f32>()
    }

    pub(super) fn initialize_header(
        &mut self,
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
    ) {
        // The daemon rotates the session key before opening this mapping.
        // Keep key ownership out of the transport layer: rotating here would
        // desynchronize KeyManager's cached cipher/fingerprint. Header
        // initialization always disables encryption, so callers outside the
        // daemon must likewise rotate the key before enabling it.
        let header = self.header();
        header.magic.store(SHARED_MEMORY_MAGIC, Ordering::Release);
        header
            .version
            .store(SHARED_MEMORY_VERSION, Ordering::Release);
        header.sample_rate.store(sample_rate, Ordering::Release);
        header.buffer_frames.store(buffer_frames, Ordering::Release);
        header.channel_count.store(channel_count, Ordering::Release);
        header
            .requested_channel_count
            .store(channel_count, Ordering::Release);
        header.write_position.store(0, Ordering::Release);
        header.read_position.store(0, Ordering::Release);
        header.active.store(0, Ordering::Release);
        header.config_changed.store(0, Ordering::Release);
        header.driver_ready.store(0, Ordering::Release);
        header.engine_ready.store(0, Ordering::Release);
        header.encrypted.store(0, Ordering::Release);
        header.key_fingerprint.store(0, Ordering::Release);
        header.frame_counter.store(0, Ordering::Release);
        header.requested_sample_rate.store(0, Ordering::Release);
        header.requested_buffer_frames.store(0, Ordering::Release);
        header
            .actual_sample_rate
            .store(sample_rate, Ordering::Release);
        header
            .actual_buffer_frames
            .store(buffer_frames, Ordering::Release);
        header.config_status.store(0, Ordering::Release);
        header.config_source.store(0, Ordering::Release);
        header.config_error_code.store(0, Ordering::Release);
        header.encryption_overflow_count.store(0, Ordering::Release);
        header.daemon_heartbeat_ms.store(0, Ordering::Release);
        header.configuring.store(0, Ordering::Release);
        header.configuring_ack.store(0, Ordering::Release);
    }

    /// Create or open the shared memory file and initialize it when needed.
    pub fn create_or_open<P: AsRef<Path>>(
        path: P,
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
    ) -> io::Result<Self> {
        Self::create_or_open_with_capacity(
            path,
            sample_rate,
            buffer_frames,
            channel_count,
            channel_count,
        )
    }

    /// Create or open a shared memory file sized for `max_channel_count` while
    /// advertising `channel_count` as the current CoreAudio format.
    pub fn create_or_open_with_capacity<P: AsRef<Path>>(
        path: P,
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
        max_channel_count: u32,
    ) -> io::Result<Self> {
        Self::create_or_open_with_max_geometry(
            path,
            sample_rate,
            buffer_frames,
            channel_count,
            buffer_frames,
            max_channel_count,
        )
    }

    /// Create or open a shared memory file sized for the largest expected
    /// runtime geometry while advertising the current format in the header.
    pub fn create_or_open_with_max_geometry<P: AsRef<Path>>(
        path: P,
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
        max_buffer_frames: u32,
        max_channel_count: u32,
    ) -> io::Result<Self> {
        let path = path.as_ref();
        let (audio_offset, _, _) = Self::audio_layout(buffer_frames, channel_count)?;
        let max_channel_count = max_channel_count.max(channel_count);
        let max_buffer_frames = max_buffer_frames.max(buffer_frames);
        let (_, max_audio_capacity, total_size) =
            Self::audio_layout(max_buffer_frames, max_channel_count)?;

        if let Some(parent) = path.parent() {
            ensure_secure_parent_dir(parent)?;
        }

        let file = open_shared_memory_file(path)?;
        file.set_len(total_size as u64)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            grant_coreaudiod_shared_memory_access(path);
        }

        // SAFETY: We hold the only handle to the file we just sized; the
        // mapping is `MAP_SHARED` and the kernel guarantees the pages are
        // valid for the length we set. Cross-process truncation under us
        // would SIGBUS the next access; we mitigate that by the per-UID
        // path which enforces one daemon per user.
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        let mut buffer = Self {
            mmap,
            path: path.to_path_buf(),
            audio_offset,
            max_audio_capacity,
        };

        let needs_init = {
            let header = buffer.header();
            header.magic.load(Ordering::Acquire) != SHARED_MEMORY_MAGIC
                || header.version.load(Ordering::Acquire) != SHARED_MEMORY_VERSION
                || header.buffer_frames.load(Ordering::Acquire) != buffer_frames
                || header.channel_count.load(Ordering::Acquire) != channel_count
                || header.sample_rate.load(Ordering::Acquire) != sample_rate
        };

        if needs_init {
            buffer.initialize_header(sample_rate, buffer_frames, channel_count);
        }

        Ok(buffer)
    }

    /// Filesystem path backing this shared memory mapping.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create or open the default per-user shared memory file.
    pub fn create_or_open_default(
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
    ) -> io::Result<Self> {
        if channel_count == 0 || channel_count > MAX_HAL_CHANNEL_COUNT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "HAL channel_count {} is out of range 1..={}",
                    channel_count, MAX_HAL_CHANNEL_COUNT
                ),
            ));
        }

        Self::create_or_open_with_max_geometry(
            get_shared_memory_path(),
            sample_rate,
            buffer_frames,
            channel_count,
            MAX_HAL_BUFFER_FRAMES,
            MAX_HAL_CHANNEL_COUNT,
        )
    }

    /// Open an existing shared memory region created by the Swift HAL driver
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref();
        // Use the same no-follow and owner/type validation as the create
        // path. The HAL mapping is a cross-process trust boundary; opening an
        // attacker-controlled symlink here would otherwise bypass the
        // validation performed during creation.
        let file = open_existing_shared_memory_file(path)?;

        // SAFETY: see `create_or_open_with_max_geometry`. The file is sized
        // by the daemon; we map it read/write and validate the magic/version
        // immediately below.
        let mmap = unsafe { MmapMut::map_mut(&file)? };

        let header_size = std::mem::size_of::<SharedAudioHeader>();
        let audio_offset = (header_size + 63) & !63;

        if mmap.len() < header_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Shared memory too small for header: {} bytes, need {} bytes",
                    mmap.len(),
                    header_size
                ),
            ));
        }

        // SAFETY: `mmap.as_ptr()` is page-aligned (which always satisfies the
        // 8-byte alignment required by `repr(C, align(8))`), and we just
        // checked the mapped length covers the full header. All accesses
        // through the returned reference are atomic operations.
        let header = unsafe { &*(mmap.as_ptr() as *const SharedAudioHeader) };

        if header.magic.load(Ordering::Acquire) != SHARED_MEMORY_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid shared memory magic number",
            ));
        }

        let version = header.version.load(Ordering::Acquire);
        if version != SHARED_MEMORY_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Incompatible shared memory version: {} (expected {})",
                    version, SHARED_MEMORY_VERSION
                ),
            ));
        }

        let buffer_frames = header.buffer_frames.load(Ordering::Acquire) as usize;
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;

        if buffer_frames == 0 || channel_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid shared memory configuration: buffer_frames={}, channel_count={}",
                    buffer_frames, channel_count
                ),
            ));
        }

        if channel_count > 128 || buffer_frames > 65536 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Shared memory configuration out of range: buffer_frames={}, channel_count={}",
                    buffer_frames, channel_count
                ),
            ));
        }

        let audio_capacity = buffer_frames * channel_count * 8;

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

        let max_audio_capacity = Self::max_audio_capacity_from_len(audio_offset, mmap.len());

        Ok(Self {
            mmap,
            path: path.to_path_buf(),
            audio_offset,
            max_audio_capacity,
        })
    }

    /// Open the default shared memory path.
    pub fn open_default() -> io::Result<Self> {
        Self::open(get_shared_memory_path())
    }

    /// Get a reference to the header.
    ///
    /// The header is accessed entirely through its atomic fields, so a
    /// shared reference is always sufficient — including on write paths.
    /// The previous `header_mut()` method was eliminated because handing
    /// out `&mut SharedAudioHeader` while the Swift HAL plugin concurrently
    /// reads the same struct was a Rust memory-model data race (undefined
    /// behaviour).
    pub fn header(&self) -> &SharedAudioHeader {
        // SAFETY: We hold a valid mmap of at least
        // `size_of::<SharedAudioHeader>()` bytes (validated at construction).
        // The pointer is page-aligned which always satisfies the 8-byte
        // alignment required by `repr(C, align(8))`. The returned reference
        // only exposes atomic operations on the fields, so any concurrent
        // store (from this process or from the Swift side) is well-defined.
        unsafe { &*(self.mmap.as_ptr() as *const SharedAudioHeader) }
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

    /// Set engine ready flag.
    ///
    /// Ordering: when transitioning to `false` we clear `engine_ready` FIRST,
    /// then the heartbeat. `refresh_daemon_heartbeat` checks `engine_ready`
    /// before stamping, so once we've cleared it the heartbeat cannot be
    /// re-populated by a racing thread.
    pub fn set_engine_ready(&self, ready: bool) {
        let header = self.header();
        if ready {
            // Publish heartbeat before flipping the ready flag so the HAL
            // never observes `ready=1 && heartbeat=0`.
            header
                .daemon_heartbeat_ms
                .store(current_unix_millis(), Ordering::Release);
            header.engine_ready.store(1, Ordering::Release);
        } else {
            header.engine_ready.store(0, Ordering::Release);
            header.daemon_heartbeat_ms.store(0, Ordering::Release);
        }
    }

    /// Refresh the daemon liveness heartbeat read by the HAL driver.
    pub fn refresh_daemon_heartbeat(&self) {
        if self.header().engine_ready.load(Ordering::Acquire) != 0 {
            self.header()
                .daemon_heartbeat_ms
                .store(current_unix_millis(), Ordering::Release);
        }
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.header().sample_rate.load(Ordering::Acquire)
    }

    /// Set sample rate via the quiesced reconfiguration protocol.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.reconfigure_quiesced(Some(sample_rate), None, None);
    }

    /// Get buffer frame size
    pub fn buffer_frames(&self) -> u32 {
        self.header().buffer_frames.load(Ordering::Acquire)
    }

    /// Set buffer frame size via the quiesced reconfiguration protocol.
    pub fn set_buffer_frames(&mut self, buffer_frames: u32) {
        let channel_count = self.header().channel_count.load(Ordering::Acquire) as usize;
        let new_capacity = (buffer_frames as usize) * channel_count * 8;

        if new_capacity > self.max_audio_capacity {
            log::warn!(
                "Requested buffer_frames {} would exceed original allocation (max {}), ignoring",
                buffer_frames,
                self.max_audio_capacity / channel_count.max(1) / 8
            );
            return;
        }

        self.reconfigure_quiesced(None, Some(buffer_frames), None);
    }

    /// Get channel count
    pub fn channel_count(&self) -> u32 {
        self.header().channel_count.load(Ordering::Acquire)
    }

    /// Maximum channel count supported by this mapping at the current buffer size.
    pub fn max_channel_count(&self) -> u32 {
        let buffer_frames = self.header().buffer_frames.load(Ordering::Acquire) as usize;
        if buffer_frames == 0 {
            return 0;
        }
        (self.max_audio_capacity / buffer_frames / 8) as u32
    }

    /// Set channel count via the quiesced reconfiguration protocol.
    pub fn set_channel_count(&mut self, channel_count: u32) {
        if channel_count == 0 {
            log::warn!("Requested channel_count 0 is invalid, ignoring");
            return;
        }

        let buffer_frames = self.header().buffer_frames.load(Ordering::Acquire) as usize;
        let new_capacity = match buffer_frames
            .checked_mul(channel_count as usize)
            .and_then(|v| v.checked_mul(8))
        {
            Some(capacity) => capacity,
            None => {
                log::warn!(
                    "Requested channel_count {} overflowed capacity math",
                    channel_count
                );
                return;
            }
        };

        if new_capacity > self.max_audio_capacity {
            log::warn!(
                "Requested channel_count {} would exceed original allocation (max {}), ignoring",
                channel_count,
                self.max_audio_capacity / buffer_frames.max(1) / 8
            );
            return;
        }

        self.reconfigure_quiesced(None, None, Some(channel_count));
    }

    /// Reconfigure geometry under a quiesce handshake.
    ///
    /// This is the only path that mutates `sample_rate`, `buffer_frames`,
    /// `channel_count`, or `audio_capacity`. See the module-level docs for
    /// the full protocol.
    pub fn reconfigure_quiesced(
        &mut self,
        sample_rate: Option<u32>,
        buffer_frames: Option<u32>,
        channel_count: Option<u32>,
    ) {
        let current_frames = self.header().buffer_frames.load(Ordering::Acquire) as usize;
        let current_channels = self.header().channel_count.load(Ordering::Acquire) as usize;
        let next_frames = buffer_frames.map_or(current_frames, |frames| frames as usize);
        let next_channels = channel_count.map_or(current_channels, |channels| channels as usize);
        let Some(next_capacity) = next_frames
            .checked_mul(next_channels)
            .and_then(|value| value.checked_mul(8))
        else {
            log::warn!(
                "Rejecting shared-memory reconfiguration with overflowing geometry: {} frames, {} channels",
                next_frames,
                next_channels
            );
            return;
        };
        if next_frames == 0 || next_channels == 0 || next_capacity > self.max_audio_capacity {
            log::warn!(
                "Rejecting shared-memory reconfiguration outside the mapped capacity: {} frames, {} channels (max {} samples)",
                next_frames,
                next_channels,
                self.max_audio_capacity
            );
            return;
        }

        {
            let header = self.header();
            header.configuring.store(1, Ordering::Release);
            header.configuring_ack.store(0, Ordering::Release);
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_nanos(RECONFIG_QUIESCE_TIMEOUT_NS);
            while header.configuring_ack.load(Ordering::Acquire) == 0 && start.elapsed() < timeout {
                std::hint::spin_loop();
            }

            if header.configuring_ack.load(Ordering::Acquire) == 0 {
                log::debug!("Timed out waiting for shared-memory IO quiesce acknowledgment");
            }

            if let Some(rate) = sample_rate {
                header.sample_rate.store(rate, Ordering::Release);
                header.actual_sample_rate.store(rate, Ordering::Release);
            }
            if let Some(frames) = buffer_frames {
                header.buffer_frames.store(frames, Ordering::Release);
                header.actual_buffer_frames.store(frames, Ordering::Release);
            }
            if let Some(channels) = channel_count {
                header.channel_count.store(channels, Ordering::Release);
            }
        }

        // Validate the new derived capacity against the mmap bound. Every IO
        // operation derives its capacity from the atomic header instead of
        // caching geometry that another process can change.
        let header = self.header();
        header.write_position.store(0, Ordering::Release);
        header.read_position.store(0, Ordering::Release);
        header.config_changed.store(1, Ordering::Release);
        header.configuring.store(0, Ordering::Release);
        header.configuring_ack.store(0, Ordering::Release);
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
        u64_to_fingerprint(self.header().key_fingerprint.load(Ordering::Acquire))
    }

    /// Set the key fingerprint
    pub fn set_key_fingerprint(&self, fingerprint: [u8; 8]) {
        self.header()
            .key_fingerprint
            .store(fingerprint_to_u64(fingerprint), Ordering::Release);
    }

    /// Drop all queued audio while preserving the current write position.
    pub fn flush_audio(&self) {
        let write_pos = self.header().write_position.load(Ordering::Acquire);
        self.header()
            .read_position
            .store(write_pos, Ordering::Release);
    }

    /// Get the current frame counter (used as nonce base)
    pub fn frame_counter(&self) -> u64 {
        self.header().frame_counter.load(Ordering::Acquire)
    }

    /// Increment the frame counter and return the new value.
    ///
    /// # Thread Safety
    /// `fetch_add` is safe for concurrent use.
    ///
    /// # Nonce Safety
    /// The frame counter is used as a nonce for encryption. The daemon rotates
    /// the session key once, under its process-instance lock, before opening
    /// the mapping. Header initialization then resets this counter while
    /// encryption is disabled, so `(key, counter)` pairs cannot repeat across
    /// daemon lifetimes.
    pub fn increment_frame_counter(&self) -> u64 {
        self.header().frame_counter.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Number of encrypted writes dropped because the ring lacked space.
    pub fn encryption_overflow_count(&self) -> u64 {
        self.header()
            .encryption_overflow_count
            .load(Ordering::Acquire)
    }

    // =========================================================================
    // Configuration methods
    // =========================================================================

    /// Set the configuration changed flag.
    pub fn set_config_changed(&self) {
        self.header().config_changed.store(1, Ordering::Release);
    }

    /// Get the requested sample rate (set by the config change requester).
    pub fn requested_sample_rate(&self) -> u32 {
        self.header().requested_sample_rate.load(Ordering::Acquire)
    }

    /// Get the requested buffer frames (set by the config change requester).
    pub fn requested_buffer_frames(&self) -> u32 {
        self.header()
            .requested_buffer_frames
            .load(Ordering::Acquire)
    }

    /// Get the requested channel count without changing live ring geometry.
    pub fn requested_channel_count(&self) -> u32 {
        self.header()
            .requested_channel_count
            .load(Ordering::Acquire)
    }

    /// Set the actual sample rate (response from the handler).
    pub fn set_actual_sample_rate(&self, rate: u32) {
        self.header()
            .actual_sample_rate
            .store(rate, Ordering::Release);
    }

    /// Get the actual sample rate
    pub fn actual_sample_rate(&self) -> u32 {
        self.header().actual_sample_rate.load(Ordering::Acquire)
    }

    /// Set the actual buffer frames (response from the handler).
    pub fn set_actual_buffer_frames(&self, frames: u32) {
        self.header()
            .actual_buffer_frames
            .store(frames, Ordering::Release);
    }

    /// Get the actual buffer frames.
    pub fn actual_buffer_frames(&self) -> u32 {
        self.header().actual_buffer_frames.load(Ordering::Acquire)
    }

    /// Get the config status (0=pending, 1=accepted, 2=negotiated, 3=error).
    pub fn config_status(&self) -> u32 {
        self.header().config_status.load(Ordering::Acquire)
    }

    /// Set the config status.
    pub fn set_config_status(&self, status: u32) {
        self.header().config_status.store(status, Ordering::Release);
    }

    /// Get the config source (1=HAL initiated, 2=Daemon initiated).
    pub fn config_source(&self) -> u32 {
        self.header().config_source.load(Ordering::Acquire)
    }

    /// Set the config source.
    pub fn set_config_source(&self, source: u32) {
        self.header().config_source.store(source, Ordering::Release);
    }

    /// Get the config error code (only valid when config_status=3).
    pub fn config_error_code(&self) -> u32 {
        self.header().config_error_code.load(Ordering::Acquire)
    }

    /// Set the config error code.
    pub fn set_config_error_code(&self, code: u32) {
        self.header()
            .config_error_code
            .store(code, Ordering::Release);
    }

    /// Request a config change (called by the requester - HAL or Daemon).
    pub fn request_config_change(
        &mut self,
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
        source: u32,
    ) {
        let header = self.header();
        header
            .requested_sample_rate
            .store(sample_rate, Ordering::Release);
        header
            .requested_buffer_frames
            .store(buffer_frames, Ordering::Release);
        if channel_count > 0 {
            header
                .requested_channel_count
                .store(channel_count, Ordering::Release);
        }
        // Release fence so the requested fields are visible before we
        // publish the source/status/config_changed flags.
        fence(Ordering::Release);
        header.config_status.store(0, Ordering::Release);
        header.config_source.store(source, Ordering::Release);
        header.config_changed.store(1, Ordering::Release);
    }

    /// Acknowledge a config change (called by the handler after processing).
    pub fn acknowledge_config_change(
        &mut self,
        actual_rate: u32,
        actual_frames: u32,
        status: u32,
        error_code: u32,
    ) {
        let header = self.header();
        header
            .actual_sample_rate
            .store(actual_rate, Ordering::Release);
        header
            .actual_buffer_frames
            .store(actual_frames, Ordering::Release);
        header
            .config_error_code
            .store(error_code, Ordering::Release);
        header.config_status.store(status, Ordering::Release);
        header.config_changed.store(0, Ordering::Release);
    }

    /// Current ring capacity derived from the cross-process geometry.
    ///
    /// The Swift HAL may negotiate a new frame/channel geometry after this
    /// mapping was opened. Caching the initial capacity would make modulo and
    /// wrap calculations disagree across processes, so every IO operation
    /// snapshots the atomic fields and validates the result against the mmap.
    pub(super) fn current_audio_capacity(&self) -> usize {
        let header = self.header();
        let frames = header.buffer_frames.load(Ordering::Acquire) as usize;
        let channels = header.channel_count.load(Ordering::Acquire) as usize;
        frames
            .checked_mul(channels)
            .and_then(|value| value.checked_mul(8))
            .filter(|capacity| *capacity > 0 && *capacity <= self.max_audio_capacity)
            .unwrap_or(0)
    }

    /// Get pointer to audio data.
    pub(super) fn audio_data(&self) -> *const f32 {
        // SAFETY: `self.audio_offset` is validated to be within `mmap.len()`
        // at construction; the resulting pointer is valid for reads up to
        // `audio_capacity * size_of::<f32>()` bytes.
        unsafe { self.mmap.as_ptr().add(self.audio_offset) as *const f32 }
    }

    /// Get mutable pointer to audio data.
    pub(super) fn audio_data_mut(&mut self) -> *mut f32 {
        // SAFETY: same as `audio_data` but yields a `*mut`. We hold an
        // exclusive borrow on `self`, so no aliasing `*const` references to
        // the audio region exist in this process. The Swift side accesses
        // the ring through its own write_position/read_position discipline.
        unsafe { self.mmap.as_mut_ptr().add(self.audio_offset) as *mut f32 }
    }

    pub(super) fn copy_audio_slots_to(&self, position: u64, slot_count: usize, output: &mut [f32]) {
        if slot_count == 0 || output.len() < slot_count {
            return;
        }

        let audio_capacity = self.current_audio_capacity();
        if audio_capacity == 0 || slot_count > audio_capacity {
            return;
        }
        let read_index = (position as usize) % audio_capacity;
        let first_part = slot_count.min(audio_capacity - read_index);
        let second_part = slot_count - first_part;

        // SAFETY: `first_part + second_part == slot_count`, both halves stay
        // within `audio_capacity` (the wrap split is explicit), and
        // `output.len() >= slot_count` is asserted above. The audio region
        // and `output` cannot overlap (different allocations).
        unsafe {
            let audio_data = self.audio_data();
            std::ptr::copy_nonoverlapping(
                audio_data.add(read_index),
                output.as_mut_ptr(),
                first_part,
            );

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    audio_data,
                    output.as_mut_ptr().add(first_part),
                    second_part,
                );
            }
        }
    }

    pub(super) fn copy_audio_slots_from(&mut self, position: u64, input: &[f32]) {
        if input.is_empty() {
            return;
        }

        let audio_capacity = self.current_audio_capacity();
        if audio_capacity == 0 || input.len() > audio_capacity {
            return;
        }
        let write_index = (position as usize) % audio_capacity;
        let first_part = input.len().min(audio_capacity - write_index);
        let second_part = input.len() - first_part;

        // SAFETY: see `copy_audio_slots_to`. We hold `&mut self` so the
        // audio region is uniquely accessible from this process.
        unsafe {
            let audio_data = self.audio_data_mut();
            std::ptr::copy_nonoverlapping(input.as_ptr(), audio_data.add(write_index), first_part);

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    input.as_ptr().add(first_part),
                    audio_data,
                    second_part,
                );
            }
        }
    }

    /// Compute the repaired read position and available slot count.
    ///
    /// Pure inspection: does NOT mutate `read_position`. Callers that want
    /// to advance the reader must do so via
    /// [`commit_read_position`](Self::commit_read_position).
    pub(super) fn compute_repair(&self, write_pos: u64, read_pos: u64) -> (u64, usize) {
        let audio_capacity = self.current_audio_capacity();
        if audio_capacity == 0 {
            return (write_pos, 0);
        }

        if write_pos < read_pos {
            return (write_pos, 0);
        }

        let available = write_pos - read_pos;
        let capacity = audio_capacity as u64;
        if available > capacity {
            let adjusted_read_pos = write_pos - capacity;
            return (adjusted_read_pos, audio_capacity);
        }

        (read_pos, available as usize)
    }

    /// Commit a repaired read position back to the shared header.
    ///
    /// There is exactly one daemon reader and one HAL writer per ring, so
    /// atomic stores from this `&self` method are well-defined.
    pub(super) fn commit_read_position(&self, new_read_pos: u64) {
        self.header()
            .read_position
            .store(new_read_pos, Ordering::Release);
    }

    /// Read audio from the shared memory ring buffer.
    /// Returns the number of frames actually read.
    pub fn read_audio(&self, buffer: &mut [f32]) -> usize {
        let header = self.header();
        if header.configuring.load(Ordering::Acquire) != 0 {
            header.configuring_ack.store(1, Ordering::Release);
            buffer.fill(0.0);
            return 0;
        }
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;
        if channel_count == 0 {
            buffer.fill(0.0);
            return 0;
        }
        let sample_count = buffer.len();

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let (read_pos, available) = self.compute_repair(write_pos, read_pos);
        let to_read = sample_count
            .min(available)
            .checked_div(channel_count)
            .unwrap_or(0)
            * channel_count;

        if to_read == 0 {
            buffer.fill(0.0);
            return 0;
        }

        let audio_capacity = self.current_audio_capacity();
        if audio_capacity == 0 {
            buffer.fill(0.0);
            return 0;
        }
        let read_index = (read_pos as usize) % audio_capacity;
        let first_part = to_read.min(audio_capacity - read_index);
        let second_part = to_read - first_part;

        // SAFETY: identical reasoning to `copy_audio_slots_to`. Bounds are
        // enforced by the `to_read.min(...)` split above.
        unsafe {
            let audio_data = self.audio_data();

            std::ptr::copy_nonoverlapping(
                audio_data.add(read_index),
                buffer.as_mut_ptr(),
                first_part,
            );

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    audio_data,
                    buffer.as_mut_ptr().add(first_part),
                    second_part,
                );
            }
        }

        if to_read < sample_count {
            buffer[to_read..].fill(0.0);
        }

        let new_read_pos = read_pos + to_read as u64;
        self.commit_read_position(new_read_pos);

        let frames_read = to_read / channel_count.max(1);
        #[cfg(feature = "audio-trace")]
        if frames_read > 0 {
            log::trace!(
                "[SHM TRACE] Rust read: {} frames, wpos={}, rpos={}",
                frames_read,
                write_pos,
                new_read_pos
            );
        }
        #[cfg(not(feature = "audio-trace"))]
        {
            let _ = write_pos;
        }

        frames_read
    }

    /// Write audio to the shared memory ring buffer.
    /// Returns the number of frames actually written.
    pub fn write_audio(&mut self, buffer: &[f32]) -> usize {
        let header = self.header();
        if header.configuring.load(Ordering::Acquire) != 0 {
            header.configuring_ack.store(1, Ordering::Release);
            return 0;
        }
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;
        let sample_count = buffer.len();

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let (_, used) = self.compute_repair(write_pos, read_pos);
        let audio_capacity = self.current_audio_capacity();
        let available = audio_capacity.saturating_sub(used);
        // A committed ring transaction must end on an interleaved frame
        // boundary. In particular, a nearly-full ring may have a remainder of
        // free sample slots that is smaller than one complete frame.
        let to_write = sample_count
            .min(available)
            .checked_div(channel_count)
            .unwrap_or(0)
            * channel_count;

        if to_write == 0 {
            return 0;
        }

        let write_index = (write_pos as usize) % audio_capacity;
        let first_part = to_write.min(audio_capacity - write_index);
        let second_part = to_write - first_part;

        // SAFETY: identical reasoning to `copy_audio_slots_from`.
        unsafe {
            let audio_data = self.audio_data_mut();

            std::ptr::copy_nonoverlapping(buffer.as_ptr(), audio_data.add(write_index), first_part);

            if second_part > 0 {
                std::ptr::copy_nonoverlapping(
                    buffer.as_ptr().add(first_part),
                    audio_data,
                    second_part,
                );
            }
        }

        let new_write_pos = write_pos + to_write as u64;
        self.header()
            .write_position
            .store(new_write_pos, Ordering::Release);

        let frames_written = to_write / channel_count.max(1);
        #[cfg(feature = "audio-trace")]
        if frames_written > 0 {
            log::trace!(
                "[SHM TRACE] Rust write: {} frames, wpos={}, rpos={}",
                frames_written,
                new_write_pos,
                read_pos
            );
        }
        #[cfg(not(feature = "audio-trace"))]
        {
            let _ = read_pos;
        }

        frames_written
    }

    /// Get available frames to read.
    pub fn available_read_frames(&self) -> usize {
        let header = self.header();
        if header.configuring.load(Ordering::Acquire) != 0 {
            header.configuring_ack.store(1, Ordering::Release);
            return 0;
        }
        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;

        if channel_count == 0 {
            return 0;
        }

        if self.is_encrypted() {
            return self.available_encrypted_read_frames(write_pos, read_pos, channel_count);
        }

        let (_, available) = self.compute_repair(write_pos, read_pos);
        available / channel_count
    }

    pub(super) fn available_encrypted_read_frames(
        &self,
        write_pos: u64,
        read_pos: u64,
        channel_count: usize,
    ) -> usize {
        let (repaired_read_pos, mut available_slots) = self.compute_repair(write_pos, read_pos);
        if repaired_read_pos != read_pos {
            // Writer lapped us. Flush: jump the reader to the writer's
            // position. A partial recovery would leave us mid-record with
            // no way to find the next valid header so we'd just flush on
            // the next read anyway.
            self.commit_read_position(write_pos);
            return 0;
        }
        let mut read_pos = repaired_read_pos;

        let mut available_frames = 0;
        let mut header_slots = [0.0f32; ENCRYPTED_RECORD_HEADER_SLOTS];
        let mut header_bytes = [0u8; ENCRYPTED_RECORD_HEADER_BYTES];

        while available_slots >= ENCRYPTED_RECORD_HEADER_SLOTS {
            self.copy_audio_slots_to(read_pos, ENCRYPTED_RECORD_HEADER_SLOTS, &mut header_slots);
            crate::encryption::samples_to_encrypted_into(&header_slots, &mut header_bytes);

            let Some(record) =
                parse_encrypted_record_header(&header_bytes, self.current_audio_capacity())
            else {
                log::warn!("Invalid encrypted audio record header; flushing encrypted ring");
                self.commit_read_position(write_pos);
                return 0;
            };

            if record.slot_count > available_slots {
                break;
            }

            available_frames += record.sample_count / channel_count;
            read_pos += record.slot_count as u64;
            available_slots -= record.slot_count;
        }

        available_frames
    }

    /// Get available space to write (in frames).
    pub fn available_write_frames(&self) -> usize {
        let header = self.header();
        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;

        if channel_count == 0 {
            return 0;
        }

        let (_, used) = self.compute_repair(write_pos, read_pos);
        self.current_audio_capacity().saturating_sub(used) / channel_count
    }

    // =========================================================================
    // Encrypted audio I/O
    // =========================================================================

    /// Write audio with encryption (allocating, convenience wrapper).
    pub fn write_audio_encrypted(
        &mut self,
        buffer: &[f32],
        cipher: &crate::encryption::AudioCipher,
    ) -> usize {
        let max_samples = MAX_HAL_BUFFER_FRAMES as usize * MAX_HAL_CHANNEL_COUNT as usize;
        let mut ciphertext_buf = Vec::with_capacity(
            encrypted_record_total_bytes(max_samples).unwrap_or(max_samples.saturating_mul(8)),
        );
        let mut encrypted_buf = Vec::with_capacity(
            encrypted_record_slots(max_samples).unwrap_or(max_samples.saturating_mul(2)),
        );
        self.write_audio_encrypted_into(buffer, cipher, &mut ciphertext_buf, &mut encrypted_buf)
    }

    /// Read audio with decryption (allocating, convenience wrapper).
    pub fn read_audio_encrypted(
        &self,
        buffer: &mut [f32],
        cipher: &crate::encryption::AudioCipher,
    ) -> usize {
        let max_samples = MAX_HAL_BUFFER_FRAMES as usize * MAX_HAL_CHANNEL_COUNT as usize;
        let mut encrypted_buf = Vec::with_capacity(
            encrypted_record_slots(max_samples).unwrap_or(max_samples.saturating_mul(2)),
        );
        let mut ciphertext_buf = Vec::with_capacity(
            encrypted_record_total_bytes(max_samples).unwrap_or(max_samples.saturating_mul(8)),
        );
        self.read_audio_encrypted_into(buffer, cipher, &mut encrypted_buf, &mut ciphertext_buf)
    }

    pub(super) fn peek_encrypted_record_header(
        &self,
        read_pos: u64,
        available_slots: usize,
        encrypted_buf: &mut Vec<f32>,
        ciphertext_buf: &mut Vec<u8>,
    ) -> Option<Option<EncryptedRecordHeader>> {
        if available_slots < ENCRYPTED_RECORD_HEADER_SLOTS {
            return Some(None);
        }

        if encrypted_buf.capacity() < ENCRYPTED_RECORD_HEADER_SLOTS
            || ciphertext_buf.capacity() < ENCRYPTED_RECORD_HEADER_BYTES
        {
            return None;
        }
        if encrypted_buf.len() < ENCRYPTED_RECORD_HEADER_SLOTS {
            encrypted_buf.resize(ENCRYPTED_RECORD_HEADER_SLOTS, 0.0);
        }
        if ciphertext_buf.len() < ENCRYPTED_RECORD_HEADER_BYTES {
            ciphertext_buf.resize(ENCRYPTED_RECORD_HEADER_BYTES, 0);
        }

        self.copy_audio_slots_to(
            read_pos,
            ENCRYPTED_RECORD_HEADER_SLOTS,
            &mut encrypted_buf[..ENCRYPTED_RECORD_HEADER_SLOTS],
        );
        crate::encryption::samples_to_encrypted_into(
            &encrypted_buf[..ENCRYPTED_RECORD_HEADER_SLOTS],
            &mut ciphertext_buf[..ENCRYPTED_RECORD_HEADER_BYTES],
        );

        parse_encrypted_record_header(
            &ciphertext_buf[..ENCRYPTED_RECORD_HEADER_BYTES],
            self.current_audio_capacity(),
        )
        .map(Some)
    }

    pub(super) fn read_next_encrypted_record_into(
        &self,
        output: &mut [f32],
        cipher: &crate::encryption::AudioCipher,
        encrypted_buf: &mut Vec<f32>,
        ciphertext_buf: &mut Vec<u8>,
    ) -> EncryptedRecordRead {
        let header = self.header();
        let write_pos = header.write_position.load(Ordering::Acquire);
        let original_read_pos = header.read_position.load(Ordering::Acquire);
        let (read_pos, available_slots) = self.compute_repair(write_pos, original_read_pos);
        if read_pos != original_read_pos {
            self.commit_read_position(write_pos);
            return EncryptedRecordRead::Empty;
        }

        let Some(record) = self.peek_encrypted_record_header(
            read_pos,
            available_slots,
            encrypted_buf,
            ciphertext_buf,
        ) else {
            log::warn!("Invalid encrypted audio record header; flushing encrypted ring");
            self.commit_read_position(write_pos);
            return EncryptedRecordRead::InvalidHeader;
        };

        let Some(record) = record else {
            return EncryptedRecordRead::Empty;
        };

        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;
        if channel_count == 0 || record.sample_count % channel_count != 0 {
            log::warn!(
                "Invalid encrypted record frame alignment: {} samples for {} channels",
                record.sample_count,
                channel_count
            );
            self.commit_read_position(write_pos);
            return EncryptedRecordRead::InvalidHeader;
        }

        if record.slot_count > available_slots {
            return EncryptedRecordRead::Empty;
        }
        if record.sample_count > output.len() {
            return EncryptedRecordRead::OutputTooSmall {
                sample_count: record.sample_count,
            };
        }

        if encrypted_buf.capacity() < record.slot_count
            || ciphertext_buf.capacity() < record.total_bytes
        {
            self.commit_read_position(write_pos);
            return EncryptedRecordRead::InvalidHeader;
        }
        if encrypted_buf.len() < record.slot_count {
            encrypted_buf.resize(record.slot_count, 0.0);
        }
        if ciphertext_buf.len() < record.total_bytes {
            ciphertext_buf.resize(record.total_bytes, 0);
        }

        self.copy_audio_slots_to(
            read_pos,
            record.slot_count,
            &mut encrypted_buf[..record.slot_count],
        );
        crate::encryption::samples_to_encrypted_into(
            &encrypted_buf[..record.slot_count],
            &mut ciphertext_buf[..record.slot_count * 4],
        );

        let ciphertext_start = ENCRYPTED_RECORD_HEADER_BYTES;
        let ciphertext_end = ciphertext_start + record.ciphertext_len;
        let ciphertext = &ciphertext_buf[ciphertext_start..ciphertext_end];

        let status = match cipher.decrypt_into(
            ciphertext,
            record.frame_counter,
            &mut output[..record.sample_count],
        ) {
            Some(decrypted_count) if decrypted_count == record.sample_count => {
                EncryptedRecordRead::Read {
                    sample_count: decrypted_count,
                }
            }
            _ => {
                log::warn!(
                    "Audio decryption failed; dropping encrypted record frame_counter={}",
                    record.frame_counter
                );
                EncryptedRecordRead::Corrupt {
                    frame_counter: record.frame_counter,
                }
            }
        };

        self.commit_read_position(read_pos + record.slot_count as u64);
        status
    }

    /// Read audio with decryption using pre-allocated buffers (allocation-free hot path).
    pub fn read_audio_encrypted_into(
        &self,
        buffer: &mut [f32],
        cipher: &crate::encryption::AudioCipher,
        encrypted_buf: &mut Vec<f32>,
        ciphertext_buf: &mut Vec<u8>,
    ) -> usize {
        if self.header().configuring.load(Ordering::Acquire) != 0 {
            self.header().configuring_ack.store(1, Ordering::Release);
            buffer.fill(0.0);
            return 0;
        }
        if !self.is_encrypted() {
            return self.read_audio(buffer);
        }

        let channel_count = self.header().channel_count.load(Ordering::Acquire) as usize;
        if channel_count == 0 {
            buffer.fill(0.0);
            return 0;
        }

        let target_samples = buffer.len() / channel_count * channel_count;
        if target_samples < buffer.len() {
            buffer[target_samples..].fill(0.0);
        }
        let mut copied_samples = 0;

        while copied_samples < target_samples {
            match self.read_next_encrypted_record_into(
                &mut buffer[copied_samples..],
                cipher,
                encrypted_buf,
                ciphertext_buf,
            ) {
                EncryptedRecordRead::Read { sample_count } => {
                    copied_samples += sample_count;
                }
                EncryptedRecordRead::Corrupt { .. } | EncryptedRecordRead::InvalidHeader => {
                    if copied_samples == 0 {
                        buffer.fill(0.0);
                        return 0;
                    }
                    break;
                }
                EncryptedRecordRead::Empty | EncryptedRecordRead::OutputTooSmall { .. } => {
                    break;
                }
            }
        }

        if copied_samples < target_samples {
            buffer[copied_samples..].fill(0.0);
        }

        copied_samples / channel_count
    }

    /// Write audio with encryption using pre-allocated buffers (allocation-free hot path).
    pub fn write_audio_encrypted_into(
        &mut self,
        samples: &[f32],
        cipher: &crate::encryption::AudioCipher,
        ciphertext_buf: &mut Vec<u8>,
        encrypted_buf: &mut Vec<f32>,
    ) -> usize {
        if self.header().configuring.load(Ordering::Acquire) != 0 {
            self.header().configuring_ack.store(1, Ordering::Release);
            return 0;
        }
        if !self.is_encrypted() {
            return self.write_audio(samples);
        }

        let header = self.header();
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;
        let sample_count = samples.len() / channel_count * channel_count;

        let max_samples = MAX_HAL_BUFFER_FRAMES as usize * MAX_HAL_CHANNEL_COUNT as usize;
        if channel_count == 0 || sample_count == 0 || sample_count > max_samples {
            return 0;
        }
        let samples = &samples[..sample_count];

        let Some(ciphertext_size) = crate::encryption::encrypted_byte_size_checked(sample_count)
        else {
            log::error!("Encrypted audio ciphertext size overflow");
            return 0;
        };
        let Some(total_bytes) = encrypted_record_total_bytes(sample_count) else {
            log::error!("Encrypted audio record size overflow");
            return 0;
        };
        let Some(encrypted_slots) = encrypted_record_slots(sample_count) else {
            log::error!("Encrypted audio slot count overflow");
            return 0;
        };

        if ciphertext_buf.capacity() < total_bytes || encrypted_buf.capacity() < encrypted_slots {
            header
                .encryption_overflow_count
                .fetch_add(1, Ordering::AcqRel);
            return 0;
        }
        if ciphertext_buf.len() < total_bytes {
            ciphertext_buf.resize(total_bytes, 0);
        }
        if encrypted_buf.len() < encrypted_slots {
            encrypted_buf.resize(encrypted_slots, 0.0);
        }

        let frame_counter = self.increment_frame_counter();
        if !write_encrypted_record_header(
            &mut ciphertext_buf[..ENCRYPTED_RECORD_HEADER_BYTES],
            sample_count,
            frame_counter,
            ciphertext_size,
        ) {
            log::error!("Encrypted audio record header overflow");
            return 0;
        }

        match cipher.encrypt_into(
            samples,
            frame_counter,
            &mut ciphertext_buf
                [ENCRYPTED_RECORD_HEADER_BYTES..ENCRYPTED_RECORD_HEADER_BYTES + ciphertext_size],
        ) {
            Some(_) => {}
            None => {
                log::error!("Encryption failed - buffer too small");
                return 0;
            }
        }

        crate::encryption::encrypted_to_samples_into(&ciphertext_buf[..total_bytes], encrypted_buf);

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let (_, used) = self.compute_repair(write_pos, read_pos);
        let available = self.current_audio_capacity().saturating_sub(used);

        if encrypted_slots > available {
            header
                .encryption_overflow_count
                .fetch_add(1, Ordering::AcqRel);
            log::warn!(
                "Encrypted audio overflow: {} slots needed, {} available, frame_counter={}",
                encrypted_slots,
                available,
                frame_counter
            );
            return 0;
        }

        self.copy_audio_slots_from(write_pos, &encrypted_buf[..encrypted_slots]);

        self.header()
            .write_position
            .store(write_pos + encrypted_slots as u64, Ordering::Release);

        sample_count / channel_count
    }
}

/// Best-effort cipher load on the non-RT init path.
pub(super) fn load_initial_cipher(
    buffer: &SharedAudioBuffer,
) -> Option<crate::encryption::AudioCipher> {
    if !buffer.is_encrypted() {
        return None;
    }
    match crate::encryption::load_session_key() {
        Ok(key) => {
            let cipher = crate::encryption::AudioCipher::new(&key);
            if super::misc::fingerprints_equal(cipher.fingerprint(), &buffer.key_fingerprint()) {
                log::info!("[HAL] Loaded session key, fingerprint matches header");
                Some(cipher)
            } else {
                log::warn!("[HAL] Session key fingerprint mismatch at init; will require reload");
                None
            }
        }
        Err(e) => {
            log::warn!("[HAL] Failed to load session key at init: {}", e);
            None
        }
    }
}
