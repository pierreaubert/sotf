# Audio Plugin System

A flexible plugin system for audio processing in the AutoEQ audio pipeline. Plugins can be chained together in a host, with each plugin processing N input channels and producing P output channels.

## Architecture

The plugin system consists of several key components:

### Core Traits

- **`Plugin`**: The main trait for audio processing plugins that can transform N input channels to P output channels
- **`ParametricInPlacePlugin`**: A simpler trait for plugins that process audio in-place (same number of input and output channels)
- **`ParametricInPlacePluginAdapter`**: Adapter to convert `ParametricInPlacePlugin` implementations to the `Plugin` trait

### Plugin Host

- **`PluginHost`**: Chains multiple plugins together, routing audio through them sequentially
- **`SharedPluginHost`**: Thread-safe wrapper for `PluginHost` using `Arc<Mutex<>>`

### Parameter System

- **`Parameter`**: Defines a plugin parameter with metadata (name, description, min/max values)
- **`ParameterValue`**: Type-safe parameter values (Float, Int, Bool)
- **`ParameterId`**: Unique identifier for parameters
- **`ParameterStore`**: Manages parameter values

### Example Plugins

- **`GainPlugin`**: Simple gain/volume control plugin (example implementation)
- **`UpmixerPlugin`**: Stereo to 5.0 surround upmixer using FFT-based Direct/Ambient decomposition

## Integration with Audio Pipeline

The plugin host integrates into the `AudioStreamingManager` pipeline:

```
Audio Decoder → Plugin Host → CamillaDSP → Audio Output
```

The plugin processing happens **before** the audio is sent to CamillaDSP, allowing you to apply custom DSP effects.

## Usage Examples

### Basic Plugin Usage

```rust
use sotf_audio::{GainPlugin, ParametricInPlacePluginAdapter, PluginHost};

// Create a plugin host for 2-channel audio at 44.1kHz
let mut host = PluginHost::new(2, 44100);

// Create a gain plugin and wrap it in an adapter
let gain_plugin = GainPlugin::new(2, -6.0); // -6dB attenuation
let adapter = ParametricInPlacePluginAdapter::new(gain_plugin);

// Add to the host
host.add_plugin(Box::new(adapter)).unwrap();

// Process audio
let input = vec![1.0, 1.0, 1.0, 1.0]; // 2 frames, 2 channels
let mut output = vec![0.0; 4];
host.process(&input, &mut output).unwrap();
```

### Chaining Multiple Plugins

```rust
let mut host = PluginHost::new(2, 44100);

// Add multiple plugins - they process in order
let gain1 = GainPlugin::new(2, -6.0);
host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(gain1))).unwrap();

let gain2 = GainPlugin::new(2, -3.0);
host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(gain2))).unwrap();

// Audio flows through gain1 first, then gain2
```

### Using with AudioStreamingManager

```rust
use sotf_audio::{AudioStreamingManager, GainPlugin, ParametricInPlacePluginAdapter};

let mut manager = AudioStreamingManager::new(camilla_binary_path);

// Load audio file
manager.load_file("audio.flac").await.unwrap();

// Enable plugin host
manager.enable_plugin_host().unwrap();

// Add plugins via the with_plugin_host closure
manager.with_plugin_host(|host| {
    let gain = GainPlugin::new(2, -3.0);
    host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(gain)))
}).unwrap().unwrap();

// Start playback - audio will flow through plugins
manager.start_playback(None, vec![], channel_map_mode, None, None).await.unwrap();
```

### Dynamic Parameter Changes

```rust
use sotf_audio::{GainPlugin, ParameterId, ParameterValue, ParametricInPlacePlugin};

let mut plugin = GainPlugin::new(2, 0.0);

// Change gain dynamically
let gain_id = ParameterId::from("gain_db");
plugin.set_parameter(gain_id, ParameterValue::Float(-12.0)).unwrap();

// Query parameters
let params = plugin.parameters();
for param in params {
    println!("{}: {:?}", param.name, param.default_value);
}
```

