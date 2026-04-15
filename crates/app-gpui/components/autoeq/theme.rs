//! AutoEQ form theme definition.

use gpui::*;
use gpui_ui_kit::number_input::NumberInputTheme;
use gpui_ui_kit::select::SelectTheme;
use gpui_ui_kit::theme::Theme;
use gpui_ui_kit::toggle::ToggleTheme;

/// Theme for the AutoEQ form
#[derive(Debug, Clone)]
pub struct AutoEqFormTheme {
    /// Card background
    pub card_bg: Rgba,
    /// Section header color
    pub header_color: Rgba,
    /// Label color
    pub label_color: Rgba,
    /// Description color
    pub description_color: Rgba,
    /// Accent color
    pub accent: Rgba,
    /// Toggle theme colors
    pub toggle_checked_bg: Rgba,
    pub toggle_unchecked_bg: Rgba,
    pub toggle_knob: Rgba,
    /// Border color
    pub border: Rgba,
    /// Text muted color
    pub text_muted: Rgba,
    /// NumberInput theme
    pub number_input_theme: NumberInputTheme,
    /// Select theme
    pub select_theme: SelectTheme,
}

impl Default for AutoEqFormTheme {
    fn default() -> Self {
        Self {
            card_bg: rgba(0x2a2a2aff),
            header_color: rgba(0xffffffff),
            label_color: rgba(0xccccccff),
            description_color: rgba(0x888888ff),
            accent: rgba(0x007accff),
            toggle_checked_bg: rgba(0x007accff),
            toggle_unchecked_bg: rgba(0x4a4a4aff),
            toggle_knob: rgba(0xffffffff),
            border: rgba(0x3a3a3aff),
            text_muted: rgba(0x888888ff),
            number_input_theme: NumberInputTheme::default(),
            select_theme: SelectTheme::default(),
        }
    }
}

impl AutoEqFormTheme {
    /// Build a `ToggleTheme` from this form theme.
    pub fn toggle_theme(&self) -> ToggleTheme {
        ToggleTheme {
            checked_bg: self.toggle_checked_bg,
            unchecked_bg: self.toggle_unchecked_bg,
            knob: self.toggle_knob,
            knob_on_checked: self.card_bg,
            track_border: self.border,
            label: self.label_color,
            accent: self.accent,
            accent_muted: self.accent,
            success: self.accent,
            border: self.border,
            text_on_accent: self.toggle_knob,
            text_muted: self.text_muted,
            text_primary: self.header_color,
            surface_hover: self.toggle_unchecked_bg,
            background: self.card_bg,
        }
    }
}

impl From<&Theme> for AutoEqFormTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            card_bg: theme.surface,
            header_color: theme.text_primary,
            label_color: theme.text_secondary,
            description_color: theme.text_muted,
            accent: theme.accent,
            toggle_checked_bg: theme.accent,
            toggle_unchecked_bg: theme.muted,
            toggle_knob: theme.text_primary,
            border: theme.border,
            text_muted: theme.text_muted,
            number_input_theme: NumberInputTheme::from(theme),
            select_theme: SelectTheme::from(theme),
        }
    }
}
