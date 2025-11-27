//! Dialog and modal rendering components

use crate::app::Screen;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_help_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let screen_name = match state.app.current_screen {
            Screen::Library => "Library",
            Screen::DirectoryManager => "Directories",
            Screen::Queue => "Queue",
            Screen::Spectrum => "Spectrum",
            Screen::Settings => "Settings",
        };

        // Get keybindings for current screen
        let keybindings = get_keybindings_for_screen(state.app.current_screen);

        // Create modal overlay (centered, 80% width, 90% height)
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa)) // Semi-transparent background
            .child(
                div()
                    .id("help-modal")
                    .w(Rems(60.0)) // 80% approx
                    .h(Rems(40.0)) // 90% approx
                    .bg(rgb(0x1e1e1e))
                    .border_2()
                    .border_color(rgb(0x007acc))
                    .rounded_md()
                    .p_4()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .mb_4()
                            .child(format!(
                                "Help - {} Screen (Press ESC or ? to close)",
                                screen_name
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            // Global keybindings section
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0x4ec9b0))
                                    .mb_2()
                                    .child("GLOBAL KEYBINDINGS"),
                            )
                            .child(self.render_keybinding(
                                "Shift-L/Q/P/O/D",
                                "Jump to Library/Queue/Plugins/Devices/Directories",
                            ))
                            .child(self.render_keybinding("+/=", "Increase volume"))
                            .child(self.render_keybinding("-/_", "Decrease volume"))
                            .child(self.render_keybinding("?", "Show this help"))
                            .child(div().h_4()) // Spacer
                            // Screen-specific keybindings section
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0x4ec9b0))
                                    .mb_2()
                                    .child(format!("{} KEYBINDINGS", screen_name.to_uppercase())),
                            )
                            .children(
                                keybindings
                                    .iter()
                                    .map(|(key, desc)| self.render_keybinding(key, desc)),
                            ),
                    ),
            )
    }

    pub(crate) fn render_keybinding(&self, key: &str, description: &str) -> impl IntoElement {
        div()
            .flex()
            .gap_4()
            .mb_1()
            .child(
                div()
                    .w(Rems(12.0))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x569cd6))
                    .child(format!("  {}", key)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xcccccc))
                    .child(description.to_string()),
            )
    }

    pub(crate) fn render_toast(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if let Some(toast) = &state.app.toast_message {
            let (bg_color, border_color, icon) = match toast.toast_type {
                crate::app::ToastType::Success => (rgb(0x1e3a1e), rgb(0x4ec9b0), "✓"),
                crate::app::ToastType::Error => (rgb(0x3a1e1e), rgb(0xf48771), "✗"),
                crate::app::ToastType::Info => (rgb(0x1e2a3a), rgb(0x569cd6), "ℹ"),
                crate::app::ToastType::Warning => (rgb(0x3a2e1e), rgb(0xdcdcaa), "⚠"),
            };

            div()
                .absolute()
                .top(px(20.0))
                .left_1_2()
                .min_w(Rems(25.0))
                .max_w(Rems(50.0))
                .bg(bg_color)
                .border_2()
                .border_color(border_color)
                .rounded_md()
                .shadow_lg()
                .p_3()
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .items_center()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .text_color(border_color)
                                .child(icon),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(rgb(0xffffff))
                                .child(toast.message.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x999999))
                                .child("ESC to dismiss"),
                        ),
                )
        } else {
            div() // Return empty div if no toast
        }
    }

    pub(crate) fn render_context_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if let Some(menu) = &state.app.context_menu {
            let menu_items: Vec<(&'static str, &'static str)> = match menu.menu_type {
                crate::app::ContextMenuType::Album => {
                    vec![("Add to Queue", "a"), ("Play Now", "enter")]
                }
                crate::app::ContextMenuType::QueueItem => {
                    vec![("Remove from Queue", "d"), ("Play from Here", "enter")]
                }
                crate::app::ContextMenuType::Plugin => vec![
                    ("Edit Plugin", "e"),
                    ("Toggle Enabled", "shift-t"),
                    ("Move Up", "u"),
                    ("Move Down", "shift-n"),
                    ("Remove Plugin", "d"),
                ],
                crate::app::ContextMenuType::Directory => {
                    vec![("Remove Directory", "d"), ("Rescan Library", "shift-s")]
                }
            };

            div()
                .absolute()
                .top(px(menu.position_y))
                .left(px(menu.position_x))
                .w(Rems(15.0))
                .bg(rgb(0x2d2d2d))
                .border_1()
                .border_color(rgb(0x007acc))
                .rounded_md()
                .shadow_lg()
                .overflow_hidden()
                .children(menu_items.into_iter().map(|(label, shortcut)| {
                    div()
                        .px_3()
                        .py_2()
                        .hover(|style| style.bg(rgb(0x3e3e3e)))
                        .cursor_pointer()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                // Close menu and execute action based on menu type
                                view.state.update(cx, |state, cx| {
                                    let menu_type = state
                                        .app
                                        .context_menu
                                        .as_ref()
                                        .map(|m| m.menu_type.clone());
                                    let item_idx = state
                                        .app
                                        .context_menu
                                        .as_ref()
                                        .map(|m| m.item_index)
                                        .unwrap_or(0);
                                    state.app.context_menu = None;

                                    if let Some(mt) = menu_type {
                                        match (mt, label) {
                                            (
                                                crate::app::ContextMenuType::Album,
                                                "Add to Queue",
                                            )
                                            | (crate::app::ContextMenuType::Album, "Play Now") => {
                                                if let Some(path) = state.app.add_album_to_queue() {
                                                    Self::play_track(state, path);
                                                }
                                            }
                                            (
                                                crate::app::ContextMenuType::QueueItem,
                                                "Remove from Queue",
                                            ) => {
                                                state.app.remove_from_queue(item_idx);
                                            }
                                            (
                                                crate::app::ContextMenuType::QueueItem,
                                                "Play from Here",
                                            ) => {
                                                state.app.current_queue_index = Some(item_idx);
                                                // Play the first track of the queue item
                                                if let Some(queue_item) =
                                                    state.app.queue.get(item_idx)
                                                {
                                                    if let Some(first_track) =
                                                        queue_item.album.tracks.first()
                                                    {
                                                        Self::play_track(
                                                            state,
                                                            first_track.path.clone(),
                                                        );
                                                    }
                                                }
                                            }
                                            (
                                                crate::app::ContextMenuType::Plugin,
                                                "Edit Plugin",
                                            ) => {
                                                state.app.enter_plugin_edit_mode();
                                            }
                                            (
                                                crate::app::ContextMenuType::Plugin,
                                                "Toggle Enabled",
                                            ) => {
                                                state.app.plugin_chain.toggle_plugin(item_idx);
                                            }
                                            (crate::app::ContextMenuType::Plugin, "Move Up") => {
                                                state.app.move_plugin_up(item_idx);
                                            }
                                            (crate::app::ContextMenuType::Plugin, "Move Down") => {
                                                state.app.move_plugin_down(item_idx);
                                            }
                                            (
                                                crate::app::ContextMenuType::Plugin,
                                                "Remove Plugin",
                                            ) => {
                                                state.app.plugin_chain.remove_plugin(item_idx);
                                            }
                                            (
                                                crate::app::ContextMenuType::Directory,
                                                "Remove Directory",
                                            ) => {
                                                state.app.selected_directory_index = item_idx;
                                                state.app.remove_selected_directory();
                                            }
                                            (
                                                crate::app::ContextMenuType::Directory,
                                                "Rescan Library",
                                            ) => {
                                                if let Err(e) = state.app.scan_library() {
                                                    log::error!("Scan failed: {}", e);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(div().text_sm().child(label))
                                .child(div().text_xs().text_color(rgb(0x666666)).child(shortcut)),
                        )
                }))
        } else {
            div() // Return empty div if no menu
        }
    }

    pub(crate) fn render_apo_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa)) // Semi-transparent background
            .child(
                div()
                    .w(Rems(40.0))
                    .bg(rgb(0x1e1e1e))
                    .border_2()
                    .border_color(rgb(0x007acc))
                    .rounded_md()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .mb_4()
                            .child("Load APO File for EQ Plugin"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .mb_2()
                            .text_color(rgb(0x999999))
                            .child("Enter path to APO file:"),
                    )
                    .child(
                        div()
                            .p_2()
                            .mb_4()
                            .rounded_md()
                            .bg(rgb(0x2d2d2d))
                            .border_1()
                            .border_color(rgb(0x007acc))
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{}█", state.app.apo_file_input)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("Enter: Load file | ESC: Cancel"),
                    ),
            )
    }

    pub(crate) fn render_sofa_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa)) // Semi-transparent background
            .child(
                div()
                    .w(Rems(40.0))
                    .bg(rgb(0x1e1e1e))
                    .border_2()
                    .border_color(rgb(0x007acc))
                    .rounded_md()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .mb_4()
                            .child("Load SOFA File for Binaural Decoder"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .mb_2()
                            .text_color(rgb(0x999999))
                            .child("Enter path to SOFA file:"),
                    )
                    .child(
                        div()
                            .p_2()
                            .mb_4()
                            .rounded_md()
                            .bg(rgb(0x2d2d2d))
                            .border_1()
                            .border_color(rgb(0x007acc))
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{}█", state.app.sofa_file_input)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("Enter: Load file | ESC: Cancel"),
                    ),
            )
    }

    pub(crate) fn render_save_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa)) // Semi-transparent background
            .child(
                div()
                    .w(Rems(50.0))
                    .max_h(Rems(30.0))
                    .bg(rgb(0x1e1e1e))
                    .border_2()
                    .border_color(rgb(0x007acc))
                    .rounded_md()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .mb_4()
                            .child("Save Plugin Preset"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .mb_2()
                            .text_color(rgb(0x999999))
                            .child("Enter preset name (or select existing to overwrite):"),
                    )
                    .child(
                        div()
                            .p_2()
                            .mb_4()
                            .rounded_md()
                            .bg(rgb(0x2d2d2d))
                            .border_1()
                            .border_color(rgb(0x007acc))
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{}█", state.app.plugin_file_input)),
                            ),
                    )
                    // Show existing presets if available
                    .when(!state.app.available_plugin_presets.is_empty(), |el| {
                        el.child(
                            div()
                                .text_sm()
                                .mb_2()
                                .text_color(rgb(0x999999))
                                .child("Existing presets (↑/↓ to select):"),
                        )
                        .child(
                            div()
                                .id("save-plugins-presets-list")
                                .max_h(Rems(12.0))
                                .overflow_y_scroll()
                                .bg(rgb(0x2d2d2d))
                                .rounded_md()
                                .p_2()
                                .children(state.app.available_plugin_presets.iter().enumerate().map(
                                    |(idx, preset)| {
                                        let is_selected = idx == state.app.selected_preset_index;
                                        div()
                                            .p_1()
                                            .rounded_md()
                                            .text_sm()
                                            .when(is_selected, |d| {
                                                d.bg(rgb(0x264f78)).text_color(rgb(0xffffff))
                                            })
                                            .when(!is_selected, |d| d.text_color(rgb(0xcccccc)))
                                            .child(preset.clone())
                                    },
                                )),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .mt_4()
                            .text_color(rgb(0x999999))
                            .child("Enter: Save | ↑/↓: Select preset | Tab: Autocomplete | ESC: Cancel"),
                    ),
            )
    }

    pub(crate) fn render_load_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa)) // Semi-transparent background
            .child(
                div()
                    .w(Rems(50.0))
                    .max_h(Rems(30.0))
                    .bg(rgb(0x1e1e1e))
                    .border_2()
                    .border_color(rgb(0x4ec9b0))
                    .rounded_md()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .mb_4()
                            .child("Load Plugin Preset"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .mb_2()
                            .text_color(rgb(0x999999))
                            .child("Enter preset name or select from list:"),
                    )
                    .child(
                        div()
                            .p_2()
                            .mb_4()
                            .rounded_md()
                            .bg(rgb(0x2d2d2d))
                            .border_1()
                            .border_color(rgb(0x4ec9b0))
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{}█", state.app.plugin_file_input)),
                            ),
                    )
                    // Show existing presets
                    .when(!state.app.available_plugin_presets.is_empty(), |el| {
                        el.child(
                            div()
                                .text_sm()
                                .mb_2()
                                .text_color(rgb(0x999999))
                                .child("Available presets (↑/↓ to select):"),
                        )
                        .child(
                            div()
                                .id("load-plugins-presets-list")
                                .max_h(Rems(12.0))
                                .overflow_y_scroll()
                                .bg(rgb(0x2d2d2d))
                                .rounded_md()
                                .p_2()
                                .children(state.app.available_plugin_presets.iter().enumerate().map(
                                    |(idx, preset)| {
                                        let is_selected = idx == state.app.selected_preset_index;
                                        div()
                                            .p_1()
                                            .rounded_md()
                                            .text_sm()
                                            .when(is_selected, |d| {
                                                d.bg(rgb(0x264f78)).text_color(rgb(0xffffff))
                                            })
                                            .when(!is_selected, |d| d.text_color(rgb(0xcccccc)))
                                            .child(preset.clone())
                                    },
                                )),
                        )
                    })
                    .when(state.app.available_plugin_presets.is_empty(), |el| {
                        el.child(
                            div()
                                .p_4()
                                .text_center()
                                .text_sm()
                                .text_color(rgb(0x999999))
                                .child("No presets found. Save a preset first with 's'."),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .mt_4()
                            .text_color(rgb(0x999999))
                            .child("Enter: Load | ↑/↓: Select preset | Tab: Autocomplete | ESC: Cancel"),
                    ),
            )
    }

    pub(crate) fn render_plugin_edit_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if let Some(plugin) = state.app.get_editing_plugin() {
            let plugin_name = plugin.plugin_type().name().to_string();
            let params = render_plugin_param_list(plugin, state.app.plugin_param_selection);

            // Create modal overlay
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(0x000000aa)) // Semi-transparent background
                .child(
                    div()
                        .id("plugin-edit-modal")
                        .w(Rems(50.0))
                        .h(Rems(35.0))
                        .bg(rgb(0x1e1e1e))
                        .border_2()
                        .border_color(rgb(0x4ec9b0))
                        .rounded_md()
                        .p_4()
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .mb_4()
                                .child(format!(
                                    "Edit Plugin: {} (Press ESC to close)",
                                    plugin_name
                                )),
                        )
                        .child(div().flex().flex_col().gap_2().children(
                            params.iter().enumerate().map(|(idx, (name, value))| {
                                let is_selected = idx == state.app.plugin_param_selection;
                                div()
                                    .p_2()
                                    .rounded_md()
                                    .when(is_selected, |div| div.bg(rgb(0x264f78)))
                                    .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if is_selected {
                                                        rgb(0xffffff)
                                                    } else {
                                                        rgb(0x569cd6)
                                                    })
                                                    .child(name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(if is_selected {
                                                        rgb(0xffffff)
                                                    } else {
                                                        rgb(0xcccccc)
                                                    })
                                                    .child(value.clone()),
                                            ),
                                    )
                            }),
                        ))
                        .child(
                            div()
                                .p_3()
                                .mt_4()
                                .rounded_md()
                                .bg(rgb(0x1e1e1e))
                                .text_xs()
                                .text_color(rgb(0x999999))
                                .child("↑/↓: Navigate params | ←/→: Adjust value | ESC: Exit"),
                        ),
                )
        } else {
            div() // Return empty div if no plugin is being edited
        }
    }
}

