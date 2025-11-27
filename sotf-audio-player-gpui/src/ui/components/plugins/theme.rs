//! Plugin UI theming parameters for consistent meter and slider styling

use gpui::*;

/// Meter styling parameters for LUFS and True Peak displays
pub struct MeterTheme {
    /// Base color for meter (green)
    pub color_normal: Rgba,
    /// Warning color when approaching threshold (yellow)
    pub color_warning: Rgba,
    /// Critical/clipping color (red)
    pub color_critical: Rgba,
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
    /// Create default meter theme with green/yellow/red colors
    pub fn default() -> Self {
        Self {
            color_normal: rgb(0x22c55e),      // Green
            color_warning: rgb(0xeab308),     // Yellow
            color_critical: rgb(0xef4444),    // Red
            color_background: rgb(0x2a2a2a),  // Dark gray
            color_border: rgb(0x444444),      // Border gray
            color_text: rgb(0xcccccc),        // Light gray text
            color_text_muted: rgb(0x888888),  // Muted gray
            bar_height: 20.0,
            border_radius: 4.0,
            border_width: 1.0,
            label_width: 32.0,  // Reduced from 80px to make bars 60% longer
            value_width: 50.0,  // Value display width
            warning_threshold: 0.75,   // 75% of range
            critical_threshold: 0.90,  // 90% of range
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
