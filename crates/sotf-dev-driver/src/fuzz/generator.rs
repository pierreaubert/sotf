use std::collections::{BTreeMap, BTreeSet};

use sotf_dev_api::Snapshot;
use thiserror::Error;

use super::manifest::{ManifestAction, SurfaceManifest};
use super::model::{Action, ActionClass, FUZZ_SCHEMA_VERSION};

pub const GENERATOR_VERSION: u16 = 1;
pub const STATE_VALID_PERCENT: u64 = 85;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
    cursor: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed,
            cursor: 0,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        self.cursor = self.cursor.wrapping_add(1);
        value ^ (value >> 31)
    }

    pub fn cursor(self) -> u64 {
        self.cursor
    }
}

pub fn derive_worker_seed(root_seed: u64, worker: u32) -> u64 {
    let mut rng = SplitMix64::new(root_seed ^ u64::from(worker).rotate_left(32));
    rng.next_u64()
}

pub fn class_for_roll(roll: u64) -> ActionClass {
    if roll % 100 < STATE_VALID_PERCENT {
        ActionClass::StateValid
    } else {
        ActionClass::BoundedChaos
    }
}

#[derive(Debug, Clone, Default)]
pub struct CoverageState {
    counts: BTreeMap<String, u64>,
    seen_state_hashes: BTreeSet<String>,
}

impl CoverageState {
    pub fn record(&mut self, keys: impl IntoIterator<Item = String>) {
        for key in keys {
            *self.counts.entry(key).or_default() += 1;
        }
    }

    pub fn record_state_hash(&mut self, hash: impl Into<String>) -> bool {
        self.seen_state_hashes.insert(hash.into())
    }

    pub fn count(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    pub fn counters(&self) -> &BTreeMap<String, u64> {
        &self.counts
    }
}

#[derive(Debug, Clone)]
pub struct Generator {
    rng: SplitMix64,
    sequence: u64,
    pub coverage: CoverageState,
}

impl Generator {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SplitMix64::new(seed),
            sequence: 0,
            coverage: CoverageState::default(),
        }
    }

    pub fn next_action(
        &mut self,
        manifest: &SurfaceManifest,
        supported: &BTreeSet<String>,
        snapshot: &Snapshot,
    ) -> Result<Action, GeneratorError> {
        let selected_class = class_for_roll(self.rng.next_u64());
        let mut candidates: Vec<_> = manifest
            .actions
            .iter()
            .filter(|action| supported.contains(&action.id))
            .filter(|action| match selected_class {
                ActionClass::StateValid => {
                    !action.recovery && !action.chaos_only && action.precondition.evaluate(snapshot)
                }
                ActionClass::BoundedChaos => !action.recovery,
                ActionClass::Recovery => action.recovery,
            })
            .collect();

        let class = if candidates.is_empty() && selected_class == ActionClass::StateValid {
            candidates = manifest
                .actions
                .iter()
                .filter(|action| supported.contains(&action.id) && action.recovery)
                .filter(|action| action.precondition.evaluate(snapshot))
                .collect();
            if candidates.is_empty() {
                return Err(GeneratorError::NoValidAction);
            }
            ActionClass::Recovery
        } else if candidates.is_empty() {
            return Err(GeneratorError::NoChaosAction);
        } else {
            selected_class
        };

        let selected = choose_weighted(&mut self.rng, &self.coverage, &candidates)
            .ok_or(GeneratorError::WeightOverflow)?;
        self.sequence = self.sequence.wrapping_add(1);
        Ok(Action {
            schema_version: FUZZ_SCHEMA_VERSION,
            sequence: self.sequence,
            id: selected.id.clone(),
            family: selected.family.clone(),
            class,
            precondition_id: selected.precondition_id.clone(),
            precondition_satisfied: selected.precondition.evaluate(snapshot),
            payload: selected.payload.clone(),
            timeout_ms: selected.timeout_ms,
            rng_cursor: self.rng.cursor(),
        })
    }
}

