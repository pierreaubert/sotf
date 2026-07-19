#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::sandboxed_plugin_creation_options::SandboxedPluginCreationOptions;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::sync::{OnceLock, RwLock};

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) const MAX_EXTERNAL_PLUGIN_DEADLINE_MICROS: u64 = 10_000;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) static DEFAULT_SANDBOXED_PLUGIN_CREATION_OPTIONS: OnceLock<
    RwLock<Option<SandboxedPluginCreationOptions>>,
> = OnceLock::new();
