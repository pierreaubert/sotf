use super::plugin_sandbox_identity::PluginSandboxIdentity;
use super::plugin_sandbox_permission::PluginSandboxPermission;
use super::plugin_sandbox_permission_decision::PluginSandboxPermissionDecision;
use super::types::PluginSandboxGrantPersistence;
use super::types::PluginSandboxPermissionOutcome;
use super::types::PluginSandboxUserGrant;
use crate::external_plugin::PluginDescriptor;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginSandboxPermissionRequest {
    pub identity: PluginSandboxIdentity,
    pub permission: PluginSandboxPermission,
    pub reason: Option<String>,
}

impl PluginSandboxPermissionRequest {
    pub fn from_descriptor(
        descriptor: &PluginDescriptor,
        permission: PluginSandboxPermission,
        reason: impl Into<Option<String>>,
    ) -> Self {
        Self {
            identity: PluginSandboxIdentity::from_descriptor(descriptor),
            permission,
            reason: reason.into(),
        }
    }

    pub fn deny(self) -> PluginSandboxPermissionDecision {
        PluginSandboxPermissionDecision {
            request: self,
            outcome: PluginSandboxPermissionOutcome::Denied,
            restart_required: false,
        }
    }

    pub fn grant_until_restart(self) -> PluginSandboxPermissionDecision {
        self.grant(PluginSandboxGrantPersistence::UntilRestart)
    }

    pub fn grant_remembered(self) -> PluginSandboxPermissionDecision {
        self.grant(PluginSandboxGrantPersistence::RememberForPlugin)
    }

    pub fn grant_already_active(self) -> PluginSandboxPermissionDecision {
        let grant = PluginSandboxUserGrant {
            identity: self.identity.clone(),
            permission: self.permission.clone(),
        };
        PluginSandboxPermissionDecision {
            request: self,
            outcome: PluginSandboxPermissionOutcome::Granted {
                grant,
                persistence: PluginSandboxGrantPersistence::RememberForPlugin,
            },
            restart_required: false,
        }
    }

    pub(super) fn grant(
        self,
        persistence: PluginSandboxGrantPersistence,
    ) -> PluginSandboxPermissionDecision {
        let grant = PluginSandboxUserGrant {
            identity: self.identity.clone(),
            permission: self.permission.clone(),
        };
        PluginSandboxPermissionDecision {
            request: self,
            outcome: PluginSandboxPermissionOutcome::Granted { grant, persistence },
            restart_required: true,
        }
    }
}
