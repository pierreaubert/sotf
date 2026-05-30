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
- **Layout diagnostics** — stable solved-tree reports with sizing metadata and warnings

## Quick Start

```rust
use gpui_builder::{solve, LayoutNode, ContainerNode, SlotNode, Sizing, Axis, LayoutPreferences};

let children = [
    SlotNode::new("sidebar", Sizing::fractional(0.2, 120.0))
        .collapsible(0.5, "Sidebar")
        .into_node(),
    LayoutNode::slot("content", Sizing::flex(300.0)),
];

let root = ContainerNode::new("root", Axis::Horizontal, Sizing::flex(0.0), &children)
    .auto_axis(1.0) // switch to vertical in portrait
    .divider_size(6.0)
    .into_node();

let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());

let sidebar = solved.find("sidebar").unwrap();
println!("sidebar: {}x{} visible={}", sidebar.width, sidebar.height, sidebar.visible);
```

## Layout Diagnostics

Use `SolvedNode::debug_report()` while iterating on complex layouts. If you
still have the declaration tree, `debug_report_with_source()` adds sizing mode,
collapse priority, and collapsibility metadata to each line.

```rust
let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());
let report = solved.debug_report_with_source(&root);

println!("{report}");
assert!(report.is_clean());
```

Reports flag suspicious output such as invalid sizes, hidden nodes without a
collapse label, and visible children that overflow a parent axis.

## Macro DSL

Use `solve_layout!` when you want to describe and solve a nested tree in one
expression without manually threading child arrays through every container.
Node identifiers become layout ids via `stringify!`, and the macro returns a
`SolvedNode`.

```rust
use gpui_builder::{Axis, LayoutPreferences, Sizing, solve_layout};

let solved = solve_layout! {
    width: 1200.0,
    height: 800.0,
    prefs: &LayoutPreferences::default(),
    container root(Axis::Horizontal, Sizing::flex(0.0);
        auto_axis = 1.0,
        divider_size = 6.0
    ) {
        slot sidebar(Sizing::fractional(0.2, 120.0);
            priority = 0.5,
            collapsible = true,
            collapse_label = "Sidebar"
        );
        slot content(Sizing::flex(300.0));
    }
};

## Responsive Snapshots

Use `solve_snapshot_matrix` to inspect the same layout across named viewport
sizes from tests, examples, or CI logs without running the GPUI showcase.

```rust
use gpui_builder::{LayoutPreferences, LayoutViewport, solve_snapshot_matrix};

let viewports = [
    LayoutViewport::new("desktop", 1200.0, 800.0),
    LayoutViewport::new("portrait", 500.0, 800.0),
];

let matrix = solve_snapshot_matrix(&root, &viewports, &LayoutPreferences::default());
println!("{}", matrix.to_markdown_table());
```
## Layout Validation

Run validation in examples, tests, or debug tooling before solving a layout tree:

```rust
use gpui_builder::validate_layout;

let report = validate_layout(&root);
assert!(report.is_clean(), "{report}");
```

Validation reports hard errors for ids and numeric constraints that can make layout
behavior ambiguous, and warnings for quality issues such as unlabeled collapsible
slots, duplicate display tiers, and empty containers.

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
