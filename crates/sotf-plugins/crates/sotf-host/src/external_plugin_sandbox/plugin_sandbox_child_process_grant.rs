use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxChildProcessGrant {
    Deny,
    AllowSignedHelpers { paths: Vec<PathBuf> },
    AllowAny,
}

impl PluginSandboxChildProcessGrant {
    pub fn allows_any_child_process(&self) -> bool {
        matches!(self, Self::AllowAny)
    }

    pub fn satisfies(&self, requested: &Self) -> bool {
        match (self, requested) {
            (Self::AllowAny, _) => true,
            (Self::AllowSignedHelpers { paths: granted }, Self::AllowSignedHelpers { paths }) => {
                paths.iter().all(|path| granted.contains(path))
            }
            (Self::Deny, Self::Deny) => true,
            _ => false,
        }
    }
}
