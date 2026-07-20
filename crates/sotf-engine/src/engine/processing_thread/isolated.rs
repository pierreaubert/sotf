use super::super::IsolatedExternalPluginWorkerStatus;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{
    IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
    IsolatedExternalPluginWorkerEvent,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::ExternalPluginProcessEvent;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::IsolatedExternalPluginWorkerReport;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::{PluginSandboxBackendCode, PluginSandboxStatusCode};

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn isolated_external_plugin_status(
    report: IsolatedExternalPluginWorkerReport,
) -> IsolatedExternalPluginWorkerStatus {
    IsolatedExternalPluginWorkerStatus {
        plugin_index: report.plugin_index,
        node_id: report.node_id,
        plugin_instance_id: report.plugin_instance_id,
        event: report.event.map(isolated_external_plugin_event),
        error: report.error,
        worker_start_count: report.worker_start_count,
        worker_exit_count: report.worker_exit_count,
        worker_launch_failure_count: report.worker_launch_failure_count,
        block_timeout_count: report.block_timeout_count,
        block_worker_failure_count: report.block_worker_failure_count,
        block_wrong_sequence_count: report.block_wrong_sequence_count,
        sandbox_status: isolated_external_plugin_sandbox_status(report.sandbox_status),
        sandbox_backend: isolated_external_plugin_sandbox_backend(report.sandbox_backend),
        sandbox_reason: report.sandbox_reason,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn isolated_external_plugin_event(
    event: ExternalPluginProcessEvent,
) -> IsolatedExternalPluginWorkerEvent {
    match event {
        ExternalPluginProcessEvent::AlreadyRunning => {
            IsolatedExternalPluginWorkerEvent::AlreadyRunning
        }
        ExternalPluginProcessEvent::Started { pid } => {
            IsolatedExternalPluginWorkerEvent::Started { pid }
        }
        ExternalPluginProcessEvent::Exited { status } => {
            IsolatedExternalPluginWorkerEvent::Exited {
                exit_code: status.code(),
            }
        }
        ExternalPluginProcessEvent::NotRunning => IsolatedExternalPluginWorkerEvent::NotRunning,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn isolated_external_plugin_sandbox_status(
    status: PluginSandboxStatusCode,
) -> IsolatedExternalPluginSandboxStatus {
    match status {
        PluginSandboxStatusCode::Unknown => IsolatedExternalPluginSandboxStatus::Unknown,
        PluginSandboxStatusCode::Disabled => IsolatedExternalPluginSandboxStatus::Disabled,
        PluginSandboxStatusCode::Enforced => IsolatedExternalPluginSandboxStatus::Enforced,
        PluginSandboxStatusCode::Unsupported => IsolatedExternalPluginSandboxStatus::Unsupported,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn isolated_external_plugin_sandbox_backend(
    backend: PluginSandboxBackendCode,
) -> IsolatedExternalPluginSandboxBackend {
    match backend {
        PluginSandboxBackendCode::Unknown => IsolatedExternalPluginSandboxBackend::Unknown,
        PluginSandboxBackendCode::LinuxLandlock => {
            IsolatedExternalPluginSandboxBackend::LinuxLandlock
        }
        PluginSandboxBackendCode::MacosProcessIsolation => {
            IsolatedExternalPluginSandboxBackend::MacosProcessIsolation
        }
        PluginSandboxBackendCode::MacosAppSandboxHelper => {
            IsolatedExternalPluginSandboxBackend::MacosAppSandboxHelper
        }
        PluginSandboxBackendCode::WindowsProcessIsolation => {
            IsolatedExternalPluginSandboxBackend::WindowsProcessIsolation
        }
    }
}
