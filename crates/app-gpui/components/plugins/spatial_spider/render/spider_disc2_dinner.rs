use super::super::data::SpeakerVertex;
use super::consts::GRID_RING_FRACTIONS;
use super::consts::LABEL_FONT_PX;
use super::consts::LABEL_RADIUS_FACTOR;
use super::consts::LFE_LABEL_FONT_PX;
use super::consts::RAY_STEP_DEG;
use super::consts::SPEAKER_DOT_PX;
use super::consts::viewport_visual_scale;
use super::misc::blend;
use super::misc::with_alpha;
use super::paint::paint_circle;
use super::paint::paint_closed_polygon;
use super::paint::paint_filled_disc;
use super::paint::paint_filled_rect;
use super::paint::paint_line;
use super::spider_colors::SpiderColors;
use super::spider_disc2_d::SpiderDisc2D;
use d3rs::text::{measure_glyph_text_width, paint_glyph_text_at};
use gpui::prelude::*;
use gpui::*;

pub(super) struct SpiderDisc2DInner {
    pub(super) polygon: Vec<SpeakerVertex>,
    pub(super) labels: Vec<(String, f32)>,
    pub(super) has_lfe: bool,
    pub(super) show_labels: bool,
    pub(super) highlight_channel: Option<usize>,
    pub(super) colors: SpiderColors,
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
        let visual_scale = viewport_visual_scale(w.min(h));
        let dot_radius = SPEAKER_DOT_PX * visual_scale;
        let label_font_size = LABEL_FONT_PX * visual_scale;
        let label_extent = self
            .labels
            .iter()
            .map(|(label, _)| {
                measure_glyph_text_width(label, label_font_size) * 0.5 + label_font_size * 0.5
            })
            .fold(0.0_f32, f32::max);
        // Keep the label's full half-width and half-height inside the rim.
        // This margin scales with the actual rem-sized viewport rather than
        // assuming the default 320px graph height.
        let rim_margin = dot_radius * 3.0 + label_extent + 2.0 * visual_scale;
        let unit = ((w.min(h) * 0.5 - rim_margin) / LABEL_RADIUS_FACTOR).max(1.0);
        let line_width = visual_scale.clamp(0.75, 1.5);

        // Background.
        paint_filled_rect(window, bounds, self.colors.background);

        // Concentric rings — approximated as N-gons (64 segments).
        for &frac in GRID_RING_FRACTIONS {
            let r = unit * frac;
            paint_circle(window, cx, cy, r, line_width, self.colors.grid, 64);
        }

        // Radial rays at every RAY_STEP_DEG (universal grid).
        let mut deg = -180.0;
        while deg < 180.0 {
            let (sx, sy) = SpiderDisc2D::polar_to_screen(cx, cy, unit, deg);
            paint_line(window, cx, cy, sx, sy, line_width, self.colors.grid);
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
            paint_line(window, cx, cy, sx, sy, line_width, guide_color);
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
                Some((self.colors.polygon_stroke, 1.5 * visual_scale)),
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
            paint_filled_disc(window, sx, sy, dot_radius, dot_color, 16);

            // Outline ring marking this dot as the highlighted (reference)
            // channel. Drawn AFTER the fill so it sits cleanly on top.
            if self.highlight_channel == Some(v.channel) {
                paint_circle(
                    window,
                    sx,
                    sy,
                    dot_radius * 2.0,
                    line_width,
                    self.colors.polygon_stroke,
                    24,
                );
            }
        }

        // Labels.
        for (label, az_deg) in &self.labels {
            let label_r = unit * LABEL_RADIUS_FACTOR;
            let (lx, ly) = SpiderDisc2D::polar_to_screen(cx, cy, label_r, *az_deg);
            let font_size = label_font_size;
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
            let lfe_r = dot_radius * 1.4;
            // Filled translucent core + opaque outline ring.
            paint_filled_disc(
                window,
                cx,
                cy,
                lfe_r,
                with_alpha(self.colors.polygon_stroke, 0.25),
                24,
            );
            paint_circle(
                window,
                cx,
                cy,
                lfe_r,
                line_width,
                self.colors.polygon_stroke,
                24,
            );
            if self.show_labels {
                let font_size = LFE_LABEL_FONT_PX * visual_scale;
                let text = "LFE";
                let text_w = measure_glyph_text_width(text, font_size);
                paint_glyph_text_at(
                    window,
                    text,
                    cx - text_w * 0.5,
                    cy + lfe_r + 4.0 * visual_scale,
                    font_size,
                    self.colors.label,
                    0.0,
                );
            }
        }
    }
}
