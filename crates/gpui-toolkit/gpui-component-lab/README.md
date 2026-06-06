# gpui-component-lab

Prop-driven story registry and component lab scaffolding for `gpui-toolkit`.

## What It Owns

- `ComponentStory`, `StoryRegistry`, `StoryProp`, `StoryPropValue`
- `ViewportPreset`, `ThemePreset`, `ResponsivePreviewMatrix`
- Built-in story metadata for `gpui-ui-kit`, `gpui-px`, and `gpui-audio-kit`
- Designer state persistence as `*.story.json`

## Commands

```bash
cargo run -p gpui-component-lab --bin gpui-component-lab -- --json
cargo run -p gpui-component-lab --bin gpui-component-lab -- --conformance
```

Safe live preview uses in-process polling for story/token JSON:

```bash
cargo run -p gpui-component-lab --bin gpui-component-lab -- --watch --token tokens.json
```

Rust source changes can be supervised with child-process relaunches. Dynamic
library reload is intentionally unsupported.
