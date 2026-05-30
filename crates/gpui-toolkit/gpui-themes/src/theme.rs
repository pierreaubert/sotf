//! Theme definition with serialization support
//!
//! Provides a serializable theme structure that can be exported to JSON or Rust code.

use serde::{Deserialize, Serialize};

// Re-export Color from gpui-ui-kit
pub use gpui_ui_kit::Color;

/// Plugin type color mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginColors {
    pub eq: Color,
    pub gain: Color,
    pub upmixer: Color,
    pub compressor: Color,
    pub limiter: Color,
    pub gate: Color,
    pub loudness: Color,
    pub binaural: Color,
    pub convolution: Color,
    pub monitor: Color,
    pub spectrum: Color,
    pub mute_solo: Color,
}

/// Graph visualization line colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphColors {
    pub input: Color,
    pub target: Color,
    pub filter_response: Color,
    pub corrected: Color,
    pub error: Color,
    pub deviation: Color,
    pub grid: Color,
    pub secondary_line: Color,
    pub directivity_er: Color,
    pub directivity_sp: Color,
}

/// EQ curve visualization colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQCurveColors {
    pub background: Color,
    pub grid: Color,
    pub curve_boost: Color,
    pub curve_cut: Color,
    pub fill_boost: Color,
    pub fill_cut: Color,
    pub zero_line: Color,
}

/// Spectrum analyzer colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumColors {
    pub background: Color,
    pub bass: Color,
    pub mids: Color,
    pub treble: Color,
}

/// Level meter colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeterColors {
    pub background: Color,
    pub normal: Color,
    pub warning: Color,
    pub clip: Color,
    pub peak: Color,
    pub text: Color,
}

/// Current version for shareable community theme bundles.
pub const COMMUNITY_THEME_SCHEMA_VERSION: u32 = 1;

/// Resolved light/dark appearance after system preference and scheduling are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeAppearance {
    Light,
    #[default]
    Dark,
}

/// Time of day used by scheduled appearance switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeOfDay {
    pub hour: u8,
    pub minute: u8,
}

impl TimeOfDay {
    pub const fn new(hour: u8, minute: u8) -> Self {
        Self { hour, minute }
    }

    pub fn checked_new(hour: u8, minute: u8) -> Option<Self> {
        if hour < 24 && minute < 60 {
            Some(Self { hour, minute })
        } else {
            None
        }
    }

    pub const fn minutes_after_midnight(self) -> u16 {
        self.hour as u16 * 60 + self.minute as u16
    }
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// Local schedule for apps that do not rely only on the operating system setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeSchedule {
    #[serde(default = "ThemeSchedule::default_light_start")]
    pub light_start: TimeOfDay,
    #[serde(default = "ThemeSchedule::default_dark_start")]
    pub dark_start: TimeOfDay,
}

impl ThemeSchedule {
    pub const fn new(light_start: TimeOfDay, dark_start: TimeOfDay) -> Self {
        Self {
            light_start,
            dark_start,
        }
    }

    pub const fn default_light_start() -> TimeOfDay {
        TimeOfDay::new(7, 0)
    }

    pub const fn default_dark_start() -> TimeOfDay {
        TimeOfDay::new(18, 0)
    }

    pub fn resolve_at_minutes(self, minutes_after_midnight: u16) -> ThemeAppearance {
        let minute = minutes_after_midnight % (24 * 60);
        let light_start = self.light_start.minutes_after_midnight();
        let dark_start = self.dark_start.minutes_after_midnight();

        if light_start == dark_start {
            return ThemeAppearance::Dark;
        }

        let is_light = if light_start < dark_start {
            minute >= light_start && minute < dark_start
        } else {
            minute >= light_start || minute < dark_start
        };

        if is_light {
            ThemeAppearance::Light
        } else {
            ThemeAppearance::Dark
        }
    }
}

impl Default for ThemeSchedule {
    fn default() -> Self {
        Self::new(Self::default_light_start(), Self::default_dark_start())
    }
}

/// Per-app theme mode override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ThemeModePreference {
    #[default]
    FollowSystem,
    Light,
    Dark,
    Scheduled {
        schedule: ThemeSchedule,
    },
}

impl ThemeModePreference {
    pub fn resolve(
        &self,
        system_appearance: ThemeAppearance,
        minutes_after_midnight: u16,
    ) -> ThemeAppearance {
        match self {
            Self::FollowSystem => system_appearance,
            Self::Light => ThemeAppearance::Light,
            Self::Dark => ThemeAppearance::Dark,
            Self::Scheduled { schedule } => schedule.resolve_at_minutes(minutes_after_midnight),
        }
    }
}

/// Accessibility classification for presets and community themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilityPalette {
    #[default]
    Standard,
    HighContrast,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl AccessibilityPalette {
    pub fn all() -> &'static [AccessibilityPalette] {
        &[
            AccessibilityPalette::Standard,
            AccessibilityPalette::HighContrast,
            AccessibilityPalette::Protanopia,
            AccessibilityPalette::Deuteranopia,
            AccessibilityPalette::Tritanopia,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            AccessibilityPalette::Standard => "Standard",
            AccessibilityPalette::HighContrast => "High Contrast",
            AccessibilityPalette::Protanopia => "Protanopia",
            AccessibilityPalette::Deuteranopia => "Deuteranopia",
            AccessibilityPalette::Tritanopia => "Tritanopia",
        }
    }

    pub fn is_color_blind_safe(self) -> bool {
        matches!(
            self,
            AccessibilityPalette::Protanopia
                | AccessibilityPalette::Deuteranopia
                | AccessibilityPalette::Tritanopia
        )
    }
}

/// Origin for an accent palette seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccentSource {
    #[default]
    Theme,
    System,
    Wallpaper,
    User,
}

/// Harmonized accent colors generated from a system, wallpaper, user, or preset seed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AccentPalette {
    pub source: AccentSource,
    pub seed: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_muted: Color,
    pub text_on_accent: Color,
}

impl AccentPalette {
    pub fn from_seed(seed: Color, source: AccentSource, appearance: ThemeAppearance) -> Self {
        let hover_delta = match appearance {
            ThemeAppearance::Light => -0.08,
            ThemeAppearance::Dark => 0.12,
        };
        let muted_lightness = match appearance {
            ThemeAppearance::Light => 0.88,
            ThemeAppearance::Dark => 0.24,
        };
        let (h, s, _) = seed.to_hsl();
        let accent_muted =
            Color::from_hsl(h, (s * 0.55).clamp(0.25, 0.8), muted_lightness).with_alpha(0.85);

        Self {
            source,
            seed,
            accent: seed,
            accent_hover: shift_lightness(seed, hover_delta),
            accent_muted,
            text_on_accent: readable_text_color(seed),
        }
    }
}

/// Easing curve for theme transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeTransitionEasing {
    Linear,
    #[default]
    EaseOut,
    EaseInOut,
}

/// Theme transition settings shared by frontends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeTransition {
    pub duration_ms: u16,
    pub easing: ThemeTransitionEasing,
    pub cross_fade: bool,
}

impl ThemeTransition {
    pub fn disabled() -> Self {
        Self {
            duration_ms: 0,
            easing: ThemeTransitionEasing::Linear,
            cross_fade: false,
        }
    }

    pub fn effective_duration_ms(self, reduce_motion: bool) -> u16 {
        if reduce_motion {
            0
        } else {
            self.duration_ms
        }
    }
}

impl Default for ThemeTransition {
    fn default() -> Self {
        Self {
            duration_ms: 220,
            easing: ThemeTransitionEasing::EaseOut,
            cross_fade: true,
        }
    }
}

