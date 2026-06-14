#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::external::external_plugin_isolation_requested;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::external::external_plugin_trust;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::is::is_untrusted_external_plugin;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::misc::reject_worker_overrides;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::parse::parse_isolated_external_plugin_config;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{ExternalPluginSandboxTiming, ExternalPluginTrust, IsolatedExternalPluginConfig};

pub fn validate_plugin_security_config(
    plugin_type: &str,
    parameters: &serde_json::Value,
) -> Result<(), String> {
    let lower = plugin_type.to_ascii_lowercase();
    match lower.as_str() {
        "external" | "external_plugin" => validate_external_plugin_security_config(parameters),
        _ => Ok(()),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn validate_external_plugin_security_config(
    parameters: &serde_json::Value,
) -> Result<(), String> {
    let trust = external_plugin_trust(parameters)?;
    reject_worker_overrides(parameters)?;
    let isolated = external_plugin_isolation_requested(parameters, trust)?;
    if is_untrusted_external_plugin(trust) && !isolated {
        return Err("untrusted external plugins must run in isolated worker processes".to_string());
    }

    if isolated {
        let config = parse_isolated_external_plugin_config(parameters, trust)?;
        validate_untrusted_external_plugin_policy(&config, trust)?;
    }

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(super) fn validate_external_plugin_security_config(
    _parameters: &serde_json::Value,
) -> Result<(), String> {
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn validate_untrusted_external_plugin_policy(
    config: &IsolatedExternalPluginConfig,
    trust: ExternalPluginTrust,
) -> Result<(), String> {
    if !is_untrusted_external_plugin(trust) {
        return Ok(());
    }

    if config.sandbox_policy.timing != ExternalPluginSandboxTiming::BeforePluginLoad {
        return Err(
            "untrusted external plugins must enter the sandbox before plugin load".to_string(),
        );
    }
    if !config.sandbox_policy.require_platform_sandbox {
        return Err("untrusted external plugins require platform sandbox enforcement".to_string());
    }
    if config.sandbox_policy.allow_network {
        return Err("untrusted external plugins cannot allow network access".to_string());
    }
    if config.sandbox_policy.allow_child_processes {
        return Err("untrusted external plugins cannot allow child processes".to_string());
    }
    if !config.sandbox_policy.extra_read_paths.is_empty()
        || !config.sandbox_policy.extra_write_paths.is_empty()
    {
        return Err(
            "untrusted external plugin configs cannot expand sandbox filesystem access".to_string(),
        );
    }

    Ok(())
}
