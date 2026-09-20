//! Keybindings settings content

#[cfg(feature = "dev-api")]
use crate::app::dev_api::{DevElementState, DevTrackExt};
use crate::app::i18n::KeybindingTranslations;
use crate::app::keybindings::{
    CustomKeybinding, KeybindingCategory, KeymapPreset, get_documented_keybindings,
    get_documented_keybindings_with_overrides, get_keybindings_with_overrides, keybinding_conflict,
    set_custom_keybinding,
};
use crate::app::types::PreferencesSetting;
use crate::app::{AppState, DocumentedKeybinding};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::accessibility::{
    AccessibilityExt, AccessibilityNode, AriaProps, AriaRole, AriaState,
};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use std::collections::HashMap;

thread_local! {
    static KEYMAP_PRESET_FOCUS_HANDLES: std::cell::RefCell<HashMap<ElementId, FocusHandle>> =
        std::cell::RefCell::new(HashMap::new());
}

fn keymap_preset_focus_handle(id: &ElementId, cx: &mut App) -> FocusHandle {
    KEYMAP_PRESET_FOCUS_HANDLES.with(|handles| {
        handles
            .borrow_mut()
            .entry(id.clone())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    })
}

fn focus_keymap_preset_relative(
    handles: &[FocusHandle],
    window: &mut Window,
    cx: &mut App,
    backwards: bool,
) -> bool {
    let Some(current) = handles.iter().position(|handle| handle.is_focused(window)) else {
        return false;
    };
    let next = if backwards {
        current.checked_sub(1)
    } else {
        (current + 1 < handles.len()).then_some(current + 1)
    };
    let Some(next) = next else {
        return false;
    };
    window.focus(&handles[next], cx);
    true
}

fn activate_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
}

