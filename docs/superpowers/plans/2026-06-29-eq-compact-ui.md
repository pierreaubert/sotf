# Compact EQ UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two compact EQ layouts for small windows — a bottom band-strip layout and an inspector-only layout — while keeping the current large-window layout unchanged.

**Architecture:** Introduce a width-based layout selector, move shared compact controls into a new `ui_eq/layout_compact.rs` module, and branch at the end of `render_eq_plugin`. All existing interaction handlers are reused; only UI-only state flags and a width field are added.

**Tech Stack:** Rust, GPUI, existing `sotf-gpui` crate.

---

## File map

| File | Responsibility |
|------|----------------|
| `crates/app-gpui/components/plugins/ui_eq/types.rs` | `EqCompactLayout` enum, `available_width` field on `EqRenderState`. |
| `crates/app-gpui/components/plugins/ui_eq.rs` | Add `mod layout_compact;`, inline unit tests. |
| `crates/app-gpui/components/plugins/ui_eq/layout_compact.rs` | New file: shared compact helpers + `render_eq_bottom_strip` + `render_eq_inspector`. |
| `crates/app-gpui/components/plugins/ui_eq/render.rs` | Make private helpers `pub(crate)`, branch on `EqCompactLayout` at end of `render_eq_plugin`. |
| `crates/app-gpui/components/plugins/custom_view_registry/render.rs` | Pass `ctx.available_width` into `EqRenderState`. |
| `crates/app-gpui/app/state/plugin.rs` | Add `eq_compact_config_open` and `eq_compact_graph_visible` to `PluginUiState`. |

---

## Task 1: Layout selector enum + unit test

