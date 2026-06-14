#[cfg(windows)]
use super::current::current_windows_user_sid;
#[cfg(windows)]
use super::misc::win32_error;
#[cfg(windows)]
use super::misc::windows_path_wide;
use std::fs::File;
use std::io;
use std::path::Path;

#[cfg(unix)]
pub(crate) fn validate_shared_memory_file(file: &File, path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let link_metadata = std::fs::symlink_metadata(path)?;
    if link_metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is a symlink", path.display()),
        ));
    }

    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a regular file", path.display()),
        ));
    }
    if metadata.uid() != unsafe { libc::getuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not owned by the current user", path.display()),
        ));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not owner-only", path.display()),
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn validate_shared_memory_file(file: &File, _path: &Path) -> io::Result<()> {
    if !file.metadata()?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "external-plugin IPC path is not a regular file",
        ));
    }
    validate_windows_owner_only_dacl(_path)?;
    Ok(())
}

#[cfg(windows)]
pub(super) fn validate_windows_owner_only_dacl(path: &Path) -> io::Result<()> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACL, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
    use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;

    let mut user_sid = current_windows_user_sid()?;
    let path_w = windows_path_wide(path);
    let mut owner: PSID = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut security_descriptor: PSECURITY_DESCRIPTOR = null_mut();

    // SAFETY: output pointers are valid and the returned security descriptor is
    // released with LocalFree below.
    let status = unsafe {
        GetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut security_descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(win32_error(status));
    }

    let result = (|| {
        if owner.is_null() || unsafe { EqualSid(owner, user_sid.as_mut_ptr() as PSID) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not owned by the current user", path.display()),
            ));
        }
        if dacl.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} has no owner-only DACL", path.display()),
            ));
        }

        // SAFETY: `dacl` comes from GetNamedSecurityInfoW and is valid until
        // `security_descriptor` is freed.
        if unsafe { (*dacl).AceCount } != 1 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL is not owner-only", path.display()),
            ));
        }

        let mut ace = null_mut();
        // SAFETY: index 0 is valid because AceCount == 1.
        if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() {
            return Err(io::Error::last_os_error());
        }
        let allowed = ace as *const ACCESS_ALLOWED_ACE;
        // SAFETY: GetAce returned a valid ACE pointer.
        let header = unsafe { (*allowed).Header };
        if header.AceType != ACCESS_ALLOWED_ACE_TYPE as u8 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL contains a non-allow ACE", path.display()),
            ));
        }
        // SAFETY: ACCESS_ALLOWED_ACE stores the SID immediately at SidStart.
        let ace_sid = unsafe { &(*allowed).SidStart as *const u32 as PSID };
        if unsafe { EqualSid(ace_sid, user_sid.as_mut_ptr() as PSID) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL grants a non-owner principal", path.display()),
            ));
        }
        // SAFETY: GetAce returned a valid ACCESS_ALLOWED_ACE.
        if unsafe { (*allowed).Mask } & FILE_ALL_ACCESS != FILE_ALL_ACCESS {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} DACL does not grant owner full access", path.display()),
            ));
        }
        Ok(())
    })();

    if !security_descriptor.is_null() {
        // SAFETY: security_descriptor was allocated by GetNamedSecurityInfoW.
        unsafe {
            LocalFree(security_descriptor as _);
        }
    }
    result
}
