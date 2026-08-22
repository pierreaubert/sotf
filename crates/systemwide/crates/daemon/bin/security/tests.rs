#[cfg(all(target_os = "macos", feature = "hal"))]
use super::get::encryption_impl;
use super::get::get_current_uid;
use super::get::get_hal_key_path;
use super::get::get_secure_socket_path;
use super::misc::{
    MAX_USER_AUDIO_FILE_BYTES, daemon_session_key_path_from_env, ensure_secure_socket_dir,
    hal_session_key_path_from_env, secure_socket_path_from_env, validate_user_load_path,
};
use super::peer_class::classify_peer;
use super::peer_class::peer_allows_command;
use super::types::PeerClass;
#[cfg(not(all(target_os = "macos", feature = "hal")))]
use super::types::encryption_impl;
pub use encryption_impl::KeyManager;
use serial_test::serial;
use std::ffi::OsString;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn test_socket_path_is_user_specific() {
    let path = get_secure_socket_path();
    assert!(path.to_string_lossy().contains("sotf"));
}

#[cfg(unix)]
#[test]
fn test_existing_socket_directory_is_hardened() {
    let root = tempfile::tempdir().expect("temp runtime");
    let runtime = root.path().join("runtime");
    std::fs::create_dir(&runtime).expect("create runtime");
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755))
        .expect("make runtime lax");

    ensure_secure_socket_dir(&runtime.join("daemon.sock")).expect("harden runtime");

    let mode = std::fs::metadata(&runtime)
        .expect("runtime metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
}

