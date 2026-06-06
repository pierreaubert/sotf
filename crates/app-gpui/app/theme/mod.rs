//! Theme system for the GPUI audio player.
//!
//! Provides color definitions for different UI themes.

// Allow approximate math constants in color values
#![allow(clippy::approx_constant)]

use gpui::{App, Rgba, SharedString};
use gpui_design::DesignExt;
use gpui_themes::{
    AccentPalette, AccentSource, AccessibilityPalette, BuiltInThemePreset, Color as ThemeColor,
    CommunityThemeBundle, CommunityThemeManifest, EditorTheme, ThemeAppearance,
    ThemeModePreference,
};
use gpui_ui_kit::theme::{Theme as UiKitTheme, ThemeVariant as UiKitThemeVariant};
use serde::{Deserialize, Serialize};

/// Available theme identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeId {
    #[default]
    Dark,
    Light,
    Midnight,
    Forest,
    BlackAndWhite,
    Onyx,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

pub mod accessible;
pub mod black;
pub mod black_and_white;
pub mod forest;
pub mod light;
pub mod midnight;
pub mod onyx;

impl ThemeId {
    pub fn all() -> &'static [ThemeId] {
        &[
            ThemeId::Dark,
            ThemeId::Light,
            ThemeId::Midnight,
            ThemeId::Forest,
            ThemeId::BlackAndWhite,
            ThemeId::Onyx,
            ThemeId::Protanopia,
            ThemeId::Deuteranopia,
            ThemeId::Tritanopia,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Light => "Light",
            ThemeId::Midnight => "Midnight",
            ThemeId::Forest => "Forest",
            ThemeId::BlackAndWhite => "Black & White",
            ThemeId::Onyx => "Onyx",
            ThemeId::Protanopia => "Protanopia",
            ThemeId::Deuteranopia => "Deuteranopia",
            ThemeId::Tritanopia => "Tritanopia",
        }
    }

    pub fn accessibility_palette(&self) -> AccessibilityPalette {
        match self {
            ThemeId::BlackAndWhite => AccessibilityPalette::HighContrast,
            ThemeId::Protanopia => AccessibilityPalette::Protanopia,
            ThemeId::Deuteranopia => AccessibilityPalette::Deuteranopia,
            ThemeId::Tritanopia => AccessibilityPalette::Tritanopia,
            _ => AccessibilityPalette::Standard,
        }
    }

    pub fn mode_preference(&self) -> ThemeModePreference {
        match self {
            ThemeId::Light => ThemeModePreference::Light,
            _ => ThemeModePreference::Dark,
        }
    }

    pub fn for_appearance(appearance: ThemeAppearance) -> ThemeId {
        match appearance {
            ThemeAppearance::Light => ThemeId::Light,
            ThemeAppearance::Dark => ThemeId::Dark,
        }
    }

    pub fn for_accessibility_palette(
        palette: AccessibilityPalette,
        appearance: ThemeAppearance,
    ) -> ThemeId {
        match palette {
            AccessibilityPalette::Standard => ThemeId::for_appearance(appearance),
            AccessibilityPalette::HighContrast => ThemeId::BlackAndWhite,
            AccessibilityPalette::Protanopia => ThemeId::Protanopia,
            AccessibilityPalette::Deuteranopia => ThemeId::Deuteranopia,
            AccessibilityPalette::Tritanopia => ThemeId::Tritanopia,
        }
    }

    pub fn next(&self) -> ThemeId {
        match self {
            ThemeId::Dark => ThemeId::Light,
            ThemeId::Light => ThemeId::Midnight,
            ThemeId::Midnight => ThemeId::Forest,
            ThemeId::Forest => ThemeId::BlackAndWhite,
            ThemeId::BlackAndWhite => ThemeId::Onyx,
            ThemeId::Onyx => ThemeId::Protanopia,
            ThemeId::Protanopia => ThemeId::Deuteranopia,
            ThemeId::Deuteranopia => ThemeId::Tritanopia,
            ThemeId::Tritanopia => ThemeId::Dark,
        }
    }
}

