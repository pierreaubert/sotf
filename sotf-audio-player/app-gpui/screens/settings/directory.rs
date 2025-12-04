//! Directory manager screen rendering functions

use crate::app::InputMode;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, HStack, Progress, StackSpacing, Text, TextSize, TextWeight, VStack,
};
use sotf_audio_player::DirectoryInfo;
use std::path::Path;
use std::time::SystemTime;

impl PlayerView {
    pub(crate) fn render_directory_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let is_add_mode = state.app.input_mode == InputMode::AddDirectory;
        let tree_items = state.app.get_directory_tree_items();
        let selected_index = state.app.selected_directory_index;
        let scan_in_progress = state.app.scan_in_progress;
        let scan_progress_tracks = state.app.scan_progress_tracks;
        let scan_progress_albums = state.app.scan_progress_albums;
        let directory_input = state.app.directory_input.clone();
        let autocomplete_suggestions = state.app.autocomplete_suggestions.clone();
        let autocomplete_index = state.app.autocomplete_index;
        let directories = state.app.library.directories.clone();

        // Build title based on state
        let title = if scan_in_progress {
            format!(
                "Directory Manager - Scanning: {}T/{}A",
                scan_progress_tracks, scan_progress_albums
            )
        } else {
            "Directory Manager".to_string()
        };

        // Build directory items inline to avoid cx lifetime issues
        let directory_items: Vec<_> = tree_items
            .iter()
            .enumerate()
            .map(|(i, (path, level, expanded))| {
                build_directory_item(
                    i,
                    path,
                    *level,
                    *expanded,
                    i == selected_index,
                    &directories,
                    &theme,
                )
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_4()
            // Title
            .child(
                Text::new(title)
                    .size(TextSize::Lg)
                    .weight(TextWeight::Semibold),
            )
            // Add Directory Input Box (always visible, styled differently when active)
            .child(self.render_add_directory_box(
                &theme,
                is_add_mode,
                &directory_input,
                &autocomplete_suggestions,
                autocomplete_index,
                cx,
            ))
            // Directory List with click handlers
            .child(self.render_directory_list(directory_items, cx))
            // Scan Progress (when scanning)
            .when(scan_in_progress, |parent| {
                parent.child(render_scan_progress(
                    scan_progress_tracks,
                    scan_progress_albums,
                    &theme,
                ))
            })
            // Help Text Footer
            .child(render_directory_help_footer(&theme, is_add_mode))
    }

    /// Render the directory list with click handlers
    fn render_directory_list(
        &self,
        items: Vec<DirectoryItemData>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("directory-list")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .overflow_y_scroll()
            .children(items.into_iter().map(|item| {
                let index = item.index;
                let hover_bg = item.hover_bg;
                let is_selected = item.is_selected;

                div()
                    .id(item.id)
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(item.bg_color)
                    .when(!is_selected, |d| d.hover(move |s| s.bg(hover_bg)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseDownEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.selected_directory_index = index;
                            });
                            cx.notify();
                        }),
                    )
                    .on_click(cx.listener(move |view, event: &ClickEvent, _window, cx| {
                        // Double-click to toggle expansion
                        if event.click_count() == 2 {
                            view.state.update(cx, |state, _cx| {
                                state.app.selected_directory_index = index;
                                state.app.toggle_directory_expansion();
                            });
                            cx.notify();
                        }
                    }))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::None)
                            .child(
                                Text::new(item.display_text)
                                    .size(TextSize::Sm)
                                    .color(item.text_color)
                                    .weight(item.font_weight),
                            )
                            .child(Text::new(item.info_str).size(TextSize::Xs).muted(true)),
                    )
            }))
    }

    /// Render the add directory input box
    fn render_add_directory_box(
        &self,
        theme: &Theme,
        is_add_mode: bool,
        directory_input: &str,
        autocomplete_suggestions: &[String],
        autocomplete_index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border_color = if is_add_mode {
            theme.accent
        } else {
            theme.border
        };

        let display_text = if is_add_mode {
            format!("{}█", directory_input)
        } else {
            "Press 'A' to add directory".to_string()
        };

        let text_color = if is_add_mode {
            theme.text_primary
        } else {
            theme.text_muted
        };

        let show_autocomplete = is_add_mode && !autocomplete_suggestions.is_empty();
        let suggestions = autocomplete_suggestions.to_vec();
        let theme_clone = theme.clone();

        div()
            .p_3()
            .rounded_md()
            .bg(theme.surface)
            .border_1()
            .border_color(border_color)
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new("Add Directory")
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Semibold),
                            )
                            .when(is_add_mode, |h| {
                                h.child(Badge::new("Editing").variant(BadgeVariant::Primary))
                            }),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.background)
                            .border_1()
                            .border_color(if is_add_mode {
                                theme.accent
                            } else {
                                theme.border
                            })
                            .cursor_pointer()
                            .when(!is_add_mode, |d| {
                                d.on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.input_mode = InputMode::AddDirectory;
                                            state.app.directory_input.clear();
                                        });
                                        cx.notify();
                                    }),
                                )
                            })
                            .child(Text::new(display_text).size(TextSize::Sm).color(text_color)),
                    )
                    .child(
                        Text::new("Tab: autocomplete | Enter: add | Esc: cancel")
                            .size(TextSize::Xs)
                            .muted(true),
                    ),
            )
            // Autocomplete dropdown
            .when(show_autocomplete, |parent| {
                parent.child(
                    div()
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .bg(theme_clone.surface)
                        .border_1()
                        .border_color(theme_clone.border)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::None)
                                .child(
                                    Text::new(format!(
                                        "Suggestions ({}/{})",
                                        autocomplete_index + 1,
                                        suggestions.len()
                                    ))
                                    .size(TextSize::Xs)
                                    .muted(true),
                                )
                                .children(suggestions.iter().take(5).enumerate().map(
                                    |(i, suggestion)| {
                                        let is_selected = i == autocomplete_index;
                                        let bg = if is_selected {
                                            theme_clone.accent_muted
                                        } else {
                                            theme_clone.surface
                                        };
                                        let text_color = if is_selected {
                                            theme_clone.text_primary
                                        } else {
                                            theme_clone.text_secondary
                                        };

                                        div().px_2().py_1().rounded(px(2.0)).bg(bg).child(
                                            Text::new(suggestion.clone())
                                                .size(TextSize::Sm)
                                                .color(text_color),
                                        )
                                    },
                                )),
                        ),
                )
            })
    }
}

