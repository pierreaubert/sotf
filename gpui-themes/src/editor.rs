//! Theme editor main component
//!
//! Provides the main theme editor UI with:
//! - Color group navigation
//! - Color editing with live preview
//! - Export to JSON and Rust

use crate::color_picker::ColorPickerView;
use crate::showcase::ComponentShowcase;
use crate::theme::{Color, ColorGroup, EditorTheme};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackSpacing, Text, TextSize, TextWeight, VStack,
};

/// Transparent color constant
const TRANSPARENT: Rgba = Rgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

/// Currently selected color field
#[derive(Debug, Clone)]
pub struct ColorField {
    pub group: ColorGroup,
    pub name: &'static str,
    pub getter: fn(&EditorTheme) -> Color,
    pub setter: fn(&mut EditorTheme, Color),
}

impl ColorField {
    pub const fn new(
        group: ColorGroup,
        name: &'static str,
        getter: fn(&EditorTheme) -> Color,
        setter: fn(&mut EditorTheme, Color),
    ) -> Self {
        Self {
            group,
            name,
            getter,
            setter,
        }
    }
}

/// All editable color fields
pub fn all_color_fields() -> Vec<ColorField> {
    vec![
        // Base colors
        ColorField::new(
            ColorGroup::Base,
            "Background",
            |t| t.background,
            |t, c| t.background = c,
        ),
        ColorField::new(
            ColorGroup::Base,
            "Background Secondary",
            |t| t.background_secondary,
            |t, c| t.background_secondary = c,
        ),
        ColorField::new(
            ColorGroup::Base,
            "Background Tertiary",
            |t| t.background_tertiary,
            |t, c| t.background_tertiary = c,
        ),
        ColorField::new(
            ColorGroup::Base,
            "Surface",
            |t| t.surface,
            |t, c| t.surface = c,
        ),
        ColorField::new(
            ColorGroup::Base,
            "Surface Hover",
            |t| t.surface_hover,
            |t, c| t.surface_hover = c,
        ),
        ColorField::new(
            ColorGroup::Base,
            "Surface Selected",
            |t| t.surface_selected,
            |t, c| t.surface_selected = c,
        ),
        // Text colors
        ColorField::new(
            ColorGroup::Text,
            "Text Primary",
            |t| t.text_primary,
            |t, c| t.text_primary = c,
        ),
        ColorField::new(
            ColorGroup::Text,
            "Text Secondary",
            |t| t.text_secondary,
            |t, c| t.text_secondary = c,
        ),
        ColorField::new(
            ColorGroup::Text,
            "Text Muted",
            |t| t.text_muted,
            |t, c| t.text_muted = c,
        ),
        ColorField::new(
            ColorGroup::Text,
            "Text Disabled",
            |t| t.text_disabled,
            |t, c| t.text_disabled = c,
        ),
        // Border colors
        ColorField::new(
            ColorGroup::Border,
            "Border",
            |t| t.border,
            |t, c| t.border = c,
        ),
        ColorField::new(
            ColorGroup::Border,
            "Border Focused",
            |t| t.border_focused,
            |t, c| t.border_focused = c,
        ),
        // Accent colors
        ColorField::new(
            ColorGroup::Accent,
            "Accent",
            |t| t.accent,
            |t, c| t.accent = c,
        ),
        ColorField::new(
            ColorGroup::Accent,
            "Accent Hover",
            |t| t.accent_hover,
            |t, c| t.accent_hover = c,
        ),
        ColorField::new(
            ColorGroup::Accent,
            "Accent Muted",
            |t| t.accent_muted,
            |t, c| t.accent_muted = c,
        ),
        // Semantic colors
        ColorField::new(
            ColorGroup::Semantic,
            "Success",
            |t| t.success,
            |t, c| t.success = c,
        ),
        ColorField::new(
            ColorGroup::Semantic,
            "Warning",
            |t| t.warning,
            |t, c| t.warning = c,
        ),
        ColorField::new(
            ColorGroup::Semantic,
            "Error",
            |t| t.error,
            |t, c| t.error = c,
        ),
        ColorField::new(ColorGroup::Semantic, "Info", |t| t.info, |t, c| t.info = c),
    ]
}

