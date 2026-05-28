//! Shared memory interface for communication with Swift HAL driver
//!
//! This module provides a Rust interface to the shared memory region
//! created by the Swift HAL driver for audio data exchange.
//!
//! # Cross-process memory model
//!
//! Every field in [`SharedAudioHeader`] that may be touched by both the daemon
//! (this crate) and the Swift HAL plugin running inside `coreaudiod` is an
//! atomic type (`AtomicU32`/`AtomicU64`). Plain non-atomic stores from one
//! process while the other process is reading the same word would be a data
//! race in the Rust/C++ abstract machines (undefined behaviour), so we publish
//! every cross-process value through `store(_, Ordering::Release)` and consume
//! it via `load(Ordering::Acquire)`. The Swift side performs equivalent
//! atomic accesses via `std::atomic`.
//!
//! The `key_fingerprint` is exposed externally as an `[u8; 8]` but stored as
//! an `AtomicU64` in big-endian byte order so that the 8 bytes are published
//! in a single atomic store.
//!
//! # Reconfiguration protocol
//!
//! Geometry changes go through [`SharedAudioBuffer::reconfigure_quiesced`]
//! which uses a `configuring` handshake flag:
//!
//! 1. Daemon stores `configuring = 1` (Release).
//! 2. Daemon spins briefly so the writer can drain its IO cycle.
//! 3. Daemon publishes new geometry and resets ring positions.
//! 4. Daemon sets `config_changed = 1` so the writer reloads geometry.
//! 5. Daemon clears `configuring = 0`.
//!
//! The legacy `set_sample_rate` / `set_buffer_frames` / `set_channel_count`
//! setters route through this path so they no longer race the HAL writer.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering, fence};

use memmap2::MmapMut;

/// Magic number for shared memory header validation: 'SOTF'
const SHARED_MEMORY_MAGIC: u32 = 0x534F5446;

/// Current protocol version
/// Version 2: Added encryption fields (encrypted, key_fingerprint, frame_counter)
/// Version 3: Added config negotiation fields for bidirectional HAL-Daemon sync
/// Version 4: Added daemon heartbeat for stale-engine detection in the HAL driver
/// Version 5: Promoted all cross-process geometry/config fields to atomics and
///            added the `configuring` quiesce handshake flag.
const SHARED_MEMORY_VERSION: u32 = 5;
pub const DEFAULT_HAL_CHANNEL_COUNT: u32 = 2;
pub const MAX_HAL_CHANNEL_COUNT: u32 = 32;
pub const MAX_HAL_BUFFER_FRAMES: u32 = 4096;

/// Bound on the spin period while waiting for the writer to observe
/// `configuring=1`. Reconfig is rare and user-driven, so a small spin
/// is acceptable.
const RECONFIG_QUIESCE_TIMEOUT_NS: u64 = 5_000_000; // 5 ms

/// Encrypted audio record magic: 'SEA1' (SotF Encrypted Audio v1)
const ENCRYPTED_RECORD_MAGIC: u32 = 0x5345_4131;
const ENCRYPTED_RECORD_HEADER_BYTES: usize = 24;
const ENCRYPTED_RECORD_HEADER_SLOTS: usize = ENCRYPTED_RECORD_HEADER_BYTES / 4;

/// Upper bound on encrypted record sample counts. Defends against bogus
/// header values that would cause integer overflow before
/// `audio_capacity` would otherwise catch them.
const MAX_ENCRYPTED_SAMPLE_COUNT: usize =
    MAX_HAL_BUFFER_FRAMES as usize * MAX_HAL_CHANNEL_COUNT as usize;

fn current_unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy)]
struct EncryptedRecordHeader {
    sample_count: usize,
    frame_counter: u64,
    ciphertext_len: usize,
    total_bytes: usize,
    slot_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncryptedRecordRead {
    Empty,
    InvalidHeader,
    OutputTooSmall { sample_count: usize },
    Corrupt { frame_counter: u64 },
    Read { sample_count: usize },
}

fn encrypted_record_total_bytes(sample_count: usize) -> Option<usize> {
    crate::encryption::encrypted_byte_size_checked(sample_count)?
        .checked_add(ENCRYPTED_RECORD_HEADER_BYTES)
}

fn encrypted_record_slots(sample_count: usize) -> Option<usize> {
    encrypted_record_total_bytes(sample_count).map(|bytes| bytes.div_ceil(4))
}

fn write_encrypted_record_header(
    output: &mut [u8],
    sample_count: usize,
    frame_counter: u64,
    ciphertext_len: usize,
) -> bool {
    if output.len() < ENCRYPTED_RECORD_HEADER_BYTES
        || sample_count > u32::MAX as usize
        || ciphertext_len > u32::MAX as usize
    {
        return false;
    }

    output[0..4].copy_from_slice(&ENCRYPTED_RECORD_MAGIC.to_be_bytes());
    output[4..8].copy_from_slice(&(sample_count as u32).to_be_bytes());
    output[8..16].copy_from_slice(&frame_counter.to_be_bytes());
    output[16..20].copy_from_slice(&(ciphertext_len as u32).to_be_bytes());
    output[20..24].copy_from_slice(&0u32.to_be_bytes());
    true
}

fn parse_encrypted_record_header(
    bytes: &[u8],
    audio_capacity: usize,
) -> Option<EncryptedRecordHeader> {
    if bytes.len() < ENCRYPTED_RECORD_HEADER_BYTES {
        return None;
    }

    let magic = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
    if magic != ENCRYPTED_RECORD_MAGIC {
        return None;
    }

    let sample_count = u32::from_be_bytes(bytes[4..8].try_into().ok()?) as usize;
    let frame_counter = u64::from_be_bytes(bytes[8..16].try_into().ok()?);
    let ciphertext_len = u32::from_be_bytes(bytes[16..20].try_into().ok()?) as usize;
    let reserved = u32::from_be_bytes(bytes[20..24].try_into().ok()?);

    if sample_count == 0 || sample_count > MAX_ENCRYPTED_SAMPLE_COUNT || reserved != 0 {
        return None;
    }

    let expected_ciphertext_len = crate::encryption::encrypted_byte_size_checked(sample_count)?;
    if ciphertext_len != expected_ciphertext_len {
        return None;
    }

    let total_bytes = ENCRYPTED_RECORD_HEADER_BYTES.checked_add(ciphertext_len)?;
    let slot_count = total_bytes.div_ceil(4);
    if slot_count == 0 || slot_count > audio_capacity {
        return None;
    }

    Some(EncryptedRecordHeader {
        sample_count,
        frame_counter,
        ciphertext_len,
        total_bytes,
        slot_count,
    })
}

/// Get the shared memory path for the current user
///
/// Security model: each user has their own shared memory region.
/// Path is based on the user's UID to match the Swift HAL driver's path.
///
/// IMPORTANT: This must match the Swift side in SharedMemory.swift which uses:
/// `/tmp/sotf-{uid}/audio.shm`
pub fn get_shared_memory_path() -> std::path::PathBuf {
    // SAFETY: `libc::getuid` is async-signal-safe, has no preconditions, and
    // cannot fail. It only reads the calling process's UID.
    let uid = unsafe { libc::getuid() };
    shared_memory_path_from_env(
        std::env::var_os("SOTF_HAL_SHARED_MEMORY_PATH"),
        std::env::var_os("SOTF_SYSTEMWIDE_RUNTIME_DIR"),
        uid,
    )
}

fn non_empty_path(value: Option<OsString>) -> Option<std::path::PathBuf> {
    value
        .map(std::path::PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn shared_memory_path_from_env(
    path_override: Option<OsString>,
    runtime_dir: Option<OsString>,
    uid: u32,
) -> std::path::PathBuf {
    if let Some(path) = non_empty_path(path_override) {
        return path;
    }

    if let Some(path) = non_empty_path(runtime_dir) {
        return path.join("audio.shm");
    }

    std::path::PathBuf::from(format!("/tmp/sotf-{}/audio.shm", uid))
}

#[cfg(unix)]
fn current_uid() -> u32 {
    // SAFETY: `libc::getuid` has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

#[cfg(unix)]
fn protected_shared_memory_parent() -> std::path::PathBuf {
    non_empty_path(std::env::var_os("SOTF_SYSTEMWIDE_RUNTIME_DIR"))
        .unwrap_or_else(|| std::path::PathBuf::from(format!("/tmp/sotf-{}", current_uid())))
}

#[cfg(unix)]
fn is_protected_shared_memory_parent(path: &Path) -> bool {
    path == protected_shared_memory_parent()
}

#[cfg(unix)]
fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }

    if is_protected_shared_memory_parent(parent) {
        let metadata = std::fs::symlink_metadata(parent)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a directory", parent.display()),
            ));
        }
        if metadata.uid() != current_uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{} is owned by uid {}, expected {}",
                    parent.display(),
                    metadata.uid(),
                    current_uid()
                ),
            ));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        grant_coreaudiod_shared_memory_access(parent);
    }

    Ok(())
}

