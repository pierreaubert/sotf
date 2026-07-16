use sha2::{Digest, Sha256};

/// Size of the authentication tag appended to each encrypted block
pub const AUTH_TAG_SIZE: usize = 16;

/// Convert bytes back to f32 samples (native endian) - allocating version
pub(super) fn bytes_to_samples(bytes: &[u8]) -> Vec<f32> {
    let sample_count = bytes.len() / std::mem::size_of::<f32>();
    let mut samples = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let sample_bytes: [u8; 4] = bytes[i * 4..(i + 1) * 4]
            .try_into()
            .expect("slice should be exactly 4 bytes");
        samples.push(f32::from_le_bytes(sample_bytes));
    }

    samples
}

/// Generate a new random 256-bit encryption key
pub fn generate_key() -> [u8; 32] {
    use rand::TryRng;
    let mut key = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut key)
        .expect("OS RNG must succeed");
    key
}

/// Compute the fingerprint (first 8 bytes of SHA256) for a key
pub fn compute_fingerprint(key: &[u8; 32]) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(key);
    let hash = hasher.finalize();
    let mut fingerprint = [0u8; 8];
    fingerprint.copy_from_slice(&hash[..8]);
    fingerprint
}

/// Format a fingerprint as a hex string
pub fn fingerprint_to_hex(fingerprint: &[u8; 8]) -> String {
    hex::encode(fingerprint)
}

/// Get the path to the session encryption key.
pub fn get_session_key_path() -> std::path::PathBuf {
    let explicit_path = std::env::var_os("SOTF_HAL_SESSION_KEY_PATH");
    let runtime_dir = std::env::var_os("SOTF_SYSTEMWIDE_RUNTIME_DIR");
    let home = std::env::var_os("HOME");

    #[cfg(unix)]
    {
        // SAFETY: getuid() has no preconditions and does not dereference memory.
        let uid = unsafe { libc::getuid() };
        let hal_key_path = std::path::PathBuf::from(format!("/tmp/sotf-{}/session.key", uid));
        session_key_path_from_env(explicit_path, runtime_dir, home, uid, hal_key_path.exists())
    }

    #[cfg(not(unix))]
    session_key_path_from_env(explicit_path, runtime_dir, home, 0, false)
}

pub(crate) fn session_key_path_from_env(
    explicit_path: Option<std::ffi::OsString>,
    runtime_dir: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    uid: u32,
    legacy_hal_key_exists: bool,
) -> std::path::PathBuf {
    let non_empty = |value: Option<std::ffi::OsString>| {
        value
            .map(std::path::PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
    };

    if let Some(path) = non_empty(explicit_path) {
        return path;
    }
    if let Some(path) = non_empty(runtime_dir) {
        return path.join("session.key");
    }
    if legacy_hal_key_exists {
        return std::path::PathBuf::from(format!("/tmp/sotf-{uid}/session.key"));
    }

    non_empty(home)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join(".config/sotf/session.key")
}

/// Load the session encryption key from disk
pub fn load_session_key() -> std::io::Result<[u8; 32]> {
    use std::io::Read;
    let path = get_session_key_path();
    let mut file = std::fs::File::open(path)?;
    let mut key = [0u8; 32];
    file.read_exact(&mut key)?;
    Ok(key)
}