## Creating Custom Plugins

### Implementing ParametricInPlacePlugin (Simpler)

For plugins that don't change the channel count:

```rust
use sotf_audio::{ParametricInPlacePlugin, PluginInfo, ProcessContext, Parameter,
                 ParameterId, ParameterValue};

pub struct MyPlugin {
    channels: usize,
    sample_rate: u32,
}

impl ParametricInPlacePlugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "My Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Your Name".to_string(),
            description: "Does something cool".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("param1", "Parameter 1", 0.0, -10.0, 10.0)
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue)
        -> Result<(), String>
    {
        // Handle parameter changes
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Return parameter values
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> Result<(), String> {
        self.sample_rate = sample_rate;
        Ok(())
    }

    fn process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext)
        -> Result<(), String>
    {
        // Process audio in-place
        // buffer is interleaved: [L0, R0, L1, R1, ...]
        for sample in buffer.iter_mut() {
            *sample *= 0.5; // Example: attenuate by 50%
        }
        Ok(())
    }
}
```

### Implementing Plugin (More Flexible)

For plugins that change channel count (e.g., stereo to mono, upmixers):

```rust
use sotf_audio::{Plugin, PluginInfo, ProcessContext, Parameter,
                 ParameterId, ParameterValue};

pub struct StereoToMonoPlugin;

impl Plugin for StereoToMonoPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Stereo to Mono".to_string(),
            version: "1.0.0".to_string(),
            author: "Your Name".to_string(),
            description: "Converts stereo to mono by averaging".to_string(),
        }
    }

    fn input_channels(&self) -> usize { 2 }
    fn output_channels(&self) -> usize { 1 }

    fn parameters(&self) -> Vec<Parameter> { vec![] }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue)
        -> Result<(), String>
    {
        Err("No parameters".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(&mut self, input: &[f32], output: &mut [f32],
               context: &ProcessContext) -> Result<(), String>
    {
        // Input: [L0, R0, L1, R1, ...]
        // Output: [M0, M1, ...]
        for frame in 0..context.num_frames {
            let left = input[frame * 2];
            let right = input[frame * 2 + 1];
            output[frame] = (left + right) * 0.5;
        }
        Ok(())
    }
}
```

## Audio Format

All audio data is in **interleaved f32** format:
- Stereo: `[L0, R0, L1, R1, L2, R2, ...]`
- 5.1 surround: `[FL0, FR0, C0, LFE0, BL0, BR0, FL1, FR1, ...]`

Each sample is a 32-bit float, typically in the range -1.0 to +1.0.

## Performance Considerations

- **Buffer Allocation**: The plugin host allocates intermediate buffers when processing. Buffers are reused across calls if the frame count doesn't change.
- **In-Place Processing**: Use `ParametricInPlacePlugin` when possible - it's more efficient as it copies less data.
- **Thread Safety**: `SharedPluginHost` provides thread-safe access via `Arc<Mutex<>>`.
- **Zero-Copy**: The last plugin in the chain writes directly to the output buffer (no extra copy).

## Testing

Run tests:
```bash
cargo test --package sotf_audio
```

Integration tests are in `tests/test_plugins.rs`.
Unit tests are in each module (`plugins/gain.rs`, etc.).

## Thread Safety

The plugin host is integrated into the audio decoder thread in `AudioStreamingManager`. Plugins are called from this dedicated audio thread, so they don't need to be thread-safe internally. However, parameter changes from the UI thread are protected by mutexes in the `AudioStreamingManager` API.

# Plugin Graph System

## Overview

The Plugin Graph system extends the linear plugin chain architecture to support **directed acyclic graphs (DAGs)** of audio plugins. This enables:

