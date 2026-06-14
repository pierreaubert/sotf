use std::path::Path;

#[cfg(target_os = "macos")]
pub(super) fn grant_coreaudiod_shared_memory_access(path: &Path) {
    let acl = if path.is_dir() {
        "_coreaudiod allow search,readattr"
    } else {
        "_coreaudiod allow read,write,readattr,writeattr"
    };

    match std::process::Command::new("/bin/chmod")
        .arg("+a")
        .arg(acl)
        .arg(path)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => {
            log::warn!(
                "Failed to grant _coreaudiod shared-memory access on {}: chmod exited with {}",
                path.display(),
                status
            );
        }
        Err(e) => {
            log::warn!(
                "Failed to grant _coreaudiod shared-memory access on {}: {}",
                path.display(),
                e
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn grant_coreaudiod_shared_memory_access(_path: &Path) {}
