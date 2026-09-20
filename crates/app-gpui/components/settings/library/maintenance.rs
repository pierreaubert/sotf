#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::LibraryMaintenanceTranslations;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonVariant, Text};

impl PlayerView {
    fn run_library_maintenance(&mut self, remove: bool, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let text = LibraryMaintenanceTranslations::for_language(state.app.ui_state.language);
        let maintenance = &state.app.settings.library.maintenance;
        if maintenance.busy || state.app.library_state.scan_in_progress {
            return;
        }
        let generation = state.app.library_state.content_generation();
        let committed_count = maintenance.removed.filter(|_| maintenance.error.is_some());
        let review = if remove {
            maintenance.review.clone()
        } else {
            None
        };
        if remove
            && review
                .as_ref()
                .is_none_or(|review| review.entries().is_empty())
        {
            return;
        }
        let path = state
            .app
            .library_state
            .library
            .get_database()
            .and_then(|db| db.backing_path());
        #[cfg(feature = "dev-api")]
        let fail_refresh = self.state.update(cx, |state, _| {
            std::mem::take(&mut state.app.settings.library.maintenance.qa_fail_next_refresh)
        });
        #[cfg(feature = "dev-api")]
        let change_before_refresh = self.state.update(cx, |state, _| {
            std::mem::take(
                &mut state
                    .app
                    .settings
                    .library
                    .maintenance
                    .qa_change_before_refresh,
            )
        });
        self.state.update(cx, |state, _| {
            let maintenance = &mut state.app.settings.library.maintenance;
            maintenance.busy = true;
            maintenance.error = None;
            maintenance.removed = committed_count;
        });
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let path = path.ok_or_else(|| text.unavailable.to_string())?;
                    let mut db = sotf_audio_player::database::MusicDatabase::open_secondary(path)
                        .map_err(|error| error.to_string())?;
                    if let Some(review) = review {
                        let removed = db
                            .remove_reviewed_missing_files(&review)
                            .map_err(|error| error.to_string())?;
                        // A failed refresh must remain retryable even after the commit.
                        let albums = db.load_library().map_err(|error| error.to_string());
                        #[cfg(feature = "dev-api")]
                        let albums = if fail_refresh {
                            Err("QA library refresh failure".into())
                        } else {
                            albums
                        };
                        Ok::<_, String>((None, Some((removed, albums))))
                    } else if let Some(count) = committed_count {
                        // Retry only the read after a committed removal; never remove twice.
                        let albums = db.load_library().map_err(|error| error.to_string());
                        #[cfg(feature = "dev-api")]
                        let albums = if fail_refresh {
                            Err("QA library refresh failure".into())
                        } else {
                            albums
                        };
                        Ok((None, Some((count, albums))))
                    } else {
                        Ok((
                            Some(
                                db.review_missing_files()
                                    .map_err(|error| error.to_string())?,
                            ),
                            None,
                        ))
                    }
                })
                .await;
            let _ = view.update(cx, |view, cx| {
                view.state.update(cx, |state, _| {
                    #[cfg(feature = "dev-api")]
                    if change_before_refresh {
                        state.app.library_state.invalidate_cache();
                    }
                    state.app.settings.library.maintenance.busy = false;
                    match result {
                        Ok((review, removed)) => {
                            state.app.settings.library.maintenance.review = review;
                            if let Some((count, albums)) = removed {
                                state.app.settings.library.maintenance.removed = Some(count);
                                match albums {
                                    Ok(albums) => {
                                        if !state
                                            .app
                                            .library_state
                                            .replace_loaded_albums_if_current(generation, albums)
                                        {
                                            state.app.settings.library.maintenance.error =
                                                Some(text.changed.into());
                                        }
                                    }
                                    Err(error) => {
                                        state.app.settings.library.maintenance.error = Some(error)
                                    }
                                }
                            }
                        }
                        Err(error) => state.app.settings.library.maintenance.error = Some(error),
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_library_maintenance(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;
        let text = LibraryMaintenanceTranslations::for_language(state.app.ui_state.language);
        let maintenance = &state.app.settings.library.maintenance;
        let busy = maintenance.busy || state.app.library_state.scan_in_progress;
        let button = Button::new(
            "library-maintenance-review",
            if maintenance.busy {
                text.checking
            } else if maintenance.removed.is_some() && maintenance.error.is_some() {
                text.retry_refresh
            } else {
                text.review
            },
        )
        .variant(ButtonVariant::Secondary)
        .disabled(busy)
        .theme(theme.to_button_theme())
        .on_click_event(cx.listener(|view, _, _, cx| view.run_library_maintenance(false, cx)));
        #[cfg(feature = "dev-api")]
        let button = button.dev_track("settings.library.maintenance-review");
        div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .mt(d.section)
            .child(Text::section_header(
                state
                    .app
                    .ui_state
                    .translations
                    .settings_database_maintenance,
            ))
            .child(Text::body(text.description))
            .child(button)
            .when_some(maintenance.error.clone(), |content, error| {
                content.child(Text::body(error).color(theme.error))
            })
            .when_some(maintenance.removed, |content, count| {
                content.child(Text::body(format!("{}: {count}", text.removed)))
            })
            .when_some(maintenance.review.as_ref(), |content, review| {
                let confirm = Button::new("library-maintenance-confirm", text.confirm)
                    .disabled(busy || review.entries().is_empty())
                    .theme(theme.to_button_theme())
                    .on_click_event(
                        cx.listener(|view, _, _, cx| view.run_library_maintenance(true, cx)),
                    );
                #[cfg(feature = "dev-api")]
                let confirm = confirm.dev_track("settings.library.maintenance-confirm");
                content
                    .child(Text::body(format!(
                        "{}: {} · {}: {}",
                        text.missing,
                        review.entries().len(),
                        text.unchecked,
                        review.unavailable_paths().len()
                    )))
                    .child(
                        div()
                            .id("library-maintenance-paths")
                            .max_h(rems(18.0))
                            .overflow_y_scroll()
                            .overflow_x_scroll()
                            .children(review.entries().iter().map(|entry| {
                                let name = entry
                                    .path()
                                    .file_name()
                                    .unwrap_or(entry.path().as_os_str())
                                    .to_string_lossy()
                                    .into_owned();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.gap_md)
                                    .mb(d.gap_md)
                                    .child(Text::label(name))
                                    .child(Text::caption(
                                        entry.path().to_string_lossy().into_owned(),
                                    ))
                            })),
                    )
                    .child(confirm)
                    .child(
                        Button::new("library-maintenance-cancel", text.cancel)
                            .variant(ButtonVariant::Secondary)
                            .disabled(busy)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                view.state.update(cx, |state, _| {
                                    state.app.settings.library.maintenance.review = None
                                });
                                cx.notify();
                            }))
                            .map(|button| {
                                #[cfg(feature = "dev-api")]
                                let button =
                                    button.dev_track("settings.library.maintenance-cancel");
                                button
                            }),
                    )
            })
    }
}
