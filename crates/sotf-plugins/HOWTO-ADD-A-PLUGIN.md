# How to Add a New Plugin

Step-by-step checklist derived from adding the AAE (Active Acoustic Enhancement)
plugin. Use this as the canonical reference when creating any new plugin.

---

## Phase 1: Create the Plugin Crate

### 1.1 Scaffold the crate

```
crates/sotf-plugins/crates/sotf-plugin-<name>/
  src/
    lib.rs          -- Plugin struct + Plugin trait impl
    params.rs       -- PARAMS const, LAYOUT const, serde struct, build_parameters()
    <dsp modules>   -- Core DSP (e.g. fdn.rs, delay_line.rs)
  bin/
    qa_<name>.rs    -- QA binary (allocation, performance, correctness)
  benches/
    <name>-benchmark.rs
  Cargo.toml
```

### 1.2 Cargo.toml

```toml
[package]
name = "sotf-plugin-<name>"
version = "0.5.1"
edition.workspace = true
license.workspace = true
# ...

[lib]
name = "sotf_plugin_<name>"
path = "src/lib.rs"

[features]
default = []
qa = ["sotf-host/qa"]

[[bin]]
name = "qa-<name>"
path = "bin/qa_<name>.rs"
required-features = ["qa"]

[dependencies]
sotf-host = { workspace = true }
log = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
criterion = { workspace = true }
hound = { workspace = true }

[[bench]]
name = "<name>-benchmark"
harness = false
```

### 1.3 params.rs — Single Source of Truth

All defaults live here. The engine and UI derive their defaults from `PARAMS`.

```rust
use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;

pub const PARAMS: &[ParamSpec] = &[
    // 0: example_param
    ParamSpec::float("Example", "example_param", 1.0, 0.0, 10.0, 0.1, "x", "Main")
        .doc("Description for this parameter"),
    // 1: enabled
    ParamSpec::bool_param("Enabled", "enabled", true, "Main")
        .doc("Toggle processing"),
];

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ /* ControlGroups */ ],
    output: &[],
    tabs: &[ /* TabSpecs */ ],
    visualizations: &[],
    column_constraints: &[],
    dynamic_sections: &[],
};

// Derive serde defaults from PARAMS (single source of truth):
sotf_host::serde_param_default! {
    PARAMS;
    fn default_example_param() -> f32 = "example_param";
    fn default_enabled() -> bool = "enabled";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyPluginParams {
    #[serde(default = "default_example_param")]
    pub example_param: f32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for MyPluginParams {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}
```

### 1.4 lib.rs — Plugin trait

Choose the right trait:
- **`Plugin`** — if the plugin changes channel count (e.g. stereo -> 5.1).
  Implement `process(input, output, context)`.
- **`ParametricInPlacePlugin`** — if input and output channel counts are equal.
  Implement `process_in_place(buffer, context)`. Wrapped by `ParametricInPlacePluginAdapter`
  in the factory.

Hot-path rules:
- Zero heap allocations in `process()` — pre-allocate all buffers in struct fields
  during `initialize()`.
- No locks, no mutexes, no system calls.
- Call `enable_ftz_daz()` at the start and `flush_denormals_inplace()` at the end.
- All loops must be bounded.
- Guard all divisions against zero.

### 1.5 Tests

Write tests covering:
- Silence in -> silence out
- Bypass transparency
- No NaN/Inf with diverse input (sine, impulse, near-clipping)
- Energy bounded (output energy < N x input energy)
- Parameter roundtrip (set -> get)
- Reset clears state
- Multiple speaker configs (if applicable)

---

## Phase 2: Register in Workspace

### 2.1 Root Cargo.toml

```toml
# Add to [workspace] members:
"crates/sotf-plugins/crates/sotf-plugin-<name>",

# Add to [workspace.dependencies]:
sotf-plugin-<name> = { path = "crates/sotf-plugins/crates/sotf-plugin-<name>" }
```

### 2.2 sotf-plugins/Cargo.toml

```toml
sotf-plugin-<name> = { workspace = true }
```

### 2.3 sotf-plugins/src/lib.rs

Three additions:

```rust
// 1. Re-export param_specs (inside pub mod param_specs { ... })
pub mod <name> {
    pub use sotf_plugin_<name>::params::*;
}

// 2. Re-export the crate
pub use sotf_plugin_<name> as plugin_<name>;

// 3. Re-export public types
pub use plugin_<name>::{MyPlugin, MyPluginParams};
```

### 2.4 sotf-plugins/src/factory.rs

