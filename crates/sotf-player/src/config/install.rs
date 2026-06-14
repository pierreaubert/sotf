use super::get::get_plugin_presets_dir;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::load::load_plugin_sandbox_grants;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::misc::dedupe_paths;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::plugin::plugin_sandbox_media_paths;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::types::PluginSandboxRuntimeStatus;
use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn install_authorized_runtime_plugin_sandbox(
    library_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<PluginSandboxRuntimeStatus, Box<dyn std::error::Error>> {
    let grants = load_plugin_sandbox_grants()?;
    let preset_root = get_plugin_presets_dir()
        .ok_or_else(|| std::io::Error::other("Could not determine plugin preset root"))?;
    let media_read_paths = plugin_sandbox_media_paths(library_dirs);
    let status = PluginSandboxRuntimeStatus {
        preset_root,
        protected_import_paths: media_read_paths.clone(),
        media_read_paths,
        persistent_grant_count: grants.grants.len(),
        runtime_external_access_disabled: true,
    };
    let options = sotf_plugins::SandboxedPluginCreationOptions::authorized_runtime(
        grants.clone(),
        status.preset_root.clone(),
        status.media_read_paths.clone(),
    );
    sotf_plugins::set_default_sandboxed_plugin_creation_options(Some(options));
    Ok(status)
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn install_import_plugin_sandbox(
    protected_media_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<PluginSandboxRuntimeStatus, Box<dyn std::error::Error>> {
    let grants = load_plugin_sandbox_grants()?;
    let preset_root = get_plugin_presets_dir()
        .ok_or_else(|| std::io::Error::other("Could not determine plugin preset root"))?;
    let protected_import_paths = dedupe_paths(protected_media_paths);
    let options = sotf_plugins::SandboxedPluginCreationOptions::import(
        grants.clone(),
        preset_root.clone(),
        protected_import_paths.clone(),
    );
    sotf_plugins::set_default_sandboxed_plugin_creation_options(Some(options));

    Ok(PluginSandboxRuntimeStatus {
        preset_root,
        media_read_paths: Vec::new(),
        protected_import_paths,
        persistent_grant_count: grants.grants.len(),
        runtime_external_access_disabled: false,
    })
}