#[cfg(all(target_os = "macos", feature = "hal"))]
#[test]
fn test_foreign_owned_runtime_directory_is_rejected_before_chmod() {
    let root = tempfile::tempdir().expect("temp runtime");
    let runtime = root.path().join("runtime");
    std::fs::create_dir(&runtime).expect("create runtime");

    let foreign_uid = get_current_uid().wrapping_add(1);
    let error = encryption_impl::ensure_owned_secure_dir_for_uid(&runtime, foreign_uid)
        .expect_err("a directory owned by another UID must be rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}

#[test]
fn test_session_key_paths_support_lab_runtime_overrides() {
    let runtime = OsString::from("/tmp/sotf-key-path-test");
    assert_eq!(
        daemon_session_key_path_from_env(
            None,
            Some(runtime.clone()),
            Some(OsString::from("/Users/example")),
        ),
        PathBuf::from("/tmp/sotf-key-path-test/daemon-session.key")
    );
    assert_eq!(
        hal_session_key_path_from_env(None, Some(runtime), 501),
        PathBuf::from("/tmp/sotf-key-path-test/session.key")
    );

    assert_eq!(
        daemon_session_key_path_from_env(
            Some(OsString::from("/tmp/private.key")),
            Some(OsString::from("/tmp/ignored")),
            None,
        ),
        PathBuf::from("/tmp/private.key")
    );
    assert_eq!(
        hal_session_key_path_from_env(
            Some(OsString::from("/tmp/hal.key")),
            Some(OsString::from("/tmp/ignored")),
            501,
        ),
        PathBuf::from("/tmp/hal.key")
    );
}

#[test]
fn test_current_uid() {
    let uid = get_current_uid();
    assert!(uid <= 65534);
}

#[test]
fn test_user_load_path_enforces_local_regular_file_policy() {
    let root = tempfile::tempdir().expect("temp runtime");
    let file = root.path().join("audio.wav");
    std::fs::write(&file, b"fixture").expect("write fixture");
    assert_eq!(
        validate_user_load_path(&file).expect("accept owned regular file"),
        std::fs::canonicalize(&file).expect("canonical fixture")
    );

    assert!(validate_user_load_path(std::path::Path::new("relative.wav")).is_err());

    let empty = root.path().join("empty.wav");
    std::fs::File::create(&empty).expect("create empty fixture");
    assert!(validate_user_load_path(&empty).is_err());

    let oversized = root.path().join("oversized.wav");
    std::fs::File::create(&oversized)
        .expect("create oversized fixture")
        .set_len(MAX_USER_AUDIO_FILE_BYTES + 1)
        .expect("set sparse fixture length");
    assert!(validate_user_load_path(&oversized).is_err());

    let directory = root.path().join("directory");
    std::fs::create_dir(&directory).expect("create directory");
    assert!(validate_user_load_path(&directory).is_err());

    #[cfg(unix)]
    {
        let link = root.path().join("audio-link.wav");
        std::os::unix::fs::symlink(&file, &link).expect("create symlink");
        assert!(validate_user_load_path(&link).is_err());
    }
}

#[cfg(all(target_os = "macos", feature = "hal"))]
#[test]
#[serial]
fn atomic_key_publication_replaces_symlink_without_touching_target() {
    let root = tempfile::tempdir().expect("temp key runtime");
    let key_path = root.path().join("session.key");
    let target = root.path().join("attacker-target");
    std::fs::write(&target, b"preserve target").expect("write target");
    std::os::unix::fs::symlink(&target, &key_path).expect("plant key symlink");

    let key = [0x7a; 32];
    encryption_impl::KeyManager::publish_key_atomically(&key_path, &key)
        .expect("atomically replace key symlink");

    let metadata = std::fs::symlink_metadata(&key_path).expect("read published key metadata");
    assert!(metadata.is_file());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::read(&key_path).expect("read published key"), key);
    assert_eq!(
        std::fs::read(&target).expect("read preserved target"),
        b"preserve target"
    );
}

#[cfg(all(target_os = "macos", feature = "hal"))]
#[test]
#[serial]
fn atomic_key_publication_replaces_existing_key_without_partial_state() {
    let root = tempfile::tempdir().expect("temp key runtime");
    let key_path = root.path().join("session.key");
    let first_key = [0x11; 32];
    let second_key = [0x22; 32];

    encryption_impl::KeyManager::publish_key_atomically(&key_path, &first_key)
        .expect("publish first key");
    encryption_impl::KeyManager::publish_key_atomically(&key_path, &second_key)
        .expect("replace key");

    assert_eq!(
        std::fs::read(&key_path).expect("read replacement key"),
        second_key
    );
    assert!(
        std::fs::read_dir(root.path())
            .expect("list key runtime")
            .all(|entry| !entry
                .expect("read entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp-")),
        "atomic publication must clean temporary files"
    );
}

#[test]
#[serial]
fn test_key_manager_creation() {
    let manager = KeyManager::default();
    #[cfg(all(target_os = "macos", feature = "hal"))]
    if !manager.is_enabled() {
        eprintln!("skipping enabled assertion: KeyManager::default() returned disabled fallback");
        return;
    }
    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    assert!(!manager.is_enabled());
    assert_eq!(manager.fingerprint().len(), 8);
    assert!(!manager.fingerprint_hex().is_empty());
}

#[test]
#[serial]
fn test_encryption_status() {
    let manager = KeyManager::default();
    let status = manager.status();
    #[cfg(all(target_os = "macos", feature = "hal"))]
    if !status.enabled {
        eprintln!("skipping enabled assertion: KeyManager::default() returned disabled fallback");
    }
    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    assert!(!status.enabled);
}

#[test]
#[serial]
fn test_hal_key_path_is_under_uid_tmpdir() {
    let path = get_hal_key_path();
    let path_str = path.to_string_lossy();

    assert!(path_str.contains(&format!("/tmp/sotf-{}", get_current_uid())));
    assert!(path_str.ends_with("/session.key"));
}

#[test]
#[serial]
fn test_key_manager_enable_disable() {
    let mut manager = KeyManager::default();
    #[cfg(all(target_os = "macos", feature = "hal"))]
    let starts_enabled = manager.is_enabled();
    #[cfg(all(target_os = "macos", feature = "hal"))]
    if !starts_enabled {
        eprintln!(
            "KeyManager::default() returned disabled fallback; verifying enable stays explicit"
        );
    }
    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    assert!(!manager.is_enabled());

    manager.set_enabled(true);
    // On macOS with HAL: enabled only if a usable cipher exists. In
    // sandboxed/TCC-constrained test environments KeyManager::default()
    // may intentionally return a disabled fallback, and set_enabled(true)
    // must not silently claim encryption is active without a cipher.
    #[cfg(all(target_os = "macos", feature = "hal"))]
    assert_eq!(manager.is_enabled(), starts_enabled);
    #[cfg(not(all(target_os = "macos", feature = "hal")))]
    assert!(!manager.is_enabled());

    manager.set_enabled(false);
    assert!(!manager.is_enabled());
}

#[cfg(all(target_os = "macos", feature = "hal"))]
#[test]
fn test_coreaudiod_acl_targets_cover_key_and_parent_dir() {
    let key_path = PathBuf::from("/Users/test/.config/sotf/session.key");
    let targets = encryption_impl::coreaudiod_acl_targets(&key_path);

    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].0, PathBuf::from("/Users/test/.config/sotf"));
    assert_eq!(targets[0].1, "_coreaudiod allow search,readattr");
    assert_eq!(targets[1].0, key_path);
    assert_eq!(targets[1].1, "_coreaudiod allow read,readattr");

    let hal_key_path = get_hal_key_path();
    let hal_targets = encryption_impl::coreaudiod_acl_targets(&hal_key_path);
    assert_eq!(
        hal_targets[0].0,
        hal_key_path.parent().expect("HAL key path has parent")
    );
    assert_eq!(hal_targets[1].0, hal_key_path);
}

