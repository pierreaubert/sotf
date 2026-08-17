//! UI state management.
//!
//! Contains all state related to the user interface including theme,
//! layout, panels, and modal states.

use crate::app::constants;
use crate::app::i18n::{Language, Translations};
use crate::app::keybindings::KeymapPreset;
use crate::app::theme::{CommunityThemeId, Theme, ThemeAccentPreference, ThemeId};
use crate::app::types::{ContextMenuState, DensityMode, ToastMessage};
use crate::app::{ActiveMenu, InputMode, LayoutMode, Screen, SettingsTab};
use gpui::EventEmitter;
use gpui_themes::{AccessibilityPalette, ThemeModePreference};
use sotf_audio_player::ReleaseChannel;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct LayoutState {
    pub queue_panel_ratio: f32,
    pub queue_list_ratio: f32,
    pub meters_panel_ratio: f32,
    pub lufs_panel_ratio: f32,
    pub rack_detail_ratio: f32,
    pub lufs_visible: bool,
    pub library_h_ratio: f32,
    pub queue_h_ratio: f32,
    pub rack_h_ratio: f32,
    pub library_v_ratio: f32,
    pub queue_v_ratio: f32,
    pub rack_v_ratio: f32,
    pub library_panel_collapsed: bool,
    pub queue_panel_collapsed: bool,
    pub rack_panel_collapsed: bool,
    pub is_dragging_queue_divider: bool,
    pub is_dragging_queue_list_divider: bool,
    pub is_dragging_meters_divider: bool,
    pub is_dragging_lufs_divider: bool,
    pub is_dragging_library_queue_divider: bool,
    pub is_dragging_queue_rack_divider: bool,
    /// Drag-start anchors. Recorded in `on_drag_start` for each divider so
    /// `on_mouse_move` can compute deltas from a stable reference instead of
    /// re-deriving the divider's "current" position from raw ratios — that
    /// derivation disagreed with the solved layout (which clamps + adjusts
    /// ratios) and produced a ~100px deadzone before the drag took effect.
    /// One pair per divider. Only meaningful while the matching
    /// `is_dragging_*` flag is set.
    pub drag_anchor_pos: f32,
    pub drag_anchor_meters_ratio: f32,
    pub drag_anchor_lufs_ratio: f32,
    pub drag_anchor_queue_list_ratio: f32,
    pub drag_anchor_library_h_ratio: f32,
    pub drag_anchor_rack_h_ratio: f32,
    pub drag_anchor_library_v_ratio: f32,
    pub drag_anchor_rack_v_ratio: f32,
}

impl EventEmitter<()> for LayoutState {}

pub const LUFS_PANEL_MIN_RATIO: f32 = 0.15;
pub const LUFS_PANEL_MAX_RATIO: f32 = 0.75;

pub fn lufs_panel_ratio_from_drag(
    anchor_ratio: f32,
    drag_delta_px: f32,
    available_height_px: f32,
) -> f32 {
    if available_height_px <= 0.0 {
        return anchor_ratio.clamp(LUFS_PANEL_MIN_RATIO, LUFS_PANEL_MAX_RATIO);
    }

    (anchor_ratio + drag_delta_px / available_height_px)
        .clamp(LUFS_PANEL_MIN_RATIO, LUFS_PANEL_MAX_RATIO)
}

impl LayoutState {
    pub fn begin_lufs_panel_drag(&mut self, pos: f32) {
        self.is_dragging_lufs_divider = true;
        self.drag_anchor_pos = pos;
        self.drag_anchor_lufs_ratio = self
            .lufs_panel_ratio
            .clamp(LUFS_PANEL_MIN_RATIO, LUFS_PANEL_MAX_RATIO);
    }

    pub fn update_lufs_panel_drag(&mut self, pos: f32, available_height_px: f32) {
        let dy = pos - self.drag_anchor_pos;
        self.lufs_panel_ratio =
            lufs_panel_ratio_from_drag(self.drag_anchor_lufs_ratio, dy, available_height_px);
    }

