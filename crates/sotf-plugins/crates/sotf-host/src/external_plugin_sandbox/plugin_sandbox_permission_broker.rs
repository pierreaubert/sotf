use super::plugin_sandbox_permission_decision::PluginSandboxPermissionDecision;
use super::plugin_sandbox_permission_request::PluginSandboxPermissionRequest;

pub trait PluginSandboxPermissionBroker {
    fn decide_permission(
        &mut self,
        request: PluginSandboxPermissionRequest,
    ) -> PluginSandboxPermissionDecision;
}
