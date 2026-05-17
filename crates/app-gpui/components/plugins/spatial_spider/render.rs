//! GPUI elements for the spatial spider visualizer.
//!
//! Two flavours:
//!
//! - [`SpiderDisc2D`]: top-down horizontal disc (`gpui::PathBuilder` paths).
//! - [`SpiderView3D`]: two intersecting reference planes drawn through
//!   [`d3rs::gpu3d::Lines3DElement`] so we inherit orbit / pan / zoom.
//!
//! Both consume the polygon geometry built by [`crate::components::plugins::spatial_spider::data`].
//! The renderer is decoupled from the underlying audio plumbing — the
//! plugin UI is responsible for materialising the [`ChannelMetric`] and
//! re-painting the element every refresh.

use super::data::{
    ChannelMetric, SpeakerVertex, SpiderPolygon, compute_polygon_2d, compute_polygon_3d,
};
use super::{SpatialSpiderSnapshot, SpiderMode, SpiderViewMode};
use crate::app::AppState;
use crate::components::design::Ds;
use d3rs::gpu3d::{Line3D, Lines3DElement, Lines3DScene, Lines3DState, Polygon3D};
use d3rs::text::{measure_glyph_text_width, paint_glyph_text_at};
use glam::Vec3;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Select, SelectOption, SelectSize, StackSpacing, Toggle, ToggleStyle, VStack};
use sotf_plugins::speaker_config::{
    SpeakerConfig, get_speaker_config, get_speaker_config_by_channels,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Default reference radius (pixels) above which the polygon labels sit.
const LABEL_RADIUS_FACTOR: f32 = 1.07;
/// Speaker dot radius in pixels.
const SPEAKER_DOT_PX: f32 = 4.0;
/// Concentric grid radii (fraction of unit). 1.0 marks 0 dB / |r|=1.
const GRID_RING_FRACTIONS: &[f32] = &[0.25, 0.5, 0.75, 1.0];
/// Radial rays drawn every N degrees.
const RAY_STEP_DEG: f32 = 30.0;

/// Colour palette used by both 2D and 3D renderers. Keeping it in one
/// struct means the plugin UIs can tint a single value (e.g. the polygon
/// fill) and inherit the rest.
#[derive(Debug, Clone)]
pub struct SpiderColors {
    pub background: Rgba,
    pub grid: Rgba,
    pub polygon_fill: Rgba,
    pub polygon_stroke: Rgba,
    pub speaker_dot: Rgba,
    pub label: Rgba,
    /// Tint for vertices with negative signed value (e.g. anti-phase
    /// correlation). Renderer interpolates between `polygon_stroke` and
    /// this colour by `|signed_value|` to flag anti-phase channels.
    pub negative_value: Rgba,
}

impl Default for SpiderColors {
    fn default() -> Self {
        Self {
            background: rgba(0x14181eff),
            grid: rgba(0x3a414cff),
            polygon_fill: rgba(0x4ea1ff40), // translucent blue
            polygon_stroke: rgba(0x4ea1ffff),
            speaker_dot: rgba(0xe6ecf4ff),
            label: rgba(0xe6ecf4ff),
            negative_value: rgba(0xff5a5aff),
        }
    }
}

impl SpiderColors {
    /// Build a spider palette that flows from the active `Theme`. Use this
    /// instead of `default()` whenever a `Theme` is in scope so light themes
    /// don't show a jarring dark patch.
    ///
    /// - `background` follows `theme.surface` (one step down from the panel).
    /// - `grid` follows `theme.border` — same hairlines as other charts.
    /// - `polygon_fill` is a translucent rendering of `theme.accent`.
    /// - `polygon_stroke` and labels use the same accent / text colors as
    ///   the rest of the panel for visual continuity.
    /// - `negative_value` (anti-phase tint) uses `theme.error` so it reads
    ///   as "alarm" without clashing with the rest of the palette.
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            background: theme.surface,
            grid: theme.border,
            polygon_fill: with_alpha(theme.accent, 0.25),
            polygon_stroke: theme.accent,
            speaker_dot: theme.text_primary,
            label: theme.text_secondary,
            negative_value: theme.error,
        }
    }
}

fn with_alpha(c: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    gpui::Rgba { a: alpha, ..c }
}

// ============================================================================
// 2D — horizontal disc
// ============================================================================

/// GPUI element drawing the 2D spider directly via `gpui::PathBuilder`.
///
/// The element is *self-contained* — it computes the polygon every paint
/// from the supplied config + metric so callers can pass `&LoudnessData`
/// directly each frame without intermediate state.
pub struct SpiderDisc2D<'a> {
    config: &'a SpeakerConfig,
    metric: ChannelMetric<'a>,
    colors: SpiderColors,
    /// Optional: show channel labels at the outer ring.
    show_labels: bool,
    /// Optional channel index to highlight (drawn with an extra ring around
    /// the speaker dot). Used to indicate the active correlation reference.
    highlight_channel: Option<usize>,
}

