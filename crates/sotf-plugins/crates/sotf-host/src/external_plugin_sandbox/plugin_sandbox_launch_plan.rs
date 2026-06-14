use super::external_plugin_sandbox_timing::ExternalPluginSandboxTiming;
use super::plugin_sandbox_launch_backend::PluginSandboxLaunchBackend;
use super::plugin_sandbox_policy::PluginSandboxPolicy;
use super::plugin_sandbox_policy_adapter_issue::PluginSandboxPolicyAdapterIssue;
use super::plugin_sandbox_policy_support_issue::PluginSandboxPolicySupportIssue;
use super::types::PluginSandboxBackendCapabilities;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSandboxLaunchPlan {
    pub backend: PluginSandboxLaunchBackend,
    pub capabilities: PluginSandboxBackendCapabilities,
    pub support_issues: Vec<PluginSandboxPolicySupportIssue>,
    pub adapter_issues: Vec<PluginSandboxPolicyAdapterIssue>,
}

impl PluginSandboxLaunchPlan {
    pub fn backend_id(&self) -> &'static str {
        self.backend.backend_id()
    }

    pub fn is_store_compatible(&self) -> bool {
        self.capabilities.store_compatible
    }

    pub fn is_fully_supported(&self) -> bool {
        self.support_issues.is_empty() && self.adapter_issues.is_empty()
    }

    pub fn validate_for_launch(&self, policy: &PluginSandboxPolicy) -> Result<(), String> {
        if policy.timing == ExternalPluginSandboxTiming::Disabled {
            return Ok(());
        }

        if policy.require_platform_sandbox && !self.support_issues.is_empty() {
            let summary = self
                .support_issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "plugin sandbox backend '{}' cannot satisfy required policy: {summary}",
                self.backend_id()
            ));
        }

        if !self.adapter_issues.is_empty() {
            let summary = self
                .adapter_issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "plugin sandbox backend '{}' cannot launch current worker policy: {summary}",
                self.backend_id()
            ));
        }

        Ok(())
    }
}
