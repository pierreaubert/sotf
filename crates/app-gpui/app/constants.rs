//! Application constants
//!
//! Centralizes magic numbers and default values used throughout the application.
//! All values should be defined here rather than inline in code.

pub mod library {
    use crate::app::ChannelFilter;
    use crate::app::LibrarySortOrder;

    /// Default items per page in library grid
    pub const DEFAULT_ITEMS_PER_PAGE: usize = 50;

    /// Minimum items per page
    pub const MIN_ITEMS_PER_PAGE: usize = 10;

    /// Maximum items per page
    pub const MAX_ITEMS_PER_PAGE: usize = 200;

    /// Card minimum width in pixels (for grid calculation)
    pub const CARD_MIN_WIDTH_PX: f32 = 160.0;

    /// Card gap in pixels
    pub const CARD_GAP_PX: f32 = 16.0;

    /// Card height in pixels (for grid calculation)
    pub const CARD_HEIGHT_PX: f32 = 220.0;

    /// Estimated header height in pixels
    pub const HEADER_HEIGHT_PX: f32 = 40.0;

    /// Estimated stats bar height in pixels
    pub const STATS_HEIGHT_PX: f32 = 100.0;

    /// Estimated filter bar height in pixels
    pub const FILTER_HEIGHT_PX: f32 = 40.0;

    /// Estimated pagination bar height in pixels
    pub const PAGINATION_HEIGHT_PX: f32 = 50.0;

    /// Estimated footer height in pixels
    pub const FOOTER_HEIGHT_PX: f32 = 60.0;

    /// Minimum columns in library grid
    pub const MIN_LIBRARY_COLUMNS: usize = 1;

    /// Default channel filter
    pub const DEFAULT_CHANNEL_FILTER: ChannelFilter = ChannelFilter::All;

    /// Default sort order
    pub const DEFAULT_SORT_ORDER: LibrarySortOrder = LibrarySortOrder::Album;

    /// Genre minimum album count for display
    pub const GENRE_MIN_ALBUMS_FOR_DISPLAY: usize = 5;

    /// Maximum artists/composers to show per letter
    pub const MAX_ARTISTS_PER_LETTER: usize = 20;

    /// Minimum genre count for display (used in filtering)
    pub const MIN_GENRE_COUNT: usize = 5;

    /// Scroll threshold for infinite scroll (pixels from bottom)
    pub const INFINITE_SCROLL_THRESHOLD_PX: f32 = 1000.0;

    /// Scroll content threshold (content should be at least 2x viewport for "needs more")
    pub const SCROLL_CONTENT_MULTIPLIER: f32 = 2.0;
}

pub mod ui {
    /// Default queue panel height ratio (0.0-1.0)
    pub const QUEUE_PANEL_DEFAULT_RATIO: f32 = 0.35;

    /// Default meters panel width ratio (0.0-1.0)
    pub const METERS_PANEL_DEFAULT_RATIO: f32 = 0.25;

    /// Default queue list width ratio (0.0-1.0)
    pub const QUEUE_LIST_DEFAULT_RATIO: f32 = 0.30;

    /// Default LUFS panel width ratio (0.0-1.0)
    pub const LUFS_PANEL_DEFAULT_RATIO: f32 = 0.25;

    /// Default window width in pixels
    pub const DEFAULT_WINDOW_WIDTH: f32 = 1200.0;

    /// Default window height in pixels
    pub const DEFAULT_WINDOW_HEIGHT: f32 = 800.0;

    /// Default window X position
    pub const DEFAULT_WINDOW_X: f32 = 100.0;

    /// Default window Y position
    pub const DEFAULT_WINDOW_Y: f32 = 100.0;

    /// Minimum window height for expanded layout mode
    pub const EXPANDED_LAYOUT_MIN_HEIGHT: f32 = 800.0;

    /// Input meter default width in pixels
    pub const INPUT_METER_DEFAULT_WIDTH: f32 = 80.0;

    /// Output meter default width in pixels
    pub const OUTPUT_METER_DEFAULT_WIDTH: f32 = 140.0;

    /// Minimum panel width in pixels
    pub const MIN_PANEL_WIDTH_PX: f32 = 50.0;

