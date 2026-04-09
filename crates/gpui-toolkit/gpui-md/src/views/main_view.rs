use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::state::MdAppState;
use crate::views::editor_pane::EditorPane;
use crate::views::find_bar::FindBar;
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

        Self {
            state,
            editor,
            preview,
            toolbar,
            find_bar,
            last_editor_scroll_y: 0.0,
            last_preview_scroll_y: 0.0,
        }
    }

    /// Synchronize scroll positions between editor and preview panes.
    /// Whichever pane's scroll changed since last frame drives the other.
    fn sync_scroll(&mut self, cx: &mut Context<Self>) {
        let editor_y: f32 = self.editor.read(cx).scroll_handle.offset().y.into();
        let preview_y: f32 = self.preview.read(cx).scroll_handle.offset().y.into();

        let editor_changed = (editor_y - self.last_editor_scroll_y).abs() > 0.5;
        let preview_changed = (preview_y - self.last_preview_scroll_y).abs() > 0.5;

        if editor_changed && !preview_changed {
            // Editor drove the scroll — sync preview proportionally
            let editor_max: f32 = self.editor.read(cx).scroll_handle.max_offset().y.into();
            let preview_max: f32 = self.preview.read(cx).scroll_handle.max_offset().y.into();

            if editor_max.abs() > 1.0 {
                let fraction = editor_y / editor_max;
                let target_y = fraction * preview_max;
                self.preview.read(cx).scroll_handle.set_offset(point(
                    self.preview.read(cx).scroll_handle.offset().x,
                    px(target_y),
                ));
            }
        } else if preview_changed && !editor_changed {
            // Preview drove the scroll — sync editor proportionally
            let editor_max: f32 = self.editor.read(cx).scroll_handle.max_offset().y.into();
            let preview_max: f32 = self.preview.read(cx).scroll_handle.max_offset().y.into();

            if preview_max.abs() > 1.0 {
                let fraction = preview_y / preview_max;
                let target_y = fraction * editor_max;
                self.editor.read(cx).scroll_handle.set_offset(point(
                    self.editor.read(cx).scroll_handle.offset().x,
                    px(target_y),
                ));
            }
        }

        self.last_editor_scroll_y = editor_y;
        self.last_preview_scroll_y = preview_y;
    }
}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // Sync scroll positions between panes
        let show_preview = self.state.read(cx).show_preview;
        if show_preview {
            self.sync_scroll(cx);
        }

        let state = self.state.read(cx);

        let file_name = state
            .document
            .file_path()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|| "Untitled".to_string());
        let dirty_marker = if state.document.is_dirty() { " *" } else { "" };

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
        let palette_text = if state.command_palette.visible {
            Some(format!("M-x: {}", state.command_palette.query))
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
            // Status bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .px_3()
                    .py_1()
                    .bg(theme.accent)
                    .text_size(px(12.0))
                    .text_color(theme.text_on_accent)
                    .child(format!("{}{}", file_name, dirty_marker))
                    .when_some(universal_arg_text, |el, text| el.child(text))
                    .when_some(isearch_text, |el, text| el.child(text))
                    .when_some(palette_text, |el, text| el.child(text))
                    .child(format!("Ln {}", cursor_line))
                    .child(format!("{} words", word_count))
                    .child(keymap_name),
            )
    }
}
