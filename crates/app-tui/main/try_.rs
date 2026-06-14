use std::fs::{File, OpenOptions};
use std::path::Path;

/// Try to acquire an exclusive advisory lock on `sotf.lock` in the config dir.
/// Returns the open `File` (must be held for process lifetime) and whether the
/// exclusive lock was obtained. If not, a second instance is already running.
#[cfg(unix)]
pub(super) fn try_acquire_lock(config_dir: &Path) -> (File, bool) {
    use std::os::unix::io::AsRawFd;

    let lock_path = config_dir.join("sotf.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("Failed to open lock file");

    let exclusive = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 };
    (file, exclusive)
}

#[cfg(windows)]
pub(super) fn try_acquire_lock(config_dir: &Path) -> (File, bool) {
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
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("Failed to open lock file");

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
    (file, exclusive)
}

#[cfg(not(any(unix, windows)))]
pub(super) fn try_acquire_lock(_config_dir: &Path) -> (File, bool) {
    let file = tempfile::tempfile().expect("Failed to create temp lock file");
    (file, true)
}
