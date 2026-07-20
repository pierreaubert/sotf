use crate::{NetworkEndpointConfig, NetworkEndpointMode, NetworkEndpointStatus};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEndpointBackend {
    Disabled,
    InputClient,
    HttpEndpoint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEndpointPlan {
    pub config: NetworkEndpointConfig,
    pub backend: NetworkEndpointBackend,
    pub status: NetworkEndpointStatus,
    pub reason: Option<String>,
}

impl NetworkEndpointPlan {
    fn new(
        config: &NetworkEndpointConfig,
        backend: NetworkEndpointBackend,
        status: NetworkEndpointStatus,
        reason: Option<String>,
    ) -> Self {
        Self {
            config: config.clone(),
            backend,
            status,
            reason,
        }
    }
}

pub fn plan_network_endpoint(config: &NetworkEndpointConfig) -> NetworkEndpointPlan {
    match config.mode {
        NetworkEndpointMode::Disabled => NetworkEndpointPlan::new(
            config,
            NetworkEndpointBackend::Disabled,
            NetworkEndpointStatus::Disabled,
            None,
        ),
        NetworkEndpointMode::InputClient => {
            #[cfg(feature = "streaming")]
            {
                NetworkEndpointPlan::new(
                    config,
                    NetworkEndpointBackend::InputClient,
                    NetworkEndpointStatus::InputClientAvailable,
                    Some("network input client support is compiled into this build".to_string()),
                )
            }
            #[cfg(not(feature = "streaming"))]
            {
                NetworkEndpointPlan::new(
                    config,
                    NetworkEndpointBackend::InputClient,
                    NetworkEndpointStatus::InputClientUnavailable,
                    Some(
                        "network input client support requires the 'streaming' feature".to_string(),
                    ),
                )
            }
        }
        NetworkEndpointMode::HttpEndpoint => {
            #[cfg(feature = "streaming")]
            {
                NetworkEndpointPlan::new(
                    config,
                    NetworkEndpointBackend::HttpEndpoint,
                    NetworkEndpointStatus::EndpointUnavailable,
                    Some(format!(
                        "HTTP endpoint will bind during engine startup at {}:{}",
                        config.bind_addr, config.port
                    )),
                )
            }
            #[cfg(not(feature = "streaming"))]
            {
                NetworkEndpointPlan::new(
                    config,
                    NetworkEndpointBackend::HttpEndpoint,
                    NetworkEndpointStatus::EndpointUnavailable,
                    Some("HTTP endpoint support requires the 'streaming' feature".to_string()),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_network_endpoint_reports_disabled() {
        let plan = plan_network_endpoint(&NetworkEndpointConfig::default());

        assert_eq!(plan.backend, NetworkEndpointBackend::Disabled);
        assert_eq!(plan.status, NetworkEndpointStatus::Disabled);
        assert_eq!(plan.reason, None);
    }

    #[test]
    fn input_client_reports_build_capability() {
        let config = NetworkEndpointConfig {
            mode: NetworkEndpointMode::InputClient,
            ..Default::default()
        };
        let plan = plan_network_endpoint(&config);

        assert_eq!(plan.backend, NetworkEndpointBackend::InputClient);
        #[cfg(feature = "streaming")]
        assert_eq!(plan.status, NetworkEndpointStatus::InputClientAvailable);
        #[cfg(not(feature = "streaming"))]
        assert_eq!(plan.status, NetworkEndpointStatus::InputClientUnavailable);
        assert!(plan.reason.is_some());
    }

    #[test]
    fn http_endpoint_reports_pending_runtime_bind() {
        let config = NetworkEndpointConfig {
            mode: NetworkEndpointMode::HttpEndpoint,
            bind_addr: "127.0.0.1".to_string(),
            port: 0,
        };
        let plan = plan_network_endpoint(&config);

        assert_eq!(plan.backend, NetworkEndpointBackend::HttpEndpoint);
        assert_eq!(plan.status, NetworkEndpointStatus::EndpointUnavailable);
        #[cfg(feature = "streaming")]
        assert!(
            plan.reason
                .as_deref()
                .unwrap()
                .contains("bind during engine startup")
        );
        #[cfg(not(feature = "streaming"))]
        assert!(
            plan.reason
                .as_deref()
                .unwrap()
                .contains("requires the 'streaming' feature")
        );
    }
}
