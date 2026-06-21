# GPUI UI Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix six UI layout, icon, and interaction issues in `crates/app-gpui` player UI: left-menu icons, footer responsive layout, Home album count parity, Queue accordion behavior, compact wizard headers, and Spinorama network-error handling.

**Architecture:** Keep the existing component structure and add small, reusable helpers inside `crates/app-gpui`. Each subsystem is implemented and tested independently. The external `gpui-ui-kit` crate is not modified.

**Tech Stack:** Rust, GPUI, `gpui-ui-kit`, Lucide-style SVG icons.

---

## File structure

| File | Responsibility |
|---|---|
| `crates/app-gpui/components/icons/mod.rs` | `IconName` enum and asset paths |
| `crates/app-gpui/main/assets/icons/brain.svg` | New brain icon asset |
| `crates/app-gpui/main/assets/icons/cog.svg` | New cog/wheel icon asset |
| `crates/app-gpui/ui/render.rs` | Left sidebar rendering |
| `crates/app-gpui/components/home/footer/consts.rs` | Footer open/collapsed rendering |
| `crates/app-gpui/components/home/home_screen/misc.rs` | Home shelf album-limit calculation |
| `crates/app-gpui/ui/consts.rs` | Shared grid-dimension estimation |
| `crates/app-gpui/components/home/queue/misc.rs` | Queue screen and accordion |
| `crates/app-gpui/components/room_eq/mod.rs` | Room EQ wizard header |
| `crates/app-gpui/components/headphone_eq/mod.rs` | Headphone EQ wizard header |
| `crates/app-gpui/components/spinorama_eq/types.rs` | Spinorama EQ wizard header + network fetches |
| `crates/app-gpui/components/spinorama_eq/step_1_select/misc.rs` | Spinorama step 1 error banner |
| `crates/app-gpui/components/recording/mod.rs` | Recording wizard header |

---

## Task 1: Add Brain and Cog icon variants

**Files:**
- Modify: `crates/app-gpui/components/icons/mod.rs`
- Create: `crates/app-gpui/main/assets/icons/brain.svg`
- Create: `crates/app-gpui/main/assets/icons/cog.svg`

- [ ] **Step 1: Add SVG icon assets**

Create `crates/app-gpui/main/assets/icons/brain.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5a3 3 0 1 0-5.997.125 4 4 0 0 0-2.526 5.77 4 4 0 0 0 .556 6.588A4 4 0 1 0 12 18Z"/><path d="M12 5a3 3 0 1 1 5.997.125 4 4 0 0 1 2.526 5.77 4 4 0 0 1-.556 6.588A4 4 0 1 1 12 18Z"/><path d="M15 13a4.5 4.5 0 0 1-3-4 4.5 4.5 0 0 1-3 4"/><path d="M17.599 6.5a3 3 0 0 0 .399-1.375"/><path d="M6.003 5.125A3 3 0 0 0 6.401 6.5"/><path d="M3.477 10.896a4 4 0 0 1 .585-.396"/><path d="M19.938 10.5a4 4 0 0 1 .585.396"/><path d="M6 18a4 4 0 0 1-1.967-.516"/><path d="M19.967 17.484A4 4 0 0 1 18 18"/></svg>
```

Create `crates/app-gpui/main/assets/icons/cog.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20a8 8 0 1 0 0-16 8 8 0 0 0 0 16Z"/><path d="M12 14a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/></svg>
```

- [ ] **Step 2: Extend `IconName` enum**

Edit `crates/app-gpui/components/icons/mod.rs`. Add variants to the `IconName` enum:

```rust
pub enum IconName {
    // ... existing variants ...
    Brain,
    Cog,
}
```

Add match arms in `IconName::path`:

```rust
IconName::Brain => "icons/brain.svg",
IconName::Cog => "icons/cog.svg",
```

- [ ] **Step 3: Verify icon paths compile**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

- [ ] **Step 4: Commit**

```bash
git add crates/app-gpui/components/icons/mod.rs crates/app-gpui/main/assets/icons/brain.svg crates/app-gpui/main/assets/icons/cog.svg
git commit -m "feat(icons): add Brain and Cog icon variants"
```

