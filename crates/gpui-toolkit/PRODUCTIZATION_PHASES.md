# gpui-toolkit Productization Phases

This branch contains several productization slices. Review and merge them as
separate PRs even when they are developed together locally.

## Phase 1: Design Tools

- Move generic token import/export/validation into `gpui-design-tools`.
- Keep app-specific adapters in `sotf-tools`.
- Verify `cargo test -p gpui-design-tools` and `cargo tree -p gpui-design-tools`.

## Phase 2: Audio Kit

- Move reusable audio controls and meter/spectrum UI into `gpui-audio-kit`.
- Remove audio re-exports from `gpui-ui-kit`.
- Verify `cargo test -p gpui-audio-kit` and `cargo check -p sotf-gpui`.

## Phase 3: Component Lab

- Add prop-driven stories, safe live preview, and JSON persistence.
- Register all first-party stories for `gpui-ui-kit`, `gpui-px`, and
  `gpui-audio-kit`.
- Keep Rust components as the source of truth; designer output remains
  `*.story.json`.
- Verify `cargo test -p gpui-component-lab`.

## Phase 4: Conformance Gate

- Emit JSON and Markdown conformance reports.
- Check design/theme/motion coverage, renderer coverage, responsive rendered
  bounds, touch targets, focus metadata, token reports, and builder layout
  state.
- Verify `just -f crates/gpui-toolkit/Justfile qa-gpui-conformance`.

## Phase 5: App Integration QA

- Run toolkit-wide obvious QA after the phase PRs are assembled.
- Verify `just -f crates/gpui-toolkit/Justfile qa-gpui-obvious`.