impl<'a> SpiderDisc2D<'a> {
    pub fn new(config: &'a SpeakerConfig, metric: ChannelMetric<'a>) -> Self {
        Self {
            config,
            metric,
            colors: SpiderColors::default(),
            show_labels: true,
            highlight_channel: None,
        }
    }

    pub fn colors(mut self, colors: SpiderColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn show_labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    /// Draw an extra ring around the dot for `channel` to mark it (e.g. the
    /// active correlation reference).
    pub fn highlight_channel(mut self, channel: Option<usize>) -> Self {
        self.highlight_channel = channel;
        self
    }

    /// Convert (azimuth_deg, radius_unit) to screen pixels.
    ///
    /// Convention from `speaker_config.rs`: `0° = front (+Y world)`,
    /// `+90° = left`. On screen we want front = top, left = left, so:
    ///   `screen_x = cx - radius * sin(azimuth)`
    ///   `screen_y = cy - radius * cos(azimuth)`
    fn polar_to_screen(cx: f32, cy: f32, radius_px: f32, azimuth_deg: f32) -> (f32, f32) {
        let az = azimuth_deg.to_radians();
        (cx - radius_px * az.sin(), cy - radius_px * az.cos())
    }
}

impl IntoElement for SpiderDisc2D<'_> {
    type Element = AnyElement;
    fn into_element(self) -> Self::Element {
        // Capture the input once and move into a custom paint element via
        // a closure-flavoured `Element`. Because `SpiderDisc2D` borrows the
        // config and metric for its lifetime, we eagerly compute the polygon
        // into owned data and hand it off.
        let polygon = compute_polygon_2d(self.config, self.metric);
        // Collect (label, azimuth) for every non-LFE non-height speaker so
        // we can both label them at the rim *and* draw a faint radial guide
        // line from the centre through each one — this makes it unambiguous
        // that polygon dots sit on the same ray as their label.
        let labels: Vec<(String, f32)> = self
            .config
            .speakers
            .iter()
            .filter(|s| !s.is_lfe && s.elevation.abs() < 1.0)
            .map(|s| (s.label.to_string(), s.azimuth))
            .collect();
        // The LFE has no direction, so it can't appear on the polygon. Render
        // it as a small coloured ring at the disc centre with an "LFE" label
        // so it stands out against the regular speaker dots.
        let has_lfe = self.config.speakers.iter().any(|s| s.is_lfe);
        SpiderDisc2DInner {
            polygon,
            labels,
            has_lfe,
            show_labels: self.show_labels,
            highlight_channel: self.highlight_channel,
            colors: self.colors,
        }
        .into_any_element()
    }
}

struct SpiderDisc2DInner {
    polygon: Vec<SpeakerVertex>,
    labels: Vec<(String, f32)>,
    has_lfe: bool,
    show_labels: bool,
    highlight_channel: Option<usize>,
    colors: SpiderColors,
}