/// Built-in editor theme presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BuiltInThemePreset {
    #[default]
    Dark,
    Light,
    HighContrast,
    Nord,
    Dracula,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl BuiltInThemePreset {
    pub fn all() -> &'static [BuiltInThemePreset] {
        &[
            BuiltInThemePreset::Dark,
            BuiltInThemePreset::Light,
            BuiltInThemePreset::HighContrast,
            BuiltInThemePreset::Nord,
            BuiltInThemePreset::Dracula,
            BuiltInThemePreset::Protanopia,
            BuiltInThemePreset::Deuteranopia,
            BuiltInThemePreset::Tritanopia,
        ]
    }

    pub fn id(self) -> &'static str {
        match self {
            BuiltInThemePreset::Dark => "dark",
            BuiltInThemePreset::Light => "light",
            BuiltInThemePreset::HighContrast => "high_contrast",
            BuiltInThemePreset::Nord => "nord",
            BuiltInThemePreset::Dracula => "dracula",
            BuiltInThemePreset::Protanopia => "protanopia",
            BuiltInThemePreset::Deuteranopia => "deuteranopia",
            BuiltInThemePreset::Tritanopia => "tritanopia",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BuiltInThemePreset::Dark => "Dark",
            BuiltInThemePreset::Light => "Light",
            BuiltInThemePreset::HighContrast => "High Contrast",
            BuiltInThemePreset::Nord => "Nord",
            BuiltInThemePreset::Dracula => "Dracula",
            BuiltInThemePreset::Protanopia => "Protanopia",
            BuiltInThemePreset::Deuteranopia => "Deuteranopia",
            BuiltInThemePreset::Tritanopia => "Tritanopia",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match normalize_theme_id(id).as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "high_contrast" | "highcontrast" => Some(Self::HighContrast),
            "nord" => Some(Self::Nord),
            "dracula" => Some(Self::Dracula),
            "protanopia" => Some(Self::Protanopia),
            "deuteranopia" => Some(Self::Deuteranopia),
            "tritanopia" => Some(Self::Tritanopia),
            _ => None,
        }
    }

    pub fn accessibility(self) -> AccessibilityPalette {
        match self {
            BuiltInThemePreset::HighContrast => AccessibilityPalette::HighContrast,
            BuiltInThemePreset::Protanopia => AccessibilityPalette::Protanopia,
            BuiltInThemePreset::Deuteranopia => AccessibilityPalette::Deuteranopia,
            BuiltInThemePreset::Tritanopia => AccessibilityPalette::Tritanopia,
            _ => AccessibilityPalette::Standard,
        }
    }

    pub fn to_theme(self) -> EditorTheme {
        match self {
            BuiltInThemePreset::Dark => EditorTheme::dark(),
            BuiltInThemePreset::Light => EditorTheme::light(),
            BuiltInThemePreset::HighContrast => EditorTheme::high_contrast(),
            BuiltInThemePreset::Nord => EditorTheme::nord(),
            BuiltInThemePreset::Dracula => EditorTheme::dracula(),
            BuiltInThemePreset::Protanopia => EditorTheme::protanopia(),
            BuiltInThemePreset::Deuteranopia => EditorTheme::deuteranopia(),
            BuiltInThemePreset::Tritanopia => EditorTheme::tritanopia(),
        }
    }
}

/// Metadata wrapper for shareable JSON theme files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunityThemeManifest {
    #[serde(default = "default_community_theme_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub accessibility: AccessibilityPalette,
    #[serde(default)]
    pub preferred_mode: ThemeModePreference,
    #[serde(default)]
    pub accent_source: AccentSource,
    #[serde(default)]
    pub transition: ThemeTransition,
}

impl CommunityThemeManifest {
    pub fn for_theme(theme: &EditorTheme) -> Self {
        Self {
            schema_version: COMMUNITY_THEME_SCHEMA_VERSION,
            id: slugify_theme_name(&theme.name),
            display_name: theme.name.clone(),
            author: String::new(),
            license: String::new(),
            tags: Vec::new(),
            accessibility: AccessibilityPalette::Standard,
            preferred_mode: ThemeModePreference::default(),
            accent_source: AccentSource::Theme,
            transition: ThemeTransition::default(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != COMMUNITY_THEME_SCHEMA_VERSION {
            return Err(format!(
                "unsupported community theme schema version {}",
                self.schema_version
            ));
        }
        if self.id.trim().is_empty() {
            return Err("theme manifest id must not be empty".to_string());
        }
        if self.display_name.trim().is_empty() {
            return Err("theme manifest display_name must not be empty".to_string());
        }
        Ok(())
    }
}

/// Shareable JSON theme bundle used by theme galleries and local imports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityThemeBundle {
    pub manifest: CommunityThemeManifest,
    pub theme: EditorTheme,
}

impl CommunityThemeBundle {
    pub fn new(manifest: CommunityThemeManifest, theme: EditorTheme) -> Self {
        Self { manifest, theme }
    }