    /// Volume change step (small adjustment)
    pub const VOLUME_STEP_SMALL: f32 = 0.02;

    /// Volume change step (large adjustment)
    pub const VOLUME_STEP_LARGE: f32 = 0.05;

    /// Default volume on startup (10%)
    pub const DEFAULT_STARTUP_VOLUME: f32 = 0.1;

    /// Default volume for new configs (10%)
    pub const DEFAULT_CONFIG_VOLUME: f32 = 0.1;

    /// Window geometry change threshold in pixels (debouncing)
    pub const WINDOW_CHANGE_THRESHOLD_PX: f32 = 1.0;
}

pub mod playback {
    /// Update interval for playback position (milliseconds)
    pub const POSITION_UPDATE_INTERVAL_MS: u64 = 100;

    /// Update interval for level meters (milliseconds)
    pub const METER_UPDATE_INTERVAL_MS: u64 = 50;

    /// Update interval for spectrum analyzer (milliseconds)
    pub const SPECTRUM_UPDATE_INTERVAL_MS: u64 = 100;

    /// Update interval for background managers (milliseconds)
    pub const MANAGER_UPDATE_INTERVAL_MS: u64 = 1000;
}

pub mod eq {
    /// EQ frequency minimum (Hz)
    pub const EQ_FREQ_MIN: f64 = 20.0;

    /// EQ frequency maximum (Hz)
    pub const EQ_FREQ_MAX: f64 = 20000.0;

    /// EQ Q factor minimum
    pub const EQ_Q_MIN: f64 = 0.1;

    /// EQ Q factor maximum
    pub const EQ_Q_MAX: f64 = 10.0;

    /// EQ gain minimum (dB)
    pub const EQ_GAIN_MIN: f64 = -24.0;

    /// EQ gain maximum (dB)
    pub const EQ_GAIN_MAX: f64 = 24.0;
}

pub mod audio {
    /// Default sample rate for playback
    pub const DEFAULT_SAMPLE_RATE: f64 = 48000.0;

    /// Common sample rates
    pub const SAMPLE_RATE_44100: u32 = 44100;
    pub const SAMPLE_RATE_48000: u32 = 48000;
    pub const SAMPLE_RATE_96000: u32 = 96000;
}

pub mod recording {
    /// Default signal duration (seconds)
    pub const DEFAULT_SIGNAL_DURATION_SECS: f32 = 5.0;

    /// Default signal level (dB)
    pub const DEFAULT_SIGNAL_LEVEL_DB: f32 = -20.0;
}

/// Design-system spacing scale (matches design-tokens/tokens.json `global.spacing`).
///
/// Use these instead of raw `px()` / `rems()` values for gaps, padding, and margins.
/// Migration is incremental — new code should use these; existing code migrates over time.
pub mod spacing {
    use gpui::{Pixels, px};

    pub const NONE: Pixels = px(0.0);
    /// 2 px — hairline gaps, badge padding
    pub const XS: Pixels = px(2.0);
    /// 4 px — tight gaps (icon ↔ label), inline padding
    pub const SM: Pixels = px(4.0);
    /// 8 px — standard internal padding, small gaps between related items
    pub const MD: Pixels = px(8.0);
    /// 16 px — section padding, card gaps, standard component spacing
    pub const LG: Pixels = px(16.0);
    /// 24 px — panel padding, large section gaps
    pub const XL: Pixels = px(24.0);
    /// 32 px — page-level margins, major section separators
    pub const XXL: Pixels = px(32.0);
}

/// Design-system border radius scale (matches design-tokens/tokens.json `global.sizing.borderRadius`).
///
/// Use these instead of raw `.rounded_md()` / `.rounded(px(N))` calls.
pub mod radius {
    use gpui::{Pixels, px};

    /// 2 px — subtle rounding (badges, inline tags)
    pub const SM: Pixels = px(2.0);
    /// 4 px — standard rounding (cards, buttons, inputs) — the design-token default
    pub const MD: Pixels = px(4.0);
    /// 8 px — prominent rounding (modals, panels, plugin shells)
    pub const LG: Pixels = px(8.0);
    /// 12 px — large rounding (dialogs, featured cards)
    pub const XL: Pixels = px(12.0);
}
