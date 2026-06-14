#[cfg(unix)]
use super::current::current_uid;
#[cfg(unix)]
use super::current::is_protected_shared_memory_parent;
#[cfg(not(target_os = "macos"))]
use super::grant::grant_coreaudiod_shared_memory_access;
#[cfg(target_os = "macos")]
use super::grant::grant_coreaudiod_shared_memory_access;
use std::io;
use std::path::Path;

#[cfg(unix)]
pub(super) fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }

    if is_protected_shared_memory_parent(parent) {
        let metadata = std::fs::symlink_metadata(parent)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a directory", parent.display()),
            ));
        }
        if metadata.uid() != current_uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{} is owned by uid {}, expected {}",
                    parent.display(),
                    metadata.uid(),
                    current_uid()
                ),
            ));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        grant_coreaudiod_shared_memory_access(parent);
    }

    Ok(())
}

#[cfg(not(unix))]
pub(super) fn ensure_secure_parent_dir(parent: &Path) -> io::Result<()> {
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