    pub fn from_theme(theme: EditorTheme) -> Self {
        Self {
            manifest: CommunityThemeManifest::for_theme(&theme),
            theme,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.manifest.validate()?;
        self.theme.validate()
    }
}

fn default_community_theme_schema_version() -> u32 {
    COMMUNITY_THEME_SCHEMA_VERSION
}

fn normalize_theme_id(id: &str) -> String {
    id.chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c == '-' || c == '_' || c.is_ascii_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .collect()
}

fn slugify_theme_name(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    if slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "custom-theme".to_string()
    } else {
        slug
    }
}

fn shift_lightness(color: Color, delta: f32) -> Color {
    let (h, s, l) = color.to_hsl();
    Color::from_hsl(h, s, (l + delta).clamp(0.0, 1.0))
}

fn readable_text_color(background: Color) -> Color {
    let black = Color::from_hex(0x000000);
    let white = Color::from_hex(0xffffff);

    if contrast_ratio(black, background) >= contrast_ratio(white, background) {
        black
    } else {
        white
    }
}

fn contrast_ratio(foreground: Color, background: Color) -> f32 {
    let foreground_luminance = relative_luminance(foreground);
    let background_luminance = relative_luminance(background);
    let lighter = foreground_luminance.max(background_luminance);
    let darker = foreground_luminance.min(background_luminance);
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(color: Color) -> f32 {
    fn channel(value: u8) -> f32 {
        let value = value as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

/// Complete theme definition with all UI colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorTheme {
    /// Theme name for display
    pub name: String,

    // Base colors
    pub background: Color,
    pub background_secondary: Color,
    pub background_tertiary: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub surface_selected: Color,

    // Text colors
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_disabled: Color,

    // Border colors
    pub border: Color,
    pub border_focused: Color,

    // Accent colors
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_muted: Color,

    // Text on accent
    pub text_on_accent: Color,
    pub text_on_accent_muted: Color,

    // Semantic colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // Level meter colors
    pub meter_normal: Color,
    pub meter_warning: Color,
    pub meter_clip: Color,

    // Button colors
    pub button_mute_active: Color,
    pub button_solo_active: Color,
    pub button_dim_active: Color,

    // Playback bar
    pub progress_bar_bg: Color,
    pub progress_bar_fill: Color,

    // Toast backgrounds
    pub toast_success_bg: Color,
    pub toast_error_bg: Color,
    pub toast_info_bg: Color,
    pub toast_warning_bg: Color,

    // Plugin colors
    pub plugin_colors: PluginColors,
    pub graph_colors: GraphColors,
    pub band_colors: Vec<Color>,
    pub eq_curve_colors: EQCurveColors,
    pub spectrum_colors: SpectrumColors,
    pub meter_colors: MeterColors,

    // Additional colors
    pub peak_indicator: Color,
    pub drag_over_highlight: Color,
    pub drag_over_border: Color,
    pub neutral_indicator: Color,
    pub warning_background: Color,
    pub knob_color: Color,
    pub optimization_color: Color,
    pub grid_color: Color,

    // Layout sizes
    pub separator_size: f32,

    // Font family
    pub font_family: String,

    /// Platform design language identifier (e.g., "neutral", "apple_hig", "material3", "fluent").
    /// Informational — the actual design system parameters live in sotf-host::DesignSystem.
    #[serde(default = "default_design_language")]
    pub design_language: String,
}

fn default_design_language() -> String {
    "neutral".to_string()
}

impl Default for EditorTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl EditorTheme {
    pub fn preset(preset: BuiltInThemePreset) -> Self {
        preset.to_theme()
    }

    pub fn accessibility_preset(accessibility: AccessibilityPalette) -> Self {
        match accessibility {
            AccessibilityPalette::Standard => Self::dark(),
            AccessibilityPalette::HighContrast => Self::high_contrast(),
            AccessibilityPalette::Protanopia => Self::protanopia(),
            AccessibilityPalette::Deuteranopia => Self::deuteranopia(),
            AccessibilityPalette::Tritanopia => Self::tritanopia(),
        }
    }

    /// Infer whether the theme is visually light or dark from the main background.
    pub fn appearance(&self) -> ThemeAppearance {
        if relative_luminance(self.background) >= 0.5 {
            ThemeAppearance::Light
        } else {
            ThemeAppearance::Dark
        }
    }

    /// Return a copy with the supplied accent palette applied to common accent slots.
    pub fn with_accent_palette(mut self, palette: AccentPalette) -> Self {
        self.accent = palette.accent;
        self.accent_hover = palette.accent_hover;
        self.accent_muted = palette.accent_muted;
        self.text_on_accent = palette.text_on_accent;
        self.text_on_accent_muted = palette.text_on_accent.with_alpha(0.8);
        self.border_focused = palette.accent;
        self.progress_bar_fill = palette.accent;
        self.drag_over_border = palette.accent;
        self.drag_over_highlight = palette.accent.with_alpha(0.25);
        self.neutral_indicator = palette.accent;
        self.optimization_color = palette.accent_hover;
        self
    }

    /// Return a copy with a harmonized accent generated from a system, wallpaper, or user seed.
    pub fn with_accent_seed(self, seed: Color, source: AccentSource) -> Self {
        let palette = AccentPalette::from_seed(seed, source, self.appearance());
        self.with_accent_palette(palette)
    }

    pub fn to_community_bundle(&self) -> CommunityThemeBundle {
        CommunityThemeBundle::from_theme(self.clone())
    }

    pub fn to_community_json(&self) -> Result<String, serde_json::Error> {
        self.to_community_bundle().to_json()
    }

    /// Validate WCAG AA contrast for core text/accent pairings.
    pub fn validate_accessibility(&self) -> Result<(), String> {
        let text_background = contrast_ratio(self.text_primary, self.background);
        if text_background < 4.5 {
            return Err(format!(
                "text_primary/background contrast {:.2}:1 is below WCAG AA",
                text_background
            ));
        }

        let text_surface = contrast_ratio(self.text_primary, self.surface);
        if text_surface < 4.5 {
            return Err(format!(
                "text_primary/surface contrast {:.2}:1 is below WCAG AA",
                text_surface
            ));
        }

        let accent_text = contrast_ratio(self.text_on_accent, self.accent);
        if accent_text < 4.5 {
            return Err(format!(
                "text_on_accent/accent contrast {:.2}:1 is below WCAG AA",
                accent_text
            ));
        }

        Ok(())
    }

    fn with_accessible_semantics(
        mut self,
        name: &str,
        accent: Color,
        success: Color,
        warning: Color,
        error: Color,
        info: Color,
        secondary: Color,
    ) -> Self {
        self.name = name.to_string();
        self = self.with_accent_seed(accent, AccentSource::Theme);
        self.success = success;
        self.warning = warning;
        self.error = error;
        self.info = info;
        self.meter_normal = success;
        self.meter_warning = warning;
        self.meter_clip = error;
        self.button_mute_active = error;
        self.button_solo_active = warning;
        self.button_dim_active = secondary;
        self.toast_success_bg = success.with_alpha(0.22);
        self.toast_error_bg = error.with_alpha(0.22);
        self.toast_info_bg = info.with_alpha(0.22);
        self.toast_warning_bg = warning.with_alpha(0.22);
        self.plugin_colors = PluginColors {
            eq: accent,
            gain: success,
            upmixer: secondary,
            compressor: error,
            limiter: warning,
            gate: secondary,
            loudness: info,
            binaural: Color::from_hex(0xcc79a7),
            convolution: accent,
            monitor: success,
            spectrum: secondary,
            mute_solo: info,
        };
        self.graph_colors = GraphColors {
            input: info,
            target: success,
            filter_response: warning,
            corrected: accent,
            error,
            deviation: secondary,
            grid: self.grid_color,
            secondary_line: self.text_secondary,
            directivity_er: Color::from_hex(0xcc79a7),
            directivity_sp: secondary,
        };
        self.band_colors = vec![
            error,
            warning,
            success,
            info,
            accent,
            secondary,
            Color::from_hex(0xcc79a7),
            Color::from_hex(0x999999),
        ];
        self.eq_curve_colors.curve_boost = success;
        self.eq_curve_colors.curve_cut = error;
        self.eq_curve_colors.fill_boost = success.with_alpha(0.28);
        self.eq_curve_colors.fill_cut = error.with_alpha(0.28);
        self.spectrum_colors.bass = success;
        self.spectrum_colors.mids = warning;
        self.spectrum_colors.treble = error;
        self.meter_colors.normal = success;
        self.meter_colors.warning = warning;
        self.meter_colors.clip = error;
        self
    }

    /// Create the default dark theme
    pub fn dark() -> Self {
        Self {
            name: "Dark".to_string(),

            // Base colors
            background: Color::from_hex(0x1e1e1e),
            background_secondary: Color::from_hex(0x252525),
            background_tertiary: Color::from_hex(0x2d2d2d),
            surface: Color::from_hex(0x2d2d2d),
            surface_hover: Color::from_hex(0x3e3e3e),
            surface_selected: Color::from_hex(0x264f78),

            // Text colors
            text_primary: Color::from_hex(0xcccccc),
            text_secondary: Color::from_hex(0x999999),
            text_muted: Color::from_hex(0x666666),
            text_disabled: Color::from_hex(0x444444),

            // Border colors
            border: Color::from_hex(0x3e3e3e),
            border_focused: Color::from_hex(0x007acc),

            // Accent colors
            accent: Color::from_hex(0x007acc),
            accent_hover: Color::from_hex(0x1c8cd9),
            accent_muted: Color::from_hex(0x264f78),

            // Text on accent
            text_on_accent: Color::from_hex(0xffffff),
            text_on_accent_muted: Color::new(255, 255, 255, 204),

            // Semantic colors
            success: Color::from_hex(0x4ec9b0),
            warning: Color::from_hex(0xdcdcaa),
            error: Color::from_hex(0xf48771),
            info: Color::from_hex(0x569cd6),

            // Level meter colors
            meter_normal: Color::from_hex(0x22c55e),
            meter_warning: Color::from_hex(0xf59e0b),
            meter_clip: Color::from_hex(0xdc2626),

            // Button colors
            button_mute_active: Color::from_hex(0xdc2626),
            button_solo_active: Color::from_hex(0xf59e0b),
            button_dim_active: Color::from_hex(0x6366f1),

            // Playback bar
            progress_bar_bg: Color::from_hex(0x3e3e3e),
            progress_bar_fill: Color::from_hex(0x007acc),

            // Toast backgrounds
            toast_success_bg: Color::from_hex(0x1e3a1e),
            toast_error_bg: Color::from_hex(0x3a1e1e),
            toast_info_bg: Color::from_hex(0x1e2a3a),
            toast_warning_bg: Color::from_hex(0x3a2e1e),

            // Plugin colors
            plugin_colors: PluginColors {
                eq: Color::from_hex(0x2563eb),
                gain: Color::from_hex(0x059669),
                upmixer: Color::from_hex(0x7c3aed),
                compressor: Color::from_hex(0xdc2626),
                limiter: Color::from_hex(0xea580c),
                gate: Color::from_hex(0xca8a04),
                loudness: Color::from_hex(0x0891b2),
                binaural: Color::from_hex(0xdb2777),
                convolution: Color::from_hex(0x4f46e5),
                monitor: Color::from_hex(0x14b8a6),
                spectrum: Color::from_hex(0x8b5cf6),
                mute_solo: Color::from_hex(0x6366f1),
            },
            graph_colors: GraphColors {
                input: Color::from_hex(0x6366f1),
                target: Color::from_hex(0x22c55e),
                filter_response: Color::from_hex(0xf59e0b),
                corrected: Color::from_hex(0x3b82f6),
                error: Color::from_hex(0xef4444),
                deviation: Color::from_hex(0x8b5cf6),
                grid: Color::new(255, 255, 255, 21),
                secondary_line: Color::from_hex(0xaaaaaa),
                directivity_er: Color::from_hex(0xf472b6),
                directivity_sp: Color::from_hex(0xc084fc),
            },
            band_colors: vec![
                Color::from_hex(0xef4444),
                Color::from_hex(0xf97316),
                Color::from_hex(0xeab308),
                Color::from_hex(0x22c55e),
                Color::from_hex(0x14b8a6),
                Color::from_hex(0x3b82f6),
                Color::from_hex(0x8b5cf6),
                Color::from_hex(0xec4899),
                Color::from_hex(0x6366f1),
                Color::from_hex(0x06b6d4),
                Color::from_hex(0x9ca3af),
            ],
            eq_curve_colors: EQCurveColors {
                background: Color::from_hex(0x1a1a1a),
                grid: Color::new(255, 255, 255, 32),
                curve_boost: Color::from_hex(0x22c55e),
                curve_cut: Color::from_hex(0xef4444),
                fill_boost: Color::new(34, 197, 94, 64),
                fill_cut: Color::new(239, 68, 68, 64),
                zero_line: Color::new(255, 255, 255, 64),
            },
            spectrum_colors: SpectrumColors {
                background: Color::from_hex(0x000000),
                bass: Color::from_hex(0x22c55e),
                mids: Color::from_hex(0xeab308),
                treble: Color::from_hex(0xef4444),
            },
            meter_colors: MeterColors {
                background: Color::from_hex(0x1e1e1e),
                normal: Color::from_hex(0x22c55e),
                warning: Color::from_hex(0xf59e0b),
                clip: Color::from_hex(0xdc2626),
                peak: Color::from_hex(0xffffff),
                text: Color::from_hex(0x999999),
            },

            // Additional colors
            peak_indicator: Color::from_hex(0xffffff),
            drag_over_highlight: Color::new(59, 130, 246, 64),
            drag_over_border: Color::from_hex(0x3b82f6),
            neutral_indicator: Color::from_hex(0x6366f1),
            warning_background: Color::new(245, 158, 11, 51),
            knob_color: Color::from_hex(0xffffff),
            optimization_color: Color::from_hex(0x8b5cf6),
            grid_color: Color::new(255, 255, 255, 21),

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: ".SystemUI".to_string(),
            design_language: "neutral".to_string(),
        }
    }

    /// Create a light theme
    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),

            background: Color::from_hex(0xf5f5f5),
            background_secondary: Color::from_hex(0xececec),
            background_tertiary: Color::from_hex(0xe0e0e0),
            surface: Color::from_hex(0xffffff),
            surface_hover: Color::from_hex(0xf0f0f0),
            surface_selected: Color::from_hex(0xd0e8ff),

            text_primary: Color::from_hex(0x1e1e1e),
            text_secondary: Color::from_hex(0x555555),
            text_muted: Color::from_hex(0x888888),
            text_disabled: Color::from_hex(0xbbbbbb),

            border: Color::from_hex(0xd0d0d0),
            border_focused: Color::from_hex(0x0066cc),

            accent: Color::from_hex(0x0066cc),
            accent_hover: Color::from_hex(0x0078e6),
            accent_muted: Color::from_hex(0xb3d4f7),

            text_on_accent: Color::from_hex(0xffffff),
            text_on_accent_muted: Color::new(255, 255, 255, 204),

            success: Color::from_hex(0x16a34a),
            warning: Color::from_hex(0xca8a04),
            error: Color::from_hex(0xdc2626),
            info: Color::from_hex(0x2563eb),

            meter_normal: Color::from_hex(0x16a34a),
            meter_warning: Color::from_hex(0xca8a04),
            meter_clip: Color::from_hex(0xdc2626),

            button_mute_active: Color::from_hex(0xdc2626),
            button_solo_active: Color::from_hex(0xca8a04),
            button_dim_active: Color::from_hex(0x6366f1),

            progress_bar_bg: Color::from_hex(0xd0d0d0),
            progress_bar_fill: Color::from_hex(0x0066cc),

            toast_success_bg: Color::from_hex(0xd1fae5),
            toast_error_bg: Color::from_hex(0xfee2e2),
            toast_info_bg: Color::from_hex(0xdbeafe),
            toast_warning_bg: Color::from_hex(0xfef3c7),

            plugin_colors: PluginColors {
                eq: Color::from_hex(0x2563eb),
                gain: Color::from_hex(0x16a34a),
                upmixer: Color::from_hex(0x7c3aed),
                compressor: Color::from_hex(0xdc2626),
                limiter: Color::from_hex(0xea580c),
                gate: Color::from_hex(0xca8a04),
                loudness: Color::from_hex(0x0891b2),
                binaural: Color::from_hex(0xdb2777),
                convolution: Color::from_hex(0x4f46e5),
                monitor: Color::from_hex(0x14b8a6),
                spectrum: Color::from_hex(0x8b5cf6),
                mute_solo: Color::from_hex(0x6366f1),
            },
            graph_colors: GraphColors {
                input: Color::from_hex(0x6366f1),
                target: Color::from_hex(0x16a34a),
                filter_response: Color::from_hex(0xca8a04),
                corrected: Color::from_hex(0x2563eb),
                error: Color::from_hex(0xdc2626),
                deviation: Color::from_hex(0x7c3aed),
                grid: Color::new(0, 0, 0, 21),
                secondary_line: Color::from_hex(0x888888),
                directivity_er: Color::from_hex(0xec4899),
                directivity_sp: Color::from_hex(0xa855f7),
            },
            band_colors: vec![
                Color::from_hex(0xdc2626),
                Color::from_hex(0xea580c),
                Color::from_hex(0xca8a04),
                Color::from_hex(0x16a34a),
                Color::from_hex(0x0d9488),
                Color::from_hex(0x2563eb),
                Color::from_hex(0x7c3aed),
                Color::from_hex(0xdb2777),
                Color::from_hex(0x4f46e5),
                Color::from_hex(0x0891b2),
                Color::from_hex(0x6b7280),
            ],
            eq_curve_colors: EQCurveColors {
                background: Color::from_hex(0xfafafa),
                grid: Color::new(0, 0, 0, 25),
                curve_boost: Color::from_hex(0x16a34a),
                curve_cut: Color::from_hex(0xdc2626),
                fill_boost: Color::new(22, 163, 74, 64),
                fill_cut: Color::new(220, 38, 38, 64),
                zero_line: Color::new(0, 0, 0, 64),
            },
            spectrum_colors: SpectrumColors {
                background: Color::from_hex(0xfafafa),
                bass: Color::from_hex(0x16a34a),
                mids: Color::from_hex(0xca8a04),
                treble: Color::from_hex(0xdc2626),
            },
            meter_colors: MeterColors {
                background: Color::from_hex(0xf5f5f5),
                normal: Color::from_hex(0x16a34a),
                warning: Color::from_hex(0xca8a04),
                clip: Color::from_hex(0xdc2626),
                peak: Color::from_hex(0x1e1e1e),
                text: Color::from_hex(0x555555),
            },

            peak_indicator: Color::from_hex(0x1e1e1e),
            drag_over_highlight: Color::new(37, 99, 235, 64),
            drag_over_border: Color::from_hex(0x2563eb),
            neutral_indicator: Color::from_hex(0x6366f1),
            warning_background: Color::new(202, 138, 4, 51),
            knob_color: Color::from_hex(0x333333),
            optimization_color: Color::from_hex(0x7c3aed),
            grid_color: Color::new(0, 0, 0, 21),

            separator_size: 20.0,
            font_family: ".SystemUI".to_string(),
            design_language: "neutral".to_string(),
        }
    }

