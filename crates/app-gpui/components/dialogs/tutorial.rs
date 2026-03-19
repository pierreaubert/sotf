//! Tutorial dialog shown on first launch to introduce the app's features.
//!
//! Simplified 3-screen welcome flow + contextual hint system.

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Checkbox, CheckboxSize, Dialog, DialogSize, HStack,
    StackAlign, StackJustify, StackSize, StackSpacing, Text, TextSize, VStack,
};

// ============================================================================
// Welcome Tutorial (3-screen dialog)
// ============================================================================

const TUTORIAL_SCREEN_COUNT: usize = 3;

struct TutorialScreen {
    title: &'static str,
    image: &'static str,
    content: &'static [&'static str],
}

const TUTORIAL_SCREENS: [TutorialScreen; TUTORIAL_SCREEN_COUNT] = [
    TutorialScreen {
        title: "Welcome to SotF Player",
        image: "tutorial/player.webp",
        content: &[
            "SotF is a high-fidelity audio player with built-in DSP processing.",
            "Browse your music library, build a queue, and enjoy real-time audio plugins \u{2014} EQ, compression, upmixing, and more.",
        ],
    },
    TutorialScreen {
        title: "Key Features",
        image: "tutorial/rack.webp",
        content: &[
            "Plugin Rack \u{2014} Chain audio processors in the Studio screen. Drag to reorder, toggle to bypass.",
            "Room EQ \u{2014} Optimize speaker response using your own measurements.",
            "Headphone & Spinorama EQ \u{2014} Target curves for headphones and speakers from spinorama.org data.",
        ],
    },
    TutorialScreen {
        title: "Getting Started",
        image: "tutorial/settings.webp",
        content: &[
            "Add a music folder in Settings > Library to start browsing your collection.",
            "Use keyboard shortcuts for fast navigation: \u{2318}0-6 to switch screens, Space to play/pause.",
            "Press ? at any time for keyboard shortcuts. Contextual hints will appear as you explore features.",
        ],
    },
];

// ============================================================================
// Contextual Hints
// ============================================================================

/// Unique identifiers for contextual hints shown once per feature encounter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HintId {
    /// First time opening the Studio screen
    StudioFirstVisit,
    /// First plugin added to the rack
    FirstPluginAdded,
    /// First time on the Room EQ screen
    RoomEqFirstVisit,
    /// Empty queue shown in library
    EmptyQueue,
}

impl HintId {
    pub fn as_str(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => "studio_first_visit",
            HintId::FirstPluginAdded => "first_plugin_added",
            HintId::RoomEqFirstVisit => "roomeq_first_visit",
            HintId::EmptyQueue => "empty_queue",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => "Plugin Rack",
            HintId::FirstPluginAdded => "Plugin Added",
            HintId::RoomEqFirstVisit => "Room EQ",
            HintId::EmptyQueue => "Build Your Queue",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => {
                "Click the + button to add audio plugins. Drag to reorder, click to edit parameters."
            }
            HintId::FirstPluginAdded => {
                "Click a plugin card to edit its parameters. Use = / - keys to adjust values."
            }
            HintId::RoomEqFirstVisit => {
                "Start by loading measurement data, then configure and run the optimizer."
            }
            HintId::EmptyQueue => {
                "Click an album in the library to add it to your playback queue."
            }
        }
    }
}

/// Contextual hint state — shown as a dismissible banner at the top of the relevant screen.
#[derive(Debug, Clone)]
pub struct ContextualHint {
    pub hint_id: HintId,
}

/// Render a contextual hint banner (dismissible callout).
pub fn render_hint_banner(
    hint: &ContextualHint,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    let title = hint.hint_id.title();
    let message = hint.hint_id.message();

    div()
        .flex()
        .items_start()
        .gap_3()
        .px_4()
        .py_3()
        .mx_4()
        .mt_2()
        .rounded_lg()
        .bg(theme.toast_info_bg)
        .border_1()
        .border_color(theme.info)
        .child(
            div()
                .text_sm()
                .text_color(theme.info)
                .font_weight(FontWeight::BOLD)
                .child("\u{1f4a1}"),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_secondary)
                        .child(message),
                ),
        )
}

// ============================================================================
// Tutorial Dialog Rendering
// ============================================================================

