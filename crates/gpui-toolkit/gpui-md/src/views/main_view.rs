use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::markdown::SourceSpan;
use crate::state::MdAppState;
use crate::views::editor_pane::{EditorPane, LINE_HEIGHT_PX as EDITOR_LINE_HEIGHT_PX};
use crate::views::find_bar::FindBar;
use crate::views::minibuffer::MiniBufferView;
use crate::views::preview_pane::PreviewPane;
use crate::views::toolbar_view::ToolbarView;

/// Root view: toolbar + find bar + split pane (editor | preview) + status bar.
///
/// Owns scroll handles for both panes and synchronizes their vertical scroll
/// positions proportionally (aligned at the vertical midpoint).
pub struct MainView {
    state: Entity<MdAppState>,
    editor: Entity<EditorPane>,
    preview: Entity<PreviewPane>,
    toolbar: Entity<ToolbarView>,
    find_bar: Entity<FindBar>,
    minibuffer: Entity<MiniBufferView>,
    /// Last known editor scroll Y offset (for detecting which pane moved).
    last_editor_scroll_y: f32,
    /// Last known preview scroll Y offset.
    last_preview_scroll_y: f32,
}

impl MainView {
    pub fn new(state: Entity<MdAppState>, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| EditorPane::new(state.clone(), cx));
        let preview = cx.new(|cx| PreviewPane::new(state.clone(), cx));
        let toolbar = cx.new(|_cx| ToolbarView::new(state.clone()));
        let find_bar = cx.new(|_cx| FindBar::new(state.clone()));
        let minibuffer = cx.new(|_cx| MiniBufferView::new(state.clone()));

        Self {
            state,
            editor,
            preview,
            toolbar,
            find_bar,
            minibuffer,
            last_editor_scroll_y: 0.0,
            last_preview_scroll_y: 0.0,
        }
    }

    /// Synchronise scroll positions between editor and preview using a
    /// **source-line anchor** via the preview's per-block source map.
    ///
    /// The old proportional approach (`editor_y/editor_max` → scaled to
    /// `preview_max`) drifts badly because the editor is uniform
    /// (20 px/line) while the preview is not: a heading block is tall,
    /// a paragraph is short, a long code block is very tall, and so on.
    /// So the same source line maps to different fractional positions
    /// in the two scroll ranges, and the gap grows as you scroll down.
    ///
    /// The accurate mapping uses, for each block `i`:
    /// - `PreviewPane::block_spans[i]`: the block's source line range
    /// - `preview_scroll.bounds_for_item(i)`: the block's rendered
    ///   pixel bounds (in document-space — GPUI stores child bounds as
    ///   if the scroll offset were 0, so they are stable across scroll
    ///   events).
    ///
    /// When the editor drives, we figure out which source line is at
    /// the top of the editor viewport, find the block that covers that
    /// line, compute a fractional position inside that block, look up
    /// the block's pixel bounds in the preview, and compute the scroll
    /// offset that places `block_top + fraction * block_height` at the
    /// preview viewport top.
    ///
    /// When the preview drives, we do the inverse: ask the preview
    /// which block is currently at its top, compute how many pixels
    /// into that block the viewport top is, convert to a fractional
    /// source line inside the block's span, and scroll the editor to
    /// `line * LINE_HEIGHT`.
    fn sync_scroll(&mut self, cx: &mut Context<Self>) {
        let editor_scroll = self.editor.read(cx).scroll_handle.clone();
        let (preview_scroll, block_spans) = {
            let preview = self.preview.read(cx);
            (preview.scroll_handle.clone(), preview.block_spans.clone())
        };

        let editor_y: f32 = editor_scroll.offset().y.into();
        let preview_y: f32 = preview_scroll.offset().y.into();

        let editor_changed = (editor_y - self.last_editor_scroll_y).abs() > 0.5;
        let preview_changed = (preview_y - self.last_preview_scroll_y).abs() > 0.5;

        // Nothing to do until we have at least one measured block.
        if block_spans.is_empty() {
            self.last_editor_scroll_y = editor_y;
            self.last_preview_scroll_y = preview_y;
            return;
        }

        let preview_viewport_top: f32 = preview_scroll.bounds().origin.y.into();

        if editor_changed && !preview_changed {
            // ---- Editor → Preview -----------------------------------------
            // Source line (1-indexed, matching comrak's source_map) at the
            // top of the editor viewport. Editor uses 0-indexed lines with
            // uniform `EDITOR_LINE_HEIGHT_PX`, so line_0 = (-editor_y / H).
            let editor_line_0: f32 = (-editor_y / EDITOR_LINE_HEIGHT_PX).max(0.0);
            let source_line_1 = editor_line_0 + 1.0;

            if let Some(target_y) = preview_y_for_source_line(
                source_line_1,
                &block_spans,
                &preview_scroll,
                preview_viewport_top,
            ) {
                preview_scroll
                    .set_offset(point(preview_scroll.offset().x, px(target_y)));
            }
        } else if preview_changed && !editor_changed {
            // ---- Preview → Editor -----------------------------------------
            // Ask GPUI which block is at the preview top, then compute
            // how deep we are inside it, convert to a source line, and
            // project onto the editor's uniform grid.
            let top_ix = preview_scroll.top_item();
            let Some(span) = block_spans.get(top_ix) else {
                self.last_editor_scroll_y = editor_y;
                self.last_preview_scroll_y = preview_y;
                return;
            };
            let Some(block_bounds) = preview_scroll.bounds_for_item(top_ix) else {
                self.last_editor_scroll_y = editor_y;
                self.last_preview_scroll_y = preview_y;
                return;
            };

            let block_top: f32 = block_bounds.origin.y.into();
            let block_height: f32 = block_bounds.size.height.into();

            // Distance, in pixels, from this block's top to the viewport
            // top, measured in document-space (i.e. including the current
            // scroll offset).
            let pixels_into_block = (preview_viewport_top - preview_y) - block_top;
            let block_fraction = if block_height > 0.5 {
                (pixels_into_block / block_height).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let span_lines = span.end_line.saturating_sub(span.start_line) as f32 + 1.0;
            let source_line_1 = span.start_line as f32 + block_fraction * span_lines;
            let editor_line_0 = (source_line_1 - 1.0).max(0.0);
            let target_editor_y = -(editor_line_0 * EDITOR_LINE_HEIGHT_PX);

            let clamped = clamp_negative_offset(
                target_editor_y,
                editor_scroll.max_offset().y.into(),
            );
            editor_scroll
                .set_offset(point(editor_scroll.offset().x, px(clamped)));
        }

        self.last_editor_scroll_y = editor_scroll.offset().y.into();
        self.last_preview_scroll_y = preview_scroll.offset().y.into();
    }
}

