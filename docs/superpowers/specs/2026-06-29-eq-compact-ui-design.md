# Compact EQ UI Design — Layouts B & C for Small Windows

**Date:** 2026-06-29  
**Scope:** `crates/app-gpui/components/plugins/ui_eq`  
**Status:** Design draft pending approval

## Problem

The current EQ UI (`render_eq_plugin` in `ui_eq/render.rs`) always renders:

1. A full-width frequency-response graph.
2. A header bar with global steppers/toggles.
3. A channel-mode selector.
4. A horizontal band-tab strip.
5. A selected-band knob panel.

On small windows this stack becomes too tall and the band tabs / knobs wrap awkwardly. We need two compact alternatives that preserve every parameter and every existing interaction, just arranged for limited space.

## Goals

- Provide **Layout B** (bottom band strip) for medium-small widths where the graph is still useful.
- Provide **Layout C** (inspector-only list) for very narrow widths where the graph would be unusable.
- Keep the current layout for large windows (`>= 900 px`).
- Make all global params editable in compact modes, not read-only.
- Keep Mute / Solo / Bypass / Active buttons functional.
- Reuse existing state and edit methods; add only a single optional UI-state flag.

## Non-goals

- No new DSP behaviour.
- No changes to parameter specs or MIDI mappings.
- No redesign of the graph itself.

## Responsive breakpoints

| Window width | Layout |
|--------------|--------|
| `>= 900 px` | Current stacked layout (graph + header + tabs + knobs) |
| `600 px – 900 px` | **Layout B** — graph top, bottom band strip |
| `< 600 px` | **Layout C** — inspector-only list; graph hidden by default |

Values are starting defaults; final breakpoints should be tuned after testing on the target display.

## State changes

1. **`EqRenderState`** gains an `available_width: f32` field so the renderer can choose a layout without reading `window_width`.
2. **`PluginUiState`** gains two UI-only bool fields (both default `false`):
   - `eq_compact_config_open` — toggles the inline expandable global config panel.
   - `eq_compact_graph_visible` — in Layout C, lets the user temporarily show the graph even though the narrow default hides it.

Both fields derive `Default`.

## Routing change

`custom_view_registry/render.rs::render_eq` currently calls `render_eq_plugin(...)` without width. Update it to pass `ctx.available_width` into `EqRenderState`. `render_eq_plugin` then selects the layout branch.

```rust
// In custom_view_registry/render.rs
let available_width = ctx.available_width; // already present in CustomViewRenderContext

super::super::render_eq_plugin(
    ctx.entity.clone(),
    ctx.plugin_idx,
    ui_eq::EqRenderState {
        // ... existing fields ...
        available_width,
    },
    ctx.theme,
    cx,
)
```

## Shared compact components

All compact layouts reuse the same primitives:

- **`render_compact_global_bar(d, entity, plugin_idx, state, theme)`**
  Slim top bar with:
  - Mode label (`EQ`, `Linear-Phase EQ`, `FIR Designer`).
  - `[Config ⚙]` toggle that expands an inline panel below the bar.
  - `[Graph □/■]` toggle (Layout C only).

- **`render_compact_config_panel(...)`**
  Inline expandable panel containing:
  - Per-channel / All-Channels toggle and channel selector pills (Standard EQ only).
  - Global steppers/toggles:
    - Standard: `Filters`, `Topology` (Biquad/SVF), `TDF-II` (On/Off).
    - Linear-Phase: `Filters`, `FIR length`, `Auto-gain`, `Mix` + latency readout.
    - FIR Designer: same as Linear-Phase plus `Phase` (Linear/Minimum).

- **`render_band_inline_editor(d, entity, plugin_idx, filter, band_idx, indexing, state, theme)`**
  A narrow column with:
  - Filter-type button set: `PK LS HS LP HP BP NO` (and `AP` where applicable).
  - Three small knobs / sliders: `Freq`, `Q`, `Gain`.
  - Action row: `[M] [S] [B]` (mute, solo, bypass). For FIR/LP variants also `[A]` (active).
  - For Standard EQ: topology pill (Biquad/SVF) only if the global topology is set to mixed per-band; otherwise omit.

- **`render_band_mute_solo_button(...)` / `render_band_active_toggle(...)`**
  Keep the existing mouse-down handlers (`toggle_eq_band_mute`, `toggle_eq_band_solo`, etc.). Do not rebuild interaction logic.

- **`render_add_band_button(...)`**
  Reuse the existing `add_eq_band` call.

## Layout B — Bottom band strip

### Structure

```
┌────────────────────────────────────────────────────────────────┐
│  EQ [Config ⚙] [All Channels ▼]                [Ch1] [Ch2] ... │  <- global bar
├────────────────────────────────────────────────────────────────┤
│                                                                │
│              Frequency Response Graph                          │
│                                                                │
├────────────────────────────────────────────────────────────────┤
│  #1 PK  1k  +3  │ #2 LS  120  +6 │ #3 HS  8k  -2 │ + │        │  <- band strip
├────────────────────────────────────────────────────────────────┤
│  ▶ #2 LS   Freq[━━━━●━━━━]  Q[━━●━━]  Gain[━━━━━●━━━]         │
│    [PK][LS][HS][LP][HP][BP][NO]   [M] [S] [B]                 │
└────────────────────────────────────────────────────────────────┘
```

### Behaviour

