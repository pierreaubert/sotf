use super::plugin_sandbox_authorization_grant::PluginSandboxAuthorizationGrant;
use super::plugin_sandbox_child_process_grant::PluginSandboxChildProcessGrant;
use super::plugin_sandbox_network_grant::PluginSandboxNetworkGrant;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxPermission {
    ReadPath { path: PathBuf },
    WritePath { path: PathBuf },
    Network(PluginSandboxNetworkGrant),
    LocalAuthorization(PluginSandboxAuthorizationGrant),
    ChildProcess(PluginSandboxChildProcessGrant),
}

impl PluginSandboxPermission {
    pub fn satisfies(&self, requested: &Self) -> bool {
        match (self, requested) {
            (Self::ReadPath { path: granted }, Self::ReadPath { path: requested }) => {
                requested.starts_with(granted)
            }
            (Self::WritePath { path: granted }, Self::ReadPath { path: requested })
            | (Self::WritePath { path: granted }, Self::WritePath { path: requested }) => {
                requested.starts_with(granted)
            }
            (Self::Network(granted), Self::Network(requested)) => granted.satisfies(requested),
            (Self::LocalAuthorization(granted), Self::LocalAuthorization(requested)) => {
                granted.satisfies(requested)
            }
            (Self::ChildProcess(granted), Self::ChildProcess(requested)) => {
                granted.satisfies(requested)
            }
            _ => false,
        }
    }
}