fn keybinding_selector_id(action_name: &str) -> String {
    action_name
        .rsplit("::")
        .next()
        .unwrap_or(action_name)
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn friendly_action_name(
    preset: KeymapPreset,
    action_name: &str,
    text: KeybindingTranslations,
) -> String {
    get_documented_keybindings(preset)
        .into_iter()
        .find(|binding| binding.action_name == Some(action_name))
        .map(|binding| text.action_description(binding.description).to_string())
        .unwrap_or_else(|| {
            action_name
                .rsplit("::")
                .next()
                .unwrap_or(action_name)
                .to_string()
        })
}

fn apply_keymap_and_persist(state: &mut AppState, cx: &mut App) {
    cx.clear_key_bindings();
    cx.bind_keys(get_keybindings_with_overrides(
        state.app.ui_state.keymap_preset,
        &state.app.settings.keybindings.overrides,
    ));
    let layout = state.layout.read(cx);
    if let Err(error) = state.app.save_config(layout) {
        log::error!("Failed to save keybindings: {error}");
    }
}

impl PlayerView {
    #[allow(clippy::type_complexity)]
    pub(crate) fn render_keybindings_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let current_preset = state.app.ui_state.keymap_preset;
        let theme = state.app.ui_state.theme.clone();
        let text = KeybindingTranslations::for_language(state.app.ui_state.language);
        let custom_overrides = state.app.settings.keybindings.overrides.clone();
        let editing_action = state.app.settings.keybindings.editing_action.clone();
        let pending_key_spec = state.app.settings.keybindings.pending_key_spec.clone();
        let pending_conflict = state.app.settings.keybindings.conflict.clone();
        let confirm_reset_all = state.app.settings.keybindings.confirm_reset_all;
        let compact_editor = state.app.ui_state.window_width
            / state.app.ui_state.font_scale.max(f32::EPSILON)
            < 600.0;

        // Build comparison data: action -> preset -> key
        let comparison = build_keybinding_comparison(text);
        let mut editable_bindings = get_documented_keybindings(current_preset)
            .into_iter()
            .filter(|binding| binding.action_name.is_some())
            .collect::<Vec<_>>();
        editable_bindings.sort_by_key(|binding| match binding.category {
            KeybindingCategory::ScreenSwitch => 0,
            KeybindingCategory::Playback => 1,
            KeybindingCategory::Navigation => 2,
            KeybindingCategory::Library => 3,
            KeybindingCategory::Queue => 4,
            KeybindingCategory::Plugins => 5,
            KeybindingCategory::ListeningTests => 6,
            KeybindingCategory::LevelMeters => 7,
            KeybindingCategory::System => 8,
        });
        // Search opens the playback shortcut at the start of the scrollable list.
        // Preserve the usual category order for ordinary navigation.
        if state.app.settings.navigation.setting == Some(PreferencesSetting::Shortcuts) {
            editable_bindings.sort_by_key(|binding| {
                binding
                    .action_name
                    .is_none_or(|name| keybinding_selector_id(name) != "playpause")
            });
        }
        let effective_bindings =
            get_documented_keybindings_with_overrides(current_preset, &custom_overrides);

        // Group by category
        let mut by_category: HashMap<
            KeybindingCategory,
            Vec<(String, HashMap<KeymapPreset, String>)>,
        > = HashMap::new();
        for (action, cat, keys) in comparison {
            by_category.entry(cat).or_default().push((action, keys));
        }

        let preset_controls = KeymapPreset::all()
            .iter()
            .map(|preset| {
                let preset = *preset;
                let selected = preset == current_preset;
                let label = text.preset_name(preset);
                let element_id = ElementId::from(SharedString::from(format!(
                    "keymap-preset-{}",
                    preset.name()
                )));
                let focus_handle = keymap_preset_focus_handle(&element_id, cx);
                cx.register_accessible(AccessibilityNode {
                    element_id: element_id.clone(),
                    label: label.into(),
                    props: AriaProps::with_role(AriaRole::Button)
                        .maybe_state(selected, AriaState::Pressed(true)),
                });

                let state_for_click = self.state.clone();
                let state_for_key = self.state.clone();
                let button = Button::new(element_id, label)
                    .variant(if selected {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Sm)
                    .selected(selected)
                    .theme(theme.to_button_theme())
                    .build()
                    .text_size(d.text_sm)
                    .px(d.pad_x)
                    .py(d.pad_y_half)
                    .track_focus(&focus_handle)
                    .track_focus_element(&focus_handle)
                    .on_click(move |_: &ClickEvent, _window, cx| {
                        state_for_click.update(cx, |state, cx| {
                            state.app.set_keymap_preset(preset);
                            state.app.settings.keybindings.editing_action = None;
                            state.app.settings.keybindings.capturing = false;
                            state.app.settings.keybindings.pending_key_spec = None;
                            state.app.settings.keybindings.conflict = None;
                            apply_keymap_and_persist(state, cx);
                            cx.notify();
                        });
                    })
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            state_for_key.update(cx, |state, cx| {
                                state.app.set_keymap_preset(preset);
                                state.app.settings.keybindings.editing_action = None;
                                state.app.settings.keybindings.capturing = false;
                                state.app.settings.keybindings.pending_key_spec = None;
                                state.app.settings.keybindings.conflict = None;
                                apply_keymap_and_persist(state, cx);
                                cx.notify();
                            });
                            cx.stop_propagation();
                        }
                    });

                #[cfg(feature = "dev-api")]
                let button = button.dev_track_with_state(
                    format!("settings.keymap-preset.{}", preset.name()),
                    DevElementState::default().selected(selected),
                );

                (button.into_any_element(), focus_handle)
            })
            .collect::<Vec<_>>();
        let preset_focus_handles = preset_controls
            .iter()
            .map(|(_, handle)| handle.clone())
            .collect::<Vec<_>>();
        let preset_buttons = preset_controls
            .into_iter()
            .map(|(button, _)| button)
            .collect::<Vec<_>>();

        let content = div()
            .id("keybindings-content")
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .size_full()
            .min_h_0()
            .when(compact_editor, |element| element.overflow_y_scroll())
            // Preset selection
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
                            .child(text.keymap_preset),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(d.gap_md)
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                if event.keystroke.key.as_str() == "tab"
                                    && focus_keymap_preset_relative(
                                        &preset_focus_handles,
                                        window,
                                        cx,
                                        event.keystroke.modifiers.shift,
                                    )
                                {
                                    cx.stop_propagation();
                                }
                            })
                            .children(preset_buttons),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(text.preset_description(current_preset)),
                    ),
            )
            .child(render_custom_keybinding_editor(
                self,
                &d,
                &theme,
                text,
                current_preset,
                compact_editor,
                editable_bindings,
                effective_bindings,
                custom_overrides,
                editing_action,
                pending_key_spec,
                pending_conflict,
                confirm_reset_all,
                self.state.clone(),
                cx,
            ))
            // Comparison table
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(text.comparison),
                    )
                    .child(
                        div()
                            .id("keybindings-table")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .border_1()
                            .border_color(theme.border)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            // Table header
                            .child(render_table_header(&d, text, &theme))
                            // Table body by category
                            .children(KeybindingCategory::all().iter().filter_map(|category| {
                                by_category.get(category).map(|rows| {
                                    render_category_section(
                                        &d,
                                        *category,
                                        rows,
                                        &theme,
                                        current_preset,
                                        text,
                                    )
                                })
                            })),
                    ),
            );
        #[cfg(feature = "dev-api")]
        let content = content.dev_track("settings.keybinding.content");
        content
    }
}

