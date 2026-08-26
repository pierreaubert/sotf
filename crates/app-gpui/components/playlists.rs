//! Playlist browser backed by the shared `PlaylistController`.

use crate::app::i18n::PlaylistTranslations;
use crate::app::state::app::PlaylistDialog;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Heading, Input, InputSize, StackSpacing, Text,
    TextSize, TextWeight, VStack,
};

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

impl PlayerView {
    pub(crate) fn render_playlists_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let text = PlaylistTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (theme, playlists, active_playlist, name, dialog, error, can_undo_delete, needs_load) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.playlist.controller.playlists().to_vec(),
                state.app.playlist.controller.active_playlist().cloned(),
                state.app.playlist.name_input.clone(),
                state.app.playlist.dialog,
                state.app.playlist.error.clone(),
                state.app.playlist.deleted_playlist.is_some(),
                !state.app.playlist.loaded,
            )
        };
        let view = cx.entity().clone();
        if needs_load {
            let load_view = view.clone();
            cx.defer(move |cx| {
                load_view.update(cx, |this, cx| {
                    this.state.update(cx, |state, _| {
                        let app = &mut state.app;
                        app.playlist.loaded = true;
                        let result = app
                            .library_state
                            .library
                            .get_database()
                            .ok_or_else(|| text.library_database_unavailable().to_string())
                            .and_then(|db| app.playlist.controller.load_playlists(db));
                        app.playlist.error = result.err();
                    });
                    cx.notify();
                });
            });
        }
        let mut result = div()
            .id("playlists-screen")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .p(d.card)
            .gap(d.section_lg)
            .bg(theme.background)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new(text.title)
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(dev_track!(
                        Button::new("playlist-import", text.import_m3u8)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click({
                                let import_view = view.clone();
                                move |_, cx| {
                                    import_view.update(cx, |this, cx| this.import_playlist(cx));
                                }
                            }),
                        "playlist.import"
                    ))
                    .child(dev_track!(
                        Button::new("playlist-create", text.new_playlist)
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click({
                                let view = view.clone();
                                move |_, cx| {
                                    view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state.app.playlist.name_input.clear();
                                            state.app.playlist.dialog = PlaylistDialog::Create;
                                            state.app.playlist.error = None;
                                        });
                                        cx.notify();
                                    });
                                }
                            }),
                        "playlist.create"
                    )),
            );
        if let Some(error) = error {
            result = result.child(Text::new(error).size(TextSize::Xs).color(theme.error));
        }
        if can_undo_delete {
            let undo_view = view.clone();
            result = result.child(dev_track!(
                Button::new("playlist-undo-delete", text.undo_delete)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click(move |_, cx| {
                        undo_view.update(cx, |this, cx| this.undo_playlist_delete(cx));
                    }),
                "playlist.undo_delete"
            ));
        }
        if matches!(
            dialog,
            PlaylistDialog::Create | PlaylistDialog::CreateFromQueue | PlaylistDialog::Rename
        ) {
            let input_view = view.clone();
            let save_view = view.clone();
            let cancel_view = view.clone();
            result = result.child(
                Card::new().content(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(
                            Text::new(match dialog {
                                PlaylistDialog::Create => text.create_playlist,
                                PlaylistDialog::CreateFromQueue => text.save_queue_as_playlist,
                                PlaylistDialog::Rename => text.rename_playlist,
                                _ => unreachable!("only editable playlist dialogs are rendered"),
                            })
                            .weight(TextWeight::Bold),
                        )
                        .child(dev_track!(
                            Input::new("playlist-name")
                                .value(name)
                                .placeholder(text.name_placeholder)
                                .size(InputSize::Sm)
                                .on_text_change(move |value, _, cx| {
                                    input_view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state.app.playlist.name_input = value.to_string()
                                        });
                                    });
                                }),
                            "playlist.name_input"
                        ))
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(dev_track!(
                                    Button::new("playlist-save", text.save)
                                        .variant(ButtonVariant::Primary)
                                        .size(ButtonSize::Sm)
                                        .theme(theme.to_button_theme())
                                        .on_click(move |_, cx| {
                                            save_view.update(cx, |this, cx| {
                                                this.save_playlist_dialog(cx)
                                            });
                                        }),
                                    "playlist.save"
                                ))
                                .child(dev_track!(
                                    Button::new("playlist-cancel", text.cancel)
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Sm)
                                        .theme(theme.to_button_theme())
                                        .on_click(move |_, cx| {
                                            cancel_view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state.app.playlist.dialog = PlaylistDialog::None
                                                });
                                                cx.notify();
                                            });
                                        }),
                                    "playlist.cancel"
                                )),
                        ),
                ),
            );
        }
        if dialog == PlaylistDialog::ConfirmDelete {
            let confirm_view = view.clone();
            let cancel_view = view.clone();
            result = result.child(
                Card::new().content(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(Text::new(text.delete_confirmation).color(theme.error))
                        .child(dev_track!(
                            Button::new("playlist-delete-confirm", text.delete)
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click(move |_, cx| {
                                    confirm_view
                                        .update(cx, |this, cx| this.delete_selected_playlist(cx));
                                }),
                            "playlist.delete_confirm"
                        ))
                        .child(dev_track!(
                            Button::new("playlist-delete-cancel", text.cancel)
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click(move |_, cx| {
                                    cancel_view.update(cx, |this, cx| {
                                        this.state.update(cx, |state, _| {
                                            state.app.playlist.dialog = PlaylistDialog::None
                                        });
                                        cx.notify();
                                    });
                                }),
                            "playlist.delete_cancel"
                        )),
                ),
            );
        }
        if playlists.is_empty() {
            result = result.child(
                Card::new().content(Text::new(text.empty_state).color(theme.text_secondary)),
            );
        }
        for (index, playlist) in playlists.into_iter().enumerate() {
            let open_view = view.clone();
            let rename_view = view.clone();
            let name = playlist.name;
            let count = playlist.entries.len();
            result = result.child(
                Card::new().content(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(
                            div()
                                .flex_1()
                                .child(Text::new(name.clone()).weight(TextWeight::Bold))
                                .child(
                                    Text::new(text.tracks(count))
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                ),
                        )
                        .child(dev_track!(
                            Button::new(
                                SharedString::from(format!("playlist-open-{index}")),
                                text.open,
                            )
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .on_click(move |_, cx| {
                                open_view.update(cx, |this, cx| this.open_playlist(index, cx));
                            }),
                            format!("playlist.open.{index}")
                        ))
                        .child(dev_track!(
                            Button::new(
                                SharedString::from(format!("playlist-rename-{index}")),
                                text.rename,
                            )
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .on_click(move |_, cx| {
                                rename_view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state.app.playlist.controller.selected_playlist_index =
                                            index;
                                        state.app.playlist.name_input = name.clone();
                                        state.app.playlist.dialog = PlaylistDialog::Rename;
                                    });
                                    cx.notify();
                                });
                            }),
                            format!("playlist.rename.{index}")
                        )),
                ),
            );
        }
        if let Some(playlist) = active_playlist {
            let delete_view = view.clone();
            let queue_view = view.clone();
            let play_view = view.clone();
            let playlist_is_empty = playlist.entries.is_empty();
            result = result.child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(dev_track!(
                        Button::new(
                            "playlist-add-library-album",
                            text.add_selected_library_album,
                        )
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
                        .on_click({
                            let add_library_view = view.clone();
                            move |_, cx| {
                                add_library_view.update(cx, |this, cx| {
                                    this.add_selected_library_album_to_playlist(cx)
                                });
                            }
                        }),
                        "playlist.add_library_album"
                    ))
                    .child(dev_track!(
                        Button::new("playlist-add-queue-album", text.add_selected_queue_album)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click({
                                let add_queue_view = view.clone();
                                move |_, cx| {
                                    add_queue_view.update(cx, |this, cx| {
                                        this.add_selected_queue_album_to_playlist(cx)
                                    });
                                }
                            }),
                        "playlist.add_queue_album"
                    ))
                    .child(dev_track!(
                        Button::new("playlist-queue-active", text.queue)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .disabled(playlist_is_empty)
                            .on_click(move |_, cx| {
                                queue_view
                                    .update(cx, |this, cx| this.enqueue_active_playlist(false, cx));
                            }),
                        "playlist.queue_active"
                    ))
                    .child(dev_track!(
                        Button::new("playlist-play-active", text.play)
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .disabled(playlist_is_empty)
                            .on_click(move |_, cx| {
                                play_view
                                    .update(cx, |this, cx| this.enqueue_active_playlist(true, cx));
                            }),
                        "playlist.play_active"
                    )),
            );
            result = result.child(dev_track!(
                Button::new("playlist-export-active", text.export_m3u8)
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click({
                        let export_view = view.clone();
                        move |_, cx| {
                            export_view.update(cx, |this, cx| this.export_active_playlist(cx));
                        }
                    }),
                "playlist.export_active"
            ));
            result = result.child(dev_track!(
                Button::new("playlist-delete-active", text.delete_playlist)
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click(move |_, cx| {
                        delete_view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                state.app.playlist.dialog = PlaylistDialog::ConfirmDelete;
                            });
                            cx.notify();
                        });
                    }),
                "playlist.delete_active"
            ));
            result = result.child(Heading::h4(playlist.name));
            if playlist.entries.is_empty() {
                result = result.child(
                    Text::new(text.empty_playlist)
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                );
            }
            for (index, entry) in playlist.entries.into_iter().enumerate() {
                let up_view = view.clone();
                let down_view = view.clone();
                let remove_view = view.clone();
                result = result.child(
                    Card::new().content(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new(format!("{}. {}", index + 1, entry.track_path.display()))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new(
                                    SharedString::from(format!("playlist-track-up-{index}")),
                                    text.up,
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .disabled(index == 0)
                                .on_click(move |_, cx| {
                                    up_view.update(cx, |this, cx| {
                                        this.move_playlist_track(index, true, cx)
                                    });
                                }),
                            )
                            .child(
                                Button::new(
                                    SharedString::from(format!("playlist-track-down-{index}")),
                                    text.down,
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .on_click(move |_, cx| {
                                    down_view.update(cx, |this, cx| {
                                        this.move_playlist_track(index, false, cx)
                                    });
                                }),
                            )
                            .child(
                                Button::new(
                                    SharedString::from(format!("playlist-track-remove-{index}")),
                                    text.remove,
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .on_click(move |_, cx| {
                                    remove_view.update(cx, |this, cx| {
                                        this.remove_playlist_track(index, cx)
                                    });
                                }),
                            ),
                    ),
                );
            }
        }
        result
    }

    fn playlist_translations(&self, cx: &Context<Self>) -> PlaylistTranslations {
        PlaylistTranslations::for_language(self.state.read(cx).app.ui_state.language)
    }

    fn save_playlist_dialog(&mut self, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            let name = app.playlist.name_input.trim().to_string();
            let result = if name.is_empty() {
                Err(text.name_required().into())
            } else if let Some(db) = app.library_state.library.get_database() {
                match app.playlist.dialog {
                    PlaylistDialog::Create => app
                        .playlist
                        .controller
                        .create_playlist(db, &name, None)
                        .map(|_| ()),
                    PlaylistDialog::CreateFromQueue => {
                        let track_paths = app
                            .queue_state
                            .iter()
                            .flat_map(|item| {
                                item.album.tracks.iter().map(|track| track.path.clone())
                            })
                            .collect::<Vec<_>>();
                        app.playlist
                            .controller
                            .create_playlist_with_tracks(db, &name, None, &track_paths)
                            .map(|_| ())
                    }
                    PlaylistDialog::Rename => app.playlist.controller.rename_playlist(
                        db,
                        app.playlist.controller.selected_playlist_index,
                        &name,
                    ),
                    _ => Ok(()),
                }
            } else {
                Err(text.library_database_unavailable().into())
            };
            match result {
                Ok(()) => {
                    app.playlist.dialog = PlaylistDialog::None;
                    app.playlist.name_input.clear();
                    app.playlist.error = None;
                }
                Err(error) => app.playlist.error = Some(error),
            }
        });
        cx.notify();
    }

    fn open_playlist(&mut self, index: usize, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            let result = app
                .library_state
                .library
                .get_database()
                .ok_or_else(|| text.library_database_unavailable().to_string())
                .and_then(|db| app.playlist.controller.open_playlist(db, index));
            app.playlist.error = result.err();
        });
        cx.notify();
    }

    fn delete_selected_playlist(&mut self, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            let index = app.playlist.controller.selected_playlist_index;
            let result = app
                .library_state
                .library
                .get_database()
                .ok_or_else(|| text.library_database_unavailable().to_string())
                .and_then(|db| {
                    app.playlist.controller.open_playlist(db, index)?;
                    let playlist = app
                        .playlist
                        .controller
                        .active_playlist()
                        .cloned()
                        .ok_or_else(|| text.active_playlist_unavailable().to_string())?;
                    app.playlist.controller.delete_playlist(db, index)?;
                    Ok(crate::app::state::app::DeletedPlaylist {
                        name: playlist.name,
                        description: playlist.description,
                        track_paths: playlist
                            .entries
                            .into_iter()
                            .map(|entry| entry.track_path)
                            .collect(),
                    })
                });
            match result {
                Ok(deleted_playlist) => {
                    app.playlist.dialog = PlaylistDialog::None;
                    app.playlist.error = None;
                    app.playlist.deleted_playlist = Some(deleted_playlist);
                    app.ui_state.toast_message =
                        Some(crate::app::ToastMessage::success(text.deleted_with_undo()));
                }
                Err(error) => app.playlist.error = Some(error),
            }
        });
        cx.notify();
    }

    fn undo_playlist_delete(&mut self, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            let Some(deleted) = app.playlist.deleted_playlist.take() else {
                return;
            };
            let result = app
                .library_state
                .library
                .get_database()
                .ok_or_else(|| text.library_database_unavailable().to_string())
                .and_then(|db| {
                    app.playlist.controller.create_playlist(
                        db,
                        &deleted.name,
                        deleted.description.as_deref(),
                    )?;
                    let index = app.playlist.controller.selected_playlist_index;
                    app.playlist
                        .controller
                        .add_tracks_to_playlist(db, index, &deleted.track_paths)
                });
            match result {
                Ok(()) => {
                    app.playlist.error = None;
                    app.ui_state.toast_message =
                        Some(crate::app::ToastMessage::success(text.restored()));
                }
                Err(error) => {
                    app.playlist.deleted_playlist = Some(deleted);
                    app.playlist.error = Some(error);
                }
            }
        });
        cx.notify();
    }

    fn remove_playlist_track(&mut self, index: usize, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            app.playlist.controller.selected_track_index = index;
            let result = app
                .library_state
                .library
                .get_database()
                .ok_or_else(|| text.library_database_unavailable().to_string())
                .and_then(|db| app.playlist.controller.remove_track(db, index));
            app.playlist.error = result.err();
        });
        cx.notify();
    }

    fn move_playlist_track(&mut self, index: usize, up: bool, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            app.playlist.controller.selected_track_index = index;
            let result = app
                .library_state
                .library
                .get_database()
                .ok_or_else(|| text.library_database_unavailable().to_string())
                .and_then(|db| {
                    if up {
                        app.playlist.controller.move_track_up(db)
                    } else {
                        app.playlist.controller.move_track_down(db)
                    }
                });
            app.playlist.error = result.err();
        });
        cx.notify();
    }

    fn enqueue_active_playlist(&mut self, play_now: bool, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let source = {
                let app = &mut state.app;
                let track_paths = app.playlist.controller.active_track_paths();
                if track_paths.is_empty() {
                    app.playlist.error = Some(text.no_tracks_to_queue().to_string());
                    return;
                }

                let outcome = app
                    .queue_state
                    .enqueue_playlist_tracks(&app.library_state.library, &track_paths);
                if outcome.added == 0 {
                    app.playlist.error = Some(match outcome.skipped_existing {
                        0 => text.no_playlist_tracks_available().to_string(),
                        count => text.all_tracks_already_queued(count),
                    });
                    return;
                }

                app.playlist.error = if outcome.skipped_missing == 0 {
                    None
                } else {
                    Some(text.queued_with_missing(outcome.added, outcome.skipped_missing))
                };

                let source = if play_now {
                    outcome.first_added_index.and_then(|index| {
                        match app.queue_state.jump_to(index) {
                            sotf_audio_player::QueuePlaybackEffect::Play(source) => Some(source),
                            _ => None,
                        }
                    })
                } else {
                    None
                };
                source
            };

            if let Some(source) = source {
                Self::play_track(state, source);
            }
        });
        cx.notify();
    }

    fn import_playlist(&mut self, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        cx.spawn(async move |view: WeakEntity<PlayerView>, cx| {
            let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("M3U playlists", &["m3u", "m3u8"])
                .set_title(text.import_m3u8)
                .pick_file()
                .await
            else {
                return;
            };
            let path = file.path().to_path_buf();
            let _ = view.update(cx, |this, cx| {
                this.state.update(cx, |state, _| {
                    let app = &mut state.app;
                    let result = app
                        .library_state
                        .library
                        .get_database()
                        .ok_or_else(|| text.library_database_unavailable().to_string())
                        .and_then(|db| app.playlist.controller.import_playlist(db, &path));
                    match result {
                        Ok(()) => {
                            app.playlist.error = None;
                            app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::success(text.imported()));
                        }
                        Err(error) => app.playlist.error = Some(error),
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn export_active_playlist(&mut self, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        let name = self
            .state
            .read(cx)
            .app
            .playlist
            .controller
            .active_playlist()
            .map(|playlist| playlist.name.clone())
            .unwrap_or_else(|| "playlist".to_string());
        let file_name = format!("{name}.m3u8");
        cx.spawn(async move |view: WeakEntity<PlayerView>, cx| {
            let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("M3U8 playlist", &["m3u8"])
                .set_file_name(&file_name)
                .set_title(text.export_m3u8)
                .save_file()
                .await
            else {
                return;
            };
            let path = file.path().to_path_buf();
            let _ = view.update(cx, |this, cx| {
                this.state.update(cx, |state, _| {
                    let app = &mut state.app;
                    let result = app
                        .playlist
                        .controller
                        .export_playlist(&app.library_state.library, &path);
                    match result {
                        Ok(()) => {
                            app.playlist.error = None;
                            app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::success(text.exported()));
                        }
                        Err(error) => app.playlist.error = Some(error),
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn add_selected_library_album_to_playlist(&mut self, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            let album = app
                .filtered_albums()
                .get(app.library_state.selected_index)
                .cloned()
                .cloned();
            match album {
                Some(album) => Self::add_album_to_active_playlist(app, album),
                None => app.playlist.error = Some(text.select_library_album_first().to_string()),
            }
        });
        cx.notify();
    }

    fn add_selected_queue_album_to_playlist(&mut self, cx: &mut Context<Self>) {
        let text = self.playlist_translations(cx);
        self.state.update(cx, |state, _| {
            let app = &mut state.app;
            let album = app
                .queue_state
                .get(app.queue_state.selected_index)
                .map(|entry| entry.album.clone());
            match album {
                Some(album) => Self::add_album_to_active_playlist(app, album),
                None => app.playlist.error = Some(text.select_queue_album_first().to_string()),
            }
        });
        cx.notify();
    }

    fn add_album_to_active_playlist(app: &mut crate::app::App, album: sotf_audio_player::Album) {
        let text = PlaylistTranslations::for_language(app.ui_state.language);
        let playlist_index = app.playlist.controller.selected_playlist_index;
        let result = app
            .library_state
            .library
            .get_database()
            .ok_or_else(|| text.library_database_unavailable().to_string())
            .and_then(|db| {
                app.playlist
                    .controller
                    .add_album_to_playlist(db, playlist_index, &album)
            });
        match result {
            Ok(()) => {
                app.playlist.error = None;
                app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                    text.added_to_playlist(&album.title),
                ));
            }
            Err(error) => app.playlist.error = Some(error),
        }
    }
}
