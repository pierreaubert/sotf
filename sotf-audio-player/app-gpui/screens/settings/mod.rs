//! Settings screen rendering functions

mod appearance;
mod audio_device;
pub mod directory;
mod headphone;
mod library;
mod room_eq;

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Accordion, AccordionItem, AccordionMode, AccordionTheme};

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let expanded = state.app.expanded_settings_sections.clone();

        // Convert expanded sections to SharedString for the Accordion
        let expanded_ids: Vec<SharedString> = expanded
            .iter()
            .map(|s| SharedString::from(s.clone()))
            .collect();

        // Pre-render all content sections (convert to AnyElement to release borrow)
        let library_content = self.render_library_settings_content(cx).into_any_element();
        let appearance_content = self
            .render_appearance_settings_content(cx)
            .into_any_element();
        let audio_device_content = self
            .render_audio_device_settings_content(cx)
            .into_any_element();
        let plugins_content = self.render_plugins_screen(cx).into_any_element();
        let room_eq_content = self.render_roomeq_settings_content(cx).into_any_element();
        let headphone_content = self
            .render_headphone_settings_content(cx)
            .into_any_element();

        // Create accordion theme from app theme
        let accordion_theme = AccordionTheme {
            header_bg: theme.surface,
            header_hover_bg: theme.surface_hover,
            content_bg: theme.background,
            border: theme.border,
            title_color: theme.text_primary,
            indicator_color: theme.text_muted,
        };

        // Build accordion items
        let items = vec![
            AccordionItem::new("library", "Library").content(library_content),
            AccordionItem::new("appearance", "Appearance").content(appearance_content),
            AccordionItem::new("audio-device", "Audio Device").content(audio_device_content),
            AccordionItem::new("plugins", "Plugins").content(plugins_content),
            AccordionItem::new("room-eq", "Room EQ").content(room_eq_content),
            AccordionItem::new("headphone", "Headphone").content(headphone_content),
        ];

        // Get weak reference for the closure
        let state_handle = self.state.downgrade();

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .overflow_y_scroll()
            .child(
                Accordion::new()
                    .items(items)
                    .expanded(expanded_ids)
                    .mode(AccordionMode::Multiple)
                    .theme(accordion_theme)
                    .on_change(move |id, new_state, _window, cx| {
                        if let Some(state) = state_handle.upgrade() {
                            let entity_id = state.entity_id();
                            state.update(cx, |state, _cx| {
                                let id_str = id.to_string();
                                if new_state {
                                    if !state.app.expanded_settings_sections.contains(&id_str) {
                                        state.app.expanded_settings_sections.push(id_str);
                                    }
                                } else {
                                    state
                                        .app
                                        .expanded_settings_sections
                                        .retain(|s| s != &id_str);
                                }
                            });
                            cx.notify(entity_id);
                        }
                    })
                    .build(),
            )
    }
}
