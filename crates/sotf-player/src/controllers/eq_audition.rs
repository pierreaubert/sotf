//! Reversible, output-stage EQ audition for both linear and routed graphs.
use crate::plugin_graph::{GraphNodeId, NodePosition, NodeRole, PluginGraph};
use crate::{EQFilter, PluginSettings, PluginType};
use math_audio_iir_fir::{Biquad, BiquadFilterType, peq_preamp_gain_max};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeKind {
    Start,
    Edit,
    Stop,
    CancelStart,
}

#[derive(Debug, Clone)]
struct PendingChange {
    graph: PluginGraph,
    corrected: bool,
    preamp_db: f64,
    kind: ChangeKind,
}

/// Owns only the two temporary nodes. Engine updates must be acknowledged via
/// `finish_update`; a failed update restores the last audible graph and mode.
#[derive(Debug, Clone)]
pub struct EqAudition {
    gain_node: GraphNodeId,
    eq_node: GraphNodeId,
    pub corrected: bool,
    pub preamp_db: f64,
    pub max_preamp_db: f64,
    pub sample_rate_hz: f64,
    pub stop_failed: bool,
    original_graph: PluginGraph,
    expected_graph: Option<serde_json::Value>,
    external_changes: bool,
    pending: Option<PendingChange>,
}