    /// Create a high contrast dark theme
    pub fn high_contrast() -> Self {
        Self {
            name: "High Contrast".to_string(),

            background: Color::from_hex(0x000000),
            background_secondary: Color::from_hex(0x0a0a0a),
            background_tertiary: Color::from_hex(0x141414),
            surface: Color::from_hex(0x1a1a1a),
            surface_hover: Color::from_hex(0x2a2a2a),
            surface_selected: Color::from_hex(0x3a3a3a),

            text_primary: Color::from_hex(0xffffff),
            text_secondary: Color::from_hex(0xdddddd),
            text_muted: Color::from_hex(0x999999),
            text_disabled: Color::from_hex(0x555555),

            border: Color::from_hex(0x555555),
            border_focused: Color::from_hex(0x00ffff),

            accent: Color::from_hex(0x00ffff),
            accent_hover: Color::from_hex(0x33ffff),
            accent_muted: Color::from_hex(0x006666),

            text_on_accent: Color::from_hex(0x000000),
            text_on_accent_muted: Color::new(0, 0, 0, 204),

            success: Color::from_hex(0x00ff00),
            warning: Color::from_hex(0xffff00),
            error: Color::from_hex(0xff0000),
            info: Color::from_hex(0x00aaff),

            meter_normal: Color::from_hex(0x00ff00),
            meter_warning: Color::from_hex(0xffff00),
            meter_clip: Color::from_hex(0xff0000),

            button_mute_active: Color::from_hex(0xff0000),
            button_solo_active: Color::from_hex(0xffff00),
            button_dim_active: Color::from_hex(0x0088ff),

            progress_bar_bg: Color::from_hex(0x333333),
            progress_bar_fill: Color::from_hex(0x00ffff),

            toast_success_bg: Color::from_hex(0x003300),
            toast_error_bg: Color::from_hex(0x330000),
            toast_info_bg: Color::from_hex(0x003333),
            toast_warning_bg: Color::from_hex(0x333300),

            plugin_colors: PluginColors {
                eq: Color::from_hex(0x0088ff),
                gain: Color::from_hex(0x00ff00),
                upmixer: Color::from_hex(0xaa00ff),
                compressor: Color::from_hex(0xff0000),
                limiter: Color::from_hex(0xff6600),
                gate: Color::from_hex(0xffaa00),
                loudness: Color::from_hex(0x00aaff),
                binaural: Color::from_hex(0xff00aa),
                convolution: Color::from_hex(0x6600ff),
                monitor: Color::from_hex(0x00ffaa),
                spectrum: Color::from_hex(0xaa00ff),
                mute_solo: Color::from_hex(0x8888ff),
            },
            graph_colors: GraphColors {
                input: Color::from_hex(0x8888ff),
                target: Color::from_hex(0x00ff00),
                filter_response: Color::from_hex(0xffaa00),
                corrected: Color::from_hex(0x0088ff),
                error: Color::from_hex(0xff0000),
                deviation: Color::from_hex(0xaa00ff),
                grid: Color::new(255, 255, 255, 40),
                secondary_line: Color::from_hex(0xcccccc),
                directivity_er: Color::from_hex(0xff66cc),
                directivity_sp: Color::from_hex(0xcc66ff),
            },
            band_colors: vec![
                Color::from_hex(0xff0000),
                Color::from_hex(0xff6600),
                Color::from_hex(0xffaa00),
                Color::from_hex(0x00ff00),
                Color::from_hex(0x00ffaa),
                Color::from_hex(0x0088ff),
                Color::from_hex(0xaa00ff),
                Color::from_hex(0xff00aa),
                Color::from_hex(0x8888ff),
                Color::from_hex(0x00aaff),
                Color::from_hex(0xcccccc),
            ],
            eq_curve_colors: EQCurveColors {
                background: Color::from_hex(0x000000),
                grid: Color::new(255, 255, 255, 60),
                curve_boost: Color::from_hex(0x00ff00),
                curve_cut: Color::from_hex(0xff0000),
                fill_boost: Color::new(0, 255, 0, 80),
                fill_cut: Color::new(255, 0, 0, 80),
                zero_line: Color::new(255, 255, 255, 100),
            },
            spectrum_colors: SpectrumColors {
                background: Color::from_hex(0x000000),
                bass: Color::from_hex(0x00ff00),
                mids: Color::from_hex(0xffaa00),
                treble: Color::from_hex(0xff0000),
            },
            meter_colors: MeterColors {
                background: Color::from_hex(0x000000),
                normal: Color::from_hex(0x00ff00),
                warning: Color::from_hex(0xffff00),
                clip: Color::from_hex(0xff0000),
                peak: Color::from_hex(0xffffff),
                text: Color::from_hex(0xdddddd),
            },

            peak_indicator: Color::from_hex(0xffffff),
            drag_over_highlight: Color::new(0, 255, 255, 80),
            drag_over_border: Color::from_hex(0x00ffff),
            neutral_indicator: Color::from_hex(0x8888ff),
            warning_background: Color::new(255, 255, 0, 60),
            knob_color: Color::from_hex(0xffffff),
            optimization_color: Color::from_hex(0xaa00ff),
            grid_color: Color::new(255, 255, 255, 40),

            separator_size: 20.0,
            font_family: ".SystemUI".to_string(),
            design_language: "neutral".to_string(),
        }
    }