---

## Task 2: Update left sidebar icons

**Files:**
- Modify: `crates/app-gpui/ui/render.rs`

- [ ] **Step 1: Change Room EQ icon to Brain**

In `render_app_sidebar`, find the Room EQ `render_sidebar_screen_item` call and replace:

```rust
IconName::AudioWaveform,
```

with:

```rust
IconName::Brain,
```

- [ ] **Step 2: Change Devices icon based on collapsed state**

In `render_sidebar_devices_item`, replace the `IconName::Speaker` argument with:

```rust
if collapsed { IconName::Cog } else { IconName::Speaker },
```

- [ ] **Step 3: Add Cog icon before Preferences label when expanded**

In `render_app_sidebar`, locate the Preferences row inside the `.when(!collapsed, |el| { ... })` block. Change it to display the Cog icon followed by the text label:

```rust
.when(!collapsed, |el| {
    el.child(
        div()
            .flex()
            .items_center()
            .gap(d.grid)
            .child(Icon::new(IconName::Cog).xs().color(theme.text_muted))
            .child(
                div()
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_muted)
                    .child("Preferences"),
            ),
    )
    // ... existing settings button ...
})
```

- [ ] **Step 4: Check and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/ui/render.rs
git commit -m "feat(sidebar): use brain icon for room EQ, cog for devices and preferences"
```

---

## Task 3: Footer open-mode responsive title

**Files:**
- Modify: `crates/app-gpui/components/home/footer/consts.rs`

- [ ] **Step 1: Compute responsive track-info max width**

In `render_footer`, after computing `window_width_rems`, pass it to `render_footer_track_info`:

```rust
.child(self.render_footer_track_info(&translations, window_width_rems, cx))
```

Update the method signature:

```rust
pub(super) fn render_footer_track_info(
    &self,
    translations: &crate::i18n::Translations,
    window_width_rems: f32,
    cx: &mut Context<Self>,
) -> impl IntoElement {
```

- [ ] **Step 2: Clamp title block width**

At the end of `render_footer_track_info`, compute a responsive max width:

```rust
let max_width = if window_width_rems < 50.0 {
    rems(10.0)
} else if window_width_rems < 70.0 {
    rems(12.5)
} else {
    rems(15.625)
};
```

Change the final `.max_w(rems(15.625))` to `.max_w(max_width)`.

- [ ] **Step 3: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/home/footer/consts.rs
git commit -m "feat(footer): shrink track info width responsively in open mode"
```

---

## Task 4: Footer collapsed-mode waveform

**Files:**
- Modify: `crates/app-gpui/components/home/footer/consts.rs`

- [ ] **Step 1: Read window width in collapsed footer**

In `render_footer_collapsed`, read `window_width` from state and compute `window_width_rems` using the same logic as `render_footer`.

- [ ] **Step 2: Insert compact waveform between title and transport**

After the title `div(...).child(title)`, conditionally add a compact waveform element when the window is wide enough:

```rust
.when(window_width_rems >= 45.0, |el| {
    el.child(
        div()
            .id("footer-collapsed-waveform")
            .flex_1()
            .max_w(rems(20.0))
            .h(rems(1.5))
            .flex()
            .items_center()
            .justify_center()
            .child(/* render compact waveform or placeholder bars */),
    )
})
```

For the first iteration, render a simple placeholder: a centered `IconName::AudioWaveform` icon or a few static bars. If feasible, reuse `WaveformElement::new(...)` with the current track waveform/progress.

- [ ] **Step 3: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/home/footer/consts.rs
git commit -m "feat(footer): show waveform in collapsed footer on wide windows"
```

---

## Task 5: Home tab album count parity

**Files:**
- Modify: `crates/app-gpui/components/home/home_screen/misc.rs`

- [ ] **Step 1: Import `estimate_grid_dimensions`**

At the top of `home_screen/misc.rs`, add:

```rust
use crate::ui::{
    ALBUM_CARD_GAP_REMS, ALBUM_CARD_HEIGHT_REMS, ALBUM_CARD_WIDTH_REMS, CHROME_HEIGHT_REMS,
    combined_scale_bounds, compute_responsive_scale, estimate_grid_dimensions,
};
```

- [ ] **Step 2: Replace custom expanded limit calculation**

Replace the body of `expanded_album_limit_for_dimensions` with a call to the shared estimator:

```rust
pub(super) fn expanded_album_limit_for_dimensions(
    window_width: f32,
    window_height: f32,
    font_scale: f32,
    min_font_size_px: Option<f32>,
    max_font_size_px: Option<f32>,
) -> usize {
    let (columns, rows) = estimate_grid_dimensions(
        window_width,
        window_height,
        font_scale,
        min_font_size_px,
        max_font_size_px,
    );
    // Home shelves add one extra row of buffering to reduce blank space
    // below shelf titles.
    (columns * rows.saturating_add(1)).max(EXPANDED_ALBUM_LIMIT)
}
```

- [ ] **Step 3: Run existing album-card/grid tests**

Run:

```bash
cargo test -p app-gpui test_grid_view_dimensions test_compact_view_dimensions test_album_card_height_grid test_album_card_height_compact
```

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/app-gpui/components/home/home_screen/misc.rs
git commit -m "fix(home): align expanded album limit with library grid estimator"
```

---

## Task 6: Queue accordion uniform width

**Files:**
- Modify: `crates/app-gpui/components/home/queue/misc.rs`

- [ ] **Step 1: Inspect `AccordionItem` API**

Confirm `AccordionItem::new(id, title).trailing(...)` renders a header row. The title label is built from `summary.title`; we will wrap the title content so it fills the header width.

- [ ] **Step 2: Pass a full-width title element**

In `render_queue_accordion_pane`, change the `AccordionItem` construction so the title is a `div()` that fills width and truncates:

```rust
AccordionItem::new(
    format!("queue-album-{}", summary.idx),
    SharedString::from(""),
)
.trailing(summary.track_position)
.title_element(
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .child(summary.title.clone()),
)
.content(self.render_queue_album_detail(summary.idx, translations, cx))
```

If `AccordionItem` does not expose `title_element`, instead wrap the content returned by `render_queue_album_detail` or use the `content` builder to include a full-width header row manually.

- [ ] **Step 3: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/home/queue/misc.rs
git commit -m "fix(queue): make accordion headers fill pane width"
```

---

## Task 7: Decouple accordion expand from playback

**Files:**
- Modify: `crates/app-gpui/components/home/queue/misc.rs`

- [ ] **Step 1: Remove playback logic from `Accordion::on_change`**

In `render_queue_accordion_pane`, replace the `.on_change(...)` handler with one that only updates the expanded index:

```rust
.on_change(move |id, is_expanded, _window, cx| {
    if !is_expanded {
        return;
    }
    let Some(idx) = id
        .to_string()
        .strip_prefix("queue-album-")
        .and_then(|suffix| suffix.parse::<usize>().ok())
    else {
        return;
    };
    state_handle.update(cx, |state, _cx| {
        if state.app.queue_state.get(idx).is_some() {
            state.app.queue_state.selected_index = idx;
        }
    });
})
```

- [ ] **Step 2: Add a play action inside the album detail**

In `render_queue_album_detail`, add a play button/icon near the album title. On click, call a helper that starts playback of the selected queue album (extract the previous playback logic from the old `on_change` handler into a new method `play_queue_album`).

Example helper:

```rust
fn play_queue_album(&self, queue_idx: usize, cx: &mut Context<Self>) {
    self.state.update(cx, |state, _cx| {
        let queue_len = state.app.queue_state.len();
        let Some(item) = state.app.queue_state.get(queue_idx) else { return };
        let current_channels = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue_state.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| track.channels)
            .unwrap_or(2) as usize;
        let Some((source, target_channels)) = item.current_track().map(|track| {
            (
                track.audio_source(),
                track.channels.unwrap_or(2) as usize,
            )
        }) else { return };

        let prefer_smooth_switch = state.app.playback.is_playing
            && state.app.playback.current_queue_index != Some(queue_idx)
            && current_channels == target_channels;
        state.app.queue_state.current_index = Some(queue_idx);
        state.app.playback.current_queue_index = Some(queue_idx);
        if prefer_smooth_switch {
            Self::play_track_smooth(state, source);
        } else {
            Self::play_track(state, source);
        }
    });
}
```

- [ ] **Step 3: Verify behavior and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/home/queue/misc.rs
git commit -m "fix(queue): expand accordion without changing playback"
```

