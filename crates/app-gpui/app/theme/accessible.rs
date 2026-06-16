use super::{GraphLineColors, PluginColorMap, Theme, rgb};
use gpui::Rgba;

pub const WCAG_AA_TEXT_CONTRAST: f32 = 4.5;
pub const WCAG_NON_TEXT_CONTRAST: f32 = 3.0;

impl Theme {
    /// Protanopia-safe dark theme using blue, yellow, teal, and purple separation.
    pub fn protanopia() -> Self {
        Self::dark().with_accessible_semantics(
            rgb(0x008fd5),
            rgb(0x009e73),
            rgb(0xe69f00),
            rgb(0xcc79a7),
            rgb(0x56b4e9),
            rgb(0xf0e442),
        )
    }

    /// Deuteranopia-safe dark theme using blue/orange/pink separation.
    pub fn deuteranopia() -> Self {
        Self::dark().with_accessible_semantics(
            rgb(0x008fd5),
            rgb(0x56b4e9),
            rgb(0xe69f00),
            rgb(0xd55e00),
            rgb(0xcc79a7),
            rgb(0xf0e442),
        )
    }

    /// Tritanopia-safe dark theme using magenta, green, orange, and blue separation.
    pub fn tritanopia() -> Self {
        Self::dark().with_accessible_semantics(
            rgb(0xcc79a7),
            rgb(0x009e73),
            rgb(0xd55e00),
            rgb(0xe64b35),
            rgb(0x999999),
            rgb(0x0072b2),
        )
    }

    fn with_accessible_semantics(
        mut self,
        accent: Rgba,
        success: Rgba,
        warning: Rgba,
        error: Rgba,
        info: Rgba,
        secondary: Rgba,
    ) -> Self {
        let accent_muted = Self::with_opacity(accent, 0.35);
        self.surface_selected = accent_muted;
        self.border_focused = accent;
        self.accent = accent;
        self.accent_hover = lighten(accent, 0.12);
        self.accent_muted = accent_muted;
        self.text_on_accent = readable_text_color(accent);
        self.text_on_accent_muted = Self::with_opacity(self.text_on_accent, 0.8);
        self.icon_on_accent = self.text_on_accent;

        self.success = success;
        self.warning = warning;
        self.error = error;
        self.info = info;
        self.feedback.meter_normal = success;
        self.feedback.meter_warning = warning;
        self.feedback.meter_clip = error;
        self.button_mute_active = error;
        self.button_solo_active = warning;
        self.button_dim_active = secondary;
        self.feedback.progress_bar_fill = accent;
        self.feedback.toast_success_bg = Self::with_opacity(success, 0.22);
        self.feedback.toast_error_bg = Self::with_opacity(error, 0.22);
        self.feedback.toast_info_bg = Self::with_opacity(info, 0.22);
        self.feedback.toast_warning_bg = Self::with_opacity(warning, 0.22);

        self.plugin_palette.plugin_colors = PluginColorMap {
            eq: accent,
            gain: success,
            upmixer: secondary,
            compressor: error,
            limiter: warning,
            gate: secondary,
            loudness: info,
            binaural: rgb(0xcc79a7),
            convolution: accent,
            monitor: success,
            spectrum: secondary,
            mute_solo: info,
        };
        self.plugin_palette.graph_colors = GraphLineColors {
            input: info,
            target: success,
            filter_response: warning,
            corrected: accent,
            error,
            deviation: secondary,
            grid: self.feedback.grid_color,
            secondary_line: self.text_secondary,
            directivity_er: rgb(0xcc79a7),
            directivity_sp: secondary,
        };
        self.plugin_palette.band_colors = vec![
            error,
            warning,
            success,
            info,
            accent,
            secondary,
            rgb(0xcc79a7),
            rgb(0x999999),
        ];
        self.plugin_palette.channel_colors = vec![info, error, success, warning, secondary, accent];

        self.plugin_palette.eq_curve_colors.curve_boost = success;
        self.plugin_palette.eq_curve_colors.curve_cut = error;
        self.plugin_palette.eq_curve_colors.fill_boost = Self::with_opacity(success, 0.28);
        self.plugin_palette.eq_curve_colors.fill_cut = Self::with_opacity(error, 0.28);
        self.plugin_palette.spectrum_colors.bass = success;
        self.plugin_palette.spectrum_colors.mids = warning;
        self.plugin_palette.spectrum_colors.treble = error;
        self.plugin_palette.meter_colors.normal = success;
        self.plugin_palette.meter_colors.warning = warning;
        self.plugin_palette.meter_colors.clip = error;
        self.feedback.drag_over_highlight = Self::with_opacity(accent, 0.25);
        self.feedback.drag_over_border = accent;
        self.feedback.neutral_indicator = accent;
        self.feedback.warning_background = Self::with_opacity(warning, 0.2);
        self.feedback.optimization_color = secondary;
        self
    }