    /// Create a dark palette tuned for protanopia-safe semantic separation.
    pub fn protanopia() -> Self {
        Self::dark().with_accessible_semantics(
            "Protanopia",
            Color::from_hex(0x0072b2),
            Color::from_hex(0x009e73),
            Color::from_hex(0xe69f00),
            Color::from_hex(0xcc79a7),
            Color::from_hex(0x56b4e9),
            Color::from_hex(0xf0e442),
        )
    }

    /// Create a dark palette tuned for deuteranopia-safe semantic separation.
    pub fn deuteranopia() -> Self {
        Self::dark().with_accessible_semantics(
            "Deuteranopia",
            Color::from_hex(0x0072b2),
            Color::from_hex(0x56b4e9),
            Color::from_hex(0xe69f00),
            Color::from_hex(0xd55e00),
            Color::from_hex(0xcc79a7),
            Color::from_hex(0xf0e442),
        )
    }

    /// Create a dark palette tuned for tritanopia-safe semantic separation.
    pub fn tritanopia() -> Self {
        Self::dark().with_accessible_semantics(
            "Tritanopia",
            Color::from_hex(0xcc79a7),
            Color::from_hex(0x009e73),
            Color::from_hex(0xd55e00),
            Color::from_hex(0xe64b35),
            Color::from_hex(0x999999),
            Color::from_hex(0x0072b2),
        )
    }

    /// Create a Nord theme
    pub fn nord() -> Self {
        Self {
            name: "Nord".to_string(),

            background: Color::from_hex(0x2e3440),
            background_secondary: Color::from_hex(0x3b4252),
            background_tertiary: Color::from_hex(0x434c5e),
            surface: Color::from_hex(0x3b4252),
            surface_hover: Color::from_hex(0x434c5e),
            surface_selected: Color::from_hex(0x4c566a),

            text_primary: Color::from_hex(0xeceff4),
            text_secondary: Color::from_hex(0xd8dee9),
            text_muted: Color::from_hex(0x81a1c1),
            text_disabled: Color::from_hex(0x4c566a),

            border: Color::from_hex(0x4c566a),
            border_focused: Color::from_hex(0x88c0d0),

            accent: Color::from_hex(0x88c0d0),
            accent_hover: Color::from_hex(0x8fbcbb),
            accent_muted: Color::from_hex(0x5e81ac),

            text_on_accent: Color::from_hex(0x2e3440),
            text_on_accent_muted: Color::new(46, 52, 64, 204),

            success: Color::from_hex(0xa3be8c),
            warning: Color::from_hex(0xebcb8b),
            error: Color::from_hex(0xbf616a),
            info: Color::from_hex(0x81a1c1),

            meter_normal: Color::from_hex(0xa3be8c),
            meter_warning: Color::from_hex(0xebcb8b),
            meter_clip: Color::from_hex(0xbf616a),

            button_mute_active: Color::from_hex(0xbf616a),
            button_solo_active: Color::from_hex(0xebcb8b),
            button_dim_active: Color::from_hex(0x5e81ac),

            progress_bar_bg: Color::from_hex(0x4c566a),
            progress_bar_fill: Color::from_hex(0x88c0d0),

            toast_success_bg: Color::from_hex(0x3e4f41),
            toast_error_bg: Color::from_hex(0x4a3638),
            toast_info_bg: Color::from_hex(0x38445a),
            toast_warning_bg: Color::from_hex(0x4c4639),

            plugin_colors: PluginColors {
                eq: Color::from_hex(0x5e81ac),
                gain: Color::from_hex(0xa3be8c),
                upmixer: Color::from_hex(0xb48ead),
                compressor: Color::from_hex(0xbf616a),
                limiter: Color::from_hex(0xd08770),
                gate: Color::from_hex(0xebcb8b),
                loudness: Color::from_hex(0x88c0d0),
                binaural: Color::from_hex(0xb48ead),
                convolution: Color::from_hex(0x81a1c1),
                monitor: Color::from_hex(0x8fbcbb),
                spectrum: Color::from_hex(0xb48ead),
                mute_solo: Color::from_hex(0x5e81ac),
            },
            graph_colors: GraphColors {
                input: Color::from_hex(0x5e81ac),
                target: Color::from_hex(0xa3be8c),
                filter_response: Color::from_hex(0xebcb8b),
                corrected: Color::from_hex(0x81a1c1),
                error: Color::from_hex(0xbf616a),
                deviation: Color::from_hex(0xb48ead),
                grid: Color::new(216, 222, 233, 30),
                secondary_line: Color::from_hex(0xd8dee9),
                directivity_er: Color::from_hex(0xb48ead),
                directivity_sp: Color::from_hex(0x81a1c1),
            },
            band_colors: vec![
                Color::from_hex(0xbf616a),
                Color::from_hex(0xd08770),
                Color::from_hex(0xebcb8b),
                Color::from_hex(0xa3be8c),
                Color::from_hex(0x8fbcbb),
                Color::from_hex(0x88c0d0),
                Color::from_hex(0x81a1c1),
                Color::from_hex(0x5e81ac),
                Color::from_hex(0xb48ead),
                Color::from_hex(0x88c0d0),
                Color::from_hex(0x4c566a),
            ],
            eq_curve_colors: EQCurveColors {
                background: Color::from_hex(0x2e3440),
                grid: Color::new(216, 222, 233, 40),
                curve_boost: Color::from_hex(0xa3be8c),
                curve_cut: Color::from_hex(0xbf616a),
                fill_boost: Color::new(163, 190, 140, 64),
                fill_cut: Color::new(191, 97, 106, 64),
                zero_line: Color::new(216, 222, 233, 80),
            },
            spectrum_colors: SpectrumColors {
                background: Color::from_hex(0x2e3440),
                bass: Color::from_hex(0xa3be8c),
                mids: Color::from_hex(0xebcb8b),
                treble: Color::from_hex(0xbf616a),
            },
            meter_colors: MeterColors {
                background: Color::from_hex(0x2e3440),
                normal: Color::from_hex(0xa3be8c),
                warning: Color::from_hex(0xebcb8b),
                clip: Color::from_hex(0xbf616a),
                peak: Color::from_hex(0xeceff4),
                text: Color::from_hex(0xd8dee9),
            },

            peak_indicator: Color::from_hex(0xeceff4),
            drag_over_highlight: Color::new(136, 192, 208, 64),
            drag_over_border: Color::from_hex(0x88c0d0),
            neutral_indicator: Color::from_hex(0x5e81ac),
            warning_background: Color::new(235, 203, 139, 51),
            knob_color: Color::from_hex(0xeceff4),
            optimization_color: Color::from_hex(0xb48ead),
            grid_color: Color::new(216, 222, 233, 30),

            separator_size: 20.0,
            font_family: ".SystemUI".to_string(),
            design_language: "neutral".to_string(),
        }
    }