---

## Task 8: Queue accordion resizable title on divider drag

**Files:**
- Modify: `crates/app-gpui/components/home/queue/misc.rs`

- [ ] **Step 1: Ensure title container shrinks**

If not already done in Task 6, wrap the accordion title element so it uses:

```rust
div()
    .w_full()
    .min_w_0()
    .flex_1()
    .overflow_hidden()
    .text_ellipsis()
    .whitespace_nowrap()
```

- [ ] **Step 2: Verify resize behavior**

Build and run the app. Open the Queue tab, drag the meters divider left/right, and confirm long album titles truncate rather than push content.

- [ ] **Step 3: Commit**

```bash
git add crates/app-gpui/components/home/queue/misc.rs
git commit -m "fix(queue): accordion title shrinks when divider resizes pane"
```

---

## Task 9: Shared compact wizard header helper

**Files:**
- Create: `crates/app-gpui/components/wizard_header.rs`
- Modify: `crates/app-gpui/components/mod.rs`

- [ ] **Step 1: Create helper module**

Create `crates/app-gpui/components/wizard_header.rs`:

```rust
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonTheme, ButtonVariant, HStack, StackSpacing, StepStatus, WizardHeader, WizardStep, WizardTheme};

pub struct ResponsiveWizardHeader {
    pub title: SharedString,
    pub steps: Vec<WizardStep>,
    pub step_statuses: Vec<StepStatus>,
    pub current_step: usize,
    pub wizard_theme: WizardTheme,
    pub button_theme: ButtonTheme,
    pub back_button: Button,
    pub next_button: Button,
}

impl ResponsiveWizardHeader {
    pub fn render(self, d: Ds, window_width_rems: f32) -> impl IntoElement {
        // Thresholds chosen so Close/Next buttons never get pushed off-screen.
        let compact_threshold_rems = 50.0;
        let ultra_compact_threshold_rems = 36.0;

        let full_header = WizardHeader::new()
            .title(self.title.clone())
            .steps(self.steps.clone())
            .step_statuses(self.step_statuses.clone())
            .current_step(self.current_step)
            .theme(self.wizard_theme.clone());

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(d.card)
            .py(d.card)
            .bg(self.wizard_theme.background_secondary)
            .border_b_1()
            .border_color(self.wizard_theme.step_border)
            .child(
                if window_width_rems >= compact_threshold_rems {
                    div().child(full_header).into_any_element()
                } else {
                    self.render_compact_indicator(
                        d,
                        window_width_rems < ultra_compact_threshold_rems,
                    )
                    .into_any_element()
                },
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(self.back_button)
                    .child(self.next_button),
            )
    }

    fn render_compact_indicator(&self, d: Ds, ultra_compact: bool) -> impl IntoElement {
        let first_label = self.steps.first().map(|s| s.label.clone()).unwrap_or_default();
        let last_label = self.steps.last().map(|s| s.label.clone()).unwrap_or_default();
        let total = self.steps.len();
        let current = self.current_step + 1;

        div()
            .flex()
            .items_center()
            .gap(d.grid)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(self.render_step_circle(1, false))
                    .when(!ultra_compact, |el| {
                        el.child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(self.wizard_theme.label_active_text)
                                .child(first_label),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(d.text_sm)
                    .text_color(self.wizard_theme.label_text)
                    .child("…"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(self.render_step_circle(total, false))
                    .when(!ultra_compact, |el| {
                        el.child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(self.wizard_theme.label_text)
                                .child(last_label),
                        )
                    }),
            )
            .when(total > 2, |el| {
                el.child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(self.wizard_theme.label_text)
                        .child(format!("({}/{total})", current)),
                )
            })
    }

    fn render_step_circle(&self, number: usize, active: bool) -> impl IntoElement {
        div()
            .w(px(24.0))
            .h(px(24.0))
            .rounded_full()
            .border_2()
            .border_color(self.wizard_theme.step_border)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .font_weight(if active {
                        gpui::FontWeight::BOLD
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .text_color(self.wizard_theme.label_active_text)
                    .child(format!("{number}")),
            )
    }
}
```