impl IntoElement for SpiderDisc2DInner {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SpiderDisc2DInner {
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
        let style = Style {
            size: Size {
                width: relative(1.0).into(),
                height: relative(1.0).into(),
            },
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
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
        let w: f32 = bounds.size.width.into();
        let h: f32 = bounds.size.height.into();
        if w <= 4.0 || h <= 4.0 {
            return;
        }
        let cx = f32::from(bounds.origin.x) + w * 0.5;
        let cy = f32::from(bounds.origin.y) + h * 0.5;
        let unit = (w.min(h) * 0.5) - SPEAKER_DOT_PX * 3.0; // leave margin for dots/labels

        // Background.
        paint_filled_rect(window, bounds, self.colors.background);

        // Concentric rings — approximated as N-gons (64 segments).
        for &frac in GRID_RING_FRACTIONS {
            let r = unit * frac;
            paint_circle(window, cx, cy, r, 1.0, self.colors.grid, 64);
        }

        // Radial rays at every RAY_STEP_DEG (universal grid).
        let mut deg = -180.0;
        while deg < 180.0 {
            let (sx, sy) = SpiderDisc2D::polar_to_screen(cx, cy, unit, deg);
            paint_line(window, cx, cy, sx, sy, 1.0, self.colors.grid);
            deg += RAY_STEP_DEG;
        }

        // Per-speaker guide rays — drawn slightly brighter than the grid so
        // it's visually unambiguous which line the polygon vertex sits on.
        // This dispels the optical illusion where a low-radius dot near the
        // centre looks like it might be at a different angle than the label
        // at the rim.
        let guide_color = blend(self.colors.grid, self.colors.label, 0.35);
        for (_, az_deg) in &self.labels {
            let (sx, sy) =
                SpiderDisc2D::polar_to_screen(cx, cy, unit * LABEL_RADIUS_FACTOR, *az_deg);
            paint_line(window, cx, cy, sx, sy, 1.0, guide_color);
        }

        // Spider polygon (fill + stroke). Skip if fewer than 3 vertices to
        // avoid drawing a degenerate fill triangle.
        if self.polygon.len() >= 3 {
            let pts: Vec<(f32, f32)> = self
                .polygon
                .iter()
                .map(|v| SpiderDisc2D::polar_to_screen(cx, cy, unit * v.radius, v.azimuth_deg))
                .collect();
            paint_closed_polygon(
                window,
                &pts,
                Some(self.colors.polygon_fill),
                Some((self.colors.polygon_stroke, 1.5)),
            );
        }

        // Speaker dots.
        for v in &self.polygon {
            let (sx, sy) = SpiderDisc2D::polar_to_screen(cx, cy, unit * v.radius, v.azimuth_deg);
            // Tint anti-phase vertices toward `negative_value` proportional
            // to |signed_value|. For SPL mode (signed = dBTP), the sign is
            // always non-negative once mapped so this is a no-op.
            let dot_color = if v.signed_value < 0.0 {
                blend(self.colors.speaker_dot, self.colors.negative_value, 0.7)
            } else {
                self.colors.speaker_dot
            };
            paint_filled_disc(window, sx, sy, SPEAKER_DOT_PX, dot_color, 16);

            // Outline ring marking this dot as the highlighted (reference)
            // channel. Drawn AFTER the fill so it sits cleanly on top.
            if self.highlight_channel == Some(v.channel) {
                paint_circle(
                    window,
                    sx,
                    sy,
                    SPEAKER_DOT_PX * 2.0,
                    1.5,
                    self.colors.polygon_stroke,
                    24,
                );
            }
        }

        // Labels.
        for (label, az_deg) in &self.labels {
            let label_r = unit * LABEL_RADIUS_FACTOR;
            let (lx, ly) = SpiderDisc2D::polar_to_screen(cx, cy, label_r, *az_deg);
            let font_size = 10.0;
            let text_w = measure_glyph_text_width(label, font_size);
            let x = lx - text_w * 0.5;
            let y = ly - font_size * 0.5;
            paint_glyph_text_at(window, label, x, y, font_size, self.colors.label, 0.0);
        }

        // LFE marker: a coloured ring at the disc centre. The LFE has no
        // directional anchor, so this is purely an indicator that the
        // channel exists in the layout — using `polygon_stroke` (the same
        // accent colour the polygon outline uses) so it stands out
        // unambiguously from the regular speaker dots.
        if self.has_lfe {
            let lfe_r = SPEAKER_DOT_PX * 1.4;
            // Filled translucent core + opaque outline ring.
            paint_filled_disc(
                window,
                cx,
                cy,
                lfe_r,
                with_alpha(self.colors.polygon_stroke, 0.25),
                24,
            );
            paint_circle(window, cx, cy, lfe_r, 1.5, self.colors.polygon_stroke, 24);
            if self.show_labels {
                let font_size = 9.0;
                let text = "LFE";
                let text_w = measure_glyph_text_width(text, font_size);
                paint_glyph_text_at(
                    window,
                    text,
                    cx - text_w * 0.5,
                    cy + lfe_r + 4.0,
                    font_size,
                    self.colors.label,
                    0.0,
                );
            }
        }
    }
}

// ============================================================================
// 3D — two intersecting planes
// ============================================================================

/// 3D spider view. Owns a shared [`Lines3DState`] so the parent view can
/// drive orbit / pan / zoom from mouse events.
pub struct SpiderView3D<'a> {
    config: &'a SpeakerConfig,
    metric: ChannelMetric<'a>,
    state: Rc<RefCell<Lines3DState>>,
    colors: SpiderColors,
    /// Colour applied to the vertical-plane polygon. Default = orange so
    /// the horizontal (blue) and vertical planes read distinctly when
    /// overlapping.
    vertical_color: Rgba,
}

impl<'a> SpiderView3D<'a> {
    pub fn new(
        config: &'a SpeakerConfig,
        metric: ChannelMetric<'a>,
        state: Rc<RefCell<Lines3DState>>,
    ) -> Self {
        Self {
            config,
            metric,
            state,
            colors: SpiderColors::default(),
            vertical_color: rgba(0xffa050ff),
        }
    }

