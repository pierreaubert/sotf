use super::model::{Action, FailureSignature};

pub trait ReplayOracle {
    fn replay(&mut self, actions: &[Action]) -> Option<FailureSignature>;
}

#[derive(Debug, Clone)]
pub struct Confirmation {
    pub outcomes: Vec<Option<FailureSignature>>,
    pub matches: u8,
}

pub fn confirm_two_of_three(
    oracle: &mut impl ReplayOracle,
    actions: &[Action],
    expected: &FailureSignature,
) -> Confirmation {
    let mut outcomes = Vec::with_capacity(3);
    let mut matches = 0;
    for _ in 0..3 {
        let outcome = oracle.replay(actions);
        if outcome.as_ref() == Some(expected) {
            matches += 1;
        }
        outcomes.push(outcome);
    }
    Confirmation { outcomes, matches }
}

pub fn minimize_actions(
    oracle: &mut impl ReplayOracle,
    original: &[Action],
    expected: &FailureSignature,
) -> Vec<Action> {
    let mut current = original.to_vec();
    let mut granularity = 2usize;
    while current.len() >= 2 {
        let chunk_size = current.len().div_ceil(granularity);
        let mut reduced = false;
        for start in (0..current.len()).step_by(chunk_size) {
            let end = (start + chunk_size).min(current.len());
            let mut candidate = current[..start].to_vec();
            candidate.extend_from_slice(&current[end..]);
            if candidate.is_empty() {
                continue;
            }
            if confirm_two_of_three(oracle, &candidate, expected).matches >= 2 {
                current = candidate;
                granularity = granularity.saturating_sub(1).max(2);
                reduced = true;
                break;
            }
        }
        if !reduced {
            if granularity >= current.len() {
                break;
            }
            granularity = (granularity * 2).min(current.len());
        }
    }
    shrink_payloads(oracle, current, expected)
}

fn shrink_payloads(
    oracle: &mut impl ReplayOracle,
    mut actions: Vec<Action>,
    expected: &FailureSignature,
) -> Vec<Action> {
    loop {
        let mut accepted = false;
        'actions: for index in 0..actions.len() {
            for payload in payload_candidates(&actions[index].payload) {
                let mut candidate = actions.clone();
                candidate[index].payload = payload;
                if confirm_two_of_three(oracle, &candidate, expected).matches >= 2 {
                    actions = candidate;
                    accepted = true;
                    break 'actions;
                }
            }
        }
        if !accepted {
            return actions;
        }
    }
}

