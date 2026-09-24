//! Integration tests for the RoomEQ per-route graph builder against the
//! real-world `gen514` 5.1.4 home-cinema fixture.
//!
//! The fixture is a slim of `data_generated/gen514/dsp.json` with the large
//! curve / IR / EPA arrays stripped — only the structural data the graph
//! builder consumes (per-channel plugin chains, routing_graph, scalar
//! bass-management metadata) is retained.

use std::path::PathBuf;

use sotf_audio_player::autoeq::DspChainOutput;
use sotf_audio_player::room_eq_types::build_room_eq_plugin_graph_config;

fn load_gen514_fixture() -> DspChainOutput {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("roomeq_gen514_dsp.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("parse fixture {}: {e}", path.display()))
}

#[test]
fn gen514_per_route_graph_preserves_ports_and_route_count() {
    let output = load_gen514_fixture();
    let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let labels: Vec<_> = config
        .nodes
        .iter()
        .filter_map(|node| node.parameters["label"].as_str())
        .collect();
    for label in ["room_eq_logical_inputs", "room_eq_physical_outputs"] {
        assert_eq!(labels.iter().filter(|value| **value == label).count(), 1);
    }
    for index in 0..10 {
        for prefix in ["room_eq_input_", "room_eq_output_sum_"] {
            let expected = format!("{prefix}{index}");
            assert_eq!(labels.iter().filter(|value| **value == expected).count(), 1);
        }
    }
    // Nine mains each have a high-pass and bass route; LFE has one bass route.
    assert_eq!(
        labels
            .iter()
            .filter(|label| label.starts_with("room_eq_route_"))
            .count(),
        19
    );
    assert!(config.nodes.iter().all(|node| node.input_channels == 10));
}

#[test]
fn gen514_per_route_graph_preserves_transfers_and_destination_sums() {
    let output = load_gen514_fixture();
    let routing = output
        .metadata
        .as_ref()
        .unwrap()
        .bass_management
        .as_ref()
        .unwrap()
        .routing_graph
        .as_ref()
        .unwrap();
    let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let mut visited = std::collections::HashSet::new();
    let mut sub_sources = std::collections::HashSet::new();
    for node in &config.nodes {
        if !node.parameters["label"]
            .as_str()
            .is_some_and(|label| label.starts_with("room_eq_route_"))
        {
            continue;
        }
        let matrix = node.parameters["matrix"].as_array().unwrap();
        assert_eq!(matrix.len(), 100);
        let nonzero: Vec<_> = matrix
            .iter()
            .enumerate()
            .filter(|(_, value)| value.as_f64().unwrap() != 0.0)
            .collect();
        assert_eq!(
            nonzero.len(),
            1,
            "route must select exactly one source/destination"
        );
        let (cell, value) = nonzero[0];
        assert_eq!(value.as_f64(), Some(1.0));
        let (source, destination) = (cell % 10, cell / 10);
        assert!(visited.insert((source, destination)), "duplicate route");
        let expected = routing
            .routes
            .iter()
            .find(|route| route.source_index == source && route.destination_index == destination)
            .expect("route must exist in fixture");
        if destination == 3 {
            sub_sources.insert(source);
        }
        let mut cursor = node.id;
        let mut gain = None;
        let mut crossover = None;
        let mut delay = None;
        let mut reached_sum = false;
        for _ in 0..config.nodes.len() {
            let edges: Vec<_> = config
                .edges
                .iter()
                .filter(|edge| edge.from_node == cursor)
                .collect();
            assert_eq!(
                edges.len(),
                1,
                "route must remain isolated until destination sum"
            );
            let next = config
                .nodes
                .iter()
                .find(|node| node.id == edges[0].to_node)
                .unwrap();
            let label = next.parameters["label"].as_str().unwrap_or("");
            if label.starts_with("room_eq_output_sum_") {
                assert_eq!(label, format!("room_eq_output_sum_{destination}"));
                reached_sum = true;
                break;
            }
            match next.plugin_type.as_str() {
                "matrix" => {
                    assert!(gain.is_none());
                    gain = next.parameters["matrix"][destination * 10 + destination].as_f64();
                }
                "crossover" => {
                    assert!(crossover.is_none());
                    crossover = Some(&next.parameters);
                }
                "delay" => {
                    assert!(delay.is_none());
                    delay = next.parameters["delay_ms"].as_f64();
                }
                other => panic!("unexpected route processor {other}"),
            }
            cursor = next.id;
        }
        assert!(reached_sum, "route must terminate at its destination sum");
        let expected_gain = 10.0_f64.powf(expected.gain_db / 20.0)
            * if expected.polarity_inverted {
                -1.0
            } else {
                1.0
            };
        assert!((gain.unwrap_or(1.0) - expected_gain).abs() < 1e-6);
        assert!((delay.unwrap_or(0.0) - expected.delay_ms).abs() < 1e-6);
        let crossover = crossover.expect("each fixture route has a crossover");
        let (frequency, branch) = if let Some(hz) = expected.high_pass_hz {
            (hz, "high")
        } else {
            (expected.low_pass_hz.unwrap(), "low")
        };
        assert!((crossover["frequency"].as_f64().unwrap() - frequency).abs() < 1e-6);
        assert_eq!(crossover["output"].as_str(), Some(branch));
    }
    assert_eq!(visited.len(), routing.routes.len());
    assert_eq!(sub_sources, (0..10).collect());
}

#[test]
fn gen514_per_route_graph_nodes_instantiate_via_factory() {
    let output = load_gen514_fixture();
    let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    for node in &config.nodes {
        let label = node
            .parameters
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("<unlabeled>");
        sotf_plugins::create_plugin(
            &node.plugin_type,
            &node.parameters,
            node.input_channels,
            48_000,
        )
        .unwrap_or_else(|err| {
            panic!(
                "gen514 per-route node '{label}' (type={}) failed to instantiate: {err}",
                node.plugin_type
            )
        });
    }
}

/// Serialise the per-route graph as a stable text snapshot. Format:
///
///     [nodes]
///     <id> <plugin_type> <label> ch=<input_channels>
///     ...
///     [edges]
///     <from_id> -> <to_id>
///     ...
///
/// Edges are sorted by `(from_id, to_id)` to make textual diffs
/// stable across non-semantic reorderings. The snapshot intentionally
/// omits per-channel parameter arrays (gains, filters, frequencies) so
/// the fixture stays readable and the test doesn't churn on every
/// numeric refinement to the optimizer — only on topology changes.
fn serialise_topology(config: &sotf_audio::engine::PluginGraphConfig) -> String {
    let mut out = String::new();
    out.push_str("[nodes]\n");
    let mut nodes: Vec<_> = config.nodes.iter().collect();
    nodes.sort_by_key(|n| n.id);
    for node in nodes {
        let label = node
            .parameters
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("<unlabeled>");
        out.push_str(&format!(
            "{} {} {} ch={}\n",
            node.id, node.plugin_type, label, node.input_channels
        ));
    }

    out.push_str("\n[edges]\n");
    let mut edges: Vec<(usize, usize)> = config
        .edges
        .iter()
        .map(|e| (e.from_node, e.to_node))
        .collect();
    edges.sort();
    for (from, to) in edges {
        out.push_str(&format!("{from} -> {to}\n"));
    }
    out
}

/// Strict golden topology snapshot for gen514. Catches silent regressions
/// where the builder shifts edges, renames labels, or adds/removes nodes
/// without those changes registering in the looser role-presence tests.
///
/// To update after a deliberate topology change:
///   `INSTA_UPDATE=overwrite cargo test gen514_per_route_graph_topology_matches_golden_snapshot`
/// — or just paste the failing output (in the panic message below) into
/// `tests/fixtures/roomeq_gen514_topology.txt`.
#[test]
fn gen514_per_route_graph_topology_matches_golden_snapshot() {
    let output = load_gen514_fixture();
    let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();
    let actual = serialise_topology(&config);

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("roomeq_gen514_topology.txt");

    // Allow regenerating the snapshot via env var, mirroring `insta`'s
    // workflow without taking the dependency.
    if std::env::var("INSTA_UPDATE").as_deref() == Ok("overwrite") {
        std::fs::write(&golden_path, &actual)
            .unwrap_or_else(|e| panic!("write golden {}: {e}", golden_path.display()));
        eprintln!("Updated golden snapshot at {}", golden_path.display());
        return;
    }

    let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
        panic!(
            "missing golden snapshot {} ({e}). To create it, run with \
             INSTA_UPDATE=overwrite",
            golden_path.display()
        )
    });

    if actual != expected {
        // Show a readable diff in the failure message: caller can copy the
        // "actual" block into the fixture if the change is intentional.
        panic!(
            "gen514 per-route graph topology drifted from the golden snapshot.\n\
             --- expected ({path}) ---\n\
             {expected}\n\
             --- actual ---\n\
             {actual}\n\
             --- end ---\n\
             If the change is intentional, run:\n  \
             INSTA_UPDATE=overwrite cargo test gen514_per_route_graph_topology_matches_golden_snapshot",
            path = golden_path.display()
        );
    }
}
