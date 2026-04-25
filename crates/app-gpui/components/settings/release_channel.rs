use crate::app::types::Screen;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use sotf_audio_player::{PluginType, ReleaseChannel};

/// A row in the feature availability table.
struct FeatureRow {
    name: &'static str,
    maturity: ReleaseChannel,
}

impl PlayerView {
    pub(crate) fn render_release_channel_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let current_channel = state.app.ui_state.release_channel;
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();

        div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(translations.settings_release_channel_title),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(translations.settings_release_channel_description),
                    )
                    .child({
                        let mut container = div().flex().flex_wrap().gap(d.section);

                        for channel in ReleaseChannel::all() {
                            let is_selected = current_channel == *channel;
                            let channel_val = *channel;
                            let accent = theme.accent;
                            let border = theme.border;
                            let surface = theme.surface;
                            let surface_selected = theme.surface_selected;
                            let text_primary = theme.text_primary;
                            let text_secondary = theme.text_secondary;

                            container = container.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "release-channel-{}",
                                        channel.name()
                                    )))
                                    .flex()
                                    .flex_col()
                                    .w(rems(13.75))
                                    .p(d.card)
                                    .rounded(d.r_md)
                                    .border_2()
                                    .border_color(if is_selected { accent } else { border })
                                    .bg(if is_selected {
                                        surface_selected
                                    } else {
                                        surface
                                    })
                                    .cursor_pointer()
                                    .hover(move |s| s.border_color(accent))
                                    .child(
                                        div()
                                            .text_size(d.text_sm)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if is_selected {
                                                accent
                                            } else {
                                                text_primary
                                            })
                                            .child(channel.name()),
                                    )
                                    .child(
                                        div()
                                            .text_size(d.text_xs)
                                            .text_color(text_secondary)
                                            .mt(d.grid)
                                            .child(channel.description()),
                                    )
                                    .child(
                                        div().mt(d.gap_md).child(
                                            Button::new(
                                                SharedString::from(format!(
                                                    "select-channel-{}",
                                                    channel.name()
                                                )),
                                                if is_selected { "Active" } else { "Select" },
                                            )
                                            .variant(if is_selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .size(ButtonSize::Xs)
                                            .full_width(true)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(
                                                    move |view, _: &ClickEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state
                                                                .app
                                                                .set_release_channel(channel_val);
                                                        });
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                    ),
                            );
                        }

                        container
                    }),
            )
            .child(self.render_feature_table(&theme, d))
    }

    /// Render the feature availability table grouped by Features and Plugins.
    fn render_feature_table(&self, theme: &crate::theme::Theme, d: Ds) -> impl IntoElement {
        // --- Features (screens) ---
        let features: Vec<FeatureRow> = vec![
            FeatureRow {
                name: "Library",
                maturity: Screen::Library.maturity(),
            },
            FeatureRow {
                name: "Queue",
                maturity: Screen::Queue.maturity(),
            },
            FeatureRow {
                name: "Spectrum",
                maturity: Screen::Spectrum.maturity(),
            },
            FeatureRow {
                name: "Settings",
                maturity: Screen::Settings.maturity(),
            },
            FeatureRow {
                name: "Recording",
                maturity: Screen::Recording.maturity(),
            },
            FeatureRow {
                name: "Headphone EQ",
                maturity: Screen::HeadphoneEq.maturity(),
            },
            FeatureRow {
                name: "Spinorama",
                maturity: Screen::Spinorama.maturity(),
            },
            FeatureRow {
                name: "Plugin Graph",
                maturity: Screen::PluginGraph.maturity(),
            },
            FeatureRow {
                name: "Studio",
                maturity: Screen::Studio.maturity(),
            },
            FeatureRow {
                name: "Room EQ",
                maturity: Screen::RoomEq.maturity(),
            },
        ];

        // --- Plugins ---
        let plugins: Vec<FeatureRow> = PluginType::all()
            .into_iter()
            .map(|p| FeatureRow {
                name: p.name(),
                maturity: p.maturity(),
            })
            .collect();

        let col_w = rems(5.0);
        let name_w = rems(12.5);
        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;
        let border = theme.border;
        let accent = theme.accent;
        let surface = theme.surface;

        // Header row
        let header = div()
            .flex()
            .border_b_1()
            .border_color(border)
            .pb(d.pad_y_half)
            .mb(d.grid)
            .child(
                div()
                    .w(name_w)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary),
            )
            .child(
                div()
                    .w(col_w)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .text_center()
                    .child("Stable"),
            )
            .child(
                div()
                    .w(col_w)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .text_center()
                    .child("Beta"),
            )
            .child(
                div()
                    .w(col_w)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .text_center()
                    .child("Alpha"),
            );

        // Helper to build one table section
        let build_section = move |label: &'static str, rows: Vec<FeatureRow>| -> Div {
            let mut section = div().flex().flex_col().gap_0p5();

            // Section header
            section = section.child(
                div().flex().mt(d.gap_md).mb(d.grid).child(
                    div()
                        .w(name_w)
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child(label),
                ),
            );

            for row in &rows {
                let mark = |channel: ReleaseChannel| -> Div {
                    div()
                        .w(col_w)
                        .text_size(d.text_xs)
                        .text_center()
                        .child(if row.maturity == channel {
                            SharedString::from("\u{2714}") // checkmark
                        } else {
                            SharedString::from("")
                        })
                        .text_color(if row.maturity == channel {
                            accent
                        } else {
                            text_secondary
                        })
                };

                section = section.child(
                    div()
                        .flex()
                        .py_0p5()
                        .rounded(d.r_sm)
                        .hover(move |s| s.bg(surface))
                        .child(
                            div()
                                .w(name_w)
                                .text_size(d.text_xs)
                                .text_color(text_primary)
                                .child(row.name),
                        )
                        .child(mark(ReleaseChannel::Prod))
                        .child(mark(ReleaseChannel::Beta))
                        .child(mark(ReleaseChannel::Alpha)),
                );
            }

            section
        };

        div()
            .flex()
            .flex_col()
            .child(header)
            .child(build_section("Features", features))
            .child(build_section("Plugins", plugins))
    }
}
