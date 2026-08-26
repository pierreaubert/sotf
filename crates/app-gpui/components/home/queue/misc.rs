use crate::app::i18n::{ContextMenuTranslations, LevelMeterTranslations, PhoneTranslations};
use crate::app::types::{MeterDisplayMode, Screen};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::queue_render::queue_meters_panel_width;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Button, ButtonSet, ButtonSetOption, ButtonSetSize,
    ButtonSize, ButtonVariant, CollapseDirection, IconButton, IconButtonSize, IconButtonVariant,
    PaneDivider, PaneDividerTheme, StackSpacing, Text, TextSize, VStack,
};
use sotf_audio_player::Track;
use std::collections::BTreeMap;

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

impl PlayerView {
    pub(crate) fn render_queue_screen(
        &self,
        solved_queue_width: Option<f32>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let layout = state.layout.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let meter_text = LevelMeterTranslations::for_language(state.app.ui_state.language);

        let meters_ratio = layout.meters_panel_ratio;
        let meter_display_mode = state.app.level_meters.display_mode;
        let window_height = state.app.ui_state.window_height;
        let window_width = state.app.ui_state.window_width;
        let hide_meters_for_rack = state.app.layout.hide_queue_meters_for_rack;
        // Use the solved queue slot, rather than reconstructing its bounds from
        // persisted ratios. The solver accounts for panel minimums, collapsed
        // panels, the app shell/sidebar, and responsive layout adjustments.
        let available_queue_width = solved_queue_width.unwrap_or_else(|| {
            crate::ui::layout_tree::solve_app_layout(window_width, window_height, &layout)
                .find("queue")
                .filter(|slot| slot.visible)
                .map_or(0.0, |slot| slot.width)
        });

        // When the meters panel is tall enough, show both LUFS and Meters stacked
        // (no toggle switch needed). The queue panel height depends on the layout ratio.
        let meters_panel_tall = window_height > 700.0;

        // Hide meters when: explicitly collapsed, OR rack is visible in 3-panel layout
        let meters_collapsed = meters_ratio < 0.05 || hide_meters_for_rack;
        let lufs_ratio = layout.lufs_panel_ratio;
        let level_meters_collapsed = lufs_ratio >= 0.90;

        let divider_theme = PaneDividerTheme {
            background: theme.background,
            background_hover: theme.surface_hover,
            background_collapsed: theme.surface,
            foreground: theme.text_muted,
            foreground_hover: theme.text_secondary,
            border: theme.border,
            tint: crate::theme::Theme::with_opacity(theme.accent, 0.42),
            tint_hover: theme.accent,
        };

        div()
            .flex()
            .size_full()
            .overflow_hidden()
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                let (
                    is_dragging_meters,
                    is_dragging_lufs,
                    anchor_pos,
                    anchor_meters_ratio,
                    anchor_lufs_ratio,
                ) = {
                    let state = view.state.read(cx);
                    let layout = state.layout.read(cx);
                    (
                        layout.is_dragging_meters_divider,
                        layout.is_dragging_lufs_divider,
                        layout.drag_anchor_pos,
                        layout.drag_anchor_meters_ratio,
                        layout.drag_anchor_lufs_ratio,
                    )
                };

                if is_dragging_meters {
                    let window_width: f32 = window.bounds().size.width.into();
                    if window_width > 0.0 {
                        let mouse_x: f32 = event.position.x.into();
                        let dx = mouse_x - anchor_pos;
                        let new_ratio = (anchor_meters_ratio - dx / window_width).clamp(0.10, 0.60);
                        view.state.update(cx, |state, cx| {
                            state.layout.update(cx, |layout, _| {
                                layout.meters_panel_ratio = new_ratio;
                            });
                        });
                    }
                }

                if is_dragging_lufs {
                    let window_height: f32 = window.bounds().size.height.into();
                    if window_height > 0.0 {
                        let mouse_y: f32 = event.position.y.into();
                        let dy = mouse_y - anchor_pos;
                        let new_ratio = (anchor_lufs_ratio + dy / window_height).clamp(0.20, 0.82);
                        view.state.update(cx, |state, cx| {
                            state.layout.update(cx, |layout, _| {
                                layout.lufs_panel_ratio = new_ratio;
                            });
                        });
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, cx| {
                        state.layout.update(cx, |layout, _| {
                            if layout.is_dragging_meters_divider || layout.is_dragging_lufs_divider
                            {
                                layout.is_dragging_meters_divider = false;
                                layout.is_dragging_lufs_divider = false;
                                if let Err(e) = state.app.save_config(layout) {
                                    log::warn!("Failed to save panel layout: {}", e);
                                }
                            }
                        });
                    });
                }),
            )
            // Queue pane: album accordion with expanded album details
            .child(self.render_queue_accordion_pane(&translations, cx))
            // Separator (Queue <-> Right meters)
            .child({
                PaneDivider::vertical("meters-divider", CollapseDirection::Right)
                    .label(meter_text.meters)
                    .collapsed(meters_collapsed)
                    .theme(divider_theme.clone())
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
                        move |pos, _window, cx| {
                            state_handle.update(cx, |state, cx| {
                                state.layout.update(cx, |layout, _| {
                                    layout.is_dragging_meters_divider = true;
                                    layout.drag_anchor_pos = pos;
                                    layout.drag_anchor_meters_ratio = layout.meters_panel_ratio;
                                });
                            });
                        }
                    })
            })
            // Right panel: LUFS and/or Level meters
            .when(!meters_collapsed, |el| {
                let state_entity = self.state.clone();

                // Use meters_panel_ratio to control width (resizable via divider drag)
                let panel_width = queue_meters_panel_width(meters_ratio, available_queue_width);

                el.child(
                    div()
                        .w(px(panel_width))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .h_full()
                        // Toggle header: only show when not tall enough to display both
                        .when(!meters_panel_tall, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .justify_center()
                                    .p(d.pad_y)
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(
                                        ButtonSet::new("meter-display-mode")
                                            .options(vec![
                                                ButtonSetOption::new("lufs", meter_text.lufs),
                                                ButtonSetOption::new("levels", meter_text.meters),
                                            ])
                                            .selected(match meter_display_mode {
                                                MeterDisplayMode::Lufs => "lufs",
                                                MeterDisplayMode::Levels => "levels",
                                            })
                                            .size(ButtonSetSize::Sm)
                                            .theme(theme.to_button_set_theme())
                                            .on_change(move |value, _window, cx| {
                                                state_entity.update(cx, |state, cx| {
                                                    let mode = if value.as_ref() == "levels" {
                                                        state.app.update_level_meter_groups();
                                                        MeterDisplayMode::Levels
                                                    } else {
                                                        MeterDisplayMode::Lufs
                                                    };
                                                    state.app.level_meters.display_mode = mode;
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            )
                        })
                        // Content: show both stacked when tall, or toggled when short
                        .when(meters_panel_tall, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .overflow_hidden()
                                    // LUFS panel on top
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .when(level_meters_collapsed, |el| el.flex_1())
                                            .when(!level_meters_collapsed, |el| {
                                                el.h(relative(lufs_ratio.clamp(0.20, 0.82)))
                                                    .flex_shrink_0()
                                            })
                                            .child(self.render_lufs_panel(cx)),
                                    )
                                    .child(
                                        PaneDivider::horizontal(
                                            "lufs-levels-divider",
                                            CollapseDirection::Down,
                                        )
                                        .label(meter_text.level_meters)
                                        .collapsed(level_meters_collapsed)
                                        .theme(divider_theme.clone())
                                        .on_toggle({
                                            let state_handle = self.state.clone();
                                            move |collapsed, _window, cx| {
                                                state_handle.update(cx, |state, cx| {
                                                    state.layout.update(cx, |layout, _| {
                                                        layout.lufs_panel_ratio =
                                                            if collapsed { 0.95 } else { 0.35 };
                                                        if let Err(e) =
                                                            state.app.save_config(layout)
                                                        {
                                                            log::debug!("Config save failed: {e}");
                                                        }
                                                    });
                                                });
                                            }
                                        })
                                        .on_drag_start({
                                            let state_handle = self.state.clone();
                                            move |pos, _window, cx| {
                                                state_handle.update(cx, |state, cx| {
                                                    state.layout.update(cx, |layout, _| {
                                                        layout.is_dragging_lufs_divider = true;
                                                        layout.drag_anchor_pos = pos;
                                                        layout.drag_anchor_lufs_ratio = layout
                                                            .lufs_panel_ratio
                                                            .clamp(0.20, 0.82);
                                                    });
                                                });
                                            }
                                        }),
                                    )
                                    // Level meters below, centered
                                    .when(!level_meters_collapsed, |el| {
                                        el.child(
                                            div()
                                                .flex_1()
                                                .overflow_hidden()
                                                .child(self.render_meters_panel(cx)),
                                        )
                                    }),
                            )
                        })
                        .when(!meters_panel_tall, |el| {
                            el.child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .when(meter_display_mode == MeterDisplayMode::Lufs, |el| {
                                        el.child(self.render_lufs_panel(cx))
                                    })
                                    .when(meter_display_mode == MeterDisplayMode::Levels, |el| {
                                        el.child(self.render_meters_panel(cx))
                                    }),
                            )
                        }),
                )
            })
    }

    // Level meter methods (render_lufs_panel, render_meters_panel, render_meter_group, etc.)
    // are now in ui/components/plugins/level_meters.rs

    pub(super) fn render_queue_accordion_pane(
        &self,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        // Extract only the tiny bits of queue data needed to build the
        // accordion. Previously this cloned the entire `Vec<QueueItem>`
        // (including every Track in every album) on every render.
        let (
            theme,
            queue_len,
            can_undo_clear,
            can_undo_remove,
            expanded_idx,
            summaries,
            max_title_chars,
        ) = {
            let state = self.state.read(cx);
            let layout = state.layout.read(cx);
            let queue_len = state.app.queue_state.len();
            let can_undo_clear = state.app.queue_state.can_undo_clear();
            let can_undo_remove = state.app.queue_state.can_undo_remove();
            let selected_idx = if queue_len == 0 {
                None
            } else {
                Some(state.app.queue_state.selected_index.min(queue_len - 1))
            };
            let fallback_expanded_idx = state
                .app
                .playback
                .current_queue_index
                .filter(|idx| *idx < queue_len)
                .or(selected_idx);
            let expanded_idx = if state.app.ui_state.queue_expansion_overridden {
                state
                    .app
                    .ui_state
                    .queue_expanded_album
                    .filter(|idx| *idx < queue_len)
            } else {
                fallback_expanded_idx
            };

            let theme = state.app.ui_state.theme.clone();
            let summaries = crate::queue_render::queue_accordion_summaries(&state.app.queue_state);
            let max_title_chars = state.app.max_chars_queue_list_title(layout);

            (
                theme,
                queue_len,
                can_undo_clear,
                can_undo_remove,
                expanded_idx,
                summaries,
                max_title_chars,
            )
        };

        let accordion_items = summaries
            .into_iter()
            .map(|summary| {
                let title = if summary.title.chars().count() > max_title_chars {
                    summary
                        .title
                        .chars()
                        .take(max_title_chars)
                        .collect::<String>()
                        + "..."
                } else {
                    summary.title
                };

                AccordionItem::new(format!("queue-album-{}", summary.idx), title)
                    .trailing(summary.track_position)
                    .content(self.render_queue_album_detail(summary.idx, translations, cx))
            })
            .collect::<Vec<_>>();

        let expanded_ids: Vec<SharedString> = expanded_idx
            .map(|idx| SharedString::from(format!("queue-album-{idx}")))
            .into_iter()
            .collect();

        let accordion_theme = theme.to_accordion_theme();
        let state_handle = self.state.clone();
        let state_for_home = self.state.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .bg(theme.background_secondary)
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(d.card)
                    .py(d.pad_y)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        IconButton::with_child(
                            "queue-home-button",
                            Icon::new(IconName::Home).color(theme.text_muted),
                        )
                        .variant(IconButtonVariant::Ghost)
                        .size(IconButtonSize::Sm)
                        .theme(theme.to_icon_button_theme())
                        .aria_label(translations.screen_library)
                        .on_click_event(move |_event, _window, cx| {
                            state_for_home.update(cx, |state, _cx| {
                                state.app.ui_state.current_screen = Screen::Library;
                            });
                        }),
                    )
                    .child(
                        div()
                            .ml(d.gap)
                            .flex_1()
                            .min_w_0()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(format!(
                                "{} ({} {})",
                                translations.queue_title, queue_len, translations.queue_albums
                            )),
                    )
                    .child(self.render_magic_radio_button(cx).into_any_element())
                    .child(
                        dev_track!(
                            Button::new(
                                "queue-save-as-playlist",
                                translations.queue_save_as_playlist,
                            )
                            .size(ButtonSize::Xs)
                            .variant(ButtonVariant::Secondary)
                            .theme(theme.to_button_theme())
                            .disabled(queue_len == 0)
                            .on_click_event(cx.listener(
                                |view, _event: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, _| {
                                        state.app.playlist.name_input.clear();
                                        state.app.playlist.dialog =
                                            crate::app::state::app::PlaylistDialog::CreateFromQueue;
                                        state.app.playlist.error = None;
                                        state.app.ui_state.current_screen = Screen::Playlists;
                                    });
                                    cx.notify();
                                },
                            )),
                            "queue.save_as_playlist"
                        )
                        .into_any_element(),
                    )
                    .child(
                        dev_track!(
                            Button::new("queue-clear", translations.queue_clear)
                                .size(ButtonSize::Xs)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .disabled(queue_len == 0)
                                .on_click_event(cx.listener(
                                    |view, _event: &ClickEvent, _window, cx| {
                                        view.state.update(cx, |state, _| {
                                            state.app.clear_queue();
                                        });
                                        cx.notify();
                                    },
                                )),
                            "queue.clear"
                        )
                        .into_any_element(),
                    )
                    .child(
                        dev_track!(
                            Button::new("queue-undo-clear", translations.queue_undo_clear)
                                .size(ButtonSize::Xs)
                                .variant(ButtonVariant::Secondary)
                                .theme(theme.to_button_theme())
                                .disabled(!can_undo_clear)
                                .on_click_event(cx.listener(
                                    |view, _event: &ClickEvent, _window, cx| {
                                        view.state.update(cx, |state, _| {
                                            let _ = state.app.undo_clear_queue();
                                        });
                                        cx.notify();
                                    },
                                )),
                            "queue.undo_clear"
                        )
                        .into_any_element(),
                    ),
            )
            .child(
                div()
                    .id("queue-accordion-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .when(queue_len == 0, |el| {
                        el.flex().items_center().justify_center().child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    Text::new(translations.queue_empty)
                                        .size(TextSize::Md)
                                        .color(theme.text_muted),
                                )
                                .child(Text::caption(translations.queue_select_album))
                                .build(),
                        )
                    })
                    .when(queue_len > 0, |el| {
                        el.child(
                            Accordion::new()
                                .items(accordion_items)
                                .mode(AccordionMode::Single)
                                .expanded(expanded_ids)
                                .bordered(false)
                                .rounded(false)
                                .theme(accordion_theme)
                                .aria_label(translations.queue_title)
                                .on_change(move |id, is_expanded, _window, cx| {
                                    let id = id.to_string();
                                    let Some(idx) = id
                                        .strip_prefix("queue-album-")
                                        .and_then(|suffix| suffix.parse::<usize>().ok())
                                    else {
                                        return;
                                    };

                                    state_handle.update(cx, |state, _cx| {
                                        if state.app.queue_state.get(idx).is_none() {
                                            return;
                                        }

                                        if is_expanded {
                                            state.app.ui_state.queue_expanded_album = Some(idx);
                                            state.app.ui_state.queue_expansion_overridden = true;
                                            state.app.queue_state.selected_index = idx;
                                        } else if state.app.ui_state.queue_expanded_album
                                            == Some(idx)
                                            || state.app.ui_state.queue_expanded_album.is_none()
                                        {
                                            state.app.ui_state.queue_expanded_album = None;
                                            state.app.ui_state.queue_expansion_overridden = true;
                                        }
                                    });
                                }),
                        )
                    }),
            )
            .child(
                dev_track!(
                    Button::new("queue-undo-remove", translations.queue_undo_remove)
                        .size(ButtonSize::Xs)
                        .variant(ButtonVariant::Secondary)
                        .theme(theme.to_button_theme())
                        .disabled(!can_undo_remove)
                        .on_click_event(cx.listener(|view, _event: &ClickEvent, _window, cx| {
                            view.state.update(cx, |state, cx| {
                            let effect = state.app.undo_remove_from_queue();
                            match effect {
                                sotf_audio_player::QueuePlaybackEffect::Reload(source)
                                | sotf_audio_player::QueuePlaybackEffect::Play(source) => {
                                    PlayerView::play_track(state, source);
                                }
                                sotf_audio_player::QueuePlaybackEffect::Stop => {
                                    if let Err(error) = state.player.stop() {
                                        log::warn!(
                                            "[UI] Failed to stop player after queue undo: {error}"
                                        );
                                    }
                                }
                                sotf_audio_player::QueuePlaybackEffect::None => {}
                            }
                            cx.notify();
                        });
                        })),
                    "queue.undo_remove"
                )
                .into_any_element(),
            )
    }

    pub(super) fn render_magic_radio_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = PhoneTranslations::for_language(state.app.ui_state.language);

        dev_track!(
            Button::new("magic-radio-btn", text.magic_radio)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(|view, _event: &ClickEvent, _window, cx| {
                    log::info!("[Queue] Magic Radio button clicked");
                    view.state
                        .update(cx, |state, _cx| match state.app.fill_queue_magic() {
                            Ok(count) => {
                                log::info!("[Queue] Magic Radio added {} tracks", count);
                            }
                            Err(e) => {
                                log::error!("[Queue] Magic Radio error: {}", e);
                            }
                        });
                    cx.notify();
                })),
            "queue.magic_radio"
        )
        .into_any_element()
    }

    pub(super) fn render_queue_album_detail(
        &self,
        queue_idx: usize,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let layout = state.layout.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let theme_for_closure = theme.clone();
        let translations = translations.clone();
        let remove_label =
            ContextMenuTranslations::for_language(state.app.ui_state.language).remove_from_queue;
        let queue_len = state.app.queue_state.len();
        let state_for_move_up = self.state.clone();
        let state_for_move_down = self.state.clone();
        let state_for_remove = self.state.clone();
        let is_active_queue_album = state.app.playback.current_queue_index == Some(queue_idx);

        if let Some(item) = state.app.queue_state.get(queue_idx) {
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

            // Adaptive album art: collapse entirely below a width threshold,
            // shrink in the medium range, full size when wide.
            let center_w = state.app.center_panel_width(layout);
            let art_size_rems: Option<f32> = if center_w < 220.0 {
                None
            } else if center_w < 380.0 {
                Some(4.5)
            } else {
                Some(7.5)
            };

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
                // Top row: Album art (left) + Album info (right)
                .child(
                    div()
                        .flex()
                        .gap(d.section)
                        .mb(d.section)
                        // Album art (omitted entirely when the centre panel is too narrow)
                        .when_some(art_size_rems, |row, size_rems| {
                            row.child({
                                let art_div = div()
                                    .w(rems(size_rems))
                                    .h(rems(size_rems))
                                    .bg(theme.surface)
                                    .rounded(d.r_lg)
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
                                        .text_size(d.text_lg)
                                        .child("♪")
                                }
                            })
                        })
                        // Album info (right of art)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(d.grid)
                                .flex_1()
                                .min_w_0()
                                // Album title
                                .child(
                                    div()
                                        .text_size(d.text_lg)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(album_title.clone()),
                                )
                                // Artist
                                .child(
                                    div()
                                        .text_size(d.text_sm)
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
                                        .gap(d.gap)
                                        .mt(d.gap)
                                        .child(
                                            div()
                                                .text_size(d.text_xs)
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.text_secondary)
                                                .child(format!("{} {}/{}", file_type, bit_depth, sample_rate_str)),
                                        )
                                        .child(
                                            div()
                                                .text_size(d.text_xs)
                                                .text_color(theme.text_muted)
                                                .child(format!("#{}", track_count)),
                                        )
                                        .child(
                                            div()
                                                .text_size(d.text_xs)
                                                .text_color(theme.text_muted)
                                                .child(translations.queue_replay_gain),
                                        )
                                        .child(
                                            div()
                                                .text_size(d.text_xs)
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
                                                .text_size(d.text_xs)
                                                .text_color(theme.text_muted)
                                                .child(translations.queue_channels),
                                        )
                                        .child(
                                            div()
                                                .text_size(d.text_xs)
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text_secondary)
                                                .child(format!("{}", channels)),
                                        )
                                        // Album favorite heart
                                        .when_some(album_id, |el, aid| {
                                            el.child(
                                                div()
                                                    .id("album-heart")
                                                    .cursor_pointer()
                                                    .ml(d.gap)
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
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(d.gap)
                                .child(
                                    dev_track!(
                                        IconButton::with_child(
                                        ("queue-move-up", queue_idx),
                                        Icon::new(IconName::ChevronUp).color(theme.text_muted),
                                    )
                                    .variant(IconButtonVariant::Ghost)
                                    .size(IconButtonSize::Sm)
                                    .theme(theme.to_icon_button_theme())
                                    .aria_label(translations.queue_move_up)
                                    .disabled(queue_idx == 0)
                                    .on_click_event(move |_event, _window, cx| {
                                        state_for_move_up.update(cx, |state, _| {
                                            if queue_idx > 0 {
                                                state.app.move_queue_item(queue_idx, queue_idx - 1);
                                            }
                                        });
                                    }),
                                        format!("queue.move_up.{queue_idx}")
                                    )
                                    .into_any_element(),
                                )
                                .child(
                                    dev_track!(
                                        IconButton::with_child(
                                        ("queue-move-down", queue_idx),
                                        Icon::new(IconName::ChevronDown).color(theme.text_muted),
                                    )
                                    .variant(IconButtonVariant::Ghost)
                                    .size(IconButtonSize::Sm)
                                    .theme(theme.to_icon_button_theme())
                                    .aria_label(translations.queue_move_down)
                                    .disabled(queue_idx + 1 >= queue_len)
                                    .on_click_event(move |_event, _window, cx| {
                                        state_for_move_down.update(cx, |state, _| {
                                            state.app.move_queue_item(queue_idx, queue_idx + 1);
                                        });
                                    }),
                                        format!("queue.move_down.{queue_idx}")
                                    )
                                    .into_any_element(),
                                )
                                .child(
                                    dev_track!(
                                        Button::new(
                                            ("queue-remove", queue_idx),
                                            remove_label,
                                        )
                                        .size(ButtonSize::Xs)
                                        .variant(ButtonVariant::Ghost)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(move |_event, _window, cx| {
                                            state_for_remove.update(cx, |state, cx| {
                                                let effect = state.app.remove_from_queue(queue_idx);
                                                match effect {
                                                    sotf_audio_player::QueuePlaybackEffect::Reload(source)
                                                    | sotf_audio_player::QueuePlaybackEffect::Play(source) => {
                                                        PlayerView::play_track(state, source);
                                                    }
                                                    sotf_audio_player::QueuePlaybackEffect::Stop => {
                                                        if let Err(error) = state.player.stop() {
                                                            log::warn!(
                                                                "[Queue] Failed to stop player after queue removal: {error}"
                                                            );
                                                        }
                                                    }
                                                    sotf_audio_player::QueuePlaybackEffect::None => {}
                                                }
                                                cx.notify();
                                            });
                                        }),
                                        format!("queue.remove.{queue_idx}")
                                    )
                                    .into_any_element(),
                                ),
                        )
                        .child(self.render_track_list(
                    queue_idx,
                    &disc_map,
                    current_track_idx,
                    is_active_queue_album,
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
                .child(Text::caption(translations.queue_select_album))
                .build()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .into_any_element()
        }
    }

    /// Render the track list with clickable items
    pub(super) fn render_track_list(
        &self,
        queue_idx: usize,
        disc_map: &BTreeMap<u32, Vec<(usize, Track)>>,
        current_track_idx: usize,
        highlight_current: bool,
        album_title: &str,
        translations: &crate::i18n::Translations,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
                        .py(d.pad_y_half)
                        .mt(d.gap)
                        .mb(d.grid)
                        .text_size(d.text_xs)
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
                let is_current = highlight_current && idx == current_track_idx;
                let duration_str = format!("{}:{:02}", duration / 60, duration % 60);
                let theme_c = theme.clone();
                let target_queue_idx = queue_idx;
                let track_play_count = track.play_count;
                let track_is_favorite = track.is_favorite;
                let heart_track_path = track.path.clone();

                all_elements.push(
                    div()
                        .id(SharedString::from(format!("track-{}", idx)))
                        .flex()
                        .items_center()
                        .gap(d.gap)
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .rounded(d.r_md)
                        .cursor_pointer()
                        .when(is_current, |el| {
                            el.bg(theme_c.accent).text_color(theme_c.text_on_accent)
                        })
                        .when(!is_current, |el| el.hover(|s| s.bg(theme_c.surface_hover)))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    let current_channels = state
                                        .app
                                        .playback
                                        .current_queue_index
                                        .and_then(|queue_idx| state.app.queue_state.get(queue_idx))
                                        .and_then(|item| item.current_track())
                                        .and_then(|track| track.channels)
                                        .unwrap_or(2)
                                        as usize;
                                    let current_queue_idx = state.app.playback.current_queue_index;
                                    let current_track_idx = current_queue_idx
                                        .and_then(|queue_idx| state.app.queue_state.get(queue_idx))
                                        .map(|item| item.current_track_index);
                                    let target =
                                        state.app.queue_state.get_mut(target_queue_idx).and_then(
                                            |item| {
                                                if idx < item.album.tracks.len() {
                                                    item.current_track_index = idx;
                                                    item.current_track().map(|track| {
                                                        (
                                                            track.audio_source(),
                                                            track.channels.unwrap_or(2) as usize,
                                                        )
                                                    })
                                                } else {
                                                    None
                                                }
                                            },
                                        );

                                    if let Some((source, target_channels)) = target {
                                        let prefer_smooth_switch = state.app.playback.is_playing
                                            && (current_queue_idx != Some(target_queue_idx)
                                                || current_track_idx != Some(idx))
                                            && current_channels == target_channels;
                                        state.app.queue_state.selected_index = target_queue_idx;
                                        state.app.queue_state.current_index =
                                            Some(target_queue_idx);
                                        state.app.playback.current_queue_index =
                                            Some(target_queue_idx);
                                        if prefer_smooth_switch {
                                            Self::play_track_smooth(state, source);
                                        } else {
                                            Self::play_track(state, source);
                                        }
                                    }
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .w(rems(1.5))
                                .text_size(d.text_xs)
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
                                .text_size(d.text_sm)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .when(is_current, |d| d.font_weight(FontWeight::SEMIBOLD))
                                .child(title),
                        )
                        // Play count (only if > 0)
                        .when(track_play_count > 0, |el| {
                            el.child(
                                div()
                                    .text_size(d.text_xs)
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
                                .text_size(d.text_xs)
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

        div().flex().flex_col().child(
            div()
                .id("track-list")
                .flex()
                .flex_col()
                .gap_0p5()
                .children(all_elements),
        )
    }

    /// Find a common prefix in track titles that matches part of the album title.
    /// For albums like "Monteverdi: Vespro della Beata Vergine", tracks often start with
    /// "Vespro della Beata Vergine" which should be stripped.
    pub(super) fn find_common_track_prefix(
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