impl PlayerView {
    pub(crate) fn render_tutorial_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let screen_idx = state.app.ui_state.tutorial_screen.min(TUTORIAL_SCREEN_COUNT - 1);
        let dont_show = state.app.ui_state.tutorial_dont_show;
        let screen = &TUTORIAL_SCREENS[screen_idx];
        let is_first = screen_idx == 0;
        let is_last = screen_idx == TUTORIAL_SCREEN_COUNT - 1;

        Dialog::new("tutorial-dialog")
            .title(screen.title)
            .size(DialogSize::Lg)
            .on_close({
                let state = self.state.clone();
                move |_window, cx| {
                    let state = state.clone();
                    cx.defer(move |cx| {
                        state.update(cx, |state, cx| {
                            Self::close_tutorial(state, cx);
                        });
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
                            .rounded_md()
                            .overflow_hidden()
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                img(screen.image)
                                    .w_full()
                                    .object_fit(ObjectFit::ScaleDown),
                            ),
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
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded(px(4.0))
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
                                    .label("Don't show again")
                                    .size(CheckboxSize::Sm)
                                    .on_change({
                                        let state = self.state.clone();
                                        move |checked, _window, cx| {
                                            let state = state.clone();
                                            cx.defer(move |cx| {
                                                state.update(cx, |state, _| {
                                                    state.app.ui_state.tutorial_dont_show = checked;
                                                });
                                            });
                                        }
                                    }),
                            )
                            // Navigation buttons
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .when(!is_first, |row| {
                                        row.child(
                                            Button::new("tutorial-prev", "Previous")
                                                .variant(ButtonVariant::Ghost)
                                                .size(ButtonSize::Xs)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_click(cx.listener(
                                                    |view, _event: &ClickEvent, _window, cx| {
                                                        let state = view.state.clone();
                                                        cx.defer(move |cx| {
                                                            state.update(cx, |state, _| {
                                                                state
                                                                    .app
                                                                    .ui_state
                                                                    .tutorial_screen = state
                                                                    .app
                                                                    .ui_state
                                                                    .tutorial_screen
                                                                    .saturating_sub(1);
                                                            });
                                                        });
                                                    },
                                                )),
                                        )
                                    })
                                    .when(!is_last, |row| {
                                        row.child(
                                            Button::new("tutorial-next", "Next")
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Xs)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_click(cx.listener(
                                                    |view, _event: &ClickEvent, _window, cx| {
                                                        let state = view.state.clone();
                                                        cx.defer(move |cx| {
                                                            state.update(cx, |state, _| {
                                                                let s = state
                                                                    .app
                                                                    .ui_state
                                                                    .tutorial_screen;
                                                                if s < TUTORIAL_SCREEN_COUNT - 1 {
                                                                    state
                                                                        .app
                                                                        .ui_state
                                                                        .tutorial_screen = s + 1;
                                                                }
                                                            });
                                                        });
                                                    },
                                                )),
                                        )
                                    })
                                    .when(is_last, |row| {
                                        row.child(
                                            Button::new("tutorial-done", "Get Started")
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Xs)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_click(cx.listener(
                                                    |view, _event: &ClickEvent, _window, cx| {
                                                        let state = view.state.clone();
                                                        cx.defer(move |cx| {
                                                            state.update(cx, |state, cx| {
                                                                Self::close_tutorial(state, cx);
                                                            });
                                                        });
                                                    },
                                                )),
                                        )
                                    }),
                            ),
                    ),
            )
    }

    /// Close the tutorial dialog and optionally mark as completed.
    /// Then proceed to check if library is empty.
    fn close_tutorial(
        state: &mut crate::app::state::app::AppState,
        cx: &mut gpui::Context<crate::app::state::app::AppState>,
    ) {
        if state.app.ui_state.tutorial_dont_show {
            state.app.tutorial_completed = true;
            // Persist immediately so the setting survives crashes
            let layout = state.layout.read(cx);
            if let Err(e) = state.app.save_config(layout) {
                log::warn!("Failed to save tutorial config: {}", e);
            }
        }
        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
        state.app.ui_state.tutorial_screen = 0;
        state.app.ui_state.tutorial_dont_show = false;

        // Now run the rest of the startup check (load library, check empty)
        if let Err(e) = state.app.load_library_from_database() {
            log::warn!("Failed to load library from database: {}", e);
        }
        if state.app.library_state.library.albums.is_empty() {
            state.app.ui_state.input_mode = crate::app::InputMode::EmptyLibraryPrompt;
        }
        state.app.is_loading_initial_data = false;
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
}