impl EqAudition {
    pub fn start(
        graph: &mut PluginGraph,
        filters: &[(String, f64, f64, f64)],
        sample_rate_hz: f64,
        corrected: bool,
    ) -> Result<Self, String> {
        if !sample_rate_hz.is_finite() || sample_rate_hz < 8_000.0 {
            return Err("A valid playback sample rate is required for audition".into());
        }
        let mut eq_filters = Vec::with_capacity(filters.len());
        let mut peq = Vec::with_capacity(filters.len());
        for (kind, frequency, q, gain_db) in filters {
            if !frequency.is_finite()
                || *frequency <= 0.0
                || *frequency >= sample_rate_hz / 2.0
                || !q.is_finite()
                || *q <= 0.0
                || !gain_db.is_finite()
            {
                return Err("EQ audition filters must be finite and below playback Nyquist".into());
            }
            let kind = match kind.to_ascii_lowercase().as_str() {
                "peak" | "pk" => BiquadFilterType::Peak,
                "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                "highshelf" | "hs" => BiquadFilterType::Highshelf,
                "lowpass" | "lp" => BiquadFilterType::Lowpass,
                "highpass" | "hp" => BiquadFilterType::Highpass,
                "bandpass" | "bp" => BiquadFilterType::Bandpass,
                "notch" | "no" => BiquadFilterType::Notch,
                "allpass" | "ap" => BiquadFilterType::AllPass,
                _ => return Err(format!("Unsupported audition filter: {kind}")),
            };
            eq_filters.push(EQFilter::new(kind, *frequency, *q, *gain_db));
            peq.push((
                1.0,
                Biquad::new(kind, *frequency, sample_rate_hz, *q, *gain_db),
            ));
        }
        let preamp_db = peq_preamp_gain_max(&peq).min(0.0);
        if !preamp_db.is_finite() || preamp_db < -120.0 {
            return Err("EQ audition requires unsupported preamp attenuation".into());
        }
        let mut candidate = graph.clone();
        let target = candidate
            .nodes
            .values()
            .find(|node| node.role == NodeRole::OutputMonitor)
            .map(|node| node.id)
            .or_else(|| candidate.output_node_id())
            .ok_or("Audition requires an output node")?;
        let channels = candidate.node_input_channels(target);
        if channels == 0 {
            return Err("Audition requires at least one output channel".into());
        }
        let gain_node =
            candidate.add_plugin_node(&PluginType::Gain, NodePosition::new(0.0, 0.0))?;
        let eq_node = candidate.add_plugin_node(&PluginType::EQ, NodePosition::new(0.0, 0.0))?;
        if let Some(node) = candidate.nodes.get_mut(&gain_node) {
            node.plugin.name = Some("Headphone audition preamp".into());
            node.plugin.settings = PluginSettings::Gain {
                channels,
                gain_db: preamp_db,
                smoothing_ms: 20.0,
            };
        }
        if let Some(node) = candidate.nodes.get_mut(&eq_node) {
            node.plugin.name = Some("Headphone audition EQ".into());
            node.plugin.enabled = corrected;
            node.plugin.settings = PluginSettings::EQ {
                channels,
                max_filters: eq_filters.len().max(1),
                filters: eq_filters,
                channel_filters: None,
                per_channel_mode: false,
                tdf2: false,
                topology: 0.0,
                auto_gain_enabled: false,
                oversampling: 1.0,
            };
        }
        for edge in &mut candidate.connections {
            if edge.to_node == target {
                edge.to_node = gain_node;
            }
        }
        for port in 0..channels {
            candidate.add_connection(gain_node, port, eq_node, port)?;
            candidate.add_connection(eq_node, port, target, port)?;
        }
        candidate.validate_routing()?;
        let pending = PendingChange {
            graph: graph.clone(),
            corrected: false,
            preamp_db,
            kind: ChangeKind::Start,
        };
        *graph = candidate;
        Ok(Self {
            gain_node,
            eq_node,
            corrected,
            preamp_db,
            max_preamp_db: preamp_db,
            sample_rate_hz,
            stop_failed: false,
            original_graph: pending.graph.clone(),
            expected_graph: serde_json::to_value(&*graph).ok(),
            external_changes: false,
            pending: Some(pending),
        })
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn is_canceling(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.kind == ChangeKind::CancelStart)
    }
    pub fn is_stopping(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.kind == ChangeKind::Stop)
    }

    pub fn set_corrected(
        &mut self,
        graph: &mut PluginGraph,
        corrected: bool,
    ) -> Result<(), String> {
        self.begin_change(graph, ChangeKind::Edit)?;
        let Some(node) = graph.nodes.get_mut(&self.eq_node) else {
            self.pending = None;
            return Err("Audition EQ is no longer in the graph".into());
        };
        node.plugin.enabled = corrected;
        self.corrected = corrected;
        self.expected_graph = serde_json::to_value(&*graph).ok();
        Ok(())
    }

    pub fn set_preamp(&mut self, graph: &mut PluginGraph, preamp_db: f64) -> Result<(), String> {
        if !preamp_db.is_finite() || !(-120.0..=self.max_preamp_db).contains(&preamp_db) {
            return Err("Preamp must retain the calculated EQ headroom".into());
        }
        self.begin_change(graph, ChangeKind::Edit)?;
        let Some(node) = graph.nodes.get_mut(&self.gain_node) else {
            self.pending = None;
            return Err("Audition preamp is no longer in the graph".into());
        };
        let PluginSettings::Gain { gain_db, .. } = &mut node.plugin.settings else {
            self.pending = None;
            return Err("Audition preamp settings were replaced".into());
        };
        *gain_db = preamp_db;
        self.preamp_db = preamp_db;
        self.expected_graph = serde_json::to_value(&*graph).ok();
        Ok(())
    }

    pub fn stop(&mut self, graph: &mut PluginGraph) -> Result<(), String> {
        self.note_external_changes(graph);
        let previous = self.pending.clone();
        let kind = if previous.as_ref().is_some_and(|pending| {
            matches!(pending.kind, ChangeKind::Start | ChangeKind::CancelStart)
        }) {
            ChangeKind::CancelStart
        } else {
            ChangeKind::Stop
        };
        let mut candidate = graph.clone();
        let incoming: Vec<_> = candidate
            .connections
            .iter()
            .filter(|edge| edge.to_node == self.gain_node)
            .cloned()
            .collect();
        let middle: Vec<_> = candidate
            .connections
            .iter()
            .filter(|edge| edge.from_node == self.gain_node && edge.to_node == self.eq_node)
            .cloned()
            .collect();
        let outgoing: Vec<_> = candidate
            .connections
            .iter()
            .filter(|edge| edge.from_node == self.eq_node)
            .cloned()
            .collect();
        candidate.remove_node(self.gain_node);
        candidate.remove_node(self.eq_node);
        for input in &incoming {
            for link in middle.iter().filter(|edge| edge.from_port == input.to_port) {
                for output in outgoing
                    .iter()
                    .filter(|edge| edge.from_port == link.to_port)
                {
                    candidate.add_connection(
                        input.from_node,
                        input.from_port,
                        output.to_node,
                        output.to_port,
                    )?;
                }
            }
        }
        candidate.validate_routing()?;
        if !self.external_changes {
            // Preserve connection identities and the ID allocator as well as
            // topology, so a pre-existing routing draft does not become stale.
            candidate = self.original_graph.clone();
        }
        self.stop_failed = false;
        self.pending = Some(previous.map_or_else(
            || PendingChange {
                graph: graph.clone(),
                corrected: self.corrected,
                preamp_db: self.preamp_db,
                kind,
            },
            |pending| PendingChange { kind, ..pending },
        ));
        *graph = candidate;
        self.expected_graph = serde_json::to_value(&*graph).ok();
        Ok(())
    }

    fn note_external_changes(&mut self, graph: &PluginGraph) {
        if self.expected_graph.is_none() || serde_json::to_value(graph).ok() != self.expected_graph
        {
            self.external_changes = true;
        }
    }

    fn begin_change(&mut self, graph: &PluginGraph, kind: ChangeKind) -> Result<(), String> {
        if self.is_pending() {
            return Err("An audition update is still pending".into());
        }
        self.note_external_changes(graph);
        self.stop_failed = false;
        self.pending = Some(PendingChange {
            graph: graph.clone(),
            corrected: self.corrected,
            preamp_db: self.preamp_db,
            kind,
        });
        Ok(())
    }

    /// Return whether this session remains active after the engine acknowledgement.
    pub fn finish_update(&mut self, graph: &mut PluginGraph, succeeded: bool) -> bool {
        let Some(pending) = self.pending.take() else {
            return true;
        };
        if succeeded {
            return !matches!(pending.kind, ChangeKind::Stop | ChangeKind::CancelStart);
        }
        *graph = pending.graph;
        self.expected_graph = serde_json::to_value(&*graph).ok();
        self.corrected = pending.corrected;
        self.preamp_db = pending.preamp_db;
        self.stop_failed = pending.kind == ChangeKind::Stop;
        !matches!(pending.kind, ChangeKind::Start | ChangeKind::CancelStart)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters() -> Vec<(String, f64, f64, f64)> {
        vec![
            ("peak".into(), 1000.0, 1.0, 6.0),
            ("peak".into(), 1000.0, 1.0, 6.0),
        ]
    }

    fn routes(graph: &PluginGraph) -> Vec<(GraphNodeId, usize, GraphNodeId, usize)> {
        let mut routes: Vec<_> = graph
            .connections
            .iter()
            .map(|edge| (edge.from_node, edge.from_port, edge.to_node, edge.to_port))
            .collect();
        routes.sort();
        routes
    }

    #[test]
    fn audition_keeps_equal_preamp_in_both_modes_and_restores_routing() {
        let mut graph = PluginGraph::with_default_rack();
        let original = graph.clone();
        let mut audition = EqAudition::start(&mut graph, &filters(), 48000.0, true).unwrap();
        assert!(
            audition.preamp_db < -11.9,
            "overlapping boosts require combined headroom"
        );
        assert!(audition.finish_update(&mut graph, true));
        let preamp = graph.nodes[&audition.gain_node].plugin.settings.clone();
        audition.set_corrected(&mut graph, false).unwrap();
        assert!(!graph.nodes[&audition.eq_node].plugin.enabled);
        assert_eq!(
            serde_json::to_value(preamp).unwrap(),
            serde_json::to_value(&graph.nodes[&audition.gain_node].plugin.settings).unwrap()
        );
        audition.finish_update(&mut graph, true);
        audition.stop(&mut graph).unwrap();
        assert!(!audition.finish_update(&mut graph, true));
        assert_eq!(routes(&graph), routes(&original));
        assert_eq!(graph.nodes.len(), original.nodes.len());
        assert_eq!(
            serde_json::to_value(&graph).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
    }

    #[test]
    fn audition_preserves_parallel_routes_and_unrelated_edits() {
        let mut graph = PluginGraph::with_default_rack();
        let output = graph
            .nodes
            .values()
            .find(|node| node.role == NodeRole::OutputMonitor)
            .unwrap()
            .id;
        let source = graph
            .connections
            .iter()
            .find(|edge| edge.to_node == output && edge.to_port == 0)
            .unwrap()
            .from_node;
        let branch = graph
            .add_plugin_node(&PluginType::Gain, NodePosition::new(0.0, 0.0))
            .unwrap();
        for port in 0..2 {
            graph.add_connection(source, port, branch, port).unwrap();
            graph.add_connection(branch, port, output, port).unwrap();
        }
        let original_routes = routes(&graph);
        let mut audition = EqAudition::start(&mut graph, &filters(), 48000.0, true).unwrap();
        audition.finish_update(&mut graph, true);
        if let PluginSettings::Gain { gain_db, .. } =
            &mut graph.nodes.get_mut(&branch).unwrap().plugin.settings
        {
            *gain_db = -3.0;
        }
        audition.stop(&mut graph).unwrap();
        audition.finish_update(&mut graph, true);
        assert_eq!(routes(&graph), original_routes);
        assert!(matches!(
            graph.nodes[&branch].plugin.settings,
            PluginSettings::Gain { gain_db: -3.0, .. }
        ));
    }

    #[test]
    fn engine_rejection_restores_last_audible_state_and_stop_can_retry() {
        let mut graph = PluginGraph::with_default_rack();
        let original = serde_json::to_value(&graph).unwrap();
        let mut audition = EqAudition::start(&mut graph, &filters(), 48000.0, true).unwrap();
        assert!(!audition.finish_update(&mut graph, false));
        assert_eq!(serde_json::to_value(&graph).unwrap(), original);
        let mut audition = EqAudition::start(&mut graph, &filters(), 48000.0, true).unwrap();
        audition.finish_update(&mut graph, true);
        let audible = serde_json::to_value(&graph).unwrap();
        audition.set_corrected(&mut graph, false).unwrap();
        assert!(audition.finish_update(&mut graph, false));
        assert!(audition.corrected);
        assert_eq!(serde_json::to_value(&graph).unwrap(), audible);
        audition.stop(&mut graph).unwrap();
        assert!(audition.finish_update(&mut graph, false));
        assert!(audition.stop_failed);
        assert_eq!(serde_json::to_value(&graph).unwrap(), audible);
        audition.stop(&mut graph).unwrap();
        assert!(!audition.finish_update(&mut graph, true));
    }

    #[test]
    fn closing_before_start_is_applied_cancels_preview() {
        let mut graph = PluginGraph::with_default_rack();
        let original = serde_json::to_value(&graph).unwrap();
        let mut audition = EqAudition::start(&mut graph, &filters(), 48000.0, true).unwrap();
        audition.stop(&mut graph).unwrap();
        assert_eq!(serde_json::to_value(&graph).unwrap(), original);
        assert!(!audition.finish_update(&mut graph, true));
    }

    #[test]
    fn invalid_filters_and_preamp_cannot_mutate_playback() {
        let mut graph = PluginGraph::with_default_rack();
        let original = serde_json::to_value(&graph).unwrap();
        assert!(
            EqAudition::start(
                &mut graph,
                &[("peak".into(), 24000.0, 1.0, 6.0)],
                48000.0,
                true
            )
            .is_err()
        );
        assert!(
            EqAudition::start(
                &mut graph,
                &[("unknown".into(), 1000.0, 1.0, 6.0)],
                48000.0,
                true
            )
            .is_err()
        );
        assert_eq!(serde_json::to_value(&graph).unwrap(), original);
        let mut audition = EqAudition::start(&mut graph, &filters(), 48000.0, true).unwrap();
        audition.finish_update(&mut graph, true);
        let preview = serde_json::to_value(&graph).unwrap();
        assert!(audition.set_preamp(&mut graph, 0.0).is_err());
        assert!(audition.set_preamp(&mut graph, f64::NAN).is_err());
        assert_eq!(serde_json::to_value(&graph).unwrap(), preview);
    }
}
