//! Queue screen rendering functions
//!
//! Level meter UI components are now consolidated in `ui/components/plugins/level_meters.rs`

use std::collections::BTreeMap;

use gpui::prelude::*;
use gpui::*;
use gpui::{InteractiveElement, Styled};
use gpui_ui_kit::{
    Button, CollapseDirection, PaneDivider, PaneDividerTheme, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

use crate::app::types::{MeterDisplayMode, Screen};
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::level_meters::LevelMeterManager;
use sotf_audio_player::Track;

use crate::ui::PlayerView;

impl PlayerView {
    pub(crate) fn render_queue_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let layout = state.layout.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();

        // Use ratios for panel widths (layout will compute actual sizes)
        let queue_list_ratio = layout.queue_list_ratio;
        let meters_ratio = layout.meters_panel_ratio;
        let meter_display_mode = state.app.meter_display_mode;
        let window_height = state.app.ui_state.window_height;
        let window_width = state.app.ui_state.window_width;
        let hide_meters_for_rack = state.app.hide_queue_meters_for_rack;

        // Calculate available width for the queue panel (between library and rack)
        let library_width = if layout.library_panel_collapsed {
            0.0
        } else {
            layout.library_h_ratio * window_width
        };
        let rack_width = if layout.rack_panel_collapsed {
            0.0
        } else {
            layout.rack_h_ratio * window_width
        };
        let available_queue_width = (window_width - library_width - rack_width).max(0.0);

        // When the meters panel is tall enough, show both LUFS and Meters stacked
        // (no toggle switch needed). The queue panel height depends on the layout ratio.
        let meters_panel_tall = window_height > 700.0;

        let queue_collapsed = queue_list_ratio < 0.05;
        // Hide meters when: explicitly collapsed, OR rack is visible in 3-panel layout
        let meters_collapsed = meters_ratio < 0.05 || hide_meters_for_rack;

        // Home button for navigation back to Library
        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .flex_col()
            .size_full()
            // Header bar with Home button and title
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_4()
                    .py_2()
                    .bg(theme.background_secondary)
                    .border_b_1()
                    .border_color(theme.border)
                    // Home button on the left
                    .child(
                        div()
                            .id("queue-home-button")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(rems(2.5))
                            .h(rems(2.0))
                            .cursor_pointer()
                            .rounded_md()
                            .hover(move |s| s.bg(surface_hover))
                            .child(Icon::new(IconName::Home).color(text_muted))
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                state_for_home.update(cx, |state, _cx| {
                                    state.app.ui_state.current_screen = Screen::Library;
                                });
                            }),
                    )
                    // Title
                    .child(
                        div()
                            .ml_2()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(translations.queue_title.to_string()),
                    ),
            )
            // Main content area
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Left panel: Queue list
                    .when(!queue_collapsed, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .w(relative(queue_list_ratio))
                        .px_2()
                        .pt_2()
                        .border_r_1()
                        .border_color(theme.border)
                        .child(
                            Text::new(format!(
                                "{} ({} {})",
                                translations.queue_title,
                                state.app.queue.len(),
                                translations.queue_albums
                            ))
                            .size(TextSize::Md)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary)
                            .build()
                            .mb_2(),
                        )
                        .child(
                            div()
                                .id("queue-list")
                                .flex()
                                .flex_col()
                                .gap_2()
                                .flex_1()
                                .overflow_y_scroll()
                                .children(state.app.queue.iter().enumerate().map(|(idx, item)| {
                                    let is_current = state.app.playback.current_queue_index == Some(idx);
                                    let theme = theme.clone();
                                    let theme_hover = theme.clone();
                                    div()
                                        .p_2()
                                        .rounded_md()
                                        .when(is_current, |d| {
                                            // Current item: accent background, subtle hover
                                            let mut hover_bg = theme.accent;
                                            hover_bg.a = 0.8;
                                            d.bg(theme.accent)
                                             .hover(move |style| style.bg(hover_bg))
                                        })
                                        .when(!is_current, |d| {
                                            d.bg(theme_hover.surface)
                                             .hover(|style| style.bg(theme_hover.surface_hover))
                                        })
                                        .cursor_pointer()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.playback.current_queue_index = Some(idx);
                                                    if let Some(source) = state.app.queue[idx]
                                                        .current_track()
                                                        .map(|t| t.audio_source())
                                                    {
                                                        Self::play_track(state, source);
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .on_mouse_up(
                                            MouseButton::Right,
                                            cx.listener(
                                                move |view, event: &MouseUpEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.playback.current_queue_index = Some(idx);
                                                        state.app.ui_state.context_menu =
                                                        Some(crate::app::ContextMenuState {
                                                            menu_type:
                                                                crate::app::ContextMenuType::QueueItem,
                                                            position_x: event.position.x.into(),
                                                            position_y: event.position.y.into(),
                                                            item_index: idx,
                                                        });
                                                    });
                                                    cx.notify();
                                                },
                                            ),
                                        )
                                        .child({
                                            // Dynamic truncation based on panel width
                                            let max_title_chars = state.app.max_chars_queue_list_title(layout);
                                            let max_artist_chars = state.app.max_chars_queue_list_artist(layout);

                                            let album_title = item.album.title.clone();
                                            let album_title_truncated = if album_title.chars().count() > max_title_chars {
                                                album_title.chars().take(max_title_chars).collect::<String>() + "..."
                                            } else {
                                                album_title
                                            };

                                            let artist = item.album.artist();
                                            let artist_truncated = if artist.chars().count() > max_artist_chars {
                                                artist.chars().take(max_artist_chars).collect::<String>() + "..."
                                            } else {
                                                artist
                                            };

                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_sm()
                                                        .text_color(if is_current { theme.text_on_accent } else { theme.text_primary })
                                                        .child(album_title_truncated),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(if is_current { theme.text_on_accent_muted } else { theme.text_muted })
                                                        .child(artist_truncated),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(if is_current { theme.text_on_accent_muted } else { theme.text_secondary })
                                                        .child(format!(
                                                            "Track {}/{}",
                                                            item.current_track_index + 1,
                                                            item.album.tracks.len()
                                                        )),
                                                )
                                        })
                                })),
                        )
                        .child(
                            div()
                                .p_2()
                                .child(
                                    Button::new("magic-radio-btn", "Magic Radio")
                                        .full_width(true)
                                        .theme(theme.to_button_theme())
                                        .build()
                                        .on_click(cx.listener(|view, _event: &ClickEvent, _window, cx| {
                                            log::info!("[Queue] Magic Radio button clicked");
                                            view.state.update(cx, |state, _cx| {
                                                match state.app.fill_queue_magic() {
                                                    Ok(count) => {
                                                        log::info!("[Queue] Magic Radio added {} tracks", count);
                                                    }
                                                    Err(e) => {
                                                        log::error!("[Queue] Magic Radio error: {}", e);
                                                    }
                                                }
                                            });
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
            // Separator 1 (Queue <-> Center)
            .child({
                let divider_theme = PaneDividerTheme {
                    background: theme.background,
                    background_hover: theme.surface_hover,
                    background_collapsed: theme.surface,
                    foreground: theme.text_muted,
                    foreground_hover: theme.text_secondary,
                    border: theme.border,
                };
                PaneDivider::vertical("queue-list-divider", CollapseDirection::Left)
                    .label("Queue")
                    .collapsed(queue_collapsed)
                    .theme(divider_theme)
                    .on_toggle({
                        let state_handle = self.state.clone();
                        move |collapsed, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.queue_list_ratio = if collapsed { 0.0 } else { 0.30 };
                                    let _ = state.app.save_config(layout);
                                });
                            });
                        }
                    })
                    .on_drag_start({
                        let state_handle = self.state.clone();
                        move |_pos, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.is_dragging_queue_list_divider = true;
                                });
                            });
                        }
                    })
            })
            // Center panel: Now playing info
            .child(
                self.render_now_playing_info(&translations, cx)
            )
            // Separator 2 (Center <-> Right)
            .child({
                let divider_theme = PaneDividerTheme {
                    background: theme.background,
                    background_hover: theme.surface_hover,
                    background_collapsed: theme.surface,
                    foreground: theme.text_muted,
                    foreground_hover: theme.text_secondary,
                    border: theme.border,
                };
                PaneDivider::vertical("meters-divider", CollapseDirection::Right)
                    .label("Meters")
                    .collapsed(meters_collapsed)
                    .theme(divider_theme)
                    .on_toggle({
                        let state_handle = self.state.clone();
                        move |collapsed, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.meters_panel_ratio = if collapsed { 0.0 } else { 0.25 };
                                    // Mutual exclusion: opening meters collapses rack
                                    if !collapsed {
                                        layout.rack_panel_collapsed = true;
                                    }
                                    let _ = state.app.save_config(layout);
                                });
                            });
                        }
                    })
                    .on_drag_start({
                        let state_handle = self.state.clone();
                        move |_pos, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.is_dragging_meters_divider = true;
                                });
                            });
                        }
                    })
            })
            // Right panel: LUFS and/or Level meters
            .when(!meters_collapsed, |d| {
                let state_entity = self.state.clone();

                // Use meters_panel_ratio to control width (resizable via divider drag)
                let panel_width = (meters_ratio * available_queue_width).clamp(120.0, available_queue_width * 0.6);

                d.child(
                    div()
                        .w(px(panel_width))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .h_full()
                        // Toggle header: only show when not tall enough to display both
                        .when(!meters_panel_tall, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .justify_center()
                                    .p_2()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(
                                        div()
                                            .flex()
                                            .rounded_md()
                                            .bg(theme.background)
                                            .border_1()
                                            .border_color(theme.border)
                                            .overflow_hidden()
                                            // LUFS button
                                            .child(
                                                div()
                                                    .id("meter-toggle-lufs")
                                                    .px_3()
                                                    .py_1()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .cursor_pointer()
                                                    .when(meter_display_mode == MeterDisplayMode::Lufs, |d| {
                                                        d.bg(theme.accent).text_color(theme.text_on_accent)
                                                    })
                                                    .when(meter_display_mode != MeterDisplayMode::Lufs, |d| {
                                                        d.text_color(theme.text_secondary)
                                                            .hover(|s| s.bg(theme.surface_hover))
                                                    })
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener({
                                                            let state = state_entity.clone();
                                                            move |_view, _: &MouseUpEvent, _window, cx| {
                                                                state.update(cx, |state, _| {
                                                                    state.app.meter_display_mode =
                                                                        MeterDisplayMode::Lufs;
                                                                });
                                                                cx.notify();
                                                            }
                                                        }),
                                                    )
                                                    .child("LUFS"),
                                            )
                                            // Levels button
                                            .child(
                                                div()
                                                    .id("meter-toggle-levels")
                                                    .px_3()
                                                    .py_1()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .cursor_pointer()
                                                    .when(meter_display_mode == MeterDisplayMode::Levels, |d| {
                                                        d.bg(theme.accent).text_color(theme.text_on_accent)
                                                    })
                                                    .when(meter_display_mode != MeterDisplayMode::Levels, |d| {
                                                        d.text_color(theme.text_secondary)
                                                            .hover(|s| s.bg(theme.surface_hover))
                                                    })
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener({
                                                            let state = state_entity.clone();
                                                            move |_view, _: &MouseUpEvent, _window, cx| {
                                                                state.update(cx, |state, _| {
                                                                    state.app.meter_display_mode =
                                                                        MeterDisplayMode::Levels;
                                                                    // Ensure meter groups are initialized
                                                                    state.app.update_level_meter_groups();
                                                                });
                                                                cx.notify();
                                                            }
                                                        }),
                                                    )
                                                    .child("Meters"),
                                            ),
                                    ),
                            )
                        })
                        // Content: show both stacked when tall, or toggled when short
                        .when(meters_panel_tall, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .overflow_hidden()
                                    // LUFS panel on top
                                    .child(self.render_lufs_panel(cx))
                                    // Level meters below, centered
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(self.render_meters_panel(cx)),
                                    ),
                            )
                        })
                        .when(!meters_panel_tall, |d| {
                            d.child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .when(meter_display_mode == MeterDisplayMode::Lufs, |d| {
                                        d.child(self.render_lufs_panel(cx))
                                    })
                                    .when(meter_display_mode == MeterDisplayMode::Levels, |d| {
                                        d.child(self.render_meters_panel(cx))
                                    }),
                            )
                        }),
                )
            }),
            )
    }

    // Level meter methods (render_lufs_panel, render_meters_panel, render_meter_group, etc.)
    // are now in ui/components/plugins/level_meters.rs

    /// Render the now playing information panel (center)
    pub(crate) fn render_now_playing_info(
        &self,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let layout = state.layout.read(cx);
        let theme = state.app.ui_state.theme.clone();
        // Clone theme for use in closures (moved into flat_map)
        let theme_for_closure = theme.clone();
        let translations = translations.clone();

        // Get current queue item with all album info
        let queue_item = state
            .app
            .playback
            .current_queue_index
            .and_then(|idx| state.app.queue.get(idx));

        let content: AnyElement = if let Some(item) = queue_item {
            let album = &item.album;
            let current_track_idx = item.current_track_index;
            let current_track = item.current_track();

            // Get replay gain from current track (or first track with it)
            let replay_gain = current_track
                .and_then(|t| t.replay_gain)
                .or_else(|| album.tracks.iter().find_map(|t| t.replay_gain));

            // Get channel count from current track
            let channels = current_track.and_then(|t| t.channels).unwrap_or(2);

            let (file_type, bit_depth, sample_rate_str) = if let Some(track) = current_track {
                let ext = track
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_uppercase();
                let bd = track
                    .bit_depth
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "--".to_string());
                let sr = track
                    .sample_rate
                    .map(|s| {
                        if s >= 1000 {
                            if s % 1000 == 0 {
                                format!("{}k", s / 1000)
                            } else {
                                format!("{:.1}k", s as f32 / 1000.0)
                            }
                        } else {
                            format!("{}", s)
                        }
                    })
                    .unwrap_or_else(|| "--".to_string());
                (ext, bd, sr)
            } else {
                ("".into(), "--".into(), "--".into())
            };

            let track_count = album.tracks.len();
            let album_id = album.id;
            let album_is_favorite = album.is_favorite;

            let album_title_full = album.title.clone();
            let artist_raw = album.artist();
            let art_path = album.album_art_path.clone();

            // Dynamic truncation based on window/panel size
            let max_title_chars = state.app.max_chars_now_playing_title(layout);
            let max_artist_chars = state.app.max_chars_now_playing_artist(layout);

            let album_title = if album_title_full.chars().count() > max_title_chars {
                album_title_full
                    .chars()
                    .take(max_title_chars)
                    .collect::<String>()
                    + "..."
            } else {
                album_title_full.clone()
            };

            let artist = if artist_raw.chars().count() > max_artist_chars {
                artist_raw
                    .chars()
                    .take(max_artist_chars)
                    .collect::<String>()
                    + "..."
            } else {
                artist_raw
            };

            // Group tracks by disc number
            let mut disc_map: BTreeMap<u32, Vec<(usize, Track)>> = BTreeMap::new();
            for (idx, track) in album.tracks.iter().enumerate() {
                let disc = track.disc_number.unwrap_or(1);
                disc_map.entry(disc).or_default().push((idx, track.clone()));
            }

            div()
                .flex()
                .flex_col()
                .flex_1()
                .child(
                    div().mb_3().child(
                        Text::new(translations.queue_now_playing)
                            .size(TextSize::Md)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    ),
                )
                // Top row: Album art (left) + Album info (right)
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .mb_4()
                        // Album art (smaller, top-left)
                        .child({
                            let art_div = div()
                                .w(rems(7.5))
                                .h(rems(7.5))
                                .bg(theme.surface)
                                .rounded_lg()
                                .overflow_hidden()
                                .flex_shrink_0();

                            if let Some(path) = art_path {
                                art_div.child(
                                    img(path)
                                        .w_full()
                                        .h_full()
                                        .object_fit(gpui::ObjectFit::Cover),
                                )
                            } else {
                                art_div
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.text_muted)
                                    .text_2xl()
                                    .child("♪")
                            }
                        })
                        // Album info (right of art)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .flex_1()
                                // Album title
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(album_title.clone()),
                                )
                                // Artist
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.text_secondary)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(artist),
                                )
                                // Technical Info (Combined)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .mt_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.text_secondary)
                                                .child(format!("{} {}/{}", file_type, bit_depth, sample_rate_str)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child(format!("#{}", track_count)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child(translations.queue_replay_gain),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(if replay_gain.is_some() {
                                                    theme.text_secondary
                                                } else {
                                                    theme.text_muted
                                                })
                                                .child(
                                                    replay_gain
                                                        .map(|g| format!("{:+.1}dB", g))
                                                        .unwrap_or_else(|| "N/A".to_string()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child(translations.queue_channels),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text_secondary)
                                                .child(format!("{}", channels)),
                                        )
                                        // Album favorite heart
                                        .when_some(album_id, |d, aid| {
                                            d.child(
                                                div()
                                                    .id("album-heart")
                                                    .cursor_pointer()
                                                    .ml_2()
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                                            cx.stop_propagation();
                                                            view.state.update(cx, |state, _cx| {
                                                                state.app.toggle_album_favorite(aid);
                                                            });
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .child(
                                                        Icon::new(if album_is_favorite {
                                                            IconName::HeartFilled
                                                        } else {
                                                            IconName::Heart
                                                        })
                                                        .xs()
                                                        .color(if album_is_favorite {
                                                            theme.accent
                                                        } else {
                                                            theme.text_muted
                                                        }),
                                                    ),
                                            )
                                        }),
                                ),
                        ),
                )
                .child(self.render_track_list(
                    &disc_map,
                    current_track_idx,
                    &album_title_full,
                    &translations,
                    &theme_for_closure,
                    cx,
                ))
                .into_any_element()
        } else {
            VStack::new()
                .spacing(StackSpacing::Xs)
                .child(
                    Text::new(translations.queue_no_track_playing)
                        .size(TextSize::Md)
                        .color(theme.text_muted),
                )
                .child(
                    Text::new(translations.queue_select_album)
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                )
                .build()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_2()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.background_secondary)
            .child(
                div()
                    .size_full()
                    .id("now-playing-scroll")
                    .overflow_y_scroll()
                    .child(content),
            )
    }

    /// Render the track list with clickable items
    fn render_track_list(
        &self,
        disc_map: &BTreeMap<u32, Vec<(usize, Track)>>,
        current_track_idx: usize,
        album_title: &str,
        translations: &crate::i18n::Translations,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Get dynamic max chars for track titles based on window size
        let state = self.state.read(cx);
        let layout = state.layout.read(cx);
        let max_track_chars = state.app.max_chars_track_title(layout);

        // Find common prefix to strip from track names
        // For albums like "Monteverdi: Vespro della Beata...", tracks often start with "Vespro della Beata..."
        let prefix_to_strip = Self::find_common_track_prefix(album_title, disc_map);

        let disc_count = disc_map.len();
        let mut all_elements: Vec<AnyElement> = Vec::new();

        for (disc_num, tracks) in disc_map.iter() {
            let show_header = disc_count > 1;

            if show_header {
                all_elements.push(
                    div()
                        .py_1()
                        .mt_2()
                        .mb_1()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_secondary)
                        .child(format!("{} {}", translations.queue_disc, disc_num))
                        .into_any_element(),
                );
            }

            for (idx, track) in tracks.iter() {
                let idx = *idx;
                let title_raw = track.title.clone().unwrap_or_else(|| "Unknown".to_string());

                // Strip common prefix from track title if present
                let title_stripped = if let Some(ref prefix) = prefix_to_strip {
                    if title_raw.starts_with(prefix) {
                        title_raw[prefix.len()..]
                            .trim_start_matches(&[' ', ':', '-', '.'][..])
                            .to_string()
                    } else {
                        title_raw
                    }
                } else {
                    title_raw
                };

                // Dynamic truncation based on window size
                let title = if title_stripped.chars().count() > max_track_chars {
                    title_stripped
                        .chars()
                        .take(max_track_chars)
                        .collect::<String>()
                        + "..."
                } else {
                    title_stripped
                };

                let duration = track.duration_secs.unwrap_or(0);
                let is_current = idx == current_track_idx;
                let duration_str = format!("{}:{:02}", duration / 60, duration % 60);
                let theme_c = theme.clone();
                let track_path = track.audio_source();
                let track_play_count = track.play_count;
                let track_is_favorite = track.is_favorite;
                let heart_track_path = track.path.clone();

                all_elements.push(
                    div()
                        .id(SharedString::from(format!("track-{}", idx)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .when(is_current, |d| {
                            d.bg(theme_c.accent).text_color(theme_c.text_on_accent)
                        })
                        .when(!is_current, |d| d.hover(|s| s.bg(theme_c.surface_hover)))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                let path = track_path.clone();
                                view.state.update(cx, |state, _cx| {
                                    // Update the track index in the current queue item
                                    if let Some(queue_idx) = state.app.playback.current_queue_index
                                        && let Some(item) = state.app.queue.get_mut(queue_idx)
                                    {
                                        item.current_track_index = idx;
                                    }
                                    Self::play_track(state, path);
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .w(rems(1.5))
                                .text_xs()
                                .text_color(if is_current {
                                    theme_c.text_on_accent
                                } else {
                                    theme_c.text_muted
                                })
                                .child(format!(
                                    "{}",
                                    track.track_number.unwrap_or((idx + 1) as u32)
                                )),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .when(is_current, |d| d.font_weight(FontWeight::SEMIBOLD))
                                .child(title),
                        )
                        // Play count (only if > 0)
                        .when(track_play_count > 0, |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(if is_current {
                                        theme_c.text_on_accent
                                    } else {
                                        theme_c.text_muted
                                    })
                                    .child(format!("#{}", track_play_count)),
                            )
                        })
                        // Favorite heart icon
                        .child(
                            div()
                                .id(SharedString::from(format!("heart-track-{}", idx)))
                                .cursor_pointer()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        // Stop propagation so the row's play handler doesn't fire
                                        cx.stop_propagation();
                                        let path = heart_track_path.clone();
                                        view.state.update(cx, |state, _cx| {
                                            state.app.toggle_track_favorite(&path);
                                        });
                                        cx.notify();
                                    }),
                                )
                                .child(
                                    Icon::new(if track_is_favorite {
                                        IconName::HeartFilled
                                    } else {
                                        IconName::Heart
                                    })
                                    .xs()
                                    .color(
                                        if track_is_favorite && is_current {
                                            theme_c.text_on_accent
                                        } else if track_is_favorite {
                                            theme_c.accent
                                        } else if is_current {
                                            theme_c.text_on_accent
                                        } else {
                                            theme_c.text_muted
                                        },
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_current {
                                    theme_c.text_on_accent
                                } else {
                                    theme_c.text_muted
                                })
                                .child(duration_str),
                        )
                        .into_any_element(),
                );
            }
        }

        div().flex().flex_col().flex_1().overflow_hidden().child(
            div()
                .id("track-list")
                .flex()
                .flex_col()
                .gap_0p5()
                .flex_1()
                .overflow_y_scroll()
                .children(all_elements),
        )
    }

    /// Find a common prefix in track titles that matches part of the album title.
    /// For albums like "Monteverdi: Vespro della Beata Vergine", tracks often start with
    /// "Vespro della Beata Vergine" which should be stripped.
    fn find_common_track_prefix(
        album_title: &str,
        disc_map: &BTreeMap<u32, Vec<(usize, Track)>>,
    ) -> Option<String> {
        // Extract potential prefix candidates from album title
        // Try the part after ":" if present, otherwise use the full title
        let candidates: Vec<&str> = if album_title.contains(':') {
            album_title
                .split(':')
                .skip(1)
                .map(|s| s.trim())
                .filter(|s| s.len() >= 5) // Minimum meaningful prefix length
                .collect()
        } else {
            vec![album_title.trim()]
        };

        // Collect all track titles
        let all_tracks: Vec<&str> = disc_map
            .values()
            .flat_map(|tracks| tracks.iter())
            .filter_map(|(_, track)| track.title.as_deref())
            .collect();

        if all_tracks.is_empty() {
            return None;
        }

        // For each candidate, check if most tracks start with it
        for candidate in candidates {
            // Try progressively shorter prefixes from the candidate
            let words: Vec<&str> = candidate.split_whitespace().collect();
            for word_count in (2..=words.len()).rev() {
                let prefix = words[..word_count].join(" ");
                if prefix.len() < 5 {
                    continue;
                }

                // Count how many tracks start with this prefix
                let matching_count = all_tracks
                    .iter()
                    .filter(|title| title.starts_with(&prefix))
                    .count();

                // If at least 75% of tracks match, use this prefix
                if matching_count * 4 >= all_tracks.len() * 3 {
                    return Some(prefix);
                }
            }
        }

        None
    }
}
