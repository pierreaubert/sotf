use super::screen_guide::ScreenGuide;
use crate::app::i18n::{DialogTranslations, TutorialTranslations};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Checkbox, CheckboxSize, Dialog, DialogSize, HStack,
    StackAlign, StackJustify, StackSize, StackSpacing, Text, TextSize, VStack,
};

const TUTORIAL_SCREEN_COUNT: usize = 7;

const TUTORIAL_IMAGES: [&str; TUTORIAL_SCREEN_COUNT] = [
    "tutorial/player.webp",
    "tutorial/rack.webp",
    "tutorial/recording.webp",
    "tutorial/spinorama.webp",
    "tutorial/headphone.webp",
    "tutorial/roomeq.webp",
    "tutorial/settings.webp",
];

impl PlayerView {
    pub(crate) fn render_tutorial_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let dialog_text = DialogTranslations::for_language(state.app.ui_state.language);
        let tutorial_text = TutorialTranslations::for_language(state.app.ui_state.language);
        let screen_idx = state
            .app
            .ui_state
            .tutorial_screen
            .min(TUTORIAL_SCREEN_COUNT - 1);
        let dont_show = state.app.ui_state.tutorial_dont_show;
        let screen = &tutorial_text.screens[screen_idx];
        let screen_image = TUTORIAL_IMAGES[screen_idx];
        let is_first = screen_idx == 0;
        let is_last = screen_idx == TUTORIAL_SCREEN_COUNT - 1;

