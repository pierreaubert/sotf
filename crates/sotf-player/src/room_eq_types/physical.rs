//! Native lowering of AutoEQ's resolved physical-routing contract.
//!
//! Source processing is shared before fan-out. Distinct route transfers are
//! never collapsed by source index; physical-output processing follows fan-in.

use autoeq::roomeq_model::PhysicalRoutingGraph;
use sotf_audio::engine::{PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig};

use super::identity::identity_matrix_parameters;
use super::misc::single_channel_matrix_parameters;

/// Validate the RoomEQ runtime protection contract without accepting plugin defaults.
fn validate_sub_limiter(plugin: &autoeq::roomeq::PluginConfigWrapper) -> anyhow::Result<()> {
    let ceiling = plugin.parameters["threshold_db"]
        .as_f64()
        .unwrap_or(f64::NAN);
    let expected = serde_json::json!({
        "threshold_db": ceiling, "release_ms": 100.0, "lookahead_ms": 5.0,
        "soft": false, "true_peak": false, "isp_mode": false,
        "dual_release": false, "mix": 1.0, "feed_forward": true,
        "link_amount": 1.0, "label": "room_eq_sub_output_limiter", "room_eq_stage": "post_route"
    });
    if !ceiling.is_finite() || !(-20.0..=-1.0).contains(&ceiling) || plugin.parameters != expected {
        anyhow::bail!("unsupported native RoomEQ sub-output limiter contract");
    }
    Ok(())
}

fn native_plugins(
    plugins: &[autoeq::roomeq::PluginConfigWrapper],
    width: usize,
) -> anyhow::Result<Vec<autoeq::roomeq::PluginConfigWrapper>> {
    let mut result = Vec::new();
    for plugin in plugins {
        // DawHost compensates reported limiter latency at the output merge.
        // The serialized delay exists for offline small-signal replay; emitting
        // it here as well would delay the mains twice.
        if plugin.plugin_type == "delay"
            && plugin.parameters["label"].as_str() == Some("room_eq_limiter_latency")
        {
            continue;
        }
        let mut plugin = plugin.clone();
        if plugin.plugin_type == "gain" {
            let gain = plugin.parameters["gain_db"]
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("physical gain requires gain_db"))?;
            let invert = match plugin.parameters.get("invert") {
                None => false,
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| anyhow::anyhow!("invalid physical gain polarity"))?,
            };
            let scalar = 10.0_f64.powf(gain / 20.0) * if invert { -1.0 } else { 1.0 };
            if !gain.is_finite() || !(scalar as f32).is_finite() {
                anyhow::bail!("physical gain is not representable by native matrix");
            }
            // Native gain has no polarity parameter. A static diagonal matrix
            // represents the full signed gain without silently losing inversion.
            let mut parameters = identity_matrix_parameters(width, "room_eq_signed_gain");
            for index in 0..width {
                parameters["matrix"][index * width + index] = serde_json::json!(scalar as f32);
            }
            plugin.plugin_type = "matrix".into();
            plugin.parameters = parameters;
        } else if plugin.plugin_type == "crossover" {
            let kind = plugin.parameters["type"].as_str().unwrap_or("");
            if !matches!(kind, "LR24" | "LR4") {
                anyhow::bail!(
                    "unsupported native physical RoomEQ crossover '{kind}'; no family substitution is permitted"
                );
            }
        } else if plugin.plugin_type == "delay" {
            // RoomEQ delays are alignment delays, not the native echo preset.
            for (field, expected) in [("mix", 1.0), ("feedback", 0.0), ("lfo_depth_ms", 0.0)] {
                if let Some(value) = plugin.parameters.get(field)
                    && value.as_f64() != Some(expected)
                {
                    anyhow::bail!("physical alignment delay has incompatible {field}");
                }
                plugin.parameters[field] = serde_json::json!(expected);
            }
            plugin.parameters["lfo_rate_hz"] = serde_json::json!(0.0);
            plugin.parameters["pitch_preserving"] = serde_json::json!(false);
        }
        result.push(plugin);
    }
    Ok(result)
}

