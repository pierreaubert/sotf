use super::plugin_sandbox_permission_broker::PluginSandboxPermissionBroker;
use super::plugin_sandbox_permission_decision::PluginSandboxPermissionDecision;
use super::plugin_sandbox_permission_request::PluginSandboxPermissionRequest;

#[derive(Debug, Default, Clone, Copy)]
pub struct DenyPluginSandboxPermissionBroker;

impl PluginSandboxPermissionBroker for DenyPluginSandboxPermissionBroker {
    fn decide_permission(
        &mut self,
        request: PluginSandboxPermissionRequest,
    ) -> PluginSandboxPermissionDecision {
        request.deny()
    }
}
