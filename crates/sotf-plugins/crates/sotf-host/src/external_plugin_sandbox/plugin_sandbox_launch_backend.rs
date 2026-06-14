use super::plugin_sandbox_backend::PluginSandboxBackend;
use super::types::PluginSandboxBackendCapabilities;
#[cfg(target_os = "linux")]
use super::types::platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginSandboxLaunchBackend {
    LinuxLandlockWorker,
    MacosAppSandboxHelper,
    WindowsAppContainerWorker,
    ProcessIsolationOnly { platform: &'static str },
}

impl PluginSandboxLaunchBackend {
    pub fn backend_id(self) -> &'static str {
        match self {
            Self::LinuxLandlockWorker => "linux-landlock-worker",
            Self::MacosAppSandboxHelper => "macos-app-sandbox-helper",
            Self::WindowsAppContainerWorker => "windows-appcontainer-worker",
            Self::ProcessIsolationOnly { platform } => platform,
        }
    }

    pub fn capabilities(self) -> PluginSandboxBackendCapabilities {
        match self {
            Self::LinuxLandlockWorker => PluginSandboxBackendCapabilities {
                filesystem: true,
                network: true,
                local_authorization_profiles: false,
                child_process_control: false,
                prompt_without_restart: false,
                store_compatible: true,
            },
            Self::MacosAppSandboxHelper => PluginSandboxBackendCapabilities {
                filesystem: true,
                network: true,
                local_authorization_profiles: true,
                child_process_control: false,
                prompt_without_restart: false,
                store_compatible: true,
            },
            Self::WindowsAppContainerWorker => PluginSandboxBackendCapabilities {
                filesystem: true,
                network: true,
                local_authorization_profiles: true,
                child_process_control: true,
                prompt_without_restart: false,
                store_compatible: true,
            },
            Self::ProcessIsolationOnly { .. } => PluginSandboxBackendCapabilities {
                filesystem: false,
                network: false,
                local_authorization_profiles: false,
                child_process_control: false,
                prompt_without_restart: false,
                store_compatible: true,
            },
        }
    }

    pub fn requires_host_launcher(self) -> bool {
        matches!(
            self,
            Self::MacosAppSandboxHelper | Self::WindowsAppContainerWorker
        )
    }

    pub fn uses_direct_worker_binary(self) -> bool {
        !self.requires_host_launcher()
    }
}

impl PluginSandboxBackend for PluginSandboxLaunchBackend {
    fn backend_id(&self) -> &'static str {
        (*self).backend_id()
    }

    fn capabilities(&self) -> PluginSandboxBackendCapabilities {
        (*self).capabilities()
    }
}