impl From<ThemeId> for UiKitThemeVariant {
    fn from(id: ThemeId) -> Self {
        match id {
            ThemeId::Dark => UiKitThemeVariant::Dark,
            ThemeId::Light => UiKitThemeVariant::Light,
            ThemeId::Midnight => UiKitThemeVariant::Midnight,
            ThemeId::Forest => UiKitThemeVariant::Forest,
            ThemeId::BlackAndWhite => UiKitThemeVariant::BlackAndWhite,
            ThemeId::Onyx => UiKitThemeVariant::Onyx,
            ThemeId::Protanopia | ThemeId::Deuteranopia | ThemeId::Tritanopia => {
                UiKitThemeVariant::Dark
            }
        }
    }
}

/// Curated community themes exposed by the app gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunityThemeId {
    Nord,
    Dracula,
}

impl CommunityThemeId {
    pub fn all() -> &'static [CommunityThemeId] {
        &[CommunityThemeId::Nord, CommunityThemeId::Dracula]
    }

    pub fn name(self) -> &'static str {
        match self {
            CommunityThemeId::Nord => "Nord",
            CommunityThemeId::Dracula => "Dracula",
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            CommunityThemeId::Nord => "nord",
            CommunityThemeId::Dracula => "dracula",
        }
    }

    pub fn author(self) -> &'static str {
        match self {
            CommunityThemeId::Nord => "SOTF Community",
            CommunityThemeId::Dracula => "SOTF Community",
        }
    }

    pub fn tags(self) -> &'static [&'static str] {
        match self {
            CommunityThemeId::Nord => &["community", "dark", "terminal"],
            CommunityThemeId::Dracula => &["community", "dark", "base16"],
        }
    }

    pub fn from_value(value: &SharedString) -> Option<Self> {
        Self::from_id(value.as_ref())
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "nord" => Some(Self::Nord),
            "dracula" => Some(Self::Dracula),
            _ => None,
        }
    }

    pub fn built_in_preset(self) -> BuiltInThemePreset {
        match self {
            CommunityThemeId::Nord => BuiltInThemePreset::Nord,
            CommunityThemeId::Dracula => BuiltInThemePreset::Dracula,
        }
    }

    pub fn editor_theme(self) -> EditorTheme {
        EditorTheme::preset(self.built_in_preset())
    }

    pub fn manifest(self) -> CommunityThemeManifest {
        let editor_theme = self.editor_theme();
        let mut manifest = CommunityThemeManifest::for_theme(&editor_theme);
        manifest.author = self.author().to_string();
        manifest.license = "MIT".to_string();
        manifest.tags = self.tags().iter().map(|tag| (*tag).to_string()).collect();
        manifest.preferred_mode = ThemeModePreference::Dark;
        manifest
    }

    pub fn bundle(self) -> CommunityThemeBundle {
        CommunityThemeBundle::new(self.manifest(), self.editor_theme())
    }

    pub fn theme(self) -> Theme {
        Theme::from_editor_theme(&self.editor_theme())
    }

    pub fn to_community_json(self) -> Result<String, serde_json::Error> {
        self.bundle().to_json()
    }
}

/// App-level accent override applied on top of the selected base theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeAccentPreference {
    #[default]
    Theme,
    System,
    Ocean,
    Mint,
    Amber,
    Rose,
    Violet,
}

