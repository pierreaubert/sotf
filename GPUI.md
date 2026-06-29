# GPUI Guidance for SOTF

This repository uses GPUI for native Rust user interfaces. The main desktop app
is `crates/app-gpui` (`sotf-gpui` library, `sotf-desktop` binary). Shared GPUI
tooling lives next to this repo in `../gpui-toolkit`.

Read this file before changing any GPUI app, component, theme, layout, or
keyboard behavior.

## Repository Structure

SOTF app code:

- `crates/app-gpui`: desktop player app, screens, app state, actions, tests.
- `crates/app-ios`: iOS app target using the same GPUI toolkit family.
- `crates/app-tvos`: tvOS app target.
- `crates/sotf-plugins/crates/plugins-gpui`: GPUI plugin UI helpers.
- `crates/sotf-plugins/crates/plugins-au`: Audio Unit integration around plugin UI.

Toolkit reference repo:

- `../gpui-toolkit/crates/gpui-ui-kit`: reusable components such as buttons,
  inputs, menus, dialogs, tabs, tables, wizard, command palette, sidebar, and
  workflow canvas.
- `../gpui-toolkit/crates/gpui-builder`: modern constraint layout solver,
  responsive display tiers, draggable dividers, layout diagnostics, layout
  stories, and visual regression manifests.
- `../gpui-toolkit/crates/gpui-design`: platform-adaptive design system for
  spacing, radii, typography, touch targets, animation, and audio-control
  geometry.
- `../gpui-toolkit/crates/gpui-themes`: theme infrastructure, theme editor, and
  theme showcase.
- `../gpui-toolkit/crates/gpui-keybinding`: reusable keybinding registry,
  presets, conflict detection, command-palette rows, and which-key hints.
- `../gpui-toolkit/crates/gpui-miniapp`: small app shell for examples and
  showcase binaries.
- `../gpui-toolkit/crates/gpui-audio-kit`: audio controls such as knobs,
  sliders, meters, and spectrum widgets.
- `../gpui-toolkit/crates/gpui-d3rs` and `../gpui-toolkit/crates/gpui-px`:
  charting and visualization primitives.
- `../gpui-toolkit/crates/gpui-component-lab`: component lab and responsive
  preview matrix for conformance work.
- `../gpui-toolkit/crates/gpui-design-tools`: token export/import and design
  conformance tools.
- `../gpui-toolkit/crates/gpui-ios` and `../gpui-toolkit/crates/gpui-au`:
  platform backends for mobile and Audio Unit embedding.
- `../gpui-toolkit/crates/gpui-scaffolder`: CLI for generating standalone
  `gpui-miniapp` projects.
- `../gpui-toolkit/crates/gpui-showcase`: standalone component gallery binary
  for `gpui-ui-kit`.
- `../gpui-toolkit/crates/gpui-toolkit`: aggregate crate that re-exports the
  whole toolkit family (useful for quick prototypes, not the default for SOTF
  production crates).

SOTF depends on the `0.8` line of `gpui-toolkit` through workspace
dependencies in the root `Cargo.toml`. Use `../gpui-toolkit` as the local source
of truth for APIs, examples, and expected patterns.

Whenever you work on SOTF GPUI code, also read `../gpui-toolkit/gpui-skill.md`.
It contains the current toolkit-wide guidance (crate selection, rendering
patterns, performance anti-patterns, and validation workflows) that is shared
across this repo and downstream consumers.

## Required Priorities for Every GPUI App

Every GPUI app in SOTF should support these as first-class concerns, not as
later polish:

1. Modern builder/layout support
   - Prefer `gpui-builder` for complex app shells, multi-panel layouts,
     responsive collapse, draggable dividers, and debug reports.
   - Keep layout state explicit and serializable where users can resize or
     collapse panels.
   - For tricky layouts, add layout stories or solved-tree assertions.