pub(super) fn build_physical_room_eq_graph(
    physical: &PhysicalRoutingGraph,
    global_plugins: &[autoeq::roomeq::PluginConfigWrapper],
) -> anyhow::Result<PluginGraphConfig> {
    let mut physical = physical.clone();
    physical.canonicalize().map_err(anyhow::Error::msg)?;
    let has_limiter = physical
        .outputs
        .iter()
        .any(|output| output.plugins.iter().any(|p| p.plugin_type == "limiter"));
    for input in &physical.inputs {
        if input.plugins.iter().any(|p| p.plugin_type == "limiter") {
            anyhow::bail!("RoomEQ limiter must follow physical output summation");
        }
    }
    for output in &physical.outputs {
        for (index, plugin) in output.plugins.iter().enumerate() {
            if plugin.plugin_type == "limiter" && index + 1 != output.plugins.len() {
                anyhow::bail!("RoomEQ limiter must be the terminal physical output processor");
            }
        }
    }
    for port in physical.inputs.iter().chain(&physical.outputs) {
        for plugin in &port.plugins {
            if plugin.parameters["label"].as_str() == Some("room_eq_limiter_latency")
                && (!has_limiter
                    || plugin.plugin_type != "delay"
                    || plugin.parameters["room_eq_stage"].as_str() != Some("post_route"))
            {
                anyhow::bail!("invalid RoomEQ limiter latency compensation marker");
            }
            if !matches!(
                plugin.plugin_type.as_str(),
                "gain" | "eq" | "delay" | "crossover" | "convolution" | "limiter"
            ) {
                anyhow::bail!(
                    "unsupported per-port physical RoomEQ plugin '{}' on '{}'",
                    plugin.plugin_type,
                    port.name
                );
            }
            if plugin.plugin_type == "limiter" {
                validate_sub_limiter(plugin)?;
            }
        }
    }
    let width = physical.inputs.len().max(physical.outputs.len());
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut add = |plugin_type: String, parameters: serde_json::Value| {
        let id = nodes.len();
        let input_channels = parameters["input_channels"]
            .as_u64()
            .map_or(width, |v| v as usize);
        nodes.push(PluginGraphNodeConfig {
            id,
            plugin_type,
            parameters,
            input_channels,
            bypassed: false,
        });
        id
    };
    // The host receives unpadded logical-input PCM. Expand each frame once,
    // before fan-out; intermediate matrices operate at the working width.
    let input_count = physical.inputs.len();
    let output_count = physical.outputs.len();
    let mut ingress = vec![0.0_f32; width * input_count];
    for index in 0..input_count {
        ingress[index * input_count + index] = 1.0;
    }
    let ingress = add(
        "matrix".into(),
        serde_json::json!({
            "label": "room_eq_logical_inputs", "input_channels": input_count,
            "output_channels": width, "matrix": ingress,
        }),
    );
    let mut global_tail = Some(ingress);
    for plugin in global_plugins
        .iter()
        .filter(|p| !super::types::is_route_replaced_global_plugin(p))
    {
        if super::infer::infer_plugin_output_channels(plugin, width) != width {
            anyhow::bail!("physical RoomEQ routing cannot follow a width-changing global plugin");
        }
        let node = add(plugin.plugin_type.clone(), plugin.parameters.clone());
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: node,
            });
        }
        global_tail = Some(node);
    }

    let mut input_tails = vec![None; physical.inputs.len()];
    for (index, input) in physical.inputs.iter().enumerate() {
        // A root without an outgoing route would become a graph sink and leak
        // an intentionally unused input into the host's final output mix.
        if !physical
            .routes
            .iter()
            .any(|route| route.input_index == index)
        {
            continue;
        }
        let mut tail = add(
            "matrix".into(),
            single_channel_matrix_parameters(
                width,
                index,
                index,
                1.0,
                format!("room_eq_input_{index}"),
                None,
            ),
        );
        if let Some(prev) = global_tail {
            edges.push(PluginGraphEdgeConfig {
                from_node: prev,
                to_node: tail,
            });
        }
        for plugin in native_plugins(&input.plugins, width)? {
            let node = add(plugin.plugin_type.clone(), plugin.parameters.clone());
            edges.push(PluginGraphEdgeConfig {
                from_node: tail,
                to_node: node,
            });
            tail = node;
        }
        input_tails[index] = Some(tail);
    }

    let mut output_sums = vec![None; physical.outputs.len()];
    for (index, output_sum) in output_sums.iter_mut().enumerate() {
        if physical
            .routes
            .iter()
            .any(|route| route.output_index == index)
        {
            *output_sum = Some(add(
                "matrix".into(),
                single_channel_matrix_parameters(
                    width,
                    index,
                    index,
                    1.0,
                    format!("room_eq_output_sum_{index}"),
                    None,
                ),
            ));
        }
    }
    for (index, route) in physical.routes.iter().enumerate() {
        let mut tail = add(
            "matrix".into(),
            single_channel_matrix_parameters(
                width,
                route.input_index,
                route.output_index,
                1.0,
                format!("room_eq_route_{index}"),
                Some(serde_json::json!({
                    "input": physical.inputs[route.input_index].name,
                    "output": physical.outputs[route.output_index].name,
                })),
            ),
        );
        edges.push(PluginGraphEdgeConfig {
            from_node: input_tails[route.input_index].expect("routed input has processing"),
            to_node: tail,
        });
        for plugin in native_plugins(
            &autoeq::roomeq_engine::physical_routing::physical_route_plugins(route),
            width,
        )? {
            let node = add(plugin.plugin_type, plugin.parameters);
            edges.push(PluginGraphEdgeConfig {
                from_node: tail,
                to_node: node,
            });
            tail = node;
        }
        edges.push(PluginGraphEdgeConfig {
            from_node: tail,
            to_node: output_sums[route.output_index].expect("routed output has a sum"),
        });
    }

    let mut egress = vec![0.0_f32; output_count * width];
    for index in 0..output_count {
        egress[index * width + index] = 1.0;
    }
    let merge = add(
        "matrix".into(),
        serde_json::json!({
            "label": "room_eq_physical_outputs", "input_channels": width,
            "output_channels": output_count, "matrix": egress,
        }),
    );
    for (output, sum) in physical.outputs.iter().zip(output_sums) {
        let Some(mut tail) = sum else { continue };
        for plugin in native_plugins(&output.plugins, width)? {
            let node = add(plugin.plugin_type.clone(), plugin.parameters.clone());
            edges.push(PluginGraphEdgeConfig {
                from_node: tail,
                to_node: node,
            });
            tail = node;
        }
        edges.push(PluginGraphEdgeConfig {
            from_node: tail,
            to_node: merge,
        });
    }
    Ok(PluginGraphConfig { nodes, edges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use autoeq::roomeq_model::{PhysicalRoute, PhysicalRoutingPort, PluginConfigWrapper};
    use serde_json::json;

    #[test]
    fn runtime_sub_limiter_catches_summed_peaks_without_double_delaying_mains() {
        use sotf_plugins::{DawHost, GraphEdge};
        for sample_rate in [44_100, 48_000, 96_000] {
            for block in [1, 7, 127, 512] {
                let latency = (5.0_f32 * 0.001 * sample_rate as f32) as usize;
                let port = |name: &str, plugins| PhysicalRoutingPort {
                    name: name.into(),
                    plugins,
                };
                let route = |input_index, output_index| PhysicalRoute {
                    input_index,
                    output_index,
                    gain_db: 0.0,
                    polarity_inverted: false,
                    delay_ms: 0.0,
                    crossover: None,
                };
                let physical = PhysicalRoutingGraph {
                    inputs: vec![port("L", vec![]), port("R", vec![])],
                    outputs: vec![
                        port(
                            "main",
                            vec![PluginConfigWrapper {
                                plugin_type: "delay".into(),
                                parameters: json!({
                                    "delay_ms": latency as f64 * 1000.0 / sample_rate as f64,
                                    "label": "room_eq_limiter_latency", "room_eq_stage": "post_route"
                                }),
                            }],
                        ),
                        port(
                            "sub",
                            vec![PluginConfigWrapper {
                                plugin_type: "limiter".into(),
                                parameters: json!({
                                    "threshold_db": -1.0, "release_ms": 100.0, "lookahead_ms": 5.0,
                                    "soft": false, "true_peak": false, "isp_mode": false,
                                    "dual_release": false, "mix": 1.0, "feed_forward": true,
                                    "link_amount": 1.0, "label": "room_eq_sub_output_limiter", "room_eq_stage": "post_route"
                                }),
                            }],
                        ),
                    ],
                    routes: vec![route(0, 0), route(0, 1), route(1, 1)],
                };
                let graph = build_physical_room_eq_graph(&physical, &[]).unwrap();
                let mut host = DawHost::new(2, sample_rate);
                let mut ids = std::collections::HashMap::new();
                for node in &graph.nodes {
                    let plugin = sotf_plugins::create_plugin(
                        &node.plugin_type,
                        &node.parameters,
                        node.input_channels,
                        sample_rate,
                    )
                    .unwrap();
                    ids.insert(
                        node.id,
                        host.add_node(format!("node_{}", node.id), plugin).unwrap(),
                    );
                }
                for edge in &graph.edges {
                    host.add_edge(GraphEdge::new(ids[&edge.from_node], ids[&edge.to_node]))
                        .unwrap();
                }
                host.build().unwrap();
                let mut observed = Vec::new();
                for start in (0..4096).step_by(block) {
                    let frames = block.min(4096 - start);
                    let input: Vec<_> = (start..start + frames)
                        .flat_map(|i| {
                            if i == 0 {
                                [0.1, 0.0]
                            } else if (1024..2048).contains(&i) {
                                [0.75, 0.75]
                            } else {
                                [0.0, 0.0]
                            }
                        })
                        .collect();
                    let mut output = vec![f32::NAN; frames * 2];
                    assert_eq!(host.process(&input, &mut output).unwrap(), frames);
                    observed.extend(output);
                }
                assert!((observed[2 * latency] - 0.1).abs() < 1e-5);
                assert!((observed[2 * latency + 1] - 0.1).abs() < 1e-5);
                assert!((observed[2 * (1500 + latency)] - 0.75).abs() < 1e-5);
                let ceiling = 10.0_f32.powf(-1.0 / 20.0);
                assert!(
                    observed
                        .chunks_exact(2)
                        .all(|f| f[1].is_finite() && f[1].abs() <= ceiling + 1e-6)
                );
                assert!(observed[2 * (1500 + latency) + 1] > 0.5);
            }
        }
    }

    fn fixture(sample_rate: u32) -> PhysicalRoutingGraph {
        let gain = |db| PluginConfigWrapper {
            plugin_type: "gain".into(),
            parameters: json!({"gain_db": db}),
        };
        PhysicalRoutingGraph {
            inputs: vec![
                PhysicalRoutingPort {
                    name: "L".into(),
                    plugins: vec![gain(-2.0)],
                },
                PhysicalRoutingPort {
                    name: "R".into(),
                    plugins: vec![],
                },
                PhysicalRoutingPort {
                    name: "unused_input".into(),
                    plugins: vec![],
                },
            ],
            outputs: vec![
                PhysicalRoutingPort {
                    name: "subs_1".into(),
                    plugins: vec![
                        PluginConfigWrapper {
                            plugin_type: "eq".into(),
                            parameters: json!({"filters": [
                                {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 3.0}
                            ]}),
                        },
                        gain(-6.0),
                    ],
                },
                PhysicalRoutingPort {
                    name: "subs_2".into(),
                    plugins: vec![gain(-3.0)],
                },
                PhysicalRoutingPort {
                    name: "silent".into(),
                    plugins: vec![],
                },
            ],
            routes: vec![
                PhysicalRoute {
                    input_index: 0,
                    output_index: 0,
                    gain_db: -3.0,
                    polarity_inverted: false,
                    delay_ms: 120_000.0 / sample_rate as f64,
                    crossover: None,
                },
                PhysicalRoute {
                    input_index: 1,
                    output_index: 0,
                    gain_db: -6.0,
                    polarity_inverted: true,
                    delay_ms: 36_000.0 / sample_rate as f64,
                    crossover: None,
                },
                PhysicalRoute {
                    input_index: 0,
                    output_index: 1,
                    gain_db: -9.0,
                    polarity_inverted: false,
                    delay_ms: 0.0,
                    crossover: None,
                },
            ],
        }
    }

    #[test]
    fn physical_native_graph_is_route_order_invariant() {
        let mut physical = fixture(48_000);
        let before = build_physical_room_eq_graph(&physical, &[]).unwrap();
        physical.routes.reverse();
        let after = build_physical_room_eq_graph(&physical, &[]).unwrap();
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(after).unwrap()
        );
    }

    #[test]
    fn physical_native_rectangular_io_preserves_frames_and_route_sums() {
        use sotf_plugins::{DawHost, GraphEdge};
        // Include expansion, contraction, fan-out, coherent fan-in, and a
        // fully silent graph. Inputs are never padded by the caller.
        for (inputs, outputs) in [(2, 3), (3, 5), (5, 2), (2, 2)] {
            for silent in [false, true] {
                let port = |index| PhysicalRoutingPort {
                    name: format!("channel_{index}"),
                    plugins: vec![],
                };
                let route = |input_index, output_index| PhysicalRoute {
                    input_index,
                    output_index,
                    gain_db: 0.0,
                    polarity_inverted: false,
                    delay_ms: 0.0,
                    crossover: None,
                };
                let physical = PhysicalRoutingGraph {
                    inputs: (0..inputs).map(port).collect(),
                    outputs: (0..outputs).map(port).collect(),
                    routes: if silent {
                        let mut inverted = route(0, 0);
                        inverted.polarity_inverted = true;
                        vec![route(0, 0), inverted]
                    } else {
                        vec![
                            route(0, 0),
                            route(0, outputs - 1),
                            route(inputs - 1, outputs - 1),
                        ]
                    },
                };
                let config = build_physical_room_eq_graph(&physical, &[]).unwrap();
                for sample_rate in [44_100, 48_000, 96_000] {
                    for block in [1, 7, 127, 512] {
                        let mut host = DawHost::new(inputs, sample_rate);
                        let mut ids = std::collections::HashMap::new();
                        for node in &config.nodes {
                            let plugin = sotf_plugins::create_plugin(
                                &node.plugin_type,
                                &node.parameters,
                                node.input_channels,
                                sample_rate,
                            )
                            .unwrap();
                            ids.insert(
                                node.id,
                                host.add_node(format!("node_{}", node.id), plugin).unwrap(),
                            );
                        }
                        for edge in &config.edges {
                            host.add_edge(GraphEdge::new(ids[&edge.from_node], ids[&edge.to_node]))
                                .unwrap();
                        }
                        host.build().unwrap();
                        assert_eq!(host.input_channels(), inputs);
                        assert_eq!(host.output_channels(), outputs);
                        for start in (0..1027).step_by(block) {
                            let frames = block.min(1027 - start);
                            let input: Vec<f32> = (0..frames * inputs)
                                .map(|sample| ((start * inputs + sample) % 31) as f32 / 64.0)
                                .collect();
                            let mut output = vec![f32::NAN; frames * outputs];
                            assert_eq!(host.process(&input, &mut output).unwrap(), frames);
                            for frame in 0..frames {
                                for channel in 0..outputs {
                                    let expected = if silent {
                                        0.0
                                    } else if channel == 0 {
                                        input[frame * inputs]
                                    } else if channel == outputs - 1 {
                                        input[frame * inputs] + input[frame * inputs + inputs - 1]
                                    } else {
                                        0.0
                                    };
                                    assert_eq!(
                                        output[frame * outputs + channel],
                                        expected,
                                        "{inputs}->{outputs}, silent={silent}, rate={sample_rate}, block={block}, frame={frame}, channel={channel}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn physical_native_rejects_unsupported_crossover_without_substitution() {
        let mut graph = fixture(48_000);
        graph.routes[0].crossover = Some(autoeq::roomeq_model::PhysicalRouteCrossover {
            crossover_type: "LR48".into(),
            frequency_hz: 80.0,
            pass: autoeq::roomeq_model::PhysicalRoutePass::Low,
        });
        let error = build_physical_room_eq_graph(&graph, &[]).unwrap_err();
        assert!(error.to_string().contains("LR48"));
        assert!(error.to_string().contains("no family substitution"));
    }

    #[test]
    fn physical_native_pcm_matches_independent_gain_delay_and_shared_eq_reference() {
        use sotf_plugins::{DawHost, GraphEdge};
        for order in [0, 24] {
            for sample_rate in [44_100, 48_000, 96_000] {
                for block_size in [32, 127, 512] {
                    let mut physical = fixture(sample_rate);
                    let crossover_scalar = match order {
                        24 => -0.5,
                        48 => 0.5,
                        _ => 1.0,
                    };
                    if order != 0 {
                        for route in &mut physical.routes {
                            route.crossover = Some(autoeq::roomeq_model::PhysicalRouteCrossover {
                                crossover_type: format!("LR{order}"),
                                frequency_hz: 1000.0,
                                pass: if route.output_index == 0 {
                                    autoeq::roomeq_model::PhysicalRoutePass::Low
                                } else {
                                    autoeq::roomeq_model::PhysicalRoutePass::High
                                },
                            });
                        }
                    }
                    let config = build_physical_room_eq_graph(&physical, &[]).unwrap();
                    let width = 3;
                    let mut host = DawHost::new(width, sample_rate);
                    let mut ids = std::collections::HashMap::new();
                    for node in &config.nodes {
                        let plugin = sotf_plugins::create_plugin(
                            &node.plugin_type,
                            &node.parameters,
                            node.input_channels,
                            sample_rate,
                        )
                        .unwrap();
                        ids.insert(
                            node.id,
                            host.add_node(format!("node_{}", node.id), plugin).unwrap(),
                        );
                    }
                    for edge in &config.edges {
                        host.add_edge(GraphEdge::new(ids[&edge.from_node], ids[&edge.to_node]))
                            .unwrap();
                    }
                    host.build().unwrap();
                    let omega = std::f64::consts::TAU * 1000.0 / sample_rate as f64;
                    let amp = |db: f64| 10.0_f64.powf(db / 20.0);
                    let mut worst = 0.0_f64;
                    // Warm up the EQ, then compare the streamed PCM to an analytic
                    // reference at the peaking filter's exact +3 dB center frequency.
                    for start in (0..12_001).step_by(block_size) {
                        let frames = block_size.min(12_001 - start);
                        let mut input = vec![0.0; frames * width];
                        for frame in 0..frames {
                            let n = (start + frame) as f64;
                            input[frame * width] = (0.1 * (omega * n).sin()) as f32;
                            input[frame * width + 1] = (0.2 * (omega * n).cos()) as f32;
                            input[frame * width + 2] = 0.3; // Must never leak from an unused input slot.
                        }
                        let mut output = vec![0.0; input.len()];
                        host.process(&input, &mut output).unwrap();
                        for frame in 0..frames {
                            let n = (start + frame) as f64;
                            if n < 4096.0 {
                                continue;
                            }
                            let sub1 = crossover_scalar
                                * amp(-3.0)
                                * (0.1 * amp(-2.0 - 3.0) * (omega * (n - 120.0)).sin()
                                    - 0.2 * amp(-6.0) * (omega * (n - 36.0)).cos());
                            let sub2 =
                                crossover_scalar * 0.1 * amp(-2.0 - 9.0 - 3.0) * (omega * n).sin();
                            for (channel, expected) in [sub1, sub2, 0.0].into_iter().enumerate() {
                                let actual = output[frame * width + channel] as f64;
                                assert!(actual.is_finite());
                                worst = worst.max((actual - expected).abs());
                            }
                        }
                    }
                    assert!(
                        worst < 3e-5,
                        "order={order}, sample_rate={sample_rate}, block={block_size}, max PCM error={worst}"
                    );
                }
            }
        }
    }
}