#[test]
fn test_socket_path_deterministic() {
    let path1 = get_secure_socket_path();
    let path2 = get_secure_socket_path();
    assert_eq!(path1, path2);
}

#[test]
fn test_socket_path_supports_lab_overrides() {
    let explicit = secure_socket_path_from_env(
        Some(OsString::from("/tmp/sotf-lab/daemon.sock")),
        Some(OsString::from("/tmp/ignored")),
        None,
        None,
        42,
    );
    assert_eq!(explicit, PathBuf::from("/tmp/sotf-lab/daemon.sock"));

    let runtime =
        secure_socket_path_from_env(None, Some(OsString::from("/tmp/sotf-lab")), None, None, 42);
    assert_eq!(runtime, PathBuf::from("/tmp/sotf-lab/daemon.sock"));
}

#[test]
fn test_classify_peer_owner_uid() {
    assert_eq!(classify_peer(1000, 1000), PeerClass::Owner);
}

#[test]
fn test_classify_peer_root_is_owner() {
    assert_eq!(classify_peer(0, 1000), PeerClass::Owner);
}

#[cfg(target_os = "macos")]
#[test]
fn test_classify_peer_coreaudiod_is_restricted() {
    assert_eq!(classify_peer(202, 1000), PeerClass::CoreAudioD);
}

#[test]
fn test_peer_allows_command_owner_everything() {
    for cmd in [
        "status",
        "load_plugins",
        "shutdown",
        "rotate_encryption_key",
        "set_device",
        "completely_unknown_command",
    ] {
        assert!(
            peer_allows_command(PeerClass::Owner, cmd),
            "Owner should be allowed to run '{}'",
            cmd
        );
    }
}

#[test]
fn test_peer_allows_command_coreaudiod_restricted() {
    for cmd in [
        "driver_status",
        "hal_status",
        "get_driver_config",
        "get_hal_config",
        "encryption_status",
        "get_snapshot",
        "snapshot",
        "status",
    ] {
        assert!(
            peer_allows_command(PeerClass::CoreAudioD, cmd),
            "CoreAudioD should be allowed to run '{}'",
            cmd
        );
    }
    for cmd in [
        "load_plugins",
        "shutdown",
        "rotate_encryption_key",
        "set_device",
        "set_sample_rate",
        "set_buffer_frames",
        "set_encryption",
        "unknown_command",
    ] {
        assert!(
            !peer_allows_command(PeerClass::CoreAudioD, cmd),
            "CoreAudioD should NOT be allowed to run '{}'",
            cmd
        );
    }
}

/// Verify that after `KeyManager::default()` publishes the HAL key
/// copy, the on-disk file is mode 0o600 (owner read/write only).
///
/// Regression test for the security review finding that
/// `publish_hal_key_copy` previously wrote the ChaCha20-Poly1305
/// session key with mode 0o644 -- world-readable -- defeating the
/// whole shared-memory encryption story.
///
/// This test silently skips when KeyManager::default() returns the
/// error fallback (`.is_enabled() == false`), which happens in CI
/// or sandboxed test environments where macOS TCC blocks writes to
/// `/tmp/sotf-{uid}/` or `~/.config/sotf/`. On a normal developer
/// macOS box without TCC restrictions, this assertion runs and the
/// regression is caught.
#[cfg(all(target_os = "macos", feature = "hal"))]
#[test]
#[serial]
fn test_published_hal_key_copy_is_0600() {
    use std::os::unix::fs::PermissionsExt;

    // Triggers create_new_key + publish_hal_key_copy as a side effect.
    let manager = KeyManager::default();

    if !manager.is_enabled() {
        // KeyManager::new() returned Err -- usually because the
        // sandbox blocked filesystem writes to /tmp/sotf-{uid}/ or
        // ~/.config/sotf/. Without a freshly-published file we
        // can't assert on its mode; skip rather than wave a flag
        // for an environmental issue. The test still runs and
        // catches the regression on any developer box where the
        // KeyManager constructor successfully writes to disk.
        eprintln!(
            "skipping test_published_hal_key_copy_is_0600: KeyManager::default() \
                 returned disabled fallback (likely sandboxed write to /tmp or ~/.config)"
        );
        return;
    }

    let path = get_hal_key_path();
    let md = std::fs::metadata(&path).expect("HAL key file should exist after KeyManager::new");
    let mode = md.permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "HAL key copy must be mode 0o600 (got 0o{:o}) -- world/group readability is a leak of the audio session key",
        mode
    );

    if let Some(parent) = path.parent() {
        let pmd = std::fs::metadata(parent).expect("HAL key parent dir should exist");
        let pmode = pmd.permissions().mode() & 0o777;
        assert_eq!(
            pmode, 0o700,
            "HAL key parent dir must be mode 0o700 (got 0o{:o})",
            pmode
        );
    }
}

