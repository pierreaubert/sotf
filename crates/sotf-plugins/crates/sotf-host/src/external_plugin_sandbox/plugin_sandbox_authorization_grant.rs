#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxAuthorizationGrant {
    Pace,
    Ilok,
    SystemKeychain,
    Any,
    Custom { id: String },
}

impl PluginSandboxAuthorizationGrant {
    pub fn satisfies(&self, requested: &Self) -> bool {
        matches!(self, Self::Any) || self == requested
    }
}
