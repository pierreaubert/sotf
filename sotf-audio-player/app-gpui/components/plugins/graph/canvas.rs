//! Graph Canvas Component
//!
//! A pannable, zoomable canvas for the plugin graph.

use gpui::prelude::*;
use gpui::*;

/// Grid configuration
const GRID_SIZE: f32 = 20.0;
const GRID_COLOR_MAJOR: Rgba = Rgba {
    r: 0.2,
    g: 0.2,
    b: 0.25,
    a: 1.0,
};
const GRID_COLOR_MINOR: Rgba = Rgba {
    r: 0.15,
    g: 0.15,
    b: 0.18,
    a: 1.0,
};

/// The graph canvas element with background grid
pub struct GraphCanvas {
    /// Current pan offset
    pub offset: Point<Pixels>,
    /// Current zoom level (0.5 to 2.0)
    pub zoom: f32,
    /// Background color
    pub background: Rgba,
    /// Whether to show the grid
    pub show_grid: bool,
}

impl Default for GraphCanvas {
    fn default() -> Self {
        Self {
            offset: point(px(0.0), px(0.0)),
            zoom: 1.0,
            background: Rgba {
                r: 0.08,
                g: 0.08,
                b: 0.10,
                a: 1.0,
            },
            show_grid: true,
        }
    }
}

impl GraphCanvas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn offset(mut self, offset: Point<Pixels>) -> Self {
        self.offset = offset;
        self
    }

    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom.clamp(0.5, 2.0);
        self
    }

    pub fn background(mut self, color: Rgba) -> Self {
        self.background = color;
        self
    }

    pub fn show_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// Convert screen coordinates to canvas coordinates
    pub fn screen_to_canvas(&self, screen_point: Point<Pixels>) -> Point<Pixels> {
        let x: f32 = screen_point.x.into();
        let y: f32 = screen_point.y.into();
        let offset_x: f32 = self.offset.x.into();
        let offset_y: f32 = self.offset.y.into();

        point(
            px((x - offset_x) / self.zoom),
            px((y - offset_y) / self.zoom),
        )
    }

    /// Convert canvas coordinates to screen coordinates
    pub fn canvas_to_screen(&self, canvas_point: Point<Pixels>) -> Point<Pixels> {
        let x: f32 = canvas_point.x.into();
        let y: f32 = canvas_point.y.into();
        let offset_x: f32 = self.offset.x.into();
        let offset_y: f32 = self.offset.y.into();

        point(px(x * self.zoom + offset_x), px(y * self.zoom + offset_y))
    }
}

impl IntoElement for GraphCanvas {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for GraphCanvas {
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
                size: size(relative(1.0).into(), relative(1.0).into()),
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
            corner_radii: Corners::default(),
            background: self.background.into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

        if !self.show_grid {
            return;
        }

        // Paint grid
        let grid_size = GRID_SIZE * self.zoom;
        let offset_x: f32 = self.offset.x.into();
        let offset_y: f32 = self.offset.y.into();
        let bounds_x: f32 = bounds.origin.x.into();
        let bounds_y: f32 = bounds.origin.y.into();
        let bounds_w: f32 = bounds.size.width.into();
        let bounds_h: f32 = bounds.size.height.into();

        // Calculate grid start positions
        let start_x = bounds_x + (offset_x % grid_size);
        let start_y = bounds_y + (offset_y % grid_size);

        // Vertical lines
        let mut x = start_x;
        let mut line_index = 0;
        while x < bounds_x + bounds_w {
            let is_major = line_index % 5 == 0;
            let color = if is_major {
                GRID_COLOR_MAJOR
            } else {
                GRID_COLOR_MINOR
            };

            let line_bounds = Bounds {
                origin: point(px(x), px(bounds_y)),
                size: size(px(1.0), px(bounds_h)),
            };

            window.paint_quad(PaintQuad {
                bounds: line_bounds,
                corner_radii: Corners::default(),
                background: color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });

            x += grid_size;
            line_index += 1;
        }

        // Horizontal lines
        let mut y = start_y;
        line_index = 0;
        while y < bounds_y + bounds_h {
            let is_major = line_index % 5 == 0;
            let color = if is_major {
                GRID_COLOR_MAJOR
            } else {
                GRID_COLOR_MINOR
            };

            let line_bounds = Bounds {
                origin: point(px(bounds_x), px(y)),
                size: size(px(bounds_w), px(1.0)),
            };

            window.paint_quad(PaintQuad {
                bounds: line_bounds,
                corner_radii: Corners::default(),
                background: color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });

            y += grid_size;
            line_index += 1;
        }
    }
}
