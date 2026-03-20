//! Tutorial dialog shown on first launch to introduce the app's features.
//!
//! Also provides a contextual hint system for first-time feature encounters.

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Checkbox, CheckboxSize, Dialog, DialogSize, HStack,
    StackAlign, StackJustify, StackSize, StackSpacing, Text, TextSize, VStack,
};

// ============================================================================
// Tutorial Dialog (7-screen walkthrough)
// ============================================================================

const TUTORIAL_SCREEN_COUNT: usize = 7;

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
            "Browse your music library, build a queue, and enjoy playback with real-time audio plugins.",
            "Use the top navigation bar to switch between Library, Queue, Studio (plugins), and more.",
        ],
    },
    TutorialScreen {
        title: "Plugin Rack",
        image: "tutorial/rack.webp",
        content: &[
            "The Studio screen hosts your plugin rack \u{2014} a chain of audio processors applied in real time.",
            "Add plugins like Crossfeed and Fletcher-Munson compensation for headphone listening.",
            "Plugins include EQ, compressor, gate, limiter, upmixer, and more. Drag to reorder, toggle to bypass.",
        ],
    },
    TutorialScreen {
        title: "Recording",
        image: "tutorial/recording.webp",
        content: &[
            "Measure your speakers or room using sweep signals or pink noise.",
            "Connect a calibrated microphone, select input/output channels, and capture frequency response data.",
            "Recordings are saved as CSV files for use in Room EQ optimization.",
        ],
    },
    TutorialScreen {
        title: "Spinorama EQ",
        image: "tutorial/spinorama.webp",
        content: &[
            "Optimize speaker EQ using measurement data from spinorama.org.",
            "Search for your speaker model, choose a target curve, and let the optimizer find the best parametric EQ filters.",
            "Results can be exported and loaded directly into the plugin rack.",
        ],
    },
    TutorialScreen {
        title: "Headphone EQ",
        image: "tutorial/headphone.webp",
        content: &[
            "Target the Harman curve or other headphone targets for accurate reproduction.",
            "Load your headphone's frequency response and optimize PEQ filters to match the target.",
            "Supports multiple optimization algorithms for best results.",
        ],
    },
    TutorialScreen {
        title: "Room EQ",
        image: "tutorial/roomeq.webp",
        content: &[
            "Correct room acoustics with multi-channel support.",
            "Use your own measurements or recordings to generate per-channel EQ corrections.",
            "Supports crossover configuration, target curves, and multiple optimization modes.",
        ],
    },
    TutorialScreen {
        title: "Preferences",
        image: "tutorial/settings.webp",
        content: &[
            "Customize the app in Settings: theme, language, keybindings, audio device, and font scale.",
            "Manage your music library directories and configure scanner threads for background analysis.",
            "Plugin presets can be saved and loaded from the Settings screen.",
        ],
    },
];

// ============================================================================
// Contextual Hints (shown once per feature encounter)
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
// Tutorial Dialog Rendering (original 7-screen walkthrough, unchanged)
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
