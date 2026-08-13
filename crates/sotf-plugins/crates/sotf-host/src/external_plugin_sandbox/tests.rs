use super::default::default_plugin_sandbox_launcher_command_for_backend;
use super::external_plugin_sandbox_policy::ExternalPluginSandboxPolicy;
use super::external_plugin_sandbox_timing::ExternalPluginSandboxTiming;
use super::external_plugin_trust::ExternalPluginTrust;
use super::plugin_sandbox_authorization_grant::PluginSandboxAuthorizationGrant;
use super::plugin_sandbox_child_process_grant::PluginSandboxChildProcessGrant;
use super::plugin_sandbox_launch_backend::PluginSandboxLaunchBackend;
use super::plugin_sandbox_network_grant::PluginSandboxNetworkGrant;
use super::plugin_sandbox_permission::PluginSandboxPermission;
use super::plugin_sandbox_policy::PluginSandboxPolicy;
use super::plugin_sandbox_policy_adapter_issue::PluginSandboxPolicyAdapterIssue;
use super::plugin_sandbox_policy_support_issue::PluginSandboxPolicySupportIssue;
use super::types::PluginSandboxBackendCapabilities;
use super::types::PluginSandboxBrokerPolicy;
use super::types::PluginSandboxFileGrant;
#[cfg(target_os = "linux")]
use super::types::platform;
use crate::external_plugin_process::ExternalPluginWorkerCommand;
use std::path::PathBuf;

mod misc;

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

#[test]
fn permission_satisfaction_rejects_parent_traversal() {
    let granted = PluginSandboxPermission::ReadPath {
        path: PathBuf::from("/tmp/plugin-data"),
    };

    assert!(!granted.satisfies(&PluginSandboxPermission::ReadPath {
        path: PathBuf::from("/tmp/plugin-data/../protected"),
    }));
    assert!(!granted.satisfies(&PluginSandboxPermission::ReadPath {
        path: PathBuf::from("/tmp/plugin-data/child/../../protected"),
    }));
    assert!(granted.satisfies(&PluginSandboxPermission::ReadPath {
        path: PathBuf::from("/tmp/plugin-data/child"),
    }));
}