- **Parallel Processing**: Multiple plugins can run concurrently on separate threads
- **Complex Routing**: Split and merge audio streams with arbitrary topologies
- **Stream Synchronization**: Automatic synchronization at merge points where streams join
- **Thread-based Concurrency**: Uses native threads (not async/tokio) for parallel processing
- **Backward Compatibility**: Drop-in replacement for `PluginHost` with the same API

## Dual API Support

`ParallelPluginGraph` supports two complementary APIs that can be used independently or together:

### 1. Chain Mode (PluginHost-Compatible)

Use the familiar `PluginHost` API for linear plugin chains:

```rust
let mut graph = ParallelPluginGraph::new_with_channels(2, 48000);

// Add plugins just like PluginHost
graph.add_plugin(Box::new(eq_plugin))?;
graph.add_plugin(Box::new(compressor_plugin))?;
graph.add_plugin(Box::new(limiter_plugin))?;

// Process (auto-builds the graph)
graph.process(&input, &mut output)?;
```

**Compatible API Methods:**
- `new_with_channels(channels, sample_rate)` - Create graph with initial channel count
- `add_plugin(plugin)` - Add plugin to end of chain
- `remove_plugin(index)` - Remove plugin by index
- `plugin_count()` - Get number of plugins
- `input_channels()` / `output_channels()` - Get channel counts
- `process(input, output)` - Process audio (auto-builds if needed)
- `reset()` - Reset all plugins

### 2. Graph Mode (Advanced Routing)

Use the graph API for complex routing topologies:

```rust
let mut graph = ParallelPluginGraph::new(48000);

// Create nodes
let node1 = graph.add_node("input", Box::new(plugin1))?;
let node2 = graph.add_node("branch_a", Box::new(plugin2))?;
let node3 = graph.add_node("branch_b", Box::new(plugin3))?;
let node4 = graph.add_node("merge", Box::new(plugin4))?;

// Connect edges
graph.add_edge(GraphEdge::new(node1, node2))?;
graph.add_edge(GraphEdge::new(node1, node3))?;
graph.add_edge(GraphEdge::new(node2, node4))?;
graph.add_edge(GraphEdge::new(node3, node4))?;

// Build and process
graph.build()?;
graph.process(&input, &mut output)?;
```

### 3. Mixed Mode

You can mix both APIs in a single graph:

```rust
let mut graph = ParallelPluginGraph::new_with_channels(2, 48000);

// Start with chain mode
graph.add_plugin(Box::new(preamp_plugin))?;

// Switch to graph mode for parallel routing
let last_node = *graph.chain_nodes.last().unwrap();
let branch_a = graph.add_node("effect_a", Box::new(reverb))?;
let branch_b = graph.add_node("effect_b", Box::new(delay))?;
let merge = graph.add_node("mixer", Box::new(mixer))?;

// Create parallel paths
graph.add_edge(GraphEdge::new(last_node, branch_a))?;
graph.add_edge(GraphEdge::new(last_node, branch_b))?;
graph.add_edge(GraphEdge::new(branch_a, merge))?;
graph.add_edge(GraphEdge::new(branch_b, merge))?;

graph.process(&input, &mut output)?;
```

## Architecture

### Core Components

#### 1. **ParallelPluginGraph**

The main graph structure that manages nodes, edges, and processing:

```rust
pub struct ParallelPluginGraph {
    nodes: HashMap<NodeId, GraphNode>,
    edges: Vec<GraphEdge>,
    stages: Vec<ProcessingStage>,
    // ...
}
```

**Key Features:**
- Automatic cycle detection
- Topological sorting for correct processing order
- Stage-based parallel execution
- Thread-safe plugin processing with `Arc<Mutex<Plugin>>`

#### 2. **GraphNode**

Represents a single plugin in the graph:

```rust
pub struct GraphNode {
    pub id: NodeId,
    pub plugin: Arc<Mutex<Box<dyn Plugin>>>,
    pub name: String,
    input_channels: usize,
    output_channels: usize,
}
```

