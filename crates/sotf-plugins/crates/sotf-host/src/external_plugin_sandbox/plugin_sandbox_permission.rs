use super::plugin_sandbox_authorization_grant::PluginSandboxAuthorizationGrant;
use super::plugin_sandbox_child_process_grant::PluginSandboxChildProcessGrant;
use super::plugin_sandbox_network_grant::PluginSandboxNetworkGrant;
use std::path::{Component, Path, PathBuf};

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
                path_is_within(granted, requested)
            }
            (Self::WritePath { path: granted }, Self::ReadPath { path: requested })
            | (Self::WritePath { path: granted }, Self::WritePath { path: requested }) => {
                path_is_within(granted, requested)
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

/// Compare path components after removing harmless `.` components.
///
/// A persisted grant is an authorization boundary, so unresolved `..`
/// components are rejected rather than compared lexically.  Otherwise a
/// request such as `/granted/../protected` would pass `starts_with` even
/// though the kernel resolves it outside the granted directory.  Symlink
/// resolution remains the responsibility of the platform sandbox immediately
/// before launch; this helper deliberately also works for paths that do not
/// exist yet (for example a newly-created preset file).
fn path_is_within(granted: &Path, requested: &Path) -> bool {
    let Some(granted) = normalize_authorization_path(granted) else {
        return false;
    };
    let Some(requested) = normalize_authorization_path(requested) else {
        return false;
    };
    requested.starts_with(granted)
}

fn normalize_authorization_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => return None,
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str())
            }
        }
    }
    Some(normalized)
}
