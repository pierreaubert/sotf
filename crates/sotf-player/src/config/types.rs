use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSandboxRuntimeStatus {
    pub preset_root: PathBuf,
    pub media_read_paths: Vec<PathBuf>,
    pub protected_import_paths: Vec<PathBuf>,
    pub persistent_grant_count: usize,
    pub runtime_external_access_disabled: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSandboxPermissionResolution {
    pub decision: sotf_plugins::PluginSandboxPermissionDecision,
    pub session_grants_changed: bool,
    pub persistent_grants_changed: bool,
    pub restart_required: bool,
}