impl ThemeAccentPreference {
    pub fn all() -> &'static [ThemeAccentPreference] {
        &[
            ThemeAccentPreference::Theme,
            ThemeAccentPreference::System,
            ThemeAccentPreference::Ocean,
            ThemeAccentPreference::Mint,
            ThemeAccentPreference::Amber,
            ThemeAccentPreference::Rose,
            ThemeAccentPreference::Violet,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            ThemeAccentPreference::Theme => "Default",
            ThemeAccentPreference::System => "System",
            ThemeAccentPreference::Ocean => "Ocean",
            ThemeAccentPreference::Mint => "Mint",
            ThemeAccentPreference::Amber => "Amber",
            ThemeAccentPreference::Rose => "Rose",
            ThemeAccentPreference::Violet => "Violet",
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            ThemeAccentPreference::Theme => "theme",
            ThemeAccentPreference::System => "system",
            ThemeAccentPreference::Ocean => "ocean",
            ThemeAccentPreference::Mint => "mint",
            ThemeAccentPreference::Amber => "amber",
            ThemeAccentPreference::Rose => "rose",
            ThemeAccentPreference::Violet => "violet",
        }
    }

    pub fn from_value(value: &SharedString) -> Option<Self> {
        match value.as_ref() {
            "theme" => Some(Self::Theme),
            "system" => Some(Self::System),
            "ocean" => Some(Self::Ocean),
            "mint" => Some(Self::Mint),
            "amber" => Some(Self::Amber),
            "rose" => Some(Self::Rose),
            "violet" => Some(Self::Violet),
            _ => None,
        }
    }

    pub fn seed_and_source(self) -> Option<(ThemeColor, AccentSource)> {
        match self {
            ThemeAccentPreference::Theme => None,
            // Fallback seed for platforms where GPUI does not expose native accent color yet.
            ThemeAccentPreference::System => {
                Some((ThemeColor::from_hex(0x0a84ff), AccentSource::System))
            }
            ThemeAccentPreference::Ocean => {
                Some((ThemeColor::from_hex(0x0072b2), AccentSource::User))
            }
            ThemeAccentPreference::Mint => {
                Some((ThemeColor::from_hex(0x009e73), AccentSource::User))
            }
            ThemeAccentPreference::Amber => {
                Some((ThemeColor::from_hex(0xe69f00), AccentSource::User))
            }
            ThemeAccentPreference::Rose => {
                Some((ThemeColor::from_hex(0xcc79a7), AccentSource::User))
            }
            ThemeAccentPreference::Violet => {
                Some((ThemeColor::from_hex(0x7e57c2), AccentSource::User))
            }
        }
    }

    pub fn preview_color(self, fallback: Rgba) -> Rgba {
        self.seed_and_source()
            .map(|(seed, _)| seed.to_rgba())
            .unwrap_or(fallback)
    }
}

/// Plugin type color mapping
#[derive(Debug, Clone)]
pub struct PluginColorMap {
    pub eq: Rgba,
    pub gain: Rgba,
    pub upmixer: Rgba,
    pub compressor: Rgba,
    pub limiter: Rgba,
    pub gate: Rgba,
    pub loudness: Rgba,
    pub binaural: Rgba,
    pub convolution: Rgba,
    pub monitor: Rgba,
    pub spectrum: Rgba,
    pub mute_solo: Rgba,
}

/// Graph visualization line colors
#[derive(Debug, Clone)]
pub struct GraphLineColors {
    pub input: Rgba,
    pub target: Rgba,
    pub filter_response: Rgba,
    pub corrected: Rgba,
    pub error: Rgba,
    pub deviation: Rgba,
    pub grid: Rgba,
    pub secondary_line: Rgba,
    pub directivity_er: Rgba,
    pub directivity_sp: Rgba,
}

/// EQ curve visualization colors
#[derive(Debug, Clone)]
pub struct EQCurveColors {
    pub background: Rgba,
    pub grid: Rgba,
    pub curve_boost: Rgba,
    pub curve_cut: Rgba,
    pub fill_boost: Rgba,
    pub fill_cut: Rgba,
    pub zero_line: Rgba,
}

/// Spectrum analyzer colors
#[derive(Debug, Clone)]
pub struct SpectrumColors {
    pub background: Rgba,
    pub bass: Rgba,   // Low frequency
    pub mids: Rgba,   // Mid frequency
    pub treble: Rgba, // High frequency
}

/// Level meter colors
#[derive(Debug, Clone)]
pub struct MeterColors {
    pub background: Rgba,
    pub normal: Rgba,
    pub warning: Rgba,
    pub clip: Rgba,
    pub peak: Rgba,
    pub text: Rgba,
}

/// Complete theme definition with all UI colors
#[derive(Debug, Clone)]
pub struct Theme {
    // Base colors
    pub background: Rgba,
    pub background_secondary: Rgba,
    pub background_tertiary: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,
    pub surface_selected: Rgba,

    // Text colors
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,

    // Border colors
    pub border: Rgba,
    pub border_focused: Rgba,

    // Accent colors
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_muted: Rgba,

    // Text on accent (for contrast on accent backgrounds)
    pub text_on_accent: Rgba,
    pub text_on_accent_muted: Rgba,
    pub icon_on_accent: Rgba,

    // Semantic colors
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub info: Rgba,

    // Level meter colors
    pub meter_normal: Rgba,
    pub meter_warning: Rgba,
    pub meter_clip: Rgba,

    // Button colors
    pub button_mute_active: Rgba,
    pub button_solo_active: Rgba,
    pub button_dim_active: Rgba,