fn choose_weighted<'a>(
    rng: &mut SplitMix64,
    coverage: &CoverageState,
    candidates: &[&'a ManifestAction],
) -> Option<&'a ManifestAction> {
    let weights: Vec<_> = candidates
        .iter()
        .map(|action| {
            let rarity = 1_000 / (1 + coverage.count(&format!("action:{}", action.id))).min(1_000);
            u64::from(action.weight).saturating_add(rarity).min(20_000)
        })
        .collect();
    let total = weights
        .iter()
        .try_fold(0_u64, |sum, weight| sum.checked_add(*weight))?;
    if total == 0 {
        return None;
    }
    let mut roll = rng.next_u64() % total;
    for (candidate, weight) in candidates.iter().zip(weights) {
        if roll < weight {
            return Some(*candidate);
        }
        roll -= weight;
    }
    candidates.last().copied()
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GeneratorError {
    #[error("no state-valid or recovery action is currently available")]
    NoValidAction,
    #[error("no bounded-chaos action is currently available")]
    NoChaosAction,
    #[error("action weights overflowed")]
    WeightOverflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::{Value, json};
    use sotf_dev_api::Snapshot;

    use super::*;
    use crate::fuzz::manifest::{Condition, ManifestAction};
    use crate::fuzz::model::{ActionPayload, TargetId};

    fn action(id: &str, condition: Condition, recovery: bool) -> ManifestAction {
        ManifestAction {
            id: id.into(),
            family: "test".into(),
            weight: 100,
            precondition_id: Some(format!("{id}-precondition")),
            precondition: condition,
            recovery,
            chaos_only: false,
            payload: ActionPayload::DevAction {
                name: id.into(),
                payload: Value::Null,
            },
            timeout_ms: 100,
            coverage: vec![],
        }
    }

    #[test]
    fn class_boundary_is_exactly_eighty_five_fifteen() {
        let valid = (0..100)
            .filter(|roll| class_for_roll(*roll) == ActionClass::StateValid)
            .count();
        assert_eq!(valid, 85);
        assert_eq!(class_for_roll(84), ActionClass::StateValid);
        assert_eq!(class_for_roll(85), ActionClass::BoundedChaos);
    }

    #[test]
    fn generation_is_deterministic() {
        let manifest = SurfaceManifest {
            schema_version: 1,
            version: 1,
            target: TargetId::Tui,
            fixture_profiles: vec![],
            actions: vec![
                action("a", Condition::Always, false),
                action("b", Condition::Always, false),
            ],
            invariants: vec![],
            workflows: vec![],
        };
        let supported = BTreeSet::from(["a".into(), "b".into()]);
        let snapshot = Snapshot::new("tui", 1, json!({})).unwrap();
        let generate = || {
            let mut generator = Generator::new(42);
            (0..32)
                .map(|_| {
                    generator
                        .next_action(&manifest, &supported, &snapshot)
                        .unwrap()
                        .id
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(generate(), generate());
        assert_eq!(derive_worker_seed(42, 2), derive_worker_seed(42, 2));
        assert_ne!(derive_worker_seed(42, 1), derive_worker_seed(42, 2));
    }

    #[test]
    fn rarity_weighting_prefers_novel_actions_without_starvation() {
        let common = action("common", Condition::Always, false);
        let rare = action("rare", Condition::Always, false);
        let candidates = [&common, &rare];
        let mut coverage = CoverageState::default();
        coverage.record((0..1_000).map(|_| "action:common".to_owned()));
        let mut rng = SplitMix64::new(99);
        let mut common_count = 0;
        let mut rare_count = 0;
        for _ in 0..2_000 {
            match choose_weighted(&mut rng, &coverage, &candidates)
                .unwrap()
                .id
                .as_str()
            {
                "common" => common_count += 1,
                "rare" => rare_count += 1,
                other => panic!("unexpected action {other}"),
            }
        }
        assert!(
            rare_count > common_count,
            "rare={rare_count} common={common_count}"
        );
        assert!(
            common_count > 0,
            "rarity weighting starved the common action"
        );
    }

    #[test]
    fn chaos_only_actions_never_enter_the_state_valid_branch() {
        let valid = action("valid", Condition::Always, false);
        let mut malformed = action("malformed", Condition::Always, false);
        malformed.chaos_only = true;
        let manifest = SurfaceManifest {
            schema_version: 1,
            version: 1,
            target: TargetId::Tui,
            fixture_profiles: vec![],
            actions: vec![valid, malformed],
            invariants: vec![],
            workflows: vec![],
        };
        let supported = BTreeSet::from(["valid".into(), "malformed".into()]);
        let snapshot = Snapshot::new("tui", 1, json!({})).unwrap();
        let mut generator = Generator::new(3);
        for _ in 0..2_000 {
            let action = generator
                .next_action(&manifest, &supported, &snapshot)
                .unwrap();
            if action.class == ActionClass::StateValid {
                assert_eq!(action.id, "valid");
            }
        }
    }

    #[test]
    fn falls_back_to_declared_recovery_only_when_valid_set_is_empty() {
        let manifest = SurfaceManifest {
            schema_version: 1,
            version: 1,
            target: TargetId::Tui,
            fixture_profiles: vec![],
            actions: vec![
                action(
                    "blocked",
                    Condition::Equals {
                        path: "state.ready".into(),
                        value: json!(true),
                    },
                    false,
                ),
                action("restart", Condition::Always, true),
            ],
            invariants: vec![],
            workflows: vec![],
        };
        let supported = BTreeSet::from(["blocked".into(), "restart".into()]);
        let snapshot = Snapshot::new("tui", 1, json!({"ready": false})).unwrap();
        let mut generator = Generator::new(1);
        for _ in 0..100 {
            let selected = generator.next_action(&manifest, &supported, &snapshot);
            if let Ok(selected) = selected
                && selected.class == ActionClass::Recovery
            {
                assert_eq!(selected.id, "restart");
                return;
            }
        }
        panic!("seed did not exercise the state-valid branch");
    }
}