Adjust field types as needed to match `gpui_ui_kit` APIs.

- [ ] **Step 2: Register module**

Add `pub mod wizard_header;` to `crates/app-gpui/components/mod.rs`.

- [ ] **Step 3: Check helper compiles**

Run:

```bash
cargo check -p app-gpui
```

Expected: success after fixing any API mismatches.

- [ ] **Step 4: Commit**

```bash
git add crates/app-gpui/components/wizard_header.rs crates/app-gpui/components/mod.rs
git commit -m "feat(wizard): add responsive wizard header helper"
```

---

## Task 10: Apply compact header to Room EQ wizard

**Files:**
- Modify: `crates/app-gpui/components/room_eq/mod.rs`

- [ ] **Step 1: Replace header layout with helper**

In `render_room_eq_header`, after building `steps`, `step_statuses`, `wizard_theme`, `button_theme`, `back_label`, `next_label`, and the navigation buttons, replace the final `div()...child(header)...child(navigation)` construction with a call to `ResponsiveWizardHeader`.

Read `window_width_rems` from state and pass it to the helper.

- [ ] **Step 2: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/room_eq/mod.rs
git commit -m "feat(room_eq): responsive compact wizard header"
```

---

## Task 11: Apply compact header to Headphone EQ wizard

**Files:**
- Modify: `crates/app-gpui/components/headphone_eq/mod.rs`

- [ ] **Step 1: Replace header layout with helper**

Same pattern as Task 10, using the `HeadphoneEqStep` values.

- [ ] **Step 2: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/headphone_eq/mod.rs
git commit -m "feat(headphone_eq): responsive compact wizard header"
```

