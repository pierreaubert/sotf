use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PROTOCOL_VERSION: u16 = 2;
pub const CAPABILITIES_SCHEMA_VERSION: u16 = 1;
pub const SNAPSHOT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolLimits {
    pub request_line_bytes: usize,
    pub header_count: usize,
    pub header_bytes: usize,
    pub command_body_bytes: usize,
    pub fixture_body_bytes: usize,
    pub response_bytes: usize,
    pub read_timeout_ms: u64,
    pub write_timeout_ms: u64,
    pub command_timeout_ms: u64,
    pub active_connections: usize,
    pub command_queue: usize,
    pub text_bytes: usize,
    pub simultaneous_touches: usize,
}

impl Default for ProtocolLimits {
    fn default() -> Self {
        Self {
            request_line_bytes: 4 * 1024,
            header_count: 64,
            header_bytes: 32 * 1024,
            command_body_bytes: 256 * 1024,
            fixture_body_bytes: 2 * 1024 * 1024,
            response_bytes: 4 * 1024 * 1024,
            read_timeout_ms: 2_000,
            write_timeout_ms: 2_000,
            command_timeout_ms: 5_000,
            active_connections: 16,
            command_queue: 64,
            text_bytes: 4 * 1024,
            simultaneous_touches: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamedCapability {
    pub name: String,
    pub family: String,
    #[serde(default)]
    pub payload_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct InputCapabilities {
    pub key: bool,
    pub text: bool,
    pub selector: bool,
    pub pointer: bool,
    pub touch: bool,
    pub scroll: bool,
    pub resize: bool,
    pub remote: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureCapability {
    pub name: String,
    pub max_body_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capabilities {
    pub schema_version: u16,
    pub protocol_version: u16,
    pub target_id: String,
    pub platform: String,
    pub process_name: String,
    pub build_version: String,
    pub build_id: String,
    pub manifest_version: u32,
    pub debug_features: Vec<String>,
    pub actions: Vec<NamedCapability>,
    pub queries: Vec<String>,
    pub inputs: InputCapabilities,
    pub fixtures: Vec<FixtureCapability>,
    pub limits: ProtocolLimits,
    pub unsupported: BTreeMap<String, String>,
}

impl Capabilities {
    pub fn new(target_id: impl Into<String>, process_name: impl Into<String>) -> Self {
        Self {
            schema_version: CAPABILITIES_SCHEMA_VERSION,
            protocol_version: PROTOCOL_VERSION,
            target_id: target_id.into(),
            platform: std::env::consts::OS.to_owned(),
            process_name: process_name.into(),
            build_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_id: "development".to_owned(),
            manifest_version: 1,
            debug_features: Vec::new(),
            actions: Vec::new(),
            queries: Vec::new(),
            inputs: InputCapabilities::default(),
            fixtures: Vec::new(),
            limits: ProtocolLimits::default(),
            unsupported: BTreeMap::new(),
        }
    }

    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        canonical_json_hash(&serde_json::to_value(self)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsyncActivity {
    pub id: String,
    pub family: String,
    pub status: String,
    pub age_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn is_finite_non_negative(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
            && self.width >= 0.0
            && self.height >= 0.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackedElement {
    pub selector: String,
    pub bounds: Rect,
    pub visible: bool,
    pub enabled: bool,
    pub selected: bool,
    pub expanded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AccessibilitySnapshot {
    pub focused_id: Option<String>,
    pub node_count: usize,
    pub roles: BTreeMap<String, usize>,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceCounters {
    pub callback_allocations: Option<u64>,
    pub queue_depth: Option<u64>,
    pub pending_tasks: Option<u64>,
    pub dropped_events: Option<u64>,
    pub target: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub schema_version: u16,
    pub target_id: String,
    pub state_revision: u64,
    pub render_revision: Option<u64>,
    pub accessibility_revision: Option<u64>,
    pub heartbeat_age_ms: u64,
    pub screen: Option<String>,
    pub mode: Option<String>,
    pub dialogs: Vec<String>,
    pub async_activity: Vec<AsyncActivity>,
    pub tracked_elements: Vec<TrackedElement>,
    pub accessibility: AccessibilitySnapshot,
    pub resources: ResourceCounters,
    pub state: Value,
    pub state_hash: String,
}

impl Snapshot {
    pub fn new(
        target_id: impl Into<String>,
        state_revision: u64,
        state: Value,
    ) -> Result<Self, serde_json::Error> {
        let state_hash = canonical_json_hash(&state)?;
        Ok(Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            target_id: target_id.into(),
            state_revision,
            render_revision: None,
            accessibility_revision: None,
            heartbeat_age_ms: 0,
            screen: None,
            mode: None,
            dialogs: Vec::new(),
            async_activity: Vec::new(),
            tracked_elements: Vec::new(),
            accessibility: AccessibilitySnapshot::default(),
            resources: ResourceCounters::default(),
            state,
            state_hash,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct QueueMetadata {
    pub depth: usize,
    pub high_water: usize,
    pub rejected: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TimingMetadata {
    pub accepted_ns: u64,
    pub queue_ns: u64,
    pub dispatch_ns: u64,
    pub total_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReplyMetadata {
    pub command_sequence: u64,
    pub state_revision_before: u64,
    pub state_revision_after: u64,
    pub render_revision: Option<u64>,
    pub accessibility_revision: Option<u64>,
    pub target_heartbeat_age_ms: Option<u64>,
    pub timing: TimingMetadata,
    pub queue: QueueMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevReply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default)]
    pub meta: ReplyMetadata,
}

impl DevReply {
    pub fn success(value: Value) -> Self {
        Self {
            ok: true,
            value: Some(value),
            error: None,
            error_code: None,
            meta: ReplyMetadata::default(),
        }
    }

    pub fn failure(code: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(error.into()),
            error_code: Some(code.into()),
            meta: ReplyMetadata::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoordinateInput {
    Pointer {
        phase: PointerPhase,
        x: f64,
        y: f64,
        button: u8,
        viewport_revision: u64,
    },
    Touch {
        phase: TouchPhase,
        id: u8,
        x: f64,
        y: f64,
        viewport_revision: u64,
    },
    Scroll {
        delta_x: f64,
        delta_y: f64,
        x: f64,
        y: f64,
        viewport_revision: u64,
    },
    Remote {
        command: String,
        pressed: bool,
    },
}

impl CoordinateInput {
    pub fn validate(&self, limits: &InputLimits) -> Result<(), InputError> {
        match self {
            Self::Pointer { x, y, button, .. } => {
                validate_point(*x, *y, limits)?;
                if *button > limits.max_button {
                    return Err(InputError::Button(*button));
                }
            }
            Self::Touch { id, x, y, .. } => {
                validate_point(*x, *y, limits)?;
                if usize::from(*id) >= limits.max_touches {
                    return Err(InputError::TouchId(*id));
                }
            }
            Self::Scroll {
                delta_x,
                delta_y,
                x,
                y,
                ..
            } => {
                validate_point(*x, *y, limits)?;
                if !delta_x.is_finite()
                    || !delta_y.is_finite()
                    || delta_x.abs() > limits.max_scroll_delta
                    || delta_y.abs() > limits.max_scroll_delta
                {
                    return Err(InputError::Scroll);
                }
            }
            Self::Remote { command, .. } => {
                if !limits.remote_commands.iter().any(|item| item == command) {
                    return Err(InputError::Remote(command.clone()));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PointerPhase {
    Move,
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TouchPhase {
    Begin,
    Move,
    End,
    Cancel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputLimits {
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub max_scroll_delta: f64,
    pub max_button: u8,
    pub max_touches: usize,
    pub remote_commands: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum InputError {
    #[error("coordinate is non-finite or outside the viewport")]
    Coordinate,
    #[error("pointer button {0} is outside the advertised range")]
    Button(u8),
    #[error("touch ID {0} is outside the advertised range")]
    TouchId(u8),
    #[error("scroll delta is non-finite or outside the advertised range")]
    Scroll,
    #[error("remote command {0:?} is not advertised")]
    Remote(String),
}

fn validate_point(x: f64, y: f64, limits: &InputLimits) -> Result<(), InputError> {
    if x.is_finite()
        && y.is_finite()
        && (0.0..=limits.viewport_width).contains(&x)
        && (0.0..=limits.viewport_height).contains(&y)
    {
        Ok(())
    } else {
        Err(InputError::Coordinate)
    }
}

pub fn canonical_json_hash(value: &Value) -> Result<String, serde_json::Error> {
    let canonical = canonicalize(value);
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .iter()
                .map(|(key, value)| (key.clone(), canonicalize(value)))
                .collect();
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn hashes_object_keys_canonically() {
        assert_eq!(
            canonical_json_hash(&json!({"b": 2, "a": {"d": 4, "c": 3}})).unwrap(),
            canonical_json_hash(&json!({"a": {"c": 3, "d": 4}, "b": 2})).unwrap()
        );
    }

    #[test]
    fn coordinate_validation_is_finite_and_bounded() {
        let limits = InputLimits {
            viewport_width: 100.0,
            viewport_height: 50.0,
            max_scroll_delta: 500.0,
            max_button: 2,
            max_touches: 2,
            remote_commands: vec!["select".into()],
        };
        let valid = CoordinateInput::Pointer {
            phase: PointerPhase::Move,
            x: 100.0,
            y: 0.0,
            button: 0,
            viewport_revision: 1,
        };
        assert_eq!(valid.validate(&limits), Ok(()));
        let invalid = CoordinateInput::Scroll {
            delta_x: f64::NAN,
            delta_y: 0.0,
            x: 10.0,
            y: 10.0,
            viewport_revision: 1,
        };
        assert_eq!(invalid.validate(&limits), Err(InputError::Scroll));
    }

    #[test]
    fn protocol_defaults_match_the_advertised_v2_safety_policy() {
        let limits = ProtocolLimits::default();
        assert_eq!(limits.request_line_bytes, 4 * 1024);
        assert_eq!(limits.header_count, 64);
        assert_eq!(limits.header_bytes, 32 * 1024);
        assert_eq!(limits.command_body_bytes, 256 * 1024);
        assert_eq!(limits.fixture_body_bytes, 2 * 1024 * 1024);
        assert_eq!(limits.response_bytes, 4 * 1024 * 1024);
        assert_eq!(limits.read_timeout_ms, 2_000);
        assert_eq!(limits.write_timeout_ms, 2_000);
        assert_eq!(limits.command_timeout_ms, 5_000);
        assert_eq!(limits.active_connections, 16);
        assert_eq!(limits.command_queue, 64);
    }
}
