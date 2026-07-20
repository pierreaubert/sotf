use crate::EngineConfig;
use serde::{Deserialize, Serialize};

use super::{
    DsdOutputPlan, NetworkEndpointPlan, OutputAccessPlan, plan_dsd_output, plan_network_endpoint,
    plan_output_access,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineFeaturePlan {
    pub output_access: OutputAccessPlan,
    pub dsd_output: DsdOutputPlan,
    pub network_endpoint: NetworkEndpointPlan,
}

pub fn plan_engine_features(config: &EngineConfig) -> EngineFeaturePlan {
    EngineFeaturePlan {
        output_access: plan_output_access(config.output_access, config.output_device.as_deref()),
        dsd_output: plan_dsd_output(config.dsd_output),
        network_endpoint: plan_network_endpoint(&config.network_endpoint),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DsdOutputMode, NetworkEndpointConfig, NetworkEndpointMode, NetworkEndpointStatus,
        OutputAccessMode,
    };

    #[test]
    fn feature_plan_matches_individual_planners() {
        let config = EngineConfig {
            output_access: OutputAccessMode::ExclusivePreferred,
            output_device: Some("ASIO:Focusrite USB".to_string()),
            dsd_output: DsdOutputMode::DopPreferred,
            network_endpoint: NetworkEndpointConfig {
                mode: NetworkEndpointMode::HttpEndpoint,
                bind_addr: "127.0.0.1".to_string(),
                port: 9137,
            },
            ..Default::default()
        };

        let plan = plan_engine_features(&config);

        assert_eq!(
            plan.output_access,
            plan_output_access(config.output_access, config.output_device.as_deref())
        );
        assert_eq!(plan.dsd_output, plan_dsd_output(config.dsd_output));
        assert_eq!(
            plan.network_endpoint,
            plan_network_endpoint(&config.network_endpoint)
        );
    }

    #[test]
    fn feature_plan_reports_endpoint_startup_status() {
        let config = EngineConfig {
            network_endpoint: NetworkEndpointConfig {
                mode: NetworkEndpointMode::HttpEndpoint,
                ..Default::default()
            },
            ..Default::default()
        };

        let plan = plan_engine_features(&config);

        assert_eq!(
            plan.network_endpoint.status,
            NetworkEndpointStatus::EndpointUnavailable
        );
        assert!(plan.network_endpoint.reason.is_some());
    }
}
