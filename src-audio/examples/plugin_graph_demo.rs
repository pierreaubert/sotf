// ============================================================================
// Plugin Graph Demo - Demonstrates graph-based plugin processing
// ============================================================================
//
// This example shows how to use the ParallelPluginGraph to create complex
// audio processing topologies with parallel processing and stream merging.

use sotf_audio::plugins::{
    GainPlugin, InPlacePluginAdapter, ParallelPluginGraph, GraphEdge,
};

fn main() -> Result<(), String> {
    println!("=== Plugin Graph Demo ===\n");

    // Example 1: Linear chain
    println!("Example 1: Linear Chain");
    println!("------------------------");
    linear_chain_example()?;

    // Example 2: Parallel diamond pattern
    println!("\nExample 2: Parallel Diamond");
    println!("---------------------------");
    parallel_diamond_example()?;

    // Example 3: Stream merging
    println!("\nExample 3: Stream Merging");
    println!("-------------------------");
    stream_merge_example()?;

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

/// Example 1: Simple linear chain of plugins
///
/// Graph topology:
///   Input -> Gain1 (-6dB) -> Gain2 (-6dB) -> Output
///
/// This demonstrates basic sequential processing.
fn linear_chain_example() -> Result<(), String> {
    let mut graph = ParallelPluginGraph::new(48000);

    // Create nodes
    let gain1 = GainPlugin::new(2, -6.0); // -6dB attenuation
    let gain2 = GainPlugin::new(2, -6.0); // -6dB attenuation

    let node1 = graph.add_node(
        "gain1".to_string(),
        Box::new(InPlacePluginAdapter::new(gain1))
    )?;

    let node2 = graph.add_node(
        "gain2".to_string(),
        Box::new(InPlacePluginAdapter::new(gain2))
    )?;

    // Connect nodes
    graph.add_edge(GraphEdge::new(node1, node2))?;

    // Build and analyze
    graph.build()?;
    println!("Graph has {} stages", graph.num_stages());
    for i in 0..graph.num_stages() {
        let stage = graph.stage_info(i).unwrap();
        println!("  Stage {}: {} nodes: {:?}", i, stage.len(), stage);
    }

    // Process audio
    let input = vec![1.0; 96]; // 48 frames × 2 channels
    let mut output = vec![0.0; 96];

    graph.process(&input, &mut output)?;

    println!("Input level:  {:?}", &input[0..4]);
    println!("Output level: {:?}", &output[0..4]);
    println!("Expected: ~0.25 (two -6dB stages = -12dB total ≈ 0.25x)");

    Ok(())
}

/// Example 2: Diamond pattern with parallel processing
///
/// Graph topology:
///              +-> Gain2 (0dB) -+
///   Input -> Gain1 (-3dB)       +-> Gain4 (0dB) -> Output
///              +-> Gain3 (0dB) -+
///
/// This demonstrates:
/// - Splitting a signal into multiple paths
/// - Parallel processing (Gain2 and Gain3 run concurrently)
/// - Stream merging at Gain4
fn parallel_diamond_example() -> Result<(), String> {
    let mut graph = ParallelPluginGraph::new(48000);
    graph.set_parallel_enabled(true); // Enable parallel processing

    // Create nodes
    let node1 = graph.add_node(
        "splitter".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -3.0)))
    )?;

    let node2 = graph.add_node(
        "branch_a".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0)))
    )?;

    let node3 = graph.add_node(
        "branch_b".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0)))
    )?;

    let node4 = graph.add_node(
        "merger".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0)))
    )?;

    // Connect edges
    graph.add_edge(GraphEdge::new(node1, node2))?; // Split to branch A
    graph.add_edge(GraphEdge::new(node1, node3))?; // Split to branch B
    graph.add_edge(GraphEdge::new(node2, node4))?; // Merge from branch A
    graph.add_edge(GraphEdge::new(node3, node4))?; // Merge from branch B

    // Build and analyze
    graph.build()?;
    println!("Graph has {} stages", graph.num_stages());
    for i in 0..graph.num_stages() {
        let stage = graph.stage_info(i).unwrap();
        println!("  Stage {}: {} nodes: {:?}", i, stage.len(), stage);
        if stage.len() > 1 {
            println!("    ^ This stage runs in parallel!");
        }
    }

    // Process audio
    let input = vec![1.0; 96];
    let mut output = vec![0.0; 96];

    graph.process(&input, &mut output)?;

    println!("Input level:  {:?}", &input[0..4]);
    println!("Output level: {:?}", &output[0..4]);
    println!("Expected: ~1.414 (-3dB split = 0.707x, then 2 paths merge = 0.707 × 2 ≈ 1.414x)");

    Ok(())
}

/// Example 3: Multiple stream merging
///
/// Graph topology:
///                +-> Path1 (-6dB) -+
///   Input -> Split (0dB)           +-> Merge (0dB) -> Output
///                +-> Path2 (-6dB) -+
///
/// This demonstrates synchronization at merge points:
/// - Both paths must complete before the merge node can process
/// - The merge node waits for all inputs (stream synchronization)
/// - Inputs are summed at the merge point
fn stream_merge_example() -> Result<(), String> {
    let mut graph = ParallelPluginGraph::new(48000);

    let split = graph.add_node(
        "split".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0)))
    )?;

    let path1 = graph.add_node(
        "path1".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -6.0)))
    )?;

    let path2 = graph.add_node(
        "path2".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -6.0)))
    )?;

    let merge = graph.add_node(
        "merge".to_string(),
        Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0)))
    )?;

    // Split
    graph.add_edge(GraphEdge::new(split, path1))?;
    graph.add_edge(GraphEdge::new(split, path2))?;

    // Merge (this is the synchronization point)
    graph.add_edge(GraphEdge::new(path1, merge))?;
    graph.add_edge(GraphEdge::new(path2, merge))?;

    // Build
    graph.build()?;
    println!("Graph has {} stages", graph.num_stages());
    for i in 0..graph.num_stages() {
        let stage = graph.stage_info(i).unwrap();
        println!("  Stage {}: {} nodes: {:?}", i, stage.len(), stage);
    }

    println!("\nStream Synchronization:");
    println!("  - The 'merge' node waits for BOTH path1 and path2 to complete");
    println!("  - This ensures sample-accurate synchronization");
    println!("  - All inputs are summed at the merge point");

    // Process
    let input = vec![1.0; 96];
    let mut output = vec![0.0; 96];

    graph.process(&input, &mut output)?;

    println!("\nInput level:  {:?}", &input[0..4]);
    println!("Output level: {:?}", &output[0..4]);
    println!("Expected: ~1.0 (both paths at -6dB = 0.5x, merge = 0.5 + 0.5 = 1.0x)");

    Ok(())
}
