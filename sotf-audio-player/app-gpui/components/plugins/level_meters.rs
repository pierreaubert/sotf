//! Level meter UI components and logic.
//!
//! This module consolidates all level meter functionality:
//! - GPU-accelerated level meter element (`LevelMeterElement`)
//! - Level meter group rendering (with M/S/D buttons)
//! - Level meter panel rendering (for queue screen)
//! - App methods for level meter group management (mute/solo/dim)

use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::PluginSettings;
use sotf_plugins::ChannelState;
use sotf_plugins::speaker_config::{
    MeterGroupSpec, get_meter_groups, get_meter_groups_by_channels, make_fallback_channel,
};
use std::panic;

use super::{MeterTheme, TickConfig, render_tick_row};
use crate::app::types::PluginUpdateType;
use crate::app::{App as AppState, ChannelGroup, ChannelInfo};
use crate::theme::Theme;
use crate::ui::PlayerView;

// ============================================================================
// dB Scale Utilities
// ============================================================================

/// dB scale positions: maps dB value to visual position (0.0 = bottom, 1.0 = top)
/// Using non-linear scale for better visual representation
pub fn db_to_position(db: f64) -> f32 {
    // -60dB = 0%, -30dB = 33%, -10dB = 66%, 0dB = 100%
    let normalized = if db <= -60.0 {
        0.0
    } else if db <= -30.0 {
        // -60 to -30: linear from 0 to 0.33
        ((db + 60.0) / 30.0) * 0.33
    } else if db <= -10.0 {
        // -30 to -10: linear from 0.33 to 0.66
        0.33 + ((db + 30.0) / 20.0) * 0.33
    } else {
        // -10 to 0: linear from 0.66 to 1.0
        0.66 + ((db + 10.0) / 10.0) * 0.34
    };
    normalized.clamp(0.0, 1.0) as f32
}

// ============================================================================
// GPU-Accelerated Level Meter Element
// ============================================================================

/// Colors used by the level meter
#[derive(Clone)]
pub struct MeterColors {
    /// Background color
    pub background: Rgba,
    /// Green (safe) level color
    pub green: Rgba,
    /// Yellow (caution) level color
    pub yellow: Rgba,
    /// Red (danger/clipping) level color
    pub red: Rgba,
    /// Peak indicator color
    pub peak: Rgba,
    /// Text color for channel name
    pub text: Rgba,
}

impl Default for MeterColors {
    fn default() -> Self {
        // These defaults match the dark theme meter colors.
        // When rendering, use Theme::meter_colors for theme-aware colors.
        Self {
            background: rgb(0x1a1a1a), // theme.surface
            green: rgb(0x22c55e),      // theme.meter_normal
            yellow: rgb(0xf59e0b),     // theme.meter_warning
            red: rgb(0xdc2626),        // theme.meter_clip
            peak: rgb(0xffffff),       // White peak indicator
            text: rgb(0x999999),       // theme.text_secondary
        }
    }
}

/// GPU-accelerated level meter element
///
/// Renders a vertical level meter with gradient coloring (green → yellow → red)
/// using direct GPU quad rendering for maximum performance.
pub struct LevelMeterElement {
    /// Current level in dB
    level_db: f64,
    /// Peak hold level in dB
    peak_db: Option<f64>,
    /// Channel name to display
    #[allow(dead_code)]
    channel_name: SharedString,
    /// Whether this channel is clipping
    is_clipping: bool,
    /// Width of the meter bar
    bar_width: Pixels,
    /// Colors for the gradient
    colors: MeterColors,
}

impl LevelMeterElement {
    /// Create a new level meter element
    pub fn new(level_db: f64, channel_name: impl Into<SharedString>) -> Self {
        Self {
            level_db,
            peak_db: None,
            channel_name: channel_name.into(),
            is_clipping: level_db > -0.1,
            bar_width: px(16.0),
            colors: MeterColors::default(),
        }
    }

    /// Set the peak hold level
    pub fn peak(mut self, peak_db: f64) -> Self {
        self.peak_db = Some(peak_db);
        self
    }

    /// Set the bar width
    pub fn width(mut self, width: Pixels) -> Self {
        self.bar_width = width;
        self
    }

    /// Set custom colors
    pub fn colors(mut self, colors: MeterColors) -> Self {
        self.colors = colors;
        self
    }

    /// Get the fill ratio for the current level
    fn fill_ratio(&self) -> f32 {
        db_to_position(self.level_db)
    }

    /// Get the yellow threshold position
    fn yellow_threshold(&self) -> f32 {
        db_to_position(-6.0)
    }

    /// Get the red threshold position
    fn red_threshold(&self) -> f32 {
        db_to_position(-1.0)
    }
}