Plugins are wrapped in `Arc<Mutex<>>` to enable:
- Thread-safe access during parallel processing
- Shared ownership across threads

#### 3. **GraphEdge**

Defines connections between nodes:

```rust
pub struct GraphEdge {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub channel_map: Option<Vec<usize>>,
}
```

Edges can include optional channel mapping for flexible routing.

#### 4. **Processing Stages**

The graph is divided into **stages** based on topological ordering:

```
Stage 0: [Input Node]
Stage 1: [Node A, Node B, Node C]  <- Can run in parallel
Stage 2: [Node D, Node E]          <- Can run in parallel
Stage 3: [Output Node]
```

Nodes within a stage have no dependencies on each other and can execute concurrently.

## Graph Processing Algorithm

### 1. **Build Phase**

When `graph.build()` is called:

1. **Cycle Detection**: Uses DFS to detect any cycles in the graph
2. **Identify I/O Nodes**: Finds nodes with no incoming edges (inputs) and no outgoing edges (outputs)
3. **Topological Sort**: Computes processing order using Kahn's algorithm
4. **Stage Computation**: Groups nodes that can run in parallel into stages

### 2. **Processing Phase**

When `graph.process()` is called:

```
For each stage in stages:
    If stage has multiple nodes AND parallel_enabled:
        Spawn threads for each node
        Use scoped threads to process nodes concurrently
        Wait for all threads to complete (implicit barrier)
    Else:
        Process nodes sequentially

    At merge points:
        Wait for all incoming streams
        Sum/mix the inputs
        Continue processing
```

### 3. **Stream Synchronization**

**Merge points** (nodes with multiple incoming edges) implement synchronization:

```rust
fn merge_inputs(&self, node_id: NodeId, ...) -> Vec<f32> {
    // Find all incoming edges
    let incoming_edges = edges.filter(|e| e.to_node == node_id);

    // Wait for all predecessor buffers (implicit synchronization)
    let mut merged = vec![0.0; buffer_size];

    // Mix all inputs
    for edge in incoming_edges {
        let src_buffer = &node_buffers[edge.from_node];
        // Mix src_buffer into merged
    }

    merged
}
```

**Key Point**: The merge operation is **synchronous** - it reads from all predecessor buffers, which are only written after those nodes complete. This ensures sample-accurate synchronization.

## Thread Safety

### Plugin Locking Strategy

Plugins are wrapped in `Arc<Mutex<Box<dyn Plugin>>>`:

- **Arc**: Allows sharing across threads
- **Mutex**: Ensures exclusive access during `process()`
- **Lock Duration**: Held only during the plugin's `process()` call

### Parallel Execution with Scoped Threads

```rust
std::thread::scope(|scope| {
    for &node_id in &stage.nodes {
        scope.spawn(move || {
            // Lock plugin, process, unlock
            let mut plugin = node.plugin.lock().unwrap();
            plugin.process(input, output, &context);
        });
    }
    // Implicit join - all threads complete before continuing
});
```

Benefits:
- No need for manual thread joins
- Automatic cleanup
- References to graph data valid within scope
- No lifetime issues

## Usage Examples

### Example 1: Linear Chain

```rust
let mut graph = ParallelPluginGraph::new(48000);

let node1 = graph.add_node("gain1".to_string(), Box::new(gain_plugin1))?;
let node2 = graph.add_node("gain2".to_string(), Box::new(gain_plugin2))?;

graph.add_edge(GraphEdge::new(node1, node2))?;
graph.build()?;

graph.process(&input, &mut output)?;
```

### Example 2: Parallel Diamond

```
      +-> Node2 ->+
Input -> Node1    -> Node4 -> Output
      +-> Node3 ->+
```

