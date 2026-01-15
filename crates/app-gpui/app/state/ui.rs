//! UI state management.
//!
//! Contains all state related to the user interface including theme,
//! layout, panels, and modal states.

use crate::app::constants;
use crate::app::i18n::{Language, Translations};
use crate::app::keybindings::KeymapPreset;
use crate::app::theme::{Theme, ThemeId};
use crate::app::types::{ContextMenuState, ToastMessage};
use crate::app::{ActiveMenu, InputMode, LayoutMode, Screen, SettingsTab};

#[derive(Debug, Clone)]
pub struct UIState {
    pub current_screen: Screen,
    pub last_screen: Screen,
    pub input_mode: InputMode,
    pub active_menu: ActiveMenu,
    pub layout_mode: LayoutMode,
    pub window_height: f32,
    pub window_width: f32,
    pub theme_id: ThemeId,
    pub theme: Theme,
    pub language: Language,
    pub translations: Translations,
    pub keymap_preset: KeymapPreset,
    pub toast_message: Option<ToastMessage>,
    pub context_menu: Option<ContextMenuState>,
    pub active_settings_tab: SettingsTab,
    pub filter_menu_open: bool,
    pub show_device_popup: bool,
    pub show_studio_menu: bool,
    pub pending_studio_close: bool,
    pub should_quit: bool,
    pub startup_db_check_done: bool,
    /// Font scale factor (1.0 = normal, >1.0 = larger, <1.0 = smaller)
    pub font_scale: f32,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            current_screen: Screen::Library,
            last_screen: Screen::Library,
            input_mode: InputMode::Normal,
            active_menu: ActiveMenu::None,
            layout_mode: LayoutMode::Compact,
            window_height: constants::ui::DEFAULT_WINDOW_HEIGHT,
            window_width: constants::ui::DEFAULT_WINDOW_WIDTH,
            theme_id: ThemeId::default(),
            theme: Theme::from_id(ThemeId::default()),
            language: Language::default(),
            translations: Translations::for_language(Language::default()),
            keymap_preset: KeymapPreset::default(),
            toast_message: None,
            context_menu: None,
            active_settings_tab: SettingsTab::Library,
            filter_menu_open: false,
            show_device_popup: false,
            show_studio_menu: false,
            pending_studio_close: false,
            should_quit: false,
            startup_db_check_done: false,
            font_scale: 1.0,
        }
    }
}

impl UIState {
    pub fn new() -> Self {
        Self::default()
    }
}
