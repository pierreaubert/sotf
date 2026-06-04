//! Tutorial dialog shown on first launch to introduce the app's features.
//!
//! Also provides a contextual hint system for first-time feature encounters.

use crate::components::design::Ds;
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
            HintId::EmptyQueue => "Click an album in the library to add it to your playback queue.",
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
    d: Ds,
) -> impl IntoElement {
    let title = hint.hint_id.title();
    let message = hint.hint_id.message();

    div()
        .flex()
        .items_start()
        .gap(d.gap_md)
        .px(d.card)
        .py(d.pad_x)
        .mx(d.card)
        .mt(d.gap)
        .rounded(d.r_lg)
        .bg(theme.toast_info_bg)
        .border_1()
        .border_color(theme.info)
        .child(
            div()
                .text_size(d.text_sm)
                .text_color(theme.info)
                .font_weight(FontWeight::BOLD)
                .child("\u{1f4a1}"),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
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
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let screen_idx = state
            .app
            .ui_state
            .tutorial_screen
            .min(TUTORIAL_SCREEN_COUNT - 1);
        let dont_show = state.app.ui_state.tutorial_dont_show;
        let screen = &TUTORIAL_SCREENS[screen_idx];
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
                            .child(img(screen.image).w_full().object_fit(ObjectFit::ScaleDown)),
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
                                    .label("Don't show again")
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
                                            Button::new("tutorial-prev", "Previous")
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
                                            Button::new("tutorial-next", "Next")
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
                                            Button::new("tutorial-done", "Get Started")
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
    fn close_tutorial(
        state: &mut crate::app::state::app::AppState,
        cx: &mut gpui::Context<crate::app::state::app::AppState>,
    ) {
        let dont_show = state.app.ui_state.tutorial_dont_show;

        // 1. Close the dialog immediately (UI update — no I/O)
        if dont_show {
            state.app.tutorial_completed = true;
        }
        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
        state.app.ui_state.tutorial_screen = 0;
        state.app.ui_state.tutorial_dont_show = false;
        state.app.is_loading_initial_data = false;

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

        let guide = ScreenGuide::for_screen(screen);

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
            .footer(Text::caption("Press ESC or F1 to close"))
    }
}

// ============================================================================
// Screen Guide content
// ============================================================================

struct GuideSection {
    heading: &'static str,
    bullets: &'static [&'static str],
}

struct ScreenGuide {
    title: &'static str,
    overview: &'static str,
    sections: &'static [GuideSection],
}

impl ScreenGuide {
    fn for_screen(screen: crate::app::Screen) -> &'static ScreenGuide {
        match screen {
            crate::app::Screen::Home => &GUIDE_LIBRARY,
            crate::app::Screen::NowPlaying => &GUIDE_QUEUE,
            crate::app::Screen::Library => &GUIDE_LIBRARY,
            crate::app::Screen::Queue => &GUIDE_QUEUE,
            crate::app::Screen::Studio => &GUIDE_STUDIO,
            crate::app::Screen::PluginGraph => &GUIDE_PLUGIN_GRAPH,
            crate::app::Screen::Spectrum => &GUIDE_SPECTRUM,
            crate::app::Screen::Settings => &GUIDE_SETTINGS,
            crate::app::Screen::Recording => &GUIDE_RECORDING,
            crate::app::Screen::RoomEq => &GUIDE_ROOM_EQ,
            crate::app::Screen::HeadphoneEq => &GUIDE_HEADPHONE_EQ,
            crate::app::Screen::Spinorama => &GUIDE_SPINORAMA,
            crate::app::Screen::Playlists => &GUIDE_LIBRARY,
        }
    }
}

