#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::consts::DEFAULT_SANDBOXED_PLUGIN_CREATION_OPTIONS;
use crate::PluginDescriptor;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{
    ExternalPluginWorkerCommand, PluginSandboxGrantStore, PluginSandboxLaunchBackend,
    PluginSandboxLifecycleMode, PluginSandboxPolicy, current_plugin_sandbox_launch_backend,
    current_plugin_sandbox_launcher_command,
};
use std::path::PathBuf;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::sync::RwLock;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone)]
pub struct SandboxedPluginCreationOptions {
    pub grants: PluginSandboxGrantStore,
    pub preset_root: PathBuf,
    pub lifecycle: PluginSandboxLifecycleMode,
    pub protected_media_paths: Vec<PathBuf>,
    pub media_read_paths: Vec<PathBuf>,
    pub sandbox_launch_backend: PluginSandboxLaunchBackend,
    pub sandbox_launcher_command: Option<ExternalPluginWorkerCommand>,
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
impl SandboxedPluginCreationOptions {
    pub fn authorized_runtime(
        grants: PluginSandboxGrantStore,
        preset_root: impl Into<PathBuf>,
        media_read_paths: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        Self {
            grants,
            preset_root: preset_root.into(),
            lifecycle: PluginSandboxLifecycleMode::AuthorizedRuntime,
            protected_media_paths: Vec::new(),
            media_read_paths: media_read_paths.into_iter().collect(),
            sandbox_launch_backend: current_plugin_sandbox_launch_backend(),
            sandbox_launcher_command: current_plugin_sandbox_launcher_command(),
        }
    }

    pub fn import(
        grants: PluginSandboxGrantStore,
        preset_root: impl Into<PathBuf>,
        protected_media_paths: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        Self {
            grants,
            preset_root: preset_root.into(),
            lifecycle: PluginSandboxLifecycleMode::Import,
            protected_media_paths: protected_media_paths.into_iter().collect(),
            media_read_paths: Vec::new(),
            sandbox_launch_backend: current_plugin_sandbox_launch_backend(),
            sandbox_launcher_command: current_plugin_sandbox_launcher_command(),
        }
    }

    pub fn with_backend(mut self, backend: PluginSandboxLaunchBackend) -> Self {
        self.sandbox_launch_backend = backend;
        self
    }

    pub fn with_launcher(mut self, launcher: Option<ExternalPluginWorkerCommand>) -> Self {
        self.sandbox_launcher_command = launcher;
        self
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn set_default_sandboxed_plugin_creation_options(
    options: Option<SandboxedPluginCreationOptions>,
) {
    let lock = DEFAULT_SANDBOXED_PLUGIN_CREATION_OPTIONS.get_or_init(|| RwLock::new(None));
    *lock
        .write()
        .expect("sandboxed plugin creation options lock poisoned") = options;
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn default_sandboxed_plugin_creation_options() -> Option<SandboxedPluginCreationOptions> {
    DEFAULT_SANDBOXED_PLUGIN_CREATION_OPTIONS
        .get()
        .and_then(|lock| {
            lock.read()
                .expect("sandboxed plugin creation options lock poisoned")
                .clone()
        })
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn sandbox_policy_for_creation_options(
    options: &SandboxedPluginCreationOptions,
    descriptor: &PluginDescriptor,
) -> Result<PluginSandboxPolicy, String> {
    let policy = match options.lifecycle {
        PluginSandboxLifecycleMode::Import => options.grants.import_policy_for_plugin(
            descriptor,
            &options.preset_root,
            options.protected_media_paths.clone(),
        ),
        PluginSandboxLifecycleMode::AuthorizedRuntime => {
            options.grants.authorized_runtime_policy_for_plugin(
                descriptor,
                &options.preset_root,
                options.media_read_paths.clone(),
            )
        }
    };
    policy.validate_protected_media_paths()?;
    Ok(policy)
}