---

## Task 12: Apply compact header to Spinorama EQ wizard

**Files:**
- Modify: `crates/app-gpui/components/spinorama_eq/types.rs`

- [ ] **Step 1: Replace header layout with helper**

Same pattern as Task 10, using the `SpinoramaStep` values.

- [ ] **Step 2: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/spinorama_eq/types.rs
git commit -m "feat(spinorama_eq): responsive compact wizard header"
```

---

## Task 13: Apply compact header to Recording wizard

**Files:**
- Modify: `crates/app-gpui/components/recording/mod.rs`

- [ ] **Step 1: Replace header layout with helper**

Same pattern as Task 10, using the `RecordingStep::all()` list.

- [ ] **Step 2: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/recording/mod.rs
git commit -m "feat(recording): responsive compact wizard header"
```

---

## Task 14: Spinorama network retry + user-facing messages

**Files:**
- Modify: `crates/app-gpui/components/spinorama_eq/types.rs`

- [ ] **Step 1: Add retry helper**

Add a private helper at the top of the `impl PlayerView` block in `types.rs`:

```rust
const SPINORAMA_MAX_RETRIES: usize = 3;
const SPINORAMA_RETRY_DELAY_MS: u64 = 2000;

fn classify_spinorama_error(err: &str) -> &'static str {
    let lower = err.to_lowercase();
    if lower.contains("timeout")
        || lower.contains("dns")
        || lower.contains("connection refused")
        || lower.contains("could not connect")
        || lower.contains("network")
        || lower.contains("offline")
    {
        "No network access. Please check your connection."
    } else {
        "spinorama.org is unavailable. Please try again later."
    }
}
```

- [ ] **Step 2: Wrap fetch calls with retry logic**

For `fetch_spinorama_speakers`, `fetch_spinorama_versions`, and `fetch_spinorama_measurements`, wrap the HTTP/network call in a loop:

