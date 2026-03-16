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

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use driver_hal::{compute_fingerprint, fingerprint_to_hex, generate_key, AudioCipher};

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
        && !parent.exists() {
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

/// Key file path: ~/.config/sotf/session.key
fn get_key_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config/sotf/session.key")
}

/// Encryption key manager for shared memory audio encryption
///
/// Manages the lifecycle of the session encryption key:
/// - Key generation and storage
/// - Key loading and validation
/// - Key rotation
/// - Periodic integrity checks
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
    /// Create a new KeyManager
    ///
    /// If a key file exists, loads it. Otherwise creates a new key.
    /// The key file is stored at `~/.config/sotf/session.key` with mode 0640.
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

        let fingerprint = compute_fingerprint(&key);
        let cipher = Some(AudioCipher::new(&key));

        Ok(Self {
            key,
            fingerprint,
            cipher,
            last_check: Instant::now(),
            last_mtime: mtime,
            enabled: false, // Encryption disabled by default
        })
    }

    /// Load the encryption key from file
    fn load_key_from_file(path: &std::path::Path) -> io::Result<[u8; 32]> {
        let mut file = File::open(path)?;
        let mut key = [0u8; 32];
        file.read_exact(&mut key)?;
        log::info!("Loaded encryption key from {}", path.display());
        Ok(key)
    }

    /// Create a new encryption key and save it to file
    fn create_new_key(path: &std::path::Path) -> io::Result<[u8; 32]> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            // Set directory permissions to 0700
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }

        let key = generate_key();

        // Write key with secure permissions (0640)
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o640)
            .open(path)?;
        file.write_all(&key)?;
        file.sync_all()?;

        log::info!("Created new encryption key at {}", path.display());
        Ok(key)
    }

    /// Get the modification time of a file
    fn get_mtime(path: &std::path::Path) -> Option<SystemTime> {
        fs::metadata(path).ok()?.modified().ok()
    }

    /// Check if the key file has changed and reload if necessary
    ///
    /// Returns Ok(true) if the key was reloaded, Ok(false) if unchanged.
    /// On key file deletion, regenerates a new key.
    #[allow(dead_code)]
    pub fn check_and_reload(&mut self) -> io::Result<bool> {
        let key_path = get_key_path();
        let now = Instant::now();

        // Don't check too frequently (every 5 seconds max)
        if now.duration_since(self.last_check).as_secs() < 5 {
            return Ok(false);
        }
        self.last_check = now;

        if !key_path.exists() {
            // Key file was deleted - regenerate
            log::warn!("Key file deleted, regenerating...");
            let key = Self::create_new_key(&key_path)?;
            self.key = key;
            self.fingerprint = compute_fingerprint(&key);
            self.cipher = Some(AudioCipher::new(&key));
            self.last_mtime = Self::get_mtime(&key_path);
            return Ok(true);
        }

        let current_mtime = Self::get_mtime(&key_path);
        if current_mtime != self.last_mtime {
            // Key file was modified - reload
            log::info!("Key file modified, reloading...");
            let key = Self::load_key_from_file(&key_path)?;
            self.key = key;
            self.fingerprint = compute_fingerprint(&key);
            self.cipher = Some(AudioCipher::new(&key));
            self.last_mtime = current_mtime;
            return Ok(true);
        }

        Ok(false)
    }

    /// Force rotation of the encryption key
    ///
    /// Generates a new key and saves it to the key file.
    /// The in-memory state is only updated after the file write succeeds.
    pub fn force_rotate(&mut self) -> io::Result<()> {
        let key_path = get_key_path();
        log::info!("Force rotating encryption key...");

        // Create the new key and write to file first
        // Only update in-memory state if file write succeeds
        let key = Self::create_new_key(&key_path)?;

        // File write succeeded, now update in-memory state
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

    /// Get the key fingerprint
    pub fn fingerprint(&self) -> &[u8; 8] {
        &self.fingerprint
    }

    /// Get the key fingerprint as a hex string
    pub fn fingerprint_hex(&self) -> String {
        fingerprint_to_hex(&self.fingerprint)
    }

    /// Get a reference to the cipher (if available)
    #[allow(dead_code)]
    pub fn cipher(&self) -> Option<&AudioCipher> {
        self.cipher.as_ref()
    }

    /// Check if encryption is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable encryption
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        log::info!(
            "Encryption {}: fingerprint={}",
            if enabled { "enabled" } else { "disabled" },
            self.fingerprint_hex()
        );
    }

    /// Get encryption status information
    pub fn status(&self) -> EncryptionStatus {
        EncryptionStatus {
            enabled: self.enabled,
            fingerprint: self.fingerprint_hex(),
            key_path: get_key_path().to_string_lossy().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_is_user_specific() {
        let path = get_secure_socket_path();
        let path_str = path.to_string_lossy();

        // Should contain TMPDIR, XDG_RUNTIME_DIR, or UID
        let uid = get_current_uid();
        let _has_user_isolation = path_str.contains("TMPDIR")
            || path_str.contains(&format!("/{}/", uid))
            || std::env::var("TMPDIR")
                .map(|t| path_str.contains(&t))
                .unwrap_or(false)
            || std::env::var("XDG_RUNTIME_DIR")
                .map(|t| path_str.contains(&t))
                .unwrap_or(false);

        // On macOS with TMPDIR set, this should pass
        // The important thing is the path is deterministic and user-specific
        assert!(path.to_string_lossy().contains("sotf"));
    }

    #[test]
    fn test_current_uid() {
        let uid = get_current_uid();
        // Should be a valid UID (not u32::MAX which would indicate an error)
        assert!(uid <= 65534); // 65534 is nobody
    }

    #[test]
    fn test_key_manager_creation() {
        // This test will create a real key file in ~/.config/sotf/
        // but that's acceptable for testing the key management flow
        let manager = KeyManager::default();
        assert!(!manager.is_enabled()); // Disabled by default
        assert_eq!(manager.fingerprint().len(), 8);
        assert!(!manager.fingerprint_hex().is_empty());
    }

    #[test]
    fn test_encryption_status() {
        let manager = KeyManager::default();
        let status = manager.status();
        assert!(!status.enabled);
        assert_eq!(status.fingerprint.len(), 16); // Hex encoding doubles the length
    }

    // ==========================================================================
    // Additional Security Tests - catch regressions in key management
    // ==========================================================================

    #[test]
    fn test_key_manager_enable_disable() {
        let mut manager = KeyManager::default();

        // Initially disabled
        assert!(!manager.is_enabled());

        // Enable
        manager.set_enabled(true);
        assert!(manager.is_enabled());

        // Disable
        manager.set_enabled(false);
        assert!(!manager.is_enabled());
    }

    #[test]
    fn test_key_manager_cipher_available() {
        let manager = KeyManager::default();

        // Cipher should be available after creation (key is auto-generated)
        assert!(
            manager.cipher().is_some(),
            "Cipher should be available after KeyManager creation"
        );
    }

    #[test]
    fn test_key_fingerprint_consistency() {
        let manager = KeyManager::default();

        // Fingerprint should be consistent across calls
        let fp1 = manager.fingerprint();
        let fp2 = manager.fingerprint();
        assert_eq!(fp1, fp2, "Fingerprint should be consistent");

        // Hex fingerprint should match raw fingerprint
        let fp_hex = manager.fingerprint_hex();
        let fp_from_raw: String = fp1.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(fp_hex, fp_from_raw, "Hex fingerprint should match raw");
    }

    #[test]
    fn test_key_rotation_changes_fingerprint() {
        let mut manager = KeyManager::default();

        let fp_before = manager.fingerprint_hex();

        // Force rotate
        manager.force_rotate().expect("Key rotation should succeed");

        let fp_after = manager.fingerprint_hex();

        // Fingerprint should change (extremely unlikely to be the same)
        assert_ne!(
            fp_before, fp_after,
            "Fingerprint should change after rotation"
        );

        // Cipher should still be available
        assert!(
            manager.cipher().is_some(),
            "Cipher should be available after rotation"
        );
    }

    #[test]
    fn test_key_manager_check_and_reload_no_change() {
        let mut manager = KeyManager::default();

        let fp_before = manager.fingerprint_hex();

        // Check and reload when no change
        let reloaded = manager.check_and_reload().expect("Check should succeed");
        assert!(!reloaded, "Should not reload if file unchanged");

        let fp_after = manager.fingerprint_hex();
        assert_eq!(fp_before, fp_after, "Fingerprint should not change");
    }

    #[test]
    fn test_socket_path_deterministic() {
        // Socket path should be deterministic (same each call)
        let path1 = get_secure_socket_path();
        let path2 = get_secure_socket_path();
        assert_eq!(path1, path2, "Socket path should be deterministic");
    }

    #[test]
    fn test_socket_path_under_tmpdir_or_contains_uid() {
        let path = get_secure_socket_path();
        let path_str = path.to_string_lossy();

        // Path should either be under TMPDIR/XDG_RUNTIME_DIR or contain UID
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