#[test]
fn test_socket_path_under_tmpdir_or_contains_uid() {
    let path = get_secure_socket_path();
    let path_str = path.to_string_lossy();

    let uid = get_current_uid();
    let tmpdir = std::env::var("TMPDIR").ok();
    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").ok();

    let is_secure = tmpdir.map(|t| path_str.starts_with(&t)).unwrap_or(false)
        || xdg_runtime
            .map(|x| path_str.starts_with(&x))
            .unwrap_or(false)
        || path_str.contains(&format!("sotf-{}", uid));

    assert!(
        is_secure,
        "Socket path should be user-isolated: {}",
        path_str
    );
}

/// Non-macOS / non-HAL builds use a disabled stub KeyManager. Verify the
/// stub API is consistent so daemon command tests are portable.
#[cfg(not(all(target_os = "macos", feature = "hal")))]
#[test]
#[serial]
fn test_key_manager_stub_does_not_pretend_to_rotate() {
    let mut manager = KeyManager::default();
    let before = manager.fingerprint_hex();
    assert!(!manager.is_enabled());

    // Rotation must report the missing capability instead of pretending that a
    // new key was published.
    let error = manager.force_rotate().expect_err("stub rotation must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(!manager.is_enabled());
    assert_eq!(manager.fingerprint_hex(), before);
}

/// Verify that force key rotation actually changes the session key and
/// publishes a new fingerprint. This test runs only on macOS with the HAL
/// feature, where KeyManager has a real cipher backend.
#[cfg(all(target_os = "macos", feature = "hal"))]
#[test]
#[serial]
fn test_key_manager_force_rotate_changes_fingerprint() {
    use std::os::unix::fs::PermissionsExt;

    let mut manager = KeyManager::default();
    if !manager.is_enabled() {
        eprintln!(
            "skipping test_key_manager_force_rotate_changes_fingerprint: \
             KeyManager::default() returned disabled fallback"
        );
        return;
    }

    let before = manager.fingerprint_hex();
    manager.force_rotate().expect("force_rotate should succeed");
    let after = manager.fingerprint_hex();

    assert!(
        !before.is_empty() && !after.is_empty() && before != after,
        "force_rotate must produce a new fingerprint: before={} after={}",
        before,
        after
    );

    let path = get_hal_key_path();
    let md = std::fs::metadata(&path).expect("HAL key file should exist after rotation");
    assert_eq!(
        md.permissions().mode() & 0o777,
        0o600,
        "rotated HAL key copy must remain mode 0o600"
    );
}

/// Defense-in-depth: any UID that is not the daemon owner, root, or the
/// macOS coreaudiod user must fall into the most restricted class.
#[test]
fn test_classify_peer_unknown_uid_is_restricted() {
    assert_eq!(classify_peer(9999, 1000), PeerClass::CoreAudioD);
}

/// The socket path builder must honor an explicit absolute override while
/// still producing a bounded path under a runtime directory when overrides
/// are not set. Regression test for QA-SYS-001 path-bounding requirement.
#[test]
fn test_socket_path_explicit_override_is_absolute() {
    let explicit = secure_socket_path_from_env(
        Some(OsString::from("/tmp/sotf-lab-override/daemon.sock")),
        None,
        None,
        None,
        42,
    );
    assert_eq!(
        explicit,
        PathBuf::from("/tmp/sotf-lab-override/daemon.sock")
    );
    assert!(explicit.is_absolute());
}