impl IntoElement for LevelMeterElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LevelMeterElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // Request a flexible layout that fills available space vertically
        let layout_id = window.request_layout(
            Style {
                size: size(self.bar_width.into(), relative(1.0).into()),
                min_size: size(self.bar_width.into(), px(60.0).into()),
                flex_grow: 1.0,
                flex_shrink: 0.0,
                ..Default::default()
            },
            [],
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let fill_ratio = self.fill_ratio();
        let yellow_threshold = self.yellow_threshold();
        let red_threshold = self.red_threshold();

        // Leave space at bottom for channel name
        let text_height = px(16.0);
        let meter_bounds = Bounds {
            origin: bounds.origin,
            size: size(bounds.size.width, bounds.size.height - text_height),
        };

        // Paint background
        window.paint_quad(PaintQuad {
            bounds: meter_bounds,
            corner_radii: Corners::all(px(2.0)),
            background: self.colors.background.into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

        // Calculate segment heights
        let green_height = fill_ratio.min(yellow_threshold);
        let yellow_height = if fill_ratio > yellow_threshold {
            (fill_ratio - yellow_threshold).min(red_threshold - yellow_threshold)
        } else {
            0.0
        };
        let red_height = if fill_ratio > red_threshold {
            fill_ratio - red_threshold
        } else {
            0.0
        };

        let meter_height = meter_bounds.size.height;

        // Paint green segment (from bottom)
        if green_height > 0.001 {
            let segment_height = meter_height * green_height;
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(
                        meter_bounds.origin.x,
                        meter_bounds.origin.y + meter_height - segment_height,
                    ),
                    size: size(meter_bounds.size.width, segment_height),
                },
                corner_radii: Corners::default(),
                background: self.colors.green.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }

        // Paint yellow segment (above green)
        if yellow_height > 0.001 {
            let segment_height = meter_height * yellow_height;
            let segment_bottom = meter_height * yellow_threshold;
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(
                        meter_bounds.origin.x,
                        meter_bounds.origin.y + meter_height - segment_bottom - segment_height,
                    ),
                    size: size(meter_bounds.size.width, segment_height),
                },
                corner_radii: Corners::default(),
                background: self.colors.yellow.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }

        // Paint red segment (above yellow)
        if red_height > 0.001 {
            let segment_height = meter_height * red_height;
            let segment_bottom = meter_height * red_threshold;
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(
                        meter_bounds.origin.x,
                        meter_bounds.origin.y + meter_height - segment_bottom - segment_height,
                    ),
                    size: size(meter_bounds.size.width, segment_height),
                },
                corner_radii: Corners::default(),
                background: self.colors.red.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }

        // Paint peak indicator if present
        if let Some(peak_db) = self.peak_db {
            let peak_pos = db_to_position(peak_db);
            let peak_y = meter_bounds.origin.y + meter_height * (1.0 - peak_pos);
            let peak_color = if self.is_clipping {
                self.colors.red
            } else {
                self.colors.peak
            };

            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(meter_bounds.origin.x, peak_y - px(1.0)),
                    size: size(meter_bounds.size.width, px(2.0)),
                },
                corner_radii: Corners::default(),
                background: peak_color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }
    }
}

// ============================================================================
// Gradient Meter Rendering (div-based)
// ============================================================================

/// Render a horizontal gain reduction meter
/// Uses render_gradient_meter for consistent styling
pub fn render_gr_meter(
    gain_reduction_db: f64, // Should be negative or 0
    max_db: f64,            // e.g., -30.0 (max gain reduction to display)
    theme: &Theme,
) -> impl IntoElement {
    use super::ticks::{TickConfig, render_tick_row};

    let gr_abs = gain_reduction_db.abs();
    let tick_config = TickConfig::gain_reduction(max_db);

    // Calculate fill ratio using tick config scale
    let fill_ratio = tick_config.value_to_position(gr_abs);

    // Color gradient: green -> yellow -> red based on amount
    let color = if gr_abs < 3.0 {
        theme.meter_normal // Green
    } else if gr_abs < 10.0 {
        theme.meter_warning // Yellow/Orange
    } else {
        theme.meter_clip // Red
    };

    div()
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        // Header row with label and value
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child("Gain Reduction"),
                )
                .child(
                    div()
                        .min_w(px(70.0))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme.surface)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(color)
                        .text_align(TextAlign::Right)
                        .child(format!("-{:.1} dB", gr_abs)),
                ),
        )
        // Meter bar (full width)
        .child(
            div()
                .h(px(12.0))
                .w_full()
                .bg(theme.background)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(fill_ratio))
                        .bg(color)
                        .rounded_l_md(),
                ),
        )
        // Tick marks (full width)
        .child(render_tick_row(&tick_config, 0.0, 0.0))
        // Legend (full width) - show as negative dB values (0, -10, -20, -30)
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(theme.text_muted)
                .children(tick_config.major_values.iter().map(|v| {
                    let label = if *v == 0.0 {
                        "0".to_string()
                    } else {
                        format!("-{:.0}", v)
                    };
                    div().child(label)
                })),
        )
}