    // Playback bar
    pub progress_bar_bg: Rgba,
    pub progress_bar_fill: Rgba,

    // Toast backgrounds
    pub toast_success_bg: Rgba,
    pub toast_error_bg: Rgba,
    pub toast_info_bg: Rgba,
    pub toast_warning_bg: Rgba,

    // Plugin colors
    pub plugin_colors: PluginColorMap,
    pub graph_colors: GraphLineColors,
    pub band_colors: Vec<Rgba>,
    pub channel_colors: Vec<Rgba>,
    pub eq_curve_colors: EQCurveColors,
    pub spectrum_colors: SpectrumColors,
    pub meter_colors: MeterColors,

    // Additional semantic colors
    pub peak_indicator: Rgba,
    pub drag_over_highlight: Rgba,
    pub drag_over_border: Rgba,
    pub neutral_indicator: Rgba,
    pub warning_background: Rgba,
    pub knob_color: Rgba,
    pub optimization_color: Rgba,
    pub grid_color: Rgba,
    pub overlay_bg: Rgba,

    // Layout sizes
    pub separator_size: f32,

    // Font family. `None` falls back to `cx.design().typography.font_family`,
    // letting the active design system (Apple HIG / Fluent / Material3 / Neutral)
    // pick a platform-native font. `Some(name)` overrides per theme.
    pub font_family: Option<SharedString>,

    // Design system tokens for platform-adaptive component geometry
    pub design_tokens: gpui_audio_kit::AudioDesignTokens,
}

impl Theme {
    /// Convert the app theme to the ui-kit theme so defaults are consistent without per-call overrides.
    pub fn to_ui_kit_theme(&self, id: ThemeId, cx: &App) -> UiKitTheme {
        UiKitTheme {
            variant: UiKitThemeVariant::from(id),
            background: self.background,
            surface: self.surface,
            surface_hover: self.surface_hover,
            muted: self.background_secondary,
            transparent: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            overlay_bg: self.overlay_bg,
            text_primary: self.text_primary,
            text_secondary: self.text_secondary,
            text_muted: self.text_muted,
            text_on_accent: self.text_on_accent,
            icon_on_accent: self.icon_on_accent,
            accent: self.accent,
            accent_hover: self.accent_hover,
            accent_muted: self.accent_muted,
            success: self.success,
            warning: self.warning,
            error: self.error,
            info: self.info,
            border: self.border,
            border_hover: self.border_focused,
            // Typography
            font_family: self.resolved_font_family(cx),
            // Badge colors - derive from semantic colors
            badge_primary_bg: Self::opacity_20pct(self.accent),
            badge_primary_text: self.accent,
            badge_success_bg: Self::opacity_20pct(self.success),
            badge_success_text: self.success,
            badge_warning_bg: Self::opacity_20pct(self.warning),
            badge_warning_text: self.warning,
            badge_error_bg: Self::opacity_20pct(self.error),
            badge_error_text: self.error,
            badge_info_bg: Self::opacity_20pct(self.info),
            badge_info_text: self.info,
            // Alert backgrounds - derive from semantic colors
            alert_info_bg: Self::opacity_20pct(self.info),
            alert_success_bg: Self::opacity_20pct(self.success),
            alert_warning_bg: Self::opacity_20pct(self.warning),
            alert_error_bg: Self::opacity_20pct(self.error),
            // Code text
            code_text: self.accent,
        }
    }

    /// Resolve the font family to use for this theme, falling back to the
    /// active design system's typography font when the theme has no override.
    pub fn resolved_font_family(&self, cx: &App) -> SharedString {
        self.font_family
            .clone()
            .unwrap_or_else(|| SharedString::from(cx.design().typography.font_family.clone()))
    }

    /// Create theme from ThemeId
    pub fn from_id(id: ThemeId) -> Self {
        match id {
            ThemeId::Dark => Self::dark(),
            ThemeId::Light => Self::light(),
            ThemeId::Midnight => Self::midnight(),
            ThemeId::Forest => Self::forest(),
            ThemeId::BlackAndWhite => Self::black_and_white(),
            ThemeId::Onyx => Self::onyx(),
            ThemeId::Protanopia => Self::protanopia(),
            ThemeId::Deuteranopia => Self::deuteranopia(),
            ThemeId::Tritanopia => Self::tritanopia(),
        }
    }