/// Data structure for a directory item (avoids lifetime issues)
struct DirectoryItemData {
    id: SharedString,
    index: usize,
    display_text: String,
    info_str: String,
    bg_color: Rgba,
    hover_bg: Rgba,
    text_color: Rgba,
    font_weight: TextWeight,
    is_selected: bool,
}

/// Build directory item data without needing cx
fn build_directory_item(
    index: usize,
    path: &Path,
    level: usize,
    expanded: bool,
    is_selected: bool,
    directories: &[DirectoryInfo],
    theme: &Theme,
) -> DirectoryItemData {
    let indent = "  ".repeat(level);
    let expand_indicator = if level == 0 {
        if expanded { "▼ " } else { "▶ " }
    } else {
        "└─ "
    };

    // Get path display - for subdirectories, just show the name
    let path_str = if level == 0 {
        path.display().to_string()
    } else {
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string())
    };

    // Find directory info for metadata
    let dir_info = find_dir_info(directories, path);
    let info_str = if let Some(info) = dir_info {
        let track_count = info.file_count;
        let album_count = info.album_count;
        let last_scan = format_relative_time(info.last_scanned);

        if level == 0 {
            format!(
                " [{} tracks, {} albums, {}]",
                track_count, album_count, last_scan
            )
        } else {
            format!(" [{} tracks, {} albums]", track_count, album_count)
        }
    } else {
        String::new()
    };

    let bg_color = if is_selected {
        theme.accent_muted
    } else {
        theme.surface
    };

    let text_color = if is_selected {
        theme.text_primary
    } else if level == 0 {
        theme.accent
    } else {
        theme.text_secondary
    };

    let font_weight = if level == 0 {
        TextWeight::Semibold
    } else {
        TextWeight::Normal
    };

    DirectoryItemData {
        id: SharedString::from(format!("dir-item-{}", index)),
        index,
        display_text: format!("{}{}{}", indent, expand_indicator, path_str),
        info_str,
        bg_color,
        hover_bg: theme.surface_hover,
        text_color,
        font_weight,
        is_selected,
    }
}

/// Render scan progress indicator
fn render_scan_progress(tracks: usize, albums: usize, theme: &Theme) -> impl IntoElement {
    div()
        .p_3()
        .rounded_md()
        .bg(theme.surface)
        .border_1()
        .border_color(theme.success)
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            Text::new("Scanning library...")
                                .size(TextSize::Sm)
                                .weight(TextWeight::Semibold),
                        )
                        .child(
                            Badge::new(format!("{} tracks", tracks)).variant(BadgeVariant::Primary),
                        )
                        .child(
                            Badge::new(format!("{} albums", albums)).variant(BadgeVariant::Info),
                        ),
                )
                .child(Progress::new(50.0).striped(true)),
        )
}

/// Render help footer with available commands
fn render_directory_help_footer(theme: &Theme, is_add_mode: bool) -> impl IntoElement {
    let help_text = if is_add_mode {
        "Tab: autocomplete | Enter: add directory | Esc: cancel"
    } else {
        "A: Add | D: Remove | Enter/L: Expand | S: Scan | Shift-R: Force Scan | M: Maintain | R: ReplayGain"
    };

    div()
        .p_3()
        .rounded_md()
        .bg(theme.background_secondary)
        .child(Text::new(help_text).size(TextSize::Xs).muted(true))
}

/// Helper to find directory info by path in the recursive structure
fn find_dir_info<'a>(directories: &'a [DirectoryInfo], path: &Path) -> Option<&'a DirectoryInfo> {
    for dir in directories {
        if dir.path == path {
            return Some(dir);
        }
        if let Some(found) = find_dir_info(&dir.subdirectories, path) {
            return Some(found);
        }
    }
    None
}

/// Format a SystemTime as relative time (e.g., "2 days ago")
fn format_relative_time(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => {
            if let Ok(elapsed) = t.elapsed() {
                let secs = elapsed.as_secs();
                if secs < 60 {
                    "just now".to_string()
                } else if secs < 3600 {
                    format!("{} min ago", secs / 60)
                } else if secs < 86400 {
                    format!("{} hrs ago", secs / 3600)
                } else {
                    format!("{} days ago", secs / 86400)
                }
            } else {
                "never".to_string()
            }
        }
        None => "never".to_string(),
    }
}
