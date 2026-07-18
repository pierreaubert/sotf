use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