```rust
let mut graph = ParallelPluginGraph::new(48000);

let node1 = graph.add_node("split".to_string(), Box::new(plugin1))?;
let node2 = graph.add_node("branch_a".to_string(), Box::new(plugin2))?;
let node3 = graph.add_node("branch_b".to_string(), Box::new(plugin3))?;
let node4 = graph.add_node("merge".to_string(), Box::new(plugin4))?;

// Split
graph.add_edge(GraphEdge::new(node1, node2))?;
graph.add_edge(GraphEdge::new(node1, node3))?;

// Merge
graph.add_edge(GraphEdge::new(node2, node4))?;
graph.add_edge(GraphEdge::new(node3, node4))?;

graph.build()?;

// Node2 and Node3 will run in parallel (Stage 1)
graph.process(&input, &mut output)?;
```

### Example 3: Channel Mapping

```rust
// Map specific channels from a 5-channel output to a 2-channel input
let edge = GraphEdge::with_channels(
    surround_node,  // 5 channels output
    stereo_node,    // 2 channels input
    vec![0, 1]      // Use channels 0 and 1 from surround
);

graph.add_edge(edge)?;
```

## Stream Synchronization Deep Dive

### Why Synchronization is Needed

Consider this graph:

```
     +-> Fast Plugin (10ms) ->+
Input                          +-> Merge -> Output
     +-> Slow Plugin (50ms) ->+
```

Without synchronization:
- Fast plugin finishes at 10ms
- Slow plugin finishes at 50ms
- **Problem**: Merge node can't start until both are ready

### How It Works

1. **Stage-based Processing**: Ensures dependencies are respected
   - Fast and Slow plugins are in Stage 1 (can run in parallel)
   - Merge node is in Stage 2 (waits for Stage 1 to complete)

2. **Thread Barrier**: When using scoped threads:
   ```rust
   std::thread::scope(|scope| {
       spawn(fast_plugin);
       spawn(slow_plugin);
   }); // <-- Implicit barrier: waits for both threads
   ```

3. **Buffer Availability**: Merge reads from both buffers only after Stage 1 completes
   ```rust
   // Stage 1 writes to buffers
   fast_buffer.write(&output);
   slow_buffer.write(&output);

   // Stage 2 reads from both (guaranteed to be ready)
   let fast_data = fast_buffer.read();
   let slow_data = slow_buffer.read();
   merged = fast_data + slow_data;
   ```

### Sample-Accurate Synchronization

The system guarantees **sample-accurate synchronization**:

- All plugins process the same number of frames (from `ProcessContext::num_frames`)
- Buffers are frame-aligned
- Merge operation sums corresponding samples:
  ```rust
  for frame in 0..num_frames {
      for channel in 0..num_channels {
          merged[frame * channels + channel] =
              input1[frame * channels + channel] +
              input2[frame * channels + channel];
      }
  }
  ```

## Performance Considerations

### When Parallel Processing Helps

Parallel processing is beneficial when:

1. **CPU-intensive plugins**: FFT, convolution, ML inference
2. **Multiple independent paths**: Diamond patterns, parallel effects chains
3. **Many plugins**: Large graphs with 3+ parallel nodes per stage

### When Sequential is Better

Sequential processing may be faster for:

1. **Lightweight plugins**: Simple gain, mixing
2. **Small graphs**: 1-2 nodes per stage
3. **Thread overhead dominates**: Very short processing times

**Tip**: Use `graph.set_parallel_enabled(false)` to disable threading and compare performance.

### Memory Usage

Each node requires:
- Plugin instance: Varies by plugin type
- Output buffer: `num_frames * output_channels * sizeof(f32)` bytes
- Mutex overhead: ~40 bytes per plugin

For a typical graph with 10 nodes, 1024 frames, 2 channels:
- Buffers: `10 * 1024 * 2 * 4 = ~80 KB`
- Total: < 100 KB

## API Reference

### ParallelPluginGraph

#### Construction
```rust
pub fn new(sample_rate: u32) -> Self
```

