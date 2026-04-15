# 0.6.16

## Features

### Accessibility (ARIA) support

GPUI has no native accessibility support. This release adds a UI-kit-level
accessibility layer so components carry semantic metadata (roles, labels,
states) that can be queried by tests, dev tools, and future screen reader
bridges.

**New module: `accessibility.rs`**

- `AriaRole` enum (30+ WAI-ARIA roles: Button, Checkbox, Switch, Slider, Combobox, Dialog, Alert, etc.)
- `AriaState` enum (Checked, Mixed, Pressed, Expanded, Selected, Disabled, Hidden)
- `AriaLive` enum (Off, Polite, Assertive)
- `AriaProps` struct with builder pattern for role, description, states, live regions, value ranges
- `AccessibilityNode` — element ID + label + props
- `AccessibilityTree` — GPUI Global that collects registrations per render frame
- `AccessibilityExt` trait on `App` — `register_accessible()` / `accessibility_tree()`

**Component integration (22 components)**

Every component gets `.aria_label()` and `.aria_role()` builder methods.
Components auto-register in the `AccessibilityTree` during `render()` with
sensible defaults:

| Component | Default Role | States |
|-----------|-------------|--------|
| Button, IconButton | Button | Disabled, Pressed |
| Checkbox | Checkbox | Checked/Mixed, Disabled |
| Toggle | Switch | Checked, Disabled |
| Input | Textbox | Disabled |
| NumberInput | Spinbutton | Disabled, value_range |
| Slider, Potentiometer, VerticalSlider, VolumeKnob | Slider | Disabled, value_range |
| Select | Combobox | Expanded, Disabled |
| Dialog | Dialog | — |
| ConfirmDialog | Alertdialog | — |
| Alert | Alert | — |
| Toast | Status/Alert | AriaLive (Polite/Assertive by variant) |
| Tabs | Tablist | — |
| Menu | Menu | — |
| SearchBar | Search | — |
| Progress | Progressbar | value_range |
| Table | Table | — |
| TreeView | Tree | — |
| Accordion | Group | — |
| Toolbar | Toolbar | — |
| Breadcrumbs | Navigation | — |
| Tooltip | Tooltip | — |

**Showcase section**

New "Accessibility" section in the component showcase demonstrating
`.aria_label()`, `.aria_role()`, default roles, and custom role overrides.

**MiniApp initialization**

`MiniApp` automatically initializes the `AccessibilityTree` global.

### Usage

```rust
// Icon-only button with accessible name
Button::new("save-btn", "💾")
    .aria_label("Save document")

// Override default role
Button::new("link-btn", "Visit website")
    .aria_role(AriaRole::Link)

// Query the tree in tests
let tree = cx.global::<AccessibilityTree>();
let node = tree.get(&ElementId::Name("save-btn".into()));
assert_eq!(node.unwrap().props.role, AriaRole::Button);
```
