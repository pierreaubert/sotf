#[cfg(feature = "gpu-3d")]
use super::super::data::{ChannelMetric, SpeakerVertex, SpiderPolygon, compute_polygon_3d};
#[cfg(any(feature = "gpu-3d", test))]
use super::misc::translucent;
#[cfg(feature = "gpu-3d")]
use super::spider_colors::SpiderColors;
#[cfg(feature = "gpu-3d")]
use crate::theme::rgba;
#[cfg(feature = "gpu-3d")]
use d3rs::gpu3d::{Line3D, Lines3DElement, Lines3DScene, Lines3DState, Polygon3D};
#[cfg(feature = "gpu-3d")]
use glam::Vec3;
#[cfg(feature = "gpu-3d")]
use gpui::prelude::*;
#[cfg(feature = "gpu-3d")]
use gpui::*;
#[cfg(feature = "gpu-3d")]
use sotf_plugins::speaker_config::SpeakerConfig;
#[cfg(feature = "gpu-3d")]
use std::cell::RefCell;
#[cfg(feature = "gpu-3d")]
use std::rc::Rc;

/// 3D spider view. Owns a shared [`Lines3DState`] so the parent view can
/// drive orbit / pan / zoom from mouse events.
#[cfg(feature = "gpu-3d")]
pub struct SpiderView3D<'a> {
    pub(super) config: &'a SpeakerConfig,
    pub(super) metric: ChannelMetric<'a>,
    pub(super) state: Rc<RefCell<Lines3DState>>,
    pub(super) colors: SpiderColors,
    /// Colour applied to the vertical-plane polygon. Default = orange so
    /// the horizontal (blue) and vertical planes read distinctly when
    /// overlapping.
    pub(super) vertical_color: Rgba,
}

#[cfg(feature = "gpu-3d")]
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
    pub(super) fn reference_lines(colors: &SpiderColors) -> Vec<Line3D> {
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

    pub(super) fn polygon_from_vertices(
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

#[cfg(feature = "gpu-3d")]
impl IntoElement for SpiderView3D<'_> {
    type Element = AnyElement;
    fn into_element(self) -> AnyElement {
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