/// Find the preview scroll offset that places `source_line_1` (1-indexed
/// source line, matching comrak) at the top of the preview viewport.
///
/// Returns `None` when GPUI has not measured the target block yet
/// (typical on first render, before layout runs).
fn preview_y_for_source_line(
    source_line_1: f32,
    block_spans: &[SourceSpan],
    preview_scroll: &ScrollHandle,
    preview_viewport_top: f32,
) -> Option<f32> {
    // Find the first block whose end_line >= source_line. Blocks are
    // stored in document order so a linear scan is fine; the typical
    // document has tens to a few hundred blocks.
    let target_line = source_line_1.max(1.0);
    let target_line_usize = target_line.floor() as usize;

    // Clamp against the first / last block so scrolling to the first
    // source line snaps to the preview's natural top (keeps the
    // container padding visible) and scrolling past the last line
    // snaps to the bottom. The `<=` on the first bound is important:
    // if we computed `viewport_top - block_top` for line 1, we would
    // get `-container_padding`, which hides the padding above block 0
    // for the whole first block. Snapping to 0 here preserves it.
    let first = block_spans.first()?;
    if target_line_usize <= first.start_line {
        return Some(0.0);
    }
    let last = block_spans.last()?;
    if target_line_usize >= last.end_line {
        return Some(preview_scroll.max_offset().y.into());
    }

    let (block_ix, span) = block_spans
        .iter()
        .enumerate()
        .find(|(_, s)| target_line_usize <= s.end_line && target_line_usize >= s.start_line)
        .or_else(|| {
            // Line sits between blocks (e.g. blank lines). Take the
            // nearest block before it.
            block_spans
                .iter()
                .enumerate()
                .filter(|(_, s)| s.start_line <= target_line_usize)
                .max_by_key(|(_, s)| s.start_line)
        })?;

    let block_bounds = preview_scroll.bounds_for_item(block_ix)?;
    let block_top: f32 = block_bounds.origin.y.into();
    let block_height: f32 = block_bounds.size.height.into();

    let span_lines = span.end_line.saturating_sub(span.start_line) as f32 + 1.0;
    let line_in_block = (target_line - span.start_line as f32).max(0.0);
    let block_fraction = (line_in_block / span_lines).clamp(0.0, 1.0);
    let sub_offset = block_fraction * block_height;

    // Place `block_top + sub_offset` at `preview_viewport_top`.
    // Since child bounds are stored as if scroll offset were 0, the
    // required scroll offset is `viewport_top - (block_top + sub_offset)`.
    let raw = preview_viewport_top - (block_top + sub_offset);
    Some(clamp_negative_offset(raw, preview_scroll.max_offset().y.into()))
}

