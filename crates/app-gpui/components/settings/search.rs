use crate::app::types::PreferencesSetting;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, Input, NumberInput, Toggle};

use std::collections::HashMap;

thread_local! {
    static SETTINGS_CHOICE_FOCUS_HANDLES: std::cell::RefCell<HashMap<ElementId, FocusHandle>> =
        std::cell::RefCell::new(HashMap::new());
}

pub(super) fn settings_choice_focus_handle(id: &ElementId, cx: &mut App) -> FocusHandle {
    SETTINGS_CHOICE_FOCUS_HANDLES.with(|handles| {
        handles
            .borrow_mut()
            .entry(id.clone())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    })
}

pub(super) fn focus_settings_choice_relative(
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

impl PlayerView {
    pub(crate) fn preference_text_input(
        &self,
        setting: PreferencesSetting,
        mut input: Input,
        cx: &Context<Self>,
    ) -> AnyElement {
        if let Some(focus) = self.preference_focus_handle(setting, cx) {
            input = input.focus_handle(focus);
        }
        self.preference_control(setting, input, cx)
    }

    /// A searchable action keeps pointer and keyboard activation on the same path.
    pub(crate) fn preference_action(
        &self,
        setting: PreferencesSetting,
        button: Button,
        disabled: bool,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let action = std::rc::Rc::new(action);
        let keyboard_action = action.clone();
        let control = button
            .disabled(disabled)
            .build()
            .text_size(d.text_sm)
            .px(d.pad_x)
            .py(d.pad_y_half)
            .when_some(
                self.preference_focus_handle(setting, cx),
                |button, focus| button.track_focus(&focus).track_focus_element(&focus),
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                if !disabled {
                    action(view, window, cx);
                }
            }))
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if !disabled {
                        keyboard_action(view, window, cx);
                    }
                    cx.stop_propagation();
                }
            }));
        self.preference_control(setting, control, cx)
    }

    /// Reuse the real setting control for search navigation and exact entry.
    pub(crate) fn preference_number_input(
        &self,
        setting: PreferencesSetting,
        mut input: NumberInput,
        cx: &Context<Self>,
    ) -> AnyElement {
        input = input.aria_label(setting.label(self.state.read(cx).app.ui_state.language));
        if let Some(focus) = self.preference_focus_handle(setting, cx) {
            input = input.focus_handle(focus);
        }
        self.preference_control(setting, input, cx)
    }

    /// Give a searchable select the same reveal/highlight anchor as other controls.
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(crate) fn preference_select(
        &self,
        setting: PreferencesSetting,
        select: gpui_ui_kit::Select,
        selector: &'static str,
        cx: &Context<Self>,
    ) -> AnyElement {
        #[cfg(feature = "dev-api")]
        let select = {
            use crate::app::dev_api::DevTrackExt;
            select.dev_track(selector)
        };
        #[cfg(not(feature = "dev-api"))]
        let _ = selector;
        let control = div()
            .id(SharedString::from(format!(
                "preferences-select-{setting:?}"
            )))
            .w(rems(14.0))
            .max_w_full()
            .child(select)
            .when_some(
                self.preference_focus_handle(setting, cx),
                |element, focus| {
                    element
                        .track_focus(&focus)
                        .track_focus_element(&focus)
                        .focusable()
                },
            );
        self.preference_control(setting, control, cx)
    }

    pub(crate) fn preference_focus_handle(
        &self,
        setting: PreferencesSetting,
        cx: &Context<Self>,
    ) -> Option<FocusHandle> {
        let navigation = &self.state.read(cx).app.settings.navigation;
        (navigation.setting == Some(setting))
            .then(|| navigation.setting_focus.clone())
            .flatten()
    }

    pub(crate) fn preference_toggle(
        &self,
        setting: PreferencesSetting,
        toggle: Toggle,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = self.state.read(cx);
        let focus = self.preference_focus_handle(setting, cx);
        let control = toggle
            .aria_label(setting.label(state.app.ui_state.language))
            .selected(focus.is_some())
            .build_with_theme(&state.app.ui_state.theme.to_toggle_theme())
            .when_some(focus, |control, focus| {
                control.track_focus(&focus).track_focus_element(&focus)
            });
        self.preference_control(setting, control, cx)
    }

    /// Highlight and reveal the mounted control while retaining its own input handlers.
    pub(crate) fn preference_control(
        &self,
        setting: PreferencesSetting,
        control: impl IntoElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = self.state.read(cx);
        let navigation = &state.app.settings.navigation;
        let selected = navigation.setting == Some(setting);
        let reveal = selected && navigation.reveal_setting;
        let accent = state.app.ui_state.theme.accent;
        let clearance = Ds::from_cx(cx).gap;
        let entity = self.state.downgrade();
        let scroll = self.scroll.preferences.clone();
        let element = div()
            .flex_shrink_0()
            .when(selected, |element| element.border_1().border_color(accent))
            .child(control)
            .on_children_prepainted(move |bounds, window, cx| {
                if !reveal {
                    return;
                }
                let Some(bounds) = bounds.first().copied() else {
                    return;
                };
                let clearance = clearance.to_pixels(window.rem_size());
                let entity = entity.clone();
                let scroll = scroll.clone();
                window.defer(cx, move |window, cx| {
                    let Some(entity) = entity.upgrade() else {
                        return;
                    };
                    let focus = entity.update(cx, |state, cx| {
                        if state.app.ui_state.current_screen != crate::app::Screen::Settings
                            || state.app.ui_state.active_settings_tab != setting.tab()
                        {
                            return None;
                        }
                        let navigation = &mut state.app.settings.navigation;
                        if navigation.setting != Some(setting) || !navigation.reveal_setting {
                            return None;
                        }
                        navigation.reveal_setting = false;
                        let focus = navigation.setting_focus.clone();
                        let viewport = scroll.bounds();
                        let offset = scroll.offset();
                        let delta = if bounds.top() - clearance < viewport.top() {
                            viewport.top() - bounds.top() + clearance
                        } else if bounds.bottom() + clearance > viewport.bottom() {
                            viewport.bottom() - bounds.bottom() - clearance
                        } else {
                            // intentional: Scroll coordinates use zero pixels as the origin.
                            px(0.0)
                        };
                        scroll.set_offset(point(offset.x, (offset.y + delta).min(px(0.0))));
                        cx.notify();
                        focus
                    });
                    if let Some(focus) = focus {
                        focus.focus(window, cx);
                    }
                });
            })
            .id(SharedString::from(format!(
                "preferences-anchor-{setting:?}"
            )));
        #[cfg(feature = "dev-api")]
        let element = {
            use crate::app::dev_api::DevTrackExt;
            element.dev_track(format!("preferences.control.{setting:?}"))
        };
        element.into_any_element()
    }
}
