use std::fs::{File, OpenOptions};
use std::path::Path;

/// Try to acquire an exclusive advisory lock on `sotf.lock` in the config dir.
///
/// Returns the open `File` (must be held for process lifetime) and whether the
/// exclusive lock was obtained. If not, a second instance is already running.
///
/// An unopenable lock file (e.g. read-only `$HOME`) is NOT fatal: it yields
/// `(None, false)` so the caller can degrade to read-only mode instead of
/// crashing startup.
#[cfg(unix)]
pub(super) fn try_acquire_lock(config_dir: &Path) -> (Option<File>, bool) {
    use std::os::unix::io::AsRawFd;

    let lock_path = config_dir.join("sotf.lock");
    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(e) => {
            log::warn!(
                "Could not open lock file {}: {e}; starting in read-only mode",
                lock_path.display()
            );
            return (None, false);
        }
    };

    let exclusive = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 };
    (Some(file), exclusive)
}

#[cfg(windows)]
pub(super) fn try_acquire_lock(config_dir: &Path) -> (Option<File>, bool) {
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LockFileEx(
            hFile: *mut core::ffi::c_void,
            dwFlags: u32,
            dwReserved: u32,
            nNumberOfBytesToLockLow: u32,
            nNumberOfBytesToLockHigh: u32,
            lpOverlapped: *mut Overlapped,
        ) -> i32;
    }

    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x00000002;
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x00000001;

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        h_event: *mut core::ffi::c_void,
    }

    let lock_path = config_dir.join("sotf.lock");
    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(e) => {
            log::warn!(
                "Could not open lock file {}: {e}; starting in read-only mode",
                lock_path.display()
            );
            return (None, false);
        }
    };

    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        h_event: core::ptr::null_mut(),
    };

    // SAFETY: LockFileEx is a well-defined Win32 API. We pass a valid file handle
    // and a zeroed OVERLAPPED struct for a synchronous non-blocking lock attempt.
    let exclusive = unsafe {
        LockFileEx(
            file.as_raw_handle() as *mut core::ffi::c_void,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        ) != 0
    };
    (Some(file), exclusive)
}

#[cfg(not(any(unix, windows)))]
pub(super) fn try_acquire_lock(_config_dir: &Path) -> (Option<File>, bool) {
    match tempfile::tempfile() {
        Ok(file) => (Some(file), true),
        Err(e) => {
            log::warn!("Could not create temp lock file: {e}; starting in read-only mode");
            (None, false)
        }
    }
}

/// Resolve the startup lock state from an optional config dir.
///
/// A missing config dir or an unopenable lock file degrades to `(None,
/// false)` — the caller starts in read-only mode instead of crashing.
pub(super) fn acquire_startup_lock(config_dir: Option<&Path>) -> (Option<File>, bool) {
    match config_dir {
        Some(dir) => try_acquire_lock(dir),
        None => {
            log::warn!("Could not determine config directory — starting in read-only mode");
            (None, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_open_failure_degrades_to_read_only_instead_of_panicking() {
        // A regular file used as the "config dir" makes `<file>/sotf.lock`
        // unopenable, simulating an unavailable lock location.
        let dir = tempfile::tempdir().expect("tempdir");
        let blocker = dir.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").expect("write blocker file");
        let (_lock, acquired) = try_acquire_lock(&blocker);
        assert!(!acquired, "unopenable lock file must mean read-only");
    }

    #[test]
    fn missing_config_dir_degrades_to_read_only() {
        let (_lock, acquired) = acquire_startup_lock(None);
        assert!(!acquired, "missing config dir must mean read-only");
    }

    #[test]
    fn healthy_config_dir_does_not_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Either outcome is a valid lock state; the point is no panic.
        let (_lock, _acquired) = acquire_startup_lock(Some(dir.path()));
    }
}
