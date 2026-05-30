# gpui-builder

Generic constraint-based layout solver for GPUI applications.

Platform-agnostic — the core solver has zero framework dependencies. An optional `showcase` feature enables a live GPUI demo binary.

## Features

- **Hard constraints** (`Sizing::Fixed`) — headers, footers, toolbars
- **Soft constraints** (`Sizing::Fractional`, `Sizing::Flex`) — resizable panels
- **Priority-based collapse** — lowest-priority panels collapse first when space is tight
- **Auto-axis switching** — flips horizontal/vertical based on aspect ratio
- **Display tiers** — panels report their active display mode (e.g. Full/Mini) based on resolved size
- **User preferences** — ratio overrides and manual collapse state
- **Draggable dividers** — configurable divider size per container
- **Text-measured sizing** — slots sized by text content via `TextMeasure` trait (from `gpui-pretext`)

## Quick Start

```rust
use gpui_builder::{solve, LayoutNode, ContainerNode, SlotNode, Sizing, Axis, LayoutPreferences};

let children = [
    LayoutNode::Slot(SlotNode {
        id: "sidebar",
        sizing: Sizing::fractional(0.2, 120.0),
        priority: 0.5,
        collapsible: true,
        display_tiers: &[],
        collapse_label: Some("Sidebar"),
    }),
    LayoutNode::Slot(SlotNode {
        id: "content",
        sizing: Sizing::flex(300.0),
        priority: 1.0,
        collapsible: false,
        display_tiers: &[],
        collapse_label: None,
    }),
];

let root = LayoutNode::Container(ContainerNode {
    id: "root",
    axis: Axis::Horizontal,
    auto_axis: Some(1.0), // switch to vertical in portrait
    sizing: Sizing::flex(0.0),
    children: &children,
    divider_size: 6.0,
});

let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());

let sidebar = solved.find("sidebar").unwrap();
println!("sidebar: {}x{} visible={}", sidebar.width, sidebar.height, sidebar.visible);
```

## Layout Stories

Define Storybook-style layout catalogs for docs, examples, tests, and future
visual tooling:

```rust
use gpui_builder::{LayoutScenario, LayoutStory, LayoutStoryCatalog};

let scenarios = [
    LayoutScenario::new("desktop", "Desktop", 1200.0, 800.0),
    LayoutScenario::new("narrow", "Narrow", 500.0, 800.0),
];
let story = LayoutStory::new("shell", "Application shell", root, &scenarios);
let stories = [story];
let catalog = LayoutStoryCatalog::new(&stories);

println!("{catalog}");
let solved = catalog.solve_all();
```

Scenarios can also carry ratio overrides and collapsed-slot preferences, so the
same catalog can drive responsive examples and regression snapshots.

## Sizing Modes

| Mode | Constructor | Behavior |
|------|-------------|----------|
| Fixed | `Sizing::Fixed(px)` | Exact pixel size, never flexes |
| Fractional | `Sizing::fractional(ratio, min)` | Fraction of remaining space with minimum |
| Flex | `Sizing::flex(min)` | Equal share of remaining space |
| Text | `Sizing::text(measure, text, opts)` | Sized by measured text content |

## Plugin Compatibility

The `compat` module bridges existing plugin layout definitions (`ColumnConstraint`) into the generic solver:

```rust
use gpui_builder::{PluginLayoutTree, PluginLayoutThresholds, plugin_adaptations};

let tree = PluginLayoutTree::from_constraints(&constraints);
let solved = solve(tree.as_layout_node(), width, height, &prefs);
let adapt = plugin_adaptations(&solved, &PluginLayoutThresholds::default());
// adapt.orientation, adapt.knob_size, adapt.slider_height, ...
```

## Showcase Binary

```bash
cargo run -p gpui-builder --features showcase --bin layout-showcase
```

Interactive demo with draggable dividers, collapsible panels, auto-axis switching, and display tiers.

## Testing

```bash
cargo test -p gpui-builder --lib
cargo check -p gpui-builder --features showcase
```