/// Render a vertical peak meter with ceiling indicator
pub fn render_peak_meter(peak_db: f64, ceiling_db: f64, theme: &Theme) -> impl IntoElement {
    let min_db = -60.0;
    let normalized = ((peak_db - min_db) / (0.0 - min_db)).clamp(0.0, 1.0) as f32;
    let ceiling_normalized = ((ceiling_db - min_db) / (0.0 - min_db)).clamp(0.0, 1.0) as f32;

    // Color based on level
    let color = if peak_db > ceiling_db {
        theme.meter_clip // Red - clipping
    } else if peak_db > ceiling_db - 3.0 {
        theme.meter_clip // Near ceiling
    } else if peak_db > -12.0 {
        theme.meter_warning // Yellow - moderate
    } else {
        theme.meter_normal // Green - safe
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        // Label
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child("Peak"),
        )
        // Meter
        .child(
            div()
                .w(px(20.0))
                .h(px(80.0))
                .bg(theme.background)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .relative()
                .overflow_hidden()
                // Level bar
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(relative(normalized))
                        .bg(color),
                )
                // Ceiling marker
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(ceiling_normalized))
                        .h(px(2.0))
                        .bg(theme.meter_clip),
                ),
        )
        // Value
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(if peak_db <= min_db {
                    "-∞".to_string()
                } else {
                    format!("{:.1}", peak_db)
                }),
        )
}

/// Render a meter with gradient coloring (green, yellow at top, red at clip)
pub fn render_gradient_meter(
    fill_ratio: f32,
    yellow_threshold: f32,
    red_threshold: f32,
    channel_name: String,
    theme: &Theme,
) -> impl IntoElement {
    // Calculate segment heights
    let green_height = fill_ratio.min(yellow_threshold);
    let yellow_height = if fill_ratio > yellow_threshold {
        (fill_ratio - yellow_threshold).min(red_threshold - yellow_threshold)
    } else {
        0.0
    };
    let red_height = if fill_ratio > red_threshold {
        fill_ratio - red_threshold
    } else {
        0.0
    };

    let theme_c = theme.clone();
    div()
        .flex()
        .flex_col()
        .items_center()
        .flex_1()
        // Meter bar container
        .child(
            div()
                .w(px(16.0))
                .flex_1()
                .min_h(px(180.0))
                .bg(theme_c.background)
                .rounded(px(2.0))
                .overflow_hidden()
                .relative()
                // Green segment (base)
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                            green_height,
                        )))
                        .bg(theme_c.meter_normal),
                )
                // Yellow segment (above green)
                .when(yellow_height > 0.001, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                yellow_threshold,
                            )))
                            .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                yellow_height,
                            )))
                            .bg(theme_c.meter_warning),
                    )
                })
                // Red segment (above yellow)
                .when(red_height > 0.001, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                red_threshold,
                            )))
                            .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                red_height,
                            )))
                            .bg(theme_c.meter_clip),
                    )
                }),
        )
        // Channel name
        .child(
            div()
                .text_xs()
                .text_color(theme.text_muted)
                .mt_1()
                .child(channel_name),
        )
}

// ============================================================================
// Standalone LUFS Display Function
// ============================================================================

