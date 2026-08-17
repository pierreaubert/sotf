use std::ffi::OsString;

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

pub(super) fn non_empty_path(value: Option<OsString>) -> Option<std::path::PathBuf> {
    value
        .map(std::path::PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

pub(super) fn shared_memory_path_from_env(
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

pub(super) fn fingerprint_to_u64(fingerprint: [u8; 8]) -> u64 {
    u64::from_be_bytes(fingerprint)
}

pub(super) fn u64_to_fingerprint(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

/// Compare public key fingerprints without an early-exit byte comparison.
/// The fingerprint is not secret, but using one helper keeps all transport
/// key-state checks uniform and avoids making the comparison timing-dependent
/// if the protocol is reused for a less-public token later.
pub(super) fn fingerprints_equal(left: &[u8; 8], right: &[u8; 8]) -> bool {
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}