        // Disable close_on_backdrop: clicking inside the dialog (buttons, checkbox)
        // must NOT trigger the backdrop close handler — only the explicit close button
        // and "Get Started" should close the tutorial.
        Dialog::new("tutorial-dialog")
            .title(screen.title)
            .size(DialogSize::Lg)
            .close_on_backdrop(false)
            .on_close({
                let state = self.state.clone();
                move |_window, cx| {
                    state.update(cx, |state, cx| {
                        Self::close_tutorial(state, cx);
                    });
                }
            })
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    // Screenshot image
                    .child(
                        div()
                            .w_full()
                            .rounded(d.r_md)
                            .overflow_hidden()
                            .border_1()
                            .border_color(theme.border)
                            .child(img(screen_image).w_full().object_fit(ObjectFit::ScaleDown)),
                    )
                    // Text content
                    .children(screen.content.iter().map(|&line| {
                        Text::new(line)
                            .size(TextSize::Sm)
                            .color(theme.text_secondary)
                            .into_any_element()
                    })),
            )
            .footer(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .width(StackSize::Full)
                    // Step dots
                    .child(
                        HStack::new()
                            .width(StackSize::Full)
                            .justify(StackJustify::Center)
                            .spacing(StackSpacing::Xs)
                            .children((0..TUTORIAL_SCREEN_COUNT).map(|i| {
                                div()
                                    .w(rems(0.5))
                                    .h(rems(0.5))
                                    .rounded(d.r_md)
                                    .bg(if i == screen_idx {
                                        theme.accent
                                    } else {
                                        theme.border
                                    })
                                    .into_any_element()
                            })),
                    )
                    // Controls row
                    .child(
                        HStack::new()
                            .width(StackSize::Full)
                            .justify(StackJustify::SpaceBetween)
                            .align(StackAlign::Center)
                            // Checkbox
                            .child(
                                Checkbox::new("tutorial-dont-show")
                                    .checked(dont_show)
                                    .label(dialog_text.dont_show_again)
                                    .size(CheckboxSize::Sm)
                                    .on_change({
                                        let state = self.state.clone();
                                        move |checked, _window, cx| {
                                            state.update(cx, |state, _| {
                                                state.app.ui_state.tutorial_dont_show = checked;
                                            });
                                        }
                                    }),
                            )
                            // Navigation buttons — use Button's own on_click (on_mouse_up)
                            // instead of .build().on_click(cx.listener(...)) which relies
                            // on Stateful<Div> click tracking and is less reliable.
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .when(!is_first, |row| {
                                        row.child(
                                            Button::new("tutorial-prev", tutorial_text.previous)
                                                .variant(ButtonVariant::Ghost)
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click({
                                                    let state = self.state.clone();
                                                    move |_window, cx| {
                                                        state.update(cx, |state, _| {
                                                            state.app.ui_state.tutorial_screen =
                                                                state
                                                                    .app
                                                                    .ui_state
                                                                    .tutorial_screen
                                                                    .saturating_sub(1);
                                                        });
                                                    }
                                                }),
                                        )
                                    })
                                    .when(!is_last, |row| {
                                        row.child(
                                            Button::new("tutorial-next", tutorial_text.next)
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click({
                                                    let state = self.state.clone();
                                                    move |_window, cx| {
                                                        state.update(cx, |state, _| {
                                                            let s =
                                                                state.app.ui_state.tutorial_screen;
                                                            if s < TUTORIAL_SCREEN_COUNT - 1 {
                                                                state
                                                                    .app
                                                                    .ui_state
                                                                    .tutorial_screen = s + 1;
                                                            }
                                                        });
                                                    }
                                                }),
                                        )
                                    })
                                    .when(is_last, |row| {
                                        row.child(
                                            Button::new("tutorial-done", tutorial_text.get_started)
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click({
                                                    let state = self.state.clone();
                                                    move |_window, cx| {
                                                        state.update(cx, |state, cx| {
                                                            Self::close_tutorial(state, cx);
                                                        });
                                                    }
                                                }),
                                        )
                                    }),
                            ),
                    ),
            )
    }

    /// Close the tutorial dialog and optionally mark as completed.
    /// Then proceed to check if library is empty.
    ///
    /// The UI is updated immediately (no freeze). Config saving and library
    /// loading happen afterwards so the event loop stays responsive — this
    /// fixes the Windows freeze where synchronous SQLite / disk I/O blocked
    /// the main thread before the dialog could close.
    pub(super) fn close_tutorial(
        state: &mut crate::app::state::app::AppState,
        cx: &mut gpui::Context<crate::app::state::app::AppState>,
    ) {
        let dont_show = state.app.ui_state.tutorial_dont_show;

        // 1. Close the dialog immediately (UI update — no I/O)
        if dont_show {
            state.app.tutorial.completed = true;
        }
        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
        state.app.ui_state.tutorial_screen = 0;
        state.app.ui_state.tutorial_dont_show = false;
        state.app.library_view.loading_initial_data = false;

        // 2. Defer I/O-heavy operations so the dialog closes first
        let entity = cx.entity().clone();
        cx.defer(move |cx| {
            entity.update(cx, |state, cx| {
                // Persist config (disk I/O)
                if dont_show {
                    let layout = state.layout.read(cx);
                    if let Err(e) = state.app.save_config(layout) {
                        log::warn!("Failed to save tutorial config: {}", e);
                    }
                }

                // Load library from database (SQLite I/O)
                if let Err(e) = state.app.load_library_from_database() {
                    log::warn!("Failed to load library from database: {}", e);
                }
                if state.app.library_state.library.albums.is_empty() {
                    state.app.ui_state.input_mode = crate::app::InputMode::EmptyLibraryPrompt;
                }
            });
        });
    }

    /// Handle keyboard input for Tutorial mode
    pub(crate) fn handle_tutorial_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "left" => {
                self.state.update(cx, |state, _| {
                    state.app.ui_state.tutorial_screen =
                        state.app.ui_state.tutorial_screen.saturating_sub(1);
                });
            }
            "right" => {
                self.state.update(cx, |state, _| {
                    let s = state.app.ui_state.tutorial_screen;
                    if s < TUTORIAL_SCREEN_COUNT - 1 {
                        state.app.ui_state.tutorial_screen = s + 1;
                    }
                });
            }
            "escape" | "enter" => {
                self.state.update(cx, |state, cx| {
                    Self::close_tutorial(state, cx);
                });
            }
            _ => {}
        }
    }

    // ========================================================================
    // Screen Guide — contextual help for the current screen
    // ========================================================================

    pub(crate) fn render_screen_guide_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let screen = state.app.ui_state.current_screen;
        let text = DialogTranslations::for_language(state.app.ui_state.language);

        let guide = ScreenGuide::for_screen(screen, state.app.ui_state.language);

        Dialog::new("screen-guide-dialog")
            .title(guide.title)
            .size(DialogSize::Lg)
            .on_close({
                let state = self.state.clone();
                move |_window, cx| {
                    state.update(cx, |state, _| {
                        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    });
                }
            })
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    // Overview
                    .child(
                        Text::new(guide.overview)
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    )
                    // Sections
                    .children(guide.sections.iter().map(|section| {
                        let theme = theme.clone();
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(Text::section_header(section.heading).color(theme.accent))
                            .children(section.bullets.iter().map(|&bullet| {
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("\u{2022}")
                                            .size(TextSize::Sm)
                                            .color(theme.text_muted),
                                    )
                                    .child(
                                        Text::new(bullet)
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .into_any_element()
                            }))
                            .into_any_element()
                    })),
            )
            .footer(Text::caption(text.about.press_escape_to_close))
    }
}