    pub fn from_community_bundle(bundle: &CommunityThemeBundle) -> Result<Self, String> {
        bundle.validate()?;
        let theme = Self::from_editor_theme(&bundle.theme);
        theme.validate_accessibility()?;
        Ok(theme)
    }

    pub fn from_editor_theme(editor_theme: &EditorTheme) -> Self {
        let mut theme = match editor_theme.appearance() {
            ThemeAppearance::Light => Self::light(),
            ThemeAppearance::Dark => Self::dark(),
        };

        theme.background = editor_theme.background.to_rgba();
        theme.background_secondary = editor_theme.background_secondary.to_rgba();
        theme.background_tertiary = editor_theme.background_tertiary.to_rgba();
        theme.surface = editor_theme.surface.to_rgba();
        theme.surface_hover = editor_theme.surface_hover.to_rgba();
        theme.surface_selected = editor_theme.surface_selected.to_rgba();

        theme.text_primary = editor_theme.text_primary.to_rgba();
        theme.text_secondary = editor_theme.text_secondary.to_rgba();
        theme.text_muted = editor_theme.text_muted.to_rgba();
        theme.text_disabled = editor_theme.text_disabled.to_rgba();

        theme.border = editor_theme.border.to_rgba();
        theme.border_focused = editor_theme.border_focused.to_rgba();

        theme.accent = editor_theme.accent.to_rgba();
        theme.accent_hover = editor_theme.accent_hover.to_rgba();
        theme.accent_muted = editor_theme.accent_muted.to_rgba();
        theme.text_on_accent = editor_theme.text_on_accent.to_rgba();
        theme.text_on_accent_muted = editor_theme.text_on_accent_muted.to_rgba();
        theme.icon_on_accent = theme.text_on_accent;

        theme.success = editor_theme.success.to_rgba();
        theme.warning = editor_theme.warning.to_rgba();
        theme.error = editor_theme.error.to_rgba();
        theme.info = editor_theme.info.to_rgba();

        theme.meter_normal = editor_theme.meter_normal.to_rgba();
        theme.meter_warning = editor_theme.meter_warning.to_rgba();
        theme.meter_clip = editor_theme.meter_clip.to_rgba();

        theme.button_mute_active = editor_theme.button_mute_active.to_rgba();
        theme.button_solo_active = editor_theme.button_solo_active.to_rgba();
        theme.button_dim_active = editor_theme.button_dim_active.to_rgba();

        theme.progress_bar_bg = editor_theme.progress_bar_bg.to_rgba();
        theme.progress_bar_fill = editor_theme.progress_bar_fill.to_rgba();

        theme.toast_success_bg = editor_theme.toast_success_bg.to_rgba();
        theme.toast_error_bg = editor_theme.toast_error_bg.to_rgba();
        theme.toast_info_bg = editor_theme.toast_info_bg.to_rgba();
        theme.toast_warning_bg = editor_theme.toast_warning_bg.to_rgba();

        theme.plugin_colors = PluginColorMap {
            eq: editor_theme.plugin_colors.eq.to_rgba(),
            gain: editor_theme.plugin_colors.gain.to_rgba(),
            upmixer: editor_theme.plugin_colors.upmixer.to_rgba(),
            compressor: editor_theme.plugin_colors.compressor.to_rgba(),
            limiter: editor_theme.plugin_colors.limiter.to_rgba(),
            gate: editor_theme.plugin_colors.gate.to_rgba(),
            loudness: editor_theme.plugin_colors.loudness.to_rgba(),
            binaural: editor_theme.plugin_colors.binaural.to_rgba(),
            convolution: editor_theme.plugin_colors.convolution.to_rgba(),
            monitor: editor_theme.plugin_colors.monitor.to_rgba(),
            spectrum: editor_theme.plugin_colors.spectrum.to_rgba(),
            mute_solo: editor_theme.plugin_colors.mute_solo.to_rgba(),
        };
        theme.graph_colors = GraphLineColors {
            input: editor_theme.graph_colors.input.to_rgba(),
            target: editor_theme.graph_colors.target.to_rgba(),
            filter_response: editor_theme.graph_colors.filter_response.to_rgba(),
            corrected: editor_theme.graph_colors.corrected.to_rgba(),
            error: editor_theme.graph_colors.error.to_rgba(),
            deviation: editor_theme.graph_colors.deviation.to_rgba(),
            grid: editor_theme.graph_colors.grid.to_rgba(),
            secondary_line: editor_theme.graph_colors.secondary_line.to_rgba(),
            directivity_er: editor_theme.graph_colors.directivity_er.to_rgba(),
            directivity_sp: editor_theme.graph_colors.directivity_sp.to_rgba(),
        };
        theme.band_colors = editor_theme
            .band_colors
            .iter()
            .map(|color| color.to_rgba())
            .collect();
        theme.channel_colors = theme.band_colors.clone();
        theme.eq_curve_colors = EQCurveColors {
            background: editor_theme.eq_curve_colors.background.to_rgba(),
            grid: editor_theme.eq_curve_colors.grid.to_rgba(),
            curve_boost: editor_theme.eq_curve_colors.curve_boost.to_rgba(),
            curve_cut: editor_theme.eq_curve_colors.curve_cut.to_rgba(),
            fill_boost: editor_theme.eq_curve_colors.fill_boost.to_rgba(),
            fill_cut: editor_theme.eq_curve_colors.fill_cut.to_rgba(),
            zero_line: editor_theme.eq_curve_colors.zero_line.to_rgba(),
        };
        theme.spectrum_colors = SpectrumColors {
            background: editor_theme.spectrum_colors.background.to_rgba(),
            bass: editor_theme.spectrum_colors.bass.to_rgba(),
            mids: editor_theme.spectrum_colors.mids.to_rgba(),
            treble: editor_theme.spectrum_colors.treble.to_rgba(),
        };
        theme.meter_colors = MeterColors {
            background: editor_theme.meter_colors.background.to_rgba(),
            normal: editor_theme.meter_colors.normal.to_rgba(),
            warning: editor_theme.meter_colors.warning.to_rgba(),
            clip: editor_theme.meter_colors.clip.to_rgba(),
            peak: editor_theme.meter_colors.peak.to_rgba(),
            text: editor_theme.meter_colors.text.to_rgba(),
        };

        theme.peak_indicator = editor_theme.peak_indicator.to_rgba();
        theme.drag_over_highlight = editor_theme.drag_over_highlight.to_rgba();
        theme.drag_over_border = editor_theme.drag_over_border.to_rgba();
        theme.neutral_indicator = editor_theme.neutral_indicator.to_rgba();
        theme.warning_background = editor_theme.warning_background.to_rgba();
        theme.knob_color = editor_theme.knob_color.to_rgba();
        theme.optimization_color = editor_theme.optimization_color.to_rgba();
        theme.grid_color = editor_theme.grid_color.to_rgba();
        theme.separator_size = editor_theme.separator_size;
        theme.font_family = if editor_theme.font_family.trim().is_empty() {
            None
        } else {
            Some(SharedString::from(editor_theme.font_family.clone()))
        };

        theme
    }

