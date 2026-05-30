use serde::{Deserialize, Serialize};
use sotf_types::{OutputAccessMode, OutputAccessStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputAccessBackend {
    SharedCpal,
    CoreAudioHogMode,
    Asio,
    IosSystemOutput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputAccessPlan {
    pub requested: OutputAccessMode,
    pub device_identifier: Option<String>,
    pub backend: OutputAccessBackend,
    pub status: OutputAccessStatus,
    pub reason: Option<String>,
}

impl OutputAccessPlan {
    fn new(
        requested: OutputAccessMode,
        output_device: Option<&str>,
        backend: OutputAccessBackend,
        status: OutputAccessStatus,
        reason: Option<String>,
    ) -> Self {
        Self {
            requested,
            device_identifier: output_device.map(str::to_string),
            backend,
            status,
            reason,
        }
    }
}

pub fn plan_output_access(
    requested: OutputAccessMode,
    output_device: Option<&str>,
) -> OutputAccessPlan {
    match requested {
        OutputAccessMode::Shared => OutputAccessPlan::new(
            requested,
            output_device,
            shared_backend(),
            OutputAccessStatus::Shared,
            None,
        ),
        OutputAccessMode::ExclusivePreferred | OutputAccessMode::ExclusiveRequired => {
            plan_exclusive_output_access(requested, output_device)
        }
    }
}

fn plan_exclusive_output_access(
    requested: OutputAccessMode,
    output_device: Option<&str>,
) -> OutputAccessPlan {
    if output_device_is_asio(output_device) {
        return OutputAccessPlan::new(
            requested,
            output_device,
            OutputAccessBackend::Asio,
            OutputAccessStatus::ExclusiveActive,
            None,
        );
    }

    if cfg!(target_os = "macos") {
        return OutputAccessPlan::new(
            requested,
            output_device,
            OutputAccessBackend::CoreAudioHogMode,
            OutputAccessStatus::ExclusivePending,
            Some(
                "CoreAudio hog-mode ownership will be attempted during playback setup".to_string(),
            ),
        );
    }

    if cfg!(target_os = "ios") {
        let status = if requested == OutputAccessMode::ExclusivePreferred {
            OutputAccessStatus::FallbackShared
        } else {
            OutputAccessStatus::Unsupported
        };
        return OutputAccessPlan::new(
            requested,
            output_device,
            OutputAccessBackend::IosSystemOutput,
            status,
            Some("iOS system output does not expose exclusive device ownership".to_string()),
        );
    }

    let status = if requested == OutputAccessMode::ExclusivePreferred {
        OutputAccessStatus::FallbackShared
    } else {
        OutputAccessStatus::Unsupported
    };
    OutputAccessPlan::new(
        requested,
        output_device,
        OutputAccessBackend::SharedCpal,
        status,
        Some("the selected cpal backend does not expose exclusive output access".to_string()),
    )
}

fn shared_backend() -> OutputAccessBackend {
    if cfg!(target_os = "ios") {
        OutputAccessBackend::IosSystemOutput
    } else {
        OutputAccessBackend::SharedCpal
    }
}

#[cfg(not(target_os = "ios"))]
fn output_device_is_asio(output_device: Option<&str>) -> bool {
    output_device.is_some_and(crate::devices::is_asio_device)
}

#[cfg(target_os = "ios")]
fn output_device_is_asio(_output_device: Option<&str>) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_output_reports_shared_backend() {
        let plan = plan_output_access(OutputAccessMode::Shared, None);

        assert_eq!(plan.status, OutputAccessStatus::Shared);
        assert_eq!(plan.reason, None);
    }

    #[test]
    fn exclusive_preferred_reports_platform_plan() {
        let plan = plan_output_access(OutputAccessMode::ExclusivePreferred, None);

        #[cfg(target_os = "macos")]
        {
            assert_eq!(plan.backend, OutputAccessBackend::CoreAudioHogMode);
            assert_eq!(plan.status, OutputAccessStatus::ExclusivePending);
        }
        #[cfg(target_os = "ios")]
        {
            assert_eq!(plan.backend, OutputAccessBackend::IosSystemOutput);
            assert_eq!(plan.status, OutputAccessStatus::FallbackShared);
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            assert_eq!(plan.backend, OutputAccessBackend::SharedCpal);
            assert_eq!(plan.status, OutputAccessStatus::FallbackShared);
        }
        assert!(plan.reason.is_some());
    }

    #[test]
    fn exclusive_required_reports_unsupported_when_no_backend_can_own_device() {
        let plan = plan_output_access(OutputAccessMode::ExclusiveRequired, None);

        #[cfg(target_os = "macos")]
        assert_eq!(plan.status, OutputAccessStatus::ExclusivePending);
        #[cfg(not(target_os = "macos"))]
        assert_eq!(plan.status, OutputAccessStatus::Unsupported);
    }

    #[cfg(not(target_os = "ios"))]
    #[test]
    fn asio_device_reports_exclusive_active() {
        let plan = plan_output_access(
            OutputAccessMode::ExclusivePreferred,
            Some("ASIO:Focusrite USB"),
        );

        assert_eq!(plan.backend, OutputAccessBackend::Asio);
        assert_eq!(plan.status, OutputAccessStatus::ExclusiveActive);
        assert_eq!(plan.reason, None);
    }
}