/// Render LUFS display with True Peak bars at top (standalone function)
pub fn render_lufs_with_true_peak(
    loudness: Option<&sotf_audio_player::LoudnessData>,
    theme: &Theme,
) -> impl IntoElement {
    let (
        integrated_lufs,
        shortterm_lufs,
        momentary_lufs,
        true_peak_left,
        true_peak_right,
        stereo_width,
    ) = if let Some(l) = loudness {
        let tp_left = l.true_peaks_dbtp.first().copied().unwrap_or(-60.0);
        let tp_right = l.true_peaks_dbtp.get(1).copied().unwrap_or(tp_left);
        // Stereo width derived from correlation: +1 = mono (0), 0 = uncorrelated (0.5), -1 = out of phase (1)
        let width = l
            .correlation_lr
            .map(|c| ((1.0 - c) / 2.0).clamp(0.0, 1.0))
            .unwrap_or(0.5);
        (
            l.integrated_lufs,
            l.shortterm_lufs,
            l.momentary_lufs,
            tp_left,
            tp_right,
            width,
        )
    } else {
        (-60.0, -60.0, -60.0, -60.0, -60.0, 0.5)
    };

    let meter_theme = MeterTheme::from_theme(theme);

    div()
        .flex()
        .flex_col()
        .gap_4()
        .p_3()
        // True Peak section (on top)
        .child({
            // Use TickConfig preset for True Peak (quadratic scale from -60 to +6)
            let tick_config = TickConfig::true_peak();

            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .mb_1()
                        .child("True Peak"),
                )
                // Left channel bar (uses same scale as ticks)
                .child(PlayerView::render_meter_bar(
                    "L",
                    true_peak_left,
                    &tick_config,
                    &meter_theme,
                ))
                // Right channel bar (uses same scale as ticks)
                .child(PlayerView::render_meter_bar(
                    "R",
                    true_peak_right,
                    &tick_config,
                    &meter_theme,
                ))
                // Tick marks (aligned with bar using same flex layout)
                .child(render_tick_row(
                    &tick_config,
                    meter_theme.label_width,
                    meter_theme.value_width,
                ))
                // True Peak legend (same flex layout as bar and ticks)
                .child(
                    div()
                        .flex()
                        .gap(px(1.0))
                        // Label spacer
                        .child(div().w(px(meter_theme.label_width)))
                        // Legend area (flex-1, justify_between for labels)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .text_color(meter_theme.color_text_muted)
                                .children(tick_config.major_values.iter().map(|db| {
                                    let label = if *db > 0.0 {
                                        format!("+{}", *db as i32)
                                    } else {
                                        format!("{}", *db as i32)
                                    };
                                    div().child(label)
                                })),
                        )
                        // Value spacer
                        .child(div().w(px(meter_theme.value_width))),
                )
        })
        // LUFS section (below)
        .child({
            // Use TickConfig preset for LUFS (quadratic scale from -60 to 0)
            let tick_config = TickConfig::lufs();

            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .mb_1()
                        .child("LUFS"),
                )
                // Integrated LUFS (uses same scale as ticks)
                .child(PlayerView::render_meter_bar(
                    "I",
                    integrated_lufs,
                    &tick_config,
                    &meter_theme,
                ))
                // Short-term LUFS (uses same scale as ticks)
                .child(PlayerView::render_meter_bar(
                    "S",
                    shortterm_lufs,
                    &tick_config,
                    &meter_theme,
                ))
                // Momentary LUFS (uses same scale as ticks)
                .child(PlayerView::render_meter_bar(
                    "M",
                    momentary_lufs,
                    &tick_config,
                    &meter_theme,
                ))
                // Tick marks (aligned with bar using same flex layout)
                .child(render_tick_row(
                    &tick_config,
                    meter_theme.label_width,
                    meter_theme.value_width,
                ))
                // LUFS legend (same flex layout as bar and ticks)
                .child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        // Label spacer
                        .child(div().w(px(meter_theme.label_width)))
                        // Legend area (flex-1, justify_between for labels)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .text_color(meter_theme.color_text_muted)
                                .child(div().child("-60"))
                                .child(div().child("-30"))
                                .child(div().child("-10"))
                                .child(div().child("0")),
                        )
                        // Value spacer
                        .child(div().w(px(meter_theme.value_width))),
                )
        })
        // Stereo Width section
        .child({
            // Use TickConfig preset for stereo width (linear scale 0 to 1)
            let tick_config = TickConfig::stereo_width();

            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .mb_1()
                        .child("Stereo Width"),
                )
                // Width bar (uses same scale as ticks)
                .child(PlayerView::render_width_bar(
                    stereo_width,
                    &tick_config,
                    &meter_theme,
                ))
                // Tick marks (aligned with bar using same flex layout)
                .child(render_tick_row(
                    &tick_config,
                    meter_theme.label_width,
                    meter_theme.value_width,
                ))
                // Width legend (same flex layout as bar and ticks)
                .child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        // Label spacer
                        .child(div().w(px(meter_theme.label_width)))
                        // Legend area (flex-1, justify_between for labels)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .text_color(meter_theme.color_text_muted)
                                .child(div().child("Mono"))
                                .child(div().child("50%"))
                                .child(div().child("Wide")),
                        )
                        // Value spacer
                        .child(div().w(px(meter_theme.value_width))),
                )
        })
}

// ============================================================================
// PlayerView Level Meter UI Methods
// ============================================================================

