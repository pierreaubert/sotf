use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sotf_dev_api::{Capabilities, DevReply, Snapshot};

pub const FUZZ_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum TargetId {
    DesktopGpui,
    Tui,
    IosSim,
    TvosSim,
    PlayerCli,
    RecorderCli,
    HeadlessServer,
    SystemwideDaemon,
    Configbar,
}

impl TargetId {
    pub const ALL: [Self; 9] = [
        Self::DesktopGpui,
        Self::Tui,
        Self::IosSim,
        Self::TvosSim,
        Self::PlayerCli,
        Self::RecorderCli,
        Self::HeadlessServer,
        Self::SystemwideDaemon,
        Self::Configbar,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DesktopGpui => "desktop-gpui",
            Self::Tui => "tui",
            Self::IosSim => "ios-sim",
            Self::TvosSim => "tvos-sim",
            Self::PlayerCli => "player-cli",
            Self::RecorderCli => "recorder-cli",
            Self::HeadlessServer => "headless-server",
            Self::SystemwideDaemon => "systemwide-daemon",
            Self::Configbar => "configbar",
        }
    }
}

impl std::fmt::Display for TargetId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TargetId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|target| target.as_str() == value)
            .ok_or_else(|| format!("unknown fuzz target {value:?}"))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    DevApi,
    Simulator,
    Process,
    Server,
    Ipc,
    Configbar,
    Synthetic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EndpointSpec {
    pub name: String,
    pub address: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetSpec {
    pub schema_version: u16,
    pub target_id: TargetId,
    pub adapter: AdapterKind,
    pub executable: Option<PathBuf>,
    pub app_identity: Option<String>,
    pub platform: String,
    pub fixture_profile: String,
    pub environment_names: Vec<String>,
    pub endpoints: Vec<EndpointSpec>,
    pub run_id_hash: String,
    pub capability_fingerprint: String,
    pub build_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionClass {
    StateValid,
    BoundedChaos,
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionPayload {
    DevAction {
        name: String,
        payload: Value,
    },
    Query {
        path: String,
    },
    Key {
        keystroke: String,
    },
    Text {
        text: String,
    },
    Selector {
        operation: String,
        selector: String,
    },
    Coordinate {
        input: sotf_dev_api::CoordinateInput,
    },
    ProcessArgv {
        argv: Vec<String>,
    },
    Stdin {
        bytes: Vec<u8>,
        eof: bool,
    },
    Signal {
        signal: String,
    },
    Http {
        endpoint: String,
        method: String,
        path: String,
        headers: BTreeMap<String, String>,
        body: Vec<u8>,
    },
    Ipc {
        command: Value,
    },
    Wait {
        duration_ms: u64,
    },
    Restart,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub schema_version: u16,
    pub sequence: u64,
    pub id: String,
    pub family: String,
    pub class: ActionClass,
    pub precondition_id: Option<String>,
    pub precondition_satisfied: bool,
    pub payload: ActionPayload,
    pub timeout_ms: u64,
    pub rng_cursor: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CoverageDelta {
    pub new_keys: Vec<String>,
    pub counters: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessObservation {
    pub pid: Option<u32>,
    pub alive: bool,
    pub exit_code: Option<i32>,
    pub signal_or_exception: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub schema_version: u16,
    pub sequence: u64,
    pub reply: Option<DevReply>,
    pub snapshot: Option<Snapshot>,
    pub process: ProcessObservation,
    pub resource: Option<ResourceSample>,
    pub new_logs: Vec<String>,
    pub crash_files: Vec<PathBuf>,
    pub coverage: CoverageDelta,
    pub screenshot: Option<PathBuf>,
    pub failure_candidate: Option<Failure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceSample {
    pub monotonic_ms: u64,
    pub rss_bytes: Option<u64>,
    pub virtual_bytes: Option<u64>,
    pub cpu_percent: Option<f32>,
    pub cpu_time_ms: Option<u64>,
    pub threads: Option<u64>,
    pub descriptors_or_handles: Option<u64>,
    pub children: Option<u64>,
    pub unavailable: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    UnexpectedExit,
    SignalOrException,
    PanicOrErrorLog,
    ValidActionRejection,
    InvariantViolation,
    CommandTimeout,
    MainLoopStall,
    WholeProcessHang,
    ResourceLeak,
    SuspectedMemoryLeak,
    CleanupInvariant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureSignature {
    pub class: FailureClass,
    pub normalized: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Failure {
    pub schema_version: u16,
    pub class: FailureClass,
    pub signature: FailureSignature,
    pub first_sequence: u64,
    pub evidence: Vec<String>,
    pub target_id: TargetId,
    pub build_id: String,
    pub confirmations: u8,
    pub artifacts: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuredSkip {
    pub target_id: TargetId,
    pub reason_code: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayConfig {
    pub schema_version: u16,
    pub target: TargetSpec,
    pub capabilities: Capabilities,
    pub fixture_digest: String,
    pub manifest_path: PathBuf,
    pub trace_path: PathBuf,
    pub artifact_dir: PathBuf,
    pub opt_ins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TraceEvent {
    RunStart {
        schema_version: u16,
        target: Box<TargetSpec>,
        seed: u64,
        worker: u32,
        worker_seed: u64,
        generator_version: u16,
        capabilities: Box<Capabilities>,
        manifest_digest: String,
        fixture_digest: String,
    },
    ActionIntent {
        action: Box<Action>,
        preceding_revision: u64,
        preceding_state_hash: String,
    },
    Observation {
        observation: Box<Observation>,
    },
    FailureConfirmation {
        signature: FailureSignature,
        attempt: u8,
        matched: bool,
    },
    Cleanup {
        graceful: bool,
        details: String,
    },
    RunEnd {
        outcome: String,
        steps: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_ids_are_stable_and_complete() {
        let names: Vec<_> = TargetId::ALL.into_iter().map(TargetId::as_str).collect();
        assert_eq!(names.len(), 9);
        for (target, name) in TargetId::ALL.into_iter().zip(names) {
            assert_eq!(name.parse::<TargetId>().unwrap(), target);
        }
    }
}