**Files:**
- Modify: `crates/app-gpui/components/plugins/ui_eq/types.rs`
- Modify: `crates/app-gpui/components/plugins/ui_eq.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `crates/app-gpui/components/plugins/ui_eq.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::types::EqCompactLayout;

    #[test]
    fn layout_selection_breakpoints() {
        assert_eq!(EqCompactLayout::from_width(1000.0), EqCompactLayout::Current);
        assert_eq!(EqCompactLayout::from_width(900.0), EqCompactLayout::Current);
        assert_eq!(EqCompactLayout::from_width(750.0), EqCompactLayout::BottomStrip);
        assert_eq!(EqCompactLayout::from_width(600.0), EqCompactLayout::BottomStrip);
        assert_eq!(EqCompactLayout::from_width(599.0), EqCompactLayout::Inspector);
        assert_eq!(EqCompactLayout::from_width(320.0), EqCompactLayout::Inspector);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p sotf-gpui layout_selection_breakpoints -- --nocapture
```

Expected: compile error because `EqCompactLayout` does not exist.

- [ ] **Step 3: Add the enum and method**

In `crates/app-gpui/components/plugins/ui_eq/types.rs`, after `EqViewMode` add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqCompactLayout {
    /// Existing large-window stacked layout.
    Current,
    /// Graph on top, horizontal band strip + inline editor below.
    BottomStrip,
    /// Scrollable band list; graph hidden by default.
    Inspector,
}

impl EqCompactLayout {
    pub fn from_width(width: f32) -> Self {
        if width >= 900.0 {
            Self::Current
        } else if width >= 600.0 {
            Self::BottomStrip
        } else {
            Self::Inspector
        }
    }
}
```

Then add `available_width: f32` to `EqRenderState`:

```rust
pub struct EqRenderState<'a> {
    // ... existing fields ...
    /// Width available for the plugin surface, used to pick a compact layout.
    pub available_width: f32,
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cargo test -p sotf-gpui layout_selection_breakpoints -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-gpui/components/plugins/ui_eq/types.rs crates/app-gpui/components/plugins/ui_eq.rs
git commit -m "feat(eq): add compact layout selector enum and breakpoints test"
```

---

## Task 2: Thread `available_width` into `EqRenderState`

**Files:**
- Modify: `crates/app-gpui/components/plugins/custom_view_registry/render.rs`

- [ ] **Step 1: Update the three EQ render call sites**

In `render_eq`, `render_linear_phase_eq`, and `render_fir_designer`, add `available_width: ctx.available_width` to the `EqRenderState` literal.

Example for `render_eq`:

```rust
super::super::render_eq_plugin(
    ctx.entity.clone(),
    ctx.plugin_idx,
    ui_eq::EqRenderState {
        channels: *channels,
        filters,
        channel_filters,
        per_channel_mode: *per_channel_mode,
        is_editing: ctx.is_editing,
        selected_param: ctx.selected_param,
        selected_band_idx,
        midi_overlay: ctx.midi_overlay,
        mode: ui_eq::EqViewMode::Standard,
        num_filters: *max_filters,
        tdf2: *tdf2,
        topology: *topology,
        available_width: ctx.available_width,
    },
    ctx.theme,
    cx,
)
```

Do the same for `render_linear_phase_eq` and `render_fir_designer`.

- [ ] **Step 2: Check compilation**

Run:

```bash
cargo check -p sotf-gpui
```

Expected: no errors (the field is unused until Task 7).

- [ ] **Step 3: Commit**

```bash
git add crates/app-gpui/components/plugins/custom_view_registry/render.rs
git commit -m "feat(eq): pass available_width into EqRenderState"
```

---

## Task 3: Add compact UI state flags

**Files:**
- Modify: `crates/app-gpui/app/state/plugin.rs`

- [ ] **Step 1: Add two bool fields to `PluginUiState`**

```rust
#[derive(Debug, Clone, Default)]
pub struct PluginUiState {
    /// Which plugin UI view mode to show
    pub plugin_ui_view: PluginUiView,
    /// Whether the controller picker dropdown is open
    pub controller_picker_open: bool,
    /// Whether the rack-level plugin configuration popover is open.
    pub rack_config_overlay_open: bool,
    /// Whether the plugin skin picker dropdown is open.
    pub rack_skin_picker_open: bool,
    /// Compact EQ: whether the global config panel is expanded.
    pub eq_compact_config_open: bool,
    /// Compact EQ: whether the graph is visible in inspector mode.
    pub eq_compact_graph_visible: bool,
}
```

- [ ] **Step 2: Check compilation**

Run:

```bash
cargo check -p sotf-gpui
```

Expected: no errors (`#[derive(Default)]` handles the new bools).

- [ ] **Step 3: Commit**

```bash
git add crates/app-gpui/app/state/plugin.rs
git commit -m "feat(eq): add compact UI state flags"
```

---

## Task 4: Make existing EQ helpers reusable

**Files:**
- Modify: `crates/app-gpui/components/plugins/ui_eq/render.rs`

- [ ] **Step 1: Change visibility of helpers**

Find these definitions and change them from `fn` / `struct` to `pub(crate) fn` / `pub(crate) struct`:

- `EqGlobalControl`
- `EqBandIndexing`
- `render_eq_visualization`
- `render_eq_knob_with_midi`
- `render_filter_type_selector`
- `render_eq_active_toggle`
- `render_eq_global_stepper`
- `render_eq_global_toggle`

For example:

```rust
pub(crate) struct EqBandIndexing { ... }
pub(crate) fn render_eq_visualization(...) -> impl IntoElement { ... }
```

- [ ] **Step 2: Check compilation**

Run:

```bash
cargo check -p sotf-gpui
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/app-gpui/components/plugins/ui_eq/render.rs
git commit -m "refactor(eq): make compact-layout helpers pub(crate)"
```

---

## Task 5: Create `layout_compact.rs` with shared helpers

**Files:**
- Create: `crates/app-gpui/components/plugins/ui_eq/layout_compact.rs`
- Modify: `crates/app-gpui/components/plugins/ui_eq.rs`

- [ ] **Step 1: Register the new module**

In `crates/app-gpui/components/plugins/ui_eq.rs`, add:

```rust
mod layout_compact;
pub use layout_compact::*;
```

- [ ] **Step 2: Create `layout_compact.rs` with the full helper set**

Create `crates/app-gpui/components/plugins/ui_eq/layout_compact.rs` with the following complete content. This file is intentionally self-contained: all layout-specific primitives, the two layout entry points, and the small reusable buttons.

```rust
//! Compact EQ layouts for small windows.
//!
//! - `render_eq_bottom_strip`: graph on top, horizontal band strip + inline editor below.
//! - `render_eq_inspector`: scrollable band list; graph optional.

use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;
use sotf_audio_player_midi::mapping::MidiOverlay;
use sotf_plugins::param_specs::{eq::BAND_TEMPLATE as EQ, find_by_key as pk};

use super::render::{
    render_eq_active_toggle, render_eq_global_stepper, render_eq_global_toggle,
    render_eq_knob_with_midi, render_eq_visualization, render_filter_type_selector,
    EqBandIndexing, EqGlobalControl,
};
use super::types::{EqRenderState, EqViewMode};

const COMPACT_GRAPH_HEIGHT: f32 = 200.0;

/// Bottom-strip layout: graph on top, band cards below, selected band expands inline.
pub fn render_eq_bottom_strip(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: EqRenderState,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    indexing: EqBandIndexing,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let config_open = entity.read(cx).app.plugin_state.plugin_ui_state.eq_compact_config_open;

    let mut root = div()
        .flex()
        .flex_col()
        .gap(d.section)
        .size_full();

    root = root.child(render_compact_global_bar(
        &d, entity.clone(), plugin_idx, &state, false, theme, cx,
    ));

    if config_open {
        root = root.child(render_compact_config_panel(
            &d, entity.clone(), plugin_idx, &state, theme, cx,
        ));
    }

    root = root.child(
        div()
            .h(px(COMPACT_GRAPH_HEIGHT))
            .child(render_eq_visualization(
                entity.clone(),
                plugin_idx,
                display_filters,
                Some(selected_band_idx),
                indexing,
                theme,
                state.available_width,
            )),
    );

    let mut strip = div()
        .flex()
        .gap(d.gap)
        .overflow_x_scroll()
        .px(d.pad_x);

    for (i, filter) in display_filters.iter().enumerate() {
        strip = strip.child(render_compact_band_card(
            &d,
            entity.clone(),
            plugin_idx,
            i,
            filter,
            i == selected_band_idx,
            theme,
        ));
    }
    strip = strip.child(render_add_band_button(&d, entity.clone(), plugin_idx, theme));
    root = root.child(strip);

    if let Some(filter) = display_filters.get(selected_band_idx) {
        root = root.child(render_compact_band_editor(
            &d,
            entity.clone(),
            plugin_idx,
            selected_band_idx,
            filter,
            indexing,
            &state,
            theme,
        ));
    }

    root
}

/// Inspector layout: vertical band list; graph toggled on/off.
pub fn render_eq_inspector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: EqRenderState,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    indexing: EqBandIndexing,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let graph_visible = entity.read(cx).app.plugin_state.plugin_ui_state.eq_compact_graph_visible;
    let config_open = entity.read(cx).app.plugin_state.plugin_ui_state.eq_compact_config_open;

    let mut root = div()
        .flex()
        .flex_col()
        .gap(d.section)
        .size_full();

    root = root.child(render_compact_global_bar(
        &d, entity.clone(), plugin_idx, &state, true, theme, cx,
    ));

    if config_open {
        root = root.child(render_compact_config_panel(
            &d, entity.clone(), plugin_idx, &state, theme, cx,
        ));
    }

    if graph_visible {
        root = root.child(
            div()
                .h(px(COMPACT_GRAPH_HEIGHT))
                .child(render_eq_visualization(
                    entity.clone(),
                    plugin_idx,
                    display_filters,
                    Some(selected_band_idx),
                    indexing,
                    theme,
                    state.available_width,
                )),
        );
        if let Some(filter) = display_filters.get(selected_band_idx) {
            root = root.child(render_compact_band_editor(
                &d,
                entity.clone(),
                plugin_idx,
                selected_band_idx,
                filter,
                indexing,
                &state,
                theme,
            ));
        }
    } else {
        let mut list = div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .overflow_y_scroll()
            .px(d.pad_x);
        for (i, filter) in display_filters.iter().enumerate() {
            list = list.child(render_compact_inspector_row(
                &d,
                entity.clone(),
                plugin_idx,
                i,
                filter,
                indexing,
                &state,
                theme,
            ));
        }
        list = list.child(render_add_band_button(&d, entity.clone(), plugin_idx, theme));
        root = root.child(list);
    }

    root
}

/// Slim top bar shared by both compact layouts.
fn render_compact_global_bar(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    show_graph_toggle: bool,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let config_open = entity.read(cx).app.plugin_state.plugin_ui_state.eq_compact_config_open;
    let graph_visible = entity.read(cx).app.plugin_state.plugin_ui_state.eq_compact_graph_visible;

    let mode_label = match state.mode {
        EqViewMode::Standard => "EQ",
        EqViewMode::LinearPhase { .. } => "Linear-Phase EQ",
        EqViewMode::FirDesigner { .. } => "FIR Designer",
    };

    let mut bar = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(d.gap)
        .px(d.pad_x)
        .py(d.pad_y_half)
        .bg(theme.surface)
        .rounded(d.r_md)
        .child(
            div()
                .flex()
                .items_center()
                .gap(d.gap)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .child(mode_label),
                )
                .child(config_toggle_button(d, entity.clone(), plugin_idx, config_open, theme)),
        );

    if show_graph_toggle {
        let graph_entity = entity.clone();
        let label = if graph_visible { "Graph ■" } else { "Graph □" };
        bar = bar.child(
            div()
                .px(d.pad_y)
                .py(d.pad_y_half)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .rounded(d.r_sm)
                .cursor_pointer()
                .when(graph_visible, |d| d.bg(theme.accent).text_color(theme.text_on_accent))
                .when(!graph_visible, |d| d.bg(theme.background_secondary).text_color(theme.text_secondary).hover(|s| s.bg(theme.surface_hover)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    graph_entity.update(cx, |state, _| {
                        let visible = &mut state.app.plugin_state.plugin_ui_state.eq_compact_graph_visible;
                        *visible = !*visible;
                    });
                })
                .child(label),
        );
    }

    bar
}

/// Expandable panel containing global controls and channel mode.
fn render_compact_config_panel(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let is_lp_mode = matches!(
        state.mode,
        EqViewMode::LinearPhase { .. } | EqViewMode::FirDesigner { .. }
    );

    let mut col = div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md);

    // Channel mode (standard EQ only)
    if !is_lp_mode {
        let all_entity = entity.clone();
        let per_entity = entity.clone();
        let per_channel = state.per_channel_mode;
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(d.grid)
                .child(mode_pill(d, "All Channels", !per_channel, theme, move |_, _, cx| {
                    all_entity.update(cx, |state, cx| {
                        state.app.set_eq_per_channel_mode(plugin_idx, false);
                        cx.notify();
                    });
                }))
                .child(mode_pill(d, "Per Channel", per_channel, theme, move |_, _, cx| {
                    per_entity.update(cx, |state, cx| {
                        state.app.set_eq_per_channel_mode(plugin_idx, true);
                        cx.notify();
                    });
                })),
        );

        if state.per_channel_mode {
            let selected_channel = entity.read(cx).app.plugin_state.selected_eq_channel;
            let channel_entity = entity.clone();
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .children((0..state.channels).map(|ch| {
                        let entity = channel_entity.clone();
                        let is_selected = ch == selected_channel;
                        mode_pill(d, channel_label(ch, state.channels), is_selected, theme, move |_, _, cx| {
                            entity.update(cx, |state, _| {
                                state.app.plugin_state.selected_eq_channel = ch;
                            });
                        })
                    })),
            );
        }
    }

    // Global controls based on EQ variant
    match &state.mode {
        EqViewMode::Standard => {
            col = col
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .child(render_eq_global_stepper(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::StandardMaxFilters,
                            "Filters", state.num_filters.to_string(), theme,
                        ))
                        .child(render_eq_global_toggle(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::StandardTopology,
                            "Topology", state.topology > 0.5, "SVF", "Biquad", theme,
                        ))
                        .child(render_eq_global_toggle(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::StandardTdf2,
                            "TDF-II", state.tdf2, "On", "Off", theme,
                        )),
                );
        }
        EqViewMode::LinearPhase {
            latency_samples,
            latency_ms,
            fir_length,
            auto_gain,
            mix,
            ..
        } => {
            col = col
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .child(render_eq_global_stepper(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::LpNumFilters,
                            "Filters", state.num_filters.to_string(), theme,
                        ))
                        .child(render_eq_global_stepper(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::LpFirLength,
                            "FIR length", fir_length.to_string(), theme,
                        ))
                        .child(render_eq_global_toggle(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::LpAutoGain,
                            "Auto-gain", *auto_gain, "On", "Off", theme,
                        ))
                        .child(render_eq_global_stepper(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::LpMix,
                            "Mix", format!("{:.0}%", mix * 100.0), theme,
                        )),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!("Latency: {latency_samples} samples ({latency_ms:.2} ms)")),
                );
        }
        EqViewMode::FirDesigner {
            latency_samples,
            latency_ms,
            fir_length,
            phase_mode,
            auto_gain,
            mix,
            ..
        } => {
            col = col
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .child(render_eq_global_stepper(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::FirNumFilters,
                            "Filters", state.num_filters.to_string(), theme,
                        ))
                        .child(render_eq_global_stepper(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::FirLength,
                            "FIR length", fir_length.to_string(), theme,
                        ))
                        .child(render_eq_global_toggle(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::FirPhaseMode,
                            "Phase", *phase_mode == "Minimum", "Minimum", "Linear", theme,
                        ))
                        .child(render_eq_global_toggle(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::FirAutoGain,
                            "Auto-gain", *auto_gain, "On", "Off", theme,
                        ))
                        .child(render_eq_global_stepper(
                            d, entity.clone(), plugin_idx,
                            EqGlobalControl::FirMix,
                            "Mix", format!("{:.0}%", mix * 100.0), theme,
                        )),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!("Latency: {latency_samples} samples ({latency_ms:.2} ms)")),
                );
        }
    }

    col
}

/// Compact clickable card for one band in the bottom strip.
fn render_compact_band_card(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    selected: bool,
    theme: &Theme,
) -> impl IntoElement {
    let entity_clone = entity.clone();
    let is_muted = filter.muted;
    div()
        .id(("eq-band-card", band_idx))
        .flex()
        .flex_col()
        .items_center()
        .gap(d.grid)
        .px(d.pad_x)
        .py(d.pad_y)
        .min_w(px(80.0))
        .rounded(d.r_md)
        .cursor_pointer()
        .when(selected, |div| {
            div.bg(theme.accent)
                .text_color(theme.text_on_accent)
                .font_weight(FontWeight::SEMIBOLD)
        })
        .when(!selected, |div| {
            div.bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
        })
        .when(is_muted, |div| div.opacity(0.5))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity_clone.update(cx, |state, _| {
                state.app.plugin_state.selected_eq_band = band_idx;
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
            });
        })
        .child(div().child(format!("#{} {}", band_idx + 1, filter.filter_type.short_name())))
        .child(
            div()
                .text_size(d.text_xs)
                .child(format!("{:.0}Hz", filter.frequency)),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .child(format!("{:+.1}dB", filter.gain_db)),
        )
}

/// Full inline editor for a single band (used by bottom strip and graph overlay).
fn render_compact_band_editor(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    indexing: EqBandIndexing,
    state: &EqRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let base_param_idx = band_idx * indexing.stride;
    let midi_overlay = state.midi_overlay;

    let mut editor = div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!("Band {}", band_idx + 1)),
                )
                .child(render_filter_type_selector(
                    d,
                    entity.clone(),
                    plugin_idx,
                    &filter.filter_type,
                    band_idx,
                    base_param_idx + indexing.filter_type,
                    None,
                    theme,
                )),
        )
        .child(
            div()
                .flex()
                .gap(d.gap)
                .justify_center()
                .child(render_eq_knob_with_midi(
                    d, entity.clone(), plugin_idx, "Freq",
                    filter.frequency,
                    pk(EQ, "freq").min_f64(), pk(EQ, "freq").max_f64(), "Hz",
                    base_param_idx + indexing.frequency,
                    state.selected_param, state.is_editing, midi_overlay, theme,
                ))
                .child(render_eq_knob_with_midi(
                    d, entity.clone(), plugin_idx, "Q",
                    filter.q,
                    pk(EQ, "q").min_f64(), pk(EQ, "q").max_f64(), "",
                    base_param_idx + indexing.q,
                    state.selected_param, state.is_editing, midi_overlay, theme,
                ))
                .child(render_eq_knob_with_midi(
                    d, entity.clone(), plugin_idx, "Gain",
                    filter.gain_db,
                    pk(EQ, "gain").min_f64(), pk(EQ, "gain").max_f64(), "dB",
                    base_param_idx + indexing.gain,
                    state.selected_param, state.is_editing, midi_overlay, theme,
                )),
        );

    // Mute / Solo buttons
    let mute_entity = entity.clone();
    let solo_entity = entity.clone();
    editor = editor.child(
        div()
            .flex()
            .gap(d.gap)
            .justify_center()
            .child(small_action_button(
                d, "M", filter.muted, theme.error, theme,
                move |_, _, cx| {
                    mute_entity.update(cx, |state, cx| {
                        state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                        if let Err(e) = state.app.toggle_eq_band_mute(band_idx) {
                            log::warn!("Failed to toggle EQ band mute: {}", e);
                        }
                        cx.notify();
                    });
                },
            ))
            .child(small_action_button(
                d, "S", filter.solo, theme.success, theme,
                move |_, _, cx| {
                    solo_entity.update(cx, |state, cx| {
                        state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                        if let Err(e) = state.app.toggle_eq_band_solo(band_idx) {
                            log::warn!("Failed to toggle EQ band solo: {}", e);
                        }
                        cx.notify();
                    });
                },
            )),
    );

    if let Some(active_local_idx) = indexing.active {
        editor = editor.child(render_eq_active_toggle(
            d,
            entity,
            plugin_idx,
            filter,
            base_param_idx + active_local_idx,
            state.selected_param,
            state.is_editing,
            theme,
        ));
    }

    editor
}

/// One self-contained row in the inspector list (card + inline editor).
fn render_compact_inspector_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    indexing: EqBandIndexing,
    state: &EqRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md)
        .child(render_compact_band_card(
            d, entity.clone(), plugin_idx, band_idx, filter, false, theme,
        ))
        .child(render_compact_band_editor(
            d, entity, plugin_idx, band_idx, filter, indexing, state, theme,
        ))
}

/// "+" button to add a band.
fn render_add_band_button(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .id("eq-add-band")
        .px(d.pad_x)
        .py_1p5()
        .text_size(d.text_sm)
        .font_weight(FontWeight::BOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .bg(theme.success)
        .text_color(theme.text_on_accent)
        .hover(|s| s.opacity(0.8))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |state, cx| {
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                if let Err(e) = state.app.add_eq_band() {
                    log::warn!("Failed to add EQ band: {}", e);
                }
                cx.notify();
            });
        })
        .child("+")
}

/// Small toggle pill used for channel mode and config toggles.
fn mode_pill<F>(
    d: &Ds,
    label: &str,
    selected: bool,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(MouseDownEvent, &mut Window, &mut Context<PlayerView>) + 'static,
{
    div()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::SEMIBOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(selected, |div| {
            div.bg(theme.accent)
                .text_color(theme.text_on_accent)
        })
        .when(!selected, |div| {
            div.bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label.to_string())
}

/// Config toggle button in the global bar.
fn config_toggle_button(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    open: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::SEMIBOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(open, |div| div.bg(theme.accent).text_color(theme.text_on_accent))
        .when(!open, |div| div.bg(theme.background_secondary).text_color(theme.text_secondary).hover(|s| s.bg(theme.surface_hover)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |state, _| {
                let open = &mut state.app.plugin_state.plugin_ui_state.eq_compact_config_open;
                *open = !*open;
            });
        })
        .child("Config ⚙")
}

/// Small circular M/S action button.
fn small_action_button<F>(
    d: &Ds,
    label: &'static str,
    active: bool,
    active_color: Rgba,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(MouseDownEvent, &mut Window, &mut Context<PlayerView>) + 'static,
{
    div()
        .w(px(28.0))
        .h(px(24.0))
        .rounded(d.r_sm)
        .flex()
        .items_center()
        .justify_center()
        .bg(if active { active_color } else { theme.background_secondary })
        .border(px(1.0))
        .border_color(if active { active_color } else { theme.border })
        .text_size(d.text_xs)
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .text_color(if active {
            theme.text_on_accent
        } else {
            theme.text_muted
        })
        .hover(|s| s.bg(if active { active_color } else { theme.surface_hover }))
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
}

/// Helper: readable label for a channel index.
fn channel_label(ch: usize, channels: usize) -> String {
    match channels {
        1 => "Mono".to_string(),
        2 => match ch {
            0 => "L".to_string(),
            1 => "R".to_string(),
            _ => format!("Ch{}", ch + 1),
        },
        _ => format!("Ch{}", ch + 1),
    }
}
```

- [ ] **Step 3: Check compilation**

Run:

```bash
cargo check -p sotf-gpui
```

Expected: may show missing `Rgba` import or unused `EqCompactLayout`. Fix any real errors; `EqCompactLayout` will be used in Task 7.

- [ ] **Step 4: Commit**

```bash
git add crates/app-gpui/components/plugins/ui_eq/layout_compact.rs crates/app-gpui/components/plugins/ui_eq.rs
git commit -m "feat(eq): add compact layout helpers and B/C entry points"
```

---

## Task 6: Wire layout branch in `render_eq_plugin`

**Files:**
- Modify: `crates/app-gpui/components/plugins/ui_eq/render.rs`

- [ ] **Step 1: Import the layout selector**

At the top of `crates/app-gpui/components/plugins/ui_eq/render.rs`, update the `super::types` import to include `EqCompactLayout`:

```rust
use super::types::{EqCompactLayout, EqRenderState, EqViewMode};
```

- [ ] **Step 2: Compute layout early**

After the `let indexing = ...;` line in `render_eq_plugin`, add:

```rust
let layout = EqCompactLayout::from_width(state.available_width);
```

- [ ] **Step 3: Avoid building the current controls section when not needed**

The existing code builds `controls_section` unconditionally. Wrap that whole `let controls_section = div()...` block (ending with the semicolon before `// Combine sections based on layout mode`) so it only builds in `Current`:

```rust
let controls_section = if layout == EqCompactLayout::Current {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(ds.section)
        .w_full()
        // ... existing controls_section contents unchanged ...
} else {
    div().into_any_element()
};
```

- [ ] **Step 4: Replace the final assembly block with a match**

Replace this final block:

```rust
// Combine sections based on layout mode

div()
    .flex()
    .flex_col()
    .items_center()
    .gap(ds.section_xl)
    .children(eq_header)
    .children(lp_header)
    .child(graph_section)
    .children(lp_analysis)
    .child(controls_section)
```

with:

```rust
match layout {
    EqCompactLayout::Current => div()
        .flex()
        .flex_col()
        .items_center()
        .gap(ds.section_xl)
        .children(eq_header)
        .children(lp_header)
        .child(graph_section)
        .children(lp_analysis)
        .child(controls_section)
        .into_any_element(),
    EqCompactLayout::BottomStrip => super::layout_compact::render_eq_bottom_strip(
        entity,
        plugin_idx,
        state,
        display_filters,
        selected_band_idx,
        indexing,
        theme,
        cx,
    )
    .into_any_element(),
    EqCompactLayout::Inspector => super::layout_compact::render_eq_inspector(
        entity,
        plugin_idx,
        state,
        display_filters,
        selected_band_idx,
        indexing,
        theme,
        cx,
    )
    .into_any_element(),
}
```

All match arms return `AnyElement` so the match type is uniform.

- [ ] **Step 5: Check compilation**

Run:

```bash
cargo check -p sotf-gpui
```

Expected: clean (warnings allowed).

- [ ] **Step 6: Run the unit test**

Run:

```bash
cargo test -p sotf-gpui layout_selection_breakpoints -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/app-gpui/components/plugins/ui_eq/render.rs
git commit -m "feat(eq): wire compact layout branch in render_eq_plugin"
```

---

## Task 7: Final verification and QA

- [ ] **Step 1: Check the whole crate**

Run:

```bash
cargo check -p sotf-gpui
```

Expected: clean (warnings allowed).

- [ ] **Step 2: Run clippy**

Run:

```bash
cargo clippy -p sotf-gpui -- -D warnings
```

Expected: clean. Fix any new warnings.

- [ ] **Step 3: Run relevant tests**

Run:

```bash
cargo test -p sotf-gpui layout_selection_breakpoints -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Manual verification checklist**

Build and run `sotf-desktop`, then:

1. Add an EQ plugin.
2. Resize the window to `> 900 px` — confirm current layout appears.
3. Resize to `750 px` — confirm Layout B (graph + band strip).
4. Click band cards in Layout B — inline editor appears.
5. Edit Freq/Q/Gain and verify the graph updates.
6. Click `[Config ⚙]` in Layout B — global controls appear.
7. Toggle channel mode inside config panel.
8. Resize to `< 600 px` — confirm Layout C (vertical list, no graph).
9. Click `[Graph □]` in Layout C — graph + selected editor overlay appears.
10. Add a band with the `+` button in both layouts.
11. Mute/Solo a band and confirm graph reflects it.
12. Switch to Linear-Phase EQ / FIR Designer and repeat steps 3–9.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(eq): compact B/C layouts for small windows"
```

---

## Spec self-review checklist

| Spec requirement | Implementing task |
|------------------|-------------------|
| Width-based layout selector (Current / B / C) | Task 1, Task 6 |
| Pass `available_width` into renderer | Task 2 |
| Layout B ASCII: graph top, band strip, inline editor | Task 5 |
| Layout C ASCII: inspector list, optional graph toggle | Task 5 |
| Global controls editable via config panel | Task 5 |
| Mute/Solo/Active buttons functional | Task 5 |
| Add-band button present | Task 5 |
| Channel mode in config panel | Task 5 |
| MIDI badges preserved (reuse `render_eq_knob_with_midi`) | Task 5 |
| Keep current layout for large windows | Task 6 |
| Unit test for breakpoints | Task 1 |
| `cargo check` / `cargo clippy` clean | Task 7 |

**Placeholder scan:** No TBD/TODO/fill-in sections. Every function body is provided.

**Type consistency:** `EqCompactLayout::from_width` used in Tasks 1, 5, 6. `available_width` field added once in Task 1 and populated in Task 2. State flags added in Task 3 and used in Task 5.
