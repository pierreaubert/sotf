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

#[cfg(not(all(target_os = "macos", feature = "hal")))]
pub(super) mod encryption_impl {
    use super::super::get::get_hal_key_path;
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
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "encryption key rotation requires the macOS HAL-enabled daemon build",
            ))
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