/// Clamp a desired (negative-or-zero) scroll offset to the valid
/// `[max_offset_y, 0]` range. `max_offset_y` is GPUI's convention: a
/// non-positive value representing the most-scrolled offset.
fn clamp_negative_offset(desired: f32, max_offset_y: f32) -> f32 {
    desired.clamp(max_offset_y.min(0.0), 0.0)
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // Sync scroll positions between panes
        let show_preview = self.state.read(cx).show_preview;
        if show_preview {
            self.sync_scroll(cx);
        }

        let state = self.state.read(cx);

        // Prefer the active buffer name; fall back to the file name.
        let buffer_name = state.current_buffer_name.clone();
        let dirty_marker = if state.document.is_dirty() { " *" } else { "" };
        let buffer_count = state.buffer_count();

        // Keep window title in sync with the current buffer.
        let title = if buffer_count > 1 {
            format!("{}{}  [{} buffers]", buffer_name, dirty_marker, buffer_count)
        } else {
            format!("{}{}", buffer_name, dirty_marker)
        };
        window.set_window_title(&title);

        let cursor_line = if state.document.len_chars() > 0 {
            state.document.char_to_line(
                state
                    .cursor
                    .position
                    .min(state.document.len_chars().saturating_sub(1)),
            ) + 1
        } else {
            1
        };
        let word_count = state.document.text().split_whitespace().count();
        let keymap_name = state.keymap_preset.name();

        // Emacs-style status indicators
        let universal_arg_text = state
            .universal_arg
            .map(|n| format!("C-u {} ", n));
        let isearch_text = if state.isearch.active {
            Some(format!("I-search: {}", state.isearch.query))
        } else {
            None
        };
        let macro_text = if state.macros.recording {
            Some("Def".to_string())
        } else {
            None
        };

        div()
            .id("main-view")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(self.toolbar.clone())
            .child(self.find_bar.clone())
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_grow()
                            .when(show_preview, |el| el.flex_basis(relative(0.5)))
                            .when(!show_preview, |el| el.flex_basis(relative(1.0)))
                            .overflow_hidden()
                            .child(self.editor.clone()),
                    )
                    .when(show_preview, |el| {
                        el.child(div().w(px(1.0)).h_full().bg(theme.border))
                            .child(
                                div()
                                    .flex_grow()
                                    .flex_basis(relative(0.5))
                                    .overflow_hidden()
                                    .child(self.preview.clone()),
                            )
                    }),
            )
            // Mode line (Emacs-style: always visible, above minibuffer)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .px_2()
                    .py(px(1.0))
                    .bg(theme.accent)
                    .text_size(px(11.0))
                    .text_color(theme.text_on_accent)
                    .child(format!("{}{}  [{} buffers]", buffer_name, dirty_marker, buffer_count))
                    .when_some(macro_text, |el, text| el.child(text))
                    .when_some(universal_arg_text, |el, text| el.child(text))
                    .when_some(isearch_text, |el, text| el.child(text))
                    .child(format!("Ln {}", cursor_line))
                    .child(format!("{} words", word_count))
                    .child(keymap_name),
            )
            // Mini-buffer / echo area (Emacs-style: below mode line)
            .child(self.minibuffer.clone())
    }
}