    pub fn with_accent_preference(self, preference: ThemeAccentPreference) -> Self {
        let Some((seed, source)) = preference.seed_and_source() else {
            return self;
        };
        self.with_accent_seed(seed, source)
    }

    pub fn with_accent_seed(self, seed: ThemeColor, source: AccentSource) -> Self {
        let palette = AccentPalette::from_seed(seed, source, self.appearance());
        self.with_accent_palette(palette)
    }

    pub fn with_accent_palette(mut self, palette: AccentPalette) -> Self {
        self.surface_selected = palette.accent_muted.to_rgba();
        self.border_focused = palette.accent.to_rgba();
        self.accent = palette.accent.to_rgba();
        self.accent_hover = palette.accent_hover.to_rgba();
        self.accent_muted = palette.accent_muted.to_rgba();
        self.text_on_accent = palette.text_on_accent.to_rgba();
        self.text_on_accent_muted = Self::with_opacity(self.text_on_accent, 0.8);
        self.icon_on_accent = self.text_on_accent;
        self.progress_bar_fill = self.accent;
        self.drag_over_highlight = Self::with_opacity(self.accent, 0.25);
        self.drag_over_border = self.accent;
        self.neutral_indicator = self.accent;
        self.optimization_color = self.accent_hover;
        self.plugin_colors.eq = self.accent;
        self.plugin_colors.convolution = self.accent;
        self.graph_colors.corrected = self.accent;
        self
    }