    pub fn colors(mut self, colors: SpiderColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn vertical_color(mut self, color: Rgba) -> Self {
        self.vertical_color = color;
        self
    }

    /// Build the wireframe reference frame: XYZ axes + unit-radius circle on
    /// each reference plane.
    fn reference_lines(colors: &SpiderColors) -> Vec<Line3D> {
        let grid = colors.grid;
        let mut out = Vec::new();
        // World axes (length 1.2 so the unit polygons still sit inside).
        out.push(Line3D {
            from: Vec3::new(-1.2, 0.0, 0.0),
            to: Vec3::new(1.2, 0.0, 0.0),
            color: grid,
            width: 1.0,
        });
        out.push(Line3D {
            from: Vec3::new(0.0, -1.2, 0.0),
            to: Vec3::new(0.0, 1.2, 0.0),
            color: grid,
            width: 1.0,
        });
        out.push(Line3D {
            from: Vec3::new(0.0, 0.0, -1.2),
            to: Vec3::new(0.0, 0.0, 1.2),
            color: grid,
            width: 1.0,
        });
        // Unit circle in the horizontal plane (z = 0).
        let n = 48;
        for i in 0..n {
            let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
            out.push(Line3D {
                from: Vec3::new(a0.cos(), a0.sin(), 0.0),
                to: Vec3::new(a1.cos(), a1.sin(), 0.0),
                color: grid,
                width: 1.0,
            });
        }
        // Unit circle in the vertical plane (x = 0).
        for i in 0..n {
            let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
            out.push(Line3D {
                from: Vec3::new(0.0, a0.cos(), a0.sin()),
                to: Vec3::new(0.0, a1.cos(), a1.sin()),
                color: grid,
                width: 1.0,
            });
        }
        out
    }

    fn polygon_from_vertices(
        vertices: &[SpeakerVertex],
        project: impl Fn(&SpeakerVertex) -> Vec3,
        fill: Rgba,
        stroke: Rgba,
    ) -> Polygon3D {
        Polygon3D {
            vertices: vertices.iter().map(&project).collect(),
            fill: Some(fill),
            stroke: Some((stroke, 1.5)),
        }
    }
}

impl IntoElement for SpiderView3D<'_> {
    type Element = AnyElement;
    fn into_element(self) -> Self::Element {
        let SpiderPolygon {
            horizontal,
            vertical,
            ..
        } = compute_polygon_3d(self.config, self.metric);

        // Horizontal plane: speaker on the unit circle direction scaled by
        // its radial value. Convention: speaker_config.to_cartesian returns
        // `[sin(az), cos(az), sin(el)]` so z = 0 for floor speakers.
        let horizontal_poly = Self::polygon_from_vertices(
            &horizontal,
            |v| Vec3::new(v.direction[0] * v.radius, v.direction[1] * v.radius, 0.0),
            self.colors.polygon_fill,
            self.colors.polygon_stroke,
        );
        // Vertical plane: project away X so vertices live in the YZ plane.
        // `direction = [sin(az), cos(az), sin(el)]`. Dropping X keeps the
        // Y/Z components; the centre speaker (az=0, el=0) lands at
        // (0, 1, 0) — the shared anchor point with the horizontal plane.
        let vertical_poly = Self::polygon_from_vertices(
            &vertical,
            |v| Vec3::new(0.0, v.direction[1] * v.radius, v.direction[2] * v.radius),
            translucent(self.vertical_color, 0.25),
            self.vertical_color,
        );

        let mut lines = Self::reference_lines(&self.colors);
        // Speaker "spokes" from origin out to each vertex on both planes —
        // visually anchors the polygon to the centre.
        for v in horizontal.iter() {
            lines.push(Line3D {
                from: Vec3::ZERO,
                to: Vec3::new(v.direction[0] * v.radius, v.direction[1] * v.radius, 0.0),
                color: self.colors.polygon_stroke,
                width: 1.0,
            });
        }
        for v in vertical.iter() {
            lines.push(Line3D {
                from: Vec3::ZERO,
                to: Vec3::new(0.0, v.direction[1] * v.radius, v.direction[2] * v.radius),
                color: self.vertical_color,
                width: 1.0,
            });
        }

        let scene = Lines3DScene {
            background: Some(self.colors.background),
            lines,
            polygons: vec![horizontal_poly, vertical_poly],
        };
        Lines3DElement::new(self.state, scene).into_any_element()
    }
}

// ============================================================================
// GPUI path helpers
// ============================================================================

fn paint_filled_rect(window: &mut Window, bounds: Bounds<Pixels>, color: Rgba) {
    let o = bounds.origin;
    let r = Point {
        x: o.x + bounds.size.width,
        y: o.y + bounds.size.height,
    };
    let mut b = PathBuilder::fill();
    b.move_to(Point { x: o.x, y: o.y });
    b.line_to(Point { x: r.x, y: o.y });
    b.line_to(Point { x: r.x, y: r.y });
    b.line_to(Point { x: o.x, y: r.y });
    b.line_to(Point { x: o.x, y: o.y });
    if let Ok(p) = b.build() {
        window.paint_path(p, color);
    }
}

