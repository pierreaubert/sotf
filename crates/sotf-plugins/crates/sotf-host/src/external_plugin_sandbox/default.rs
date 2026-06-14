use super::misc::home_dir;
use super::plugin_sandbox_launch_backend::PluginSandboxLaunchBackend;
use crate::external_plugin_process::ExternalPluginWorkerCommand;
use std::path::PathBuf;

pub fn default_plugin_sandbox_launcher_command_for_backend(
    backend: PluginSandboxLaunchBackend,
) -> Option<ExternalPluginWorkerCommand> {
    match backend {
        PluginSandboxLaunchBackend::MacosAppSandboxHelper => {
            Some(ExternalPluginWorkerCommand::default_macos_sandbox_helper_binary())
        }
        _ => None,
    }
}

pub fn default_plugin_sandbox_protected_media_paths() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };

    [
        "Music", "music", "Audio", "audio", "WAV", "wav", "wavs", "Stems", "stems",
    ]
    .into_iter()
    .map(|component| home.join(component))
    .collect()
}
