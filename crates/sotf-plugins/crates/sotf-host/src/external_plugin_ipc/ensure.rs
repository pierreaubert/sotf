#[cfg(unix)]
use super::clamp::clamp_dir_permissions;
#[cfg(windows)]
use super::clamp::clamp_dir_permissions;
#[cfg(unix)]
use super::current::current_user_tag;
#[cfg(windows)]
use super::current::current_user_tag;
#[cfg(windows)]
use super::current::set_windows_owner_only_dacl;
#[cfg(windows)]
use super::validate::validate_windows_owner_only_dacl;
use std::io;
use std::path::{Path, PathBuf};

pub(super) fn create_session_dir() -> io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!("sotf-plugin-ipc-{}", current_user_tag()));
    ensure_secure_parent_dir(&root)?;

    for _ in 0..128 {
        let token: u128 = rand::random();
        let session_dir = root.join(format!("session-{}-{token:032x}", std::process::id()));
        match std::fs::create_dir(&session_dir) {
            Ok(()) => {
                clamp_dir_permissions(&session_dir)?;
                return Ok(session_dir);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate unique external-plugin IPC session directory",
    ))
}

#[cfg(unix)]
pub(super) fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }

    let metadata = std::fs::symlink_metadata(parent)?;
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a directory", parent.display()),
        ));
    }
    if metadata.uid() != unsafe { libc::getuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not owned by the current user", parent.display()),
        ));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }
    if !std::fs::metadata(parent)?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a directory", parent.display()),
        ));
    }
    set_windows_owner_only_dacl(parent)?;
    validate_windows_owner_only_dacl(parent)?;
    Ok(())
}
