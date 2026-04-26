//! Recording Saving Step (Step 4)
//!
//! Save recordings and configuration to disk.

use crate::app::types::ChannelRecordingState;
use crate::app::types::recording::RoomDimensionUnit;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Heading, Input, StackAlign, StackSpacing,
    Text, TextSize, VStack,
};

/// Filter the available-speakers list to the top matches for a query.
/// Case-insensitive substring match with a hard ceiling so the dropdown
/// never blows up even if every speaker in the catalog is a match.
fn filter_speakers(catalog: &[String], query: &str, limit: usize) -> Vec<String> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let q = query.to_lowercase();
    catalog
        .iter()
        .filter(|name| name.to_lowercase().contains(&q))
        .take(limit)
        .cloned()
        .collect()
}

impl PlayerView {
    /// Render the saving step UI
    pub(crate) fn render_recording_saving_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // IMPORTANT: render paths run INSIDE a GPUI entity update, so any
        // `state.update(...)` or `view.update(...)` called directly from
        // here triggers a re-entrant update and panics with
        // `cannot update PlayerView while it is already being updated`.
        //
        // We therefore:
        //   1. Read-only snapshots only during render.
        //   2. Any state mutation (syncing the channel_speakers vec,
        //      kicking off the spinorama catalog fetch) is scheduled
        //      via `cx.defer` so it runs after this update finishes.
        //   3. The render code tolerates an un-synced state by using
        //      `.get(i).cloned().unwrap_or_default()` for all lookups.
        let needs_fetch = {
            let snap = self.state.read(cx);
            let sp = &snap.app.measurement_state.spinorama_eq_state;
            let rec = &snap.app.measurement_state.recording_state;
            let catalog_missing = sp.available_speakers.is_empty() && !sp.loading_speakers;
            let speakers_unsynced = rec.channel_speakers.len() != rec.channel_recordings.len();
            catalog_missing || speakers_unsynced
        };
        if needs_fetch {
            // Capture the PlayerView entity outside the defer closure so
            // the deferred block can call `update` on it after the
            // current render finishes. `cx.defer` takes a single `cx`
            // argument — see `components/room_eq/custom_target_modal.rs`
            // for the same pattern.
            let view = cx.entity().clone();
            cx.defer(move |cx| {
                view.update(cx, |this, cx| {
                    this.state.update(cx, |state, _| {
                        state
                            .app
                            .measurement_state
                            .recording_state
                            .sync_channel_speakers_length();
                    });
                    let need_catalog = {
                        let snap = this.state.read(cx);
                        let sp = &snap.app.measurement_state.spinorama_eq_state;
                        sp.available_speakers.is_empty() && !sp.loading_speakers
                    };
                    if need_catalog {
                        this.fetch_spinorama_speakers(cx);
                    }
                });
            });
        }

        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;

        let has_recordings = recording_state
            .channel_recordings
            .iter()
            .any(|r| r.state == ChannelRecordingState::Done);