#[cfg(not(unix))]
fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn open_shared_memory_file(path: &Path) -> io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
        options.mode(0o600);
    }

    let file = options.open(path)?;
    validate_shared_memory_file(&file, path)?;
    Ok(file)
}

#[cfg(unix)]
fn validate_shared_memory_file(file: &std::fs::File, path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a regular file", path.display()),
        ));
    }
    if metadata.uid() != current_uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} is owned by uid {}, expected {}",
                path.display(),
                metadata.uid(),
                current_uid()
            ),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_shared_memory_file(_file: &std::fs::File, _path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn grant_coreaudiod_shared_memory_access(path: &Path) {
    let acl = if path.is_dir() {
        "_coreaudiod allow search,readattr"
    } else {
        "_coreaudiod allow read,write,readattr,writeattr"
    };

    match std::process::Command::new("/bin/chmod")
        .arg("+a")
        .arg(acl)
        .arg(path)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => {
            log::warn!(
                "Failed to grant _coreaudiod shared-memory access on {}: chmod exited with {}",
                path.display(),
                status
            );
        }
        Err(e) => {
            log::warn!(
                "Failed to grant _coreaudiod shared-memory access on {}: {}",
                path.display(),
                e
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn grant_coreaudiod_shared_memory_access(_path: &Path) {}

/// Header structure for shared memory region.
///
/// Must match the Swift side exactly. All cross-process fields are atomic
/// (`AtomicU32`/`AtomicU64`). `AtomicU32` and `AtomicU64` have the same
/// memory layout and alignment as plain `u32`/`u64` (guaranteed by the
/// standard library), so the byte layout remains compatible with the
/// Swift `struct SharedAudioHeader` mirror.
#[repr(C, align(8))]
pub struct SharedAudioHeader {
    /// Magic number for validation (0x534F5446 = 'SOTF')
    pub magic: AtomicU32,
    /// Protocol version
    pub version: AtomicU32,
    /// Current sample rate in Hz
    pub sample_rate: AtomicU32,
    /// Frames per buffer
    pub buffer_frames: AtomicU32,
    /// Number of audio channels
    pub channel_count: AtomicU32,

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
    /// First 8 bytes of SHA256 hash of the encryption key, stored in
    /// big-endian byte order (so `to_be_bytes` gives the canonical 8-byte
    /// fingerprint).
    pub key_fingerprint: AtomicU64,
    /// Frame counter for nonce generation (monotonically increasing, never reuse!)
    pub frame_counter: AtomicU64,

    // Config negotiation fields (version 3+)
    /// Requested sample rate (set by requester, either HAL or Daemon)
    pub requested_sample_rate: AtomicU32,
    /// Requested buffer frames (set by requester)
    pub requested_buffer_frames: AtomicU32,
    /// Actual sample rate in use (set by responder after negotiation)
    pub actual_sample_rate: AtomicU32,
    /// Actual buffer frames in use (set by responder after negotiation)
    pub actual_buffer_frames: AtomicU32,
    /// Config status: 0=pending, 1=accepted, 2=negotiated, 3=error
    pub config_status: AtomicU32,
    /// Config source: 1=HAL initiated, 2=Daemon initiated
    pub config_source: AtomicU32,
    /// Error code if config_status=3
    pub config_error_code: AtomicU32,

    // Statistics
    /// Number of times encrypted write failed due to insufficient buffer space
    pub encryption_overflow_count: AtomicU64,
    /// Daemon liveness heartbeat in Unix epoch milliseconds.
    pub daemon_heartbeat_ms: AtomicU64,

    // Reconfiguration handshake (version 5+)
    /// 1 while the daemon is performing a quiesced reconfiguration. The
    /// Swift HAL plugin must drop any pending write and refrain from
    /// publishing new `write_position` values while this is set.
    pub configuring: AtomicU32,
}

// Compile-time guarantee that the header stays within the size budget the
// Swift side mirror has reserved.
const _: () = assert!(std::mem::size_of::<SharedAudioHeader>() <= 256);
const _: () = assert!(std::mem::align_of::<SharedAudioHeader>() == 8);

fn fingerprint_to_u64(fingerprint: [u8; 8]) -> u64 {
    u64::from_be_bytes(fingerprint)
}

fn u64_to_fingerprint(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

/// Shared audio buffer for communication with Swift HAL driver
pub struct SharedAudioBuffer {
    mmap: MmapMut,
    path: PathBuf,
    audio_offset: usize,
    audio_capacity: usize,
    /// Maximum audio capacity based on original mmap size (for validation)
    max_audio_capacity: usize,
}

impl SharedAudioBuffer {
    fn audio_layout(buffer_frames: u32, channel_count: u32) -> io::Result<(usize, usize, usize)> {
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

    fn max_audio_capacity_from_len(audio_offset: usize, mmap_len: usize) -> usize {
        mmap_len.saturating_sub(audio_offset) / std::mem::size_of::<f32>()
    }

    fn initialize_header(&mut self, sample_rate: u32, buffer_frames: u32, channel_count: u32) {
        // Rotate the session key before publishing any state. Resetting
        // `frame_counter` to 0 with the previous key on disk would create
        // a catastrophic AEAD nonce-reuse window (same key + same counter
        // as a prior session). Rotation is best-effort: if disk I/O fails
        // we still initialise the header, but the daemon will fail to
        // authenticate against the stale key and force re-pairing.
        if let Err(e) = crate::encryption::rotate_session_key() {
            log::warn!(
                "Failed to rotate session key during header initialization: {} \
                 (continuing with previous key — encrypted frames may fail to authenticate)",
                e
            );
        }

        let header = self.header();
        header.magic.store(SHARED_MEMORY_MAGIC, Ordering::Release);
        header
            .version
            .store(SHARED_MEMORY_VERSION, Ordering::Release);
        header.sample_rate.store(sample_rate, Ordering::Release);
        header.buffer_frames.store(buffer_frames, Ordering::Release);
        header.channel_count.store(channel_count, Ordering::Release);
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
        let (audio_offset, audio_capacity, _) = Self::audio_layout(buffer_frames, channel_count)?;
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
            audio_capacity,
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
        let file = OpenOptions::new().read(true).write(true).open(path)?;

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
            audio_capacity,
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
        {
            let header = self.header();
            header.configuring.store(1, Ordering::Release);
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_nanos(RECONFIG_QUIESCE_TIMEOUT_NS);
            while start.elapsed() < timeout {
                std::hint::spin_loop();
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

        // Recompute audio_capacity from the new geometry. Re-borrow the
        // header in a fresh scope so `self.audio_capacity` is not blocked
        // by an outstanding immutable borrow.
        let frames = self.header().buffer_frames.load(Ordering::Acquire) as usize;
        let channels = self.header().channel_count.load(Ordering::Acquire) as usize;
        if let Some(cap) = frames.checked_mul(channels).and_then(|v| v.checked_mul(8))
            && cap <= self.max_audio_capacity
        {
            self.audio_capacity = cap;
        }

        let header = self.header();
        header.write_position.store(0, Ordering::Release);
        header.read_position.store(0, Ordering::Release);
        header.config_changed.store(1, Ordering::Release);
        header.configuring.store(0, Ordering::Release);
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
    /// The frame counter is used as a nonce for encryption. The session key
    /// is rotated in `initialize_header` every time the daemon (re)opens
    /// with a new geometry, so the `(key, counter)` pair never repeats
    /// across runs.
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
            header.channel_count.store(channel_count, Ordering::Release);
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

    /// Get pointer to audio data.
    fn audio_data(&self) -> *const f32 {
        // SAFETY: `self.audio_offset` is validated to be within `mmap.len()`
        // at construction; the resulting pointer is valid for reads up to
        // `audio_capacity * size_of::<f32>()` bytes.
        unsafe { self.mmap.as_ptr().add(self.audio_offset) as *const f32 }
    }

    /// Get mutable pointer to audio data.
    fn audio_data_mut(&mut self) -> *mut f32 {
        // SAFETY: same as `audio_data` but yields a `*mut`. We hold an
        // exclusive borrow on `self`, so no aliasing `*const` references to
        // the audio region exist in this process. The Swift side accesses
        // the ring through its own write_position/read_position discipline.
        unsafe { self.mmap.as_mut_ptr().add(self.audio_offset) as *mut f32 }
    }

    fn copy_audio_slots_to(&self, position: u64, slot_count: usize, output: &mut [f32]) {
        debug_assert!(output.len() >= slot_count);
        if slot_count == 0 {
            return;
        }

        let read_index = (position as usize) % self.audio_capacity;
        let first_part = slot_count.min(self.audio_capacity - read_index);
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

    fn copy_audio_slots_from(&mut self, position: u64, input: &[f32]) {
        if input.is_empty() {
            return;
        }

        let write_index = (position as usize) % self.audio_capacity;
        let first_part = input.len().min(self.audio_capacity - write_index);
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
    fn compute_repair(&self, write_pos: u64, read_pos: u64) -> (u64, usize) {
        if self.audio_capacity == 0 {
            return (write_pos, 0);
        }

        if write_pos < read_pos {
            return (write_pos, 0);
        }

        let available = write_pos - read_pos;
        let capacity = self.audio_capacity as u64;
        if available > capacity {
            let adjusted_read_pos = write_pos - capacity;
            return (adjusted_read_pos, self.audio_capacity);
        }

        (read_pos, available as usize)
    }

    /// Commit a repaired read position back to the shared header.
    ///
    /// There is exactly one daemon reader and one HAL writer per ring, so
    /// atomic stores from this `&self` method are well-defined.
    fn commit_read_position(&self, new_read_pos: u64) {
        self.header()
            .read_position
            .store(new_read_pos, Ordering::Release);
    }

    /// Read audio from the shared memory ring buffer.
    /// Returns the number of frames actually read.
    pub fn read_audio(&self, buffer: &mut [f32]) -> usize {
        let header = self.header();
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;
        let sample_count = buffer.len();

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let (read_pos, available) = self.compute_repair(write_pos, read_pos);
        let to_read = sample_count.min(available);

        if to_read == 0 {
            buffer.fill(0.0);
            return 0;
        }

        let read_index = (read_pos as usize) % self.audio_capacity;
        let first_part = to_read.min(self.audio_capacity - read_index);
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
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;
        let sample_count = buffer.len();

        let write_pos = header.write_position.load(Ordering::Acquire);
        let read_pos = header.read_position.load(Ordering::Acquire);

        let (_, used) = self.compute_repair(write_pos, read_pos);
        let available = self.audio_capacity.saturating_sub(used);
        let to_write = sample_count.min(available);

        if to_write == 0 {
            return 0;
        }

        let write_index = (write_pos as usize) % self.audio_capacity;
        let first_part = to_write.min(self.audio_capacity - write_index);
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

    fn available_encrypted_read_frames(
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

            let Some(record) = parse_encrypted_record_header(&header_bytes, self.audio_capacity)
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
        self.audio_capacity.saturating_sub(used) / channel_count
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
        let mut ciphertext_buf = Vec::new();
        let mut encrypted_buf = Vec::new();
        self.write_audio_encrypted_into(buffer, cipher, &mut ciphertext_buf, &mut encrypted_buf)
    }

    /// Read audio with decryption (allocating, convenience wrapper).
    pub fn read_audio_encrypted(
        &self,
        buffer: &mut [f32],
        cipher: &crate::encryption::AudioCipher,
    ) -> usize {
        let mut encrypted_buf = Vec::new();
        let mut ciphertext_buf = Vec::new();
        self.read_audio_encrypted_into(buffer, cipher, &mut encrypted_buf, &mut ciphertext_buf)
    }

    fn peek_encrypted_record_header(
        &self,
        read_pos: u64,
        available_slots: usize,
        encrypted_buf: &mut Vec<f32>,
        ciphertext_buf: &mut Vec<u8>,
    ) -> Option<Option<EncryptedRecordHeader>> {
        if available_slots < ENCRYPTED_RECORD_HEADER_SLOTS {
            return Some(None);
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
            self.audio_capacity,
        )
        .map(Some)
    }

    fn read_next_encrypted_record_into(
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

        if record.slot_count > available_slots {
            return EncryptedRecordRead::Empty;
        }
        if record.sample_count > output.len() {
            return EncryptedRecordRead::OutputTooSmall {
                sample_count: record.sample_count,
            };
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
        if !self.is_encrypted() {
            return self.read_audio(buffer);
        }

        let channel_count = self.header().channel_count.load(Ordering::Acquire) as usize;
        if channel_count == 0 {
            buffer.fill(0.0);
            return 0;
        }

        let mut copied_samples = 0;

        while copied_samples < buffer.len() {
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

        if copied_samples < buffer.len() {
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
        if !self.is_encrypted() {
            return self.write_audio(samples);
        }

        let header = self.header();
        let channel_count = header.channel_count.load(Ordering::Acquire) as usize;
        let sample_count = samples.len();

        if channel_count == 0 || sample_count == 0 {
            return 0;
        }

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
        let available = self.audio_capacity.saturating_sub(used);

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

// =============================================================================
// Adapter types for compatibility with existing code
// =============================================================================

/// Reader adapter for HAL input.
///
/// The cipher is loaded once at construction. If the daemon rotates the
/// session key while we're running, the read path detects the fingerprint
/// mismatch and returns silence (RT-safe) until [`HalInputReader::reload_cipher`]
/// is invoked from a non-RT control thread.
#[derive(Default)]
pub struct HalInputReader {
    buffer: Option<SharedAudioBuffer>,
    cipher: Option<crate::encryption::AudioCipher>,
    encrypted_samples_buf: Vec<f32>,
    ciphertext_buf: Vec<u8>,
    decrypted_record_buf: Vec<f32>,
    pending_decrypted_samples: Vec<f32>,
    pending_sample_offset: usize,
}

fn pre_alloc_capacity_samples() -> usize {
    MAX_HAL_BUFFER_FRAMES as usize * MAX_HAL_CHANNEL_COUNT as usize
}

fn read_encrypted_with_staging(
    shared: &SharedAudioBuffer,
    output: &mut [f32],
    cipher: &crate::encryption::AudioCipher,
    encrypted_samples_buf: &mut Vec<f32>,
    ciphertext_buf: &mut Vec<u8>,
    decrypted_record_buf: &mut Vec<f32>,
    pending_decrypted_samples: &mut Vec<f32>,
    pending_sample_offset: &mut usize,
) -> usize {
    let channel_count = shared.channel_count() as usize;
    if channel_count == 0 {
        output.fill(0.0);
        return 0;
    }

    let mut copied_samples = 0;

    if *pending_sample_offset < pending_decrypted_samples.len() {
        let pending_available = pending_decrypted_samples.len() - *pending_sample_offset;
        let to_copy = pending_available.min(output.len());
        output[..to_copy].copy_from_slice(
            &pending_decrypted_samples[*pending_sample_offset..*pending_sample_offset + to_copy],
        );
        *pending_sample_offset += to_copy;
        copied_samples += to_copy;

        if *pending_sample_offset >= pending_decrypted_samples.len() {
            pending_decrypted_samples.clear();
            *pending_sample_offset = 0;
        }
    }

    while copied_samples < output.len() {
        match shared.read_next_encrypted_record_into(
            decrypted_record_buf,
            cipher,
            encrypted_samples_buf,
            ciphertext_buf,
        ) {
            EncryptedRecordRead::Read { sample_count } => {
                let remaining = output.len() - copied_samples;
                let to_copy = sample_count.min(remaining);
                output[copied_samples..copied_samples + to_copy]
                    .copy_from_slice(&decrypted_record_buf[..to_copy]);
                copied_samples += to_copy;

                if to_copy < sample_count {
                    pending_decrypted_samples.clear();
                    pending_decrypted_samples
                        .extend_from_slice(&decrypted_record_buf[to_copy..sample_count]);
                    *pending_sample_offset = 0;
                    break;
                }
            }
            EncryptedRecordRead::OutputTooSmall { sample_count } => {
                // Worst-case sample_count is MAX_HAL_BUFFER_FRAMES *
                // MAX_HAL_CHANNEL_COUNT, which `HalInputReader::new`
                // pre-allocates for. If a caller hand-constructs the
                // reader with smaller capacity (e.g. in tests) the resize
                // will allocate the first time — RT-safety relies on the
                // production constructor.
                decrypted_record_buf.resize(sample_count, 0.0);
            }
            EncryptedRecordRead::Corrupt { .. } | EncryptedRecordRead::InvalidHeader => {
                if copied_samples == 0 {
                    output.fill(0.0);
                    return 0;
                }
                break;
            }
            EncryptedRecordRead::Empty => break,
        }
    }

    if copied_samples < output.len() {
        output[copied_samples..].fill(0.0);
    }

    copied_samples / channel_count
}

impl HalInputReader {
    /// Create a new HAL input reader.
    ///
    /// Pre-allocates staging buffers sized for the worst-case HAL geometry
    /// so the audio path will never reallocate. If encryption is enabled at
    /// construction time, the session key is loaded once here (off the
    /// audio thread).
    pub fn new() -> Option<Self> {
        let path = get_shared_memory_path();
        log::info!("[HAL INPUT] Attempting to open SharedMemory at: {:?}", path);

        match SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                log::info!(
                    "[HAL INPUT] SharedMemory opened: sample_rate={}, buffer_frames={}, channels={}, driver_ready={}, active={}",
                    buffer.sample_rate(),
                    buffer.buffer_frames(),
                    buffer.channel_count(),
                    buffer.driver_ready(),
                    buffer.is_active()
                );

                let pre_alloc = pre_alloc_capacity_samples();
                let encrypted_slots = encrypted_record_slots(pre_alloc).unwrap_or(pre_alloc * 2);
                let ciphertext_bytes =
                    encrypted_record_total_bytes(pre_alloc).unwrap_or(pre_alloc * 8);

                let cipher = load_initial_cipher(&buffer);

                Some(Self {
                    buffer: Some(buffer),
                    cipher,
                    encrypted_samples_buf: Vec::with_capacity(encrypted_slots),
                    ciphertext_buf: Vec::with_capacity(ciphertext_bytes),
                    decrypted_record_buf: Vec::with_capacity(pre_alloc),
                    pending_decrypted_samples: Vec::with_capacity(pre_alloc),
                    pending_sample_offset: 0,
                })
            }
            Err(e) => {
                log::error!("[HAL INPUT] Failed to open SharedMemory: {}", e);
                None
            }
        }
    }

    /// Re-load the session key from disk and replace the cached cipher.
    ///
    /// Must be called from a non-RT thread. Audio reads return silence
    /// while the cached cipher's fingerprint disagrees with the header's
    /// — call this to recover.
    pub fn reload_cipher(&mut self) -> std::io::Result<()> {
        if let Some(buf) = self.buffer.as_ref() {
            let key = crate::encryption::load_session_key()?;
            let cipher = crate::encryption::AudioCipher::new(&key);
            if cipher.fingerprint() == &buf.key_fingerprint() {
                self.cipher = Some(cipher);
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Loaded session key fingerprint does not match shared memory header",
                ))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalInputReader not connected to shared memory",
            ))
        }
    }

    /// Returns true when encrypted shared memory is active but this reader has
    /// no matching cached cipher.
    pub fn needs_cipher_reload(&self) -> bool {
        let Some(buf) = self.buffer.as_ref() else {
            return false;
        };
        if !buf.is_encrypted() {
            return false;
        }
        let header_fingerprint = buf.key_fingerprint();
        !self
            .cipher
            .as_ref()
            .map(|cipher| cipher.fingerprint() == &header_fingerprint)
            .unwrap_or(false)
    }

    /// Check if connected to the HAL driver.
    pub fn is_connected(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.driver_ready())
            .unwrap_or(false)
    }

    /// Read audio samples from the HAL.
    ///
    /// Real-time safe: no filesystem I/O, no allocations, no per-call
    /// formatting. If encryption is on and the cached cipher's fingerprint
    /// no longer matches the header, returns silence.
    pub fn read(&mut self, buffer: &mut [f32]) -> usize {
        if let Some(buf) = &self.buffer {
            buf.refresh_daemon_heartbeat();

            if buf.is_encrypted() {
                let header_fingerprint = buf.key_fingerprint();
                let fingerprint_ok = self
                    .cipher
                    .as_ref()
                    .map(|c| c.fingerprint() == &header_fingerprint)
                    .unwrap_or(false);

                if !fingerprint_ok {
                    // RT-safe: silence until a control thread calls
                    // `reload_cipher`. No disk I/O on the audio path.
                    buffer.fill(0.0);
                    return 0;
                }

                if let Some(cipher) = &self.cipher {
                    return read_encrypted_with_staging(
                        buf,
                        buffer,
                        cipher,
                        &mut self.encrypted_samples_buf,
                        &mut self.ciphertext_buf,
                        &mut self.decrypted_record_buf,
                        &mut self.pending_decrypted_samples,
                        &mut self.pending_sample_offset,
                    );
                }
                buffer.fill(0.0);
                return 0;
            }

            buf.read_audio(buffer)
        } else {
            0
        }
    }

    /// Get the current HAL format as `(sample_rate, channel_count, buffer_frames)`.
    ///
    /// Returns `Err` when the reader is not connected to shared memory.
    /// Prefer this over the legacy `sample_rate()`/`channel_count()`
    /// accessors which returned 0 (or formerly 48000/2) on disconnect.
    pub fn current_format(&self) -> std::io::Result<(u32, u32, u32)> {
        match self.buffer.as_ref() {
            Some(buf) => Ok((buf.sample_rate(), buf.channel_count(), buf.buffer_frames())),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalInputReader not connected to shared memory",
            )),
        }
    }

    /// Sample rate (returns 0 when disconnected). Prefer
    /// [`HalInputReader::current_format`].
    pub fn sample_rate(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(0)
    }

    /// Channel count (returns 0 when disconnected). Prefer
    /// [`HalInputReader::current_format`].
    pub fn channel_count(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.channel_count()).unwrap_or(0)
    }

    /// Get available frames to read.
    pub fn available_read_frames(&self) -> usize {
        let shared_frames = self
            .buffer
            .as_ref()
            .map(|b| {
                b.refresh_daemon_heartbeat();
                b.available_read_frames()
            })
            .unwrap_or(0);
        let pending_samples = self
            .pending_decrypted_samples
            .len()
            .saturating_sub(self.pending_sample_offset);
        let channels = self.channel_count() as usize;
        shared_frames + pending_samples.checked_div(channels).unwrap_or(0)
    }
}

/// Best-effort cipher load on the non-RT init path.
fn load_initial_cipher(buffer: &SharedAudioBuffer) -> Option<crate::encryption::AudioCipher> {
    if !buffer.is_encrypted() {
        return None;
    }
    match crate::encryption::load_session_key() {
        Ok(key) => {
            let cipher = crate::encryption::AudioCipher::new(&key);
            if cipher.fingerprint() == &buffer.key_fingerprint() {
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

/// Writer adapter for HAL output.
///
/// Same RT-safety contract as [`HalInputReader`]: cipher loaded once at
/// construction, audio path performs no filesystem I/O.
#[derive(Default)]
pub struct HalOutputWriter {
    buffer: Option<SharedAudioBuffer>,
    cipher: Option<crate::encryption::AudioCipher>,
    ciphertext_buf: Vec<u8>,
    encrypted_buf: Vec<f32>,
}

impl HalOutputWriter {
    /// Create a new HAL output writer.
    pub fn new() -> Option<Self> {
        match SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                let pre_alloc = pre_alloc_capacity_samples();
                let ciphertext_bytes =
                    encrypted_record_total_bytes(pre_alloc).unwrap_or(pre_alloc * 8);
                let encrypted_slots = encrypted_record_slots(pre_alloc).unwrap_or(pre_alloc * 2);
                let cipher = load_initial_cipher(&buffer);
                Some(Self {
                    buffer: Some(buffer),
                    cipher,
                    ciphertext_buf: Vec::with_capacity(ciphertext_bytes),
                    encrypted_buf: Vec::with_capacity(encrypted_slots),
                })
            }
            Err(_) => None,
        }
    }

    /// Re-load the session key from disk and replace the cached cipher.
    pub fn reload_cipher(&mut self) -> std::io::Result<()> {
        if let Some(buf) = self.buffer.as_ref() {
            let key = crate::encryption::load_session_key()?;
            let cipher = crate::encryption::AudioCipher::new(&key);
            if cipher.fingerprint() == &buf.key_fingerprint() {
                self.cipher = Some(cipher);
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Loaded session key fingerprint does not match shared memory header",
                ))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalOutputWriter not connected to shared memory",
            ))
        }
    }

    /// Check if connected to the HAL driver.
    pub fn is_connected(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.driver_ready())
            .unwrap_or(false)
    }

    /// Write audio samples to the HAL.
    pub fn write(&mut self, buffer: &[f32]) -> usize {
        let is_encrypted = self.buffer.as_ref().is_some_and(|b| b.is_encrypted());

        if is_encrypted {
            let header_fingerprint = self.buffer.as_ref().map(|b| b.key_fingerprint());
            let fingerprint_ok = matches!(
                (&self.cipher, header_fingerprint),
                (Some(c), Some(fp)) if c.fingerprint() == &fp
            );

            if !fingerprint_ok {
                return 0;
            }

            if let Some(cipher) = self.cipher.as_ref()
                && let Some(buf) = &mut self.buffer
            {
                return buf.write_audio_encrypted_into(
                    buffer,
                    cipher,
                    &mut self.ciphertext_buf,
                    &mut self.encrypted_buf,
                );
            }
            return 0;
        }

        if let Some(buf) = &mut self.buffer {
            buf.write_audio(buffer)
        } else {
            0
        }
    }

    /// Get the current HAL format as `(sample_rate, channel_count, buffer_frames)`.
    /// Returns `Err` when disconnected.
    pub fn current_format(&self) -> std::io::Result<(u32, u32, u32)> {
        match self.buffer.as_ref() {
            Some(buf) => Ok((buf.sample_rate(), buf.channel_count(), buf.buffer_frames())),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalOutputWriter not connected to shared memory",
            )),
        }
    }

    /// Sample rate (returns 0 when disconnected). Prefer
    /// [`HalOutputWriter::current_format`].
    pub fn sample_rate(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(0)
    }

    /// Channel count (returns 0 when disconnected). Prefer
    /// [`HalOutputWriter::current_format`].
    pub fn channel_count(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.channel_count()).unwrap_or(0)
    }

    /// Buffer frame size (returns 0 when disconnected). Prefer
    /// [`HalOutputWriter::current_format`].
    pub fn buffer_frames(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.buffer_frames()).unwrap_or(0)
    }

    /// Set sample rate via the quiesced reconfiguration protocol.
    pub fn set_sample_rate(&mut self, sample_rate: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_sample_rate(sample_rate);
            true
        } else {
            false
        }
    }

    /// Set channel count via the quiesced reconfiguration protocol.
    pub fn set_channel_count(&mut self, channel_count: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_channel_count(channel_count);
            true
        } else {
            false
        }
    }

    /// Set buffer frame size via the quiesced reconfiguration protocol.
    pub fn set_buffer_frames(&mut self, buffer_frames: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_buffer_frames(buffer_frames);
            true
        } else {
            false
        }
    }

    /// Set engine ready flag.
    pub fn set_engine_ready(&self, ready: bool) {
        if let Some(buffer) = &self.buffer {
            buffer.set_engine_ready(ready);
        }
    }

    /// Check if configuration has changed (signaled by Swift driver).
    pub fn config_changed(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.config_changed())
            .unwrap_or(false)
    }

    /// Clear the configuration changed flag.
    pub fn clear_config_changed(&self) {
        if let Some(buffer) = &self.buffer {
            buffer.clear_config_changed();
        }
    }

    /// Signal configuration change to the Swift driver.
    pub fn set_config_changed(&self) {
        if let Some(buffer) = &self.buffer {
            buffer.set_config_changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, tempdir};

    #[test]
    fn test_header_size() {
        assert!(std::mem::size_of::<SharedAudioHeader>() <= 256);
        assert_eq!(std::mem::align_of::<SharedAudioHeader>(), 8);
    }

    #[test]
    fn test_shared_memory_path_supports_lab_overrides() {
        let explicit = shared_memory_path_from_env(
            Some(OsString::from("/tmp/sotf-lab/custom-audio.shm")),
            Some(OsString::from("/tmp/ignored")),
            42,
        );
        assert_eq!(explicit, PathBuf::from("/tmp/sotf-lab/custom-audio.shm"));

        let runtime = shared_memory_path_from_env(None, Some(OsString::from("/tmp/sotf-lab")), 42);
        assert_eq!(runtime, PathBuf::from("/tmp/sotf-lab/audio.shm"));

        let fallback = shared_memory_path_from_env(None, None, 42);
        assert_eq!(fallback, PathBuf::from("/tmp/sotf-42/audio.shm"));
    }

    fn create_mock_shared_memory(
        sample_rate: u32,
        buffer_frames: u32,
        channel_count: u32,
    ) -> NamedTempFile {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let buffer = SharedAudioBuffer::create_or_open(
            temp_file.path(),
            sample_rate,
            buffer_frames,
            channel_count,
        )
        .expect("Failed to create mock shared memory");
        buffer.header().driver_ready.store(1, Ordering::Release);
        buffer.header().active.store(1, Ordering::Release);
        drop(buffer);
        temp_file
    }

    #[test]
    fn test_create_or_open_initializes_daemon_owned_file() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let buffer = SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 512, 2)
            .expect("Failed to create shared memory");

        assert_eq!(buffer.sample_rate(), 48_000);
        assert_eq!(buffer.buffer_frames(), 512);
        assert_eq!(buffer.channel_count(), 2);
        assert!(!buffer.driver_ready());
        assert!(!buffer.is_active());

        let reopened = SharedAudioBuffer::open(temp_file.path()).expect("Failed to reopen buffer");
        assert_eq!(reopened.sample_rate(), 48_000);
        assert_eq!(reopened.buffer_frames(), 512);
        assert_eq!(reopened.channel_count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn test_create_or_open_rejects_symlink_shared_memory_file() {
        let dir = tempdir().expect("Failed to create temp dir");
        let target = dir.path().join("target");
        std::fs::write(&target, b"not shared memory").expect("Failed to create target file");
        let link = dir.path().join("audio.shm");
        std::os::unix::fs::symlink(&target, &link).expect("Failed to create symlink");

        assert!(
            SharedAudioBuffer::create_or_open(&link, 48_000, 512, 2).is_err(),
            "symlink shared-memory path must be rejected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_create_or_open_clamps_file_mode_to_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("audio.shm");
        let _buffer = SharedAudioBuffer::create_or_open(&path, 48_000, 512, 2)
            .expect("Failed to create shared memory");

        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn test_create_or_open_creates_missing_parent_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().expect("Failed to create temp dir");
        let parent = dir.path().join("new-parent");
        let path = parent.join("audio.shm");
        let _buffer = SharedAudioBuffer::create_or_open(&path, 48_000, 512, 2)
            .expect("Failed to create shared memory");

        let mode = std::fs::metadata(&parent)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    /// Regression test for the `&mut SharedAudioHeader` data-race fix:
    /// previously these fields were plain `u32`/`u64` written via
    /// `header_mut()`. Verifies every cross-process field round-trips
    /// through an atomic store/load and survives drop+reopen.
    #[test]
    fn test_atomic_field_roundtrip_for_cross_process_fields() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        // Size the mapping for the maximum geometry we'll exercise below
        // (1024 frames, 8 channels) so the post-mutation reopen sees a
        // mapping large enough to hold the (now larger) declared geometry.
        let buffer = SharedAudioBuffer::create_or_open_with_max_geometry(
            temp_file.path(),
            44_100,
            256,
            6,
            1024,
            8,
        )
        .expect("Failed to create shared memory");

        let h = buffer.header();
        h.sample_rate.store(96_000, Ordering::Release);
        h.buffer_frames.store(1024, Ordering::Release);
        h.channel_count.store(6, Ordering::Release);
        h.requested_sample_rate.store(48_000, Ordering::Release);
        h.requested_buffer_frames.store(512, Ordering::Release);
        h.actual_sample_rate.store(48_000, Ordering::Release);
        h.actual_buffer_frames.store(512, Ordering::Release);
        h.config_error_code.store(7, Ordering::Release);
        buffer.set_key_fingerprint([0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);
        buffer.set_encrypted(true);
        h.active.store(1, Ordering::Release);
        h.driver_ready.store(1, Ordering::Release);
        h.engine_ready.store(1, Ordering::Release);
        h.configuring.store(0, Ordering::Release);

        assert_eq!(buffer.sample_rate(), 96_000);
        assert_eq!(buffer.buffer_frames(), 1024);
        assert_eq!(buffer.channel_count(), 6);
        assert_eq!(buffer.requested_sample_rate(), 48_000);
        assert_eq!(buffer.requested_buffer_frames(), 512);
        assert_eq!(buffer.actual_sample_rate(), 48_000);
        assert_eq!(buffer.actual_buffer_frames(), 512);
        assert_eq!(buffer.config_error_code(), 7);
        assert_eq!(
            buffer.key_fingerprint(),
            [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]
        );
        assert!(buffer.is_encrypted());
        assert!(buffer.is_active());
        assert!(buffer.driver_ready());

        drop(buffer);
        let reopened = SharedAudioBuffer::open(temp_file.path()).expect("Failed to reopen");
        assert_eq!(reopened.sample_rate(), 96_000);
        assert_eq!(reopened.buffer_frames(), 1024);
        assert_eq!(reopened.channel_count(), 6);
        assert_eq!(reopened.requested_sample_rate(), 48_000);
        assert_eq!(reopened.actual_sample_rate(), 48_000);
        assert_eq!(reopened.config_error_code(), 7);
        assert_eq!(
            reopened.key_fingerprint(),
            [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]
        );
        assert!(reopened.is_encrypted());
    }

    #[test]
    fn test_current_format_returns_err_when_disconnected() {
        let reader = HalInputReader::default();
        assert!(reader.current_format().is_err());
        assert_eq!(reader.sample_rate(), 0);
        assert_eq!(reader.channel_count(), 0);

        let writer = HalOutputWriter::default();
        assert!(writer.current_format().is_err());
        assert_eq!(writer.sample_rate(), 0);
        assert_eq!(writer.channel_count(), 0);
        assert_eq!(writer.buffer_frames(), 0);
    }

    #[test]
    fn test_reconfigure_quiesced_sets_and_clears_configuring_flag() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let mut buffer =
            SharedAudioBuffer::create_or_open_with_capacity(temp_file.path(), 48_000, 512, 2, 32)
                .expect("Failed to create shared memory");

        assert_eq!(buffer.header().configuring.load(Ordering::Acquire), 0);

        buffer.reconfigure_quiesced(Some(96_000), Some(1024), Some(8));

        assert_eq!(
            buffer.header().configuring.load(Ordering::Acquire),
            0,
            "configuring flag must be cleared after reconfigure_quiesced returns"
        );
        assert!(buffer.config_changed());
        assert_eq!(buffer.sample_rate(), 96_000);
        assert_eq!(buffer.buffer_frames(), 1024);
        assert_eq!(buffer.channel_count(), 8);
        assert_eq!(buffer.actual_sample_rate(), 96_000);
        assert_eq!(buffer.actual_buffer_frames(), 1024);
        assert_eq!(buffer.header().write_position.load(Ordering::Acquire), 0);
        assert_eq!(buffer.header().read_position.load(Ordering::Acquire), 0);
    }

    #[test]
    fn test_create_or_open_with_capacity_allows_hal_growth_to_32ch() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let mut buffer =
            SharedAudioBuffer::create_or_open_with_capacity(temp_file.path(), 48_000, 512, 2, 32)
                .expect("Failed to create shared memory");

        assert_eq!(buffer.channel_count(), 2);
        buffer.set_channel_count(32);

        assert_eq!(buffer.channel_count(), 32);
        assert_eq!(buffer.header().write_position.load(Ordering::Acquire), 0);
        assert_eq!(buffer.header().read_position.load(Ordering::Acquire), 0);

        let reopened = SharedAudioBuffer::open(temp_file.path()).expect("Failed to reopen buffer");
        assert_eq!(reopened.channel_count(), 32);
    }

    #[test]
    fn test_create_or_open_preserves_runtime_state_for_same_geometry() {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let buffer = SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 512, 2)
            .expect("Failed to create shared memory");
        buffer.set_engine_ready(true);
        buffer.header().driver_ready.store(1, Ordering::Release);
        buffer.header().write_position.store(64, Ordering::Release);
        buffer.header().read_position.store(32, Ordering::Release);
        drop(buffer);

        let reopened = SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 512, 2)
            .expect("Failed to reopen shared memory");

        assert!(reopened.driver_ready());
        assert_eq!(reopened.header().engine_ready.load(Ordering::Acquire), 1);
        assert_eq!(reopened.header().write_position.load(Ordering::Acquire), 64);
        assert_eq!(reopened.header().read_position.load(Ordering::Acquire), 32);
    }

    #[test]
    fn test_shared_memory_roundtrip_bit_exact() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        assert_eq!(buffer.sample_rate(), sample_rate);
        assert_eq!(buffer.buffer_frames(), buffer_frames);
        assert_eq!(buffer.channel_count(), channel_count);
        assert!(buffer.driver_ready());

        let num_samples = buffer_frames as usize * channel_count as usize;
        let input_audio: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
            })
            .collect();

        let frames_written = buffer.write_audio(&input_audio);
        assert_eq!(frames_written, buffer_frames as usize);

        let mut output_audio = vec![0.0f32; num_samples];
        let frames_read = buffer.read_audio(&mut output_audio);
        assert_eq!(frames_read, buffer_frames as usize);

        for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
            assert_eq!(input.to_bits(), output.to_bits(), "Sample {} mismatch", i);
        }
    }

    #[test]
    fn test_invalid_magic_number() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        let mut buffer = vec![0u8; 4096];
        buffer[0..4].copy_from_slice(&0x12345678u32.to_ne_bytes());
        buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        assert!(
            result
                .as_ref()
                .err()
                .map(|e| e.to_string().contains("Invalid shared memory magic"))
                .unwrap_or(false),
            "Expected magic error, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_invalid_version() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        let mut buffer = vec![0u8; 4096];
        buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
        buffer[4..8].copy_from_slice(&99u32.to_ne_bytes());
        file.write_all(&buffer).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let result = SharedAudioBuffer::open(file.path());
        assert!(
            result
                .as_ref()
                .err()
                .map(|e| e.to_string().contains("Incompatible shared memory version"))
                .unwrap_or(false),
            "Expected version error, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_config_negotiation_round_trip() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        let new_sample_rate = 96000;
        let new_buffer_frames = 512;
        buffer.request_config_change(new_sample_rate, new_buffer_frames, channel_count, 1);

        assert!(buffer.config_changed());
        assert_eq!(buffer.config_source(), 1);
        assert_eq!(buffer.requested_sample_rate(), new_sample_rate);
        assert_eq!(buffer.requested_buffer_frames(), new_buffer_frames);

        buffer.acknowledge_config_change(new_sample_rate, new_buffer_frames, 1, 0);

        assert!(!buffer.config_changed());
        assert_eq!(buffer.config_status(), 1);
        assert_eq!(buffer.actual_sample_rate(), new_sample_rate);
        assert_eq!(buffer.actual_buffer_frames(), new_buffer_frames);
    }

    #[test]
    fn test_config_negotiation_error() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        buffer.request_config_change(999_999, 512, channel_count, 1);
        buffer.acknowledge_config_change(0, 0, 3, 42);

        assert_eq!(buffer.config_status(), 3);
        assert_eq!(buffer.config_error_code(), 42);
    }

    #[test]
    fn test_frame_counter_increment() {
        let sample_rate = 48000;
        let buffer_frames = 1024;
        let channel_count = 2;
        let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        assert_eq!(buffer.frame_counter(), 0);
        let new_counter = buffer.increment_frame_counter();
        assert_eq!(new_counter, 1);
        assert_eq!(buffer.frame_counter(), 1);

        for expected in 2..=100 {
            let counter = buffer.increment_frame_counter();
            assert_eq!(counter, expected);
        }
    }

    fn test_audio_cipher() -> crate::encryption::AudioCipher {
        let key = [0x42u8; 32];
        crate::encryption::AudioCipher::new(&key)
    }

    fn sequential_audio(frame_count: usize, channel_count: usize, offset: usize) -> Vec<f32> {
        (0..frame_count * channel_count)
            .map(|sample| ((offset + sample) as f32 * 0.0001) - 0.5)
            .collect()
    }

    #[test]
    fn test_encrypted_available_read_frames_reports_plaintext_frames() {
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        let cipher = test_audio_cipher();
        buffer.set_key_fingerprint(*cipher.fingerprint());
        buffer.set_encrypted(true);

        let mut ciphertext_buf = Vec::new();
        let mut encrypted_buf = Vec::new();
        let samples = sequential_audio(192, 2, 0);

        assert_eq!(
            buffer.write_audio_encrypted_into(
                &samples,
                &cipher,
                &mut ciphertext_buf,
                &mut encrypted_buf,
            ),
            192
        );

        assert_eq!(buffer.available_read_frames(), 192);
    }

    #[test]
    fn test_flush_audio_drops_pending_ring_data() {
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        let samples = sequential_audio(128, 2, 0);

        assert_eq!(buffer.write_audio(&samples), 128);
        assert_eq!(buffer.available_read_frames(), 128);

        buffer.flush_audio();

        assert_eq!(buffer.available_read_frames(), 0);
        assert_eq!(
            buffer.header().write_position.load(Ordering::Acquire),
            buffer.header().read_position.load(Ordering::Acquire)
        );
    }

    #[test]
    fn test_read_audio_handles_inverted_ring_positions() {
        // After the repair refactor the reader does NOT rewrite
        // `read_position` from a shared reference on plain reads — the
        // repaired position is consumed locally and only the post-read
        // position is committed. This test verifies inverted positions
        // still return 0 frames and don't leak stale data.
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        buffer.header().write_position.store(64, Ordering::Release);
        buffer.header().read_position.store(96, Ordering::Release);

        let mut output = vec![1.0; 32];
        assert_eq!(buffer.read_audio(&mut output), 0);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn test_available_read_frames_clamps_overfull_ring() {
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        let capacity = buffer.audio_capacity as u64;

        buffer
            .header()
            .write_position
            .store(capacity + 128, Ordering::Release);
        buffer.header().read_position.store(0, Ordering::Release);

        assert_eq!(buffer.available_read_frames(), buffer.audio_capacity / 2);
    }

    #[test]
    fn test_encrypted_hal_sized_records_read_back_in_plaintext_order() {
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        let cipher = test_audio_cipher();
        buffer.set_key_fingerprint(*cipher.fingerprint());
        buffer.set_encrypted(true);

        let mut ciphertext_buf = Vec::new();
        let mut encrypted_buf = Vec::new();
        let mut expected = Vec::new();

        for chunk in 0..5 {
            let samples = sequential_audio(192, 2, chunk * 192 * 2);
            expected.extend_from_slice(&samples);
            assert_eq!(
                buffer.write_audio_encrypted_into(
                    &samples,
                    &cipher,
                    &mut ciphertext_buf,
                    &mut encrypted_buf,
                ),
                192
            );
        }

        let mut output = vec![0.0; expected.len()];
        let frames_read = buffer.read_audio_encrypted_into(
            &mut output,
            &cipher,
            &mut encrypted_buf,
            &mut ciphertext_buf,
        );

        assert_eq!(frames_read, 960);
        for (index, (actual, expected)) in output.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "sample {index} mismatch"
            );
        }
    }

    #[test]
    fn test_hal_input_reader_stages_partial_encrypted_record() {
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        let writer_cipher = test_audio_cipher();
        let reader_cipher = test_audio_cipher();
        buffer.set_key_fingerprint(*writer_cipher.fingerprint());
        buffer.set_encrypted(true);

        let mut ciphertext_buf = Vec::new();
        let mut encrypted_buf = Vec::new();
        let mut expected = Vec::new();

        for chunk in 0..6 {
            let samples = sequential_audio(192, 2, chunk * 192 * 2);
            expected.extend_from_slice(&samples);
            assert_eq!(
                buffer.write_audio_encrypted_into(
                    &samples,
                    &writer_cipher,
                    &mut ciphertext_buf,
                    &mut encrypted_buf,
                ),
                192
            );
        }

        let mut reader = HalInputReader {
            buffer: Some(buffer),
            cipher: Some(reader_cipher),
            encrypted_samples_buf: Vec::new(),
            ciphertext_buf: Vec::new(),
            decrypted_record_buf: Vec::new(),
            pending_decrypted_samples: Vec::new(),
            pending_sample_offset: 0,
        };

        let mut output = vec![0.0; 1024 * 2];
        assert_eq!(reader.available_read_frames(), 1152);
        assert_eq!(reader.read(&mut output), 1024);
        for (index, (actual, expected)) in output.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "sample {index} mismatch"
            );
        }

        let mut tail = vec![0.0; 128 * 2];
        assert_eq!(reader.available_read_frames(), 128);
        assert_eq!(reader.read(&mut tail), 128);
        for (index, (actual, expected)) in
            tail.iter().zip(expected[output.len()..].iter()).enumerate()
        {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "tail sample {index} mismatch"
            );
        }
    }

    #[test]
    fn test_hal_input_reader_reports_cipher_reload_need() {
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        let writer_cipher = test_audio_cipher();
        let stale_cipher = crate::encryption::AudioCipher::new(&[0x43u8; 32]);

        buffer.set_key_fingerprint(*writer_cipher.fingerprint());
        buffer.set_encrypted(false);

        let mut reader = HalInputReader {
            buffer: Some(buffer),
            cipher: Some(stale_cipher),
            encrypted_samples_buf: Vec::new(),
            ciphertext_buf: Vec::new(),
            decrypted_record_buf: Vec::new(),
            pending_decrypted_samples: Vec::new(),
            pending_sample_offset: 0,
        };

        assert!(
            !reader.needs_cipher_reload(),
            "unencrypted shared memory should not require a cipher reload"
        );

        reader.buffer.as_ref().unwrap().set_encrypted(true);
        assert!(
            reader.needs_cipher_reload(),
            "encrypted shared memory should report stale cached cipher"
        );

        reader.cipher = Some(writer_cipher);
        assert!(
            !reader.needs_cipher_reload(),
            "matching cached cipher should be considered current"
        );

        reader.cipher = None;
        assert!(
            reader.needs_cipher_reload(),
            "encrypted shared memory without a cached cipher should reload"
        );
    }

    #[test]
    fn test_corrupt_encrypted_record_is_dropped() {
        let temp_file = create_mock_shared_memory(48_000, 512, 2);
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        let cipher = test_audio_cipher();
        buffer.set_key_fingerprint(*cipher.fingerprint());
        buffer.set_encrypted(true);

        let mut ciphertext_buf = Vec::new();
        let mut encrypted_buf = Vec::new();
        let corrupt = sequential_audio(192, 2, 0);
        let good = sequential_audio(192, 2, corrupt.len());

        assert_eq!(
            buffer.write_audio_encrypted_into(
                &corrupt,
                &cipher,
                &mut ciphertext_buf,
                &mut encrypted_buf,
            ),
            192
        );
        assert_eq!(
            buffer.write_audio_encrypted_into(
                &good,
                &cipher,
                &mut ciphertext_buf,
                &mut encrypted_buf,
            ),
            192
        );

        // SAFETY (test-only): flip one bit inside the first encrypted
        // record's ciphertext slot to simulate tampering.
        unsafe {
            let tampered_slot = buffer.audio_data_mut().add(6);
            *tampered_slot = f32::from_bits((*tampered_slot).to_bits() ^ 0x0000_0001);
        }

        let mut output = vec![0.0; corrupt.len()];
        assert_eq!(
            buffer.read_audio_encrypted_into(
                &mut output,
                &cipher,
                &mut encrypted_buf,
                &mut ciphertext_buf,
            ),
            0
        );

        let frames_read = buffer.read_audio_encrypted_into(
            &mut output,
            &cipher,
            &mut encrypted_buf,
            &mut ciphertext_buf,
        );

        assert_eq!(frames_read, 192);
        for (index, (actual, expected)) in output.iter().zip(good.iter()).enumerate() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "sample {index} mismatch"
            );
        }
    }

    #[test]
    fn test_engine_ready_flag_clears_heartbeat_after_ready() {
        let temp_file = create_mock_shared_memory(48_000, 1024, 2);
        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        let engine_ready = buffer.header().engine_ready.load(Ordering::Acquire);
        assert_eq!(engine_ready, 0);

        buffer.set_engine_ready(true);
        assert_eq!(buffer.header().engine_ready.load(Ordering::Acquire), 1);
        let first_heartbeat = buffer.header().daemon_heartbeat_ms.load(Ordering::Acquire);
        assert!(first_heartbeat > 0);

        buffer.refresh_daemon_heartbeat();
        let refreshed = buffer.header().daemon_heartbeat_ms.load(Ordering::Acquire);
        assert!(refreshed >= first_heartbeat);

        buffer.set_engine_ready(false);
        assert_eq!(buffer.header().engine_ready.load(Ordering::Acquire), 0);
        // After engine_ready=0, refresh_daemon_heartbeat must not revive
        // the heartbeat.
        buffer.refresh_daemon_heartbeat();
        assert_eq!(
            buffer.header().daemon_heartbeat_ms.load(Ordering::Acquire),
            0
        );
    }

    #[test]
    fn test_encryption_flag() {
        let temp_file = create_mock_shared_memory(48000, 1024, 2);
        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

        assert!(!buffer.is_encrypted());

        let fingerprint = [1, 2, 3, 4, 5, 6, 7, 8];
        buffer.set_encrypted(true);
        buffer.set_key_fingerprint(fingerprint);

        assert!(buffer.is_encrypted());
        assert_eq!(buffer.key_fingerprint(), fingerprint);

        buffer.set_encrypted(false);
        assert!(!buffer.is_encrypted());
    }

    #[test]
    fn test_active_flag() {
        let temp_file = create_mock_shared_memory(48000, 1024, 2);
        let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
        assert!(buffer.is_active());
    }

    #[test]
    fn test_multichannel_configurations() {
        let configurations = vec![
            (2, "Stereo"),
            (6, "5.1 Surround"),
            (8, "7.1 Surround"),
            (16, "9.1.6"),
            (32, "Maximum HAL supported"),
        ];

        for (channel_count, name) in configurations {
            let temp_file = create_mock_shared_memory(48000, 256, channel_count);
            let buffer = SharedAudioBuffer::open(temp_file.path())
                .unwrap_or_else(|_| panic!("Failed to open {} buffer", name));

            assert_eq!(buffer.channel_count(), channel_count, "{}", name);

            let samples = vec![0.5f32; 256 * channel_count as usize];
            let mut output = vec![0.0f32; 256 * channel_count as usize];

            let mut buffer = buffer;
            buffer.write_audio(&samples);
            buffer.read_audio(&mut output);

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