    /// Create a Dracula theme
    pub fn dracula() -> Self {
        Self {
            name: "Dracula".to_string(),

            background: Color::from_hex(0x282a36),
            background_secondary: Color::from_hex(0x21222c),
            background_tertiary: Color::from_hex(0x191a21),
            surface: Color::from_hex(0x44475a),
            surface_hover: Color::from_hex(0x6272a4),
            surface_selected: Color::from_hex(0x6272a4),

            text_primary: Color::from_hex(0xf8f8f2),
            text_secondary: Color::from_hex(0xbfbfbf),
            text_muted: Color::from_hex(0x6272a4),
            text_disabled: Color::from_hex(0x44475a),

            border: Color::from_hex(0x44475a),
            border_focused: Color::from_hex(0xbd93f9),

            accent: Color::from_hex(0xbd93f9),
            accent_hover: Color::from_hex(0xff79c6),
            accent_muted: Color::from_hex(0x6272a4),

            text_on_accent: Color::from_hex(0x282a36),
            text_on_accent_muted: Color::new(40, 42, 54, 204),

            success: Color::from_hex(0x50fa7b),
            warning: Color::from_hex(0xf1fa8c),
            error: Color::from_hex(0xff5555),
            info: Color::from_hex(0x8be9fd),

            meter_normal: Color::from_hex(0x50fa7b),
            meter_warning: Color::from_hex(0xf1fa8c),
            meter_clip: Color::from_hex(0xff5555),

            button_mute_active: Color::from_hex(0xff5555),
            button_solo_active: Color::from_hex(0xf1fa8c),
            button_dim_active: Color::from_hex(0xbd93f9),

            progress_bar_bg: Color::from_hex(0x44475a),
            progress_bar_fill: Color::from_hex(0xbd93f9),

            toast_success_bg: Color::from_hex(0x1e3a26),
            toast_error_bg: Color::from_hex(0x3a1e1e),
            toast_info_bg: Color::from_hex(0x1e2f3a),
            toast_warning_bg: Color::from_hex(0x3a3a1e),

            plugin_colors: PluginColors {
                eq: Color::from_hex(0x8be9fd),
                gain: Color::from_hex(0x50fa7b),
                upmixer: Color::from_hex(0xbd93f9),
                compressor: Color::from_hex(0xff5555),
                limiter: Color::from_hex(0xffb86c),
                gate: Color::from_hex(0xf1fa8c),
                loudness: Color::from_hex(0x8be9fd),
                binaural: Color::from_hex(0xff79c6),
                convolution: Color::from_hex(0xbd93f9),
                monitor: Color::from_hex(0x50fa7b),
                spectrum: Color::from_hex(0xbd93f9),
                mute_solo: Color::from_hex(0x6272a4),
            },
            graph_colors: GraphColors {
                input: Color::from_hex(0x6272a4),
                target: Color::from_hex(0x50fa7b),
                filter_response: Color::from_hex(0xf1fa8c),
                corrected: Color::from_hex(0x8be9fd),
                error: Color::from_hex(0xff5555),
                deviation: Color::from_hex(0xbd93f9),
                grid: Color::new(248, 248, 242, 25),
                secondary_line: Color::from_hex(0xbfbfbf),
                directivity_er: Color::from_hex(0xff79c6),
                directivity_sp: Color::from_hex(0xbd93f9),
            },
            band_colors: vec![
                Color::from_hex(0xff5555),
                Color::from_hex(0xffb86c),
                Color::from_hex(0xf1fa8c),
                Color::from_hex(0x50fa7b),
                Color::from_hex(0x8be9fd),
                Color::from_hex(0xbd93f9),
                Color::from_hex(0xff79c6),
                Color::from_hex(0x6272a4),
                Color::from_hex(0xbd93f9),
                Color::from_hex(0x8be9fd),
                Color::from_hex(0x44475a),
            ],
            eq_curve_colors: EQCurveColors {
                background: Color::from_hex(0x282a36),
                grid: Color::new(248, 248, 242, 35),
                curve_boost: Color::from_hex(0x50fa7b),
                curve_cut: Color::from_hex(0xff5555),
                fill_boost: Color::new(80, 250, 123, 64),
                fill_cut: Color::new(255, 85, 85, 64),
                zero_line: Color::new(248, 248, 242, 80),
            },
            spectrum_colors: SpectrumColors {
                background: Color::from_hex(0x282a36),
                bass: Color::from_hex(0x50fa7b),
                mids: Color::from_hex(0xf1fa8c),
                treble: Color::from_hex(0xff5555),
            },
            meter_colors: MeterColors {
                background: Color::from_hex(0x282a36),
                normal: Color::from_hex(0x50fa7b),
                warning: Color::from_hex(0xf1fa8c),
                clip: Color::from_hex(0xff5555),
                peak: Color::from_hex(0xf8f8f2),
                text: Color::from_hex(0xbfbfbf),
            },

            peak_indicator: Color::from_hex(0xf8f8f2),
            drag_over_highlight: Color::new(189, 147, 249, 64),
            drag_over_border: Color::from_hex(0xbd93f9),
            neutral_indicator: Color::from_hex(0x6272a4),
            warning_background: Color::new(241, 250, 140, 51),
            knob_color: Color::from_hex(0xf8f8f2),
            optimization_color: Color::from_hex(0xbd93f9),
            grid_color: Color::new(248, 248, 242, 25),

            separator_size: 20.0,
            font_family: ".SystemUI".to_string(),
            design_language: "neutral".to_string(),
        }
    }

