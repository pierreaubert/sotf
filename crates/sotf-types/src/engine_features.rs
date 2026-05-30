//! Engine feature policy and capability status types.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LatencyCompensationMode {
    Disabled,
    #[default]
    Enabled,
}

impl LatencyCompensationMode {
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputAccessMode {
    #[default]
    Shared,
    ExclusivePreferred,
    ExclusiveRequired,
}

impl OutputAccessMode {
    pub fn prefers_exclusive(self) -> bool {
        matches!(self, Self::ExclusivePreferred | Self::ExclusiveRequired)
    }

    pub fn requires_exclusive(self) -> bool {
        matches!(self, Self::ExclusiveRequired)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputAccessStatus {
    #[default]
    Shared,
    ExclusivePending,
    ExclusiveActive,
    FallbackShared,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EngineOversamplingPolicy {
    Disabled,
    #[default]
    PluginPreferred,
    Force2x,
    Force4x,
}

impl EngineOversamplingPolicy {
    pub fn plugin_preferred_enabled(self) -> bool {
        matches!(self, Self::PluginPreferred)
    }

    pub fn forced_factor(self) -> Option<u32> {
        match self {
            Self::Force2x => Some(2),
            Self::Force4x => Some(4),
            Self::Disabled | Self::PluginPreferred => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DsdOutputMode {
    #[default]
    Disabled,
    PcmDecode,
    DopPreferred,
    DopRequired,
    NativePreferred,
    NativeRequired,
}

impl DsdOutputMode {
    pub fn requires_bitstream_output(self) -> bool {
        matches!(self, Self::DopRequired | Self::NativeRequired)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DsdOutputStatus {
    #[default]
    Disabled,
    PcmDecodeUnavailable,
    DopUnavailable,
    NativeUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEndpointMode {
    #[default]
    Disabled,
    InputClient,
    HttpEndpoint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEndpointConfig {
    #[serde(default)]
    pub mode: NetworkEndpointMode,
    #[serde(default = "default_network_bind_addr")]
    pub bind_addr: String,
    #[serde(default = "default_network_endpoint_port")]
    pub port: u16,
}

impl Default for NetworkEndpointConfig {
    fn default() -> Self {
        Self {
            mode: NetworkEndpointMode::Disabled,
            bind_addr: default_network_bind_addr(),
            port: default_network_endpoint_port(),
        }
    }
}

fn default_network_bind_addr() -> String {
    "0.0.0.0".to_string()
}

fn default_network_endpoint_port() -> u16 {
    17890
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEndpointStatus {
    #[default]
    Disabled,
    InputClientAvailable,
    InputClientUnavailable,
    EndpointRunning,
    EndpointUnavailable,
}
