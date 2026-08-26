use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sotf_dev_api::Snapshot;
use thiserror::Error;

use super::model::{ActionPayload, TargetId};

pub const MANIFEST_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurfaceManifest {
    pub schema_version: u16,
    pub version: u32,
    pub target: TargetId,
    #[serde(default)]
    pub fixture_profiles: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ManifestAction>,
    #[serde(default)]
    pub invariants: Vec<ManifestInvariant>,
    #[serde(default)]
    pub workflows: Vec<WorkflowEdge>,
}

impl SurfaceManifest {
    pub fn parse_toml(source: &str) -> Result<Self, ManifestError> {
        let manifest: Self = toml::from_str(source)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::Schema(self.schema_version));
        }
        let mut ids = BTreeSet::new();
        for action in &self.actions {
            if action.id.trim().is_empty() || action.family.trim().is_empty() {
                return Err(ManifestError::EmptyAction);
            }
            if !ids.insert(&action.id) {
                return Err(ManifestError::DuplicateAction(action.id.clone()));
            }
            if action.weight == 0 || action.weight > 10_000 {
                return Err(ManifestError::Weight(action.id.clone()));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, ManifestError> {
        let bytes = serde_json::to_vec(self)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestAction {
    pub id: String,
    pub family: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub precondition_id: Option<String>,
    #[serde(default)]
    pub precondition: Condition,
    #[serde(default)]
    pub recovery: bool,
    #[serde(default)]
    pub chaos_only: bool,
    pub payload: ActionPayload,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub coverage: Vec<String>,
}

fn default_weight() -> u32 {
    100
}

fn default_timeout_ms() -> u64 {
    5_000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Condition {
    #[default]
    Always,
    Equals {
        path: String,
        value: Value,
    },
    NotEquals {
        path: String,
        value: Value,
    },
    Exists {
        path: String,
    },
    In {
        path: String,
        values: Vec<Value>,
    },
    SelectorVisible {
        selector: String,
    },
    SelectorEnabled {
        selector: String,
    },
    AsyncIdle,
    ProcessPhase {
        phase: String,
    },
    All {
        conditions: Vec<Condition>,
    },
    Any {
        conditions: Vec<Condition>,
    },
    Not {
        condition: Box<Condition>,
    },
}

impl Condition {
    pub fn evaluate(&self, snapshot: &Snapshot) -> bool {
        match self {
            Self::Always => true,
            Self::Equals { path, value } => lookup(snapshot, path) == Some(value),
            Self::NotEquals { path, value } => lookup(snapshot, path).is_some_and(|v| v != value),
            Self::Exists { path } => lookup(snapshot, path).is_some(),
            Self::In { path, values } => {
                lookup(snapshot, path).is_some_and(|value| values.contains(value))
            }
            Self::SelectorVisible { selector } => snapshot
                .tracked_elements
                .iter()
                .any(|element| element.selector == *selector && element.visible),
            Self::SelectorEnabled { selector } => snapshot
                .tracked_elements
                .iter()
                .any(|element| element.selector == *selector && element.visible && element.enabled),
            Self::AsyncIdle => snapshot.async_activity.is_empty(),
            Self::ProcessPhase { phase } => lookup(snapshot, "state.process_phase")
                .and_then(Value::as_str)
                .is_some_and(|value| value == phase),
            Self::All { conditions } => conditions.iter().all(|item| item.evaluate(snapshot)),
            Self::Any { conditions } => conditions.iter().any(|item| item.evaluate(snapshot)),
            Self::Not { condition } => !condition.evaluate(snapshot),
        }
    }
}

fn lookup<'a>(snapshot: &'a Snapshot, path: &str) -> Option<&'a Value> {
    let path = path.strip_prefix("state.").unwrap_or(path);
    if path.is_empty() {
        return Some(&snapshot.state);
    }
    path.split('.')
        .try_fold(&snapshot.state, |value, component| match value {
            Value::Object(map) => map.get(component),
            Value::Array(values) => component
                .parse::<usize>()
                .ok()
                .and_then(|index| values.get(index)),
            _ => None,
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestInvariant {
    pub id: String,
    pub condition: Condition,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowEdge {
    pub from: String,
    pub action: String,
    pub to: String,
    #[serde(default)]
    pub fixture: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityIntersection {
    pub supported_actions: BTreeSet<String>,
    pub missing_manifest_actions: BTreeSet<String>,
    pub unclassified_runtime_actions: BTreeSet<String>,
    pub families: BTreeMap<String, usize>,
}

pub fn intersect_capabilities(
    manifest: &SurfaceManifest,
    capabilities: &sotf_dev_api::Capabilities,
) -> CapabilityIntersection {
    let runtime_ids: BTreeSet<_> = capabilities
        .actions
        .iter()
        .map(|item| item.name.clone())
        .collect();
    let supported_actions = manifest
        .actions
        .iter()
        .filter(|action| action_supported(action, capabilities, &runtime_ids))
        .map(|action| action.id.clone())
        .collect::<BTreeSet<_>>();
    let missing_manifest_actions = manifest
        .actions
        .iter()
        .filter(|action| !supported_actions.contains(&action.id))
        .map(|action| action.id.clone())
        .collect();
    let classified_runtime_actions = manifest
        .actions
        .iter()
        .filter_map(|action| match &action.payload {
            ActionPayload::DevAction { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let unclassified_runtime_actions = runtime_ids
        .difference(&classified_runtime_actions)
        .cloned()
        .collect();
    let mut families = BTreeMap::new();
    for action in manifest
        .actions
        .iter()
        .filter(|item| supported_actions.contains(&item.id))
    {
        *families.entry(action.family.clone()).or_insert(0) += 1;
    }
    CapabilityIntersection {
        supported_actions,
        missing_manifest_actions,
        unclassified_runtime_actions,
        families,
    }
}

fn action_supported(
    action: &ManifestAction,
    capabilities: &sotf_dev_api::Capabilities,
    runtime_actions: &BTreeSet<String>,
) -> bool {
    match &action.payload {
        ActionPayload::DevAction { name, .. } => runtime_actions.contains(name),
        ActionPayload::Query { path } => capabilities.queries.contains(path),
        ActionPayload::Key { .. } => capabilities.inputs.key,
        ActionPayload::Text { .. } => capabilities.inputs.text,
        ActionPayload::Selector { .. } => capabilities.inputs.selector,
        ActionPayload::Coordinate { input } => match input {
            sotf_dev_api::CoordinateInput::Pointer { .. } => capabilities.inputs.pointer,
            sotf_dev_api::CoordinateInput::Touch { .. } => capabilities.inputs.touch,
            sotf_dev_api::CoordinateInput::Scroll { .. } => capabilities.inputs.scroll,
            sotf_dev_api::CoordinateInput::Remote { command, .. } => {
                capabilities.inputs.remote.contains(command)
            }
        },
        ActionPayload::ProcessArgv { .. }
        | ActionPayload::Stdin { .. }
        | ActionPayload::Signal { .. }
        | ActionPayload::Http { .. }
        | ActionPayload::Ipc { .. }
        | ActionPayload::Wait { .. }
        | ActionPayload::Restart => true,
    }
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("invalid manifest TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("manifest schema {0} is unsupported")]
    Schema(u16),
    #[error("manifest action ID and family must be non-empty")]
    EmptyAction,
    #[error("duplicate manifest action {0:?}")]
    DuplicateAction(String),
    #[error("manifest action {0:?} has an invalid weight")]
    Weight(String),
    #[error("serializing manifest: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sotf_dev_api::{Capabilities, Snapshot};

    use super::*;

    #[test]
    fn evaluates_typed_preconditions() {
        let mut snapshot = Snapshot::new(
            "test",
            1,
            json!({"screen":"library", "process_phase":"ready"}),
        )
        .unwrap();
        snapshot
            .tracked_elements
            .push(sotf_dev_api::TrackedElement {
                selector: "play".into(),
                bounds: sotf_dev_api::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                visible: true,
                enabled: true,
                selected: false,
                expanded: false,
            });
        let condition = Condition::All {
            conditions: vec![
                Condition::Equals {
                    path: "state.screen".into(),
                    value: json!("library"),
                },
                Condition::SelectorEnabled {
                    selector: "play".into(),
                },
                Condition::AsyncIdle,
            ],
        };
        assert!(condition.evaluate(&snapshot));
    }

    #[test]
    fn evaluates_every_manifest_precondition_operator() {
        let mut snapshot = Snapshot::new(
            "test",
            7,
            json!({
                "screen": "library",
                "process_phase": "ready",
                "mode": "albums",
            }),
        )
        .unwrap();
        snapshot
            .tracked_elements
            .push(sotf_dev_api::TrackedElement {
                selector: "play".into(),
                bounds: sotf_dev_api::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                visible: true,
                enabled: true,
                selected: false,
                expanded: false,
            });

        let conditions = [
            Condition::Always,
            Condition::Equals {
                path: "state.screen".into(),
                value: json!("library"),
            },
            Condition::NotEquals {
                path: "state.screen".into(),
                value: json!("queue"),
            },
            Condition::Exists {
                path: "state.mode".into(),
            },
            Condition::In {
                path: "state.mode".into(),
                values: vec![json!("tracks"), json!("albums")],
            },
            Condition::SelectorVisible {
                selector: "play".into(),
            },
            Condition::SelectorEnabled {
                selector: "play".into(),
            },
            Condition::AsyncIdle,
            Condition::ProcessPhase {
                phase: "ready".into(),
            },
            Condition::All {
                conditions: vec![Condition::Always, Condition::AsyncIdle],
            },
            Condition::Any {
                conditions: vec![
                    Condition::Equals {
                        path: "state.screen".into(),
                        value: json!("missing"),
                    },
                    Condition::Always,
                ],
            },
            Condition::Not {
                condition: Box::new(Condition::Equals {
                    path: "state.screen".into(),
                    value: json!("queue"),
                }),
            },
        ];
        for condition in conditions {
            assert!(condition.evaluate(&snapshot), "{condition:?}");
        }
    }

    #[test]
    fn reports_capability_drift_in_both_directions() {
        let manifest = SurfaceManifest {
            schema_version: 1,
            version: 1,
            target: TargetId::Tui,
            fixture_profiles: vec![],
            actions: vec![ManifestAction {
                id: "known".into(),
                family: "navigation".into(),
                weight: 100,
                precondition_id: None,
                precondition: Condition::Always,
                recovery: false,
                chaos_only: false,
                payload: ActionPayload::DevAction {
                    name: "known".into(),
                    payload: Value::Null,
                },
                timeout_ms: 1_000,
                coverage: vec![],
            }],
            invariants: vec![],
            workflows: vec![],
        };
        let mut capabilities = Capabilities::new("tui", "sotf-tui");
        capabilities.actions.push(sotf_dev_api::NamedCapability {
            name: "new".into(),
            family: "navigation".into(),
            payload_schema: None,
        });
        let intersection = intersect_capabilities(&manifest, &capabilities);
        assert!(intersection.missing_manifest_actions.contains("known"));
        assert!(intersection.unclassified_runtime_actions.contains("new"));
    }

    #[test]
    fn every_checked_in_target_manifest_parses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz");
        for target in TargetId::ALL {
            let path = root.join(format!("{}.toml", target.as_str()));
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
            let manifest = SurfaceManifest::parse_toml(&source)
                .unwrap_or_else(|error| panic!("parsing {}: {error}", path.display()));
            assert_eq!(manifest.target, target, "{}", path.display());
        }
    }
}
