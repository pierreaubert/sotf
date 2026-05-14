//! Integration tests for the RoomEQ factored graph builder against the
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
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("parse fixture {}: {e}", path.display()))
}

#[test]
fn gen514_factored_graph_has_one_node_per_role_for_10_channels() {
    let output = load_gen514_fixture();
    let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();

    // 10 input channels: L, R, C, LFE, SL, SR, TFL, TFR, TBL, TBR.
    let labels: Vec<&str> = config
        .nodes
        .iter()
        .filter_map(|n| n.parameters.get("label").and_then(|l| l.as_str()))
        .collect();

    for required in [
        "room_eq_gain_pre",
        "room_eq_eq_pre",
        "room_eq_xover_hp",
        "room_eq_delay_hp",
        "room_eq_xover_lp",
        "room_eq_delay_lp",
        "room_eq_matrix_to_sub_bus",
        "room_eq_eq_post",
        "room_eq_gain_post",
    ] {
        let count = labels.iter().filter(|l| **l == required).count();
        assert_eq!(
            count, 1,
            "factored graph must emit exactly one '{required}', got {count} in {labels:?}"
        );
    }

    // Every node carries the full 10-channel width — no per-channel
    // sub-graphs at narrower widths.
    for node in &config.nodes {
        assert_eq!(
            node.input_channels, 10,
            "node '{}' should be at 10-channel width, got {}",
            node.parameters
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or("<unlabeled>"),
            node.input_channels
        );
    }
}

#[test]
fn gen514_factored_graph_matrix_sums_redirected_bass_onto_lfe_row() {
    let output = load_gen514_fixture();
    let config = build_room_eq_plugin_graph_config(&output, 48_000.0).unwrap();

    let matrix_node = config
        .nodes
        .iter()
        .find(|n| n.parameters.get("label").and_then(|l| l.as_str()) == Some("room_eq_matrix_to_sub_bus"))
        .expect("factored sub-bus matrix node");
    let matrix: Vec<f32> = matrix_node.parameters["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    let n = 10usize;
    assert_eq!(matrix.len(), n * n);

    // gen514 input_channels order: [L, R, C, LFE, SL, SR, TFL, TFR, TBL, TBR].
    // LFE is at index 3. Every main source has a redirected_bass_lowpass_to_sub
    // route into LFE; LFE has its own lfe_lowpass_to_sub. So row 3 should have
    // 10 non-zero entries.
    let lfe_row: Vec<f32> = (0..n).map(|src| matrix[3 * n + src]).collect();
    let nonzero = lfe_row.iter().filter(|c| **c > 0.0).count();
    assert_eq!(nonzero, 10, "LFE row should sum from every source: {lfe_row:?}");

    // Other rows should be all zero (no other sub destinations in gen514).
    for dst in (0..n).filter(|&d| d != 3) {
        for src in 0..n {
            assert_eq!(
                matrix[dst * n + src], 0.0,
                "matrix[{dst}][{src}] must be 0 for non-LFE destination row"
            );
        }
    }
}

#[test]
fn gen514_factored_graph_nodes_instantiate_via_factory() {
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
                "gen514 factored node '{label}' (type={}) failed to instantiate: {err}",
                node.plugin_type
            )
        });
    }
}

/// Serialise the factored graph as a stable text snapshot. Format:
///
///     [nodes]
///     <id> <plugin_type> <label> ch=<input_channels>
///     ...
///     [edges]
///     <from_label> -> <to_label>
///     ...
///
/// Edges are sorted by `(from_label, to_label)` to make textual diffs
/// stable across non-semantic reorderings. The snapshot intentionally
/// omits per-channel parameter arrays (gains, filters, frequencies) so
/// the fixture stays readable and the test doesn't churn on every
/// numeric refinement to the optimizer — only on topology changes.
fn serialise_topology(config: &sotf_audio::engine::PluginGraphConfig) -> String {
    let label_for = |id: usize| -> String {
        config
            .nodes
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.parameters.get("label"))
            .and_then(|l| l.as_str())
            .unwrap_or("<unlabeled>")
            .to_string()
    };

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
    let mut edges: Vec<(String, String)> = config
        .edges
        .iter()
        .map(|e| (label_for(e.from_node), label_for(e.to_node)))
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
///   `INSTA_UPDATE=overwrite cargo test gen514_factored_graph_topology_matches_golden_snapshot`
/// — or just paste the failing output (in the panic message below) into
/// `tests/fixtures/roomeq_gen514_topology.txt`.
#[test]
fn gen514_factored_graph_topology_matches_golden_snapshot() {
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
            "gen514 factored graph topology drifted from the golden snapshot.\n\
             --- expected ({path}) ---\n\
             {expected}\n\
             --- actual ---\n\
             {actual}\n\
             --- end ---\n\
             If the change is intentional, run:\n  \
             INSTA_UPDATE=overwrite cargo test gen514_factored_graph_topology_matches_golden_snapshot",
            path = golden_path.display()
        );
    }
}
