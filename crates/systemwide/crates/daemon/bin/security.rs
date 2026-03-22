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
fn get_key_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config/sotf/session.key")
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
    use std::time::{Instant, SystemTime};

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

            let fingerprint = compute_fingerprint(&key);
            let cipher = Some(AudioCipher::new(&key));

            Ok(Self {
                key,
                fingerprint,
                cipher,
                last_check: Instant::now(),
                last_mtime: mtime,
                enabled: false,
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
                .mode(0o640)
                .open(path)?;
            file.write_all(&key)?;
            file.sync_all()?;

            log::info!("Created new encryption key at {}", path.display());
            Ok(key)
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
                key_path: get_key_path().to_string_lossy().to_string(),
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
        assert!(!manager.is_enabled());
        assert_eq!(manager.fingerprint().len(), 8);
        assert!(!manager.fingerprint_hex().is_empty());
    }

    #[test]
    fn test_encryption_status() {
        let manager = KeyManager::default();
        let status = manager.status();
        assert!(!status.enabled);
    }

    #[test]
    fn test_key_manager_enable_disable() {
        let mut manager = KeyManager::default();
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

    #[test]
    fn test_socket_path_deterministic() {
        let path1 = get_secure_socket_path();
        let path2 = get_secure_socket_path();
        assert_eq!(path1, path2);
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