impl PlayerView {
    /// Render vertical dB legend
    pub fn render_vertical_legend(&self, theme: &Theme, align_right: bool) -> impl IntoElement {
        let ticks = [0, -6, -12, -18, -24, -30, -40, -50, -60];
        let theme = theme.clone();

        div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(280.0))
            .p(px(2.0)) // Match meter group padding
            // Outer container (matches meters_row flex_1 h_full)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(200.0))
                    // Ticks area (matches meter_bar flex_1)
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h(px(180.0))
                            .w(px(24.0))
                            .overflow_hidden()
                            .children(ticks.into_iter().map(move |db| {
                                let pos = db_to_position(db as f64);
                                // Use top positioning: top = (1 - pos), then offset by half line height
                                let top_fraction = 1.0 - pos;

                                // Adjust label offset for edge labels to keep them visible:
                                // - Top label (0 dB): move label down
                                // - Bottom label (-60 dB): move label up
                                // - Other labels: no additional offset
                                let label_offset = if db == 0 {
                                    px(6.0) // Top: move label down
                                } else if db == -60 {
                                    px(-6.0) // Bottom: move label up
                                } else {
                                    px(0.0) // No additional offset
                                };

                                let label = div()
                                    .text_size(px(9.0))
                                    .text_color(theme.text_muted)
                                    .mt(label_offset)
                                    .child(format!("{}", db));

                                let tick = div().w(px(4.0)).h(px(1.0)).bg(theme.border);

                                let container = div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .top(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                        top_fraction,
                                    )))
                                    // Offset by half line height (~6px for 9px text) to center tick on position
                                    .mt(px(-6.0))
                                    .flex()
                                    .items_center()
                                    .justify_between();

                                if align_right {
                                    // Legend on right: tick → label (tick points toward meter on left)
                                    container.child(tick).child(label)
                                } else {
                                    // Legend on left: label → tick (tick points toward meter on right)
                                    container.child(label).child(tick)
                                }
                            })),
                    )
                    // Spacer for Channel Name (matches render_gradient_meter channel name)
                    .child(
                        div().text_xs().mt_1().opacity(0.0).child("X"), // Dummy text to match height
                    ),
            )
            // Spacer (matches MSD buttons height and margin)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .mt_1()
                    .items_center()
                    .justify_center()
                    .opacity(0.0) // Invisible, just for spacing
                    .child(div().px(px(2.0)).py(px(2.0)).text_xs().child("M"))
                    .child(div().px(px(2.0)).py(px(2.0)).text_xs().child("S"))
                    .child(div().px(px(2.0)).py(px(2.0)).text_xs().child("D")),
            )
    }

    /// Render a single meter group with M/S/D buttons below the channels
    pub fn render_meter_group(
        &self,
        group: &ChannelGroup,
        group_idx: usize,
        _is_selected: bool,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted = group.muted;
        let soloed = group.soloed;
        let dimmed = group.dimmed;

        // Pre-compute channel data to avoid closure issues with cx
        let channel_data: Vec<_> = group
            .channels
            .iter()
            .map(|channel| {
                let peak = loudness
                    .and_then(|l| l.channel_peaks.get(channel.index))
                    .copied()
                    .unwrap_or(0.0);

                let peak_db = if peak > 0.0001 {
                    20.0 * peak.log10()
                } else {
                    -60.0
                };

                let fill_ratio = db_to_position(peak_db);
                let yellow_threshold = db_to_position(-6.0);
                let red_threshold = db_to_position(-1.0);

                (
                    fill_ratio,
                    yellow_threshold,
                    red_threshold,
                    channel.name.clone(),
                )
            })
            .collect();

        let theme_c = theme.clone();
        div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(280.0))
            .p(px(2.0))
            // Removed rounded_md and selection background logic
            .bg(theme_c.background_secondary)
            // Channel meters
            .child(
                div()
                    .flex()
                    .gap(px(1.0))
                    .flex_1()
                    .min_h(px(200.0))
                    .children(channel_data.into_iter().map(
                        |(fill_ratio, yellow_threshold, red_threshold, name)| {
                            render_gradient_meter(
                                fill_ratio,
                                yellow_threshold,
                                red_threshold,
                                name,
                                theme,
                            )
                        },
                    )),
            )
            // M/S/D buttons vertical column below meters, centered
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .mt_1()
                    .items_center()
                    .justify_center()
                    .child(self.render_msd_button(
                        "M",
                        muted,
                        theme.button_mute_active,
                        group_idx,
                        "mute",
                        theme,
                        cx,
                    ))
                    .child(self.render_msd_button(
                        "S",
                        soloed,
                        theme.button_solo_active,
                        group_idx,
                        "solo",
                        theme,
                        cx,
                    ))
                    .child(self.render_msd_button(
                        "D",
                        dimmed,
                        theme.button_dim_active,
                        group_idx,
                        "dim",
                        theme,
                        cx,
                    )),
            )
    }

    /// Render M/S/D button (interactive)
    pub fn render_msd_button(
        &self,
        label: &'static str,
        active: bool,
        active_color: gpui::Rgba,
        group_idx: usize,
        button_type: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme_c = theme.clone();
        div()
            .id((button_type, group_idx))
            .px(px(2.0))
            .py(px(2.0))
            .rounded(px(2.0))
            .text_xs()
            .cursor_pointer()
            .when(active, |d| {
                d.bg(active_color).text_color(theme_c.text_primary)
            })
            .when(!active, |d| {
                d.bg(theme_c.surface)
                    .text_color(theme_c.text_muted)
                    .hover(|style| style.bg(theme_c.surface_hover))
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        if group_idx < state.app.level_meter_groups.len() {
                            match button_type {
                                "mute" => {
                                    state.app.level_meter_groups[group_idx].muted =
                                        !state.app.level_meter_groups[group_idx].muted
                                }
                                "solo" => {
                                    state.app.level_meter_groups[group_idx].soloed =
                                        !state.app.level_meter_groups[group_idx].soloed
                                }
                                "dim" => {
                                    state.app.level_meter_groups[group_idx].dimmed =
                                        !state.app.level_meter_groups[group_idx].dimmed
                                }
                                _ => {}
                            }
                        }
                    });
                    cx.notify();
                }),
            )
            .child(label)
    }

    /// Render separate Meters panel (for queue screen)
    pub fn render_meters_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, loudness, groups, selected_group) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.playback.loudness_info.clone(),
                state.app.level_meter_groups.clone(),
                state.app.selected_level_meter_group,
            )
        };

        let theme_c = theme.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .bg(theme.background)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .mb_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Level Meters"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(2.0))
                            .rounded(px(3.0))
                            .text_xs()
                            .bg(theme.surface)
                            .text_color(theme.text_secondary)
                            .cursor_pointer()
                            .hover(|style| style.bg(theme_c.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        for group in &mut state.app.level_meter_groups {
                                            group.muted = false;
                                            group.soloed = false;
                                            group.dimmed = false;
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("Clear All"),
                    ),
            )
            .child({
                // Use a for loop to avoid FnMut closure escape issues with cx
                let mut meter_elements = Vec::new();

                // Left Legend
                meter_elements.push(
                    self.render_vertical_legend(&theme, false)
                        .into_any_element(),
                );

                for (idx, group) in groups.iter().enumerate() {
                    let is_selected = idx == selected_group;
                    meter_elements.push(
                        self.render_meter_group(
                            group,
                            idx,
                            is_selected,
                            loudness.as_ref(),
                            &theme,
                            cx,
                        )
                        .into_any_element(),
                    );
                }

                // Right Legend
                meter_elements.push(self.render_vertical_legend(&theme, true).into_any_element());

                div()
                    .id("meter-groups-scroll")
                    .flex()
                    .flex_1()
                    .gap(px(0.0))
                    .overflow_x_scroll()
                    .min_h(px(300.0))
                    .children(meter_elements)
            })
    }

    /// Render unified meter bar with consistent styling
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    pub fn render_meter_bar(
        label: &str,
        value: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        // Use the same scale as the ticks for bar fill
        let ratio = tick_config.value_to_position(value);
        let bar_color = meter_theme.color_for_ratio(ratio);

        div()
            .flex()
            .items_center()
            .gap(px(4.0)) // Tighter gap for more bar space
            // Label
            .child(
                div()
                    .w(px(meter_theme.label_width))
                    .text_xs()
                    .text_color(meter_theme.color_text)
                    .child(label.to_string()),
            )
            // Bar with border
            .child(
                div()
                    .flex_1()
                    .h(px(meter_theme.bar_height))
                    .rounded(px(meter_theme.border_radius))
                    .border(px(meter_theme.border_width))
                    .border_color(meter_theme.color_border)
                    .bg(meter_theme.color_background)
                    .overflow_hidden()
                    .child(div().h_full().w(gpui::relative(ratio)).bg(bar_color)),
            )
            // Value display
            .child(
                div()
                    .w(px(meter_theme.value_width))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(meter_theme.color_text)
                    .text_align(TextAlign::Right)
                    .child(format!("{:.1}", value)),
            )
    }

    /// Render stereo width bar (0 = mono, 1 = wide)
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    pub fn render_width_bar(
        width: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        // Use the same scale as the ticks for bar fill
        let ratio = tick_config.value_to_position(width);
        // Color: cyan/teal for width (uses info color from theme via meter_theme)
        let bar_color = meter_theme.color_info;

        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            // Label
            .child(
                div()
                    .w(px(meter_theme.label_width))
                    .text_xs()
                    .text_color(meter_theme.color_text)
                    .child("W"),
            )
            // Bar with border
            .child(
                div()
                    .flex_1()
                    .h(px(meter_theme.bar_height))
                    .rounded(px(meter_theme.border_radius))
                    .border(px(meter_theme.border_width))
                    .border_color(meter_theme.color_border)
                    .bg(meter_theme.color_background)
                    .overflow_hidden()
                    .child(div().h_full().w(gpui::relative(ratio)).bg(bar_color)),
            )
            // Value display
            .child(
                div()
                    .w(px(meter_theme.value_width))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(meter_theme.color_text)
                    .text_align(TextAlign::Right)
                    .child(format!("{:.0}%", width * 100.0)),
            )
    }

    /// Render LUFS display with True Peak bars at top (wrapper method)
    pub fn render_lufs_with_true_peak(
        &self,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        theme: &Theme,
    ) -> impl IntoElement {
        // Call the standalone function
        render_lufs_with_true_peak(loudness, theme)
    }

    /// Render separate LUFS panel
    pub fn render_lufs_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, loudness) = {
            let state = self.state.read(cx);
            (state.app.ui_state.theme.clone(), state.app.playback.loudness_info.clone())
        };
        div()
            .flex()
            .flex_col()
            .w(px(400.0))
            .p_4()
            .bg(theme.background)
            .child(self.render_lufs_with_true_peak(loudness.as_ref(), &theme))
    }
}

