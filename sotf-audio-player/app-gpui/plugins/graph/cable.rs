//! Cable/Connection Rendering
//!
//! Renders bezier curve connections between node ports.

use gpui::prelude::*;
use gpui::*;

/// A cable connecting two ports with a bezier curve
pub struct CableElement {
    /// Start point (output port)
    pub from: Point<Pixels>,
    /// End point (input port)
    pub to: Point<Pixels>,
    /// Cable color
    pub color: Rgba,
    /// Whether this cable is selected
    pub selected: bool,
    /// Whether this is a preview cable (being dragged)
    pub preview: bool,
    /// Cable thickness
    pub thickness: f32,
}

impl CableElement {
    pub fn new(from: Point<Pixels>, to: Point<Pixels>) -> Self {
        Self {
            from,
            to,
            color: Rgba {
                r: 0.5,
                g: 0.7,
                b: 0.9,
                a: 1.0,
            },
            selected: false,
            preview: false,
            thickness: 2.5,
        }
    }

    pub fn color(mut self, color: Rgba) -> Self {
        self.color = color;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn preview(mut self, preview: bool) -> Self {
        self.preview = preview;
        if preview {
            self.color.a = 0.6;
        }
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// Calculate bezier control points for a smooth horizontal-biased curve
    fn control_points(&self) -> (Point<Pixels>, Point<Pixels>) {
        let from_x: f32 = self.from.x.into();
        let from_y: f32 = self.from.y.into();
        let to_x: f32 = self.to.x.into();
        let to_y: f32 = self.to.y.into();

        let dx = (to_x - from_x).abs();
        let offset = dx.max(50.0) * 0.5; // Horizontal bias

        let cp1 = point(px(from_x + offset), px(from_y));
        let cp2 = point(px(to_x - offset), px(to_y));

        (cp1, cp2)
    }

    /// Evaluate cubic bezier at parameter t
    fn bezier_point(&self, t: f32, cp1: Point<Pixels>, cp2: Point<Pixels>) -> Point<Pixels> {
        let from_x: f32 = self.from.x.into();
        let from_y: f32 = self.from.y.into();
        let to_x: f32 = self.to.x.into();
        let to_y: f32 = self.to.y.into();
        let cp1_x: f32 = cp1.x.into();
        let cp1_y: f32 = cp1.y.into();
        let cp2_x: f32 = cp2.x.into();
        let cp2_y: f32 = cp2.y.into();

        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        let x = uuu * from_x + 3.0 * uu * t * cp1_x + 3.0 * u * tt * cp2_x + ttt * to_x;
        let y = uuu * from_y + 3.0 * uu * t * cp1_y + 3.0 * u * tt * cp2_y + ttt * to_y;

        point(px(x), px(y))
    }
}

impl IntoElement for CableElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CableElement {
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
        // Cables don't participate in layout - they're painted absolutely
        let layout_id = window.request_layout(
            Style {
                position: gpui::Position::Absolute,
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
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let (cp1, cp2) = self.control_points();

        // Approximate bezier with line segments and render as thin quads
        let segments = 20;
        let thickness = if self.selected {
            self.thickness + 1.5
        } else {
            self.thickness
        };

        let color = if self.selected {
            Rgba {
                r: 1.0,
                g: 0.9,
                b: 0.3,
                a: self.color.a,
            }
        } else {
            self.color
        };

        for i in 0..segments {
            let t0 = i as f32 / segments as f32;
            let t1 = (i + 1) as f32 / segments as f32;

            let p0 = self.bezier_point(t0, cp1, cp2);
            let p1 = self.bezier_point(t1, cp1, cp2);

            // Calculate perpendicular offset for line thickness
            let dx: f32 = (p1.x - p0.x).into();
            let dy: f32 = (p1.y - p0.y).into();
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            let nx = -dy / len * thickness / 2.0;
            let ny = dx / len * thickness / 2.0;

            // Create a quad for this segment
            let p0_x: f32 = p0.x.into();
            let p0_y: f32 = p0.y.into();
            let p1_x: f32 = p1.x.into();
            let p1_y: f32 = p1.y.into();

            let min_x = (p0_x - nx.abs()).min(p1_x - nx.abs());
            let min_y = (p0_y - ny.abs()).min(p1_y - ny.abs());
            let max_x = (p0_x + nx.abs()).max(p1_x + nx.abs());
            let max_y = (p0_y + ny.abs()).max(p1_y + ny.abs());

            let segment_bounds = Bounds {
                origin: point(px(min_x), px(min_y)),
                size: size(px(max_x - min_x + 1.0), px(max_y - min_y + 1.0)),
            };

            // Draw a small quad for each segment
            window.paint_quad(PaintQuad {
                bounds: segment_bounds,
                corner_radii: Corners::all(px(thickness / 2.0)),
                background: color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }

        // Draw endpoint circles
        let endpoint_radius = thickness + 1.0;
        for point in [self.from, self.to] {
            let point_x: f32 = point.x.into();
            let point_y: f32 = point.y.into();
            let endpoint_bounds = Bounds {
                origin: gpui::point(
                    px(point_x - endpoint_radius),
                    px(point_y - endpoint_radius),
                ),
                size: size(px(endpoint_radius * 2.0), px(endpoint_radius * 2.0)),
            };
            window.paint_quad(PaintQuad {
                bounds: endpoint_bounds,
                corner_radii: Corners::all(px(endpoint_radius)),
                background: color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }
    }
}