    /// Validate the core color pairs that carry readable UI state.
    ///
    /// This intentionally avoids decorative pairs such as hairline borders.
    /// Selection backgrounds are composited over the surface first because many
    /// accent-muted colors are transparent overlays.
    pub fn validate_accessibility(&self) -> Result<(), String> {
        validate_pair(
            "text_primary/background",
            self.text_primary,
            self.background,
            WCAG_AA_TEXT_CONTRAST,
        )?;
        validate_pair(
            "text_primary/surface",
            self.text_primary,
            self.surface,
            WCAG_AA_TEXT_CONTRAST,
        )?;
        validate_pair(
            "text_secondary/surface",
            self.text_secondary,
            self.surface,
            WCAG_NON_TEXT_CONTRAST,
        )?;
        validate_pair(
            "text_on_accent/accent",
            self.text_on_accent,
            self.accent,
            WCAG_AA_TEXT_CONTRAST,
        )?;

        let selected_surface = composite_over(self.surface_selected, self.surface);
        validate_pair(
            "text_primary/surface_selected",
            self.text_primary,
            selected_surface,
            WCAG_AA_TEXT_CONTRAST,
        )?;
        validate_pair(
            "accent/surface",
            self.accent,
            self.surface,
            WCAG_NON_TEXT_CONTRAST,
        )?;

        Ok(())
    }
}

fn lighten(color: Rgba, amount: f32) -> Rgba {
    Rgba {
        r: (color.r + amount).clamp(0.0, 1.0),
        g: (color.g + amount).clamp(0.0, 1.0),
        b: (color.b + amount).clamp(0.0, 1.0),
        a: color.a,
    }
}

fn readable_text_color(background: Rgba) -> Rgba {
    let black = rgb(0x000000);
    let white = rgb(0xffffff);

    if contrast_ratio(black, background) >= contrast_ratio(white, background) {
        black
    } else {
        white
    }
}

fn validate_pair(
    label: &'static str,
    foreground: Rgba,
    background: Rgba,
    minimum: f32,
) -> Result<(), String> {
    let contrast = contrast_ratio(foreground, background);
    if contrast < minimum {
        Err(format!(
            "{label} contrast {contrast:.2}:1 is below required {minimum:.1}:1"
        ))
    } else {
        Ok(())
    }
}

pub fn contrast_ratio(foreground: Rgba, background: Rgba) -> f32 {
    let foreground_luminance = relative_luminance(foreground);
    let background_luminance = relative_luminance(background);
    let lighter = foreground_luminance.max(background_luminance);
    let darker = foreground_luminance.min(background_luminance);
    (lighter + 0.05) / (darker + 0.05)
}

pub fn composite_over(foreground: Rgba, background: Rgba) -> Rgba {
    let alpha = foreground.a + background.a * (1.0 - foreground.a);
    if alpha <= f32::EPSILON {
        return Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
    }

    Rgba {
        r: (foreground.r * foreground.a + background.r * background.a * (1.0 - foreground.a))
            / alpha,
        g: (foreground.g * foreground.a + background.g * background.a * (1.0 - foreground.a))
            / alpha,
        b: (foreground.b * foreground.a + background.b * background.a * (1.0 - foreground.a))
            / alpha,
        a: alpha,
    }
}

fn relative_luminance(color: Rgba) -> f32 {
    fn channel(value: f32) -> f32 {
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}
