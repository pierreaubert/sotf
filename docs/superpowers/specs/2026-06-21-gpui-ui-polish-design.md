# GPUI UI Polish — Design Spec

**Date:** 2026-06-21  
**Scope:** `crates/app-gpui` player UI  
**Status:** Approved

---

## 1. Goal

Fix a collection of layout, icon, and interaction issues in the GPUI music-player UI:

1. Left-menu icons (brain for Room EQ, cog/wheel for collapsed Devices and expanded Preferences).
2. Footer responsive layout in open and collapsed modes.
3. Home tab album-per-row parity with the Search/Library tab.
4. Queue accordion uniform width, decoupled expand/play, and resizable title truncation.
5. Compact wizard navigation at narrow window widths for all four wizards.
6. Spinorama EQ network-error handling with retries and user-facing messages.

---

## 2. Approach

**Chosen approach: shared helpers, local wrappers (Option B).**

We will keep the existing component structure and add small, reusable helpers inside `crates/app-gpui`. This avoids copy-pasting responsive logic across four wizard headers and the footer, while keeping the diff focused and reviewable. We will **not** modify the external `gpui-ui-kit` dependency.

---

## 3. Affected files

| Area | Files |
|---|---|
| Icons | `crates/app-gpui/components/icons/mod.rs`  
| | `crates/app-gpui/main/assets/icons/brain.svg` (new)  
| | `crates/app-gpui/main/assets/icons/cog.svg` (new) |
| Left sidebar | `crates/app-gpui/ui/render.rs` |
| Footer | `crates/app-gpui/components/home/footer/consts.rs` |
| Home grid | `crates/app-gpui/components/home/home_screen/misc.rs`, `crates/app-gpui/ui/consts.rs` |
| Queue accordion | `crates/app-gpui/components/home/queue/misc.rs` |
| Wizard headers | `crates/app-gpui/components/room_eq/mod.rs`  
| | `crates/app-gpui/components/headphone_eq/mod.rs`  
| | `crates/app-gpui/components/spinorama_eq/types.rs`  
| | `crates/app-gpui/components/recording/mod.rs` |
| Spinorama network | `crates/app-gpui/components/spinorama_eq/types.rs`  
| | `crates/app-gpui/components/spinorama_eq/step_1_select/misc.rs` |

---

## 4. Detailed design

### 4.1 Left menu icons

- Add `Brain` and `Cog` variants to `IconName` in `components/icons/mod.rs`, with asset paths `icons/brain.svg` and `icons/cog.svg`.
- Add matching SVG files to `crates/app-gpui/main/assets/icons/`.
- In `ui/render.rs`:
  - Room EQ item: use `IconName::Brain` instead of `IconName::AudioWaveform`.
  - Devices item: use `IconName::Cog` when `collapsed == true`; keep `IconName::Speaker` when expanded.
  - Preferences row (expanded): prepend `IconName::Cog` before the "Preferences" text label.

### 4.2 Footer

#### Open mode

`render_footer_track_info` currently hard-codes `max_w(rems(15.625))`. This can still push the collapse button off-screen for very long titles on small windows.

- Compute a responsive maximum width for the track-info block based on `window_width_rems`.
- Keep `overflow_hidden().text_ellipsis().whitespace_nowrap()` on title/album/artist text so the block shrinks gracefully.
- Ensure the right-side collapse chevron (`render_footer_right`) always remains inside the viewport.

#### Collapsed mode

When the window is wide enough, show a small waveform/level visualization centered between the title and the transport:

- Add a new compact waveform element (or reuse `WaveformElement`) to `render_footer_collapsed`.
- Place it after the title with `flex_1()` and a reasonable `max_w`, so it sits visually halfway between the title and the transport.
- Hide the waveform below a width breakpoint to preserve the existing narrow layout.

### 4.3 Home tab album count

`expanded_album_limit_for_dimensions` in `home_screen/misc.rs` uses a horizontal reserve of `192 px`, while the library/search grid uses `estimate_grid_dimensions` with only `2 × effective_rem` of reserve. This causes column-count mismatch.

- Align the Home expanded-shelf calculation with `estimate_grid_dimensions` from `crate::ui::consts`.
- Alternatively, update `HOME_SHELF_CONTENT_RESERVE_PX` to match the library reserve. The result must yield the same number of albums per row as the Search tab for the same window size.

### 4.4 Queue accordion

In `components/home/queue/misc.rs`:

1. **Uniform accordion width.** Ensure the title label container inside each accordion header fills the full width of the pane and truncates with `text_ellipsis()`. All accordion headers must have identical width regardless of title length.
2. **Decouple expand from play.** The `Accordion::on_change` handler must only update the expanded UI state. Move playback selection to a dedicated click target inside the accordion body (e.g. a play icon on the album header or clicking a track row). Expanding/collapsing an album must not change what is currently playing.
3. **Resizable title on divider drag.** When the meters divider is dragged, the accordion pane width changes. The album-title label must use `min_w_0()` and `flex_1()` so it shrinks and continues to truncate instead of clipping the divider.

### 4.5 Wizard headers (all wizards)

`gpui-ui-kit`’s `WizardHeader` auto-switches to `CurrentIcon` below 560 px, showing only the current step. We need a custom compact indicator so first/last step numbers/labels plus Close/Next remain visible at narrow widths.

- Add a helper function (e.g. `render_responsive_wizard_header`) or inline logic in each of the four wizard header renderers.
- Above a compact threshold: use the existing `WizardHeader`.
- Below the threshold: render a compact indicator such as:
  - `1 Load data … 7 x >` (first step label + ellipsis + last step number + close/next icons)
  - `1 … 7 x >` (even narrower)
- Keep the existing Close/Next buttons always visible on the right.

Apply this to:

- `render_room_eq_header`
- `render_headphone_eq_header`
- `render_spinorama_header`
- `render_recording_header`

### 4.6 Spinorama EQ network errors

In `components/spinorama_eq/types.rs`:

1. **Retry logic.** Modify `fetch_spinorama_speakers`, `fetch_spinorama_versions`, and `fetch_spinorama_measurements` to retry failed requests up to 3 times total, with a 2-second pause between attempts.
2. **User-facing messages.** After 3 failures, classify the error:
   - Network-level errors (timeout, DNS failure, connection refused) → "No network access. Please check your connection."
   - HTTP non-2xx or spinorama.org-specific failures → "spinorama.org is unavailable. Please try again later."
3. **Inline banner layout.** In `step_1_select/misc.rs`, constrain the error banner width to the visible screen and wrap/truncate the text so it never overflows its container. Keep `text_ellipsis()` and a max width derived from `app_width`.

---

## 5. Testing

- `cargo check --workspace` and `cargo clippy --workspace` after every change.
- `cargo test --workspace` before marking the task complete.
- Manual/visual verification of:
  - Sidebar icons in collapsed and expanded states.
  - Footer open mode with a very long track title at half-screen laptop width.
  - Footer collapsed mode at large window width.
  - Home vs. Search album-per-row count for identical window sizes.
  - Queue accordion width uniformity and divider resize behavior.
  - Wizard compact header at narrow widths for Recording, Room EQ, Headphone EQ, and Spinorama EQ.
  - Spinorama offline/unavailable error path.

---

## 6. Out of scope

- No changes to the external `gpui-ui-kit` crate.
- No new business logic or DSP changes.
- No unrelated refactoring of wizard state machines or audio engine code.
