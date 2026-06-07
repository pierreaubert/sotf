# gpui-component-lab

Prop-driven story registry and component lab scaffolding for `gpui-toolkit`.

## What It Owns

- `ComponentStory`, `StoryRegistry`, `StoryProp`, `StoryPropValue`
- `ViewportPreset`, `ThemePreset`, `MotionPreset`, `ResponsivePreviewMatrix`
- Built-in story metadata for `gpui-ui-kit`, `gpui-px`, and `gpui-audio-kit`
- Renderer-backed built-in stories:
  `ui-kit.button`, `ui-kit.form`, `ui-kit.status`, `ui-kit.navigation`,
  `ui-kit.feedback`, `ui-kit.card`, every public `gpui-ui-kit` showcase
  section, `px.line`, `px.bar`, `px.scatter`, `px.area`, `px.heatmap`,
  `px.contour`, `px.isoline`, `px.pie`, `px.boxplot`, `px.treemap`,
  `px.surface3d`, `audio-kit.potentiometer`, `audio-kit.vertical-slider`,
  `audio-kit.volume-knob`, `audio-kit.meter`, `audio-kit.horizontal-meter`,
  `audio-kit.spectrum`, and `audio-kit.spectrum-axis`
- Designer state persistence as `*.story.json`, including selected motion and
  builder layout constraints, alignment, overflow, surface, gap, and border
  settings
- Conformance checks that first-party toolkit stories have matching lab
  renderers, responsive rendered bounds, touch target metadata, focus labels,
  valid builder layout state, design presets, motion presets, and token reports

## Commands

```bash
cargo run -p gpui-component-lab --bin gpui-component-lab -- --json
cargo run -p gpui-component-lab --bin gpui-component-lab -- --conformance
cargo run -p gpui-component-lab --bin gpui-component-lab -- \
  --conformance \
  --report-json target/gpui-conformance/component-lab.json \
  --report-markdown target/gpui-conformance/component-lab.md
```

Safe live preview launches the lab and uses in-process polling for story/token
JSON:

```bash
cargo run -p gpui-component-lab --bin gpui-component-lab -- --watch --token tokens.json
```

Rust source changes can be supervised with child-process relaunches. Dynamic
library reload is intentionally unsupported.

```bash
cargo run -p gpui-component-lab --bin gpui-component-lab -- \
  --supervise-rust \
  --child-command "cargo run -p gpui-component-lab --bin gpui-component-lab -- --watch"
```
