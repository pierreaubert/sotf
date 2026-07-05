use super::misc::with_alpha;
use gpui::*;

/// Colour palette used by both 2D and 3D renderers. Keeping it in one
/// struct means the plugin UIs can tint a single value (e.g. the polygon
/// fill) and inherit the rest.
#[derive(Debug, Clone)]
pub struct SpiderColors {
    pub background: Rgba,
    pub grid: Rgba,
    pub polygon_fill: Rgba,
    pub polygon_stroke: Rgba,
    pub speaker_dot: Rgba,
    pub label: Rgba,
    /// Tint for vertices with negative signed value (e.g. anti-phase
    /// correlation). Renderer interpolates between `polygon_stroke` and
    /// this colour by `|signed_value|` to flag anti-phase channels.
    pub negative_value: Rgba,
}

impl Default for SpiderColors {
    fn default() -> Self {
        let theme = crate::theme::Theme::dark();
        Self::from_theme(&theme)
    }
}

impl SpiderColors {
    /// Build a spider palette that flows from the active `Theme`. Use this
    /// instead of `default()` whenever a `Theme` is in scope so light themes
    /// don't show a jarring dark patch.
    ///
    /// - `background` follows `theme.surface` (one step down from the panel).
    /// - `grid` follows `theme.border` — same hairlines as other charts.
    /// - `polygon_fill` is a translucent rendering of `theme.accent`.
    /// - `polygon_stroke` and labels use the same accent / text colors as
    ///   the rest of the panel for visual continuity.
    /// - `negative_value` (anti-phase tint) uses `theme.error` so it reads
    ///   as "alarm" without clashing with the rest of the palette.
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            background: theme.surface,
            grid: theme.border,
            polygon_fill: with_alpha(theme.accent, 0.25),
            polygon_stroke: theme.accent,
            speaker_dot: theme.text_primary,
            label: theme.text_secondary,
            negative_value: theme.error,
        }
    }
}