static GUIDE_LIBRARY: ScreenGuide = ScreenGuide {
    title: "Library Guide",
    overview: "The Library displays your music collection organized by album. Add directories containing audio files and SotF will scan and index them automatically.",
    sections: &[
        GuideSection {
            heading: "Adding Music",
            bullets: &[
                "Go to Settings > Library to add music directories.",
                "SotF scans for FLAC, MP3, AAC, ALAC, OGG, WAV, and more.",
                "Library rescans automatically when directories change.",
            ],
        },
        GuideSection {
            heading: "Browsing",
            bullets: &[
                "Use arrow keys or scroll to browse albums.",
                "Press '/' to search by album, artist, or title.",
                "Press 'T' to toggle between flat view and artist tree view.",
                "In tree view, use left/right arrows to collapse/expand artists.",
            ],
        },
        GuideSection {
            heading: "Sorting & Filtering",
            bullets: &[
                "Press 'S' or keys 1\u{2013}4 to sort by Artist, Album, Title, or Year.",
                "Press 'C' or keys 5\u{2013}9 to filter by channel count (Mono, Stereo, Surround, etc.).",
            ],
        },
        GuideSection {
            heading: "Playback",
            bullets: &[
                "Press Enter or 'A' to add an album to the queue and start playing.",
                "Right-click an album for more options (Add to Queue, Play Now).",
            ],
        },
    ],
};

static GUIDE_QUEUE: ScreenGuide = ScreenGuide {
    title: "Queue Guide",
    overview: "The Queue shows your current playback list. Albums are played in order and tracks within each album play sequentially.",
    sections: &[
        GuideSection {
            heading: "Navigation",
            bullets: &[
                "Use arrow keys to browse queue items.",
                "Press left/right to expand or collapse album tracks.",
                "Press Enter to play the selected album from its first track.",
            ],
        },
        GuideSection {
            heading: "Managing the Queue",
            bullets: &[
                "Press 'D' or Delete to remove the selected album from the queue.",
                "Press 'C' to clear the entire queue.",
                "Right-click for context menu options.",
            ],
        },
        GuideSection {
            heading: "Transport Controls",
            bullets: &[
                "Space: Play / Pause.",
                "N or '>': Next track.",
                "B or '<': Previous track.",
                "+/\u{2013}: Adjust volume. M: Toggle mute.",
            ],
        },
        GuideSection {
            heading: "Level Meters",
            bullets: &[
                "The meters panel shows real-time loudness per channel group.",
                "Use Tab to focus the meters panel, then arrow keys to select groups.",
                "Shift-M: Mute group. Shift-S: Solo group.",
            ],
        },
    ],
};

static GUIDE_STUDIO: ScreenGuide = ScreenGuide {
    title: "Studio / Plugin Rack Guide",
    overview: "The Studio hosts your plugin rack \u{2014} a chain of audio processors applied to the playback signal in real time. Plugins are processed in order from top to bottom.",
    sections: &[
        GuideSection {
            heading: "Adding Plugins",
            bullets: &[
                "Click the '+' button or use shortcut keys to add plugins:",
                "  E = Parametric EQ, U = Upmixer, G = Gain, L = Limiter",
                "  O = Compressor, B = Gate, and more.",
                "Each plugin type can be added multiple times.",
            ],
        },
        GuideSection {
            heading: "Editing Parameters",
            bullets: &[
                "Click a plugin card or press Enter to open its parameter editor.",
                "Use +/\u{2013} keys or drag knobs to adjust parameter values.",
                "Press Escape to close the editor.",
            ],
        },
        GuideSection {
            heading: "Managing the Chain",
            bullets: &[
                "Space: Toggle a plugin on/off (bypass).",
                "Shift-U / Shift-N: Move a plugin up/down in the chain.",
                "D or Delete: Remove a plugin from the chain.",
                "Drag plugin cards to reorder them.",
            ],
        },
        GuideSection {
            heading: "Presets",
            bullets: &[
                "Shift-S: Save the current plugin chain as a named preset.",
                "L: Load a previously saved preset.",
                "Presets store the full chain including all parameter values.",
            ],
        },
        GuideSection {
            heading: "Key Plugins",
            bullets: &[
                "EQ: Parametric equalizer with peak, shelf, and pass filters.",
                "Upmixer: Converts stereo to 5.0 surround via FFT spatial processing.",
                "Crossfeed: Blends stereo channels for headphone listening.",
                "Fletcher-Munson: Loudness compensation for low-volume listening.",
                "Compressor / Gate / Limiter: Dynamics processing.",
            ],
        },
    ],
};