    /// Save theme to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Load theme from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Generate Rust code for this theme
    pub fn to_rust_code(&self) -> String {
        fn color_to_rust(c: &Color) -> String {
            if c.a == 255 {
                format!("Color::from_hex(0x{:02x}{:02x}{:02x})", c.r, c.g, c.b)
            } else {
                format!("Color::new({}, {}, {}, {})", c.r, c.g, c.b, c.a)
            }
        }

        let code = format!(
            r#"/// {} theme
pub fn {}() -> EditorTheme {{
    EditorTheme {{
        name: "{}".to_string(),

        // Base colors
        background: {},
        background_secondary: {},
        background_tertiary: {},
        surface: {},
        surface_hover: {},
        surface_selected: {},

        // Text colors
        text_primary: {},
        text_secondary: {},
        text_muted: {},
        text_disabled: {},

        // Border colors
        border: {},
        border_focused: {},

        // Accent colors
        accent: {},
        accent_hover: {},
        accent_muted: {},

        // Text on accent
        text_on_accent: {},
        text_on_accent_muted: {},

        // Semantic colors
        success: {},
        warning: {},
        error: {},
        info: {},

        // Level meter colors
        meter_normal: {},
        meter_warning: {},
        meter_clip: {},

        // Button colors
        button_mute_active: {},
        button_solo_active: {},
        button_dim_active: {},

        // Playback bar
        progress_bar_bg: {},
        progress_bar_fill: {},

        // Toast backgrounds
        toast_success_bg: {},
        toast_error_bg: {},
        toast_info_bg: {},
        toast_warning_bg: {},

        // Additional colors
        peak_indicator: {},
        drag_over_highlight: {},
        drag_over_border: {},
        neutral_indicator: {},
        warning_background: {},
        knob_color: {},
        optimization_color: {},
        grid_color: {},

        separator_size: {:.1},
        font_family: "{}".to_string(),
        design_language: "{}".to_string(),

        plugin_colors: PluginColors {{
            eq: {},
            gain: {},
            upmixer: {},
            compressor: {},
            limiter: {},
            gate: {},
            loudness: {},
            binaural: {},
            convolution: {},
            monitor: {},
            spectrum: {},
            mute_solo: {},
        }},
        graph_colors: GraphColors {{
            input: {},
            target: {},
            filter_response: {},
            corrected: {},
            error: {},
            deviation: {},
            grid: {},
            secondary_line: {},
            directivity_er: {},
            directivity_sp: {},
        }},
        band_colors: vec![
{}
        ],
        eq_curve_colors: EQCurveColors {{
            background: {},
            grid: {},
            curve_boost: {},
            curve_cut: {},
            fill_boost: {},
            fill_cut: {},
            zero_line: {},
        }},
        spectrum_colors: SpectrumColors {{
            background: {},
            bass: {},
            mids: {},
            treble: {},
        }},
        meter_colors: MeterColors {{
            background: {},
            normal: {},
            warning: {},
            clip: {},
            peak: {},
            text: {},
        }},
    }}
}}
"#,
            self.name,
            self.name.to_lowercase().replace(' ', "_"),
            self.name,
            color_to_rust(&self.background),
            color_to_rust(&self.background_secondary),
            color_to_rust(&self.background_tertiary),
            color_to_rust(&self.surface),
            color_to_rust(&self.surface_hover),
            color_to_rust(&self.surface_selected),
            color_to_rust(&self.text_primary),
            color_to_rust(&self.text_secondary),
            color_to_rust(&self.text_muted),
            color_to_rust(&self.text_disabled),
            color_to_rust(&self.border),
            color_to_rust(&self.border_focused),
            color_to_rust(&self.accent),
            color_to_rust(&self.accent_hover),
            color_to_rust(&self.accent_muted),
            color_to_rust(&self.text_on_accent),
            color_to_rust(&self.text_on_accent_muted),
            color_to_rust(&self.success),
            color_to_rust(&self.warning),
            color_to_rust(&self.error),
            color_to_rust(&self.info),
            color_to_rust(&self.meter_normal),
            color_to_rust(&self.meter_warning),
            color_to_rust(&self.meter_clip),
            color_to_rust(&self.button_mute_active),
            color_to_rust(&self.button_solo_active),
            color_to_rust(&self.button_dim_active),
            color_to_rust(&self.progress_bar_bg),
            color_to_rust(&self.progress_bar_fill),
            color_to_rust(&self.toast_success_bg),
            color_to_rust(&self.toast_error_bg),
            color_to_rust(&self.toast_info_bg),
            color_to_rust(&self.toast_warning_bg),
            color_to_rust(&self.peak_indicator),
            color_to_rust(&self.drag_over_highlight),
            color_to_rust(&self.drag_over_border),
            color_to_rust(&self.neutral_indicator),
            color_to_rust(&self.warning_background),
            color_to_rust(&self.knob_color),
            color_to_rust(&self.optimization_color),
            color_to_rust(&self.grid_color),
            self.separator_size,
            self.font_family,
            self.design_language,
            color_to_rust(&self.plugin_colors.eq),
            color_to_rust(&self.plugin_colors.gain),
            color_to_rust(&self.plugin_colors.upmixer),
            color_to_rust(&self.plugin_colors.compressor),
            color_to_rust(&self.plugin_colors.limiter),
            color_to_rust(&self.plugin_colors.gate),
            color_to_rust(&self.plugin_colors.loudness),
            color_to_rust(&self.plugin_colors.binaural),
            color_to_rust(&self.plugin_colors.convolution),
            color_to_rust(&self.plugin_colors.monitor),
            color_to_rust(&self.plugin_colors.spectrum),
            color_to_rust(&self.plugin_colors.mute_solo),
            color_to_rust(&self.graph_colors.input),
            color_to_rust(&self.graph_colors.target),
            color_to_rust(&self.graph_colors.filter_response),
            color_to_rust(&self.graph_colors.corrected),
            color_to_rust(&self.graph_colors.error),
            color_to_rust(&self.graph_colors.deviation),
            color_to_rust(&self.graph_colors.grid),
            color_to_rust(&self.graph_colors.secondary_line),
            color_to_rust(&self.graph_colors.directivity_er),
            color_to_rust(&self.graph_colors.directivity_sp),
            self.band_colors
                .iter()
                .map(|c| format!("            {},", color_to_rust(c)))
                .collect::<Vec<_>>()
                .join("\n"),
            color_to_rust(&self.eq_curve_colors.background),
            color_to_rust(&self.eq_curve_colors.grid),
            color_to_rust(&self.eq_curve_colors.curve_boost),
            color_to_rust(&self.eq_curve_colors.curve_cut),
            color_to_rust(&self.eq_curve_colors.fill_boost),
            color_to_rust(&self.eq_curve_colors.fill_cut),
            color_to_rust(&self.eq_curve_colors.zero_line),
            color_to_rust(&self.spectrum_colors.background),
            color_to_rust(&self.spectrum_colors.bass),
            color_to_rust(&self.spectrum_colors.mids),
            color_to_rust(&self.spectrum_colors.treble),
            color_to_rust(&self.meter_colors.background),
            color_to_rust(&self.meter_colors.normal),
            color_to_rust(&self.meter_colors.warning),
            color_to_rust(&self.meter_colors.clip),
            color_to_rust(&self.meter_colors.peak),
            color_to_rust(&self.meter_colors.text),
        );

        code
    }

    /// Validate theme invariants.
    pub fn validate(&self) -> Result<(), String> {
        if self.band_colors.is_empty() {
            return Err("band_colors must not be empty".to_string());
        }
        Ok(())
    }

    /// Create a GPUI-compatible button theme
    pub fn to_button_theme(&self) -> gpui_ui_kit::ButtonTheme {
        let mut error_hover = self.error.to_rgba();
        error_hover.a = 0.8;

        gpui_ui_kit::ButtonTheme {
            accent: self.accent.to_rgba(),
            accent_hover: self.accent_hover.to_rgba(),
            surface: self.surface.to_rgba(),
            surface_hover: self.surface_hover.to_rgba(),
            text_primary: self.text_primary.to_rgba(),
            text_secondary: self.text_secondary.to_rgba(),
            text_on_accent: self.text_on_accent.to_rgba(),
            error: self.error.to_rgba(),
            error_hover,
            border: self.border.to_rgba(),
            transparent: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        }
    }

    /// Create a GPUI-compatible slider theme
    pub fn to_slider_theme(&self) -> gpui_ui_kit::SliderTheme {
        let mut disabled_label = self.text_disabled.to_rgba();
        disabled_label.a = 0.5;

        gpui_ui_kit::SliderTheme {
            track: self.surface_hover.to_rgba(),
            fill: self.accent.to_rgba(),
            thumb: self.text_primary.to_rgba(),
            thumb_hover: self.text_secondary.to_rgba(),
            thumb_active: self.accent.to_rgba(),
            label: self.text_primary.to_rgba(),
            value: self.text_secondary.to_rgba(),
            disabled_label,
            disabled_fill: self.text_disabled.to_rgba(),
        }
    }

    /// Create a GPUI-compatible accordion theme
    pub fn to_accordion_theme(&self) -> gpui_ui_kit::AccordionTheme {
        gpui_ui_kit::AccordionTheme {
            header_bg: self.surface.to_rgba(),
            header_hover_bg: self.surface_hover.to_rgba(),
            header_active_bg: self.accent_muted.to_rgba(),
            content_bg: self.background.to_rgba(),
            border: self.border.to_rgba(),
            accent_tint: self.accent_muted.to_rgba(),
            accent: self.accent.to_rgba(),
            title_color: self.text_primary.to_rgba(),
            indicator_color: self.text_muted.to_rgba(),
        }
    }