/// Main view tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorTab {
    #[default]
    Colors,
    Preview,
    Export,
}

/// Theme editor state
pub struct ThemeEditor {
    /// Current theme being edited
    pub theme: EditorTheme,
    /// Currently selected color group
    pub selected_group: ColorGroup,
    /// Currently selected color field index within group
    pub selected_field_index: usize,
    /// Current tab
    pub current_tab: EditorTab,
    /// All color fields
    pub color_fields: Vec<ColorField>,
    /// Expanded accordion sections
    pub expanded_sections: Vec<SharedString>,
    /// Color picker model
    pub color_picker: Option<Entity<ColorPickerView>>,
    /// Component showcase model
    pub showcase: Entity<ComponentShowcase>,
    /// Export format (json or rust)
    pub export_format: String,
}

impl ThemeEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let theme = EditorTheme::dark();
        let showcase = cx.new(|_| ComponentShowcase::new(theme.clone()));

        Self {
            theme,
            selected_group: ColorGroup::Base,
            selected_field_index: 0,
            current_tab: EditorTab::Colors,
            color_fields: all_color_fields(),
            expanded_sections: vec![SharedString::from("Base Colors")],
            color_picker: None,
            showcase,
            export_format: "json".to_string(),
        }
    }

    /// Get fields for a specific group
    fn fields_for_group(&self, group: ColorGroup) -> Vec<&ColorField> {
        self.color_fields.iter().filter(|f| f.group == group).collect()
    }

    /// Get current selected field
    fn current_field(&self) -> Option<&ColorField> {
        let fields = self.fields_for_group(self.selected_group);
        fields.get(self.selected_field_index).copied()
    }

    /// Update a color and sync to showcase
    #[allow(dead_code)]
    fn update_color(&mut self, field: &ColorField, color: Color, cx: &mut Context<Self>) {
        (field.setter)(&mut self.theme, color);
        // Update showcase
        self.showcase.update(cx, |showcase, _| {
            showcase.set_theme(self.theme.clone());
        });
        cx.notify();
    }

    /// Load a preset theme
    fn load_preset(&mut self, preset: &str, cx: &mut Context<Self>) {
        self.theme = match preset {
            "dark" => EditorTheme::dark(),
            "light" => EditorTheme::light(),
            _ => EditorTheme::dark(),
        };
        self.showcase.update(cx, |showcase, _| {
            showcase.set_theme(self.theme.clone());
        });
        cx.notify();
    }

    /// Render the sidebar with color groups
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let selected_group = self.selected_group;

        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                div()
                    .p_3()
                    .border_b_1()
                    .border_color(theme.border.to_rgba())
                    .child(
                        Text::new("Color Groups")
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_secondary.to_rgba()),
                    ),
            )
            .children(ColorGroup::all().iter().map(|group| {
                let is_selected = *group == selected_group;
                let bg = if is_selected {
                    theme.surface_selected.to_rgba()
                } else {
                    TRANSPARENT
                };
                let text_color = if is_selected {
                    theme.text_primary.to_rgba()
                } else {
                    theme.text_secondary.to_rgba()
                };

                div()
                    .id(SharedString::from(format!("group-{:?}", group)))
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .bg(bg)
                    .hover(|s| s.bg(theme.surface_hover.to_rgba()))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener({
                            let group = *group;
                            move |this, _: &MouseUpEvent, _window, cx| {
                                this.selected_group = group;
                                this.selected_field_index = 0;
                                cx.notify();
                            }
                        }),
                    )
                    .child(Text::new(group.label()).size(TextSize::Sm).color(text_color))
            }))
            .build()
    }

    /// Render color list for current group
    fn render_color_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let fields = self.fields_for_group(self.selected_group);
        let selected_index = self.selected_field_index;

        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                div()
                    .p_3()
                    .border_b_1()
                    .border_color(theme.border.to_rgba())
                    .child(
                        Text::new(self.selected_group.label())
                            .size(TextSize::Md)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary.to_rgba()),
                    ),
            )
            .children(fields.iter().enumerate().map(|(idx, field)| {
                let color = (field.getter)(&self.theme);
                let is_selected = idx == selected_index;
                let bg = if is_selected {
                    theme.surface_selected.to_rgba()
                } else {
                    TRANSPARENT
                };

                div()
                    .id(SharedString::from(format!("field-{}", field.name)))
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .bg(bg)
                    .hover(|s| s.bg(theme.surface_hover.to_rgba()))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener({
                            move |this, _: &MouseUpEvent, _window, cx| {
                                this.selected_field_index = idx;
                                cx.notify();
                            }
                        }),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                div()
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded(px(4.0))
                                    .bg(color.to_rgba())
                                    .border_1()
                                    .border_color(theme.border.to_rgba()),
                            )
                            .child(Text::new(field.name).size(TextSize::Sm).color(theme.text_primary.to_rgba()))
                            .child(div().flex_1())
                            .child(
                                Text::new(SharedString::from(color.to_hex_string()))
                                    .size(TextSize::Xs)
                                    .color(theme.text_muted.to_rgba()),
                            )
                            .build(),
                    )
            }))
            .build()
    }

    /// Render color editor panel
    fn render_color_editor(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;

        if let Some(field) = self.current_field() {
            let color = (field.getter)(&self.theme);
            let field_name = field.name;

            div()
                .p_4()
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            Text::new(SharedString::from(format!("Edit: {}", field_name)))
                                .size(TextSize::Md)
                                .weight(TextWeight::Bold)
                                .color(theme.text_primary.to_rgba()),
                        )
                        // Large color preview
                        .child(
                            div()
                                .w_full()
                                .h(px(80.0))
                                .rounded_lg()
                                .bg(color.to_rgba())
                                .border_1()
                                .border_color(theme.border.to_rgba()),
                        )
                        // Hex display
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(Text::new("Hex:").size(TextSize::Sm).color(theme.text_secondary.to_rgba()))
                                .child(
                                    Text::new(SharedString::from(color.to_hex_string()))
                                        .size(TextSize::Md)
                                        .weight(TextWeight::Medium)
                                        .color(theme.text_primary.to_rgba()),
                                )
                                .build(),
                        )
                        // RGBA display
                        .child(
                            Text::new(SharedString::from(format!(
                                "RGBA: {}, {}, {}, {}",
                                color.r, color.g, color.b, color.a
                            )))
                            .size(TextSize::Sm)
                            .color(theme.text_muted.to_rgba()),
                        )
                        // HSL display
                        .child({
                            let (h, s, l) = color.to_hsl();
                            Text::new(SharedString::from(format!(
                                "HSL: {:.0}°, {:.0}%, {:.0}%",
                                h * 360.0,
                                s * 100.0,
                                l * 100.0
                            )))
                            .size(TextSize::Sm)
                            .color(theme.text_muted.to_rgba())
                        })
                        .build(),
                )
        } else {
            div().p_4().child(
                Text::new("Select a color to edit")
                    .size(TextSize::Md)
                    .color(theme.text_muted.to_rgba()),
            )
        }
    }

    /// Render the colors tab
    fn render_colors_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;

        div()
            .flex()
            .flex_row()
            .size_full()
            // Sidebar
            .child(
                div()
                    .w(px(180.0))
                    .h_full()
                    .bg(theme.background_secondary.to_rgba())
                    .border_r_1()
                    .border_color(theme.border.to_rgba())
                    .child(self.render_sidebar(cx)),
            )
            // Color list
            .child(
                div()
                    .w(px(280.0))
                    .h_full()
                    .bg(theme.background.to_rgba())
                    .border_r_1()
                    .border_color(theme.border.to_rgba())
                    .child(self.render_color_list(cx)),
            )
            // Color editor
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .bg(theme.background_secondary.to_rgba())
                    .child(self.render_color_editor(cx)),
            )
    }

    /// Render the preview tab
    fn render_preview_tab(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.showcase.clone())
    }

    /// Render the export tab
    fn render_export_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let export_format = self.export_format.clone();

        let export_content = if export_format == "json" {
            self.theme.to_json().unwrap_or_else(|e| format!("Error: {}", e))
        } else {
            self.theme.to_rust_code()
        };

        div()
            .p_6()
            .size_full()
            .child(
                VStack::new()
                    .spacing(StackSpacing::Lg)
                    // Theme name display
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Theme Name:")
                                    .size(TextSize::Md)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary.to_rgba()),
                            )
                            .child(
                                Text::new(SharedString::from(self.theme.name.clone()))
                                    .size(TextSize::Md)
                                    .color(theme.text_primary.to_rgba()),
                            )
                            .build(),
                    )
                    // Format selection
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(Text::new("Export Format:").size(TextSize::Md).color(theme.text_primary.to_rgba()))
                            .child(
                                Button::new("format-json", "JSON")
                                    .variant(if export_format == "json" {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    })
                                    .size(ButtonSize::Sm)
                                    .build()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                        this.export_format = "json".to_string();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("format-rust", "Rust")
                                    .variant(if export_format == "rust" {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    })
                                    .size(ButtonSize::Sm)
                                    .build()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                        this.export_format = "rust".to_string();
                                        cx.notify();
                                    })),
                            )
                            .build(),
                    )
                    // Export preview
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .p_4()
                            .bg(theme.background_tertiary.to_rgba())
                            .rounded_lg()
                            .border_1()
                            .border_color(theme.border.to_rgba())
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.text_primary.to_rgba())
                                    .child(export_content),
                            ),
                    )
                    // Action buttons
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Button::new("copy-btn", "Copy to Clipboard")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Md)
                                    .build(),
                            )
                            .child(
                                Button::new("save-btn", "Save to File")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Md)
                                    .build(),
                            )
                            .build(),
                    )
                    .build(),
            )
    }

    /// Render the header with presets and tabs
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let current_tab = self.current_tab;

        VStack::new()
            .spacing(StackSpacing::None)
            // Top bar with presets
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(theme.background_secondary.to_rgba())
                    .border_b_1()
                    .border_color(theme.border.to_rgba())
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Theme Editor")
                                    .size(TextSize::Lg)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary.to_rgba()),
                            )
                            .child(div().flex_1())
                            .child(Text::new("Load Preset:").size(TextSize::Sm).color(theme.text_secondary.to_rgba()))
                            .child(
                                Button::new("preset-dark", "Dark")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .build()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                        this.load_preset("dark", cx);
                                    })),
                            )
                            .child(
                                Button::new("preset-light", "Light")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .build()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                        this.load_preset("light", cx);
                                    })),
                            )
                            .build(),
                    ),
            )
            // Tab bar
            .child(
                div()
                    .px_4()
                    .py_1()
                    .bg(theme.surface.to_rgba())
                    .border_b_1()
                    .border_color(theme.border.to_rgba())
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::None)
                            .child(self.render_tab_button("Colors", EditorTab::Colors, current_tab, cx))
                            .child(self.render_tab_button("Preview", EditorTab::Preview, current_tab, cx))
                            .child(self.render_tab_button("Export", EditorTab::Export, current_tab, cx))
                            .build(),
                    ),
            )
            .build()
    }

    fn render_tab_button(
        &self,
        label: &'static str,
        tab: EditorTab,
        current: EditorTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = &self.theme;
        let is_selected = tab == current;
        let bg = if is_selected {
            theme.surface_selected.to_rgba()
        } else {
            TRANSPARENT
        };
        let text_color = if is_selected {
            theme.text_primary.to_rgba()
        } else {
            theme.text_secondary.to_rgba()
        };

        div()
            .id(SharedString::from(format!("tab-{:?}", tab)))
            .cursor_pointer()
            .px_4()
            .py_2()
            .bg(bg)
            .hover(|s| s.bg(theme.surface_hover.to_rgba()))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseUpEvent, _window, cx| {
                    this.current_tab = tab;
                    cx.notify();
                }),
            )
            .child(Text::new(label).size(TextSize::Sm).color(text_color))
    }
}

impl Render for ThemeEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let current_tab = self.current_tab;

        div()
            .size_full()
            .bg(theme.background.to_rgba())
            .flex()
            .flex_col()
            // Header
            .child(self.render_header(cx))
            // Content based on tab
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(match current_tab {
                        EditorTab::Colors => self.render_colors_tab(cx).into_any_element(),
                        EditorTab::Preview => self.render_preview_tab(cx).into_any_element(),
                        EditorTab::Export => self.render_export_tab(cx).into_any_element(),
                    }),
            )
    }
}