2. Design system support
   - Use `gpui-design` / local design helpers for spacing, radii, typography,
     touch targets, and animation.
   - Do not scatter raw pixel constants through app UI. In `app-gpui`, follow
     `crates/app-gpui/AGENTS.md`: use `Ds::from_cx(cx)`, `spacing::*`,
     `radius::*`, or existing design-system wrappers.
   - Run `python3 scripts/check-design-tokens.py` before committing UI changes.
     It flags raw `px(N.0)` values and manual `Text::new(...).size().weight()`
     chains that have semantic constructors in `gpui-ui-kit`.
   - Prefer semantic typography constructors (`Text::caption`, `Text::eyebrow`,
     `Text::section_header`, `Text::label`, `Heading::h1`/`h4`) over rebuilding
     the same size/weight chain by hand.
   - Keep dense product UI calm and scannable. SOTF is an audio workstation and
     player, not a marketing page.

3. Theme support
   - Colors must come from theme state, not hardcoded UI colors.
   - New components must render correctly in light, dark, and custom themes.
   - Prefer semantic theme fields: background, surface, border, text,
     text-muted, accent, warning, error, success.
   - Data colors for charts/meters may be domain palettes, but surrounding UI
     chrome still comes from the theme.

4. Keyboard support
   - Each app screen must have keyboard behavior for its main workflow.
   - Use `gpui-keybinding` concepts where a screen has discoverable commands:
     documented bindings, categories, conflict checks, command-palette data,
     and platform-aware labels.
   - Menus, dialogs, overlays, search fields, and pickers must handle Escape,
     Enter, focus movement, and disabled states consistently.
   - Do not add mouse-only controls for important workflows.

5. Accessibility and focus
   - Keep focus ownership clear when opening and closing overlays.
   - Use accessible labels/roles when components expose them.
   - Keyboard navigation should match visible selection state.

## Performance & Correctness

Both SOTF and `gpui-toolkit` have repeatedly fixed the same hot-path mistakes.
Avoid them in new GPUI code:

### Zero-copy and caching

- Move data generation (paths, scales, tick arrays, spectra) out of the
  paint/render closure and cache it in the component model.
- Never clone the full theme inside a render loop; theme state is already
  reference-counted (`Arc<Theme>`) in the toolkit.
- Reuse buffers and textures rather than reallocating per frame. Per-frame
  allocations in meters, spectrum views, and waveform renders are a common
  source of UI jank.
- Use `gpui-profiler` from `../gpui-toolkit` when diagnosing allocation hot
  paths.

### Animation lifetimes

- Use `WeakEntity` for animation timers so closures do not keep dead views
  alive.
- Drive animations from background timers with a capped frame rate, not from
  `Render`.

### Focus and input stability

- `focus_handle.is_focused(window)` returns `false` during
  `RenderOnce::render()` because the old element is destroyed before the new
  one calls `.track_focus()`. Do not gate editing state on `is_focused()` during
  render; trust the component's internal state and use `window.on_focus_out()`
  for blur detection.
- GPUI dispatches action key bindings **before** `on_key_down` handlers. If the
  app binds `-`, `enter`, or similar keys, those bindings consume keystrokes
  before `Input`/`NumberInput` sees them. Switch the root `key_context` between
  `"PlayerView"` and `"TextInput"` and include
  `gpui_ui_kit::is_number_input_editing()` in the text-input-mode check.
- Never wrap an `Input` or `NumberInput` in a parent
  `div().on_key_down(|..| cx.stop_propagation())`. Parent capture-phase handlers
  fire before the focused child's handler and will block all keystrokes. The
  input components already call `cx.stop_propagation()` internally when they
  consume a key.

### Layout and state

- Persist expensive interactive surfaces (e.g., `WorkflowCanvas`, complex
  pickers) in the parent model instead of reconstructing them every render.
- Avoid cloning full collections (queues, groups, peak-hold arrays, loudness
  history) on every render; pass references or cache the rendered data.

## Building SOTF GPUI Apps

From the SOTF repo root:

```bash
cargo check -p sotf-gpui
cargo test -p sotf-gpui test_album_context_menu --test e2e
cargo run --bin sotf-desktop
```

For a release-style desktop build:

```bash
cargo run --bin sotf-desktop --release
```

Useful focused checks:

```bash
cargo test -p sotf-gpui --test e2e
cargo test -p sotf-gpui room_eq
python3 scripts/check-design-tokens.py
```