    /// Create a GPUI-compatible tabs theme
    pub fn to_tabs_theme(&self) -> gpui_ui_kit::TabsTheme {
        gpui_ui_kit::TabsTheme {
            container_bg: self.surface.to_rgba(),
            container_border: self.border.to_rgba(),
            selected_bg: self.surface_selected.to_rgba(),
            selected_hover_bg: self.surface_hover.to_rgba(),
            hover_bg: self.surface_hover.to_rgba(),
            accent: self.accent.to_rgba(),
            text_selected: self.text_primary.to_rgba(),
            text_unselected: self.text_secondary.to_rgba(),
            text_hover: self.text_primary.to_rgba(),
            badge_bg: self.surface_hover.to_rgba(),
            close_color: self.text_muted.to_rgba(),
            close_hover_color: self.text_primary.to_rgba(),
            icon_selected: None,
            icon_unselected: None,
        }
    }
}

/// Color group for organizing theme editor UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorGroup {
    Base,
    Text,
    Border,
    Accent,
    Semantic,
    Meter,
    Button,
    Progress,
    Toast,
    Plugin,
    Graph,
    Spectrum,
    Additional,
}

impl ColorGroup {
    pub fn all() -> &'static [ColorGroup] {
        &[
            ColorGroup::Base,
            ColorGroup::Text,
            ColorGroup::Border,
            ColorGroup::Accent,
            ColorGroup::Semantic,
            ColorGroup::Meter,
            ColorGroup::Button,
            ColorGroup::Progress,
            ColorGroup::Toast,
            ColorGroup::Plugin,
            ColorGroup::Graph,
            ColorGroup::Spectrum,
            ColorGroup::Additional,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ColorGroup::Base => "Base Colors",
            ColorGroup::Text => "Text Colors",
            ColorGroup::Border => "Border Colors",
            ColorGroup::Accent => "Accent Colors",
            ColorGroup::Semantic => "Semantic Colors",
            ColorGroup::Meter => "Level Meters",
            ColorGroup::Button => "Button States",
            ColorGroup::Progress => "Progress Bar",
            ColorGroup::Toast => "Toast Notifications",
            ColorGroup::Plugin => "Plugin Colors",
            ColorGroup::Graph => "Graph Colors",
            ColorGroup::Spectrum => "Spectrum Colors",
            ColorGroup::Additional => "Additional",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rgba_eq(actual: gpui::Rgba, expected: gpui::Rgba) {
        assert!((actual.r - expected.r).abs() <= f32::EPSILON);
        assert!((actual.g - expected.g).abs() <= f32::EPSILON);
        assert!((actual.b - expected.b).abs() <= f32::EPSILON);
        assert!((actual.a - expected.a).abs() <= f32::EPSILON);
    }

    #[test]
    fn test_theme_json_roundtrip() {
        let theme = EditorTheme::dark();
        let json = theme.to_json().unwrap();
        let loaded = EditorTheme::from_json(&json).unwrap();
        assert_eq!(loaded.name, theme.name);
        assert_eq!(loaded.background.r, theme.background.r);
    }

    #[test]
    fn test_to_rust_code_includes_nested_structs() {
        let theme = EditorTheme::dark();
        let code = theme.to_rust_code();
        // Should contain nested struct initializations, not abbreviated comment.
        assert!(
            code.contains("PluginColors {"),
            "Rust code should include PluginColors initialization"
        );
        assert!(
            code.contains("GraphColors {"),
            "Rust code should include GraphColors initialization"
        );
        assert!(
            code.contains("EQCurveColors {"),
            "Rust code should include EQCurveColors initialization"
        );
        assert!(
            code.contains("SpectrumColors {"),
            "Rust code should include SpectrumColors initialization"
        );
        assert!(
            code.contains("MeterColors {"),
            "Rust code should include MeterColors initialization"
        );
        assert!(
            !code.contains("plugin_colors, graph_colors, etc."),
            "Rust code should not contain abbreviated placeholder"
        );
    }

    #[test]
    fn test_to_accordion_theme_maps_accent_fields() {
        let theme = EditorTheme::dark();
        let accordion = theme.to_accordion_theme();

        assert_rgba_eq(accordion.header_active_bg, theme.accent_muted.to_rgba());
        assert_rgba_eq(accordion.accent_tint, theme.accent_muted.to_rgba());
        assert_rgba_eq(accordion.accent, theme.accent.to_rgba());
    }

    #[test]
    fn test_validate_band_colors_non_empty() {
        let mut theme = EditorTheme::dark();
        assert!(theme.validate().is_ok());
        theme.band_colors.clear();
        assert!(theme.validate().is_err());
    }

    #[test]
    fn test_theme_schedule_resolves_day_and_night() {
        let schedule = ThemeSchedule::new(TimeOfDay::new(7, 30), TimeOfDay::new(18, 0));

        assert_eq!(schedule.resolve_at_minutes(8 * 60), ThemeAppearance::Light);
        assert_eq!(schedule.resolve_at_minutes(20 * 60), ThemeAppearance::Dark);
        assert_eq!(
            ThemeModePreference::Scheduled { schedule }.resolve(ThemeAppearance::Light, 23 * 60),
            ThemeAppearance::Dark
        );
    }

    #[test]
    fn test_theme_schedule_supports_wraparound_light_period() {
        let schedule = ThemeSchedule::new(TimeOfDay::new(18, 0), TimeOfDay::new(7, 0));

        assert_eq!(schedule.resolve_at_minutes(23 * 60), ThemeAppearance::Light);
        assert_eq!(schedule.resolve_at_minutes(12 * 60), ThemeAppearance::Dark);
    }

    #[test]
    fn test_accent_palette_applies_readable_text() {
        let seed = Color::from_hex(0xf0e442);
        let palette = AccentPalette::from_seed(seed, AccentSource::System, ThemeAppearance::Dark);
        let theme = EditorTheme::dark().with_accent_palette(palette);

        assert_eq!(theme.accent, seed);
        assert_eq!(theme.border_focused, seed);
        assert!(contrast_ratio(theme.text_on_accent, theme.accent) >= 4.5);
    }

    #[test]
    fn test_color_blind_presets_are_accessible() {
        for preset in [
            BuiltInThemePreset::Protanopia,
            BuiltInThemePreset::Deuteranopia,
            BuiltInThemePreset::Tritanopia,
        ] {
            let theme = preset.to_theme();
            assert!(preset.accessibility().is_color_blind_safe());
            assert!(
                theme.validate_accessibility().is_ok(),
                "{} should meet core contrast requirements",
                preset.name()
            );
        }
    }

    #[test]
    fn test_community_theme_bundle_roundtrip() {
        let mut manifest = CommunityThemeManifest::for_theme(&EditorTheme::dracula());
        manifest.author = "SOTF".to_string();
        manifest.tags = vec!["community".to_string(), "dark".to_string()];
        manifest.accessibility = AccessibilityPalette::Standard;

        let bundle = CommunityThemeBundle::new(manifest, EditorTheme::dracula());
        let json = bundle.to_json().unwrap();
        let loaded = CommunityThemeBundle::from_json(&json).unwrap();

        assert_eq!(
            loaded.manifest.schema_version,
            COMMUNITY_THEME_SCHEMA_VERSION
        );
        assert_eq!(loaded.manifest.id, "dracula");
        assert_eq!(loaded.manifest.author, "SOTF");
        assert_eq!(loaded.theme.name, "Dracula");
        assert!(loaded.validate().is_ok());
    }

    #[test]
    fn test_transition_respects_reduce_motion() {
        let transition = ThemeTransition::default();

        assert_eq!(transition.effective_duration_ms(false), 220);
        assert_eq!(transition.effective_duration_ms(true), 0);
        assert_eq!(ThemeTransition::disabled().effective_duration_ms(false), 0);
    }

    #[test]
    fn test_builtin_preset_lookup_accepts_friendly_ids() {
        assert_eq!(
            BuiltInThemePreset::from_id("High Contrast"),
            Some(BuiltInThemePreset::HighContrast)
        );
        assert_eq!(BuiltInThemePreset::from_id("tokyo-night"), None);
    }
}
