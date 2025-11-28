//! GPU-accelerated Level Meter Element
//!
//! A custom Element implementation for rendering audio level meters
//! with direct GPU drawing for high performance and smooth animation.

use gpui::*;
use std::panic;

/// dB scale positions: maps dB value to visual position (0.0 = bottom, 1.0 = top)
/// Using non-linear scale for better visual representation
fn db_to_position(db: f64) -> f32 {
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
    channel_name: SharedString,
    /// Whether this channel is clipping
    is_clipping: bool,
    /// Width of the meter bar
    bar_width: Pixels,
    /// Colors for the gradient
    colors: MeterColors,
}

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
        Self {
            background: rgb(0x1e1e1e),
            green: rgb(0x22c55e),
            yellow: rgb(0xf59e0b),
            red: rgb(0xdc2626),
            peak: rgb(0xffffff),
            text: rgb(0x999999),
        }
    }
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