fn paint_line(window: &mut Window, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Rgba) {
    let mut b = PathBuilder::stroke(px(width));
    b.move_to(Point {
        x: px(x0),
        y: px(y0),
    });
    b.line_to(Point {
        x: px(x1),
        y: px(y1),
    });
    if let Ok(p) = b.build() {
        window.paint_path(p, color);
    }
}

fn paint_circle(window: &mut Window, cx: f32, cy: f32, r: f32, width: f32, color: Rgba, segs: u32) {
    if r <= 0.5 || segs < 3 {
        return;
    }
    let mut b = PathBuilder::stroke(px(width));
    for i in 0..=segs {
        let a = i as f32 / segs as f32 * std::f32::consts::TAU;
        let x = cx + r * a.cos();
        let y = cy + r * a.sin();
        let p = Point { x: px(x), y: px(y) };
        if i == 0 {
            b.move_to(p);
        } else {
            b.line_to(p);
        }
    }
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

fn paint_filled_disc(window: &mut Window, cx: f32, cy: f32, r: f32, color: Rgba, segs: u32) {
    if r <= 0.0 || segs < 3 {
        return;
    }
    let mut b = PathBuilder::fill();
    for i in 0..=segs {
        let a = i as f32 / segs as f32 * std::f32::consts::TAU;
        let x = cx + r * a.cos();
        let y = cy + r * a.sin();
        let p = Point { x: px(x), y: px(y) };
        if i == 0 {
            b.move_to(p);
        } else {
            b.line_to(p);
        }
    }
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

fn paint_closed_polygon(
    window: &mut Window,
    pts: &[(f32, f32)],
    fill: Option<Rgba>,
    stroke: Option<(Rgba, f32)>,
) {
    if pts.len() < 3 {
        return;
    }
    if let Some(fill_color) = fill {
        let mut b = PathBuilder::fill();
        let (x0, y0) = pts[0];
        b.move_to(Point {
            x: px(x0),
            y: px(y0),
        });
        for &(x, y) in &pts[1..] {
            b.line_to(Point { x: px(x), y: px(y) });
        }
        b.line_to(Point {
            x: px(x0),
            y: px(y0),
        });
        if let Ok(p) = b.build() {
            window.paint_path(p, fill_color);
        }
    }
    if let Some((stroke_color, w)) = stroke {
        let mut b = PathBuilder::stroke(px(w));
        let (x0, y0) = pts[0];
        b.move_to(Point {
            x: px(x0),
            y: px(y0),
        });
        for &(x, y) in &pts[1..] {
            b.line_to(Point { x: px(x), y: px(y) });
        }
        b.line_to(Point {
            x: px(x0),
            y: px(y0),
        });
        if let Ok(p) = b.build() {
            window.paint_path(p, stroke_color);
        }
    }
}

fn blend(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba {
        r: a.r * (1.0 - t) + b.r * t,
        g: a.g * (1.0 - t) + b.g * t,
        b: a.b * (1.0 - t) + b.b * t,
        a: a.a * (1.0 - t) + b.a * t,
    }
}

fn translucent(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
}

// ============================================================================
// Shared panel — used by both the upmixer's custom Spatial tab and the
// generic ui_layout_renderer "spatial_spider" custom-viz hook so both paths
// stay in lockstep.
// ============================================================================

/// Render the complete spider panel (header + body). Both the upmixer
/// custom view and the layout-renderer custom-viz hook delegate here so
/// they share toggles, ref-channel selector, and palette.
///
/// - `speaker_config_id`: optional explicit speaker config id (e.g. "5.1.4")
///   when the host knows it. When `None`, we fall back to deriving the
///   layout from the loudness data's channel count.
pub fn render_spatial_spider_panel(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    snapshot: &SpatialSpiderSnapshot,
    speaker_config_id: Option<&str>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let cfg_opt = resolve_speaker_config(snapshot, speaker_config_id);
    let header = render_spatial_spider_controls(d, entity, plugin_idx, snapshot, cfg_opt, theme);
    let body = render_spatial_spider_graph(d, snapshot, cfg_opt, theme);

    VStack::new()
        .spacing(StackSpacing::Sm)
        .child(header)
        .child(body)
        .build()
        .into_any_element()
}

/// Render only the controls row (mode toggles + ref-channel selector). Use
/// when you want to host the graph separately from its controls (e.g. a
/// permanent graph row below a tab bar).
pub fn render_spatial_spider_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    build_header(
        d,
        entity,
        plugin_idx,
        snapshot.ui.spider_mode,
        snapshot.ui.view_mode,
        snapshot.ui.correlation_ref_channel,
        snapshot.ui.ref_channel_select_open,
        cfg_opt,
        theme,
    )
}

/// Render only the graph (no controls). Use when the controls live elsewhere
/// (e.g. embedded in a plugin's tab content) and you want the visualization
/// to occupy its own row.
pub fn render_spatial_spider_graph(
    d: &Ds,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    build_body(
        d,
        snapshot,
        cfg_opt,
        snapshot.ui.view_mode,
        snapshot.ui.spider_mode,
        snapshot.ui.correlation_ref_channel,
        theme,
    )
}

/// Resolve the speaker layout to render against. Explicit id wins, else
/// derive from the loudness data's channel count, else `None`.
pub fn resolve_speaker_config(
    snapshot: &SpatialSpiderSnapshot,
    speaker_config_id: Option<&str>,
) -> Option<&'static SpeakerConfig> {
    speaker_config_id.and_then(get_speaker_config).or_else(|| {
        snapshot
            .loudness
            .as_ref()
            .and_then(|li| get_speaker_config_by_channels(li.true_peaks_dbtp.len()))
    })
}