static GUIDE_PLUGIN_GRAPH: ScreenGuide = ScreenGuide {
    title: "Plugin Graph Guide",
    overview: "The Plugin Graph provides a 2D visual representation of your audio processing chain. Nodes represent plugins and connections show the signal flow.",
    sections: &[
        GuideSection {
            heading: "Interaction",
            bullets: &[
                "Click and drag nodes to rearrange the graph layout.",
                "Drag from an output port to an input port to create connections.",
                "Click a node to select it, then press Delete to remove it.",
                "Space: Toggle the selected plugin on/off.",
            ],
        },
        GuideSection {
            heading: "Tips",
            bullets: &[
                "The graph view and rack view show the same chain \u{2014} changes sync automatically.",
                "Use the graph view for complex chains to visualize signal routing.",
            ],
        },
    ],
};

static GUIDE_SPECTRUM: ScreenGuide = ScreenGuide {
    title: "Spectrum Analyzer Guide",
    overview: "The Spectrum screen shows a real-time FFT frequency spectrum of the audio being played. It helps you visualize the frequency content and verify EQ corrections.",
    sections: &[
        GuideSection {
            heading: "Reading the Display",
            bullets: &[
                "X-axis: Frequency (20 Hz \u{2013} 20 kHz, logarithmic scale).",
                "Y-axis: Amplitude in dB.",
                "The curve updates in real time as audio plays.",
            ],
        },
        GuideSection {
            heading: "Usage Tips",
            bullets: &[
                "Compare before/after EQ by toggling plugins on and off.",
                "Look for resonant peaks that may need correction.",
                "Bass roll-off below 40 Hz is normal for most speakers.",
            ],
        },
    ],
};

static GUIDE_SETTINGS: ScreenGuide = ScreenGuide {
    title: "Settings Guide",
    overview: "Configure the application, manage your music library directories, and customize appearance.",
    sections: &[
        GuideSection {
            heading: "Appearance",
            bullets: &[
                "Theme: Choose between light and dark themes (T to cycle).",
                "Language: Select your preferred UI language (Alt-L to cycle).",
                "Font Scale: Adjust the overall UI size for readability.",
            ],
        },
        GuideSection {
            heading: "Library",
            bullets: &[
                "Add or remove music directories to scan.",
                "Configure the number of scanner threads for background analysis.",
                "ReplayGain, Waveform, and Bliss scanners run in the background.",
            ],
        },
        GuideSection {
            heading: "Audio",
            bullets: &[
                "Select the output audio device.",
                "Configure sample rate and buffer size.",
                "Adjust ReplayGain mode (Track, Album, Off).",
            ],
        },
        GuideSection {
            heading: "Plugins",
            bullets: &[
                "Save and load plugin chain presets from this section.",
                "Shift-S to save, L to load.",
            ],
        },
    ],
};

static GUIDE_RECORDING: ScreenGuide = ScreenGuide {
    title: "Recording Guide",
    overview: "Record acoustic measurements of your speakers or room using a calibrated microphone. Recordings are saved as data files for use in Room EQ optimization.",
    sections: &[
        GuideSection {
            heading: "Step 1: Configure",
            bullets: &[
                "Select the playback device (speakers) and recording device (microphone).",
                "Choose the signal type: sweep (recommended) or pink noise.",
                "Select which channels to measure.",
            ],
        },
        GuideSection {
            heading: "Step 2: Capture",
            bullets: &[
                "Position the microphone at your listening position.",
                "Click 'Record' for each channel to capture the measurement.",
                "Keep the room quiet during recording.",
            ],
        },
        GuideSection {
            heading: "Step 3: Evaluate",
            bullets: &[
                "Review the captured frequency response for each channel.",
                "Re-record individual channels if needed.",
            ],
        },
        GuideSection {
            heading: "Step 4: Save",
            bullets: &[
                "Export the measurement data for use in Room EQ optimization.",
                "Data is saved as CSV files in the project directory.",
            ],
        },
    ],
};

