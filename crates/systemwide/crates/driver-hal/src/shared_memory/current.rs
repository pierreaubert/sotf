use super::misc::non_empty_path;
use std::path::Path;

pub(super) fn current_unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(unix)]
pub(super) fn current_uid() -> u32 {
    // SAFETY: `libc::getuid` has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

#[cfg(unix)]
pub(super) fn protected_shared_memory_parent() -> std::path::PathBuf {
    non_empty_path(std::env::var_os("SOTF_SYSTEMWIDE_RUNTIME_DIR"))
        .unwrap_or_else(|| std::path::PathBuf::from(format!("/tmp/sotf-{}", current_uid())))
}

#[cfg(unix)]
pub(super) fn is_protected_shared_memory_parent(path: &Path) -> bool {
    path == protected_shared_memory_parent()
}