fn get_keybindings_for_screen(screen: Screen) -> Vec<(&'static str, &'static str)> {
    match screen {
        Screen::Library => vec![
            ("↑/↓ or K/J", "Navigate albums/artists"),
            ("PageUp/PageDown", "Jump by page"),
            ("/", "Search albums"),
            ("T", "Toggle tree view / flat view"),
            ("H/L or ←/→", "Collapse/expand artists in tree view"),
            ("S or 1/2/3/4", "Sort by Artist/Album/Title/Year"),
            ("C or 5/6/7/8/9", "Filter: All/Mono/Stereo/Multi/Mixed"),
            ("A or Enter", "Add album to queue"),
            ("Shift-Q", "Go to queue screen"),
        ],
        Screen::DirectoryManager => vec![
            ("↑/↓ or K/J", "Navigate directories"),
            ("PageUp/PageDown", "Jump by page"),
            ("Enter/→/L", "Expand/collapse directory"),
            ("Shift-A", "Add directory"),
            ("D/Delete", "Remove selected directory"),
            ("Shift-S", "Scan library"),
        ],
        Screen::Queue => vec![
            ("↑/↓ or K/J", "Navigate queue items"),
            ("Enter", "Play selected album from start"),
            ("H/L or ←/→", "Expand/collapse album tracks"),
            ("Space", "Play/Pause"),
            ("N or >", "Next track"),
            ("B or <", "Previous track"),
            ("D/Delete", "Remove from queue"),
            ("C", "Clear entire queue"),
        ],
        Screen::Spectrum => vec![("Space", "Play/Pause"), ("N", "Next track")],
        Screen::Settings => vec![
            ("T", "Cycle theme"),
            ("Alt-L", "Cycle language"),
        ],
    }
}