    pub fn end_lufs_panel_drag(&mut self) -> bool {
        let was_dragging = self.is_dragging_lufs_divider;
        self.is_dragging_lufs_divider = false;
        was_dragging
    }
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            queue_panel_ratio: constants::ui::QUEUE_PANEL_DEFAULT_RATIO,
            queue_list_ratio: constants::ui::QUEUE_LIST_DEFAULT_RATIO,
            meters_panel_ratio: constants::ui::METERS_PANEL_DEFAULT_RATIO,
            lufs_panel_ratio: constants::ui::LUFS_PANEL_DEFAULT_RATIO,
            rack_detail_ratio: 0.22,
            lufs_visible: true,
            library_h_ratio: 0.30,
            queue_h_ratio: 0.40,
            rack_h_ratio: 0.30,
            library_v_ratio: 0.40,
            queue_v_ratio: 0.35,
            rack_v_ratio: 0.25,
            library_panel_collapsed: false,
            queue_panel_collapsed: false,
            rack_panel_collapsed: true,
            is_dragging_queue_divider: false,
            is_dragging_queue_list_divider: false,
            is_dragging_meters_divider: false,
            is_dragging_lufs_divider: false,
            is_dragging_library_queue_divider: false,
            is_dragging_queue_rack_divider: false,
            drag_anchor_pos: 0.0,
            drag_anchor_meters_ratio: 0.0,
            drag_anchor_lufs_ratio: 0.0,
            drag_anchor_queue_list_ratio: 0.0,
            drag_anchor_library_h_ratio: 0.0,
            drag_anchor_rack_h_ratio: 0.0,
            drag_anchor_library_v_ratio: 0.0,
            drag_anchor_rack_v_ratio: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UIState {
    pub current_screen: Screen,
    pub last_screen: Screen,
    pub input_mode: InputMode,
    pub active_menu: ActiveMenu,
    pub density_mode: DensityMode,
    pub layout_mode: LayoutMode,
    pub window_height: f32,
    pub window_width: f32,
    pub theme_id: ThemeId,
    pub theme: Theme,
    pub theme_mode_preference: ThemeModePreference,
    pub accessibility_palette: AccessibilityPalette,
    pub theme_accent_preference: ThemeAccentPreference,
    pub community_theme_id: Option<CommunityThemeId>,
    pub community_theme_json_draft: String,
    pub reduce_motion: bool,
    pub language: Language,
    pub translations: Translations,
    pub keymap_preset: KeymapPreset,
    pub toast_message: Option<ToastMessage>,
    pub context_menu: Option<ContextMenuState>,
    pub active_settings_tab: SettingsTab,
    pub filter_menu_open: bool,
    pub show_device_popup: bool,
    pub show_studio_menu: bool,
    pub show_sotf_api_connection_qr: bool,
    pub primary_nav_collapsed: bool,
    pub footer_collapsed: bool,
    pub expanded_home_sections: BTreeSet<String>,
    pub phone_home_shelf: crate::app::PhoneHomeShelf,
    pub phone_queue_editing: bool,
    pub phone_shuffle_enabled: bool,
    pub phone_repeat_enabled: bool,
    pub phone_plugin_rack_editing: bool,
    pub phone_spectrum_hold: bool,
    pub phone_spectrum_hold_magnitudes: Option<Vec<f32>>,
    pub phone_spectrum_smoothed: bool,
    pub phone_plugin_graph_list: bool,
    pub phone_plugin_graph_actions_open: bool,
    pub phone_stream_sources_open: bool,
    pub phone_keybindings_query: String,
    pub phone_tab_bar_hidden: bool,
    pub queue_expanded_album: Option<usize>,
    pub queue_expansion_overridden: bool,
    pub pending_studio_close: bool,
    pub should_quit: bool,
    pub startup_db_check_done: bool,
    /// Font scale factor (1.0 = normal, >1.0 = larger, <1.0 = smaller)
    pub font_scale: f32,
    /// Feature release channel controlling visibility of beta/alpha features
    pub release_channel: ReleaseChannel,
    /// Number of scanner threads (None = auto-detect, capped at 4)
    pub scanner_threads: Option<u8>,
    /// Maximum number of CPU cores SotF is allowed to use (None = all available)
    pub max_cpu_cores: Option<u8>,
    /// Minimum font size in pixels (None = default 12px)
    pub min_font_size_px: Option<f32>,
    /// Maximum font size in pixels (None = default 32px)
    pub max_font_size_px: Option<f32>,
    /// Selected design system language (None = platform default)
    pub design_language: Option<String>,
    /// Current tutorial screen index (0-6)
    pub tutorial_screen: usize,
    /// Whether "don't show again" checkbox is checked in tutorial dialog
    pub tutorial_dont_show: bool,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            current_screen: Screen::Home,
            last_screen: Screen::Library,
            input_mode: InputMode::Normal,
            active_menu: ActiveMenu::None,
            density_mode: DensityMode::Standard,
            layout_mode: LayoutMode::Compact,
            window_height: constants::ui::DEFAULT_WINDOW_HEIGHT,
            window_width: constants::ui::DEFAULT_WINDOW_WIDTH,
            theme_id: ThemeId::default(),
            theme: Theme::from_id(ThemeId::default()),
            theme_mode_preference: ThemeModePreference::default(),
            accessibility_palette: AccessibilityPalette::default(),
            theme_accent_preference: ThemeAccentPreference::default(),
            community_theme_id: None,
            community_theme_json_draft: String::new(),
            reduce_motion: false,
            language: Language::default(),
            translations: Translations::for_language(Language::default()),
            keymap_preset: KeymapPreset::default(),
            toast_message: None,
            context_menu: None,
            active_settings_tab: SettingsTab::Library,
            filter_menu_open: false,
            show_device_popup: false,
            show_studio_menu: false,
            show_sotf_api_connection_qr: false,
            primary_nav_collapsed: false,
            footer_collapsed: false,
            expanded_home_sections: BTreeSet::new(),
            phone_home_shelf: crate::app::PhoneHomeShelf::default(),
            phone_queue_editing: false,
            phone_shuffle_enabled: false,
            phone_repeat_enabled: false,
            phone_plugin_rack_editing: false,
            phone_spectrum_hold: false,
            phone_spectrum_hold_magnitudes: None,
            phone_spectrum_smoothed: false,
            phone_plugin_graph_list: false,
            phone_plugin_graph_actions_open: false,
            phone_stream_sources_open: false,
            phone_keybindings_query: String::new(),
            phone_tab_bar_hidden: false,
            queue_expanded_album: None,
            queue_expansion_overridden: false,
            pending_studio_close: false,
            should_quit: false,
            startup_db_check_done: false,
            font_scale: 1.0,
            release_channel: ReleaseChannel::default(),
            scanner_threads: None,
            max_cpu_cores: None,
            min_font_size_px: None,
            max_font_size_px: None,
            design_language: None,
            tutorial_screen: 0,
            tutorial_dont_show: false,
        }
    }
}

impl UIState {
    pub fn new() -> Self {
        Self::default()
    }
}
