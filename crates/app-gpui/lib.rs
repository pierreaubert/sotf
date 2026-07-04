// ============================================================================
// SOTF GPUI Audio Player Library
// ============================================================================
//
// This library module exposes internal types and functions for testing.
// The main application binary is in main.rs.

#![recursion_limit = "8192"]
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

#[cfg(all(feature = "dev-api", not(debug_assertions)))]
compile_error!(
    "The app-gpui dev-api is QA/debug-only and must not be compiled in release/prod builds."
);

pub mod asset_cache;
pub mod eq_layout;
pub mod level_meter_render;
pub mod plugin_file_picker;
pub mod queue_render;

#[cfg(test)]
mod test_harness {
    #[test]
    fn gpui_app_lib_test_harness_is_intentionally_minimal() {}
}

#[cfg(not(test))]
pub mod components;
#[cfg(all(target_os = "linux", not(test)))]
pub mod desktop_integration;
#[cfg(all(not(any(target_os = "ios", target_os = "tvos")), not(test)))]
pub mod media_controls;

// Note: ui must be loaded before app because app re-exports from ui::components::host
#[cfg(not(test))]
pub mod app;
#[cfg(not(test))]
pub mod ui;

// Re-export modules at crate root for simpler imports
#[cfg(not(test))]
pub use app::config;
#[cfg(not(test))]
pub use app::i18n;
#[cfg(not(test))]
pub use app::keybindings;
#[cfg(not(test))]
pub use app::theme;

// Re-export commonly used types for testing
#[cfg(not(test))]
pub use app::{
    App, AppState, ChannelFilter, ChannelGroup, ChannelInfo, ContextMenuState, ContextMenuType,
    InputMode, LayoutMode, LibrarySortOrder, PlatformStyle, QueueItem, Screen, SettingsTab,
    ToastMessage, ToastType, get_param_count,
};

// Re-export additional types for testing
#[cfg(not(test))]
pub use app::types::{
    CalibrationData, ChannelDspChain, ChannelMapping, ChannelMeasurement, ChannelOptResult,
    ChannelRecording, ChannelRecordingState, CrossoverType, DspChainOutput, DspPluginConfig,
    HeadphoneEqStep, LibraryStats, MeasureState, MeterDisplayMode, MultiSpeakerMode,
    PlaybackDeviceConfig, PlotSmoothing, PluginViewMode, RecordingDeviceConfig, RecordingResult,
    RecordingSignalType, RecordingState, RecordingStep, ReplayGainMode, RoomEqAlgorithm,
    RoomEqMeasurementsFile, RoomEqOptimizationMode, RoomEqOptimizerConfig, RoomEqSpeakerConfig,
    RoomEqState, RoomEqStep, SpeakerConfigType, SpeakerConfiguration,
};

// Re-export config types for testing
#[cfg(not(test))]
pub use app::config::{Config, PanelLayout, RecordingConfigState, WindowGeometry};

// Re-export state types for testing
#[cfg(not(test))]
pub use app::state::playback::PlaybackState;

// Re-export component types for testing
#[cfg(not(test))]
pub use components::home::image_cache::{ImageAccessTracker, TrackerStats};
#[cfg(not(test))]
pub use components::icons::{Icon, IconName, IconSize};
#[cfg(not(test))]
pub use gpui_audio_kit::{ScaleType, TickConfig, TickMark};

// Re-export debug types for testing
#[cfg(not(test))]
pub use app::debug::{MAX_HISTORY_SIZE, StateHistory};

// Re-export playback event types for testing
#[cfg(not(test))]
pub use app::state::playback_events::{
    EventStoreSummary, MAX_EVENTS, PlaybackEvent, PlaybackEventStore, PlaybackSnapshot,
    TrackChangeTrigger,
};

// Re-export config helpers for testing
#[cfg(not(test))]
pub use app::config::default_volume;

// Re-export migration functions for testing
#[cfg(not(test))]
pub use components::migration::{check_needs_migration, sanitize_filename};

// Re-export image cache constant for testing
#[cfg(not(test))]
pub use components::home::image_cache::MAX_CACHE_SIZE;

// Re-export UI functions for testing
#[cfg(not(test))]
pub use ui::{
    DEFAULT_MAX_FONT_SIZE_PX, DEFAULT_MIN_FONT_SIZE_PX, combined_scale_bounds,
    compute_responsive_scale, engine_stop_without_queue_should_clear, estimate_grid_dimensions,
    is_phone_sized_window, responsive_scale_reference_size, screen_shows_rack_data,
};

// Re-export room EQ rack-apply helper for testing.
// Exposes the pure functions "Apply to Rack" uses for upsert/classification
// of Room EQ plugins. The implementations now live in
// `sotf-player::autoeq::apply` so the TUI shares the same algorithm; we
// re-export through the GPUI lib so `tests/room_eq_apply_tests.rs` keeps
// building.
#[cfg(not(test))]
pub use components::room_eq::render::{
    RoomEqReportCurve, RoomEqReportData, calculate_room_eq_log_trend,
    is_room_eq_sub_or_lfe_channel, room_eq_channel_sort_key, room_eq_passband_trend_fit_domain,
    room_eq_python_default_smoothing_octaves, room_eq_report_channel_has_renderable_data,
    room_eq_report_data_from_dsp_output, room_eq_report_eq_y_range, room_eq_report_y_range,
    room_eq_trend_fit_domain, should_render_filter_plot, sum_room_eq_responses_db,
};
#[cfg(not(test))]
pub use components::room_eq::{
    RoomEqProgressChartSeries, room_eq_channel_chain_by_name, room_eq_display_response_points,
    room_eq_initial_response_points, room_eq_progress_chart_series,
};
#[cfg(not(test))]
pub use sotf_audio_player::autoeq::{classify_channel_eq_filters, upsert_named_room_eq_plugins};

// Re-export EQ chart functions and constants for testing
#[cfg(not(test))]
pub use components::plugins::ui_eq::{
    CHART_BOTTOM_MARGIN, CHART_HEIGHT, CHART_LEFT_MARGIN, CHART_RIGHT_MARGIN, CHART_TOP_MARGIN,
    GPUI_PX_MARGIN_TOP, MAX_FREQ, MIN_FREQ, Q_BAR_MAX_WIDTH, Q_BAR_MIN_WIDTH, SAMPLE_RATE,
    calculate_band_response, calculate_plot_width, calculate_plot_width_without_legend,
    calculate_response_at_freq, drag_delta_to_q_change, freq_to_x, gain_to_y,
    get_filter_type_index, q_to_bar_width, x_to_freq, y_to_gain,
};