- The graph keeps the same draggable control points and legend.
- Band strip shows one compact card per band: `#N TYPE  FREQ  GAIN`.
- Clicking a card selects that band and expands the inline editor below the strip.
- Only one band is expanded at a time.
- The `[+]` card at the end adds a new band.
- Horizontal overflow scrolls; cards do not wrap, preserving vertical space.

### Sizing

- Graph height: `200 px` (reduced from `300 px`).
- Band card height: `~48 px`.
- Inline editor height: `~120 px`.
- Total target height: `≤ 380 px`.

## Layout C — Inspector-only narrow mode

### Structure

```
┌─────────────────────────────────┐
│  EQ [Config ⚙] [Graph □]        │  <- global bar
├─────────────────────────────────┤
│  #1 PK   1.0kHz   +3dB          │
│  Freq [━━━━━●━━━━]              │
│  Q    [━━━●━━━━━]  Gain [━━●━━] │
│  [PK][LS][HS][LP][HP][BP][NO]   │
│  [M] [S] [B]                    │
├─────────────────────────────────┤
│  #2 LS   120Hz    +6dB          │
│  Freq [━━━━●━━━━━]              │
│  Q    [━━━━●━━━━]  Gain [━━━●━━]│
│  [PK][LS][HS][LP][HP][BP][NO]   │
│  [M] [S] [B]                    │
├─────────────────────────────────┤
│  #3 HS   8.0kHz   -2dB          │
│  ...                            │
├─────────────────────────────────┤
│  + Add band                     │
└─────────────────────────────────┘
```

### Behaviour

- The graph is hidden by default.
- Each band is a self-contained editor row; all bands are editable without selecting first.
- The `[Graph □]` toggle switches to a temporary graph overlay that covers the list. A second tap (or `[List]` button) returns to the list.
- When the graph overlay is open, it behaves like Layout B's graph area plus a single "selected band" inline editor at the bottom.
- Vertical overflow scrolls the whole inspector.

### Sizing

- Minimum supported width: `320 px`.
- Each band row height: `~140 px`.
- Target: 2–3 bands visible without scrolling.

## Global config panel

Both B and C move the full set of global controls into an inline expandable panel below the global bar, keeping the main surface clean without requiring a popover API.

```
┌─────────────────────────────┐
│  EQ [Config ⚙] [Graph □]    │
├─────────────────────────────┤
│  CONFIG                     │
│  [All Channels] [Per Channel]│
│  Filters  [  8  ] ◀ ▶       │
│  Topology [Biquad] [SVF]    │
│  TDF-II   [On] [Off]        │
│  ─────────────────────────  │
│  Latency: 1023 smp (21 ms)  │
│  Auto-gain [On] [Off]       │
│  Mix       [ 100% ] ◀ ▶     │
└─────────────────────────────┘
```

Controls in the panel use the same `EqGlobalControl` enum and edit methods already implemented in `ui_eq/render.rs`. The panel is toggled by the `[Config ⚙]` button.

## MIDI overlay support

- Small knobs in the inline editor must still call the existing `render_eq_knob_with_midi` helper so MIDI-assignment badges and the page indicator remain visible.
- When a knob is too small for a badge, fall back to a dot indicator that expands on hover.

## Keyboard / accessibility

- Band tabs/cards remain focusable and respond to Enter to select.
- Arrow keys move between knobs in an inline editor row.
- The global bar buttons have explicit `id(...)` for focus tracking.

## Implementation outline

1. Add `available_width: f32` to `EqRenderState` in `ui_eq/types.rs`.
2. Add `eq_compact_graph_visible: bool` to `PluginUiState` in `app/state/plugin.rs`.
3. Update `custom_view_registry/render.rs::render_eq` to populate `available_width`.
4. In `ui_eq/render.rs::render_eq_plugin`:
   - Read `available_width`.
   - Branch on width:
     - `>= 900 px`: existing body.
     - `600–900 px`: call new `render_eq_layout_b(...)`.
     - `< 600 px`: call new `render_eq_layout_c(...)`.
5. Extract reusable helpers:
   - `render_compact_global_bar`
   - `render_band_inline_editor`
   - `render_band_card_compact`
6. Implement Layout B and Layout C in new private functions at the bottom of `ui_eq/render.rs`.
7. Add a simple GPUI popover for the global config, or reuse the existing rack config popover if appropriate.

## Files touched

- `crates/app-gpui/components/plugins/ui_eq/types.rs`
- `crates/app-gpui/components/plugins/ui_eq/render.rs`
- `crates/app-gpui/components/plugins/custom_view_registry/render.rs`
- `crates/app-gpui/app/state/plugin.rs`

## Verification

- Manual resize test: verify layout switches at 900 px and 600 px.
- All EQ variants (Standard, Linear-Phase, FIR Designer) render correctly in B and C.
- All global params can be edited via the config popover.
- Mute / Solo / Bypass / Active buttons update the engine state.
- MIDI badges still appear on assigned knobs.
- Graph control points remain draggable in Layout B and in Layout C's graph overlay.

## Open questions

1. Should the 900 / 600 px breakpoints be configurable in settings, or hard-coded?
2. Should Layout C persist the user's `[Graph □]` choice across plugin re-opens, or reset to hidden each time?
3. Should the channel selector be hidden entirely in Layout C and surfaced only through `[Config]`, or kept in the global bar?
