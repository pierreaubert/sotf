use super::misc::{
    daemon_session_key_path_from_env, hal_session_key_path_from_env, secure_socket_path_from_env,
};
use std::path::PathBuf;

/// Get the secure socket path for this user
///
/// On macOS, $TMPDIR is per-user and already secured.
/// On Linux, $XDG_RUNTIME_DIR provides similar isolation.
/// Fallback uses UID in the path.
pub fn get_secure_socket_path() -> PathBuf {
    secure_socket_path_from_env(
        std::env::var_os("SOTF_DAEMON_SOCKET_PATH"),
        std::env::var_os("SOTF_SYSTEMWIDE_RUNTIME_DIR"),
        std::env::var_os("TMPDIR"),
        std::env::var_os("XDG_RUNTIME_DIR"),
        get_current_uid(),
    )
}

/// Get current user ID
pub(super) fn get_current_uid() -> u32 {
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

/// Get peer UID from socket file descriptor
#[cfg(target_os = "macos")]
pub(super) fn get_peer_uid(fd: std::os::unix::io::RawFd) -> Result<u32, String> {
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
pub(super) fn get_peer_uid(fd: std::os::unix::io::RawFd) -> Result<u32, String> {
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

/// Daemon-private key path. Lab runtime overrides keep this out of the real
/// user configuration directory.
#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
pub(super) fn get_key_path() -> PathBuf {
    daemon_session_key_path_from_env(
        std::env::var_os("SOTF_DAEMON_SESSION_KEY_PATH"),
        std::env::var_os("SOTF_SYSTEMWIDE_RUNTIME_DIR"),
        std::env::var_os("HOME"),
    )
}

/// HAL-readable copy of the session key.
pub(crate) fn get_hal_key_path() -> PathBuf {
    hal_session_key_path_from_env(
        std::env::var_os("SOTF_HAL_SESSION_KEY_PATH"),
        std::env::var_os("SOTF_SYSTEMWIDE_RUNTIME_DIR"),
        get_current_uid(),
    )
}

/// Return the current daemon UID. Pub wrapper around the internal helper
/// so the binary entrypoint can build a `classify_peer` argument without
/// duplicating the libc call.
pub fn current_uid() -> u32 {
    get_current_uid()
}

#[cfg(all(target_os = "macos", feature = "hal"))]
pub(super) mod encryption_impl {
    use super::super::*;
    use super::*;
    use driver_hal::{AudioCipher, compute_fingerprint, fingerprint_to_hex, try_generate_key};
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn ensure_owned_secure_dir(path: &Path) -> io::Result<()> {
        if path.exists() {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("runtime path {} is not a directory", path.display()),
                ));
            }
            if metadata.uid() != get_current_uid() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "runtime directory {} is owned by another user",
                        path.display()
                    ),
                ));
            }
        } else {
            fs::create_dir_all(path)?;
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub(in super::super) fn coreaudiod_acl_targets(
        key_path: &Path,
    ) -> Vec<(PathBuf, &'static str)> {
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
        enabled: bool,
    }

    impl KeyManager {
        #[cfg(test)]
        pub(crate) fn for_test() -> Self {
            let key = [0x53; 32];
            Self {
                key,
                fingerprint: compute_fingerprint(&key),
                cipher: Some(AudioCipher::new(&key)),
                enabled: true,
            }
        }

        pub fn new() -> io::Result<Self> {
            let key_path = get_key_path();
            let key = if key_path.exists() {
                Self::load_key_from_file(&key_path)?
            } else {
                Self::create_new_key(&key_path)?
            };
            grant_coreaudiod_key_access(&key_path);
            Self::publish_hal_key_copy(&key)?;

            let fingerprint = compute_fingerprint(&key);
            let cipher = Some(AudioCipher::new(&key));

            Ok(Self {
                key,
                fingerprint,
                cipher,
                enabled: true,
            })
        }

        fn load_key_from_file(path: &std::path::Path) -> io::Result<[u8; 32]> {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || metadata.uid() != get_current_uid() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "encryption key {} is not owned by this user",
                        path.display()
                    ),
                ));
            }
            let mut file = File::open(path)?;
            let mut key = [0u8; 32];
            file.read_exact(&mut key)?;
            log::info!("Loaded encryption key from {}", path.display());
            Ok(key)
        }

        fn create_new_key(path: &std::path::Path) -> io::Result<[u8; 32]> {
            if let Some(parent) = path.parent() {
                ensure_owned_secure_dir(parent)?;
            }

            let key = try_generate_key()?;

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
                // Reject a pre-created foreign-owned runtime directory before
                // touching permissions or key material. Otherwise another
                // local user can squat the first-run path and force the
                // daemon into its disabled-key fallback.
                ensure_owned_secure_dir(parent)?;
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

        pub fn force_rotate(&mut self) -> io::Result<()> {
            let key_path = get_key_path();
            log::info!("Force rotating encryption key...");

            let key = Self::create_new_key(&key_path)?;
            grant_coreaudiod_key_access(&key_path);
            Self::publish_hal_key_copy(&key)?;

            let fingerprint = compute_fingerprint(&key);
            let cipher = AudioCipher::new(&key);
            self.key = key;
            self.fingerprint = fingerprint;
            self.cipher = Some(cipher);

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

        pub fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub fn set_enabled(&mut self, enabled: bool) {
            if enabled && self.cipher.is_none() {
                self.enabled = false;
                log::warn!("Encryption cannot be enabled because no session cipher is available");
                return;
            }
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
                    enabled: false,
                }
            })
        }
    }
}