```rust
// Add to imports:
use crate::{MyPlugin, MyPluginParams, ...};

// Add match arm in create_plugin():
"<name>" => {
    let params: MyPluginParams = serde_json::from_value(parameters.clone())
        .map_err(|e| format!("Failed to parse <name> params: {e}"))?;
    let plugin = MyPlugin::from_params(params);
    Ok(Box::new(plugin))  // or ParametricInPlacePluginAdapter::new(plugin)
}
```

If the plugin changes channel count (like upmixer/AAE), don't wrap in
`ParametricInPlacePluginAdapter` — return `Box::new(plugin)` directly.

---

## Phase 3: Benchmarks

### 3.1 Dedicated benchmark (crate-level)

Create `benches/<name>-benchmark.rs` with criterion groups covering:
- Block sizes (256, 512, 1024, 2048)
- Sample rates (44100, 48000, 96000)
- Configuration variants (e.g. speaker configs, presets)
- Production config with large blocks

### 3.2 sotf-plugins/benches/all-plugins-benchmark.rs

Add a `benchmark_<name>()` function and register it in the `criterion_group!`.

### 3.3 sotf-plugins/benches/allocation-benchmark.rs

Add a `test_<name>_zero_alloc()` function and register it in the
`benchmark_zero_allocation` group.

---

## Phase 4: QA Binary

Create `bin/qa_<name>.rs` following the standard pattern:

```rust
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    // 1. Plugin-specific tests (correctness, parameter effects, etc.)
    // 2. run_standard_tests(&mut plugin, "MyPlugin");
    //    - Latency reporting
    //    - Zero allocations (CountingAlloc asserts 0 in hot path)
    //    - Performance benchmark (CPU < 5%)
}
```

Run: `cargo run --bin qa-<name> --features qa -p sotf-plugin-<name> --release`

---

## Phase 5: Engine Integration

### 5.1 sotf-engine/src/plugins/mod.rs

Five additions:

```rust
// 1. PluginType enum — add variant
enum PluginType { ..., MyPlugin, }

// 2. name(), description(), all(), release_channel() — add match arms
Self::MyPlugin => "MyPlugin",
Self::MyPlugin => "Description here",
Self::MyPlugin,                        // in all()
Self::MyPlugin => ReleaseChannel::Beta, // or Prod/Alpha

// 3. serde_param_default! block — derive defaults from PARAMS
use sotf_plugins::param_specs::<name> as <name>_specs;
sotf_plugins::serde_param_default! {
    <name>_specs::PARAMS;
    fn default_<name>_param1() -> f64 = "param1";
    // ... one per field
}

// 4. PluginSettings enum — add variant with all fields
MyPlugin {
    #[serde(default = "default_<name>_param1")]
    param1: f64,
    // ...
},

// 5. Match arms in impl PluginSettings:
//    - plugin_type() -> PluginType::MyPlugin
//    - required_input_channels() -> Some(2) or None
//    - to_plugin_config() -> PluginConfig JSON
//    - default_for() -> Self::MyPlugin { ... } from specs
```

### 5.2 sotf-engine/src/plugin_param_accessors.rs

Add entry in the `impl_param_accessors!` macro.

### 5.3 sotf-engine/src/engine/manager_thread.rs

- Add `"<name>"` to the `valid_types` array.
- Add timeout estimation: `"<name>" => { <ms> }`.

---

## Phase 6: Bridge Crates

### 6.1 plugins-bridge

- `Cargo.toml`: add dependency.
- `src/factory.rs`: add match arm in `create_plugin()` and entry in
  `available_plugin_types()`.

### 6.2 plugins-ffi/src/parameter_map.rs (if PARAMS const exists)

Add match arm in `get_param_specs()`:
```rust
"MyPlugin" | "my_plugin" => <name>::PARAMS,
```

### 6.3 plugins-nih (optional, for VST3/CLAP export)

- `Cargo.toml`: add feature flag.
- `src/lib.rs`: add `sotf_nih_plugin!` block.
- `src/wrapper.rs`: add param spec match.

---

## Phase 7: Player Integration

### 7.1 sotf-player/src/plugin_graph.rs

If the plugin changes channel count, add `PluginType::MyPlugin` alongside
`PluginType::Upmixer` in every channel-tracking match arm (there are ~6).

If it has a `speaker_config` field, add `PluginSettings::MyPlugin { speaker_config, .. }`
alongside `PluginSettings::Upmixer { speaker_config, .. }`.

### 7.2 Canonical catalog exposure