/// Calculate ideal width for meters panel based on channel count
/// Returns width in pixels: 200px for stereo, up to 400px for 16 channels
pub fn calculate_meters_panel_width(num_channels: usize) -> f32 {
    // Base width for stereo (2 channels)
    let base_width = 200.0_f32;
    // Max width for 16 channels
    let max_width = 400.0_f32;
    // Scale linearly between 2 and 16 channels
    let channels = (num_channels as f32).clamp(2.0, 16.0);
    let scale = (channels - 2.0) / 14.0; // 0.0 for 2ch, 1.0 for 16ch
    base_width + scale * (max_width - base_width)
}

// ============================================================================
// App Level Meter Group Management
// ============================================================================

pub trait LevelMeterManager {
    fn update_level_meter_groups(&mut self);
    fn clear_level_meter_mutes_and_solos(&mut self);
    fn toggle_level_meter_mute(&mut self);
    fn toggle_level_meter_solo(&mut self);
    fn toggle_level_meter_dim(&mut self);
    fn select_next_level_meter_group(&mut self);
    fn select_previous_level_meter_group(&mut self);
    fn select_next_level_meter_control(&mut self);
    fn select_previous_level_meter_control(&mut self);
    fn toggle_selected_level_meter_control(&mut self);
    fn update_channel_mute_solo_plugin(&mut self);
}