        let recording_dir = recording_state.recording_directory.clone();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(Heading::h4("Save Recordings"))
                    .child(
                        Text::new(translations.recording_saving_desc)
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    ),
            )
            .child(self.render_save_name_card(cx))
            .child(self.render_save_location_card(cx))
            .child(self.render_room_info_card(cx))
            .child(self.render_setup_description_card(cx))
            .child(self.render_channel_speakers_card(cx))
            .child(self.render_save_contents_card(cx))
            .child(self.render_save_actions(has_recordings, recording_dir, cx))
    }

    /// Render the save name input card
    fn render_save_name_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let save_name = state
            .app
            .measurement_state
            .recording_state
            .save_name
            .clone();
        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::eyebrow("RECORDING NAME").color(theme.accent))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new(translations.recording_name_label)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            div()
                                .w(px(300.0)) // intentional: fixed name input field width
                                .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                                    cx.stop_propagation();
                                })
                                .child(
                                    Input::new("save_name_input")
                                        .value(save_name.clone())
                                        .placeholder("Enter recording name")
                                        .on_text_change({
                                            let view = view.clone();
                                            move |value, _window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .recording_state
                                                            .save_name = value;
                                                    });
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        ),
                )
                .child(Text::caption(
                    "This name will be used for the subdirectory containing your recordings.",
                )),
        )
    }

    /// Render the save location card
    fn render_save_location_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let base_dir = recording_state.recording_base_directory.clone();
        let base_dir_display = base_dir.clone().unwrap_or_else(|| "Not set".to_string());

        let save_name = &recording_state.save_name;
        let full_path = if base_dir.is_some() {
            format!("{}/{}/", base_dir_display, save_name)
        } else {
            "Not set".to_string()
        };

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::eyebrow("SAVE LOCATION").color(theme.accent))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new(translations.recording_base_directory)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(base_dir_display.clone())
                                .size(TextSize::Xs)
                                .color(theme.text_primary),
                        )
                        .child(
                            Button::new("browse_save_dir", "Browse...")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.browse_recording_directory(cx);
                                        });
                                    }
                                }),
                        )
                        .when(base_dir.is_some(), |stack| {
                            let view = view.clone();
                            let theme = theme.clone();
                            stack.child(
                                Button::new("clear_save_dir", "Clear")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Xs)
                                    .theme(theme.to_button_theme())
                                    .on_click({
                                        move |_, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .recording_base_directory = None;
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .recording_directory = None;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                        }),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xs)
                        .align(StackAlign::Center)
                        .child(
                            Text::new(translations.recording_full_path)
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(Text::label(full_path).color(theme.text_primary)),
                ),
        )
    }

    /// Render the save contents card showing what will be saved
    fn render_save_contents_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let save_name = &recording_state.save_name;

        let recorded_channels: Vec<_> = recording_state
            .channel_recordings
            .iter()
            .filter(|r| r.state == ChannelRecordingState::Done)
            .collect();

        // Create safe version of save_name for filenames
        let safe_save_name: String = save_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::eyebrow("FILES TO SAVE").color(theme.accent))
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Xs)
                        // recordings.json
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .align(StackAlign::Center)
                                .child(Text::new("+").size(TextSize::Xs).color(theme.success))
                                .child(
                                    Text::label(format!("{}.json", safe_save_name))
                                        .color(theme.text_primary),
                                )
                                .child(Text::caption("- Configuration and measurement data")),
                        )
                        // Per-channel files
                        .children(
                            recorded_channels
                                .iter()
                                .flat_map(|rec| {
                                    let safe_channel_name: String = rec
                                        .channel_name
                                        .chars()
                                        .map(|c| {
                                            if c.is_alphanumeric() || c == '_' || c == '-' {
                                                c
                                            } else {
                                                '_'
                                            }
                                        })
                                        .collect();

                                    vec![
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new("+")
                                                    .size(TextSize::Xs)
                                                    .color(theme.success),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "{}_{}.wav",
                                                    safe_save_name, safe_channel_name
                                                ))
                                                .size(TextSize::Xs)
                                                .color(theme.text_primary),
                                            )
                                            .child(Text::caption(format!(
                                                "- {} recording",
                                                rec.channel_name
                                            )))
                                            .into_any_element(),
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new("+")
                                                    .size(TextSize::Xs)
                                                    .color(theme.success),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "{}_{}.csv",
                                                    safe_save_name, safe_channel_name
                                                ))
                                                .size(TextSize::Xs)
                                                .color(theme.text_primary),
                                            )
                                            .child(Text::caption(format!(
                                                "- {} frequency response",
                                                rec.channel_name
                                            )))
                                            .into_any_element(),
                                    ]
                                })
                                .collect::<Vec<_>>(),
                        ),
                ),
        )
    }

    /// Render save action buttons
    fn render_save_actions(
        &self,
        has_recordings: bool,
        recording_dir: Option<String>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let status_message = state
            .app
            .measurement_state
            .recording_state
            .status_message
            .clone();
        let view = cx.entity().clone();

        let can_save = has_recordings && recording_dir.is_some();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::eyebrow("ACTIONS").color(theme.accent))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(
                            Button::new("save_recordings", "Save All")
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Md)
                                .disabled(!can_save)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.save_recordings(cx);
                                        });
                                    }
                                }),
                        )
                        .child(
                            Button::new("load_recordings", "Load Previous")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.load_recordings_from_file(cx);
                                        });
                                    }
                                }),
                        ),
                )
                .when(!status_message.is_empty(), |stack| {
                    let theme = theme.clone();
                    let is_error = status_message.to_lowercase().contains("error")
                        || status_message.to_lowercase().contains("failed");
                    stack.child(
                        Text::new(status_message.clone())
                            .size(TextSize::Xs)
                            .color(if is_error { theme.error } else { theme.success }),
                    )
                })
                .when(!can_save, |stack| {
                    let reason = if !has_recordings {
                        "No recordings to save. Go back to capture some channels."
                    } else {
                        "No save directory selected. Go back to setup to select a directory."
                    };
                    stack.child(Text::new(reason).size(TextSize::Xs).color(theme.warning))
                }),
        )
    }

    /// Render the room-dimensions card (3 number inputs + unit toggle).
    ///
    /// The three inputs (W × D × H) are always interpreted in the
    /// currently-selected unit; the toggle to the right swaps between
    /// metric (meters) and imperial (feet). Conversion to canonical
    /// meters happens at save time via
    /// [`RecordingState::room_dimensions_for_save`] — the state on the
    /// UI side never carries mixed units.
    fn render_room_info_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let rec = &state.app.measurement_state.recording_state;
        let width = rec.room_width_input;
        let depth = rec.room_depth_input;
        let height = rec.room_height_input;
        let unit = rec.room_dimension_unit;
        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::eyebrow("ROOM DIMENSIONS").color(theme.accent))
                .child(Text::caption(
                    "Width × Depth × Height. Optional, but lets the optimizer auto-tune \
                     the Schroeder frequency from room volume.",
                ))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(dimension_field(
                            "room_width",
                            "Width",
                            width,
                            unit,
                            theme.clone(),
                            view.clone(),
                            |rec, v| rec.room_width_input = v,
                        ))
                        .child(dimension_field(
                            "room_depth",
                            "Depth",
                            depth,
                            unit,
                            theme.clone(),
                            view.clone(),
                            |rec, v| rec.room_depth_input = v,
                        ))
                        .child(dimension_field(
                            "room_height",
                            "Height",
                            height,
                            unit,
                            theme.clone(),
                            view.clone(),
                            |rec, v| rec.room_height_input = v,
                        ))
                        .child(
                            Button::new("room_unit_toggle", unit.label())
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                let rec = &mut state
                                                    .app
                                                    .measurement_state
                                                    .recording_state;
                                                rec.room_dimension_unit =
                                                    rec.room_dimension_unit.toggled();
                                            });
                                            cx.notify();
                                        });
                                    }
                                }),
                        ),
                ),
        )
    }

    /// Render the free-form "setup description" text card.
    fn render_setup_description_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let description = state
            .app
            .measurement_state
            .recording_state
            .setup_description
            .clone();
        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::eyebrow("SETUP DESCRIPTION").color(theme.accent))
                .child(Text::caption(
                    "Notes about the listening position, acoustic treatment, \
                     equipment chain, anything worth remembering next session.",
                ))
                .child(
                    div()
                        .w(px(560.0)) // intentional: wide description field width
                        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                            cx.stop_propagation();
                        })
                        .child(
                            Input::new("setup_description_input")
                                .value(description)
                                .placeholder(
                                    "e.g. small bedroom, equilateral triangle, \
                                     bass traps in corners, 2.2m sweet-spot to plane",
                                )
                                .on_text_change({
                                    let view = view.clone();
                                    move |value, _window, cx| {
                                        view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                state
                                                    .app
                                                    .measurement_state
                                                    .recording_state
                                                    .setup_description = value;
                                            });
                                            cx.notify();
                                        });
                                    }
                                }),
                        ),
                ),
        )
    }

    /// Render the per-channel speaker-identity card.
    ///
    /// One row per recorded channel. Each row has a text input for the
    /// speaker brand+model and renders a filtered-autocomplete dropdown
    /// of suggestions from the spinorama.org catalog
    /// (`spinorama_eq_state.available_speakers`). The catalog is
    /// populated the first time the save step renders — see
    /// [`render_recording_saving_step`].
    fn render_channel_speakers_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let rec = &state.app.measurement_state.recording_state;
        let catalog = state
            .app
            .measurement_state
            .spinorama_eq_state
            .available_speakers
            .clone();
        let is_loading = state
            .app
            .measurement_state
            .spinorama_eq_state
            .loading_speakers;
        let open_row = rec.channel_speaker_autocomplete_open;

        // Snapshot the per-row (channel_name, current_value) pairs so we
        // can build the dropdown without borrowing `state` into the
        // listeners. The `channel_speakers` vec is kept in sync with
        // `channel_recordings` by `sync_channel_speakers_length` at
        // render entry.
        let rows: Vec<(usize, String, String)> = rec
            .channel_recordings
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let current = rec.channel_speakers.get(i).cloned().unwrap_or_default();
                (i, r.channel_name.clone(), current)
            })
            .collect();
        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::eyebrow("SPEAKERS PER CHANNEL").color(theme.accent))
                .child(Text::caption(if is_loading {
                    "Loading catalog from spinorama.org…"
                } else if catalog.is_empty() {
                    "Catalog unavailable. Type freely — any label is saved as-is."
                } else {
                    "Type to filter the spinorama.org catalog; click a match to fill in."
                }))
                .children(rows.into_iter().map(|(row, channel_name, current)| {
                    let suggestions = if open_row == Some(row) {
                        filter_speakers(&catalog, &current, 8)
                    } else {
                        Vec::new()
                    };
                    let exact_match = catalog.iter().any(|c| c == &current);
                    let show_dropdown = !suggestions.is_empty() && !exact_match;

                    VStack::new()
                        .spacing(StackSpacing::Xs)
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .align(StackAlign::Center)
                                .child(div().w(px(80.0)).child(
                                    // intentional: channel label column
                                    Text::label(channel_name.clone()).color(theme.text_primary),
                                ))
                                .child(
                                    div()
                                        .w(px(380.0)) // intentional: speaker input field width
                                        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                                            cx.stop_propagation();
                                        })
                                        .child(
                                            Input::new(SharedString::from(format!(
                                                "channel_speaker_input_{}",
                                                row
                                            )))
                                            .value(current.clone())
                                            .placeholder("Brand and model")
                                            .on_text_change({
                                                let view = view.clone();
                                                move |value, _window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            let rec = &mut state
                                                                .app
                                                                .measurement_state
                                                                .recording_state;
                                                            rec.sync_channel_speakers_length();
                                                            if let Some(slot) =
                                                                rec.channel_speakers.get_mut(row)
                                                            {
                                                                *slot = value;
                                                            }
                                                            rec.channel_speaker_autocomplete_open =
                                                                Some(row);
                                                        });
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                        ),
                                ),
                        )
                        .when(show_dropdown, |el| {
                            el.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "channel_speaker_suggestions_{}",
                                        row
                                    )))
                                    .ml(px(88.0)) // intentional: align under the input (label col + gap)
                                    .w(px(380.0)) // intentional: match input field width
                                    .max_h(px(220.0)) // intentional: dropdown scroll viewport
                                    .overflow_y_scroll()
                                    .bg(theme.surface)
                                    .rounded(d.r_md)
                                    .border_1()
                                    .border_color(theme.border)
                                    .children(suggestions.into_iter().map(|s| {
                                        let suggestion = s.clone();
                                        div()
                                            .id(SharedString::from(format!(
                                                "channel_speaker_opt_{}_{}",
                                                row, suggestion
                                            )))
                                            .px(d.pad_y)
                                            .py(d.pad_y_half)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.surface_hover))
                                            .child(Text::new(suggestion.clone()).size(TextSize::Xs))
                                            .on_mouse_down(MouseButton::Left, {
                                                let view = view.clone();
                                                let picked = suggestion.clone();
                                                move |_event, _window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            let rec = &mut state
                                                                .app
                                                                .measurement_state
                                                                .recording_state;
                                                            rec.sync_channel_speakers_length();
                                                            if let Some(slot) =
                                                                rec.channel_speakers.get_mut(row)
                                                            {
                                                                *slot = picked.clone();
                                                            }
                                                            rec.channel_speaker_autocomplete_open =
                                                                None;
                                                        });
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                    })),
                            )
                        })
                })),
        )
    }
}

