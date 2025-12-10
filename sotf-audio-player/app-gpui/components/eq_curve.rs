//! GPU-accelerated EQ Frequency Response Curve Element
//!
//! A custom Element implementation for rendering smooth EQ frequency response curves
//! with direct GPU path rendering for high performance.

use autoeq_iir::Biquad;
use gpui::*;
use sotf_audio_player::EQFilter;
use std::panic;
use std::sync::Arc;

/// Default sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// GPU-accelerated EQ frequency response curve element
///
/// Renders a smooth frequency response curve using GPU path rendering
/// for maximum performance and visual quality.
pub struct EQCurveElement {
    /// EQ filters to visualize
    filters: Arc<[EQFilter]>,
    /// Frequency axis range (Hz)
    freq_range: (f64, f64),
    /// dB axis range
    db_range: (f64, f64),
    /// Number of points to sample for the curve
    num_points: usize,
    /// Colors for the curve
    colors: EQCurveColors,
    /// Whether to fill under the curve
    fill_curve: bool,
    /// Height of the element
    height: Pixels,
}

/// Colors used by the EQ curve
#[derive(Clone)]
pub struct EQCurveColors {
    /// Background color
    pub background: Rgba,
    /// Grid line color
    pub grid: Rgba,
    /// Curve stroke color for boost
    pub curve_boost: Rgba,
    /// Curve stroke color for cut
    pub curve_cut: Rgba,
    /// Fill color for boost area
    pub fill_boost: Rgba,
    /// Fill color for cut area
    pub fill_cut: Rgba,
    /// Zero line color
    pub zero_line: Rgba,
}

impl Default for EQCurveColors {
    fn default() -> Self {
        Self {
            background: rgb(0x1a1a1a),
            grid: rgba(0xffffff20),
            curve_boost: rgb(0x22c55e),
            curve_cut: rgb(0xef4444),
            fill_boost: rgba(0x22c55e40),
            fill_cut: rgba(0xef444440),
            zero_line: rgba(0xffffff40),
        }
    }
}

impl EQCurveElement {
    /// Create a new EQ curve element with the given filters
    pub fn new(filters: impl Into<Arc<[EQFilter]>>) -> Self {
        Self {
            filters: filters.into(),
            freq_range: (20.0, 20000.0),
            db_range: (-24.0, 24.0),
            num_points: 128,
            colors: EQCurveColors::default(),
            fill_curve: true,
            height: px(200.0),
        }
    }

    /// Set the frequency range
    pub fn frequency_range(mut self, min: f64, max: f64) -> Self {
        self.freq_range = (min, max);
        self
    }

    /// Set the dB range
    pub fn db_range(mut self, min: f64, max: f64) -> Self {
        self.db_range = (min, max);
        self
    }

    /// Set the number of sample points
    pub fn num_points(mut self, num: usize) -> Self {
        self.num_points = num.max(16);
        self
    }

    /// Set custom colors
    pub fn colors(mut self, colors: EQCurveColors) -> Self {
        self.colors = colors;
        self
    }

    /// Set whether to fill under the curve
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill_curve = fill;
        self
    }

    /// Set the height
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// Convert frequency to normalized position (0.0 to 1.0, logarithmic)
    fn freq_to_x(&self, freq: f64) -> f64 {
        let log_min = self.freq_range.0.log10();
        let log_max = self.freq_range.1.log10();
        let log_freq = freq.log10();
        (log_freq - log_min) / (log_max - log_min)
    }

    /// Convert normalized position to frequency (logarithmic)
    fn x_to_freq(&self, x: f64) -> f64 {
        let log_min = self.freq_range.0.log10();
        let log_max = self.freq_range.1.log10();
        let log_freq = log_min + x * (log_max - log_min);
        10.0_f64.powf(log_freq)
    }

    /// Convert dB to normalized position (0.0 = top, 1.0 = bottom)
    fn db_to_y(&self, db: f64) -> f64 {
        let (min_db, max_db) = self.db_range;
        1.0 - (db - min_db) / (max_db - min_db)
    }

    /// Calculate the combined response in dB at a given frequency
    fn calculate_response(&self, freq: f64) -> f64 {
        if self.filters.is_empty() {
            return 0.0;
        }
        self.filters
            .iter()
            .map(|f| {
                let biquad = Biquad::new(
                    f.filter_type.clone(),
                    f.frequency,
                    SAMPLE_RATE,
                    f.q,
                    f.gain_db,
                );
                biquad.log_result(freq)
            })
            .sum()
    }

    /// Generate frequency response points
    fn generate_curve_points(&self, bounds: &Bounds<Pixels>) -> Vec<(f64, f64)> {
        let origin_x: f32 = bounds.origin.x.into();
        let origin_y: f32 = bounds.origin.y.into();
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();

        (0..=self.num_points)
            .map(|i| {
                let x_norm = i as f64 / self.num_points as f64;
                let freq = self.x_to_freq(x_norm);
                let db = self.calculate_response(freq);
                let y_norm = self.db_to_y(db);

                let x = origin_x as f64 + x_norm * width as f64;
                let y = origin_y as f64 + y_norm * height as f64;
                (x, y)
            })
            .collect()
    }
}