#[allow(clippy::too_many_arguments)]
fn build_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    spider_mode: SpiderMode,
    view_mode: SpiderViewMode,
    ref_channel: usize,
    ref_channel_select_open: bool,
    cfg_opt: Option<&'static SpeakerConfig>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let e_2d = entity.clone();
    let e_3d = entity.clone();
    let e_spl = entity.clone();
    let e_corr = entity.clone();

    div()
        .flex()
        .items_center()
        .gap(d.gap_md)
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child("Spatial Field".to_string()),
        )
        .child(
            // TODO: this widget today reflects the *chain output* (the last
            // permanent LoudnessMonitor), not the host plugin's own output.
            // When per-plugin analyzer hooks land, replace this label with
            // the host plugin's name so the source is unambiguous.
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child("(chain out)".to_string()),
        )
        .child(
            Toggle::new(("spider-view-2d", plugin_idx))
                .checked(view_mode == SpiderViewMode::Disc2D)
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .on_change(move |checked, _, cx| {
                    if checked {
                        e_2d.update(cx, |st, cx| {
                            st.app.spatial_spider.view_mode = SpiderViewMode::Disc2D;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child("2D".to_string()),
        )
        .child(
            Toggle::new(("spider-view-3d", plugin_idx))
                .checked(view_mode == SpiderViewMode::View3D)
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .on_change(move |checked, _, cx| {
                    if checked {
                        e_3d.update(cx, |st, cx| {
                            st.app.spatial_spider.view_mode = SpiderViewMode::View3D;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child("3D".to_string()),
        )
        .child(div().w(px(1.0)).h(px(14.0)).bg(theme.border))
        .child(
            Toggle::new(("spider-mode-spl", plugin_idx))
                .checked(matches!(spider_mode, SpiderMode::Spl))
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .on_change(move |checked, _, cx| {
                    if checked {
                        e_spl.update(cx, |st, cx| {
                            st.app.spatial_spider.spider_mode = SpiderMode::Spl;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child("SPL".to_string()),
        )
        .child(
            Toggle::new(("spider-mode-corr", plugin_idx))
                .checked(matches!(spider_mode, SpiderMode::CorrelationFromRef { .. }))
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .on_change({
                    let ref_ch = ref_channel;
                    move |checked, _, cx| {
                        if checked {
                            e_corr.update(cx, |st, cx| {
                                st.app.spatial_spider.spider_mode =
                                    SpiderMode::CorrelationFromRef {
                                        ref_channel: ref_ch,
                                    };
                                cx.notify();
                            });
                        }
                    }
                }),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child("Correlation".to_string()),
        )
        .child(build_ref_channel_select(
            d,
            entity,
            plugin_idx,
            spider_mode,
            ref_channel,
            ref_channel_select_open,
            cfg_opt,
            theme,
        ))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn build_ref_channel_select(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    spider_mode: SpiderMode,
    ref_channel: usize,
    is_open: bool,
    cfg_opt: Option<&'static SpeakerConfig>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let active = matches!(spider_mode, SpiderMode::CorrelationFromRef { .. });
    let cfg = match cfg_opt {
        Some(c) => c,
        None => return div().w(px(0.0)).into_any_element(),
    };
    let options: Vec<SelectOption> = cfg
        .speakers
        .iter()
        .filter(|s| !s.is_lfe)
        .map(|s| SelectOption::new(s.label.to_string(), s.label.to_string()))
        .collect();
    let selected_label = cfg
        .speakers
        .iter()
        .find(|s| s.channel == ref_channel && !s.is_lfe)
        .map(|s| s.label.to_string())
        .unwrap_or_else(|| {
            cfg.speakers
                .iter()
                .find(|s| !s.is_lfe)
                .map(|s| s.label.to_string())
                .unwrap_or_default()
        });

    div()
        .flex()
        .items_center()
        .gap(d.gap)
        .when(!active, |el| el.opacity(0.4))
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child("Ref:".to_string()),
        )
        .child(
            Select::new(("spider-ref-channel", plugin_idx))
                .options(options)
                .selected(selected_label)
                .is_open(is_open)
                .size(SelectSize::Xs)
                .theme(theme.to_select_theme())
                .on_toggle({
                    let entity = entity.clone();
                    move |open, _window, cx| {
                        entity.update(cx, |st, cx| {
                            st.app.spatial_spider.ref_channel_select_open = open;
                            cx.notify();
                        });
                    }
                })
                .on_change({
                    let entity = entity.clone();
                    move |value, _window, cx| {
                        let picked = cfg
                            .speakers
                            .iter()
                            .find(|s| s.label == value.as_ref() && !s.is_lfe)
                            .map(|s| s.channel)
                            .unwrap_or(0);
                        entity.update(cx, |st, cx| {
                            st.app.spatial_spider.correlation_ref_channel = picked;
                            if let SpiderMode::CorrelationFromRef { .. } =
                                st.app.spatial_spider.spider_mode
                            {
                                st.app.spatial_spider.spider_mode =
                                    SpiderMode::CorrelationFromRef {
                                        ref_channel: picked,
                                    };
                            }
                            // Close the dropdown after a selection so the
                            // next click reopens it cleanly.
                            st.app.spatial_spider.ref_channel_select_open = false;
                            cx.notify();
                        });
                    }
                }),
        )
        .into_any_element()
}

fn build_body(
    d: &Ds,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    view_mode: SpiderViewMode,
    spider_mode: SpiderMode,
    ref_channel: usize,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let cfg = match cfg_opt {
        None => {
            return div()
                .h(px(280.0))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child("Waiting for audio…".to_string()),
                )
                .into_any_element();
        }
        Some(c) => c,
    };
    let loudness = snapshot.loudness.as_ref();
    let n = cfg.total_channels;

    // SPL buffer.
    let metric_buf: Vec<f64> = match spider_mode {
        SpiderMode::Spl => loudness
            .map(|li| li.true_peaks_dbtp.iter().copied().collect())
            .unwrap_or_else(|| vec![f64::NEG_INFINITY; n]),
        SpiderMode::CorrelationFromRef { .. } => Vec::new(),
    };
    // Correlation row.
    let corr_row: Vec<f32> = match (spider_mode, loudness) {
        (SpiderMode::CorrelationFromRef { ref_channel: rc }, Some(li))
            if !li.correlation_matrix.is_empty() && li.correlation_samples_seen > 0 =>
        {
            let mc = li.correlation_matrix.len();
            let n_ch = (mc as f64).sqrt() as usize;
            if n_ch * n_ch == mc && rc < n_ch {
                let row_start = rc * n_ch;
                let row_end = row_start + n_ch;
                li.correlation_matrix
                    .get(row_start..row_end)
                    .map(|s| s.to_vec())
                    .unwrap_or_else(|| vec![0.0; n])
            } else {
                vec![0.0; n]
            }
        }
        (SpiderMode::CorrelationFromRef { .. }, _) => vec![0.0; n],
        _ => Vec::new(),
    };
    let metric = match spider_mode {
        SpiderMode::Spl => ChannelMetric::Spl(&metric_buf),
        SpiderMode::CorrelationFromRef { .. } => ChannelMetric::Correlation(&corr_row),
    };

    let palette = SpiderColors::from_theme(theme);
    let highlight =
        matches!(spider_mode, SpiderMode::CorrelationFromRef { .. }).then_some(ref_channel);
    // Container fixes both dimensions explicitly so the child's
    // `relative(1.0)` request resolves to a non-zero rect. Going through a
    // flex parent has bitten us before — `relative(1.0)` width inside a
    // flex row without an explicit flex_basis collapses to 0.
    let container = || div().h(px(320.0)).w_full();
    match view_mode {
        SpiderViewMode::Disc2D => container()
            .child(
                SpiderDisc2D::new(cfg, metric)
                    .colors(palette)
                    .highlight_channel(highlight),
            )
            .into_any_element(),
        SpiderViewMode::View3D => {
            // Wrap the 3D element in an interactive container so mouse
            // events drive the OrbitControls. State is shared via Rc so
            // every event handler mutates the same camera.
            let camera_state = snapshot.ui.camera_3d.clone();
            attach_orbit_handlers(container().id("spider-3d-viewport"), camera_state.clone())
                .child(SpiderView3D::new(cfg, metric, camera_state).colors(palette))
                .into_any_element()
        }
    }
}

/// Attach left-drag → rotate, middle-drag → pan, scroll → zoom handlers to
/// an interactive (id'd) div, mutating the supplied `Lines3DState` so the
/// next paint picks up the new camera.
fn attach_orbit_handlers(
    container: Stateful<Div>,
    state: Rc<RefCell<Lines3DState>>,
) -> Stateful<Div> {
    let s_down_l = state.clone();
    let s_down_m = state.clone();
    let s_move = state.clone();
    let s_up_l = state.clone();
    let s_up_m = state.clone();
    let s_scroll = state;

    container
        .on_mouse_down(MouseButton::Left, move |event, _window, _cx| {
            let mut st = s_down_l.borrow_mut();
            st.dragging = true;
            st.last_mouse = Some(event.position);
        })
        .on_mouse_down(MouseButton::Middle, move |event, _window, _cx| {
            let mut st = s_down_m.borrow_mut();
            st.panning = true;
            st.last_mouse = Some(event.position);
        })
        .on_mouse_move(move |event, _window, _cx| {
            let mut st = s_move.borrow_mut();
            let Some(last) = st.last_mouse else { return };
            let dx = f32::from(event.position.x - last.x);
            let dy = f32::from(event.position.y - last.y);
            if st.dragging {
                st.controls.rotate(dx, dy);
                st.update_camera();
            } else if st.panning {
                let camera = st.camera.clone();
                st.controls.pan(dx, dy, &camera);
                st.update_camera();
            }
            if st.dragging || st.panning {
                st.last_mouse = Some(event.position);
            }
        })
        .on_mouse_up(MouseButton::Left, move |_event, _window, _cx| {
            let mut st = s_up_l.borrow_mut();
            st.dragging = false;
            if !st.panning {
                st.last_mouse = None;
            }
        })
        .on_mouse_up(MouseButton::Middle, move |_event, _window, _cx| {
            let mut st = s_up_m.borrow_mut();
            st.panning = false;
            if !st.dragging {
                st.last_mouse = None;
            }
        })
        .on_scroll_wheel(move |event, _window, _cx| {
            let mut st = s_scroll.borrow_mut();
            // GPUI ScrollWheelEvent.delta is a ScrollDelta enum with Pixels
            // or Lines variants; both expose a vertical magnitude via `y`.
            // We normalise to a small unitless step suitable for OrbitControls.
            let step = match event.delta {
                ScrollDelta::Pixels(p) => f32::from(p.y) / 100.0,
                ScrollDelta::Lines(l) => l.y * 0.5,
            };
            st.controls.zoom(step);
            st.update_camera();
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polar_to_screen_front_is_up() {
        let (x, y) = SpiderDisc2D::polar_to_screen(100.0, 100.0, 10.0, 0.0);
        assert!((x - 100.0).abs() < 1e-4);
        // Front (azimuth 0°) maps to "up" — smaller y in screen space.
        assert!(y < 100.0, "expected y < 100, got {}", y);
    }

    #[test]
    fn polar_to_screen_left_is_left() {
        let (x, y) = SpiderDisc2D::polar_to_screen(100.0, 100.0, 10.0, 90.0);
        // Azimuth +90° = left → smaller x.
        assert!(x < 100.0, "expected x < 100, got {}", x);
        assert!((y - 100.0).abs() < 1e-4);
    }

    #[test]
    fn polar_to_screen_right_is_right() {
        let (x, y) = SpiderDisc2D::polar_to_screen(100.0, 100.0, 10.0, -90.0);
        assert!(x > 100.0, "expected x > 100, got {}", x);
        assert!((y - 100.0).abs() < 1e-4);
    }

    #[test]
    fn blend_endpoints_are_pure() {
        let a = Rgba {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 0.4,
        };
        let b = Rgba {
            r: 0.9,
            g: 0.8,
            b: 0.7,
            a: 0.6,
        };
        let m0 = blend(a, b, 0.0);
        let m1 = blend(a, b, 1.0);
        assert!((m0.r - a.r).abs() < 1e-6 && (m1.r - b.r).abs() < 1e-6);
    }

    #[test]
    fn translucent_overrides_only_alpha() {
        let a = Rgba {
            r: 0.3,
            g: 0.4,
            b: 0.5,
            a: 1.0,
        };
        let t = translucent(a, 0.2);
        assert_eq!(t.r, a.r);
        assert_eq!(t.g, a.g);
        assert_eq!(t.b, a.b);
        assert!((t.a - 0.2).abs() < 1e-6);
    }
}