impl LevelMeterManager for AppState {
    /// Update level meter groups based on current speaker configuration or channel count
    /// Creates a default stereo layout when no audio is playing
    /// Uses caching to avoid rebuilding every frame
    /// Update level meter groups based on current speaker configuration or channel count
    /// Creates a default stereo layout when no audio is playing
    /// Uses caching to avoid rebuilding every frame
    fn update_level_meter_groups(&mut self) {
        let num_channels = self
            .playback
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        // Default to stereo (2 channels) when no audio is playing
        // This ensures meters are always visible with -60 dB default
        let num_channels = if num_channels == 0 { 2 } else { num_channels };

        // Get current speaker config
        let current_speaker_config = self.plugin_state.plugin_chain.output_speaker_config().map(String::from);

        // Skip rebuilding if nothing has changed
        if num_channels == self.level_meter_last_channel_count
            && current_speaker_config == self.level_meter_last_speaker_config
            && !self.level_meter_groups.is_empty()
        {
            return;
        }

        // Update cache
        self.level_meter_last_channel_count = num_channels;
        self.level_meter_last_speaker_config = current_speaker_config.clone();

        self.level_meter_groups.clear();

        // Try to get meter groups from the speaker config (via upmixer plugin)
        // This handles collisions like 5.1.4 vs 7.1.2 (both 10 channels)
        let meter_groups: Option<&[MeterGroupSpec]> = current_speaker_config
            .as_deref()
            .and_then(get_meter_groups)
            .or_else(|| get_meter_groups_by_channels(num_channels));

        if let Some(groups) = meter_groups {
            // Convert static specs to runtime groups
            for group_spec in groups {
                self.level_meter_groups.push(ChannelGroup {
                    name: group_spec.name.to_string(),
                    channels: group_spec
                        .channels
                        .iter()
                        .map(|ch| ChannelInfo {
                            index: ch.index,
                            name: ch.label.to_string(),
                            display_name: ch
                                .display_chars
                                .iter()
                                .map(|s| (*s).to_string())
                                .collect(),
                        })
                        .collect(),
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
        } else {
            // Fallback for unknown channel counts (mono, quad, or exotic configs)
            match num_channels {
                1 => {
                    // Mono
                    self.level_meter_groups.push(ChannelGroup {
                        name: "Mono".to_string(),
                        channels: vec![ChannelInfo {
                            index: 0,
                            name: "M".to_string(),
                            display_name: vec!["M".to_string()],
                        }],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
                4 => {
                    // Quad (FL, FR, SL, SR) - not a standard speaker config
                    self.level_meter_groups.push(ChannelGroup {
                        name: "L/R".to_string(),
                        channels: vec![
                            ChannelInfo {
                                index: 0,
                                name: "L".to_string(),
                                display_name: vec!["L".to_string()],
                            },
                            ChannelInfo {
                                index: 1,
                                name: "R".to_string(),
                                display_name: vec!["R".to_string()],
                            },
                        ],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                    self.level_meter_groups.push(ChannelGroup {
                        name: "Surrounds".to_string(),
                        channels: vec![
                            ChannelInfo {
                                index: 2,
                                name: "SL".to_string(),
                                display_name: vec!["S".to_string(), "L".to_string()],
                            },
                            ChannelInfo {
                                index: 3,
                                name: "SR".to_string(),
                                display_name: vec!["S".to_string(), "R".to_string()],
                            },
                        ],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
                _ => {
                    // Generic fallback - treat all channels as one group
                    let channels: Vec<ChannelInfo> = (0..num_channels)
                        .map(|i| {
                            let spec = make_fallback_channel(i);
                            ChannelInfo {
                                index: spec.index,
                                name: spec.label.to_string(),
                                display_name: spec
                                    .display_chars
                                    .iter()
                                    .map(|s| (*s).to_string())
                                    .collect(),
                            }
                        })
                        .collect();
                    self.level_meter_groups.push(ChannelGroup {
                        name: "All Channels".to_string(),
                        channels,
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
            }
        }

        // Update ChannelMuteSolo plugin to have correct number of channels
        self.update_channel_mute_solo_plugin();
    }

    /// Clear all mutes, solos, and dims in level meter groups
    /// Clear all mutes, solos, and dims in level meter groups
    fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meter_groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_channel_mute_solo_plugin();
    }

    /// Toggle mute for the selected level meter group
    /// Toggle mute for the selected level meter group
    fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.muted = !group.muted;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle solo for the selected level meter group
    /// Toggle solo for the selected level meter group
    fn toggle_level_meter_solo(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            let is_currently_soloed = group.soloed;

            // Solo behavior: only one group can be soloed at a time
            // When soloing, set soloed=true on selected group, soloed=false on all others
            // When un-soloing, set soloed=false on selected group
            for (idx, g) in self.level_meter_groups.iter_mut().enumerate() {
                if idx == self.selected_level_meter_group {
                    g.soloed = !is_currently_soloed;
                } else {
                    g.soloed = false;
                }
            }

            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle dim for the selected level meter group
    /// Toggle dim for the selected level meter group
    fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.dimmed = !group.dimmed;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Update the ChannelMuteSolo plugin based on current level meter group states
    fn update_channel_mute_solo_plugin(&mut self) {
        // Calculate total channel count
        let num_channels: usize = self
            .level_meter_groups
            .iter()
            .map(|g| g.channels.len())
            .sum();

        if num_channels == 0 {
            return;
        }

        // Build per-channel states from groups
        let mut channel_states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false
            };
            num_channels
        ];

        for group in &self.level_meter_groups {
            for channel_info in &group.channels {
                if channel_info.index < num_channels {
                    channel_states[channel_info.index] = ChannelState {
                        muted: group.muted,
                        soloed: group.soloed,
                        dimmed: group.dimmed,
                    };
                }
            }
        }

        // Determine if any channel is muted, soloed, or dimmed
        let enabled = channel_states
            .iter()
            .any(|s| s.muted || s.soloed || s.dimmed);

        // Find and update the ChannelMuteSolo plugin
        for i in 0..self.plugin_state.plugin_chain.len() {
            if let Some(plugin) = self.plugin_state.plugin_chain.get_plugin_mut(i) {
                if matches!(&plugin.settings, PluginSettings::ChannelMuteSolo { .. }) {
                    // Update settings in memory
                    plugin.settings = PluginSettings::ChannelMuteSolo {
                        enabled,
                        channel_states: channel_states.clone(),
                    };
                    // Flag that plugins need updating
                    self.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
                    return;
                }
            }
        }
    }

    /// Navigate to next level meter group
    /// Navigate to next level meter group
    fn select_next_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            self.selected_level_meter_group =
                (self.selected_level_meter_group + 1) % self.level_meter_groups.len();
        }
    }

    /// Navigate to previous level meter group
    /// Navigate to previous level meter group
    fn select_previous_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            if self.selected_level_meter_group == 0 {
                self.selected_level_meter_group = self.level_meter_groups.len() - 1;
            } else {
                self.selected_level_meter_group -= 1;
            }
        }
    }

    /// Navigate between mute, solo, and dim controls
    /// Navigate between mute, solo, and dim controls
    fn select_next_level_meter_control(&mut self) {
        self.level_meter_control_selection = (self.level_meter_control_selection + 1) % 3;
    }

    /// Navigate between mute, solo, and dim controls (previous)
    /// Navigate between mute, solo, and dim controls (previous)
    fn select_previous_level_meter_control(&mut self) {
        self.level_meter_control_selection = if self.level_meter_control_selection == 0 {
            2
        } else {
            self.level_meter_control_selection - 1
        };
    }

    /// Toggle the currently selected level meter control (mute/solo/dim)
    /// Toggle the currently selected level meter control (mute/solo/dim)
    fn toggle_selected_level_meter_control(&mut self) {
        match self.level_meter_control_selection {
            0 => self.toggle_level_meter_mute(),
            1 => self.toggle_level_meter_solo(),
            2 => self.toggle_level_meter_dim(),
            _ => {}
        }
    }
}