The `app-gpui` crate disables ordinary lib tests in some places because of GPUI
macro stack limits. Prefer targeted e2e, component, lifecycle, state-machine,
and config test binaries when possible.

## Building the Toolkit

From `../gpui-toolkit`:

```bash
just --list
just check
just qa-gpui-obvious
just demo
just examples
```

Focused toolkit checks:

```bash
cargo test -p gpui-builder
cargo test -p gpui-design
cargo test -p gpui-keybinding
cargo test -p gpui-ui-kit
cargo check -p gpui-themes
```

Showcase builds:

```bash
# Standalone component gallery (now in its own crate)
cargo run -p gpui-showcase --release

# Family demos
just demo-ui-kit
just demo-builder
just demo-component-lab
just demo-themes
just examples-audio-kit
```

Use toolkit examples and showcases to understand component behavior before
reimplementing it in SOTF.

## Debugging a GPUI App

Start narrow:

1. Reproduce the exact interaction in the smallest screen or test.
2. Find the state owner: `app-gpui/app/state`, screen component, or shared
   `sotf-player` controller.
3. Check whether a GPUI action changes app state, player state, and UI state in
   the same transaction.
4. Verify overlays close and input mode returns to `Normal`.
5. Verify async workers clear their loading flag on success, error, and channel
   disconnect.
6. Run the narrowest e2e or component test, then `cargo check -p sotf-gpui`.

For layout bugs:

- Use `gpui-builder` solved-tree debug reports for panel shells.
- Inspect fixed dimensions, min sizes, collapse priorities, and active display
  tiers.
- Add a story or regression for desktop, narrow, and short-height viewports.

For theme bugs:

- Check light, dark, and custom themes.
- Search for hardcoded `rgb`, `rgba`, `hsla`, or raw color constants.
- Confirm charts bridge from theme to chart axis/grid/text colors.

For keyboard bugs:

- Check the active `InputMode`.
- Check focus handle ownership.
- Confirm Escape closes overlays before global actions fire.
- Confirm Enter/Space activation matches click activation.
- Run keybinding conflict checks when using `gpui-keybinding`.

For playback or long-running UI spinners:

- Do not assume the spinner owns the bug. Trace the action from UI event to app
  state to player/controller result.
- Loading flags must be reset on every return path.
- An "Add" action should not start playback unless the UI label says so.

## Implementation Rules

- Keep app crates thin. Business logic belongs in `sotf-player`, `sotf-engine`,
  plugin crates, or domain crates.
- Prefer `gpui-ui-kit`, `gpui-audio-kit`, `gpui-builder`, `gpui-design`, and
  `gpui-keybinding` over local one-off widgets.
- Use native GPUI `div()` rendering and `impl IntoElement`; do not introduce
  HTML/SVG-style UI unless the surrounding component already does so for a
  specific reason.
- Do not duplicate theme, design, or keybinding registries in app code.
- Add tests at the state/controller boundary for behavior and e2e/component
  tests for UI interactions.
- Preserve user worktree changes. This repo often has unrelated GPUI work in
  flight.

## Where to Look First

SOTF:

- `crates/app-gpui/app/state`: app model and UI state.
- `crates/app-gpui/app/queue.rs`: queue actions.
- `crates/app-gpui/ui`: GPUI action handlers, render shell, playback bridge.
- `crates/app-gpui/components`: reusable app components and screens.
- `crates/app-gpui/tests`: e2e, lifecycle, state-machine, component, and config
  tests.
- `crates/app-gpui/AGENTS.md`: local typography and design-token rules.

Toolkit:

- `../gpui-toolkit/README.md`: crate map and common commands.
- `../gpui-toolkit/MIGRATION.md`: design-system and theme migration rules.
- `../gpui-toolkit/crates/gpui-builder/README.md`: layout solver and debug
  reports.
- `../gpui-toolkit/crates/gpui-design/README.md`: design presets and `cx.design`.
- `../gpui-toolkit/crates/gpui-keybinding/README.md`: keybinding registry and
  discovery data.
- `../gpui-toolkit/crates/gpui-ui-kit`: component implementations and examples.