#### Configuration
```rust
pub fn set_parallel_enabled(&mut self, enabled: bool)
```

#### Building the Graph
```rust
pub fn add_node(&mut self, name: String, plugin: Box<dyn Plugin>)
    -> Result<NodeId, String>

pub fn add_edge(&mut self, edge: GraphEdge) -> Result<(), String>

pub fn build(&mut self) -> Result<(), String>
```

#### Processing
```rust
pub fn process(&self, input: &[f32], output: &mut [f32])
    -> Result<usize, String>

pub fn reset(&self)
```

#### Introspection
```rust
pub fn num_stages(&self) -> usize
pub fn stage_info(&self, stage_idx: usize) -> Option<Vec<String>>
pub fn total_latency_samples(&self) -> usize
```

### GraphEdge

```rust
pub fn new(from: NodeId, to: NodeId) -> Self
pub fn with_channels(from: NodeId, to: NodeId, channels: Vec<usize>) -> Self
```

## Comparison with PluginHost

| Feature | PluginHost | ParallelPluginGraph (Chain Mode) | ParallelPluginGraph (Graph Mode) |
|---------|-----------|----------------------------------|----------------------------------|
| Topology | Linear chain | Linear chain | Directed acyclic graph |
| Parallel Processing | No | No* | Yes (per-stage) |
| Stream Merging | No | No | Yes (automatic sync) |
| Channel Mapping | No | No | Yes (per-edge) |
| Threading | Single-threaded | Single/Multi** | Multi-threaded (scoped) |
| API Compatibility | Native | 100% compatible | Extended API |
| Complexity | Low | Low | Medium |
| Use Case | Simple chains | Simple chains | Complex routing |

\* Parallel processing can be enabled even in chain mode for benchmarking
\** Configurable via `set_parallel_enabled()`

**Migration from PluginHost:**

```rust
// Old code with PluginHost
let mut host = PluginHost::new(2, 48000);
host.add_plugin(Box::new(plugin1))?;
host.add_plugin(Box::new(plugin2))?;
host.process(&input, &mut output)?;

// New code with ParallelPluginGraph - EXACT SAME API
let mut host = ParallelPluginGraph::new_with_channels(2, 48000);
host.add_plugin(Box::new(plugin1))?;
host.add_plugin(Box::new(plugin2))?;
host.process(&input, &mut output)?;
```

No changes needed! ParallelPluginGraph is a drop-in replacement.

## Future Enhancements

Potential improvements:

1. **Latency Compensation**: Automatically delay fast paths to align with slow paths
2. **Dynamic Graphs**: Hot-swap nodes without rebuilding
3. **GPU Acceleration**: Offload stages to GPU
4. **Lock-free Processing**: Use message passing instead of mutexes
5. **NUMA Awareness**: Pin threads to CPU cores for better cache locality

## Troubleshooting

### Common Issues

**1. Graph fails to build with "cycle detected"**
- Check for feedback loops in your graph
- Ensure edges don't create circular dependencies

**2. Output is twice as loud as expected**
- Check for unintended merging (multiple paths to output node)
- Verify gain levels at merge points

**3. Performance worse than sequential**
- Try disabling parallel processing: `graph.set_parallel_enabled(false)`
- Profile to find bottlenecks
- Consider if plugins are lightweight enough to benefit from threading

**4. Channel count mismatch errors**
- Verify input/output channel counts match at each edge
- Use `GraphEdge::with_channels()` for channel mapping

## See Also

- `src-audio/src/plugins/plugin_graph_parallel.rs` - Implementation
- `src-audio/examples/plugin_graph_demo.rs` - Examples
- `src-audio/src/plugins/host.rs` - Linear plugin host (for comparison)


## Future Enhancements

Potential future additions:
- VST3 wrapper for loading third-party plugins
- Latency compensation when chaining plugins
- MIDI parameter automation


