//! Shared A/B Testing session and runtime-graph orchestration.
//!
//! UI shells own presentation state and background-task scheduling. This
//! controller owns blind trial state and the temporary A/B Compare graph used
//! for playback, so GPUI and TUI cannot drift in routing behavior.

use sotf_audio::plugins::{PluginSettings, PluginType};
use sotf_plugins::plugin_ab_compare::ABComparePluginParams;

use crate::PluginUpdateEffect;
use crate::plugin_graph::{GraphNodeId, NodePosition, PluginGraph, SpecialNodeType};

use super::ab_test_session::{AbTestError, AbTestSession, TrialAnswer, TrialCue, TrialMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbTestPhase {
    Setup,
    Ready,
    Trial(TrialMode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbTestView {
    pub phase: AbTestPhase,
    pub available_cues: Vec<TrialCue>,
    pub available_answers: Vec<TrialAnswer>,
    pub completed_trials: usize,
    pub abx_score: (usize, usize),
    pub runtime_active: bool,
}

#[derive(Debug, Clone, Default)]
struct AbTestRuntime {
    original_graph: Option<PluginGraph>,
    compare_node: Option<GraphNodeId>,
}

#[derive(Debug, Clone, Default)]
pub struct AbTestController {
    session: Option<AbTestSession>,
    runtime: AbTestRuntime,
}

impl AbTestController {
    pub fn session(&self) -> Option<&AbTestSession> {
        self.session.as_ref()
    }

    pub fn session_mut(&mut self) -> Option<&mut AbTestSession> {
        self.session.as_mut()
    }

    pub fn replace_session(&mut self, session: AbTestSession) -> Result<(), AbTestError> {
        if self.runtime.original_graph.is_some() {
            return Err(AbTestError::RuntimeAlreadyActive);
        }
        session.validate()?;
        self.session = Some(session);
        Ok(())
    }

    pub fn clear_session(&mut self) -> Result<(), AbTestError> {
        if self.runtime.original_graph.is_some() {
            return Err(AbTestError::RuntimeAlreadyActive);
        }
        self.session = None;
        Ok(())
    }

    pub fn start_trial(&mut self, mode: TrialMode) -> Result<u32, AbTestError> {
        self.session
            .as_mut()
            .ok_or(AbTestError::InvalidSetup)?
            .start_trial(mode)
    }

    pub fn commit_trial(
        &mut self,
        answer: TrialAnswer,
        confidence: Option<u8>,
        notes: Option<String>,
    ) -> Result<(), AbTestError> {
        self.session
            .as_mut()
            .ok_or(AbTestError::InvalidSetup)?
            .commit_trial(answer, confidence, notes)?;
        Ok(())
    }

    /// Replace the user graph with a minimal, tool-owned A/B Compare runtime.
    ///
    /// Path hosts and fixed level matching are installed once here. Later cue
    /// changes touch only the plugin's binary `selected_path` field.
    pub fn enter_runtime(
        &mut self,
        graph: &mut PluginGraph,
    ) -> Result<PluginUpdateEffect, AbTestError> {
        if self.runtime.original_graph.is_some() {
            return Err(AbTestError::RuntimeAlreadyActive);
        }
        let session = self.session.as_ref().ok_or(AbTestError::InvalidSetup)?;
        let params = session.runtime_setup_config()?;
        let runtime_graph = build_runtime_graph(&params, session.setup.channels)?;
        let original_graph = std::mem::replace(graph, runtime_graph);
        let compare_node = graph
            .nodes
            .values()
            .find(|node| node.plugin.plugin_type() == PluginType::ABCompare)
            .map(|node| node.id)
            .ok_or_else(|| {
                *graph = original_graph.clone();
                AbTestError::RuntimeGraph("comparison node is missing".into())
            })?;
        self.runtime.original_graph = Some(original_graph);
        self.runtime.compare_node = Some(compare_node);
        Ok(PluginUpdateEffect::Structural)
    }

    /// Activate a concealed cue without exposing its A/B assignment to a UI.
    pub fn activate_cue(
        &mut self,
        graph: &mut PluginGraph,
        cue: TrialCue,
    ) -> Result<PluginUpdateEffect, AbTestError> {
        let session = self.session.as_ref().ok_or(AbTestError::InvalidSetup)?;
        let selected = session.path_for_pending_cue(cue)?;
        let node_id = self
            .runtime
            .compare_node
            .ok_or(AbTestError::RuntimeNotActive)?;
        let node = graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| AbTestError::RuntimeGraph("comparison node was removed".into()))?;
        let PluginSettings::ABCompare {
            mix,
            mix_mode,
            selected_path,
            ..
        } = &mut node.plugin.settings
        else {
            return Err(AbTestError::RuntimeGraph(
                "tool-owned node is not A/B Compare".into(),
            ));
        };
        *mix_mode = 1;
        *selected_path = match selected {
            super::ab_test_session::PathSelection::A => 0,
            super::ab_test_session::PathSelection::B => 1,
        };
        *mix = if *selected_path == 0 { -1.0 } else { 1.0 };
        // selected_path is the canonical binary selector. `mix` mirrors it in
        // local state, but the engine needs only the semantic selector update.
        Ok(PluginUpdateEffect::ParameterByNodeId {
            node_id,
            param_index: 2,
        })
    }

    /// Restore the exact graph that was active before the A/B runtime.
    pub fn leave_runtime(
        &mut self,
        graph: &mut PluginGraph,
    ) -> Result<PluginUpdateEffect, AbTestError> {
        let Some(original) = self.runtime.original_graph.take() else {
            return Ok(PluginUpdateEffect::None);
        };
        *graph = original;
        self.runtime.compare_node = None;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn view(&self) -> AbTestView {
        let pending_mode = self.session.as_ref().and_then(AbTestSession::pending_mode);
        let (available_cues, available_answers) = controls_for_mode(pending_mode);
        let completed_trials = self
            .session
            .as_ref()
            .map_or(0, |session| session.trials.len());
        let abx_score = self
            .session
            .as_ref()
            .map_or((0, 0), AbTestSession::abx_score);
        AbTestView {
            phase: match (&self.session, pending_mode) {
                (None, _) => AbTestPhase::Setup,
                (Some(_), None) => AbTestPhase::Ready,
                (Some(_), Some(mode)) => AbTestPhase::Trial(mode),
            },
            available_cues,
            available_answers,
            completed_trials,
            abx_score,
            runtime_active: self.runtime.original_graph.is_some(),
        }
    }
}

fn controls_for_mode(mode: Option<TrialMode>) -> (Vec<TrialCue>, Vec<TrialAnswer>) {
    match mode {
        Some(TrialMode::BlindAb) => (
            vec![TrialCue::First, TrialCue::Second],
            vec![TrialAnswer::First, TrialAnswer::Second],
        ),
        Some(TrialMode::Abx) => (
            vec![
                TrialCue::ReferenceA,
                TrialCue::ReferenceB,
                TrialCue::Unknown,
            ],
            vec![TrialAnswer::A, TrialAnswer::B],
        ),
        None => (Vec::new(), Vec::new()),
    }
}

fn build_runtime_graph(
    params: &ABComparePluginParams,
    channels: usize,
) -> Result<PluginGraph, AbTestError> {
    if channels != 2 {
        // A/B Compare currently reports a fixed stereo layout through
        // PluginGraphNode::channel_counts_for. Reject wider sessions rather
        // than constructing a graph that silently drops channels.
        return Err(AbTestError::InvalidSetup);
    }
    let mut graph = PluginGraph::new();
    let input = graph.add_special_node(
        SpecialNodeType::Input,
        NodePosition::new(0.0, 100.0),
        channels,
    );
    let compare = graph
        .add_plugin_node(&PluginType::ABCompare, NodePosition::new(200.0, 100.0))
        .map_err(AbTestError::RuntimeGraph)?;
    let output = graph.add_special_node(
        SpecialNodeType::Output,
        NodePosition::new(400.0, 100.0),
        channels,
    );

    let node = graph
        .nodes
        .get_mut(&compare)
        .ok_or_else(|| AbTestError::RuntimeGraph("failed to create comparison node".into()))?;
    node.plugin.name = Some("A/B Testing Runtime".into());
    node.plugin.settings = settings_from_params(params)?;
    node.input_channels = channels;
    node.output_channels = channels;

    for channel in 0..channels {
        graph
            .add_connection(input, channel, compare, channel)
            .map_err(AbTestError::RuntimeGraph)?;
        graph
            .add_connection(compare, channel, output, channel)
            .map_err(AbTestError::RuntimeGraph)?;
    }
    Ok(graph)
}

fn settings_from_params(params: &ABComparePluginParams) -> Result<PluginSettings, AbTestError> {
    Ok(PluginSettings::ABCompare {
        mix: f64::from(params.mix),
        mix_mode: match params.mix_mode {
            sotf_plugins::plugin_ab_compare::MixMode::Potentiometer => 0,
            sotf_plugins::plugin_ab_compare::MixMode::Binary => 1,
        },
        selected_path: params.selected_path,
        bypass: params.bypass,
        auto_gain_enabled: params.auto_gain_enabled,
        loudness_type: match params.loudness_type {
            sotf_plugins::plugin_ab_compare::LoudnessType::Momentary => 0,
            sotf_plugins::plugin_ab_compare::LoudnessType::ShortTerm => 1,
        },
        max_auto_gain_db: f64::from(params.max_auto_gain_db),
        gain_smoothing_ms: f64::from(params.gain_smoothing_ms),
        mix_transition_ms: f64::from(params.mix_transition_ms),
        path_a_config: serde_json::to_string(&params.path_a)
            .map_err(|error| AbTestError::Serialization(error.to_string()))?,
        path_b_config: serde_json::to_string(&params.path_b)
            .map_err(|error| AbTestError::Serialization(error.to_string()))?,
        path_a_file: String::new(),
        path_b_file: String::new(),
        phase_invert_a: params.phase_invert_a,
        phase_invert_b: params.phase_invert_b,
        difference_mode: params.difference_mode,
        band_mask_low_hz: f64::from(params.band_mask_low_hz),
        band_mask_high_hz: f64::from(params.band_mask_high_hz),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controllers::ab_test_session::{
        ChainSnapshot, LevelMatchMeasurement, LevelMatchMetric, ListeningTestSetup, MediaSegment,
    };
    use sotf_plugins::plugin_ab_compare::PathConfig;

    fn session() -> AbTestSession {
        AbTestSession::new(
            "test-session",
            ListeningTestSetup {
                path_a: ChainSnapshot::new("A", PathConfig::None).unwrap(),
                path_b: ChainSnapshot::new("B", PathConfig::None).unwrap(),
                media: MediaSegment {
                    media_id: "media".into(),
                    media_path: None,
                    start_ms: 0,
                    duration_ms: 3_000,
                },
                sample_rate: 48_000,
                channels: 2,
                level_match: LevelMatchMeasurement {
                    metric: LevelMatchMetric::ShortTermLufs,
                    window_ms: 3_000,
                    path_a_db: -20.0,
                    path_b_db: -20.0,
                    correction_b_db: 0.0,
                    tolerance_db: 0.1,
                    max_correction_db: 12.0,
                },
                switch_transition_ms: 20.0,
                participant_id: None,
                app_version: "test".into(),
            },
            42,
        )
        .unwrap()
    }

    #[test]
    fn runtime_replaces_and_restores_original_graph() {
        let mut graph = PluginGraph::with_default_rack();
        let original = serde_json::to_value(&graph).unwrap();
        let mut controller = AbTestController::default();
        controller.replace_session(session()).unwrap();

        assert!(matches!(
            controller.enter_runtime(&mut graph).unwrap(),
            PluginUpdateEffect::Structural
        ));
        assert_eq!(graph.nodes.len(), 1);
        assert!(controller.view().runtime_active);

        assert!(matches!(
            controller.leave_runtime(&mut graph).unwrap(),
            PluginUpdateEffect::Structural
        ));
        assert_eq!(serde_json::to_value(&graph).unwrap(), original);
        assert!(!controller.view().runtime_active);
    }

    #[test]
    fn cue_switch_keeps_static_path_configuration() {
        let mut graph = PluginGraph::with_default_rack();
        let mut controller = AbTestController::default();
        controller.replace_session(session()).unwrap();
        controller.start_trial(TrialMode::Abx).unwrap();
        controller.enter_runtime(&mut graph).unwrap();

        let node_id = controller.runtime.compare_node.unwrap();
        let before = match &graph.nodes[&node_id].plugin.settings {
            PluginSettings::ABCompare {
                path_a_config,
                path_b_config,
                ..
            } => (path_a_config.clone(), path_b_config.clone()),
            _ => unreachable!(),
        };
        let effect = controller
            .activate_cue(&mut graph, TrialCue::ReferenceA)
            .unwrap();
        assert!(matches!(
            effect,
            PluginUpdateEffect::ParameterByNodeId {
                node_id: emitted_node_id,
                param_index: 2
            } if emitted_node_id == node_id
        ));
        controller
            .activate_cue(&mut graph, TrialCue::Unknown)
            .unwrap();
        let after = match &graph.nodes[&node_id].plugin.settings {
            PluginSettings::ABCompare {
                path_a_config,
                path_b_config,
                ..
            } => (path_a_config.clone(), path_b_config.clone()),
            _ => unreachable!(),
        };
        assert_eq!(after, before);
    }

    #[test]
    fn runtime_settings_preserve_band_mask() {
        let mut params = ABComparePluginParams::default();
        params.band_mask_low_hz = 123.0;
        params.band_mask_high_hz = 4_567.0;

        let settings = settings_from_params(&params).unwrap();
        match settings {
            PluginSettings::ABCompare {
                band_mask_low_hz,
                band_mask_high_hz,
                ..
            } => {
                assert_eq!(band_mask_low_hz, 123.0);
                assert_eq!(band_mask_high_hz, 4_567.0);
            }
            _ => panic!("expected A/B Compare settings"),
        }
    }

    #[test]
    fn view_exposes_semantic_controls_not_assignments() {
        let mut controller = AbTestController::default();
        controller.replace_session(session()).unwrap();
        controller.start_trial(TrialMode::BlindAb).unwrap();
        let view = controller.view();
        assert_eq!(view.available_cues, vec![TrialCue::First, TrialCue::Second]);
        assert_eq!(
            view.available_answers,
            vec![TrialAnswer::First, TrialAnswer::Second]
        );
    }

    #[test]
    fn runtime_lease_rejects_double_enter_and_leave_is_idempotent() {
        let mut graph = PluginGraph::with_default_rack();
        let mut controller = AbTestController::default();
        controller.replace_session(session()).unwrap();

        controller.enter_runtime(&mut graph).unwrap();
        assert!(matches!(
            controller.enter_runtime(&mut graph),
            Err(AbTestError::RuntimeAlreadyActive)
        ));
        assert!(matches!(
            controller.leave_runtime(&mut graph).unwrap(),
            PluginUpdateEffect::Structural
        ));
        assert!(matches!(
            controller.leave_runtime(&mut graph).unwrap(),
            PluginUpdateEffect::None
        ));
    }

    #[test]
    fn runtime_rejects_non_stereo_without_replacing_graph() {
        let mut session = session();
        session.setup.channels = 6;
        let mut graph = PluginGraph::with_default_rack();
        let original = serde_json::to_value(&graph).unwrap();
        let mut controller = AbTestController::default();
        controller.replace_session(session).unwrap();

        assert!(matches!(
            controller.enter_runtime(&mut graph),
            Err(AbTestError::InvalidSetup)
        ));
        assert_eq!(serde_json::to_value(&graph).unwrap(), original);
        assert!(!controller.view().runtime_active);
    }
}
