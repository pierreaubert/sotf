use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::state::MdAppState;
use crate::views::editor_pane::EditorPane;
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

        // Observe editor and preview so MainView re-renders (and runs
        // sync_scroll) whenever either pane scrolls. GPUI notifies a view's
        // owning entity on scroll, so observing those entities gives us a
        // scroll-change callback.
        cx.observe(&editor, |_this, _editor, cx| {
            cx.notify();
        })
        .detach();
        cx.observe(&preview, |_this, _preview, cx| {
            cx.notify();
        })
        .detach();

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

    /// Synchronize scroll positions between editor and preview panes.
    ///
    /// Uses a **source-line fraction** approach: the source line at the
    /// centre of the driving pane is used to compute a fraction of the
    /// document (line / total_lines), and the follower pane is scrolled to
    /// show that same fraction at its own centre. This keeps the two views
    /// aligned at the vertical middle regardless of how differently they
    /// render blocks (long code blocks in preview, uniform 20px lines in
    /// editor, etc.).
    fn sync_scroll(&mut self, cx: &mut Context<Self>) {
        const LINE_HEIGHT: f32 = 20.0;

        let editor_scroll = self.editor.read(cx).scroll_handle.clone();
        let preview_scroll = self.preview.read(cx).scroll_handle.clone();

        let editor_y: f32 = editor_scroll.offset().y.into();
        let preview_y: f32 = preview_scroll.offset().y.into();

        let editor_changed = (editor_y - self.last_editor_scroll_y).abs() > 0.5;
        let preview_changed = (preview_y - self.last_preview_scroll_y).abs() > 0.5;

        let doc_lines = self.state.read(cx).document.len_lines().max(1) as f32;

        let editor_viewport_h: f32 = editor_scroll.bounds().size.height.into();
        let editor_max: f32 = editor_scroll.max_offset().y.into();
        let preview_viewport_h: f32 = preview_scroll.bounds().size.height.into();
        let preview_max: f32 = preview_scroll.max_offset().y.into();

        if editor_changed && !preview_changed {
            // Editor drove the scroll. Compute the source line at the editor's
            // vertical middle, turn it into a fraction of total lines, and
            // position the preview so that same fraction is at ITS centre.
            //
            // Editor scroll offset is negative; -editor_y is the distance the
            // content has scrolled up.
            let editor_center_content_y = (-editor_y) + editor_viewport_h * 0.5;
            let center_line = (editor_center_content_y / LINE_HEIGHT).max(0.0);
            let frac = (center_line / doc_lines).clamp(0.0, 1.0);

            // Preview total scrollable content height.
            // preview_max is negative; -preview_max + viewport_h = full content height.
            let preview_content_h = (-preview_max) + preview_viewport_h;
            let target_center = frac * preview_content_h;
            let target_scroll_top = target_center - preview_viewport_h * 0.5;
            let clamped = clamp_scroll(-target_scroll_top, preview_max);
            preview_scroll.set_offset(point(preview_scroll.offset().x, px(clamped)));
        } else if preview_changed && !editor_changed {
            // Preview drove the scroll. Mirror the line-fraction approach.
            let preview_center_content_y = (-preview_y) + preview_viewport_h * 0.5;
            let preview_content_h = (-preview_max) + preview_viewport_h;
            if preview_content_h.abs() < 1.0 {
                self.last_editor_scroll_y = editor_scroll.offset().y.into();
                self.last_preview_scroll_y = preview_scroll.offset().y.into();
                return;
            }
            let frac = (preview_center_content_y / preview_content_h).clamp(0.0, 1.0);

            let target_center_line = frac * doc_lines;
            let target_center_y = target_center_line * LINE_HEIGHT;
            let target_scroll_top = target_center_y - editor_viewport_h * 0.5;
            let clamped = clamp_scroll(-target_scroll_top, editor_max);
            editor_scroll.set_offset(point(editor_scroll.offset().x, px(clamped)));
        }

        self.last_editor_scroll_y = editor_scroll.offset().y.into();
        self.last_preview_scroll_y = preview_scroll.offset().y.into();
    }
}

/// Clamp `desired` (negative-or-zero scroll offset) to the valid range
/// `[max_offset_y, 0]` where `max_offset_y` is also negative or zero.
fn clamp_scroll(desired: f32, max_offset_y: f32) -> f32 {
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
        let palette_text = if state.command_palette.visible {
            Some(format!("M-x: {}", state.command_palette.query))
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
            // Mini-buffer (renders only when active)
            .child(self.minibuffer.clone())
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
                    .child(format!("{}{}  [{} buffers]", buffer_name, dirty_marker, buffer_count))
                    .when_some(macro_text, |el, text| el.child(text))
                    .when_some(universal_arg_text, |el, text| el.child(text))
                    .when_some(isearch_text, |el, text| el.child(text))
                    .when_some(palette_text, |el, text| el.child(text))
                    .child(format!("Ln {}", cursor_line))
                    .child(format!("{} words", word_count))
                    .child(keymap_name),
            )
    }
}