// Helper function to render plugin parameters for editing
fn render_plugin_param_list(
    plugin: &sotf_audio_player::Plugin,
    selected_idx: usize,
) -> Vec<(String, String)> {
    use sotf_audio_player::PluginSettings;

    match &plugin.settings {
        PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            lfe_cutoff_hz,
            stereo_width,
            bandpass_hz,
            height_gain,
            lfe_gain,
            enable_subharmonic_synth,
            subharmonic_gain,
            enable_hr_direct,
            hr_sharpen,
            safety_cap_db,
            decorrelation_mode,
        } => vec![
            ("Speaker Config".to_string(), speaker_config.clone()),
            (
                "Gain Front Direct".to_string(),
                format!("{:.1} dB", gain_front_direct),
            ),
            (
                "Gain Front Ambient".to_string(),
                format!("{:.1} dB", gain_front_ambient),
            ),
            (
                "Gain Rear Ambient".to_string(),
                format!("{:.1} dB", gain_rear_ambient),
            ),
            ("LFE Cutoff".to_string(), format!("{:.0} Hz", lfe_cutoff_hz)),
            ("Stereo Width".to_string(), format!("{:.2}", stereo_width)),
            ("Bandpass".to_string(), format!("{:.0} Hz", bandpass_hz)),
            ("Height Gain".to_string(), format!("{:.1} dB", height_gain)),
            ("LFE Gain".to_string(), format!("{:.1} dB", lfe_gain)),
            (
                "Subharmonic Synth".to_string(),
                if *enable_subharmonic_synth {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            (
                "Subharmonic Gain".to_string(),
                format!("{:.1} dB", subharmonic_gain),
            ),
            (
                "HR Direct".to_string(),
                if *enable_hr_direct {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            ("HR Sharpen".to_string(), format!("{:.2}", hr_sharpen)),
            ("Safety Cap".to_string(), format!("{:.1} dB", safety_cap_db)),
            (
                "Decorrelation".to_string(),
                format!("{:?}", decorrelation_mode),
            ),
        ],
        PluginSettings::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Ratio".to_string(), format!("{:.1}:1", ratio)),
            ("Attack".to_string(), format!("{:.1} ms", attack_ms)),
            ("Release".to_string(), format!("{:.0} ms", release_ms)),
            ("Knee".to_string(), format!("{:.1} dB", knee_db)),
            (
                "Makeup Gain".to_string(),
                format!("{:.1} dB", makeup_gain_db),
            ),
            ("Mix".to_string(), format!("{:.0}%", mix * 100.0)),
            (
                "Auto Makeup".to_string(),
                if *auto_makeup {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            (
                "Link Channels".to_string(),
                if *link_channels {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            (
                "Sidechain HPF".to_string(),
                format!("{:.0} Hz", sidechain_hpf_hz),
            ),
        ],
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
            mix,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Release".to_string(), format!("{:.0} ms", release_ms)),
            ("Mix".to_string(), format!("{:.0}%", mix * 100.0)),
        ],
        PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        } => vec![
            ("Threshold".to_string(), format!("{:.1} dB", threshold_db)),
            ("Ratio".to_string(), format!("{:.1}:1", ratio)),
            ("Attack".to_string(), format!("{:.1} ms", attack_ms)),
            ("Release".to_string(), format!("{:.0} ms", release_ms)),
            ("Mix".to_string(), format!("{:.0}%", mix * 100.0)),
            (
                "Link Channels".to_string(),
                if *link_channels {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            (
                "Sidechain HPF".to_string(),
                format!("{:.0} Hz", sidechain_hpf_hz),
            ),
        ],
        PluginSettings::LoudnessCompensation {
            target_lufs,
            min_gain_db,
            max_gain_db,
        } => vec![
            (
                "Target LUFS".to_string(),
                format!("{:.1} LUFS", target_lufs),
            ),
            ("Min Gain".to_string(), format!("{:.1} dB", min_gain_db)),
            ("Max Gain".to_string(), format!("{:.1} dB", max_gain_db)),
        ],
        PluginSettings::EQ { filters } => {
            let mut params = vec![];
            for (i, filter) in filters.iter().enumerate() {
                params.push((
                    format!("Filter {} Freq", i + 1),
                    format!("{:.0} Hz", filter.frequency),
                ));
                params.push((format!("Filter {} Q", i + 1), format!("{:.2}", filter.q)));
                params.push((
                    format!("Filter {} Gain", i + 1),
                    format!("{:.1} dB", filter.gain_db),
                ));
                params.push((
                    format!("Filter {} Type", i + 1),
                    format!("{:?}", filter.filter_type),
                ));
            }
            params
        }
        PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
        } => vec![
            (
                "SOFA File".to_string(),
                if sofa_file.is_empty() {
                    "None".to_string()
                } else {
                    sofa_file.clone()
                },
            ),
            ("Input Channels".to_string(), format!("{}", input_channels)),
            (
                "Optimization".to_string(),
                if *enable_optimization {
                    "On".to_string()
                } else {
                    "Off".to_string()
                },
            ),
            (
                "Externalization".to_string(),
                format!("{:.2}", externalization),
            ),
            (
                "Near Field".to_string(),
                format!("{:.2}", near_field_strength),
            ),
        ],
        PluginSettings::Convolution {
            ir_file,
            mix,
            gain_db,
        } => vec![
            (
                "IR File".to_string(),
                if ir_file.is_empty() {
                    "None".to_string()
                } else {
                    ir_file.clone()
                },
            ),
            ("Mix".to_string(), format!("{:.0}%", mix * 100.0)),
            ("Gain".to_string(), format!("{:.1} dB", gain_db)),
        ],
        PluginSettings::LoudnessMonitor => vec![(
            "Info".to_string(),
            "Real-time LUFS and peak meters".to_string(),
        )],
        PluginSettings::SpectrumAnalyzer {
            num_bins,
            min_freq,
            max_freq,
            smoothing,
        } => vec![
            ("Frequency Bins".to_string(), format!("{}", num_bins)),
            ("Min Frequency".to_string(), format!("{:.0} Hz", min_freq)),
            ("Max Frequency".to_string(), format!("{:.0} Hz", max_freq)),
            ("Smoothing".to_string(), format!("{:.2}", smoothing)),
        ],
        PluginSettings::Gain { gain_db } => vec![("Gain".to_string(), format!("{:.1} dB", gain_db))],
        PluginSettings::ChannelMuteSolo { .. } => {
            vec![("Mute/Solo".to_string(), "Use meters panel".to_string())]
        }
    }
}

impl PlayerView {
    /// Render a comprehensive keyboard shortcuts dialog
    pub(crate) fn render_keyboard_shortcuts_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();

        let global_shortcuts = vec![
            ("Space", "Play / Pause"),
            ("N", "Next track"),
            ("P", "Previous track"),
            ("+/-", "Volume up/down"),
            ("M", "Toggle mute"),
            ("1-5", "Switch screens (Library/Queue/Plugins/Devices/Settings)"),
            ("?", "Toggle help"),
            ("Esc", "Close dialog / Cancel"),
            ("T", "Cycle theme"),
            ("Alt-L", "Cycle language"),
            ("Cmd-Q", "Quit"),
        ];

        let library_shortcuts = vec![
            ("↑/↓ or K/J", "Navigate albums"),
            ("Enter", "Add album to queue and play"),
            ("Q", "Add album to queue"),
            ("/", "Search"),
            ("V", "Toggle grid/list view"),
            ("S", "Cycle sort order"),
            ("C", "Cycle channel filter"),
        ];

        let queue_shortcuts = vec![
            ("↑/↓ or K/J", "Navigate queue"),
            ("X", "Remove from queue"),
            ("Shift-X", "Clear queue"),
            ("Tab", "Select meter group"),
            ("Shift-M", "Mute selected group"),
            ("Shift-S", "Solo selected group"),
        ];

        let plugin_shortcuts = vec![
            ("E/U/G/L/O/B", "Add EQ/Upmixer/Gate/Limiter/Loudness/Binaural"),
            ("Enter/e", "Edit plugin parameters"),
            ("D/Delete", "Delete plugin"),
            ("Space", "Toggle plugin on/off"),
            ("Shift-U/N", "Move plugin up/down"),
            ("Shift-S/l", "Save/Load plugin preset"),
        ];

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000099))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.input_mode = crate::app::InputMode::Normal;
                    });
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("shortcuts-dialog")
                    .w(px(700.0))
                    .max_h(px(600.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .p_6()
                    .overflow_y_scroll()
                    // Header
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .mb_4()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child("Keyboard Shortcuts"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("Press ESC to close"),
                            ),
                    )
                    // Shortcut sections
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_6()
                            // Global
                            .child(self.render_shortcut_section("Global", &global_shortcuts, &theme))
                            // Library
                            .child(self.render_shortcut_section("Library", &library_shortcuts, &theme))
                            // Queue
                            .child(self.render_shortcut_section("Queue", &queue_shortcuts, &theme))
                            // Plugins
                            .child(self.render_shortcut_section("Plugins", &plugin_shortcuts, &theme)),
                    ),
            )
    }

    fn render_shortcut_section(
        &self,
        title: &str,
        shortcuts: &[(&str, &str)],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .min_w(px(280.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.accent)
                    .mb_2()
                    .child(title.to_string()),
            )
            .children(shortcuts.iter().map(|(key, desc)| {
                let theme = theme.clone();
                div()
                    .flex()
                    .justify_between()
                    .py_1()
                    .child(
                        div()
                            .px_2()
                            .py(px(2.0))
                            .bg(theme.surface_hover)
                            .rounded(px(3.0))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_primary)
                            .child(key.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_secondary)
                            .child(desc.to_string()),
                    )
            }))
    }

    /// Render a small About dialog
    pub(crate) fn render_about_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000099))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.input_mode = crate::app::InputMode::Normal;
                    });
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("about-dialog")
                    .w(px(320.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .p_6()
                    // Logo/Title
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.accent)
                                    .child("SotF"),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .text_color(theme.text_primary)
                                    .child("Sound of the Future"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.text_secondary)
                                    .child("Audio Player & Processing Engine"),
                            ),
                    )
                    // Version info
                    .child(
                        div()
                            .mt_4()
                            .pt_4()
                            .border_t_1()
                            .border_color(theme.border)
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("Version 0.5.3")
                            .child("Built with Rust & GPUI")
                            .child("spinorama.org")
                    )
                    // Close hint
                    .child(
                        div()
                            .mt_4()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .text_center()
                            .child("Click anywhere to close"),
                    ),
            )
    }
}