#[allow(clippy::too_many_arguments)]
fn render_custom_keybinding_editor(
    view: &PlayerView,
    d: &Ds,
    theme: &crate::app::Theme,
    text: KeybindingTranslations,
    current_preset: KeymapPreset,
    compact: bool,
    editable_bindings: Vec<DocumentedKeybinding>,
    effective_bindings: Vec<DocumentedKeybinding>,
    custom_overrides: Vec<CustomKeybinding>,
    editing_action: Option<String>,
    pending_key_spec: Option<String>,
    pending_conflict: Option<crate::app::keybindings::KeybindingConflict>,
    confirm_reset_all: bool,
    state_entity: Entity<AppState>,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let effective_by_action = effective_bindings
        .into_iter()
        .filter_map(|binding| {
            binding
                .action_name
                .map(|action_name| (action_name.to_string(), binding.key))
        })
        .collect::<HashMap<_, _>>();
    let custom_actions = custom_overrides
        .iter()
        .map(|custom| custom.action_name.as_str())
        .collect::<std::collections::HashSet<_>>();
    let capture_focus = keymap_preset_focus_handle(
        &ElementId::from(SharedString::from("keybinding-capture")),
        cx,
    );

    let rows = editable_bindings
        .into_iter()
        .filter_map(|binding| {
            let action_name = binding.action_name?;
            let selector_id = keybinding_selector_id(action_name);
            let action_label = text.action_description(binding.description).to_string();
            let current_key = effective_by_action
                .get(action_name)
                .cloned()
                .unwrap_or_else(|| text.not_assigned.to_string());
            let is_custom = custom_actions.contains(action_name);
            let edit_id =
                ElementId::from(SharedString::from(format!("keybinding-edit-{selector_id}")));
            let is_search_target = selector_id == "playpause";
            let edit_focus = if is_search_target {
                view.preference_focus_handle(PreferencesSetting::Shortcuts, cx)
                    .unwrap_or_else(|| keymap_preset_focus_handle(&edit_id, cx))
            } else {
                keymap_preset_focus_handle(&edit_id, cx)
            };
            cx.register_accessible(AccessibilityNode {
                element_id: edit_id.clone(),
                label: format!("{}: {}", text.edit, action_label).into(),
                props: AriaProps::with_role(AriaRole::Button),
            });

            let click_state = state_entity.clone();
            let key_state = state_entity.clone();
            let click_action = action_name.to_string();
            let key_action = click_action.clone();
            let click_capture_focus = capture_focus.clone();
            let key_capture_focus = capture_focus.clone();
            let edit_button = Button::new(edit_id, text.edit)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Sm)
                .theme(theme.to_button_theme())
                .build()
                .text_size(d.text_sm)
                .px(d.pad_x)
                .py(d.pad_y_half)
                .track_focus(&edit_focus)
                .track_focus_element(&edit_focus)
                .on_click(move |_: &ClickEvent, window, cx| {
                    click_state.update(cx, |state, cx| {
                        state.app.settings.keybindings.editing_action = Some(click_action.clone());
                        state.app.settings.keybindings.capturing = true;
                        state.app.settings.keybindings.pending_key_spec = None;
                        state.app.settings.keybindings.conflict = None;
                        state.app.settings.keybindings.confirm_reset_all = false;
                        cx.notify();
                    });
                    window.focus(&click_capture_focus, cx);
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if activate_key(event) {
                        key_state.update(cx, |state, cx| {
                            state.app.settings.keybindings.editing_action =
                                Some(key_action.clone());
                            state.app.settings.keybindings.capturing = true;
                            state.app.settings.keybindings.pending_key_spec = None;
                            state.app.settings.keybindings.conflict = None;
                            state.app.settings.keybindings.confirm_reset_all = false;
                            cx.notify();
                        });
                        window.focus(&key_capture_focus, cx);
                        cx.stop_propagation();
                    }
                });
            #[cfg(feature = "dev-api")]
            let edit_button =
                edit_button.dev_track(format!("settings.keybinding.edit.{selector_id}"));
            let edit_button = if is_search_target {
                view.preference_control(PreferencesSetting::Shortcuts, edit_button, cx)
            } else {
                edit_button.into_any_element()
            };

            let reset_button = is_custom.then(|| {
                let reset_id = ElementId::from(SharedString::from(format!(
                    "keybinding-reset-{selector_id}"
                )));
                let reset_focus = keymap_preset_focus_handle(&reset_id, cx);
                cx.register_accessible(AccessibilityNode {
                    element_id: reset_id.clone(),
                    label: format!("{}: {}", text.reset_this, action_label).into(),
                    props: AriaProps::with_role(AriaRole::Button),
                });
                let click_state = state_entity.clone();
                let key_state = state_entity.clone();
                let click_action = action_name.to_string();
                let key_action = click_action.clone();
                let button = Button::new(reset_id, text.reset_this)
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .build()
                    .text_size(d.text_sm)
                    .px(d.pad_x)
                    .py(d.pad_y_half)
                    .track_focus(&reset_focus)
                    .track_focus_element(&reset_focus)
                    .on_click(move |_: &ClickEvent, _window, cx| {
                        click_state.update(cx, |state, cx| {
                            state
                                .app
                                .settings
                                .keybindings
                                .overrides
                                .retain(|custom| custom.action_name != click_action);
                            state.app.settings.keybindings.editing_action = None;
                            state.app.settings.keybindings.pending_key_spec = None;
                            state.app.settings.keybindings.conflict = None;
                            apply_keymap_and_persist(state, cx);
                            cx.notify();
                        });
                    })
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                        if activate_key(event) {
                            key_state.update(cx, |state, cx| {
                                state
                                    .app
                                    .settings
                                    .keybindings
                                    .overrides
                                    .retain(|custom| custom.action_name != key_action);
                                state.app.settings.keybindings.editing_action = None;
                                state.app.settings.keybindings.pending_key_spec = None;
                                state.app.settings.keybindings.conflict = None;
                                apply_keymap_and_persist(state, cx);
                                cx.notify();
                            });
                            cx.stop_propagation();
                        }
                    });
                #[cfg(feature = "dev-api")]
                let button = button.dev_track(format!("settings.keybinding.reset.{selector_id}"));
                button.into_any_element()
            });

            Some(
                div()
                    .flex()
                    .items_center()
                    .when(compact, |element| element.flex_col().items_start())
                    .gap(d.gap_md)
                    .w_full()
                    .px(d.pad_x)
                    .py(d.pad_y_half)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(action_label),
                    )
                    .child(
                        div()
                            .w(rems(8.0))
                            .when(compact, |element| element.w_full())
                            .text_size(d.text_xs)
                            .text_color(if is_custom {
                                theme.accent
                            } else {
                                theme.text_muted
                            })
                            .child(current_key),
                    )
                    .when(is_custom, |element| {
                        element.child(
                            div()
                                .text_size(d.text_xs)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.accent)
                                .child(text.custom),
                        )
                    })
                    .child(edit_button)
                    .children(reset_button),
            )
        })
        .collect::<Vec<_>>();

    let reset_all_button = (!custom_overrides.is_empty() && !confirm_reset_all).then(|| {
        let id = ElementId::from(SharedString::from("keybinding-reset-all"));
        let focus = keymap_preset_focus_handle(&id, cx);
        cx.register_accessible(AccessibilityNode {
            element_id: id.clone(),
            label: text.reset_all.into(),
            props: AriaProps::with_role(AriaRole::Button),
        });
        let click_state = state_entity.clone();
        let key_state = state_entity.clone();
        let button = Button::new(id, text.reset_all)
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Sm)
            .theme(theme.to_button_theme())
            .build()
            .text_size(d.text_sm)
            .px(d.pad_x)
            .py(d.pad_y_half)
            .track_focus(&focus)
            .track_focus_element(&focus)
            .on_click(move |_: &ClickEvent, _window, cx| {
                click_state.update(cx, |state, cx| {
                    state.app.settings.keybindings.confirm_reset_all = true;
                    state.app.settings.keybindings.editing_action = None;
                    state.app.settings.keybindings.pending_key_spec = None;
                    state.app.settings.keybindings.conflict = None;
                    cx.notify();
                });
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                if activate_key(event) {
                    key_state.update(cx, |state, cx| {
                        state.app.settings.keybindings.confirm_reset_all = true;
                        state.app.settings.keybindings.editing_action = None;
                        state.app.settings.keybindings.pending_key_spec = None;
                        state.app.settings.keybindings.conflict = None;
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            });
        #[cfg(feature = "dev-api")]
        let button = button.dev_track("settings.keybinding.reset-all");
        button.into_any_element()
    });

    let confirmation = confirm_reset_all.then(|| {
        let cancel_state = state_entity.clone();
        let confirm_state = state_entity.clone();
        let cancel = Button::new("keybinding-reset-all-cancel", text.cancel)
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Sm)
            .theme(theme.to_button_theme())
            .build()
            .text_size(d.text_sm)
            .px(d.pad_x)
            .py(d.pad_y_half)
            .on_click(move |_: &ClickEvent, _window, cx| {
                cancel_state.update(cx, |state, cx| {
                    state.app.settings.keybindings.confirm_reset_all = false;
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let cancel = cancel.dev_track("settings.keybinding.reset-all-cancel");

        let confirm = Button::new("keybinding-reset-all-confirm", text.reset_all)
            .variant(ButtonVariant::Destructive)
            .size(ButtonSize::Sm)
            .theme(theme.to_button_theme())
            .build()
            .text_size(d.text_sm)
            .px(d.pad_x)
            .py(d.pad_y_half)
            .on_click(move |_: &ClickEvent, _window, cx| {
                confirm_state.update(cx, |state, cx| {
                    state.app.settings.keybindings.overrides.clear();
                    state.app.settings.keybindings.confirm_reset_all = false;
                    apply_keymap_and_persist(state, cx);
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let confirm = confirm.dev_track("settings.keybinding.reset-all-confirm");

        div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .p(d.pad_x)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.warning)
            .bg(theme.surface)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child(text.reset_all_question),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_secondary)
                    .child(text.reset_all_warning),
            )
            .child(div().flex().gap(d.gap_md).child(cancel).child(confirm))
    });

    let editor = editing_action.map(|action_name| {
        let action_label = friendly_action_name(current_preset, &action_name, text);
        let capture_id = ElementId::from(SharedString::from("keybinding-capture"));
        cx.register_accessible(AccessibilityNode {
            element_id: capture_id.clone(),
            label: format!("{}: {}", text.capture_prompt, action_label).into(),
            props: AriaProps::with_role(AriaRole::Button),
        });

        let capture_state = state_entity.clone();
        let capture_action = action_name.clone();
        let capture = div()
            .id(capture_id)
            .focusable()
            .track_focus(&capture_focus)
            .track_focus_element(&capture_focus)
            .focus_visible(|style| {
                style
                    .border_color(theme.border_focused)
                    .bg(theme.surface_hover)
            })
            .cursor_pointer()
            .border_1()
            .border_color(theme.border)
            .rounded(d.r_md)
            .px(d.pad_x)
            .py(d.pad_y)
            .text_size(d.text_sm)
            .text_color(if pending_key_spec.is_some() {
                theme.accent
            } else {
                theme.text_muted
            })
            .child(
                pending_key_spec
                    .clone()
                    .unwrap_or_else(|| text.capture_prompt.to_string()),
            )
            .on_mouse_down(MouseButton::Left, {
                let capture_focus = capture_focus.clone();
                move |_event, window, cx| window.focus(&capture_focus, cx)
            })
            .on_click({
                let capture_focus = capture_focus.clone();
                let capture_state = state_entity.clone();
                move |_: &ClickEvent, window, cx| {
                    capture_state.update(cx, |state, _cx| {
                        state.app.settings.keybindings.capturing = true;
                    });
                    window.focus(&capture_focus, cx);
                }
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "tab" {
                    return;
                }
                if key == "escape" {
                    capture_state.update(cx, |state, cx| {
                        state.app.settings.keybindings.editing_action = None;
                        state.app.settings.keybindings.capturing = false;
                        state.app.settings.keybindings.pending_key_spec = None;
                        state.app.settings.keybindings.conflict = None;
                        cx.notify();
                    });
                    cx.stop_propagation();
                    return;
                }
                if matches!(key, "shift" | "control" | "alt" | "command" | "fn") {
                    cx.stop_propagation();
                    return;
                }
                let key_spec = event.keystroke.to_string();
                capture_state.update(cx, |state, cx| {
                    state.app.settings.keybindings.conflict = keybinding_conflict(
                        state.app.ui_state.keymap_preset,
                        &state.app.settings.keybindings.overrides,
                        &capture_action,
                        &key_spec,
                    )
                    .unwrap_or_else(|error| {
                        log::warn!("Could not capture keybinding: {error}");
                        None
                    });
                    state.app.settings.keybindings.pending_key_spec = Some(key_spec.clone());
                    cx.notify();
                });
                cx.stop_propagation();
            });
        #[cfg(feature = "dev-api")]
        let capture = capture.dev_track("settings.keybinding.capture");

        let conflict = pending_conflict.as_ref().map(|conflict| {
            let existing =
                friendly_action_name(current_preset, &conflict.existing_action_name, text);
            let status_id = ElementId::from(SharedString::from("keybinding-conflict"));
            cx.register_accessible(AccessibilityNode {
                element_id: status_id.clone(),
                label: format!("{}: {}", text.conflict_prefix, existing).into(),
                props: AriaProps::with_role(AriaRole::Status),
            });
            let status = div()
                .id(status_id)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.warning)
                .child(format!(
                    "{}: {} ({})",
                    text.conflict_prefix, existing, conflict.key_spec
                ));
            #[cfg(feature = "dev-api")]
            let status = status.dev_track("settings.keybinding.conflict");
            status.into_any_element()
        });

        let cancel_state = state_entity.clone();
        let cancel = Button::new("keybinding-edit-cancel", text.cancel)
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Sm)
            .theme(theme.to_button_theme())
            .build()
            .text_size(d.text_sm)
            .px(d.pad_x)
            .py(d.pad_y_half)
            .on_click(move |_: &ClickEvent, _window, cx| {
                cancel_state.update(cx, |state, cx| {
                    state.app.settings.keybindings.editing_action = None;
                    state.app.settings.keybindings.capturing = false;
                    state.app.settings.keybindings.pending_key_spec = None;
                    state.app.settings.keybindings.conflict = None;
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let cancel = cancel.dev_track("settings.keybinding.cancel");

        let save = pending_key_spec.clone().map(|key_spec| {
            let save_state = state_entity.clone();
            let save_action = action_name.clone();
            let has_conflict = pending_conflict.is_some();
            let label = if has_conflict {
                text.overwrite
            } else {
                text.save
            };
            let button = Button::new("keybinding-edit-save", label)
                .variant(if has_conflict {
                    ButtonVariant::Destructive
                } else {
                    ButtonVariant::Primary
                })
                .size(ButtonSize::Sm)
                .theme(theme.to_button_theme())
                .build()
                .text_size(d.text_sm)
                .px(d.pad_x)
                .py(d.pad_y_half)
                .on_click(move |_: &ClickEvent, _window, cx| {
                    save_state.update(cx, |state, cx| {
                        set_custom_keybinding(
                            &mut state.app.settings.keybindings.overrides,
                            save_action.clone(),
                            key_spec.clone(),
                        );
                        state.app.settings.keybindings.editing_action = None;
                        state.app.settings.keybindings.capturing = false;
                        state.app.settings.keybindings.pending_key_spec = None;
                        state.app.settings.keybindings.conflict = None;
                        apply_keymap_and_persist(state, cx);
                        cx.notify();
                    });
                });
            #[cfg(feature = "dev-api")]
            let button = button.dev_track(if has_conflict {
                "settings.keybinding.overwrite"
            } else {
                "settings.keybinding.save"
            });
            button.into_any_element()
        });

        div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .p(d.pad_x)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.accent)
            .bg(theme.surface)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child(action_label),
            )
            .child(capture)
            .children(conflict)
            .child(div().flex().gap(d.gap_md).child(cancel).children(save))
    });

    div()
        .flex()
        .flex_col()
        .gap(d.gap_md)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .when(compact, |element| element.flex_col().items_start())
                .gap(d.gap_md)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .min_w_0()
                        .gap(d.gap)
                        .child(
                            div()
                                .text_size(d.text_sm)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child(text.customize),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .text_size(d.text_xs)
                                .text_color(theme.text_muted)
                                .child(text.customize_description),
                        ),
                )
                .children(reset_all_button),
        )
        .children(confirmation)
        .children(editor)
        .child({
            let list = div()
                .id("custom-keybindings-list")
                .flex()
                .flex_col()
                .max_h(if compact { rems(8.0) } else { rems(12.0) })
                .overflow_y_scroll()
                .rounded(d.r_md)
                .border_1()
                .border_color(theme.border)
                .bg(theme.surface)
                .children(rows);
            #[cfg(feature = "dev-api")]
            let list = list.dev_track("settings.keybinding.list");
            list
        })
}

fn render_table_header(
    d: &Ds,
    text: KeybindingTranslations,
    theme: &crate::app::Theme,
) -> impl IntoElement {
    div()
        .flex()
        .w_full()
        .bg(theme.surface_hover)
        .border_b_1()
        .border_color(theme.border)
        .px(d.pad_x)
        .py(d.pad_y)
        .child(
            div()
                .flex_1()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(text.action),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child(text.default_preset),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Vim"),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Emacs"),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("VSCode"),
        )
}

fn render_category_section(
    d: &Ds,
    category: KeybindingCategory,
    rows: &[(String, HashMap<KeymapPreset, String>)],
    theme: &crate::app::Theme,
    current_preset: KeymapPreset,
    text: KeybindingTranslations,
) -> impl IntoElement {
    let accent = theme.accent;
    let border = theme.border;
    let background = theme.background;
    let text_secondary = theme.text_secondary;
    let text_muted = theme.text_muted;
    let surface_hover = theme.surface_hover;

    let row_elements: Vec<_> = rows
        .iter()
        .map(|(action, keys)| {
            render_comparison_row(
                d,
                action.clone(),
                keys.clone(),
                current_preset,
                accent,
                text_secondary,
                text_muted,
                border,
                surface_hover,
            )
        })
        .collect();

    div()
        .flex()
        .flex_col()
        // Category header
        .child(
            div()
                .w_full()
                .bg(background)
                .border_b_1()
                .border_color(border)
                .px(d.pad_x)
                .py(d.pad_y_half)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child(text.category_name(category)),
                ),
        )
        // Rows
        .children(row_elements)
}

fn render_comparison_row(
    d: &Ds,
    action: String,
    keys: HashMap<KeymapPreset, String>,
    current_preset: KeymapPreset,
    accent: gpui::Rgba,
    text_secondary: gpui::Rgba,
    text_muted: gpui::Rgba,
    border: gpui::Rgba,
    surface_hover: gpui::Rgba,
) -> impl IntoElement {
    let presets = [
        KeymapPreset::Default,
        KeymapPreset::Vim,
        KeymapPreset::Emacs,
        KeymapPreset::VSCode,
    ];

    let key_cells: Vec<_> = presets
        .iter()
        .map(|preset| {
            let key = keys.get(preset).map(|s| s.as_str()).unwrap_or("-");
            let is_current = *preset == current_preset;
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .text_align(gpui::TextAlign::Center)
                .text_color(if is_current { accent } else { text_muted })
                .font_weight(if is_current {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .child(key.to_string())
        })
        .collect();

    div()
        .flex()
        .w_full()
        .border_b_1()
        .border_color(border)
        .px(d.pad_x)
        .py(d.pad_y_half)
        .hover(move |s| s.bg(surface_hover))
        .child(
            div()
                .flex_1()
                .text_size(d.text_xs)
                .text_color(text_secondary)
                .child(action),
        )
        .children(key_cells)
}

/// Build a comparison of keybindings across all presets
/// Returns: Vec<(action_description, category, HashMap<preset, key>)>
fn build_keybinding_comparison(
    text: KeybindingTranslations,
) -> Vec<(String, KeybindingCategory, HashMap<KeymapPreset, String>)> {
    // Collect all unique actions across all presets
    let mut action_map: HashMap<String, (KeybindingCategory, HashMap<KeymapPreset, String>)> =
        HashMap::new();

    for preset in KeymapPreset::all() {
        let bindings = get_documented_keybindings(*preset);
        for binding in bindings {
            let entry = action_map
                .entry(text.action_description(binding.description).to_string())
                .or_insert_with(|| (binding.category, HashMap::new()));
            entry.1.insert(*preset, binding.key.to_string());
        }
    }

    // Convert to vec and sort by category then action name
    let mut result: Vec<_> = action_map
        .into_iter()
        .map(|(action, (cat, keys))| (action, cat, keys))
        .collect();

    result.sort_by(|a, b| {
        let cat_order = |c: &KeybindingCategory| match c {
            KeybindingCategory::Playback => 0,
            KeybindingCategory::Navigation => 1,
            KeybindingCategory::ScreenSwitch => 2,
            KeybindingCategory::Library => 3,
            KeybindingCategory::Queue => 4,
            KeybindingCategory::Plugins => 5,
            KeybindingCategory::ListeningTests => 6,
            KeybindingCategory::LevelMeters => 7,
            KeybindingCategory::System => 8,
        };
        cat_order(&a.1).cmp(&cat_order(&b.1)).then(a.0.cmp(&b.0))
    });

    result
}
