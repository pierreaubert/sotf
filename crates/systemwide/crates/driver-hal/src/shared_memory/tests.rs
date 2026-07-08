use super::SharedAudioHeader;
use super::consts::SHARED_MEMORY_MAGIC;
use super::consts::SHARED_MEMORY_VERSION;
use super::hal_input_reader::HalInputReader;
use super::hal_output_writer::HalOutputWriter;
use super::misc::shared_memory_path_from_env;
use super::shared_audio_buffer::SharedAudioBuffer;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use std::io::Write;
use tempfile::{NamedTempFile, tempdir};

mod misc;

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

/// Corrupted shared-memory headers must be rejected rather than silently
/// used. Regression test for QA-SYS-001.
#[test]
fn test_corrupted_header_zero_channel_count() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let mut buffer = vec![0u8; 4096];
    buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
    buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
    // channel_count stays 0; valid sample_rate and buffer_frames.
    buffer[8..12].copy_from_slice(&48_000u32.to_ne_bytes());
    buffer[12..16].copy_from_slice(&512u32.to_ne_bytes());
    file.write_all(&buffer).expect("Failed to write");
    file.flush().expect("Failed to flush");

    let result = SharedAudioBuffer::open(file.path());
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e
                .to_string()
                .contains("Invalid shared memory configuration"))
            .unwrap_or(false),
        "Expected configuration error for zero channel_count, got: {:?}",
        result.err()
    );
}

#[test]
fn test_corrupted_header_out_of_range_geometry() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let mut buffer = vec![0u8; 4096];
    buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
    buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
    buffer[8..12].copy_from_slice(&48_000u32.to_ne_bytes());
    // buffer_frames far above the allowed maximum.
    buffer[12..16].copy_from_slice(&1_000_000u32.to_ne_bytes());
    buffer[16..20].copy_from_slice(&2u32.to_ne_bytes());
    file.write_all(&buffer).expect("Failed to write");
    file.flush().expect("Failed to flush");

    let result = SharedAudioBuffer::open(file.path());
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e.to_string().contains("out of range"))
            .unwrap_or(false),
        "Expected out-of-range error, got: {:?}",
        result.err()
    );
}

#[test]
fn test_open_rejects_file_too_small_for_header() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    // Write only the magic/version, not enough for the full header.
    let mut buffer = vec![0u8; 8];
    buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
    buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
    file.write_all(&buffer).expect("Failed to write");
    file.flush().expect("Failed to flush");

    let result = SharedAudioBuffer::open(file.path());
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e.to_string().contains("too small for header"))
            .unwrap_or(false),
        "Expected 'too small for header' error, got: {:?}",
        result.err()
    );
}

#[test]
fn test_open_rejects_file_too_small_for_declared_geometry() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let mut buffer = vec![0u8; 4096];
    buffer[0..4].copy_from_slice(&SHARED_MEMORY_MAGIC.to_ne_bytes());
    buffer[4..8].copy_from_slice(&SHARED_MEMORY_VERSION.to_ne_bytes());
    buffer[8..12].copy_from_slice(&48_000u32.to_ne_bytes());
    buffer[12..16].copy_from_slice(&512u32.to_ne_bytes());
    buffer[16..20].copy_from_slice(&2u32.to_ne_bytes());
    file.write_all(&buffer).expect("Failed to write");
    file.flush().expect("Failed to flush");

    let result = SharedAudioBuffer::open(file.path());
    assert!(
        result
            .as_ref()
            .err()
            .map(|e| e.to_string().contains("Shared memory too small"))
            .unwrap_or(false),
        "Expected 'too small' error for declared geometry, got: {:?}",
        result.err()
    );
}

/// Daemon restart with a different geometry must re-initialize the header,
/// resetting ring positions, ready flags, and encryption state. Same-geometry
/// reopen is covered by `test_create_or_open_preserves_runtime_state_for_same_geometry`.
#[test]
fn test_daemon_restart_with_geometry_change_resets_runtime_state() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let buffer = SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 512, 2)
        .expect("Failed to create shared memory");
    buffer.set_engine_ready(true);
    buffer.header().driver_ready.store(1, Ordering::Release);
    buffer.header().write_position.store(64, Ordering::Release);
    buffer.header().read_position.store(32, Ordering::Release);
    buffer.set_key_fingerprint([0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);
    buffer.set_encrypted(true);
    drop(buffer);

    // Reopen with a different geometry — this should trigger header
    // re-initialization and therefore reset runtime state.
    let reopened = SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 1024, 2)
        .expect("Failed to reopen shared memory");

    assert_eq!(reopened.buffer_frames(), 1024);
    assert_eq!(reopened.header().write_position.load(Ordering::Acquire), 0);
    assert_eq!(reopened.header().read_position.load(Ordering::Acquire), 0);
    assert_eq!(reopened.header().engine_ready.load(Ordering::Acquire), 0);
    assert!(!reopened.driver_ready());
    assert!(!reopened.is_encrypted());
    assert_eq!(reopened.key_fingerprint(), [0u8; 8]);
}

/// After daemon restart the HAL reader must refuse to load a cached cipher
/// whose fingerprint does not match the shared-memory header. This prevents
/// decoding with a stale key after key rotation. Regression test for
/// QA-SYS-001 daemon-restart / reconnection behavior.
#[test]
fn test_load_initial_cipher_rejects_fingerprint_mismatch() {
    use crate::encryption::{AudioCipher, generate_key};

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let buffer =
        SharedAudioBuffer::create_or_open(temp_file.path(), 48_000, 512, 2).expect("create");

    let stale_key = generate_key();
    let stale_cipher = AudioCipher::new(&stale_key);

    // Simulate a header that advertises encryption with a fingerprint that
    // does not match any key currently on disk.
    buffer.set_encrypted(true);
    buffer.set_key_fingerprint(*stale_cipher.fingerprint());

    let loaded = super::shared_audio_buffer::load_initial_cipher(&buffer);
    assert!(
        loaded.is_none(),
        "load_initial_cipher must return None when on-disk key fingerprint does not match header"
    );
}

/// Default shared-memory path must be user-isolated under the expected runtime
/// directory. Regression test for QA-SYS-001 path-bounding requirement.
#[test]
fn test_default_shared_memory_path_is_user_isolated() {
    let path = super::misc::get_shared_memory_path();
    let path_str = path.to_string_lossy();

    let uid = unsafe { libc::getuid() };
    let tmpdir = std::env::var("TMPDIR").ok();
    let runtime = std::env::var("SOTF_SYSTEMWIDE_RUNTIME_DIR").ok();

    let is_under_expected = runtime.map(|r| path_str.starts_with(&r)).unwrap_or(false)
        || tmpdir.map(|t| path_str.starts_with(&t)).unwrap_or(false)
        || path_str.starts_with(&format!("/tmp/sotf-{}/", uid));

    assert!(
        is_under_expected,
        "default shared-memory path should be user-isolated: {}",
        path_str
    );
}
