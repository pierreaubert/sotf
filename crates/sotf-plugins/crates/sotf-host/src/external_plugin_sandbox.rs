//! Sandbox policy for isolated external plugin workers.
//!
//! The public policy is intentionally portable, but enforcement is platform
//! specific. Linux currently applies a best-effort Landlock filesystem sandbox.
//! macOS and Windows expose explicit process-isolation-only backends when native
//! sandbox enforcement is unavailable in this build.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::external_plugin::PluginDescriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginSandboxTiming {
    Disabled,
    BeforePluginLoad,
    AfterPluginLoad,
}

impl ExternalPluginSandboxTiming {
    pub fn as_arg(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::BeforePluginLoad => "before-plugin-load",
            Self::AfterPluginLoad => "after-plugin-load",
        }
    }
}

impl FromStr for ExternalPluginSandboxTiming {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "disabled" | "off" | "none" => Ok(Self::Disabled),
            "before-plugin-load" | "before_load" | "before-load" | "pre_load" | "preload" => {
                Ok(Self::BeforePluginLoad)
            }
            "after-plugin-load" | "after_load" | "after-load" | "post_load" | "postload" => {
                Ok(Self::AfterPluginLoad)
            }
            other => Err(format!("unknown external-plugin sandbox timing '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginTrust {
    Unknown,
    Untrusted,
    Signed,
}

impl FromStr for ExternalPluginTrust {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "unknown" => Ok(Self::Unknown),
            "untrusted" => Ok(Self::Untrusted),
            "signed" | "trusted" | "known" => Ok(Self::Signed),
            other => Err(format!("unknown external-plugin trust value '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPluginSandboxPolicy {
    pub timing: ExternalPluginSandboxTiming,
    pub require_platform_sandbox: bool,
    pub allow_network: bool,
    pub allow_child_processes: bool,
    pub extra_read_paths: Vec<PathBuf>,
    pub extra_write_paths: Vec<PathBuf>,
}

impl ExternalPluginSandboxPolicy {
    pub fn disabled() -> Self {
        Self {
            timing: ExternalPluginSandboxTiming::Disabled,
            require_platform_sandbox: false,
            allow_network: true,
            allow_child_processes: true,
            extra_read_paths: Vec::new(),
            extra_write_paths: Vec::new(),
        }
    }

    pub fn for_trust(trust: ExternalPluginTrust) -> Self {
        match trust {
            ExternalPluginTrust::Signed => Self {
                timing: ExternalPluginSandboxTiming::AfterPluginLoad,
                require_platform_sandbox: false,
                allow_network: false,
                allow_child_processes: false,
                extra_read_paths: Vec::new(),
                extra_write_paths: Vec::new(),
            },
            ExternalPluginTrust::Unknown | ExternalPluginTrust::Untrusted => Self {
                timing: ExternalPluginSandboxTiming::BeforePluginLoad,
                require_platform_sandbox: should_require_platform_sandbox(trust),
                allow_network: false,
                allow_child_processes: false,
                extra_read_paths: Vec::new(),
                extra_write_paths: Vec::new(),
            },
        }
    }

    pub fn command_args(&self) -> Vec<String> {
        let mut args = vec![
            "--sandbox-timing".to_string(),
            self.timing.as_arg().to_string(),
        ];

        if self.require_platform_sandbox {
            args.push("--sandbox-required".to_string());
        }
        if self.allow_network {
            args.push("--sandbox-allow-network".to_string());
        }
        if self.allow_child_processes {
            args.push("--sandbox-allow-child-processes".to_string());
        }

        for path in &self.extra_read_paths {
            args.push("--sandbox-read-path".to_string());
            args.push(path.display().to_string());
        }
        for path in &self.extra_write_paths {
            args.push("--sandbox-write-path".to_string());
            args.push(path.display().to_string());
        }

        args
    }
}

const fn should_require_platform_sandbox(trust: ExternalPluginTrust) -> bool {
    match trust {
        ExternalPluginTrust::Signed => false,
        ExternalPluginTrust::Unknown | ExternalPluginTrust::Untrusted => cfg!(target_os = "linux"),
    }
}

impl Default for ExternalPluginSandboxPolicy {
    fn default() -> Self {
        Self::for_trust(ExternalPluginTrust::Unknown)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalPluginSandboxStatus {
    Disabled,
    Enforced {
        backend: &'static str,
    },
    Unsupported {
        backend: &'static str,
        reason: String,
    },
}

impl ExternalPluginSandboxStatus {
    pub fn is_enforced(&self) -> bool {
        matches!(self, Self::Enforced { .. })
    }
}

pub fn enter_external_plugin_sandbox(
    policy: &ExternalPluginSandboxPolicy,
    descriptor: &PluginDescriptor,
    shared_memory_path: &Path,
) -> Result<ExternalPluginSandboxStatus, String> {
    if policy.timing == ExternalPluginSandboxTiming::Disabled {
        return Ok(ExternalPluginSandboxStatus::Disabled);
    }

    let status = platform::enter(policy, descriptor, shared_memory_path)?;
    if policy.require_platform_sandbox && !status.is_enforced() {
        return Err(format!(
            "external-plugin sandbox is required but was not enforced: {status:?}"
        ));
    }
    Ok(status)
}

#[cfg(target_os = "linux")]
mod platform {
    use std::ffi::CString;
    use std::mem;
    use std::os::fd::RawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use crate::external_plugin::PluginDescriptor;

    use super::{ExternalPluginSandboxPolicy, ExternalPluginSandboxStatus};

    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
    const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

    const FS_EXECUTE: u64 = 1 << 0;
    const FS_WRITE_FILE: u64 = 1 << 1;
    const FS_READ_FILE: u64 = 1 << 2;
    const FS_READ_DIR: u64 = 1 << 3;
    const FS_REMOVE_DIR: u64 = 1 << 4;
    const FS_REMOVE_FILE: u64 = 1 << 5;
    const FS_MAKE_CHAR: u64 = 1 << 6;
    const FS_MAKE_DIR: u64 = 1 << 7;
    const FS_MAKE_REG: u64 = 1 << 8;
    const FS_MAKE_SOCK: u64 = 1 << 9;
    const FS_MAKE_FIFO: u64 = 1 << 10;
    const FS_MAKE_BLOCK: u64 = 1 << 11;
    const FS_MAKE_SYM: u64 = 1 << 12;
    const FS_REFER: u64 = 1 << 13;
    const FS_TRUNCATE: u64 = 1 << 14;

    const NET_BIND_TCP: u64 = 1 << 0;
    const NET_CONNECT_TCP: u64 = 1 << 1;

    #[repr(C)]
    struct LandlockRulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
        scoped: u64,
    }

    #[repr(C)]
    struct LandlockPathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    pub fn enter(
        policy: &ExternalPluginSandboxPolicy,
        descriptor: &PluginDescriptor,
        shared_memory_path: &Path,
    ) -> Result<ExternalPluginSandboxStatus, String> {
        let abi = landlock_abi()?;
        if abi <= 0 {
            set_no_new_privs()?;
            return Ok(ExternalPluginSandboxStatus::Unsupported {
                backend: "linux-landlock",
                reason: "Landlock is not supported or disabled by the running kernel".to_string(),
            });
        }

        let handled_access_fs = fs_access_mask_for_abi(abi);
        let handled_access_net = if abi >= 4 && !policy.allow_network {
            NET_BIND_TCP | NET_CONNECT_TCP
        } else {
            0
        };
        let ruleset_attr = LandlockRulesetAttr {
            handled_access_fs,
            handled_access_net,
            scoped: 0,
        };

        let ruleset_fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &ruleset_attr,
                mem::size_of::<LandlockRulesetAttr>(),
                0,
            ) as RawFd
        };
        if ruleset_fd < 0 {
            return Err(format!(
                "failed to create Landlock ruleset: {}",
                std::io::Error::last_os_error()
            ));
        }

        let result = apply_rules(
            policy,
            descriptor,
            shared_memory_path,
            ruleset_fd,
            handled_access_fs,
        )
        .and_then(|_| restrict_self(ruleset_fd));
        unsafe {
            libc::close(ruleset_fd);
        }
        result?;

        Ok(ExternalPluginSandboxStatus::Enforced {
            backend: "linux-landlock",
        })
    }

    fn apply_rules(
        policy: &ExternalPluginSandboxPolicy,
        descriptor: &PluginDescriptor,
        shared_memory_path: &Path,
        ruleset_fd: RawFd,
        handled_access_fs: u64,
    ) -> Result<(), String> {
        add_path_rule(
            ruleset_fd,
            &descriptor.path,
            (FS_READ_FILE | FS_READ_DIR | FS_EXECUTE) & handled_access_fs,
        )?;

        add_path_rule(
            ruleset_fd,
            shared_memory_path,
            (FS_READ_FILE | FS_WRITE_FILE | FS_TRUNCATE) & handled_access_fs,
        )?;

        for path in &policy.extra_read_paths {
            add_path_rule(
                ruleset_fd,
                path,
                (FS_READ_FILE | FS_READ_DIR | FS_EXECUTE) & handled_access_fs,
            )?;
        }
        for path in &policy.extra_write_paths {
            add_path_rule(ruleset_fd, path, writable_access() & handled_access_fs)?;
        }

        Ok(())
    }

    fn writable_access() -> u64 {
        FS_READ_FILE
            | FS_WRITE_FILE
            | FS_READ_DIR
            | FS_REMOVE_DIR
            | FS_REMOVE_FILE
            | FS_MAKE_CHAR
            | FS_MAKE_DIR
            | FS_MAKE_REG
            | FS_MAKE_SOCK
            | FS_MAKE_FIFO
            | FS_MAKE_BLOCK
            | FS_MAKE_SYM
            | FS_REFER
            | FS_TRUNCATE
    }

    fn fs_access_mask_for_abi(abi: i32) -> u64 {
        let mut mask = writable_access() | FS_EXECUTE;
        if abi < 2 {
            mask &= !FS_REFER;
        }
        if abi < 3 {
            mask &= !FS_TRUNCATE;
        }
        mask
    }

    fn add_path_rule(ruleset_fd: RawFd, path: &Path, access: u64) -> Result<(), String> {
        if access == 0 {
            return Ok(());
        }
        let path = canonicalize_if_possible(path);
        let fd = open_path_fd(&path)?;
        let rule = LandlockPathBeneathAttr {
            allowed_access: access,
            parent_fd: fd,
        };
        let result = unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                ruleset_fd,
                LANDLOCK_RULE_PATH_BENEATH,
                &rule,
                0,
            )
        };
        unsafe {
            libc::close(fd);
        }
        if result < 0 {
            return Err(format!(
                "failed to add Landlock path rule for '{}': {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn canonicalize_if_possible(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn open_path_fd(path: &Path) -> Result<RawFd, String> {
        let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            format!(
                "cannot add sandbox path with interior NUL byte: '{}'",
                path.display()
            )
        })?;
        let fd = unsafe {
            libc::open(
                c_path.as_ptr(),
                libc::O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(format!(
                "failed to open sandbox path '{}': {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(fd)
    }

    fn restrict_self(ruleset_fd: RawFd) -> Result<(), String> {
        set_no_new_privs()?;
        let result = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0) };
        if result < 0 {
            return Err(format!(
                "failed to enter Landlock sandbox: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn landlock_abi() -> Result<i32, String> {
        let abi = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<LandlockRulesetAttr>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if abi < 0 {
            let err = std::io::Error::last_os_error();
            let code = err.raw_os_error().unwrap_or_default();
            if code == libc::ENOSYS || code == libc::EOPNOTSUPP {
                return Ok(0);
            }
            return Err(format!("failed to query Landlock ABI: {err}"));
        }
        Ok(abi as i32)
    }

    fn set_no_new_privs() -> Result<(), String> {
        let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if result != 0 {
            return Err(format!(
                "failed to set no_new_privs before sandboxing external plugin: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    #[cfg(target_os = "macos")]
    const BACKEND_NAME: &str = "macos-process-isolation";
    #[cfg(target_os = "windows")]
    const BACKEND_NAME: &str = "windows-process-isolation";
    #[cfg(target_os = "macos")]
    const BACKEND_NOTE: &str =
        "macOS native sandbox backend is unavailable in this build; worker uses process isolation";
    #[cfg(target_os = "windows")]
    const BACKEND_NOTE: &str = "Windows native sandbox backend is unavailable in this build; worker uses process isolation";

    use std::fs::OpenOptions;
    use std::path::Path;

    use crate::external_plugin::PluginDescriptor;
    use crate::external_plugin_ipc;

    use super::{ExternalPluginSandboxPolicy, ExternalPluginSandboxStatus};

    pub fn enter(
        _policy: &ExternalPluginSandboxPolicy,
        _descriptor: &PluginDescriptor,
        _shared_memory_path: &Path,
    ) -> Result<ExternalPluginSandboxStatus, String> {
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        let file = options
            .open(_shared_memory_path)
            .map_err(|err| format!("shared memory is not accessible: {err}"))?;

        external_plugin_ipc::validate_shared_memory_file(&file, _shared_memory_path)
            .map_err(|err| format!("shared memory failed sandbox integrity check: {err}"))?;

        Ok(ExternalPluginSandboxStatus::Unsupported {
            backend: BACKEND_NAME,
            reason: BACKEND_NOTE.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_plugin::{PluginDescriptor, PluginFormat};
    use crate::external_plugin_ipc::{PluginIpcLayout, SecurePluginSharedMemory};

    #[test]
    fn trust_maps_to_expected_sandbox_timing() {
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Signed).timing,
            ExternalPluginSandboxTiming::AfterPluginLoad
        );
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown).timing,
            ExternalPluginSandboxTiming::BeforePluginLoad
        );
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Untrusted).timing,
            ExternalPluginSandboxTiming::BeforePluginLoad
        );
    }

    #[test]
    fn untrusted_requires_platform_enforcement() {
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Untrusted)
                .require_platform_sandbox,
            cfg!(target_os = "linux")
        );
        assert!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown)
                .require_platform_sandbox
                == cfg!(target_os = "linux")
        );
        assert!(
            !ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Signed)
                .require_platform_sandbox
        );
    }

    #[test]
    fn sandbox_timing_parses_compat_aliases() {
        assert_eq!(
            "pre_load".parse::<ExternalPluginSandboxTiming>().unwrap(),
            ExternalPluginSandboxTiming::BeforePluginLoad
        );
        assert_eq!(
            "after-load".parse::<ExternalPluginSandboxTiming>().unwrap(),
            ExternalPluginSandboxTiming::AfterPluginLoad
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_unknown_trust_returns_unsupported_status() {
        let temp = tempfile::tempdir().unwrap();
        let plugin_path = temp.path().join("sandbox-test.clap");
        std::fs::write(&plugin_path, b"stub").unwrap();
        let descriptor = PluginDescriptor {
            id: "sandbox.test".into(),
            name: "sandbox-test".into(),
            vendor: "test".into(),
            version: "0.1".into(),
            format: PluginFormat::Clap,
            path: plugin_path,
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: crate::external_plugin::PluginScanStatus::Discovered,
        };
        let shared =
            SecurePluginSharedMemory::create(PluginIpcLayout::new(48_000, 64, 2, 2).unwrap())
                .unwrap();
        let policy = ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown);

        let status = enter_external_plugin_sandbox(&policy, &descriptor, shared.path()).unwrap();
        assert!(matches!(
            status,
            ExternalPluginSandboxStatus::Unsupported { .. }
        ));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_required_sandbox_reports_error() {
        let temp = tempfile::tempdir().unwrap();
        let plugin_path = temp.path().join("sandbox-required-test.clap");
        std::fs::write(&plugin_path, b"stub").unwrap();
        let descriptor = PluginDescriptor {
            id: "sandbox.required.test".into(),
            name: "sandbox-required-test".into(),
            vendor: "test".into(),
            version: "0.1".into(),
            format: PluginFormat::Clap,
            path: plugin_path,
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: crate::external_plugin::PluginScanStatus::Discovered,
        };
        let shared =
            SecurePluginSharedMemory::create(PluginIpcLayout::new(48_000, 64, 2, 2).unwrap())
                .unwrap();
        let mut policy = ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown);
        policy.require_platform_sandbox = true;

        let err = enter_external_plugin_sandbox(&policy, &descriptor, shared.path()).unwrap_err();
        assert!(err.contains("required"));
    }
}
