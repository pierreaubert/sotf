use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Validate a user-supplied audio path before handing it to the engine.
///
/// IPC is restricted to trusted peer UIDs, but the daemon still must not
/// silently follow a swapped symlink or accept a directory/device as an
/// audio file. The engine opens the path after this check, so this is a
/// deliberate same-user trust boundary rather than a claim of race-free
/// filesystem sandboxing.
pub(crate) fn validate_user_load_path(path: &Path) -> std::io::Result<()> {
    if path.as_os_str().is_empty() || path.as_os_str().to_string_lossy().contains('\0') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "audio path must be non-empty and contain no NUL bytes",
        ));
    }

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("audio path {} is not a regular file", path.display()),
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if metadata.uid() != super::get::get_current_uid() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("audio path {} is not owned by this user", path.display()),
            ));
        }
    }

    Ok(())
}

pub(super) fn non_empty_path(value: Option<OsString>) -> Option<PathBuf> {
    value
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

pub(super) fn secure_socket_path_from_env(
    socket_override: Option<OsString>,
    runtime_dir: Option<OsString>,
    tmpdir: Option<OsString>,
    xdg_runtime_dir: Option<OsString>,
    uid: u32,
) -> PathBuf {
    if let Some(path) = non_empty_path(socket_override) {
        return path;
    }

    if let Some(path) = non_empty_path(runtime_dir) {
        return path.join("daemon.sock");
    }

    // Try macOS per-user temp directory first
    if let Some(tmpdir) = non_empty_path(tmpdir) {
        return tmpdir.join("sotf-daemon.sock");
    }

    // Try Linux XDG runtime directory
    if let Some(xdg) = non_empty_path(xdg_runtime_dir) {
        return xdg.join("sotf-daemon.sock");
    }

    // Fallback: use UID in path
    PathBuf::from(format!("/tmp/sotf-{}/daemon.sock", uid))
}

pub(super) fn daemon_session_key_path_from_env(
    path_override: Option<OsString>,
    runtime_dir: Option<OsString>,
    home: Option<OsString>,
) -> PathBuf {
    if let Some(path) = non_empty_path(path_override) {
        return path;
    }
    if let Some(path) = non_empty_path(runtime_dir) {
        return path.join("daemon-session.key");
    }

    non_empty_path(home)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/sotf/session.key")
}

pub(super) fn hal_session_key_path_from_env(
    path_override: Option<OsString>,
    runtime_dir: Option<OsString>,
    uid: u32,
) -> PathBuf {
    if let Some(path) = non_empty_path(path_override) {
        return path;
    }
    if let Some(path) = non_empty_path(runtime_dir) {
        return path.join("session.key");
    }

    PathBuf::from(format!("/tmp/sotf-{uid}/session.key"))
}

/// Ensure the socket directory exists with secure permissions
pub fn ensure_secure_socket_dir(socket_path: &Path) -> std::io::Result<()> {
    let Some(parent) = socket_path.parent() else {
        return Ok(());
    };

    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let metadata = std::fs::symlink_metadata(parent)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("socket parent {} is not a directory", parent.display()),
            ));
        }

        let mode = metadata.permissions().mode();
        // The explicitly opted-in legacy socket lives directly in the
        // system sticky directory. `/tmp` is intentionally root-owned and
        // world-searchable; changing it to 0700 would break unrelated users.
        // The socket itself is still protected by lstat/bind hardening and
        // peer-credential authorization.
        let is_shared_sticky_dir = metadata.uid() == 0 && mode & 0o1000 != 0 && mode & 0o002 != 0;
        if is_shared_sticky_dir {
            return Ok(());
        }

        if metadata.uid() != super::get::current_uid() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "socket parent {} is not owned by this user",
                    parent.display()
                ),
            ));
        }

        // Harden pre-existing runtime directories as well as directories we
        // create. This closes the gap where a user-provided runtime directory
        // remained group/world accessible after daemon startup.
        if mode & 0o077 != 0 {
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).map_err(
                |error| {
                    std::io::Error::new(
                        error.kind(),
                        format!(
                            "failed to harden socket parent {}: {error}",
                            parent.display()
                        ),
                    )
                },
            )?;
        }
    }

    Ok(())
}
