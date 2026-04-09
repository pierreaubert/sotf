use comrak::Arena;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::markdown::{MdThemeColors, SourceMap, parse_markdown, render_markdown};
use crate::state::MdAppState;

/// Rendered markdown preview pane with click-to-locate and inline WYSIWYG editing.
pub struct PreviewPane {
    state: Entity<MdAppState>,
    pub scroll_handle: ScrollHandle,
    /// Document version at last parse — skip re-parsing when unchanged.
    cached_version: u64,
    /// Cached source map from the last parse.
    cached_source_map: SourceMap,
}

impl PreviewPane {
    pub fn new(state: Entity<MdAppState>, cx: &mut Context<Self>) -> Self {
        // Only re-render when the document version changes (not on every cursor move)
        cx.observe(&state, |this, state, cx| {
            let version = state.read(cx).document.version();
            if version != this.cached_version {
                cx.notify();
            }
        })
        .detach();

        Self {
            state,
            scroll_handle: ScrollHandle::new(),
            cached_version: u64::MAX, // force first parse
            cached_source_map: SourceMap::new(),
        }
    }
}

impl Render for PreviewPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let state_entity = self.state.clone();
        let state = self.state.read(cx);
        let text = state.document.text();
        let doc_version = state.document.version();
        let editing_block = state.editing_block.clone();
        let editing_text = state.editing_block_text.clone();

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

        let raw_elements = render_markdown(root, &mut source_map, &md_colors);

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

                elements.push(
                    div()
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover_bg))
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            state_for_click.update(cx, |s, _cx| s.jump_to_line(line));
                        })
                        .on_mouse_down(MouseButton::Right, move |_ev, _window, cx| {
                            state_for_edit
                                .update(cx, |s, _cx| s.start_inline_edit(&block_id_for_edit));
                        })
                        .child(raw_el)
                        .into_any_element(),
                );
            } else {
                elements.push(raw_el);
            }
        }

        div()
            .id("preview-pane")
            .size_full()
            .bg(theme.background)
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .p_4()
            .text_color(theme.text_primary)
            .text_size(px(15.0))
            .children(elements)
    }
}
