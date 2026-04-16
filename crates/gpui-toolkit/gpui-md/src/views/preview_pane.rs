use comrak::Arena;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::accessibility::AccessibilityExt;
use gpui_ui_kit::theme::ThemeExt;

use crate::markdown::{MdThemeColors, SourceMap, SourceSpan, parse_markdown, render_markdown};
use crate::state::MdAppState;

/// Rendered markdown preview pane with click-to-locate and inline WYSIWYG editing.
pub struct PreviewPane {
    state: Entity<MdAppState>,
    pub scroll_handle: ScrollHandle,
    /// Document version at last parse — skip re-parsing when unchanged.
    cached_version: u64,
    /// Cached source map from the last parse.
    cached_source_map: SourceMap,
    /// Cached value of show_preview_line_numbers for change detection.
    cached_show_line_numbers: bool,
    /// Source spans for each block, aligned with child index in the
    /// `.children(elements)` call. `block_spans[i]` is the span of the
    /// i-th child of the preview scroll container, which lets sync code
    /// use `scroll_handle.bounds_for_item(i)` to get that block's
    /// rendered pixel bounds.
    pub block_spans: Vec<SourceSpan>,
}

impl PreviewPane {
    pub fn new(state: Entity<MdAppState>, cx: &mut Context<Self>) -> Self {
        // Re-render on document changes and settings changes (line numbers, etc.)
        cx.observe(&state, |this, state, cx| {
            let s = state.read(cx);
            let version = s.document.version();
            let show_ln = s.show_preview_line_numbers;
            // Re-render on document changes and settings changes
            if version != this.cached_version || show_ln != this.cached_show_line_numbers {
                cx.notify();
            }
        })
        .detach();

        Self {
            state,
            scroll_handle: ScrollHandle::new(),
            cached_version: u64::MAX, // force first parse
            cached_source_map: SourceMap::new(),
            cached_show_line_numbers: false,
            block_spans: Vec::new(),
        }
    }
}

