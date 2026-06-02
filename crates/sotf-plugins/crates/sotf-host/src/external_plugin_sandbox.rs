//! Sandbox policy for isolated external plugin workers.
//!
//! The public policy is intentionally portable, but enforcement is platform
//! specific. Linux currently applies a best-effort Landlock filesystem sandbox.
//! macOS and Windows expose explicit process-isolation-only backends when native
//! sandbox enforcement is unavailable in this build.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::external_plugin::PluginDescriptor;
use crate::external_plugin_process::ExternalPluginWorkerCommand;

pub const MACOS_APP_SANDBOX_HELPER_ENV: &str = "SOTF_MACOS_APP_SANDBOX_HELPER";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginSandboxIdentity {
    pub plugin_id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub format: String,
    pub path: PathBuf,
}

impl PluginSandboxIdentity {
    pub fn from_descriptor(descriptor: &PluginDescriptor) -> Self {
        Self {
            plugin_id: descriptor.id.clone(),
            name: descriptor.name.clone(),
            vendor: descriptor.vendor.clone(),
            version: descriptor.version.clone(),
            format: format!("{:?}", descriptor.format),
            path: descriptor.path.clone(),
        }
    }

    pub fn stable_preset_component(&self) -> String {
        sanitize_path_component(&format!(
            "{}-{}-{}",
            self.format, self.vendor, self.plugin_id
        ))
    }
}

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

    fn grant(self, persistence: PluginSandboxGrantPersistence) -> PluginSandboxPermissionDecision {
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginSandboxPermissionDecision {
    pub request: PluginSandboxPermissionRequest,
    pub outcome: PluginSandboxPermissionOutcome,
    pub restart_required: bool,
}

impl PluginSandboxPermissionDecision {
    pub fn apply_to_store(&self, store: &mut PluginSandboxGrantStore) -> bool {
        store.apply_decision(self)
    }

    pub fn granted_permission(&self) -> Option<&PluginSandboxPermission> {
        match &self.outcome {
            PluginSandboxPermissionOutcome::Denied => None,
            PluginSandboxPermissionOutcome::Granted { grant, .. } => Some(&grant.permission),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxPermissionOutcome {
    Denied,
    Granted {
        grant: PluginSandboxUserGrant,
        persistence: PluginSandboxGrantPersistence,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxGrantPersistence {
    UntilRestart,
    RememberForPlugin,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginSandboxUserGrant {
    pub identity: PluginSandboxIdentity,
    pub permission: PluginSandboxPermission,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginSandboxGrantStore {
    pub grants: Vec<PluginSandboxUserGrant>,
}

impl PluginSandboxGrantStore {
    pub fn grants_for(
        &self,
        identity: &PluginSandboxIdentity,
    ) -> impl Iterator<Item = &PluginSandboxUserGrant> {
        self.grants
            .iter()
            .filter(move |grant| &grant.identity == identity)
    }

    pub fn remember(&mut self, grant: PluginSandboxUserGrant) {
        if !self.grants.contains(&grant) {
            self.grants.push(grant);
        }
    }

    pub fn revoke(&mut self, grant: &PluginSandboxUserGrant) -> bool {
        let before = self.grants.len();
        self.grants.retain(|stored| stored != grant);
        self.grants.len() != before
    }

    pub fn grants_permission(
        &self,
        identity: &PluginSandboxIdentity,
        permission: &PluginSandboxPermission,
    ) -> bool {
        self.grants_for(identity)
            .any(|grant| grant.permission.satisfies(permission))
    }

    pub fn apply_decision(&mut self, decision: &PluginSandboxPermissionDecision) -> bool {
        let PluginSandboxPermissionOutcome::Granted { grant, persistence } = &decision.outcome
        else {
            return false;
        };

        if *persistence != PluginSandboxGrantPersistence::RememberForPlugin {
            return false;
        }

        let before = self.grants.len();
        self.remember(grant.clone());
        self.grants.len() != before
    }

    pub fn apply_session_decision(&mut self, decision: &PluginSandboxPermissionDecision) -> bool {
        let PluginSandboxPermissionOutcome::Granted { grant, .. } = &decision.outcome else {
            return false;
        };

        let before = self.grants.len();
        self.remember(grant.clone());
        self.grants.len() != before
    }

    pub fn strict_policy_for_plugin(
        &self,
        descriptor: &PluginDescriptor,
        preset_root: impl Into<PathBuf>,
    ) -> PluginSandboxPolicy {
        let identity = PluginSandboxIdentity::from_descriptor(descriptor);
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir(
            preset_root.into().join(identity.stable_preset_component()),
        );
        policy.apply_user_grants(self.grants_for(&identity));
        policy
    }

    pub fn import_policy_for_plugin(
        &self,
        descriptor: &PluginDescriptor,
        preset_root: impl Into<PathBuf>,
        protected_media_paths: impl IntoIterator<Item = PathBuf>,
    ) -> PluginSandboxPolicy {
        let identity = PluginSandboxIdentity::from_descriptor(descriptor);
        let mut policy = PluginSandboxPolicy::import_with_preset_dir_and_protected_media_paths(
            preset_root.into().join(identity.stable_preset_component()),
            protected_media_paths,
        );
        policy.apply_import_user_grants(self.grants_for(&identity));
        policy
    }

    pub fn authorized_runtime_policy_for_plugin(
        &self,
        descriptor: &PluginDescriptor,
        preset_root: impl Into<PathBuf>,
        media_read_paths: impl IntoIterator<Item = PathBuf>,
    ) -> PluginSandboxPolicy {
        let identity = PluginSandboxIdentity::from_descriptor(descriptor);
        PluginSandboxPolicy::authorized_runtime_with_preset_dir_and_media_paths(
            preset_root.into().join(identity.stable_preset_component()),
            media_read_paths,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxLifecycleMode {
    Import,
    AuthorizedRuntime,
}

/// Portable sandbox policy for untrusted external plugins.
///
/// This capability model is intentionally platform-neutral and Store-friendly:
/// app layers can explain and persist narrow grants without depending on a
/// Linux, macOS, or Windows-specific sandbox vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginSandboxPolicy {
    pub timing: ExternalPluginSandboxTiming,
    pub require_platform_sandbox: bool,
    pub file_access: Vec<PluginSandboxFileGrant>,
    #[serde(default)]
    pub protected_media_paths: Vec<PathBuf>,
    pub network: PluginSandboxNetworkGrant,
    pub local_authorizations: Vec<PluginSandboxAuthorizationGrant>,
    pub child_processes: PluginSandboxChildProcessGrant,
    pub broker: PluginSandboxBrokerPolicy,
}

impl PluginSandboxPolicy {
    pub fn strict_with_preset_dir(preset_dir: impl Into<PathBuf>) -> Self {
        Self {
            timing: ExternalPluginSandboxTiming::BeforePluginLoad,
            require_platform_sandbox: should_require_platform_sandbox(ExternalPluginTrust::Unknown),
            file_access: vec![
                PluginSandboxFileGrant::PluginBundleReadExecute,
                PluginSandboxFileGrant::PresetDirectoryReadWrite {
                    path: preset_dir.into(),
                },
            ],
            protected_media_paths: Vec::new(),
            network: PluginSandboxNetworkGrant::Deny,
            local_authorizations: Vec::new(),
            child_processes: PluginSandboxChildProcessGrant::Deny,
            broker: PluginSandboxBrokerPolicy::PromptAndRestart,
        }
    }

    pub fn import_with_preset_dir_and_protected_media_paths(
        preset_dir: impl Into<PathBuf>,
        protected_media_paths: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        let mut policy = Self::strict_with_preset_dir(preset_dir);
        policy.network = PluginSandboxNetworkGrant::AnyOutbound;
        policy.broker = PluginSandboxBrokerPolicy::PromptAndRestart;
        policy.protected_media_paths = dedupe_paths(protected_media_paths);
        policy
    }

    pub fn authorized_runtime_with_preset_dir_and_media_paths(
        preset_dir: impl Into<PathBuf>,
        media_read_paths: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        let mut policy = Self::strict_with_preset_dir(preset_dir);
        policy.network = PluginSandboxNetworkGrant::Deny;
        policy.local_authorizations = Vec::new();
        policy.child_processes = PluginSandboxChildProcessGrant::Deny;
        policy.broker = PluginSandboxBrokerPolicy::NoPrompt;
        for path in media_read_paths {
            push_unique(
                &mut policy.file_access,
                PluginSandboxFileGrant::ReadOnlyPath { path },
            );
        }
        policy
    }

    pub fn disabled() -> Self {
        Self {
            timing: ExternalPluginSandboxTiming::Disabled,
            require_platform_sandbox: false,
            file_access: Vec::new(),
            protected_media_paths: Vec::new(),
            network: PluginSandboxNetworkGrant::AnyOutbound,
            local_authorizations: vec![PluginSandboxAuthorizationGrant::Any],
            child_processes: PluginSandboxChildProcessGrant::AllowAny,
            broker: PluginSandboxBrokerPolicy::NoPrompt,
        }
    }

    pub fn from_legacy(policy: &ExternalPluginSandboxPolicy) -> Self {
        let mut file_access = Vec::new();
        file_access.push(PluginSandboxFileGrant::PluginBundleReadExecute);
        file_access.extend(
            policy
                .extra_read_paths
                .iter()
                .cloned()
                .map(|path| PluginSandboxFileGrant::ReadOnlyPath { path }),
        );
        file_access.extend(
            policy
                .extra_write_paths
                .iter()
                .cloned()
                .map(|path| PluginSandboxFileGrant::ReadWritePath { path }),
        );

        Self {
            timing: policy.timing,
            require_platform_sandbox: policy.require_platform_sandbox,
            file_access,
            protected_media_paths: Vec::new(),
            network: if policy.allow_network {
                PluginSandboxNetworkGrant::AnyOutbound
            } else {
                PluginSandboxNetworkGrant::Deny
            },
            local_authorizations: Vec::new(),
            child_processes: if policy.allow_child_processes {
                PluginSandboxChildProcessGrant::AllowAny
            } else {
                PluginSandboxChildProcessGrant::Deny
            },
            broker: PluginSandboxBrokerPolicy::PromptAndRestart,
        }
    }

    pub fn to_legacy_policy(&self) -> ExternalPluginSandboxPolicy {
        let mut extra_read_paths = Vec::new();
        let mut extra_write_paths = Vec::new();

        for grant in &self.file_access {
            if self.file_grant_protected_overlap(grant).is_some() {
                continue;
            }
            match grant {
                PluginSandboxFileGrant::PluginBundleReadExecute => {}
                PluginSandboxFileGrant::PresetDirectoryReadWrite { path }
                | PluginSandboxFileGrant::ReadWritePath { path } => {
                    extra_write_paths.push(path.clone());
                }
                PluginSandboxFileGrant::ReadOnlyPath { path } => {
                    extra_read_paths.push(path.clone());
                }
            }
        }

        ExternalPluginSandboxPolicy {
            timing: self.timing,
            require_platform_sandbox: self.require_platform_sandbox,
            allow_network: self.network.allows_any_outbound(),
            allow_child_processes: self.child_processes.allows_any_child_process(),
            extra_read_paths,
            extra_write_paths,
        }
    }

    pub fn command_args(&self) -> Result<Vec<String>, String> {
        self.command_args_for_launch_plan(&self.current_backend_launch_plan())
    }

    pub fn command_args_for_backend(
        &self,
        backend: PluginSandboxLaunchBackend,
    ) -> Result<Vec<String>, String> {
        self.command_args_for_launch_plan(&self.launch_plan(backend))
    }

    pub fn command_args_for_launch_plan(
        &self,
        plan: &PluginSandboxLaunchPlan,
    ) -> Result<Vec<String>, String> {
        self.validate_protected_media_paths()?;
        plan.validate_for_launch(self)?;
        let json = serde_json::to_string(self)
            .map_err(|err| format!("failed to serialize plugin sandbox policy: {err}"))?;
        Ok(vec!["--sandbox-policy-json".to_string(), json])
    }

    pub fn launch_plan(&self, backend: PluginSandboxLaunchBackend) -> PluginSandboxLaunchPlan {
        let capabilities = backend.capabilities();
        PluginSandboxLaunchPlan {
            backend,
            capabilities,
            support_issues: self.support_issues(capabilities),
            adapter_issues: self.legacy_worker_adapter_issues(),
        }
    }

    pub fn current_backend_launch_plan(&self) -> PluginSandboxLaunchPlan {
        self.launch_plan(current_plugin_sandbox_launch_backend())
    }

    pub fn legacy_worker_adapter_issues(&self) -> Vec<PluginSandboxPolicyAdapterIssue> {
        if self.timing == ExternalPluginSandboxTiming::Disabled {
            return Vec::new();
        }

        let mut issues = Vec::new();
        match &self.network {
            PluginSandboxNetworkGrant::Deny | PluginSandboxNetworkGrant::AnyOutbound => {}
            grant => issues.push(
                PluginSandboxPolicyAdapterIssue::GranularNetworkUnsupported {
                    grant: grant.clone(),
                },
            ),
        }
        if let PluginSandboxChildProcessGrant::AllowSignedHelpers { paths } = &self.child_processes
        {
            issues.push(
                PluginSandboxPolicyAdapterIssue::SignedHelperProcessesUnsupported {
                    paths: paths.clone(),
                },
            );
        }
        issues
    }

    pub fn validate_legacy_worker_adapter(&self) -> Result<(), String> {
        let issues = self.legacy_worker_adapter_issues();
        if issues.is_empty() {
            return Ok(());
        }

        let summary = issues
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        Err(format!(
            "portable plugin sandbox policy cannot be represented by the current worker adapter: {summary}"
        ))
    }

    pub fn apply_user_grants<'a>(
        &mut self,
        grants: impl IntoIterator<Item = &'a PluginSandboxUserGrant>,
    ) {
        for grant in grants {
            self.apply_permission(&grant.permission);
        }
    }

    pub fn apply_import_user_grants<'a>(
        &mut self,
        grants: impl IntoIterator<Item = &'a PluginSandboxUserGrant>,
    ) {
        for grant in grants {
            if !self.permission_overlaps_protected_media(&grant.permission) {
                self.apply_permission(&grant.permission);
            }
        }
    }

    pub fn apply_permission(&mut self, permission: &PluginSandboxPermission) {
        match permission {
            PluginSandboxPermission::ReadPath { path } => {
                push_unique(
                    &mut self.file_access,
                    PluginSandboxFileGrant::ReadOnlyPath { path: path.clone() },
                );
            }
            PluginSandboxPermission::WritePath { path } => {
                push_unique(
                    &mut self.file_access,
                    PluginSandboxFileGrant::ReadWritePath { path: path.clone() },
                );
            }
            PluginSandboxPermission::Network(grant) => {
                self.network = grant.clone();
            }
            PluginSandboxPermission::LocalAuthorization(grant) => {
                push_unique(&mut self.local_authorizations, grant.clone());
            }
            PluginSandboxPermission::ChildProcess(grant) => {
                self.child_processes = grant.clone();
            }
        }
    }

    pub fn validate_protected_media_paths(&self) -> Result<(), String> {
        let overlaps = self
            .file_access
            .iter()
            .filter_map(|grant| self.file_grant_protected_overlap(grant))
            .collect::<Vec<_>>();
        if overlaps.is_empty() {
            return Ok(());
        }

        Err(format!(
            "plugin sandbox policy grants protected media path access during import: {}",
            overlaps
                .iter()
                .map(|(granted, protected)| {
                    format!("{} overlaps {}", granted.display(), protected.display())
                })
                .collect::<Vec<_>>()
                .join("; ")
        ))
    }

    fn permission_overlaps_protected_media(&self, permission: &PluginSandboxPermission) -> bool {
        match permission {
            PluginSandboxPermission::ReadPath { path }
            | PluginSandboxPermission::WritePath { path } => self
                .protected_media_paths
                .iter()
                .any(|protected| paths_overlap(path, protected)),
            _ => false,
        }
    }

    fn file_grant_protected_overlap(
        &self,
        grant: &PluginSandboxFileGrant,
    ) -> Option<(PathBuf, PathBuf)> {
        let path = match grant {
            PluginSandboxFileGrant::ReadOnlyPath { path }
            | PluginSandboxFileGrant::ReadWritePath { path } => path,
            PluginSandboxFileGrant::PluginBundleReadExecute
            | PluginSandboxFileGrant::PresetDirectoryReadWrite { .. } => return None,
        };
        self.protected_media_paths
            .iter()
            .find(|protected| paths_overlap(path, protected))
            .map(|protected| (path.clone(), protected.clone()))
    }

    pub fn support_issues(
        &self,
        capabilities: PluginSandboxBackendCapabilities,
    ) -> Vec<PluginSandboxPolicySupportIssue> {
        if self.timing == ExternalPluginSandboxTiming::Disabled {
            return Vec::new();
        }

        let mut issues = Vec::new();
        if !capabilities.filesystem && !self.file_access.is_empty() {
            issues.push(PluginSandboxPolicySupportIssue::FilesystemAccessUnsupported);
        }
        if !capabilities.network && self.network != PluginSandboxNetworkGrant::Deny {
            issues.push(PluginSandboxPolicySupportIssue::NetworkGrantUnsupported {
                grant: self.network.clone(),
            });
        }
        if !capabilities.local_authorization_profiles {
            for grant in &self.local_authorizations {
                issues.push(
                    PluginSandboxPolicySupportIssue::LocalAuthorizationUnsupported {
                        grant: grant.clone(),
                    },
                );
            }
        }
        if !capabilities.child_process_control
            && self.child_processes != PluginSandboxChildProcessGrant::Deny
        {
            issues.push(
                PluginSandboxPolicySupportIssue::ChildProcessGrantUnsupported {
                    grant: self.child_processes.clone(),
                },
            );
        }
        if !capabilities.prompt_without_restart
            && self.broker == PluginSandboxBrokerPolicy::ReportOnly
        {
            issues.push(PluginSandboxPolicySupportIssue::PromptWithoutRestartUnsupported);
        }
        issues
    }

    pub fn current_backend_support_issues(&self) -> Vec<PluginSandboxPolicySupportIssue> {
        self.support_issues(current_plugin_sandbox_backend_capabilities())
    }

    pub fn is_supported_by(&self, capabilities: PluginSandboxBackendCapabilities) -> bool {
        self.support_issues(capabilities).is_empty()
    }
}

impl From<&ExternalPluginSandboxPolicy> for PluginSandboxPolicy {
    fn from(policy: &ExternalPluginSandboxPolicy) -> Self {
        Self::from_legacy(policy)
    }
}

impl From<&PluginSandboxPolicy> for ExternalPluginSandboxPolicy {
    fn from(policy: &PluginSandboxPolicy) -> Self {
        policy.to_legacy_policy()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxFileGrant {
    PluginBundleReadExecute,
    PresetDirectoryReadWrite { path: PathBuf },
    ReadOnlyPath { path: PathBuf },
    ReadWritePath { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxNetworkGrant {
    Deny,
    LoopbackOnly,
    RemoteTcp { hosts: Vec<String>, ports: Vec<u16> },
    AnyOutbound,
}

impl PluginSandboxNetworkGrant {
    pub fn allows_any_outbound(&self) -> bool {
        matches!(self, Self::AnyOutbound)
    }

    pub fn satisfies(&self, requested: &Self) -> bool {
        match (self, requested) {
            (Self::AnyOutbound, Self::Deny | Self::LoopbackOnly | Self::RemoteTcp { .. }) => true,
            (Self::AnyOutbound, Self::AnyOutbound) => true,
            (Self::LoopbackOnly, Self::Deny | Self::LoopbackOnly) => true,
            (
                Self::RemoteTcp {
                    hosts: granted_hosts,
                    ports: granted_ports,
                },
                Self::RemoteTcp {
                    hosts: requested_hosts,
                    ports: requested_ports,
                },
            ) => {
                requested_hosts
                    .iter()
                    .all(|host| granted_hosts.contains(host))
                    && requested_ports
                        .iter()
                        .all(|port| granted_ports.contains(port))
            }
            (Self::Deny, Self::Deny) => true,
            _ => false,
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginSandboxBrokerPolicy {
    NoPrompt,
    PromptAndRestart,
    ReportOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginSandboxPolicySupportIssue {
    FilesystemAccessUnsupported,
    NetworkGrantUnsupported {
        grant: PluginSandboxNetworkGrant,
    },
    LocalAuthorizationUnsupported {
        grant: PluginSandboxAuthorizationGrant,
    },
    ChildProcessGrantUnsupported {
        grant: PluginSandboxChildProcessGrant,
    },
    PromptWithoutRestartUnsupported,
}

impl std::fmt::Display for PluginSandboxPolicySupportIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilesystemAccessUnsupported => {
                write!(f, "backend cannot enforce filesystem path grants")
            }
            Self::NetworkGrantUnsupported { grant } => {
                write!(f, "backend cannot enforce network grant {grant:?}")
            }
            Self::LocalAuthorizationUnsupported { grant } => {
                write!(
                    f,
                    "backend cannot enforce local authorization grant {grant:?}"
                )
            }
            Self::ChildProcessGrantUnsupported { grant } => {
                write!(f, "backend cannot enforce child-process grant {grant:?}")
            }
            Self::PromptWithoutRestartUnsupported => {
                write!(
                    f,
                    "backend cannot prompt and update permissions without restart"
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginSandboxPolicyAdapterIssue {
    GranularNetworkUnsupported {
        grant: PluginSandboxNetworkGrant,
    },
    LocalAuthorizationUnsupported {
        grant: PluginSandboxAuthorizationGrant,
    },
    SignedHelperProcessesUnsupported {
        paths: Vec<PathBuf>,
    },
}

impl std::fmt::Display for PluginSandboxPolicyAdapterIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GranularNetworkUnsupported { grant } => {
                write!(
                    f,
                    "current worker adapter supports only denied or unrestricted network, not {grant:?}"
                )
            }
            Self::LocalAuthorizationUnsupported { grant } => {
                write!(
                    f,
                    "current worker adapter cannot represent local authorization grant {grant:?}"
                )
            }
            Self::SignedHelperProcessesUnsupported { paths } => {
                write!(
                    f,
                    "current worker adapter cannot restrict child processes to signed helpers {paths:?}"
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginSandboxBackendCapabilities {
    pub filesystem: bool,
    pub network: bool,
    pub local_authorization_profiles: bool,
    pub child_process_control: bool,
    pub prompt_without_restart: bool,
    pub store_compatible: bool,
}

pub trait PluginSandboxBackend {
    fn backend_id(&self) -> &'static str;
    fn capabilities(&self) -> PluginSandboxBackendCapabilities;
}

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

pub trait PluginSandboxPermissionBroker {
    fn decide_permission(
        &mut self,
        request: PluginSandboxPermissionRequest,
    ) -> PluginSandboxPermissionDecision;
}

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

pub fn current_plugin_sandbox_backend_capabilities() -> PluginSandboxBackendCapabilities {
    platform::capabilities()
}

pub fn current_plugin_sandbox_launch_backend() -> PluginSandboxLaunchBackend {
    platform::launch_backend()
}

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

pub fn current_plugin_sandbox_launcher_command() -> Option<ExternalPluginWorkerCommand> {
    default_plugin_sandbox_launcher_command_for_backend(current_plugin_sandbox_launch_backend())
}

fn push_unique<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn dedupe_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        push_unique(&mut deduped, normalize_path_for_policy(&path));
    }
    deduped
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    let left = normalize_path_for_policy(left);
    let right = normalize_path_for_policy(right);
    left.starts_with(&right) || right.starts_with(&left)
}

fn normalize_path_for_policy(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }

    let mut missing_components = Vec::new();
    let mut existing = path;
    while let Some(parent) = existing.parent() {
        if let Some(name) = existing.file_name() {
            missing_components.push(name.to_os_string());
        }
        if let Ok(mut canonical_parent) = parent.canonicalize() {
            for component in missing_components.iter().rev() {
                canonical_parent.push(component);
            }
            return canonical_parent;
        }
        existing = parent;
    }

    path.to_path_buf()
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

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .or_else(|| {
                let drive = std::env::var_os("HOMEDRIVE")?;
                let path = std::env::var_os("HOMEPATH")?;
                let mut home = PathBuf::from(drive);
                home.push(path);
                Some(home.into_os_string())
            })
            .map(PathBuf::from)
    }

    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn sanitize_path_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "external-plugin".to_string()
    } else {
        sanitized.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExternalPluginSandboxTiming {
    Disabled,
    BeforePluginLoad,
    AfterPluginLoad,
}

impl ExternalPluginSandboxTiming {
    pub fn as_arg(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::BeforePluginLoad => "before-plugin-load",
            Self::AfterPluginLoad => "after-plugin-load",
        }
    }
}

impl FromStr for ExternalPluginSandboxTiming {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "disabled" | "off" | "none" => Ok(Self::Disabled),
            "before-plugin-load" | "before_load" | "before-load" | "pre_load" | "preload" => {
                Ok(Self::BeforePluginLoad)
            }
            "after-plugin-load" | "after_load" | "after-load" | "post_load" | "postload" => {
                Ok(Self::AfterPluginLoad)
            }
            other => Err(format!("unknown external-plugin sandbox timing '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginTrust {
    Unknown,
    Untrusted,
    Signed,
}

impl FromStr for ExternalPluginTrust {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "unknown" => Ok(Self::Unknown),
            "untrusted" => Ok(Self::Untrusted),
            "signed" | "trusted" | "known" => Ok(Self::Signed),
            other => Err(format!("unknown external-plugin trust value '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPluginSandboxPolicy {
    pub timing: ExternalPluginSandboxTiming,
    pub require_platform_sandbox: bool,
    pub allow_network: bool,
    pub allow_child_processes: bool,
    pub extra_read_paths: Vec<PathBuf>,
    pub extra_write_paths: Vec<PathBuf>,
}

impl ExternalPluginSandboxPolicy {
    pub fn disabled() -> Self {
        Self {
            timing: ExternalPluginSandboxTiming::Disabled,
            require_platform_sandbox: false,
            allow_network: true,
            allow_child_processes: true,
            extra_read_paths: Vec::new(),
            extra_write_paths: Vec::new(),
        }
    }

    pub fn for_trust(trust: ExternalPluginTrust) -> Self {
        match trust {
            ExternalPluginTrust::Signed => Self {
                timing: ExternalPluginSandboxTiming::AfterPluginLoad,
                require_platform_sandbox: false,
                allow_network: false,
                allow_child_processes: false,
                extra_read_paths: Vec::new(),
                extra_write_paths: Vec::new(),
            },
            ExternalPluginTrust::Unknown | ExternalPluginTrust::Untrusted => Self {
                timing: ExternalPluginSandboxTiming::BeforePluginLoad,
                require_platform_sandbox: should_require_platform_sandbox(trust),
                allow_network: false,
                allow_child_processes: false,
                extra_read_paths: Vec::new(),
                extra_write_paths: Vec::new(),
            },
        }
    }

    pub fn command_args(&self) -> Vec<String> {
        let mut args = vec![
            "--sandbox-timing".to_string(),
            self.timing.as_arg().to_string(),
        ];

        if self.require_platform_sandbox {
            args.push("--sandbox-required".to_string());
        }
        if self.allow_network {
            args.push("--sandbox-allow-network".to_string());
        }
        if self.allow_child_processes {
            args.push("--sandbox-allow-child-processes".to_string());
        }

        for path in &self.extra_read_paths {
            args.push("--sandbox-read-path".to_string());
            args.push(path.display().to_string());
        }
        for path in &self.extra_write_paths {
            args.push("--sandbox-write-path".to_string());
            args.push(path.display().to_string());
        }

        args
    }
}

const fn should_require_platform_sandbox(trust: ExternalPluginTrust) -> bool {
    match trust {
        ExternalPluginTrust::Signed => false,
        ExternalPluginTrust::Unknown | ExternalPluginTrust::Untrusted => {
            cfg!(any(
                target_os = "linux",
                target_os = "macos",
                target_os = "windows"
            ))
        }
    }
}

impl Default for ExternalPluginSandboxPolicy {
    fn default() -> Self {
        Self::for_trust(ExternalPluginTrust::Unknown)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalPluginSandboxStatus {
    Disabled,
    Enforced {
        backend: &'static str,
    },
    Unsupported {
        backend: &'static str,
        reason: String,
    },
}

impl ExternalPluginSandboxStatus {
    pub fn is_enforced(&self) -> bool {
        matches!(self, Self::Enforced { .. })
    }
}

pub fn enter_external_plugin_sandbox(
    policy: &ExternalPluginSandboxPolicy,
    descriptor: &PluginDescriptor,
    shared_memory_path: &Path,
) -> Result<ExternalPluginSandboxStatus, String> {
    if policy.timing == ExternalPluginSandboxTiming::Disabled {
        return Ok(ExternalPluginSandboxStatus::Disabled);
    }

    let status = platform::enter(policy, descriptor, shared_memory_path)?;
    if policy.require_platform_sandbox && !status.is_enforced() {
        return Err(format!(
            "external-plugin sandbox is required but was not enforced: {status:?}"
        ));
    }
    Ok(status)
}

#[cfg(target_os = "linux")]
mod platform {
    use std::ffi::CString;
    use std::mem;
    use std::os::fd::RawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use crate::external_plugin::PluginDescriptor;

    use super::{ExternalPluginSandboxPolicy, ExternalPluginSandboxStatus};

    pub fn launch_backend() -> super::PluginSandboxLaunchBackend {
        super::PluginSandboxLaunchBackend::LinuxLandlockWorker
    }

    pub fn capabilities() -> super::PluginSandboxBackendCapabilities {
        launch_backend().capabilities()
    }

    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
    const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

    const FS_EXECUTE: u64 = 1 << 0;
    const FS_WRITE_FILE: u64 = 1 << 1;
    const FS_READ_FILE: u64 = 1 << 2;
    const FS_READ_DIR: u64 = 1 << 3;
    const FS_REMOVE_DIR: u64 = 1 << 4;
    const FS_REMOVE_FILE: u64 = 1 << 5;
    const FS_MAKE_CHAR: u64 = 1 << 6;
    const FS_MAKE_DIR: u64 = 1 << 7;
    const FS_MAKE_REG: u64 = 1 << 8;
    const FS_MAKE_SOCK: u64 = 1 << 9;
    const FS_MAKE_FIFO: u64 = 1 << 10;
    const FS_MAKE_BLOCK: u64 = 1 << 11;
    const FS_MAKE_SYM: u64 = 1 << 12;
    const FS_REFER: u64 = 1 << 13;
    const FS_TRUNCATE: u64 = 1 << 14;

    const NET_BIND_TCP: u64 = 1 << 0;
    const NET_CONNECT_TCP: u64 = 1 << 1;

    #[repr(C)]
    struct LandlockRulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
        scoped: u64,
    }

    #[repr(C)]
    struct LandlockPathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    pub fn enter(
        policy: &ExternalPluginSandboxPolicy,
        descriptor: &PluginDescriptor,
        shared_memory_path: &Path,
    ) -> Result<ExternalPluginSandboxStatus, String> {
        let abi = landlock_abi()?;
        if abi <= 0 {
            set_no_new_privs()?;
            return Ok(ExternalPluginSandboxStatus::Unsupported {
                backend: "linux-landlock",
                reason: "Landlock is not supported or disabled by the running kernel".to_string(),
            });
        }

        let handled_access_fs = fs_access_mask_for_abi(abi);
        let handled_access_net = if abi >= 4 && !policy.allow_network {
            NET_BIND_TCP | NET_CONNECT_TCP
        } else {
            0
        };
        let mut unsupported_reasons = Vec::new();
        if abi < 4 && !policy.allow_network {
            unsupported_reasons.push("network denial requires Landlock ABI 4 or newer".to_string());
        }
        if !policy.allow_child_processes {
            unsupported_reasons.push(
                "child-process denial requires a seccomp/job-control backend not present in this build"
                    .to_string(),
            );
        }
        let ruleset_attr = LandlockRulesetAttr {
            handled_access_fs,
            handled_access_net,
            scoped: 0,
        };

        let ruleset_fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &ruleset_attr,
                mem::size_of::<LandlockRulesetAttr>(),
                0,
            ) as RawFd
        };
        if ruleset_fd < 0 {
            return Err(format!(
                "failed to create Landlock ruleset: {}",
                std::io::Error::last_os_error()
            ));
        }

        let result = apply_rules(
            policy,
            descriptor,
            shared_memory_path,
            ruleset_fd,
            handled_access_fs,
        )
        .and_then(|_| restrict_self(ruleset_fd));
        unsafe {
            libc::close(ruleset_fd);
        }
        result?;

        if !unsupported_reasons.is_empty() {
            return Ok(ExternalPluginSandboxStatus::Unsupported {
                backend: "linux-landlock",
                reason: unsupported_reasons.join("; "),
            });
        }

        Ok(ExternalPluginSandboxStatus::Enforced {
            backend: "linux-landlock",
        })
    }

    fn apply_rules(
        policy: &ExternalPluginSandboxPolicy,
        descriptor: &PluginDescriptor,
        shared_memory_path: &Path,
        ruleset_fd: RawFd,
        handled_access_fs: u64,
    ) -> Result<(), String> {
        add_path_rule(
            ruleset_fd,
            &descriptor.path,
            (FS_READ_FILE | FS_READ_DIR | FS_EXECUTE) & handled_access_fs,
        )?;

        add_path_rule(
            ruleset_fd,
            shared_memory_path,
            (FS_READ_FILE | FS_WRITE_FILE | FS_TRUNCATE) & handled_access_fs,
        )?;

        for path in &policy.extra_read_paths {
            add_path_rule(
                ruleset_fd,
                path,
                (FS_READ_FILE | FS_READ_DIR | FS_EXECUTE) & handled_access_fs,
            )?;
        }
        for path in &policy.extra_write_paths {
            add_path_rule(ruleset_fd, path, writable_access() & handled_access_fs)?;
        }

        Ok(())
    }

    fn writable_access() -> u64 {
        FS_READ_FILE
            | FS_WRITE_FILE
            | FS_READ_DIR
            | FS_REMOVE_DIR
            | FS_REMOVE_FILE
            | FS_MAKE_CHAR
            | FS_MAKE_DIR
            | FS_MAKE_REG
            | FS_MAKE_SOCK
            | FS_MAKE_FIFO
            | FS_MAKE_BLOCK
            | FS_MAKE_SYM
            | FS_REFER
            | FS_TRUNCATE
    }

    fn fs_access_mask_for_abi(abi: i32) -> u64 {
        let mut mask = writable_access() | FS_EXECUTE;
        if abi < 2 {
            mask &= !FS_REFER;
        }
        if abi < 3 {
            mask &= !FS_TRUNCATE;
        }
        mask
    }

    fn add_path_rule(ruleset_fd: RawFd, path: &Path, access: u64) -> Result<(), String> {
        if access == 0 {
            return Ok(());
        }
        let path = canonicalize_if_possible(path);
        let fd = open_path_fd(&path)?;
        let rule = LandlockPathBeneathAttr {
            allowed_access: access,
            parent_fd: fd,
        };
        let result = unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                ruleset_fd,
                LANDLOCK_RULE_PATH_BENEATH,
                &rule,
                0,
            )
        };
        unsafe {
            libc::close(fd);
        }
        if result < 0 {
            return Err(format!(
                "failed to add Landlock path rule for '{}': {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn canonicalize_if_possible(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn open_path_fd(path: &Path) -> Result<RawFd, String> {
        let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            format!(
                "cannot add sandbox path with interior NUL byte: '{}'",
                path.display()
            )
        })?;
        let fd = unsafe {
            libc::open(
                c_path.as_ptr(),
                libc::O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(format!(
                "failed to open sandbox path '{}': {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(fd)
    }

    fn restrict_self(ruleset_fd: RawFd) -> Result<(), String> {
        set_no_new_privs()?;
        let result = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0) };
        if result < 0 {
            return Err(format!(
                "failed to enter Landlock sandbox: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn landlock_abi() -> Result<i32, String> {
        let abi = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<LandlockRulesetAttr>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if abi < 0 {
            let err = std::io::Error::last_os_error();
            let code = err.raw_os_error().unwrap_or_default();
            if code == libc::ENOSYS || code == libc::EOPNOTSUPP {
                return Ok(0);
            }
            return Err(format!("failed to query Landlock ABI: {err}"));
        }
        Ok(abi as i32)
    }

    fn set_no_new_privs() -> Result<(), String> {
        let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if result != 0 {
            return Err(format!(
                "failed to set no_new_privs before sandboxing external plugin: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    #[cfg(target_os = "macos")]
    const BACKEND_NAME: &str = "macos-process-isolation";
    #[cfg(target_os = "macos")]
    const MACOS_HELPER_BACKEND_NAME: &str = "macos-app-sandbox-helper";
    #[cfg(target_os = "macos")]
    const MACOS_APP_SANDBOX_CONTAINER_ENV: &str = "APP_SANDBOX_CONTAINER_ID";
    #[cfg(target_os = "windows")]
    const BACKEND_NAME: &str = "windows-process-isolation";
    #[cfg(target_os = "macos")]
    const BACKEND_NOTE: &str =
        "macOS native sandbox backend is unavailable in this build; worker uses process isolation";
    #[cfg(target_os = "windows")]
    const BACKEND_NOTE: &str = "Windows native sandbox backend is unavailable in this build; worker uses process isolation";

    #[cfg(target_os = "macos")]
    use std::ffi::OsStr;
    use std::fs::OpenOptions;
    use std::path::Path;

    use crate::external_plugin::PluginDescriptor;
    use crate::external_plugin_ipc;

    use super::{ExternalPluginSandboxPolicy, ExternalPluginSandboxStatus};

    #[cfg(target_os = "macos")]
    pub(super) fn macos_launch_backend_from_container_id(
        container_id: Option<&OsStr>,
    ) -> super::PluginSandboxLaunchBackend {
        if container_id
            .and_then(OsStr::to_str)
            .is_some_and(|id| !id.is_empty())
        {
            return super::PluginSandboxLaunchBackend::MacosAppSandboxHelper;
        }

        super::PluginSandboxLaunchBackend::ProcessIsolationOnly {
            platform: BACKEND_NAME,
        }
    }

    pub fn launch_backend() -> super::PluginSandboxLaunchBackend {
        #[cfg(target_os = "macos")]
        {
            macos_launch_backend_from_container_id(
                std::env::var_os(MACOS_APP_SANDBOX_CONTAINER_ENV).as_deref(),
            )
        }

        #[cfg(not(target_os = "macos"))]
        {
            super::PluginSandboxLaunchBackend::ProcessIsolationOnly {
                platform: BACKEND_NAME,
            }
        }
    }

    pub fn capabilities() -> super::PluginSandboxBackendCapabilities {
        launch_backend().capabilities()
    }

    pub fn enter(
        _policy: &ExternalPluginSandboxPolicy,
        _descriptor: &PluginDescriptor,
        _shared_memory_path: &Path,
    ) -> Result<ExternalPluginSandboxStatus, String> {
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        let file = options
            .open(_shared_memory_path)
            .map_err(|err| format!("shared memory is not accessible: {err}"))?;

        external_plugin_ipc::validate_shared_memory_file(&file, _shared_memory_path)
            .map_err(|err| format!("shared memory failed sandbox integrity check: {err}"))?;

        #[cfg(target_os = "macos")]
        if std::env::var_os(super::MACOS_APP_SANDBOX_HELPER_ENV).is_some() {
            return Ok(ExternalPluginSandboxStatus::Enforced {
                backend: MACOS_HELPER_BACKEND_NAME,
            });
        }

        Ok(ExternalPluginSandboxStatus::Unsupported {
            backend: BACKEND_NAME,
            reason: BACKEND_NOTE.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_plugin::{PluginDescriptor, PluginFormat};
    use crate::external_plugin_ipc::{PluginIpcLayout, SecurePluginSharedMemory};

    fn descriptor(id: &str) -> PluginDescriptor {
        PluginDescriptor {
            id: id.into(),
            name: "sandbox-test".into(),
            vendor: "test vendor".into(),
            version: "0.1".into(),
            format: PluginFormat::Clap,
            path: "/tmp/sandbox-test.clap".into(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: crate::external_plugin::PluginScanStatus::Discovered,
        }
    }

    #[test]
    fn trust_maps_to_expected_sandbox_timing() {
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Signed).timing,
            ExternalPluginSandboxTiming::AfterPluginLoad
        );
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown).timing,
            ExternalPluginSandboxTiming::BeforePluginLoad
        );
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Untrusted).timing,
            ExternalPluginSandboxTiming::BeforePluginLoad
        );
    }

    #[test]
    fn untrusted_requires_platform_enforcement() {
        assert_eq!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Untrusted)
                .require_platform_sandbox,
            cfg!(any(
                target_os = "linux",
                target_os = "macos",
                target_os = "windows"
            ))
        );
        assert!(
            ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown)
                .require_platform_sandbox
                == cfg!(any(
                    target_os = "linux",
                    target_os = "macos",
                    target_os = "windows"
                ))
        );
        assert!(
            !ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Signed)
                .require_platform_sandbox
        );
    }

    #[test]
    fn sandbox_timing_parses_compat_aliases() {
        assert_eq!(
            "pre_load".parse::<ExternalPluginSandboxTiming>().unwrap(),
            ExternalPluginSandboxTiming::BeforePluginLoad
        );
        assert_eq!(
            "after-load".parse::<ExternalPluginSandboxTiming>().unwrap(),
            ExternalPluginSandboxTiming::AfterPluginLoad
        );
    }

    #[test]
    fn strict_portable_policy_allows_only_plugin_and_preset_directory_by_default() {
        let policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");

        assert_eq!(policy.timing, ExternalPluginSandboxTiming::BeforePluginLoad);
        assert_eq!(policy.network, PluginSandboxNetworkGrant::Deny);
        assert_eq!(policy.child_processes, PluginSandboxChildProcessGrant::Deny);
        assert_eq!(policy.local_authorizations, Vec::new());
        assert_eq!(
            policy.file_access,
            vec![
                PluginSandboxFileGrant::PluginBundleReadExecute,
                PluginSandboxFileGrant::PresetDirectoryReadWrite {
                    path: PathBuf::from("/tmp/sotf-presets")
                },
            ]
        );
    }

    #[test]
    fn portable_policy_converts_to_legacy_worker_policy() {
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        policy
            .file_access
            .push(PluginSandboxFileGrant::ReadOnlyPath {
                path: PathBuf::from("/tmp/read-only"),
            });

        assert!(policy.validate_legacy_worker_adapter().is_ok());
        let legacy = policy.to_legacy_policy();
        assert!(!legacy.allow_network);
        assert!(!legacy.allow_child_processes);
        assert_eq!(
            legacy.extra_read_paths,
            vec![PathBuf::from("/tmp/read-only")]
        );
        assert_eq!(
            legacy.extra_write_paths,
            vec![PathBuf::from("/tmp/sotf-presets")]
        );
    }

    #[test]
    fn portable_policy_reports_legacy_worker_adapter_gaps() {
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        policy.require_platform_sandbox = false;
        policy.network = PluginSandboxNetworkGrant::LoopbackOnly;
        policy.local_authorizations = vec![PluginSandboxAuthorizationGrant::Pace];
        policy.child_processes = PluginSandboxChildProcessGrant::AllowSignedHelpers {
            paths: vec![PathBuf::from("/tmp/helper")],
        };

        let issues = policy.legacy_worker_adapter_issues();
        assert_eq!(
            issues,
            vec![
                PluginSandboxPolicyAdapterIssue::GranularNetworkUnsupported {
                    grant: PluginSandboxNetworkGrant::LoopbackOnly,
                },
                PluginSandboxPolicyAdapterIssue::SignedHelperProcessesUnsupported {
                    paths: vec![PathBuf::from("/tmp/helper")],
                },
            ]
        );
        let err = policy.command_args().unwrap_err();
        assert!(err.contains("cannot launch current worker policy"));
    }

    #[test]
    fn portable_policy_command_args_round_trip() {
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        policy.require_platform_sandbox = false;
        let args = policy.command_args().unwrap();

        assert_eq!(args[0], "--sandbox-policy-json");
        let decoded: PluginSandboxPolicy = serde_json::from_str(&args[1]).unwrap();
        assert_eq!(decoded, policy);
    }

    #[test]
    fn portable_policy_command_args_can_target_selected_backend() {
        let policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        let args = policy
            .command_args_for_backend(PluginSandboxLaunchBackend::MacosAppSandboxHelper)
            .unwrap();

        assert_eq!(args[0], "--sandbox-policy-json");
        let decoded: PluginSandboxPolicy = serde_json::from_str(&args[1]).unwrap();
        assert_eq!(decoded, policy);
    }

    #[test]
    fn default_launcher_command_for_macos_helper_uses_helper_binary() {
        let launcher = default_plugin_sandbox_launcher_command_for_backend(
            PluginSandboxLaunchBackend::MacosAppSandboxHelper,
        )
        .unwrap();

        assert!(
            launcher
                .program()
                .ends_with(ExternalPluginWorkerCommand::DEFAULT_MACOS_SANDBOX_HELPER_BINARY)
        );
        assert!(
            default_plugin_sandbox_launcher_command_for_backend(
                PluginSandboxLaunchBackend::LinuxLandlockWorker
            )
            .is_none()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_app_sandbox_container_selects_helper_backend() {
        assert_eq!(
            super::platform::macos_launch_backend_from_container_id(Some(std::ffi::OsStr::new(
                "org.spinorama.sotf"
            ))),
            PluginSandboxLaunchBackend::MacosAppSandboxHelper
        );
        assert_eq!(
            super::platform::macos_launch_backend_from_container_id(Some(std::ffi::OsStr::new(""))),
            PluginSandboxLaunchBackend::ProcessIsolationOnly {
                platform: "macos-process-isolation"
            }
        );
        assert_eq!(
            super::platform::macos_launch_backend_from_container_id(None),
            PluginSandboxLaunchBackend::ProcessIsolationOnly {
                platform: "macos-process-isolation"
            }
        );
    }

    #[test]
    fn portable_policy_command_args_reject_selected_process_only_backend() {
        let policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        let err = policy
            .command_args_for_backend(PluginSandboxLaunchBackend::ProcessIsolationOnly {
                platform: "test-process-only",
            })
            .unwrap_err();

        assert!(err.contains("cannot satisfy required policy"));
    }

    #[test]
    fn plugin_identity_uses_store_safe_preset_component() {
        let descriptor = PluginDescriptor {
            id: "com.test/plugin:unsafe".into(),
            name: "sandbox-test".into(),
            vendor: "test vendor".into(),
            version: "0.1".into(),
            format: PluginFormat::Clap,
            path: "/tmp/sandbox-test.clap".into(),
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: crate::external_plugin::PluginScanStatus::Discovered,
        };

        let identity = PluginSandboxIdentity::from_descriptor(&descriptor);
        assert_eq!(
            identity.stable_preset_component(),
            "Clap-test_vendor-com.test_plugin_unsafe"
        );
    }

    #[test]
    fn grant_store_builds_strict_per_plugin_policy() {
        let descriptor = descriptor("com.test.strict");
        let store = PluginSandboxGrantStore::default();

        let policy = store.strict_policy_for_plugin(&descriptor, "/tmp/sotf-presets");

        assert_eq!(policy.network, PluginSandboxNetworkGrant::Deny);
        assert_eq!(policy.child_processes, PluginSandboxChildProcessGrant::Deny);
        assert_eq!(
            policy.file_access,
            vec![
                PluginSandboxFileGrant::PluginBundleReadExecute,
                PluginSandboxFileGrant::PresetDirectoryReadWrite {
                    path: PathBuf::from("/tmp/sotf-presets/Clap-test_vendor-com.test.strict")
                },
            ]
        );
    }

    #[test]
    fn grant_store_applies_only_matching_plugin_grants() {
        let plugin_descriptor = descriptor("com.test.needs-network");
        let identity = PluginSandboxIdentity::from_descriptor(&plugin_descriptor);
        let other_identity = PluginSandboxIdentity::from_descriptor(&descriptor("com.test.other"));
        let mut store = PluginSandboxGrantStore::default();

        store.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::LoopbackOnly),
        });
        store.remember(PluginSandboxUserGrant {
            identity,
            permission: PluginSandboxPermission::LocalAuthorization(
                PluginSandboxAuthorizationGrant::Pace,
            ),
        });
        store.remember(PluginSandboxUserGrant {
            identity: other_identity,
            permission: PluginSandboxPermission::WritePath {
                path: PathBuf::from("/tmp/other"),
            },
        });

        let policy = store.strict_policy_for_plugin(&plugin_descriptor, "/tmp/sotf-presets");

        assert_eq!(policy.network, PluginSandboxNetworkGrant::LoopbackOnly);
        assert_eq!(
            policy.local_authorizations,
            vec![PluginSandboxAuthorizationGrant::Pace]
        );
        assert!(
            !policy
                .file_access
                .contains(&PluginSandboxFileGrant::ReadWritePath {
                    path: PathBuf::from("/tmp/other")
                })
        );
    }

    #[test]
    fn import_policy_filters_grants_that_overlap_protected_media() {
        let plugin_descriptor = descriptor("com.test.import");
        let identity = PluginSandboxIdentity::from_descriptor(&plugin_descriptor);
        let mut store = PluginSandboxGrantStore::default();
        store.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::ReadPath {
                path: PathBuf::from("/tmp/external-cache"),
            },
        });
        store.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::ReadPath {
                path: PathBuf::from("/tmp/music"),
            },
        });
        store.remember(PluginSandboxUserGrant {
            identity,
            permission: PluginSandboxPermission::WritePath {
                path: PathBuf::from("/tmp"),
            },
        });

        let policy = store.import_policy_for_plugin(
            &plugin_descriptor,
            "/tmp/sotf-presets",
            vec![PathBuf::from("/tmp/music")],
        );

        assert_eq!(policy.network, PluginSandboxNetworkGrant::AnyOutbound);
        assert!(
            policy
                .file_access
                .contains(&PluginSandboxFileGrant::ReadOnlyPath {
                    path: PathBuf::from("/tmp/external-cache")
                })
        );
        assert!(
            !policy
                .file_access
                .contains(&PluginSandboxFileGrant::ReadOnlyPath {
                    path: PathBuf::from("/tmp/music")
                })
        );
        assert!(
            !policy
                .file_access
                .contains(&PluginSandboxFileGrant::ReadWritePath {
                    path: PathBuf::from("/tmp")
                })
        );
        assert!(policy.validate_protected_media_paths().is_ok());
    }

    #[test]
    fn import_policy_rejects_manual_protected_media_overlap() {
        let mut policy = PluginSandboxPolicy::import_with_preset_dir_and_protected_media_paths(
            "/tmp/sotf-presets",
            vec![PathBuf::from("/tmp/music")],
        );
        policy
            .file_access
            .push(PluginSandboxFileGrant::ReadOnlyPath {
                path: PathBuf::from("/tmp"),
            });

        let err = policy.validate_protected_media_paths().unwrap_err();

        assert!(err.contains("overlaps"));
        assert!(err.contains("music"));
        assert!(
            policy
                .command_args()
                .unwrap_err()
                .contains("protected media")
        );
        assert_eq!(
            policy.to_legacy_policy().extra_read_paths,
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn authorized_runtime_policy_ignores_external_grants_and_allows_media() {
        let plugin_descriptor = descriptor("com.test.runtime");
        let identity = PluginSandboxIdentity::from_descriptor(&plugin_descriptor);
        let mut store = PluginSandboxGrantStore::default();
        store.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
        });
        store.remember(PluginSandboxUserGrant {
            identity,
            permission: PluginSandboxPermission::LocalAuthorization(
                PluginSandboxAuthorizationGrant::Pace,
            ),
        });

        let policy = store.authorized_runtime_policy_for_plugin(
            &plugin_descriptor,
            "/tmp/sotf-presets",
            vec![
                PathBuf::from("/tmp/music"),
                PathBuf::from("/tmp/wav"),
                PathBuf::from("/tmp/stems"),
            ],
        );

        assert_eq!(policy.network, PluginSandboxNetworkGrant::Deny);
        assert_eq!(policy.local_authorizations, Vec::new());
        assert_eq!(policy.child_processes, PluginSandboxChildProcessGrant::Deny);
        assert!(
            policy
                .file_access
                .contains(&PluginSandboxFileGrant::ReadOnlyPath {
                    path: PathBuf::from("/tmp/music")
                })
        );
        assert!(
            policy
                .file_access
                .contains(&PluginSandboxFileGrant::ReadOnlyPath {
                    path: PathBuf::from("/tmp/wav")
                })
        );
        assert!(
            policy
                .file_access
                .contains(&PluginSandboxFileGrant::ReadOnlyPath {
                    path: PathBuf::from("/tmp/stems")
                })
        );
    }

    #[test]
    fn grant_store_deduplicates_and_revokes_grants() {
        let identity = PluginSandboxIdentity::from_descriptor(&descriptor("com.test.dedupe"));
        let grant = PluginSandboxUserGrant {
            identity,
            permission: PluginSandboxPermission::ReadPath {
                path: PathBuf::from("/tmp/read"),
            },
        };
        let mut store = PluginSandboxGrantStore::default();

        store.remember(grant.clone());
        store.remember(grant.clone());
        assert_eq!(store.grants.len(), 1);
        assert!(store.revoke(&grant));
        assert!(!store.revoke(&grant));
        assert!(store.grants.is_empty());
    }

    #[test]
    fn grant_store_matches_broader_remembered_permissions() {
        let identity = PluginSandboxIdentity::from_descriptor(&descriptor("com.test.broad"));
        let mut store = PluginSandboxGrantStore::default();
        store.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
        });
        store.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::WritePath {
                path: PathBuf::from("/tmp/plugin-cache"),
            },
        });

        assert!(store.grants_permission(
            &identity,
            &PluginSandboxPermission::Network(PluginSandboxNetworkGrant::LoopbackOnly)
        ));
        assert!(store.grants_permission(
            &identity,
            &PluginSandboxPermission::ReadPath {
                path: PathBuf::from("/tmp/plugin-cache/preset.json")
            }
        ));
    }

    #[test]
    fn remembered_permission_decision_persists_and_requires_restart() {
        let descriptor = descriptor("com.test.prompt");
        let request = PluginSandboxPermissionRequest::from_descriptor(
            &descriptor,
            PluginSandboxPermission::LocalAuthorization(PluginSandboxAuthorizationGrant::Pace),
            Some("license check".to_string()),
        );

        let decision = request.grant_remembered();
        let mut store = PluginSandboxGrantStore::default();

        assert!(decision.restart_required);
        assert_eq!(
            decision.granted_permission(),
            Some(&PluginSandboxPermission::LocalAuthorization(
                PluginSandboxAuthorizationGrant::Pace
            ))
        );
        assert!(decision.apply_to_store(&mut store));
        assert_eq!(store.grants.len(), 1);
    }

    #[test]
    fn until_restart_permission_decision_does_not_persist() {
        let descriptor = descriptor("com.test.session");
        let request = PluginSandboxPermissionRequest::from_descriptor(
            &descriptor,
            PluginSandboxPermission::Network(PluginSandboxNetworkGrant::LoopbackOnly),
            None,
        );

        let decision = request.grant_until_restart();
        let mut store = PluginSandboxGrantStore::default();

        assert!(decision.restart_required);
        assert!(!decision.apply_to_store(&mut store));
        assert!(store.apply_session_decision(&decision));
        assert_eq!(store.grants.len(), 1);
    }

    #[test]
    fn default_permission_broker_denies_without_restart() {
        let descriptor = descriptor("com.test.default-deny");
        let request = PluginSandboxPermissionRequest::from_descriptor(
            &descriptor,
            PluginSandboxPermission::Network(PluginSandboxNetworkGrant::AnyOutbound),
            None,
        );
        let mut broker = DenyPluginSandboxPermissionBroker;

        let decision = broker.decide_permission(request);

        assert_eq!(decision.outcome, PluginSandboxPermissionOutcome::Denied);
        assert!(!decision.restart_required);
    }

    #[test]
    fn already_active_permission_decision_does_not_require_restart() {
        let descriptor = descriptor("com.test.active");
        let request = PluginSandboxPermissionRequest::from_descriptor(
            &descriptor,
            PluginSandboxPermission::Network(PluginSandboxNetworkGrant::LoopbackOnly),
            None,
        );

        let decision = request.grant_already_active();

        assert!(!decision.restart_required);
        assert_eq!(
            decision.granted_permission(),
            Some(&PluginSandboxPermission::Network(
                PluginSandboxNetworkGrant::LoopbackOnly
            ))
        );
    }

    #[test]
    fn denied_permission_decision_does_not_require_restart_or_persist() {
        let descriptor = descriptor("com.test.denied");
        let request = PluginSandboxPermissionRequest::from_descriptor(
            &descriptor,
            PluginSandboxPermission::ChildProcess(PluginSandboxChildProcessGrant::AllowAny),
            Some("helper launch".to_string()),
        );

        let decision = request.deny();
        let mut store = PluginSandboxGrantStore::default();

        assert!(!decision.restart_required);
        assert_eq!(decision.granted_permission(), None);
        assert!(!decision.apply_to_store(&mut store));
        assert!(store.grants.is_empty());
    }

    #[test]
    fn permission_broker_can_drive_decision_flow() {
        struct RememberingBroker;

        impl PluginSandboxPermissionBroker for RememberingBroker {
            fn decide_permission(
                &mut self,
                request: PluginSandboxPermissionRequest,
            ) -> PluginSandboxPermissionDecision {
                request.grant_remembered()
            }
        }

        let descriptor = descriptor("com.test.broker");
        let request = PluginSandboxPermissionRequest::from_descriptor(
            &descriptor,
            PluginSandboxPermission::WritePath {
                path: PathBuf::from("/tmp/plugin-cache"),
            },
            None,
        );
        let mut broker = RememberingBroker;
        let mut store = PluginSandboxGrantStore::default();

        let decision = broker.decide_permission(request);
        assert!(store.apply_decision(&decision));
        assert_eq!(store.grants.len(), 1);
    }

    #[test]
    fn policy_reports_no_support_issues_for_fully_capable_backend() {
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        policy.network = PluginSandboxNetworkGrant::LoopbackOnly;
        policy.local_authorizations = vec![PluginSandboxAuthorizationGrant::Pace];
        policy.child_processes = PluginSandboxChildProcessGrant::AllowSignedHelpers {
            paths: vec![PathBuf::from("/tmp/helper")],
        };

        assert!(policy.is_supported_by(PluginSandboxBackendCapabilities {
            filesystem: true,
            network: true,
            local_authorization_profiles: true,
            child_process_control: true,
            prompt_without_restart: true,
            store_compatible: true,
        }));
    }

    #[test]
    fn policy_reports_filesystem_gap_for_process_only_backend() {
        let policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");

        let issues = policy.support_issues(PluginSandboxBackendCapabilities {
            filesystem: false,
            network: false,
            local_authorization_profiles: false,
            child_process_control: false,
            prompt_without_restart: false,
            store_compatible: true,
        });

        assert_eq!(
            issues,
            vec![PluginSandboxPolicySupportIssue::FilesystemAccessUnsupported]
        );
    }

    #[test]
    fn launch_plan_rejects_required_process_only_backend() {
        let policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        let plan = policy.launch_plan(PluginSandboxLaunchBackend::ProcessIsolationOnly {
            platform: "test-process-only",
        });

        assert_eq!(plan.backend_id(), "test-process-only");
        assert!(plan.is_store_compatible());
        assert!(!plan.is_fully_supported());
        let err = plan.validate_for_launch(&policy).unwrap_err();
        assert!(err.contains("cannot satisfy required policy"));
        assert!(err.contains("filesystem"));
    }

    #[test]
    fn launch_plan_allows_optional_process_only_backend_with_visible_gaps() {
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        policy.require_platform_sandbox = false;
        let plan = policy.launch_plan(PluginSandboxLaunchBackend::ProcessIsolationOnly {
            platform: "test-process-only",
        });

        assert_eq!(
            plan.support_issues,
            vec![PluginSandboxPolicySupportIssue::FilesystemAccessUnsupported]
        );
        assert!(plan.validate_for_launch(&policy).is_ok());
    }

    #[test]
    fn launch_plan_rejects_current_worker_adapter_gaps() {
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        policy.require_platform_sandbox = false;
        policy.network = PluginSandboxNetworkGrant::LoopbackOnly;
        let plan = policy.launch_plan(PluginSandboxLaunchBackend::LinuxLandlockWorker);

        let err = plan.validate_for_launch(&policy).unwrap_err();
        assert!(err.contains("cannot launch current worker policy"));
    }

    #[test]
    fn policy_reports_non_default_grant_gaps() {
        let mut policy = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
        policy.network = PluginSandboxNetworkGrant::RemoteTcp {
            hosts: vec!["license.example".into()],
            ports: vec![443],
        };
        policy.local_authorizations = vec![PluginSandboxAuthorizationGrant::Ilok];
        policy.child_processes = PluginSandboxChildProcessGrant::AllowAny;
        policy.broker = PluginSandboxBrokerPolicy::ReportOnly;

        let issues = policy.support_issues(PluginSandboxBackendCapabilities {
            filesystem: true,
            network: false,
            local_authorization_profiles: false,
            child_process_control: false,
            prompt_without_restart: false,
            store_compatible: true,
        });

        assert_eq!(
            issues,
            vec![
                PluginSandboxPolicySupportIssue::NetworkGrantUnsupported {
                    grant: PluginSandboxNetworkGrant::RemoteTcp {
                        hosts: vec!["license.example".into()],
                        ports: vec![443],
                    },
                },
                PluginSandboxPolicySupportIssue::LocalAuthorizationUnsupported {
                    grant: PluginSandboxAuthorizationGrant::Ilok,
                },
                PluginSandboxPolicySupportIssue::ChildProcessGrantUnsupported {
                    grant: PluginSandboxChildProcessGrant::AllowAny,
                },
                PluginSandboxPolicySupportIssue::PromptWithoutRestartUnsupported,
            ]
        );
    }

    #[test]
    fn disabled_policy_skips_support_diagnostics() {
        let policy = PluginSandboxPolicy::disabled();

        assert!(
            policy
                .support_issues(PluginSandboxBackendCapabilities {
                    filesystem: false,
                    network: false,
                    local_authorization_profiles: false,
                    child_process_control: false,
                    prompt_without_restart: false,
                    store_compatible: true,
                })
                .is_empty()
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_unknown_trust_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let plugin_path = temp.path().join("sandbox-test.clap");
        std::fs::write(&plugin_path, b"stub").unwrap();
        let descriptor = PluginDescriptor {
            id: "sandbox.test".into(),
            name: "sandbox-test".into(),
            vendor: "test".into(),
            version: "0.1".into(),
            format: PluginFormat::Clap,
            path: plugin_path,
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: crate::external_plugin::PluginScanStatus::Discovered,
        };
        let shared =
            SecurePluginSharedMemory::create(PluginIpcLayout::new(48_000, 64, 2, 2).unwrap())
                .unwrap();
        let policy = ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown);

        let err = enter_external_plugin_sandbox(&policy, &descriptor, shared.path()).unwrap_err();
        assert!(err.contains("required"));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_required_sandbox_reports_error() {
        let temp = tempfile::tempdir().unwrap();
        let plugin_path = temp.path().join("sandbox-required-test.clap");
        std::fs::write(&plugin_path, b"stub").unwrap();
        let descriptor = PluginDescriptor {
            id: "sandbox.required.test".into(),
            name: "sandbox-required-test".into(),
            vendor: "test".into(),
            version: "0.1".into(),
            format: PluginFormat::Clap,
            path: plugin_path,
            audio_inputs: 2,
            audio_outputs: 2,
            is_instrument: false,
            categories: Vec::new(),
            scan_status: crate::external_plugin::PluginScanStatus::Discovered,
        };
        let shared =
            SecurePluginSharedMemory::create(PluginIpcLayout::new(48_000, 64, 2, 2).unwrap())
                .unwrap();
        let mut policy = ExternalPluginSandboxPolicy::for_trust(ExternalPluginTrust::Unknown);
        policy.require_platform_sandbox = true;

        let err = enter_external_plugin_sandbox(&policy, &descriptor, shared.path()).unwrap_err();
        assert!(err.contains("required"));
    }
}