impl IntoElement for EQCurveElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EQCurveElement {
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
        let layout_id = window.request_layout(
            Style {
                size: size(relative(1.0).into(), self.height.into()),
                min_size: size(px(100.0).into(), px(80.0).into()),
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
        // Paint background
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(px(4.0)),
            background: self.colors.background.into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

        // Paint grid lines
        self.paint_grid(bounds, window);

        // Paint zero line (0 dB reference)
        let zero_y = bounds.origin.y + bounds.size.height * self.db_to_y(0.0) as f32;
        window.paint_quad(PaintQuad {
            bounds: Bounds {
                origin: point(bounds.origin.x, zero_y - px(0.5)),
                size: size(bounds.size.width, px(1.0)),
            },
            corner_radii: Corners::default(),
            background: self.colors.zero_line.into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

        // Generate curve points
        let curve_points = self.generate_curve_points(&bounds);

        if self.fill_curve {
            // Paint filled areas (boost and cut separately)
            self.paint_filled_curve(&curve_points, bounds, window);
        }

        // Paint the curve as a series of small line segments (quads)
        self.paint_curve_line(&curve_points, bounds, window);
    }
}

impl EQCurveElement {
    /// Paint grid lines for frequency and dB reference
    fn paint_grid(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        // Frequency grid lines (octave-based)
        let freq_marks = [
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ];
        for &freq in &freq_marks {
            let x_norm = self.freq_to_x(freq) as f32;
            if x_norm >= 0.0 && x_norm <= 1.0 {
                let x = bounds.origin.x + bounds.size.width * x_norm;
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(x - px(0.5), bounds.origin.y),
                        size: size(px(1.0), bounds.size.height),
                    },
                    corner_radii: Corners::default(),
                    background: self.colors.grid.into(),
                    border_widths: Edges::default(),
                    border_color: Hsla::transparent_black(),
                    border_style: Default::default(),
                });
            }
        }

        // dB grid lines
        let db_marks = [-24.0, -18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0, 24.0];
        for &db in &db_marks {
            if db >= self.db_range.0 && db <= self.db_range.1 {
                let y_norm = self.db_to_y(db) as f32;
                let y = bounds.origin.y + bounds.size.height * y_norm;
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(bounds.origin.x, y - px(0.5)),
                        size: size(bounds.size.width, px(1.0)),
                    },
                    corner_radii: Corners::default(),
                    background: self.colors.grid.into(),
                    border_widths: Edges::default(),
                    border_color: Hsla::transparent_black(),
                    border_style: Default::default(),
                });
            }
        }
    }

    /// Paint the curve as filled polygons (boost above 0dB, cut below 0dB)
    fn paint_filled_curve(
        &self,
        points: &[(f64, f64)],
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) {
        let origin_y: f32 = bounds.origin.y.into();
        let height: f32 = bounds.size.height.into();
        let zero_y = (origin_y + height * self.db_to_y(0.0) as f32) as f64;

        // For each segment, fill a quad from zero line to curve
        for i in 0..points.len().saturating_sub(1) {
            let (x1, y1) = points[i];
            let (x2, y2) = points[i + 1];

            // Determine if this segment is boost or cut
            let mid_y = (y1 + y2) / 2.0;
            let is_boost = mid_y < zero_y;

            let fill_color = if is_boost {
                self.colors.fill_boost
            } else {
                self.colors.fill_cut
            };

            // Create a quad from x1,zero_y to x2,zero_y to x2,y2 to x1,y1
            // For simplicity, we'll just draw a rectangle approximation
            let top_y = y1.min(y2).min(zero_y);
            let bottom_y = y1.max(y2).max(zero_y);

            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(px(x1 as f32), px(top_y as f32)),
                    size: size(px((x2 - x1) as f32), px((bottom_y - top_y) as f32)),
                },
                corner_radii: Corners::default(),
                background: fill_color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }
    }

    /// Paint the curve line using small quads
    fn paint_curve_line(&self, points: &[(f64, f64)], bounds: Bounds<Pixels>, window: &mut Window) {
        let origin_y: f32 = bounds.origin.y.into();
        let height: f32 = bounds.size.height.into();
        let zero_y = (origin_y + height * self.db_to_y(0.0) as f32) as f64;
        let line_width = 2.0;

        for i in 0..points.len().saturating_sub(1) {
            let (x1, y1) = points[i];
            let (x2, y2) = points[i + 1];

            // Determine color based on whether we're above or below zero
            let mid_y = (y1 + y2) / 2.0;
            let curve_color = if mid_y < zero_y {
                self.colors.curve_boost
            } else {
                self.colors.curve_cut
            };

            // Calculate line segment
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();

            if len < 0.001 {
                continue;
            }

            // Perpendicular vector for line thickness
            let _nx = -dy / len * line_width / 2.0;
            let _ny = dx / len * line_width / 2.0;

            // For a simple approximation, draw a thin rectangle
            let min_x = x1.min(x2);
            let max_x = x1.max(x2);
            let min_y = (y1 - line_width / 2.0).min(y2 - line_width / 2.0);
            let max_y = (y1 + line_width / 2.0).max(y2 + line_width / 2.0);

            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(px(min_x as f32), px(min_y as f32)),
                    size: size(
                        px((max_x - min_x).max(line_width) as f32),
                        px((max_y - min_y).max(line_width) as f32),
                    ),
                },
                corner_radii: Corners::default(),
                background: curve_color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }
    }
}

/// Compact EQ curve for inline display (e.g., in plugin list)
pub struct CompactEQCurve {
    /// EQ filters to visualize
    filters: Arc<[EQFilter]>,
    /// Width of the element
    width: Pixels,
    /// Height of the element
    height: Pixels,
}

impl CompactEQCurve {
    /// Create a new compact EQ curve
    pub fn new(filters: impl Into<Arc<[EQFilter]>>) -> Self {
        Self {
            filters: filters.into(),
            width: px(80.0),
            height: px(40.0),
        }
    }

    /// Set the size
    pub fn size(mut self, width: Pixels, height: Pixels) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

impl IntoElement for CompactEQCurve {
    type Element = EQCurveElement;

    fn into_element(self) -> Self::Element {
        EQCurveElement::new(self.filters)
            .height(self.height)
            .num_points(32)
            .fill(false)
    }
}