/// Build a single "label + number input" block for the room-dimensions
/// card. Kept as a free function (not a method) to sidestep lifetime
/// gymnastics inside the HStack chain.
fn dimension_field(
    id: &'static str,
    label: &'static str,
    current: f64,
    unit: RoomDimensionUnit,
    theme: crate::app::theme::Theme,
    view: gpui::Entity<PlayerView>,
    apply: fn(&mut crate::app::types::recording::RecordingState, f64),
) -> impl IntoElement {
    let display = if current > 0.0 {
        format!("{:.2}", current)
    } else {
        String::new()
    };
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new(format!("{} ({})", label, unit.label()))
                .size(TextSize::Xs)
                .color(theme.text_secondary),
        )
        .child(
            div()
                .w(px(100.0)) // intentional: dimension numeric-input column width
                .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                    cx.stop_propagation();
                })
                .child(
                    Input::new(id)
                        .value(display)
                        .placeholder("0.00")
                        .on_text_change(move |value, _window, cx| {
                            // Empty string means "clear" — stored as 0.0
                            // which `room_dimensions_for_save` treats as
                            // "not specified" so the whole triple is
                            // dropped from serialization.
                            let parsed = value.trim().parse::<f64>().unwrap_or(0.0).max(0.0);
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    apply(&mut state.app.measurement_state.recording_state, parsed);
                                });
                                cx.notify();
                            });
                        }),
                ),
        )
}
