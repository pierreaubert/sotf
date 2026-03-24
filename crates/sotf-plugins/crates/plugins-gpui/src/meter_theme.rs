//! Plugin UI theming parameters for consistent meter and slider styling

use crate::PluginViewTheme;
use gpui::Rgba;

/// Meter styling parameters for LUFS and True Peak displays
pub struct MeterTheme {
    /// Base color for meter (green)
    pub color_normal: Rgba,
    /// Warning color when approaching threshold (yellow)
    pub color_warning: Rgba,
    /// Critical/clipping color (red)
    pub color_critical: Rgba,
    /// Info color (cyan/teal for stereo width, LUFS, etc.)
    pub color_info: Rgba,
    /// Background color for empty portion of meter
    pub color_background: Rgba,
    /// Border color
    pub color_border: Rgba,
    /// Text color for labels
    pub color_text: Rgba,
    /// Muted text color for legends
    pub color_text_muted: Rgba,

    /// Height of horizontal meter bars
    pub bar_height: f32,
    /// Border radius for rounded corners
    pub border_radius: f32,
    /// Border width
    pub border_width: f32,
    /// Label width for meter labels
    pub label_width: f32,
    /// Value display width
    pub value_width: f32,

    /// Warning threshold position (0.0 to 1.0)
    pub warning_threshold: f32,
    /// Critical threshold position (0.0 to 1.0)
    pub critical_threshold: f32,
}

impl MeterTheme {
    /// Create meter theme from a [`PluginViewTheme`].
    pub fn from_plugin_theme(theme: &PluginViewTheme) -> Self {
        Self {
            color_normal: theme.meter_normal,
            color_warning: theme.meter_warning,
            color_critical: theme.meter_clip,
            color_info: theme.info,
            color_background: theme.surface,
            color_border: theme.border,
            color_text: theme.text_secondary,
            color_text_muted: theme.text_muted,
            bar_height: 20.0,
            border_radius: 4.0,
            border_width: 1.0,
            label_width: 32.0,
            value_width: 50.0,
            warning_threshold: 0.75,
            critical_threshold: 0.90,
        }
    }

    /// Get color for a given fill ratio (0.0 to 1.0)
    pub fn color_for_ratio(&self, ratio: f32) -> Rgba {
        if ratio >= self.critical_threshold {
            self.color_critical
        } else if ratio >= self.warning_threshold {
            self.color_warning
        } else {
            self.color_normal
        }
    }
}

/// True Peak meter configuration
pub struct TruePeakConfig {
    /// Minimum dB value
    pub min_db: f64,
    /// Maximum dB value
    pub max_db: f64,
    /// dB markers to display
    pub markers: Vec<f64>,
}

impl TruePeakConfig {
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self {
            min_db: -60.0,
            max_db: 6.0,
            markers: vec![-60.0, -30.0, -10.0, 0.0, 6.0],
        }
    }

    /// Convert dB value to fill ratio (0.0 to 1.0)
    pub fn db_to_ratio(&self, db: f64) -> f32 {
        let normalized = (db - self.min_db) / (self.max_db - self.min_db);
        normalized.clamp(0.0, 1.0) as f32
    }
}

/// LUFS meter configuration
pub struct LufsConfig {
    /// Minimum dB value
    pub min_db: f64,
    /// Maximum dB value
    pub max_db: f64,
    /// dB markers to display
    pub markers: Vec<f64>,
    /// Target LUFS level
    pub target_db: f64,
}

impl LufsConfig {
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self {
            min_db: -60.0,
            max_db: 0.0,
            markers: vec![-60.0, -50.0, -40.0, -30.0, -20.0, -10.0, 0.0],
            target_db: -24.0, // EBU R128 target
        }
    }

    /// Convert dB value to fill ratio (0.0 to 1.0)
    pub fn db_to_ratio(&self, db: f64) -> f32 {
        let normalized = (db - self.min_db) / (self.max_db - self.min_db);
        normalized.clamp(0.0, 1.0) as f32
    }
}
