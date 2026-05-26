//! Security module for sotf_daemon
//!
//! Provides authentication and authorization for IPC communications,
//! and encryption key management for shared memory audio data.
//!
//! Security model:
//! - Each user runs their own daemon instance
//! - Socket is placed in user-private directory ($TMPDIR or /tmp/sotf-$UID/)
//! - Peer credentials are verified on connection
//! - Only same-user or root can connect
//! - Optional encryption of audio data in shared memory via ChaCha20-Poly1305

use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

/// Get the secure socket path for this user
///
/// On macOS, $TMPDIR is per-user and already secured.
/// On Linux, $XDG_RUNTIME_DIR provides similar isolation.
/// Fallback uses UID in the path.
pub fn get_secure_socket_path() -> PathBuf {
    // Try macOS per-user temp directory first
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        return PathBuf::from(tmpdir).join("sotf-daemon.sock");
    }

    // Try Linux XDG runtime directory
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(xdg).join("sotf-daemon.sock");
    }

    // Fallback: use UID in path
    let uid = get_current_uid();
    PathBuf::from(format!("/tmp/sotf-{}/daemon.sock", uid))
}

/// Get current user ID
fn get_current_uid() -> u32 {
    #[cfg(unix)]
    {
        // SAFETY: getuid() is always safe to call
        unsafe { libc::getuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Verify that the connecting peer is authorized
///
/// Returns Ok(uid) if authorized, Err with reason if not.
///
/// Authorization rules:
/// - Same UID as daemon: allowed
/// - Root (UID 0): allowed
/// - _coreaudiod (UID 202 on macOS): allowed for HAL communication
/// - Others: denied
#[cfg(unix)]
pub fn verify_peer_credentials(stream: &UnixStream) -> Result<u32, String> {
    use std::os::unix::io::AsRawFd;

    let fd = stream.as_raw_fd();
    let peer_uid = get_peer_uid(fd)?;
    let my_uid = get_current_uid();

    // Allow same user
    if peer_uid == my_uid {
        return Ok(peer_uid);
    }

    // Allow root
    if peer_uid == 0 {
        log::debug!("Allowing root connection");
        return Ok(peer_uid);
    }

    // Allow _coreaudiod (UID 202 on macOS) for HAL driver communication
    #[cfg(target_os = "macos")]
    if peer_uid == 202 {
        log::debug!("Allowing _coreaudiod connection");
        return Ok(peer_uid);
    }

    Err(format!(
        "Unauthorized connection: peer UID {} (expected {} or 0)",
        peer_uid, my_uid
    ))
}

/// Get peer UID from socket file descriptor
#[cfg(target_os = "macos")]
fn get_peer_uid(fd: std::os::unix::io::RawFd) -> Result<u32, String> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;

    // SAFETY: getpeereid is safe when passed a valid socket fd
    let result = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };

    if result != 0 {
        return Err(format!(
            "getpeereid failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    Ok(uid)
}

#[cfg(target_os = "linux")]
fn get_peer_uid(fd: std::os::unix::io::RawFd) -> Result<u32, String> {
    use std::mem::MaybeUninit;

    let mut cred = MaybeUninit::<libc::ucred>::uninit();
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

    // SAFETY: getsockopt is safe with correct parameters
    let result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            cred.as_mut_ptr() as *mut libc::c_void,
            &mut len,
        )
    };

    if result != 0 {
        return Err(format!(
            "getsockopt SO_PEERCRED failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    // SAFETY: getsockopt succeeded, cred is initialized
    let cred = unsafe { cred.assume_init() };
    Ok(cred.uid)
}

#[cfg(not(unix))]
pub fn verify_peer_credentials(_stream: &UnixStream) -> Result<u32, String> {
    // Non-Unix platforms: no credential verification available
    Ok(0)
}

/// Ensure the socket directory exists with secure permissions
pub fn ensure_secure_socket_dir(socket_path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = socket_path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;

        // Set directory permissions to 0700 (owner only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

// =============================================================================
// Encryption Key Management
// =============================================================================

/// Encryption status information
#[derive(Debug, Clone)]
pub struct EncryptionStatus {
    /// Whether encryption is currently enabled
    pub enabled: bool,
    /// Key fingerprint as hex string
    pub fingerprint: String,
    /// Path to the key file
    pub key_path: String,
}

/// Key file path: ~/.config/sotf/session.key
#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
fn get_key_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config/sotf/session.key")
}

/// HAL-readable copy of the session key.
pub(crate) fn get_hal_key_path() -> PathBuf {
    PathBuf::from(format!("/tmp/sotf-{}/session.key", get_current_uid()))
}

/// Per-UID command authorization classes.
///
/// Most callers run as the daemon's own UID (full access). The macOS HAL
/// runs inside `coreaudiod` (UID 202) and only needs status / config /
/// encryption-key visibility -- it must NOT be able to load arbitrary
/// plugin chains, change devices, or shut the daemon down. Root is given
/// the same broad access as the owning UID since on macOS that is the
/// administrator running the daemon manually for debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerClass {
    /// Same-UID caller (or root) -- all commands allowed.
    Owner,
    /// macOS `_coreaudiod` (UID 202) -- restricted HAL-protocol commands only.
    CoreAudioD,
}

/// Classify an authenticated peer UID into a permission class.
///
/// The `daemon_uid` argument is the UID the daemon itself runs as.
pub fn classify_peer(peer_uid: u32, daemon_uid: u32) -> PeerClass {
    if peer_uid == daemon_uid || peer_uid == 0 {
        return PeerClass::Owner;
    }
    #[cfg(target_os = "macos")]
    if peer_uid == 202 {
        return PeerClass::CoreAudioD;
    }
    // verify_peer_credentials() would have rejected the connection long
    // before we reach this point for any other UID. Treat as CoreAudioD
    // (the most restricted class) as a defense-in-depth fallback.
    let _ = peer_uid;
    PeerClass::CoreAudioD
}

/// Return the current daemon UID. Pub wrapper around the internal helper
/// so the binary entrypoint can build a `classify_peer` argument without
/// duplicating the libc call.
pub fn current_uid() -> u32 {
    get_current_uid()
}

/// Whether a peer of the given class may invoke the named command.
///
/// `command_name` matches the `#[serde(rename = "...")]` tag from the
/// `Command` enum in `sotf_daemon.rs`. Unknown commands are rejected by
/// default for non-Owner classes (deny-by-default).
pub fn peer_allows_command(class: PeerClass, command_name: &str) -> bool {
    match class {
        PeerClass::Owner => true,
        PeerClass::CoreAudioD => matches!(
            command_name,
            // HAL needs to query driver / encryption state so it can
            // attach the shared-memory cipher. Everything else (loading
            // plugins, choosing devices, shutting the daemon down) is
            // out of scope for the audio driver process.
            "driver_status"
                | "hal_status"
                | "get_driver_config"
                | "get_hal_config"
                | "encryption_status"
                | "status"
        ),
    }
}

// =============================================================================
// Full encryption support (macOS with HAL feature)
// =============================================================================

#[cfg(all(target_os = "macos", feature = "hal"))]
mod encryption_impl {
    use super::*;
    use driver_hal::{AudioCipher, compute_fingerprint, fingerprint_to_hex, generate_key};
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Instant, SystemTime};

    #[cfg(target_os = "macos")]
    pub(super) fn coreaudiod_acl_targets(key_path: &Path) -> Vec<(PathBuf, &'static str)> {
        let mut targets = Vec::new();
        if let Some(parent) = key_path.parent() {
            targets.push((parent.to_path_buf(), "_coreaudiod allow search,readattr"));
        }
        targets.push((key_path.to_path_buf(), "_coreaudiod allow read,readattr"));
        targets
    }

    #[cfg(target_os = "macos")]
    fn grant_coreaudiod_key_access(key_path: &Path) {
        for (path, acl) in coreaudiod_acl_targets(key_path) {
            match Command::new("/bin/chmod")
                .arg("+a")
                .arg(acl)
                .arg(&path)
                .status()
            {
                Ok(status) if status.success() => {
                    log::debug!("Granted _coreaudiod key access on {}", path.display());
                }
                Ok(status) => {
                    log::warn!(
                        "Failed to grant _coreaudiod key access on {}: chmod exited with {}",
                        path.display(),
                        status
                    );
                }
                Err(e) => {
                    log::warn!(
                        "Failed to grant _coreaudiod key access on {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn grant_coreaudiod_key_access(_key_path: &Path) {}

    /// Encryption key manager for shared memory audio encryption
    #[allow(dead_code)]
    pub struct KeyManager {
        key: [u8; 32],
        fingerprint: [u8; 8],
        cipher: Option<AudioCipher>,
        last_check: Instant,
        last_mtime: Option<SystemTime>,
        enabled: bool,
    }

    impl KeyManager {
        pub fn new() -> io::Result<Self> {
            let key_path = get_key_path();
            let (key, mtime) = if key_path.exists() {
                (
                    Self::load_key_from_file(&key_path)?,
                    Self::get_mtime(&key_path),
                )
            } else {
                let key = Self::create_new_key(&key_path)?;
                (key, Self::get_mtime(&key_path))
            };
            grant_coreaudiod_key_access(&key_path);
            Self::publish_hal_key_copy(&key)?;

            let fingerprint = compute_fingerprint(&key);
            let cipher = Some(AudioCipher::new(&key));

            Ok(Self {
                key,
                fingerprint,
                cipher,
                last_check: Instant::now(),
                last_mtime: mtime,
                enabled: true,
            })
        }

        fn load_key_from_file(path: &std::path::Path) -> io::Result<[u8; 32]> {
            let mut file = File::open(path)?;
            let mut key = [0u8; 32];
            file.read_exact(&mut key)?;
            log::info!("Loaded encryption key from {}", path.display());
            Ok(key)
        }

        fn create_new_key(path: &std::path::Path) -> io::Result<[u8; 32]> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            }

            let key = generate_key();

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(path)?;
            file.write_all(&key)?;
            file.sync_all()?;

            log::info!("Created new encryption key at {}", path.display());
            Ok(key)
        }

        fn publish_hal_key_copy(key: &[u8; 32]) -> io::Result<()> {
            let path = get_hal_key_path();
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
                // Always clamp the parent dir to 0o700 (owner only). Other
                // users must not be able to enumerate the directory; the
                // _coreaudiod (UID 202) process is granted access by the
                // explicit ACL applied via grant_coreaudiod_key_access().
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            }

            // Create with mode 0o600 (owner read/write only). The
            // ChaCha20-Poly1305 audio session key must never be
            // world-readable. _coreaudiod (UID 202) reads it via the
            // macOS `chmod +a` ACL applied separately by
            // grant_coreaudiod_key_access(). Remove any pre-existing
            // file first so we never inherit looser permissions from a
            // prior daemon run that wrote 0o644.
            let _ = fs::remove_file(&path);

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(key)?;
            file.sync_all()?;
            // Re-assert the mode in case the open call honored a
            // permissive umask. This is the canonical post-write
            // permission.
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            grant_coreaudiod_key_access(&path);

            log::info!(
                "Published HAL-readable encryption key at {} (mode 0600 + _coreaudiod ACL)",
                path.display()
            );
            Ok(())
        }

        fn get_mtime(path: &std::path::Path) -> Option<SystemTime> {
            fs::metadata(path).ok()?.modified().ok()
        }

        #[allow(dead_code)]
        pub fn check_and_reload(&mut self) -> io::Result<bool> {
            let key_path = get_key_path();
            let now = Instant::now();

            if now.duration_since(self.last_check).as_secs() < 5 {
                return Ok(false);
            }
            self.last_check = now;

            if !key_path.exists() {
                log::warn!("Key file deleted, regenerating...");
                let key = Self::create_new_key(&key_path)?;
                grant_coreaudiod_key_access(&key_path);
                Self::publish_hal_key_copy(&key)?;
                self.key = key;
                self.fingerprint = compute_fingerprint(&key);
                self.cipher = Some(AudioCipher::new(&key));
                self.last_mtime = Self::get_mtime(&key_path);
                return Ok(true);
            }

            let current_mtime = Self::get_mtime(&key_path);
            if current_mtime != self.last_mtime {
                log::info!("Key file modified, reloading...");
                let key = Self::load_key_from_file(&key_path)?;
                grant_coreaudiod_key_access(&key_path);
                Self::publish_hal_key_copy(&key)?;
                self.key = key;
                self.fingerprint = compute_fingerprint(&key);
                self.cipher = Some(AudioCipher::new(&key));
                self.last_mtime = current_mtime;
                return Ok(true);
            }

            Ok(false)
        }

        pub fn force_rotate(&mut self) -> io::Result<()> {
            let key_path = get_key_path();
            log::info!("Force rotating encryption key...");

            let key = Self::create_new_key(&key_path)?;
            grant_coreaudiod_key_access(&key_path);
            Self::publish_hal_key_copy(&key)?;

            let fingerprint = compute_fingerprint(&key);
            let cipher = AudioCipher::new(&key);
            let mtime = Self::get_mtime(&key_path);

            self.key = key;
            self.fingerprint = fingerprint;
            self.cipher = Some(cipher);
            self.last_mtime = mtime;

            log::info!(
                "Encryption key rotated successfully, fingerprint: {}",
                self.fingerprint_hex()
            );
            Ok(())
        }

        pub fn fingerprint(&self) -> &[u8; 8] {
            &self.fingerprint
        }

        pub fn fingerprint_hex(&self) -> String {
            fingerprint_to_hex(&self.fingerprint)
        }

        #[allow(dead_code)]
        pub fn cipher(&self) -> Option<&AudioCipher> {
            self.cipher.as_ref()
        }

        pub fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub fn set_enabled(&mut self, enabled: bool) {
            self.enabled = enabled;
            log::info!(
                "Encryption {}: fingerprint={}",
                if enabled { "enabled" } else { "disabled" },
                self.fingerprint_hex()
            );
        }

        pub fn status(&self) -> EncryptionStatus {
            EncryptionStatus {
                enabled: self.enabled,
                fingerprint: self.fingerprint_hex(),
                key_path: get_hal_key_path().to_string_lossy().to_string(),
            }
        }
    }

    impl Default for KeyManager {
        fn default() -> Self {
            Self::new().unwrap_or_else(|e| {
                log::error!("Failed to create KeyManager: {}", e);
                Self {
                    key: [0u8; 32],
                    fingerprint: [0u8; 8],
                    cipher: None,
                    last_check: Instant::now(),
                    last_mtime: None,
                    enabled: false,
                }
            })
        }
    }
}

// =============================================================================
// Stub encryption (non-macOS or no HAL feature)
// =============================================================================

#[cfg(not(all(target_os = "macos", feature = "hal")))]
mod encryption_impl {
    use super::*;

    /// Stub key manager when encryption is not available.
    ///
    /// Encryption requires shared memory (macOS HAL only). On other platforms,
    /// this stub provides the same API but encryption is always disabled.
    pub struct KeyManager {
        enabled: bool,
    }

    impl KeyManager {
        pub fn new() -> std::io::Result<Self> {
            Ok(Self { enabled: false })
        }

        pub fn force_rotate(&mut self) -> std::io::Result<()> {
            log::warn!("Encryption not available on this platform");
            Ok(())
        }

        #[allow(dead_code)]
        pub fn fingerprint(&self) -> &[u8; 8] {
            &[0u8; 8]
        }

        pub fn fingerprint_hex(&self) -> String {
            "0000000000000000".to_string()
        }

        pub fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub fn set_enabled(&mut self, enabled: bool) {
            if enabled {
                log::warn!("Encryption not available on this platform, ignoring enable request");
            }
            self.enabled = false;
        }

        pub fn status(&self) -> EncryptionStatus {
            EncryptionStatus {
                enabled: false,
                fingerprint: self.fingerprint_hex(),
                key_path: get_hal_key_path().to_string_lossy().to_string(),
            }
        }
    }

    impl Default for KeyManager {
        fn default() -> Self {
            Self::new().unwrap_or(Self { enabled: false })
        }
    }
}

pub use encryption_impl::KeyManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_is_user_specific() {
        let path = get_secure_socket_path();
        assert!(path.to_string_lossy().contains("sotf"));
    }

    #[test]
    fn test_current_uid() {
        let uid = get_current_uid();
        assert!(uid <= 65534);
    }

    #[test]
    fn test_key_manager_creation() {
        let manager = KeyManager::default();
        #[cfg(all(target_os = "macos", feature = "hal"))]
        assert!(manager.is_enabled());
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        assert!(!manager.is_enabled());
        assert_eq!(manager.fingerprint().len(), 8);
        assert!(!manager.fingerprint_hex().is_empty());
    }

    #[test]
    fn test_encryption_status() {
        let manager = KeyManager::default();
        let status = manager.status();
        #[cfg(all(target_os = "macos", feature = "hal"))]
        assert!(status.enabled);
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        assert!(!status.enabled);
    }

    #[test]
    fn test_hal_key_path_is_under_uid_tmpdir() {
        let path = get_hal_key_path();
        let path_str = path.to_string_lossy();

        assert!(path_str.contains(&format!("/tmp/sotf-{}", get_current_uid())));
        assert!(path_str.ends_with("/session.key"));
    }

    #[test]
    fn test_key_manager_enable_disable() {
        let mut manager = KeyManager::default();
        #[cfg(all(target_os = "macos", feature = "hal"))]
        assert!(manager.is_enabled());
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        assert!(!manager.is_enabled());

        manager.set_enabled(true);
        // On macOS with hal: enabled. On other platforms: stays false.
        #[cfg(all(target_os = "macos", feature = "hal"))]
        assert!(manager.is_enabled());
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
}