fn payload_candidates(payload: &super::model::ActionPayload) -> Vec<super::model::ActionPayload> {
    use super::model::ActionPayload;

    match payload {
        ActionPayload::DevAction { name, payload } => shrink_json_once(payload)
            .into_iter()
            .map(|payload| ActionPayload::DevAction {
                name: name.clone(),
                payload,
            })
            .collect(),
        ActionPayload::Text { text } if !text.is_empty() => vec![ActionPayload::Text {
            text: text.chars().take(text.chars().count() / 2).collect(),
        }],
        ActionPayload::Wait { duration_ms } if *duration_ms > 0 => {
            vec![ActionPayload::Wait { duration_ms: 0 }]
        }
        ActionPayload::ProcessArgv { argv } if argv.len() > 1 => vec![ActionPayload::ProcessArgv {
            argv: argv[..argv.len().div_ceil(2)].to_vec(),
        }],
        ActionPayload::Stdin { bytes, eof } if !bytes.is_empty() => vec![ActionPayload::Stdin {
            bytes: bytes[..bytes.len() / 2].to_vec(),
            eof: *eof,
        }],
        ActionPayload::Http {
            endpoint,
            method,
            path,
            headers,
            body,
        } => {
            let mut candidates = Vec::new();
            for header in headers.keys() {
                let mut smaller = headers.clone();
                smaller.remove(header);
                candidates.push(ActionPayload::Http {
                    endpoint: endpoint.clone(),
                    method: method.clone(),
                    path: path.clone(),
                    headers: smaller,
                    body: body.clone(),
                });
            }
            if !body.is_empty() {
                candidates.push(ActionPayload::Http {
                    endpoint: endpoint.clone(),
                    method: method.clone(),
                    path: path.clone(),
                    headers: headers.clone(),
                    body: body[..body.len() / 2].to_vec(),
                });
            }
            candidates
        }
        ActionPayload::Ipc { command } => shrink_json_once(command)
            .into_iter()
            .map(|command| ActionPayload::Ipc { command })
            .collect(),
        ActionPayload::Coordinate { input } => {
            use sotf_dev_api::CoordinateInput;
            let zeroed = match input {
                CoordinateInput::Pointer {
                    phase,
                    button,
                    viewport_revision,
                    ..
                } => CoordinateInput::Pointer {
                    phase: *phase,
                    x: 0.0,
                    y: 0.0,
                    button: *button,
                    viewport_revision: *viewport_revision,
                },
                CoordinateInput::Touch {
                    phase,
                    id,
                    viewport_revision,
                    ..
                } => CoordinateInput::Touch {
                    phase: *phase,
                    id: *id,
                    x: 0.0,
                    y: 0.0,
                    viewport_revision: *viewport_revision,
                },
                CoordinateInput::Scroll {
                    viewport_revision, ..
                } => CoordinateInput::Scroll {
                    delta_x: 0.0,
                    delta_y: 0.0,
                    x: 0.0,
                    y: 0.0,
                    viewport_revision: *viewport_revision,
                },
                CoordinateInput::Remote { .. } => return Vec::new(),
            };
            (zeroed != *input)
                .then_some(ActionPayload::Coordinate { input: zeroed })
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

fn shrink_json_once(value: &serde_json::Value) -> Vec<serde_json::Value> {
    use serde_json::Value;

    match value {
        Value::Object(map) => {
            let mut candidates = Vec::new();
            for key in map.keys() {
                let mut smaller = map.clone();
                smaller.remove(key);
                candidates.push(Value::Object(smaller));
            }
            for (key, child) in map {
                for smaller_child in shrink_json_once(child) {
                    let mut smaller = map.clone();
                    smaller.insert(key.clone(), smaller_child);
                    candidates.push(Value::Object(smaller));
                }
            }
            candidates
        }
        Value::Array(values) if !values.is_empty() => {
            vec![Value::Array(values[..values.len() / 2].to_vec())]
        }
        Value::String(text) if !text.is_empty() => vec![Value::String(
            text.chars().take(text.chars().count() / 2).collect(),
        )],
        Value::Number(number) if number.as_f64() != Some(0.0) => vec![serde_json::json!(0)],
        Value::Bool(true) => vec![Value::Bool(false)],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sotf_dev_api::{CoordinateInput, PointerPhase};

    use crate::fuzz::model::{ActionClass, ActionPayload, FUZZ_SCHEMA_VERSION, FailureClass};

    use super::*;

    struct ContainsCrash;

    struct JsonCrash;

    impl ReplayOracle for ContainsCrash {
        fn replay(&mut self, actions: &[Action]) -> Option<FailureSignature> {
            actions
                .iter()
                .any(|action| action.id == "crash")
                .then(|| FailureSignature {
                    class: FailureClass::SignalOrException,
                    normalized: "crash".into(),
                })
        }
    }

    impl ReplayOracle for JsonCrash {
        fn replay(&mut self, actions: &[Action]) -> Option<FailureSignature> {
            actions
                .iter()
                .any(|action| {
                    matches!(
                        &action.payload,
                        ActionPayload::DevAction { payload, .. }
                            if payload.get("crash") == Some(&json!(true))
                    )
                })
                .then(|| FailureSignature {
                    class: FailureClass::SignalOrException,
                    normalized: "json-crash".into(),
                })
        }
    }

    fn action(id: &str) -> Action {
        Action {
            schema_version: FUZZ_SCHEMA_VERSION,
            sequence: 1,
            id: id.into(),
            family: "test".into(),
            class: ActionClass::StateValid,
            precondition_id: None,
            precondition_satisfied: true,
            payload: ActionPayload::Wait { duration_ms: 10 },
            timeout_ms: 100,
            rng_cursor: 1,
        }
    }

    #[test]
    fn delta_minimizes_with_two_of_three_confirmation() {
        let signature = FailureSignature {
            class: FailureClass::SignalOrException,
            normalized: "crash".into(),
        };
        let minimized = minimize_actions(
            &mut ContainsCrash,
            &[action("a"), action("crash"), action("b")],
            &signature,
        );
        assert_eq!(minimized.len(), 1);
        assert_eq!(minimized[0].id, "crash");
    }

    #[test]
    fn removes_payload_fields_and_shrinks_coordinates() {
        let json_signature = FailureSignature {
            class: FailureClass::SignalOrException,
            normalized: "json-crash".into(),
        };
        let mut json_action = action("json-crash");
        json_action.payload = ActionPayload::DevAction {
            name: "trigger".into(),
            payload: json!({"crash": true, "noise": 1234, "label": "unneeded"}),
        };
        let minimized = minimize_actions(&mut JsonCrash, &[json_action], &json_signature);
        assert_eq!(
            minimized[0].payload,
            ActionPayload::DevAction {
                name: "trigger".into(),
                payload: json!({"crash": true}),
            }
        );

        let signature = FailureSignature {
            class: FailureClass::SignalOrException,
            normalized: "crash".into(),
        };
        let mut coordinate = action("crash");
        coordinate.payload = ActionPayload::Coordinate {
            input: CoordinateInput::Pointer {
                phase: PointerPhase::Move,
                x: 640.0,
                y: 480.0,
                button: 0,
                viewport_revision: 3,
            },
        };
        let minimized = minimize_actions(&mut ContainsCrash, &[coordinate], &signature);
        assert!(matches!(
            minimized[0].payload,
            ActionPayload::Coordinate {
                input: CoordinateInput::Pointer { x: 0.0, y: 0.0, .. }
            }
        ));
    }
}