    pub fn appearance(&self) -> ThemeAppearance {
        let luminance =
            0.2126 * self.background.r + 0.7152 * self.background.g + 0.0722 * self.background.b;
        if luminance >= 0.5 {
            ThemeAppearance::Light
        } else {
            ThemeAppearance::Dark
        }
    }
}

/// Helper function to create Rgba from 24-bit hex value (alpha = 1.0)
pub(crate) fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

/// Helper function to create Rgba from 32-bit hex value (RRGGBBAA)
pub(crate) fn rgba(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 24) & 0xFF) as f32 / 255.0,
        g: ((hex >> 16) & 0xFF) as f32 / 255.0,
        b: ((hex >> 8) & 0xFF) as f32 / 255.0,
        a: (hex & 0xFF) as f32 / 255.0,
    }
}

impl Theme {
    /// Apply opacity to a color (0.0 = transparent, 1.0 = opaque)
    pub fn with_opacity(color: Rgba, opacity: f32) -> Rgba {
        let mut c = color;
        c.a = opacity.clamp(0.0, 1.0);
        c
    }

    /// Common opacity: 8% (~21 alpha)
    pub fn opacity_8pct(color: Rgba) -> Rgba {
        Self::with_opacity(color, 0.08)
    }

    /// Common opacity: 20% (~51 alpha)
    pub fn opacity_20pct(color: Rgba) -> Rgba {
        Self::with_opacity(color, 0.2)
    }

    /// Common opacity: 25% (~64 alpha)
    pub fn opacity_25pct(color: Rgba) -> Rgba {
        Self::with_opacity(color, 0.25)
    }

    /// Common opacity: 50% (~128 alpha)
    pub fn opacity_50pct(color: Rgba) -> Rgba {
        Self::with_opacity(color, 0.5)
    }

    /// Convert to ButtonTheme for use with ui_kit Button component
    pub fn to_button_theme(&self) -> gpui_ui_kit::ButtonTheme {
        gpui_ui_kit::ButtonTheme {
            accent: self.accent,
            accent_hover: self.accent_hover,
            surface: self.surface,
            surface_hover: self.surface_hover,
            text_primary: self.text_primary,
            text_secondary: self.text_secondary,
            // Use text_on_accent for Primary variant buttons (on accent background)
            text_on_accent: self.text_on_accent,
            error: self.error,
            error_hover: Self::with_opacity(self.error, 0.8),
            border: self.border,
            transparent: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        }
    }

    /// Convert to AccordionTheme for use with ui_kit Accordion component
    pub fn to_accordion_theme(&self) -> gpui_ui_kit::AccordionTheme {
        gpui_ui_kit::AccordionTheme {
            header_bg: self.surface,
            header_hover_bg: self.surface_hover,
            header_active_bg: self.accent_muted,
            content_bg: self.background,
            border: Self::with_opacity(self.text_muted, 0.62),
            accent_tint: Self::with_opacity(self.accent, 0.46),
            accent: self.accent,
            title_color: self.text_primary,
            indicator_color: self.text_muted,
        }
    }

    /// Convert to SliderTheme for use with ui_kit Slider component
    pub fn to_slider_theme(&self) -> gpui_ui_kit::SliderTheme {
        gpui_ui_kit::SliderTheme {
            track: self.surface_hover,
            fill: self.accent,
            thumb: self.text_primary,
            thumb_hover: self.text_secondary,
            thumb_active: self.accent,
            label: self.text_primary,
            value: self.text_secondary,
            disabled_label: Self::opacity_50pct(self.text_muted),
            disabled_fill: self.text_muted,
        }
    }

    /// Convert to IconButtonTheme for use with ui_kit IconButton component
    pub fn to_icon_button_theme(&self) -> gpui_ui_kit::IconButtonTheme {
        gpui_ui_kit::IconButtonTheme {
            ghost_bg: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            ghost_hover_bg: self.surface_hover,
            selected_bg: self.surface_selected,
            selected_hover_bg: self.surface_hover,
            filled_bg: self.surface,
            filled_hover_bg: self.surface_hover,
            accent: self.accent,
            accent_hover: self.accent_hover,
            text: self.text_primary,
            text_on_accent: self.text_on_accent,
            border: self.border,
        }
    }