Set `allowed_in_ab_compare: true` in the plugin's canonical `PLUGIN_CATALOG`
metadata when it can be constructed without discovery or platform state. The
player and GPUI A/B pickers are derived from that metadata; do not add another
plugin list.

---

## Phase 8: App Integration

### 8.1 app-gpui/app/actions.rs

Add `QuickAddMyPlugin` to the actions macro.

### 8.2 app-gpui/ui/plugin.rs

```rust
quick_add_plugin_handler!(quick_add_my_plugin, QuickAddMyPlugin, PluginType::MyPlugin);
```

### 8.3 app-gpui/ui/render.rs

Wire the action listener:
```rust
.on_action(cx.listener(Self::quick_add_my_plugin))
```

### 8.4 app-gpui/components/plugins/

| File | What to add |
|------|-------------|
| `ui_plugin_shell.rs` | Color, icon, display name (3 match arms) |
| `ui_rack.rs` | Tooltip description |
| `ui_rack_detail.rs` | Category, single-instance constraint (if applicable), speaker config dropdown (if applicable), channel tracking |
| `ui_graph.rs` | Plugin menu entry, color, channel counts |
| `custom_view_registry.rs` | Type key mapping |

### 8.5 systemwide/crates/daemon/bin/sotf_daemon.rs

- `plugin_type_to_engine_str()`: add `PluginType::MyPlugin => "<name>"`.
- `plugin_type_category()`: add to appropriate category.

---

## Phase 9: CLI Player

### 9.1 app-cli/bin/sotf_player_cli.rs

1. **CLI args struct**: Add a `MyPluginArgs` struct with `#[derive(clap::Args)]`
   containing an `--<name>` enable flag and per-parameter `--<name>-<param>` args.
2. **PluginArgs**: Add `#[command(flatten)] <name>: MyPluginArgs` to the
   `PluginArgs` struct.
3. **Create function**: Add `fn create_<name>_plugin_config(args) -> Result<PluginConfig>`.
4. **Traditional mode** (`build_traditional_mode_plugins`): Add an `if plugins.<name>.enabled`
   block that creates and pushes the plugin config. Handle channel count changes.
5. **Rack mode** (`build_rack_mode_plugins`): Add a `"<name>"` match arm that creates
   `PluginSettings::MyPlugin { ... }` via `chain.add_plugin(&PluginType::MyPlugin)`.

---

## Phase 10: TUI Player

### 10.1 app-tui

The TUI uses the generic plugin controller from `sotf-player` and the
`PluginSettings`-based parameter system. If all engine integration (Phase 5)
is complete, the TUI works automatically:
- Plugin appears in the add-plugin menu via `PluginType::all()`
- Parameters render from the `PARAMS`/`LAYOUT` constants
- Channel routing works via `plugin_graph.rs` (Phase 7)

No TUI-specific code changes are needed unless you want custom widgets.

---

## Verification Checklist

```bash
# 1. Plugin crate
cargo check -p sotf-plugin-<name>
cargo test -p sotf-plugin-<name> --lib

# 2. QA binary
cargo run --bin qa-<name> --features qa -p sotf-plugin-<name> --release

# 3. Benchmarks
cargo bench -p sotf-plugin-<name> --bench <name>-benchmark

# 4. Parent crate
cargo check -p sotf-plugins
cargo bench -p sotf-plugins --bench allocation-benchmark -- "<name>"

# 5. Engine
cargo check -p sotf-engine

# 6. Full workspace
cargo check --workspace
```

---

## File Count Summary

Adding a new plugin touches approximately:

| Area | Files | Purpose |
|------|-------|---------|
| Plugin crate | 3-10 | DSP, params, tests, QA, bench |
| Workspace root | 1 | Cargo.toml (members + deps) |
| sotf-plugins | 3 | Cargo.toml, lib.rs, factory.rs |
| sotf-plugins/benches | 2 | all-plugins, allocation |
| plugins-bridge | 2 | Cargo.toml, factory.rs |
| sotf-engine | 3 | plugins/mod.rs, param_accessors, manager_thread |
| sotf-player | 2 | plugin_graph.rs, ab_compare_path.rs |
| app-gpui | 7 | actions, plugin, render, shell, rack, graph, registry |
| app-cli | 1 | sotf_player_cli.rs (args + create + wire) |
| app-tui | 0 | Automatic via PluginType::all() + PARAMS/LAYOUT |
| daemon | 1 | sotf_daemon.rs |
| **Total** | **~25 files** | |