impl Render for PreviewPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let state_entity = self.state.clone();
        let state = self.state.read(cx);
        let text = state.document.text();
        let doc_version = state.document.version();
        let editing_block = state.editing_block.clone();
        let editing_text = state.editing_block_text.clone();
        let show_line_numbers = state.show_preview_line_numbers;
        let font_size = state.font_size;
        let preview_font_size = font_size + 1.0; // preview is slightly larger than editor
        let text_muted = theme.text_muted;

        let md_colors = MdThemeColors::from_theme(&theme);

        let arena = Arena::new();
        let root = parse_markdown(&arena, &text);
        let mut source_map = self.cached_source_map.clone();

        // Always rebuild elements (GPUI consumes them), but only
        // rebuild the source map when the document has changed.
        let needs_source_map_update = doc_version != self.cached_version;
        if needs_source_map_update {
            source_map = SourceMap::new();
        }

        // Get content width for Knuth-Plass justification.
        // Use window bounds directly so it works on first render (no mouse move needed).
        // Preview pane takes ~50% of window width in split view, minus p_4 padding (32px).
        let window_width: f32 = window.bounds().size.width.into();
        let justify_width = if window_width > 100.0 && window_width < 10000.0 {
            (window_width * 0.5) - 32.0
        } else {
            0.0
        };
        let text_system = window.text_system().clone();
        let (raw_elements, a11y_nodes) = render_markdown(root, &mut source_map, &md_colors, justify_width, Some(&text_system), preview_font_size);
        for node in a11y_nodes {
            cx.register_accessible(node);
        }

        self.cached_show_line_numbers = show_line_numbers;
        if needs_source_map_update {
            self.cached_version = doc_version;
            self.cached_source_map = source_map.clone();
            state_entity.update(cx, |s, _cx| {
                s.source_map = source_map.clone();
                s.last_parsed_version = doc_version;
            });
        }

        let blocks_by_line = source_map.blocks_by_line();
        let block_lines: Vec<(String, usize)> = blocks_by_line
            .iter()
            .map(|(id, span)| ((*id).clone(), span.start_line))
            .collect();

        // Build `block_spans` aligned with the child index that each
        // block will occupy in the `.children(elements)` call below.
        // `scroll_handle.bounds_for_item(i)` then returns the rendered
        // pixel bounds for `block_spans[i]`, which is what the scroll
        // sync in `MainView` uses to align editor lines to preview
        // pixels without drift.
        self.block_spans = (0..raw_elements.len())
            .map(|i| {
                let block_id = format!("md-block-{}", i);
                source_map
                    .get(&block_id)
                    .copied()
                    .unwrap_or(SourceSpan {
                        start_line: 0,
                        start_col: 0,
                        end_line: 0,
                        end_col: 0,
                    })
            })
            .collect();

        let mut elements: Vec<AnyElement> = Vec::new();
        for (i, raw_el) in raw_elements.into_iter().enumerate() {
            let block_id = format!("md-block-{}", i);
            let source_line = block_lines
                .iter()
                .find(|(id, _)| id == &block_id)
                .map(|(_, line)| *line);

            if editing_block.as_deref() == Some(&block_id) {
                let state_for_commit = state_entity.clone();
                let state_for_cancel = state_entity.clone();
                let accent = theme.accent;
                let surface = theme.surface;
                let border = theme.border;

                elements.push(
                    div()
                        .p_2()
                        .rounded_sm()
                        .bg(surface)
                        .border_1()
                        .border_color(accent)
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_family("monospace")
                                .text_color(theme.text_primary)
                                .child(editing_text.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .mt_2()
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(accent)
                                        .text_color(theme.text_on_accent)
                                        .text_size(px(11.0))
                                        .cursor_pointer()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            move |_ev, _window, cx| {
                                                state_for_commit
                                                    .update(cx, |s, _cx| s.commit_inline_edit());
                                            },
                                        )
                                        .child("Save"),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(border)
                                        .text_color(theme.text_secondary)
                                        .text_size(px(11.0))
                                        .cursor_pointer()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            move |_ev, _window, cx| {
                                                state_for_cancel
                                                    .update(cx, |s, _cx| s.cancel_inline_edit());
                                            },
                                        )
                                        .child("Cancel"),
                                ),
                        )
                        .into_any_element(),
                );
            } else if let Some(line) = source_line {
                let state_for_click = state_entity.clone();
                let state_for_edit = state_entity.clone();
                let block_id_for_edit = block_id.clone();
                let hover_bg = theme.surface_hover;

                let mut block_div = div()
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                        state_for_click.update(cx, |s, _cx| s.jump_to_line(line));
                    })
                    .on_mouse_down(MouseButton::Right, move |_ev, _window, cx| {
                        state_for_edit
                            .update(cx, |s, _cx| s.start_inline_edit(&block_id_for_edit));
                    });

                if show_line_numbers {
                    // Render as a row with line number gutter + content
                    block_div = block_div
                        .flex()
                        .flex_row()
                        .child(
                            div()
                                .w(px(36.0))
                                .flex_shrink_0()
                                .text_size(px(11.0))
                                .text_color(text_muted)
                                .text_right()
                                .pr_2()
                                .pt(px(2.0))
                                .child(format!("{}", line + 1)),
                        )
                        .child(
                            div().flex_grow().min_w(px(0.0)).overflow_hidden().child(raw_el),
                        );
                } else {
                    block_div = block_div.child(raw_el);
                }

                elements.push(block_div.into_any_element());
            } else {
                elements.push(raw_el);
            }
        }

        div()
            .id("preview-pane")
            .size_full()
            .bg(theme.background)
            .overflow_y_scroll()
            .overflow_x_hidden()
            .track_scroll(&self.scroll_handle)
            .on_scroll_wheel({
                let state_scroll = state_entity.clone();
                move |_ev, _window, cx| {
                    // Poke state so MainView re-renders and sync_scroll fires.
                    state_scroll.update(cx, |_s, _cx| {});
                }
            })
            .p_4()
            .text_color(theme.text_primary)
            .text_size(px(preview_font_size))
            .children(elements)
    }
}