    /// Convert to TabsTheme for use with ui_kit Tabs component
    pub fn to_tabs_theme(&self) -> gpui_ui_kit::TabsTheme {
        gpui_ui_kit::TabsTheme {
            container_bg: self.surface,
            container_border: self.border,
            selected_bg: self.surface_selected,
            selected_hover_bg: self.surface_hover,
            hover_bg: self.surface_hover,
            accent: self.accent,
            // Use text_on_accent for selected text since accent is used as background
            text_selected: self.text_on_accent,
            text_unselected: self.text_secondary,
            text_hover: self.text_primary,
            badge_bg: self.surface_hover,
            close_color: self.text_muted,
            close_hover_color: self.text_primary,
            icon_selected: Some(self.icon_on_accent),
            icon_unselected: None,
        }
    }

    /// Convert to MenuTheme for use with ui_kit Menu component
    pub fn to_menu_theme(&self) -> gpui_ui_kit::MenuTheme {
        gpui_ui_kit::MenuTheme {
            background: self.surface,
            border: self.border,
            separator: self.border,
            text: self.text_secondary,
            text_hover: self.text_primary,
            text_disabled: self.text_disabled,
            text_shortcut: self.text_muted,
            hover_bg: self.surface_hover,
            danger_hover_bg: self.error,
        }
    }

    /// Convert to PotentiometerTheme for use with audio-kit Potentiometer component
    pub fn to_potentiometer_theme(&self) -> gpui_audio_kit::PotentiometerTheme {
        gpui_audio_kit::PotentiometerTheme {
            surface: self.surface,
            surface_hover: self.surface_hover,
            knob_bg: self.background_secondary,
            accent: self.accent,
            accent_muted: self.accent_muted,
            border: self.border,
            text_secondary: self.text_secondary,
            text_primary: self.text_primary,
            text_muted: self.text_muted,
            text_on_accent: self.text_on_accent,
            background_secondary: self.background_secondary,
        }
    }

    /// Convert to ToggleTheme for use with ui_kit Toggle component
    pub fn to_toggle_theme(&self) -> gpui_ui_kit::ToggleTheme {
        gpui_ui_kit::ToggleTheme {
            checked_bg: self.accent,
            unchecked_bg: self.surface,
            knob: self.text_primary,
            knob_on_checked: self.text_on_accent,
            track_border: self.border,
            label: self.text_secondary,
            accent: self.accent,
            accent_muted: self.accent_muted,
            success: self.success,
            border: self.border,
            text_on_accent: self.text_on_accent,
            text_muted: self.text_muted,
            text_primary: self.text_primary,
            surface_hover: self.surface_hover,
            background: self.background,
        }
    }

    /// Convert to ButtonSetTheme for use with ui_kit ButtonSet component
    pub fn to_button_set_theme(&self) -> gpui_ui_kit::ButtonSetTheme {
        gpui_ui_kit::ButtonSetTheme {
            bg: self.surface,
            bg_hover: self.surface_hover,
            bg_selected: self.accent,
            text_color: self.text_secondary,
            text_color_selected: self.text_on_accent,
            border: self.border,
            border_selected: self.accent,
        }
    }

    /// Convert to ContextMenuTheme for use with ui_kit ContextMenu component
    pub fn to_context_menu_theme(&self) -> gpui_ui_kit::ContextMenuTheme {
        gpui_ui_kit::ContextMenuTheme {
            backdrop: gpui::rgba(0x00000001),
            background: self.surface,
            border: self.border,
            separator: self.border,
            text: self.text_secondary,
            text_hover: self.text_primary,
            text_disabled: self.text_disabled,
            text_shortcut: self.text_muted,
            hover_bg: self.surface_hover,
            danger_hover_bg: self.error,
        }
    }

    /// Convert to SelectTheme for use with ui_kit Select component
    pub fn to_select_theme(&self) -> gpui_ui_kit::SelectTheme {
        gpui_ui_kit::SelectTheme {
            trigger_bg: self.surface,
            trigger_border: self.border,
            trigger_border_hover: self.accent,
            trigger_border_focused: self.accent,
            dropdown_bg: self.surface,
            dropdown_border: self.border,
            selected_bg: self.accent,
            option_hover_bg: self.surface_hover,
            label_color: self.text_secondary,
            text_color: self.text_primary,
            placeholder_color: self.text_muted,
            option_text_color: self.text_secondary,
            selected_text_color: self.text_on_accent,
            disabled_color: self.text_muted,
            arrow_color: self.text_muted,
        }
    }
}