static GUIDE_ROOM_EQ: ScreenGuide = ScreenGuide {
    title: "Room EQ Guide",
    overview: "Optimize parametric EQ filters to correct room acoustics. Load your own measurements and let the optimizer find the best correction filters for each channel.",
    sections: &[
        GuideSection {
            heading: "Step 1: Load Data",
            bullets: &[
                "Load a JSON measurement file (from the Recording screen or external tools).",
                "The file should contain frequency response data per channel.",
            ],
        },
        GuideSection {
            heading: "Step 2: Configure",
            bullets: &[
                "Set the number of EQ filters (more filters = finer correction).",
                "Choose frequency range, Q limits, and gain bounds.",
                "Optionally set a target curve and crossover frequency.",
            ],
        },
        GuideSection {
            heading: "Step 3: Optimize",
            bullets: &[
                "Click 'Optimize' to run the multi-channel optimizer.",
                "The optimizer uses differential evolution to find optimal filter placement.",
                "Progress is shown in real time.",
            ],
        },
        GuideSection {
            heading: "Step 4: Review & Export",
            bullets: &[
                "Review per-channel results and frequency response plots.",
                "Export the correction filters to apply them in the plugin rack.",
            ],
        },
    ],
};

static GUIDE_HEADPHONE_EQ: ScreenGuide = ScreenGuide {
    title: "Headphone EQ Guide",
    overview: "Optimize EQ filters to match a target curve (e.g. Harman) for your headphones. Load your headphone's frequency response measurement and run the optimizer.",
    sections: &[
        GuideSection {
            heading: "Step 1: Select File",
            bullets: &[
                "Load a CSV file with your headphone's frequency response.",
                "Measurements can come from sites like AutoEQ, RTings, or your own recordings.",
            ],
        },
        GuideSection {
            heading: "Step 2: Configure",
            bullets: &[
                "Choose a target curve (Harman Over-Ear 2018, Harman In-Ear, or custom).",
                "Set the number of PEQ filters and optimization parameters.",
            ],
        },
        GuideSection {
            heading: "Step 3: Optimize",
            bullets: &[
                "Run the optimizer to find the best parametric EQ filters.",
                "Multiple algorithms are available (COBYLA, Differential Evolution, etc.).",
            ],
        },
        GuideSection {
            heading: "Step 4: Apply",
            bullets: &[
                "Review the corrected response curve.",
                "Apply the filters directly to the plugin rack's EQ.",
            ],
        },
    ],
};

static GUIDE_SPINORAMA: ScreenGuide = ScreenGuide {
    title: "Spinorama EQ Guide",
    overview: "Optimize speaker EQ using measurement data from spinorama.org. Search for your speaker model, choose a loss function, and let the optimizer find the best PEQ filters.",
    sections: &[
        GuideSection {
            heading: "Step 1: Select Speaker",
            bullets: &[
                "Search the spinorama.org database for your speaker model.",
                "Data includes on-axis, listening window, and predicted in-room response.",
            ],
        },
        GuideSection {
            heading: "Step 2: Configure",
            bullets: &[
                "Choose a loss function: 'speaker-flat' (minimize deviation) or 'speaker-score' (optimize Harman score).",
                "Set filter count, frequency range, and Q/gain limits.",
                "Select the optimization algorithm.",
            ],
        },
        GuideSection {
            heading: "Step 3: Optimize",
            bullets: &[
                "Run the optimizer. Progress and current best score are shown live.",
                "Higher filter counts give finer correction but may require longer optimization.",
            ],
        },
        GuideSection {
            heading: "Step 4: Apply",
            bullets: &[
                "Review the optimized frequency response curves.",
                "Load the resulting filters into the plugin rack's EQ.",
            ],
        },
    ],
};