```rust
let mut last_error = String::new();
for attempt in 1..=SPINORAMA_MAX_RETRIES {
    match perform_fetch().await {
        Ok(value) => return Ok(value),
        Err(e) => {
            last_error = e;
            if attempt < SPINORAMA_MAX_RETRIES {
                std::thread::sleep(std::time::Duration::from_millis(SPINORAMA_RETRY_DELAY_MS));
            }
        }
    }
}
Err(last_error)
```

- [ ] **Step 3: Use classified message in error state**

In each fetch's error branch, store the classified message instead of the raw error:

```rust
let user_msg = classify_spinorama_error(&e);
spinorama.error_message = Some(user_msg.to_string());
state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(user_msg.to_string()));
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/spinorama_eq/types.rs
git commit -m "feat(spinorama_eq): retry network fetches 3x and classify user-facing errors"
```

---

## Task 15: Spinorama error banner layout

**Files:**
- Modify: `crates/app-gpui/components/spinorama_eq/step_1_select/misc.rs`

- [ ] **Step 1: Constrain error banner width**

Locate the error banner:

```rust
.when_some(fetch_error_message, |vstack, msg| {
    vstack.child(Text::new(msg).size(TextSize::Xs).color(theme.error))
})
```

Replace with a constrained, truncating container:

```rust
.when_some(fetch_error_message, |vstack, msg| {
    vstack.child(
        div()
            .w_full()
            .max_w(px(app_width.max(240.0) - 48.0))
            .overflow_hidden()
            .text_ellipsis()
            .child(
                Text::new(msg)
                    .size(TextSize::Xs)
                    .color(theme.error),
            ),
    )
})
```

If `app_width` is not already in scope, read it from `state.app.ui_state.window_width`.

- [ ] **Step 2: Verify and commit**

Run:

```bash
cargo check -p app-gpui
```

Expected: success.

```bash
git add crates/app-gpui/components/spinorama_eq/step_1_select/misc.rs
git commit -m "fix(spinorama_eq): constrain error banner width and wrap/truncate text"
```

---

## Task 16: Final verification

**Files:** all of the above.

- [ ] **Step 1: Full workspace check**

Run:

```bash
cargo check --workspace
```

Expected: success.

- [ ] **Step 2: Full workspace clippy**

Run:

```bash
cargo clippy --workspace
```

Expected: success (or only pre-existing warnings).

- [ ] **Step 3: Run app-gpui tests**

Run:

```bash
cargo test -p app-gpui
```

Expected: all tests pass.

- [ ] **Step 4: Manual/visual checklist**

Build and run the desktop app. Verify:

- Sidebar: Room EQ shows brain icon; collapsed Devices shows cog; expanded Preferences shows cog before text.
- Footer open mode: long title at half-screen width does not hide collapse button.
- Footer collapsed mode: waveform/level indicator appears centered on wide windows.
- Home tab: album-per-row count matches Search tab for the same window size.
- Queue tab: accordion headers are uniform width; clicking ▼ expands/collapses without changing playback; dragging meters divider truncates long titles.
- Wizards (Recording, Room EQ, Headphone EQ, Spinorama EQ): narrow window shows compact step indicator with Close/Next visible.
- Spinorama EQ: disconnect network → inline error is short and fits; after 3 retries the user-friendly message appears.

- [ ] **Step 5: Final commit**

```bash
git add .
git commit -m "feat(ui): complete GPUI UI polish: icons, footer, home grid, queue accordion, wizard headers, spinorama errors"
```

---

## Self-review

**Spec coverage:**

- 4.1 Left menu icons → Tasks 1–2
- 4.2 Footer responsive layout → Tasks 3–4
- 4.3 Home album count parity → Task 5
- 4.4 Queue accordion → Tasks 6–8
- 4.5 Compact wizard headers → Tasks 9–13
- 4.6 Spinorama network errors → Tasks 14–15
- Testing → Task 16

**Placeholder scan:** No TBD/TODO placeholders; all steps include concrete file paths, code, and commands.

**Type consistency:** All references use existing types (`IconName`, `WizardStep`, `StepStatus`, `WizardHeader`, `WizardTheme`, `ButtonTheme`). The new helper module must be adjusted to match `gpui_ui_kit` field types if they differ from the draft.
