use super::misc::compute_transfer;
use gpui::prelude::*;
use gpui::*;

/// Custom element that paints a smooth dynamics transfer curve using PathBuilder.
///
/// Features:
/// - Smooth curve (64 sample points, no staircase)
/// - Filled area under curve with gradient opacity
/// - Grid lines with dB scale
/// - Unity gain reference line (diagonal)
/// - Animated operating point dot
pub(super) struct TransferCurveElement {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) threshold_db: f64,
    pub(super) ratio: f64,
    pub(super) knee_db: f64,
    pub(super) is_limiter: bool,
    pub(super) input_level_db: Option<f64>,
    pub(super) accent: Rgba,
    pub(super) compressed_color: Rgba,
    pub(super) operating_point_color: Rgba,
    pub(super) bg: Rgba,
    pub(super) grid_color: Rgba,
    pub(super) text_color: Rgba,
}

impl IntoElement for TransferCurveElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TransferCurveElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
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
                size: Size {
                    width: px(self.width).into(),
                    height: px(self.height).into(),
                },
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
        let ox = bounds.origin.x;
        let oy = bounds.origin.y;
        let w = self.width;
        let h = self.height;
        let pad = 4.0; // Inner padding

        // Background
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(px(8.0)),
            background: self.bg.into(),
            border_widths: Edges::all(px(1.0)),
            border_color: self.grid_color.into(),
            border_style: BorderStyle::default(),
        });

        // Helper: dB to pixel coordinates
        // Input range: -60 to 0 dB → x: pad to w-pad
        // Output range: -60 to 0 dB → y: h-pad (bottom) to pad (top)
        let db_to_x = |db: f64| -> f32 {
            let norm = ((db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
            pad + norm * (w - 2.0 * pad)
        };
        let db_to_y = |db: f64| -> f32 {
            let norm = ((db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
            (h - pad) - norm * (h - 2.0 * pad)
        };

        // Grid lines (-48, -36, -24, -12, 0 dB)
        let grid_line_color = Rgba {
            r: self.grid_color.r,
            g: self.grid_color.g,
            b: self.grid_color.b,
            a: 0.3,
        };
        for &db in &[-48.0, -36.0, -24.0, -12.0] {
            let y_pos = db_to_y(db);
            // Horizontal grid line
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(pad), oy + px(y_pos)),
                    size: size(px(w - 2.0 * pad), px(1.0)),
                },
                corner_radii: Corners::default(),
                background: grid_line_color.into(),
                border_widths: Edges::default(),
                border_color: gpui::transparent_black(),
                border_style: BorderStyle::default(),
            });
            // Vertical grid line
            let x_pos = db_to_x(db);
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(x_pos), oy + px(pad)),
                    size: size(px(1.0), px(h - 2.0 * pad)),
                },
                corner_radii: Corners::default(),
                background: grid_line_color.into(),
                border_widths: Edges::default(),
                border_color: gpui::transparent_black(),
                border_style: BorderStyle::default(),
            });
        }

        // Unity gain reference line (diagonal from -60,-60 to 0,0)
        let unity_color = Rgba {
            r: self.text_color.r,
            g: self.text_color.g,
            b: self.text_color.b,
            a: 0.2,
        };
        {
            let mut builder = PathBuilder::fill();
            let line_w = 1.0_f32;
            // Draw diagonal as a thin parallelogram
            builder.move_to(point(ox + px(db_to_x(-60.0)), oy + px(db_to_y(-60.0))));
            builder.line_to(point(
                ox + px(db_to_x(-60.0) + line_w),
                oy + px(db_to_y(-60.0)),
            ));
            builder.line_to(point(ox + px(db_to_x(0.0) + line_w), oy + px(db_to_y(0.0))));
            builder.line_to(point(ox + px(db_to_x(0.0)), oy + px(db_to_y(0.0))));
            if let Ok(path) = builder.build() {
                window.paint_path(path, unity_color);
            }
        }

        // Transfer curve — smooth line with filled area underneath
        let num_points = 64;
        let mut curve_points: Vec<(f32, f32)> = Vec::with_capacity(num_points + 1);

        for i in 0..=num_points {
            let input_db = -60.0 + (i as f64 / num_points as f64) * 60.0;
            let output_db = compute_transfer(
                input_db,
                self.threshold_db,
                self.ratio,
                self.knee_db,
                self.is_limiter,
            );
            curve_points.push((db_to_x(input_db), db_to_y(output_db)));
        }

        // Filled area under curve (accent color at low opacity)
        {
            let fill_color = Rgba {
                r: self.accent.r,
                g: self.accent.g,
                b: self.accent.b,
                a: 0.15,
            };
            let mut builder = PathBuilder::fill();
            // Start at bottom-left
            builder.move_to(point(ox + px(curve_points[0].0), oy + px(h - pad)));
            // Up to curve start
            builder.line_to(point(
                ox + px(curve_points[0].0),
                oy + px(curve_points[0].1),
            ));
            // Along the curve
            for &(cx_pt, cy_pt) in &curve_points[1..] {
                builder.line_to(point(ox + px(cx_pt), oy + px(cy_pt)));
            }
            // Down to bottom-right
            if let Some(last) = curve_points.last() {
                builder.line_to(point(ox + px(last.0), oy + px(h - pad)));
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, fill_color);
            }
        }

        // Curve stroke (thicker line on top of the fill)
        {
            let stroke_width = 2.0_f32;
            // Draw the curve as a thin filled band (PathBuilder doesn't have stroke)
            let mut builder = PathBuilder::fill();

            // Forward pass (top edge of the band)
            builder.move_to(point(
                ox + px(curve_points[0].0),
                oy + px(curve_points[0].1 - stroke_width / 2.0),
            ));
            for &(cx_pt, cy_pt) in &curve_points[1..] {
                builder.line_to(point(ox + px(cx_pt), oy + px(cy_pt - stroke_width / 2.0)));
            }
            // Backward pass (bottom edge of the band)
            for &(cx_pt, cy_pt) in curve_points.iter().rev() {
                builder.line_to(point(ox + px(cx_pt), oy + px(cy_pt + stroke_width / 2.0)));
            }

            if let Ok(path) = builder.build() {
                window.paint_path(path, self.accent);
            }
        }

        // Threshold indicator — vertical dashed line at threshold
        {
            let thresh_x = db_to_x(self.threshold_db);
            let thresh_color = Rgba {
                r: self.compressed_color.r,
                g: self.compressed_color.g,
                b: self.compressed_color.b,
                a: 0.5,
            };
            // Draw as a series of small rectangles (dashed effect)
            let dash_len = 4.0_f32;
            let gap_len = 3.0_f32;
            let mut y_cur = pad;
            while y_cur < h - pad {
                let seg_h = dash_len.min(h - pad - y_cur);
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(ox + px(thresh_x), oy + px(y_cur)),
                        size: size(px(1.0), px(seg_h)),
                    },
                    corner_radii: Corners::default(),
                    background: thresh_color.into(),
                    border_widths: Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: BorderStyle::default(),
                });
                y_cur += dash_len + gap_len;
            }
        }

        // Operating point dot
        if let Some(input_db) = self.input_level_db {
            let output_db = compute_transfer(
                input_db,
                self.threshold_db,
                self.ratio,
                self.knee_db,
                self.is_limiter,
            );
            let dot_x = db_to_x(input_db);
            let dot_y = db_to_y(output_db);
            let dot_r = 5.0_f32;

            // Outer glow
            let glow_color = Rgba {
                r: self.operating_point_color.r,
                g: self.operating_point_color.g,
                b: self.operating_point_color.b,
                a: 0.3,
            };
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(dot_x - dot_r - 2.0), oy + px(dot_y - dot_r - 2.0)),
                    size: size(px((dot_r + 2.0) * 2.0), px((dot_r + 2.0) * 2.0)),
                },
                corner_radii: Corners::all(px(dot_r + 2.0)),
                background: glow_color.into(),
                border_widths: Edges::default(),
                border_color: gpui::transparent_black(),
                border_style: BorderStyle::default(),
            });

            // Inner dot
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(ox + px(dot_x - dot_r), oy + px(dot_y - dot_r)),
                    size: size(px(dot_r * 2.0), px(dot_r * 2.0)),
                },
                corner_radii: Corners::all(px(dot_r)),
                background: self.operating_point_color.into(),
                border_widths: Edges::all(px(1.5)),
                border_color: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.8,
                }
                .into(),
                border_style: BorderStyle::default(),
            });

            // Crosshair lines from dot to axes
            let crosshair_color = Rgba {
                r: self.operating_point_color.r,
                g: self.operating_point_color.g,
                b: self.operating_point_color.b,
                a: 0.25,
            };
            // Vertical line (to x-axis) — only draw if there's space below the dot
            let vert_line_h = (h - pad) - dot_y - dot_r;
            if vert_line_h > 1.0 {
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(ox + px(dot_x), oy + px(dot_y + dot_r)),
                        size: size(px(1.0), px(vert_line_h)),
                    },
                    corner_radii: Corners::default(),
                    background: crosshair_color.into(),
                    border_widths: Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
            // Horizontal line (to y-axis) — only draw if there's space left of the dot
            let horiz_line_w = dot_x - pad - dot_r;
            if horiz_line_w > 1.0 {
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(ox + px(pad), oy + px(dot_y)),
                        size: size(px(horiz_line_w), px(1.0)),
                    },
                    corner_radii: Corners::default(),
                    background: crosshair_color.into(),
                    border_widths: Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: BorderStyle::default(),
                });
            }
        }
    }
}
