//! GPUI Element implementation for 3D surface rendering

use super::camera::{Camera3D, OrbitControls};
use super::config::Surface3DConfig;
use super::config::SurfacePlotType;
use super::data::SurfaceData;
use super::mesh::SurfaceMesh;
use super::renderer::Surface3DRenderer;
use crate::color::D3Color;
use crate::contour::ContourGenerator;
use crate::shape::contour_smoothing::StrokePoint;
use crate::text::{GlyphTextConfig, HorizontalTextAnchor, VerticalTextAnchor, paint_chart_text_at};
use glam::Vec3;
use gpui::*;
use image::{Frame, RgbaImage};
use std::cell::RefCell;
use std::panic;
use std::rc::Rc;
use std::sync::Arc;

const MAX_SURFACE_RENDER_DIMENSION: f32 = 4096.0;

/// Interactive state for 3D surface element
#[derive(Debug, Clone)]
pub struct Surface3DState {
    /// Orbit camera controls
    pub controls: OrbitControls,
    /// Camera
    pub camera: Camera3D,
    /// Is mouse currently dragging (for rotation)
    pub dragging: bool,
    /// Is middle mouse dragging (for pan)
    pub panning: bool,
    /// Last mouse position
    pub last_mouse: Option<Point<Pixels>>,
}

impl Default for Surface3DState {
    fn default() -> Self {
        let controls = OrbitControls::default();
        let camera = controls.to_camera();
        Self {
            controls,
            camera,
            dragging: false,
            panning: false,
            last_mouse: None,
        }
    }
}

impl Surface3DState {
    /// Create state with custom initial camera position
    pub fn new(distance: f32, azimuth_deg: f32, elevation_deg: f32) -> Self {
        let controls = OrbitControls::default().with_position(distance, azimuth_deg, elevation_deg);
        let camera = controls.to_camera();
        Self {
            controls,
            camera,
            dragging: false,
            panning: false,
            last_mouse: None,
        }
    }

    /// Update camera from controls
    pub fn update_camera(&mut self) {
        self.controls.update_camera(&mut self.camera);
    }
}

/// GPUI Element for interactive 3D surface visualization
#[derive(Clone)]
pub struct Surface3DElement {
    data: SurfaceData,
    config: Surface3DConfig,
    state: Rc<RefCell<Surface3DState>>,
    renderer: Rc<RefCell<Option<Surface3DRenderer>>>,
    mesh: Rc<RefCell<Option<SurfaceMesh>>>,
}

impl Surface3DElement {
    /// Create a new 3D surface element
    pub fn new(data: SurfaceData, config: Surface3DConfig) -> Self {
        let state = Surface3DState::new(
            config.camera_distance,
            config.camera_azimuth,
            config.camera_elevation,
        );

        Self {
            data,
            config,
            state: Rc::new(RefCell::new(state)),
            renderer: Rc::new(RefCell::new(None)),
            mesh: Rc::new(RefCell::new(None)),
        }
    }

    /// Create with default configuration
    pub fn from_data(data: SurfaceData) -> Self {
        Self::new(data, Surface3DConfig::default())
    }

    /// Update the surface data
    pub fn set_data(&mut self, data: SurfaceData) {
        self.data = data;
        // Clear cached mesh to force regeneration
        *self.mesh.borrow_mut() = None;
    }

    /// Update configuration
    pub fn set_config(&mut self, config: Surface3DConfig) {
        // If plot type changes, we need to regenerate the mesh
        if self.config.plot_type != config.plot_type {
            *self.mesh.borrow_mut() = None;
        }
        self.config = config;
        if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
            renderer.set_config(self.config.clone());
        }
    }

    /// Get mutable access to state for external control
    pub fn state(&self) -> Rc<RefCell<Surface3DState>> {
        self.state.clone()
    }

    /// Set external state for the element (allows sharing state with view)
    pub fn with_state(mut self, state: Rc<RefCell<Surface3DState>>) -> Self {
        self.state = state;
        self
    }

    fn ensure_renderer(&self) -> bool {
        let mut renderer_ref = self.renderer.borrow_mut();
        if renderer_ref.is_none() {
            *renderer_ref = Some(Surface3DRenderer::new(self.config.clone()));
        }
        true
    }

    fn ensure_mesh(&self) {
        let mut mesh_ref = self.mesh.borrow_mut();
        if mesh_ref.is_none() {
            let mesh = SurfaceMesh::from_data(&self.data, self.config.plot_type);
            if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
                renderer.set_mesh(&mesh);
            }
            *mesh_ref = Some(mesh);
        }
    }

    fn normalized_z_grid(&self) -> Vec<f64> {
        let x_count = self.data.x_count();
        let y_count = self.data.y_count();
        let mut values = Vec::with_capacity(x_count * y_count);
        for y in 0..y_count {
            for x in 0..x_count {
                let z = self.data.z_at(x, y).unwrap_or(self.data.z_min);
                values.push(self.data.normalize_z(z).clamp(0.0, 1.0) as f64);
            }
        }
        values
    }

    fn isoline_levels(&self) -> Vec<f64> {
        let step = self.config.isoline_step.max(0.001) as f64;
        let mut level = step;
        let mut levels = Vec::new();
        while level < 1.0 {
            levels.push(level);
            level += step;
        }
        levels
    }

    fn isoline_world_position(&self, x: f64, y: f64, normalized_z: f64) -> Vec3 {
        let nx = self.data.normalize_x(x);
        let ny = self.data.normalize_y(y);
        let nz = normalized_z as f32;
        match self.config.plot_type {
            SurfacePlotType::Cartesian => Vec3::new(nx, nz - 0.5, ny),
            SurfacePlotType::Spherical => {
                let phi = nx * std::f32::consts::FRAC_PI_2;
                let theta = ny * std::f32::consts::PI;
                let radius = 1.0;
                let y_pos = radius * phi.sin();
                let r_xz = radius * phi.cos();
                let x_pos = r_xz * theta.sin();
                let z_pos = r_xz * theta.cos();
                Vec3::new(x_pos, y_pos, z_pos)
            }
        }
    }

    fn paint_projected_isolines(
        &self,
        bounds: Bounds<Pixels>,
        width: f32,
        height: f32,
        camera: &Camera3D,
        window: &mut Window,
    ) {
        if !self.config.isolines
            || self.config.isoline_opacity <= 0.0
            || self.config.isoline_width_px <= 0.0
            || self.data.x_count() < 2
            || self.data.y_count() < 2
        {
            return;
        }

        let levels = self.isoline_levels();
        if levels.is_empty() {
            return;
        }

        let values = self.normalized_z_grid();
        let contour_segments = ContourGenerator::new(self.data.x_count(), self.data.y_count())
            .x_values(self.data.x_values.clone())
            .y_values(self.data.y_values.clone())
            .x_log_interpolation(self.data.x_log)
            .y_log_interpolation(self.data.y_log)
            .upsample_factor(self.config.isoline_upsample_factor)
            .contour_segments(&values, &levels);
        let mut stroke_color = D3Color {
            r: self.config.isoline_color[0].clamp(0.0, 1.0),
            g: self.config.isoline_color[1].clamp(0.0, 1.0),
            b: self.config.isoline_color[2].clamp(0.0, 1.0),
            a: 1.0,
        }
        .to_rgba();
        stroke_color.a *= self.config.isoline_opacity.clamp(0.0, 1.0);

        for segment in contour_segments {
            let start_world =
                self.isoline_world_position(segment.start.x, segment.start.y, segment.value);
            let end_world =
                self.isoline_world_position(segment.end.x, segment.end.y, segment.value);
            let Some(start) = camera.project_to_screen(start_world, width, height) else {
                continue;
            };
            let Some(end) = camera.project_to_screen(end_world, width, height) else {
                continue;
            };

            paint_stroke_segment(
                &[
                    StrokePoint::new(start.x, start.y),
                    StrokePoint::new(end.x, end.y),
                ],
                bounds,
                self.config.isoline_width_px,
                stroke_color,
                window,
            );
        }
    }

    fn paint_projected_grid(
        &self,
        bounds: Bounds<Pixels>,
        width: f32,
        height: f32,
        camera: &Camera3D,
        overlay_color: Rgba,
        window: &mut Window,
    ) {
        if !self.config.show_grid
            || !self.config.show_axes
            || self.config.plot_type != SurfacePlotType::Cartesian
        {
            return;
        }

        for line in cartesian_grid_lines(&self.data, camera) {
            let (stroke_width, alpha) = match line.kind {
                CartesianGridLineKind::Minor => (0.75, 0.18),
                CartesianGridLineKind::Major => (1.0, 0.36),
                CartesianGridLineKind::Border => (1.25, 0.68),
            };
            let Some(start) = camera.project_to_screen(line.start, width, height) else {
                continue;
            };
            let Some(end) = camera.project_to_screen(line.end, width, height) else {
                continue;
            };

            let mut color = overlay_color;
            color.a *= alpha;
            paint_stroke_segment(
                &[
                    StrokePoint::new(start.x, start.y),
                    StrokePoint::new(end.x, end.y),
                ],
                bounds,
                stroke_width,
                color,
                window,
            );
        }
    }
}

fn paint_stroke_segment(
    segment: &[StrokePoint],
    bounds: Bounds<Pixels>,
    width_px: f32,
    color: Rgba,
    window: &mut Window,
) {
    if segment.len() < 2 {
        return;
    }

    let mut builder = PathBuilder::stroke(px(width_px));
    builder.move_to(Point {
        x: px(segment[0].x) + bounds.origin.x,
        y: px(segment[0].y) + bounds.origin.y,
    });
    for point in &segment[1..] {
        builder.line_to(Point {
            x: px(point.x) + bounds.origin.x,
            y: px(point.y) + bounds.origin.y,
        });
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CartesianGridLineKind {
    Minor,
    Major,
    Border,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CartesianGridLine {
    start: Vec3,
    end: Vec3,
    kind: CartesianGridLineKind,
}

#[derive(Debug, Clone, Default)]
struct AxisGridTicks {
    major: Vec<f32>,
    minor: Vec<f32>,
}

fn cartesian_grid_lines(data: &SurfaceData, camera: &Camera3D) -> Vec<CartesianGridLine> {
    let x_ticks = frequency_grid_ticks(data);
    let angle_ticks = angle_grid_ticks(data);
    let spl_ticks = spl_grid_ticks(data);
    let x_face = if camera.position.x >= camera.target.x {
        -1.0
    } else {
        1.0
    };
    let z_face = if camera.position.z >= camera.target.z {
        -1.0
    } else {
        1.0
    };

    let mut lines = Vec::new();
    push_cartesian_grid_lines_for_ticks(
        &mut lines,
        &x_ticks.minor,
        CartesianGridLineKind::Minor,
        |x| {
            [
                (Vec3::new(x, -0.5, -1.0), Vec3::new(x, -0.5, 1.0)),
                (Vec3::new(x, -0.5, z_face), Vec3::new(x, 0.5, z_face)),
            ]
        },
    );
    push_cartesian_grid_lines_for_ticks(
        &mut lines,
        &angle_ticks.minor,
        CartesianGridLineKind::Minor,
        |z| {
            [
                (Vec3::new(-1.0, -0.5, z), Vec3::new(1.0, -0.5, z)),
                (Vec3::new(x_face, -0.5, z), Vec3::new(x_face, 0.5, z)),
            ]
        },
    );
    push_cartesian_grid_lines_for_ticks(
        &mut lines,
        &spl_ticks.minor,
        CartesianGridLineKind::Minor,
        |y| {
            [
                (Vec3::new(x_face, y, -1.0), Vec3::new(x_face, y, 1.0)),
                (Vec3::new(-1.0, y, z_face), Vec3::new(1.0, y, z_face)),
            ]
        },
    );
    push_cartesian_grid_lines_for_ticks(
        &mut lines,
        &x_ticks.major,
        CartesianGridLineKind::Major,
        |x| {
            [
                (Vec3::new(x, -0.5, -1.0), Vec3::new(x, -0.5, 1.0)),
                (Vec3::new(x, -0.5, z_face), Vec3::new(x, 0.5, z_face)),
            ]
        },
    );
    push_cartesian_grid_lines_for_ticks(
        &mut lines,
        &angle_ticks.major,
        CartesianGridLineKind::Major,
        |z| {
            [
                (Vec3::new(-1.0, -0.5, z), Vec3::new(1.0, -0.5, z)),
                (Vec3::new(x_face, -0.5, z), Vec3::new(x_face, 0.5, z)),
            ]
        },
    );
    push_cartesian_grid_lines_for_ticks(
        &mut lines,
        &spl_ticks.major,
        CartesianGridLineKind::Major,
        |y| {
            [
                (Vec3::new(x_face, y, -1.0), Vec3::new(x_face, y, 1.0)),
                (Vec3::new(-1.0, y, z_face), Vec3::new(1.0, y, z_face)),
            ]
        },
    );
    push_box_border_lines(&mut lines, x_face, z_face);
    lines
}

fn push_cartesian_grid_lines_for_ticks<const N: usize>(
    lines: &mut Vec<CartesianGridLine>,
    ticks: &[f32],
    kind: CartesianGridLineKind,
    make_lines: impl Fn(f32) -> [(Vec3, Vec3); N],
) {
    for &tick in ticks {
        for (start, end) in make_lines(tick) {
            lines.push(CartesianGridLine { start, end, kind });
        }
    }
}

fn push_box_border_lines(lines: &mut Vec<CartesianGridLine>, x_face: f32, z_face: f32) {
    let floor_edges = [
        (Vec3::new(-1.0, -0.5, -1.0), Vec3::new(1.0, -0.5, -1.0)),
        (Vec3::new(1.0, -0.5, -1.0), Vec3::new(1.0, -0.5, 1.0)),
        (Vec3::new(1.0, -0.5, 1.0), Vec3::new(-1.0, -0.5, 1.0)),
        (Vec3::new(-1.0, -0.5, 1.0), Vec3::new(-1.0, -0.5, -1.0)),
    ];
    let x_wall_edges = [
        (Vec3::new(x_face, -0.5, -1.0), Vec3::new(x_face, -0.5, 1.0)),
        (Vec3::new(x_face, 0.5, -1.0), Vec3::new(x_face, 0.5, 1.0)),
        (Vec3::new(x_face, -0.5, -1.0), Vec3::new(x_face, 0.5, -1.0)),
        (Vec3::new(x_face, -0.5, 1.0), Vec3::new(x_face, 0.5, 1.0)),
    ];
    let z_wall_edges = [
        (Vec3::new(-1.0, -0.5, z_face), Vec3::new(1.0, -0.5, z_face)),
        (Vec3::new(-1.0, 0.5, z_face), Vec3::new(1.0, 0.5, z_face)),
        (Vec3::new(-1.0, -0.5, z_face), Vec3::new(-1.0, 0.5, z_face)),
        (Vec3::new(1.0, -0.5, z_face), Vec3::new(1.0, 0.5, z_face)),
    ];

    for (start, end) in floor_edges
        .into_iter()
        .chain(x_wall_edges)
        .chain(z_wall_edges)
    {
        push_unique_border_line(lines, start, end);
    }
}

fn push_unique_border_line(lines: &mut Vec<CartesianGridLine>, start: Vec3, end: Vec3) {
    const EPS: f32 = 1e-4;
    let same_point = |a: Vec3, b: Vec3| (a - b).length_squared() < EPS * EPS;
    if lines.iter().any(|line| {
        line.kind == CartesianGridLineKind::Border
            && ((same_point(line.start, start) && same_point(line.end, end))
                || (same_point(line.start, end) && same_point(line.end, start)))
    }) {
        return;
    }
    lines.push(CartesianGridLine {
        start,
        end,
        kind: CartesianGridLineKind::Border,
    });
}

fn frequency_grid_ticks(data: &SurfaceData) -> AxisGridTicks {
    let mut major = normalized_x_positions(
        data.x_ticks
            .clone()
            .unwrap_or_else(default_frequency_ticks)
            .into_iter(),
        data,
    );
    let mut minor = if data.x_log {
        normalized_x_positions(log_frequency_minor_ticks(data).into_iter(), data)
    } else {
        normalized_x_positions(
            linear_subdivision_ticks(data.x_min, data.x_max, 25).into_iter(),
            data,
        )
    };
    sanitize_axis_positions(&mut major, -1.0, 1.0);
    sanitize_axis_positions(&mut minor, -1.0, 1.0);
    remove_positions(&mut minor, &major);
    AxisGridTicks { major, minor }
}

fn angle_grid_ticks(data: &SurfaceData) -> AxisGridTicks {
    let mut major = normalized_y_positions(angle_major_ticks(data).into_iter(), data);
    let mut minor = normalized_y_positions(
        linear_step_ticks(data.y_min, data.y_max, 10.0).into_iter(),
        data,
    );
    sanitize_axis_positions(&mut major, -1.0, 1.0);
    sanitize_axis_positions(&mut minor, -1.0, 1.0);
    remove_positions(&mut minor, &major);
    AxisGridTicks { major, minor }
}

fn spl_grid_ticks(data: &SurfaceData) -> AxisGridTicks {
    let (major_ticks, major_step) = spl_major_ticks(data);
    let mut major = normalized_z_positions(major_ticks.into_iter(), data);
    let mut minor = if data.z_ticks.is_some() {
        Vec::new()
    } else {
        normalized_z_positions(
            linear_step_ticks(data.z_min, data.z_max, major_step / 5.0).into_iter(),
            data,
        )
    };
    sanitize_axis_positions(&mut major, -0.5, 0.5);
    sanitize_axis_positions(&mut minor, -0.5, 0.5);
    remove_positions(&mut minor, &major);
    AxisGridTicks { major, minor }
}

fn default_frequency_ticks() -> Vec<f64> {
    vec![
        100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ]
}

fn angle_major_ticks(data: &SurfaceData) -> Vec<f64> {
    data.y_ticks
        .clone()
        .unwrap_or_else(|| linear_step_ticks(data.y_min, data.y_max, 30.0))
}

fn spl_major_ticks(data: &SurfaceData) -> (Vec<f64>, f64) {
    if let Some(ticks) = data.z_ticks.clone() {
        return (ticks, 1.0);
    }

    let range = data.z_max - data.z_min;
    let step = if range > 40.0 {
        10.0
    } else if range > 20.0 {
        5.0
    } else if range > 10.0 {
        2.0
    } else {
        1.0
    };
    (linear_step_ticks(data.z_min, data.z_max, step), step)
}

fn linear_step_ticks(min: f64, max: f64, step: f64) -> Vec<f64> {
    if !min.is_finite() || !max.is_finite() || !step.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    let start = (min / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut value = start;
    while value <= max + step * 1e-3 {
        ticks.push(value);
        value += step;
    }
    ticks
}

fn linear_subdivision_ticks(min: f64, max: f64, divisions: usize) -> Vec<f64> {
    if divisions == 0 || !min.is_finite() || !max.is_finite() || min >= max {
        return Vec::new();
    }
    (0..=divisions)
        .map(|i| min + (max - min) * i as f64 / divisions as f64)
        .collect()
}

fn log_frequency_minor_ticks(data: &SurfaceData) -> Vec<f64> {
    let min = if data.x_min > 0.0 {
        data.x_min
    } else {
        data.x_values
            .iter()
            .copied()
            .filter(|value| *value > 0.0 && value.is_finite())
            .fold(f64::INFINITY, f64::min)
    };
    let max = data.x_max;
    if !min.is_finite() || !max.is_finite() || min <= 0.0 || max <= min {
        return Vec::new();
    }

    let start_decade = min.log10().floor() as i32;
    let end_decade = max.log10().ceil() as i32;
    let mut ticks = Vec::new();
    for decade in start_decade..=end_decade {
        let base = 10_f64.powi(decade);
        for multiplier in 2..10 {
            let value = base * multiplier as f64;
            if value >= min && value <= max {
                ticks.push(value);
            }
        }
    }
    ticks
}

fn normalized_x_positions(ticks: impl Iterator<Item = f64>, data: &SurfaceData) -> Vec<f32> {
    ticks.map(|value| data.normalize_x(value)).collect()
}

fn normalized_y_positions(ticks: impl Iterator<Item = f64>, data: &SurfaceData) -> Vec<f32> {
    ticks.map(|value| data.normalize_y(value)).collect()
}

fn normalized_z_positions(ticks: impl Iterator<Item = f64>, data: &SurfaceData) -> Vec<f32> {
    ticks.map(|value| data.normalize_z(value) - 0.5).collect()
}

fn sanitize_axis_positions(values: &mut Vec<f32>, min: f32, max: f32) {
    const EPS: f32 = 1e-4;
    values.retain(|value| value.is_finite() && *value > min + EPS && *value < max - EPS);
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup_by(|a, b| (*a - *b).abs() < EPS);
}

fn remove_positions(values: &mut Vec<f32>, positions_to_remove: &[f32]) {
    const EPS: f32 = 1e-4;
    values.retain(|value| {
        !positions_to_remove
            .iter()
            .any(|position| (*value - *position).abs() < EPS)
    });
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartesianGridLineDebugKind {
    Minor,
    Major,
    Border,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianGridLineDebug {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub kind: CartesianGridLineDebugKind,
}

#[doc(hidden)]
pub fn cartesian_grid_lines_for_testing(
    data: &SurfaceData,
    camera: &Camera3D,
) -> Vec<CartesianGridLineDebug> {
    cartesian_grid_lines(data, camera)
        .into_iter()
        .map(|line| CartesianGridLineDebug {
            start: line.start.to_array(),
            end: line.end.to_array(),
            kind: match line.kind {
                CartesianGridLineKind::Minor => CartesianGridLineDebugKind::Minor,
                CartesianGridLineKind::Major => CartesianGridLineDebugKind::Major,
                CartesianGridLineKind::Border => CartesianGridLineDebugKind::Border,
            },
        })
        .collect()
}

impl IntoElement for Surface3DElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Surface3DElement {
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
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        // Update camera aspect ratio
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        if height > 0.0 {
            self.state.borrow_mut().camera.aspect = width / height;
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Register mouse event handlers (must be done during paint)
        let _state = self.state.clone();
        let _bounds_for_handler = bounds;

        // Mouse event handlers are now handled by the parent view

        // Now render the surface
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        let scale_factor = window.scale_factor().clamp(1.0, 3.0);
        let mut render_width = width * scale_factor;
        let mut render_height = height * scale_factor;
        let max_render_dimension = render_width.max(render_height);
        if max_render_dimension > MAX_SURFACE_RENDER_DIMENSION {
            let downscale = MAX_SURFACE_RENDER_DIMENSION / max_render_dimension;
            render_width *= downscale;
            render_height *= downscale;
        }
        let width_u32 = render_width.ceil().max(1.0) as u32;
        let height_u32 = render_height.ceil().max(1.0) as u32;

        if width_u32 > 0 && height_u32 > 0 {
            // Ensure renderer and mesh are initialized
            self.ensure_renderer();
            self.ensure_mesh();

            // Resize renderer if needed
            if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
                renderer.resize(width_u32, height_u32);

                // Update camera and render
                let state = self.state.borrow();

                let log_settings = if self.data.x_log {
                    let min_x = self.data.x_min as f32;
                    let max_x = self.data.x_max as f32;
                    Some((min_x, max_x))
                } else {
                    None
                };

                if let Some(pixels) = renderer.render(&state.camera, log_settings) {
                    // Create RgbaImage from RGBA pixel data
                    if let Some(rgba_image) = RgbaImage::from_raw(width_u32, height_u32, pixels) {
                        // Create a Frame from the image
                        let frame = Frame::new(rgba_image);

                        // Create GPUI RenderImage from the frame
                        let render_image = RenderImage::new(vec![frame]);

                        // Paint the image
                        let _ = window.paint_image(
                            bounds,
                            Corners::default(),
                            Arc::new(render_image),
                            0,
                            false,
                        );
                    }
                }
            }
        }

        // Pick overlay color that contrasts with the background
        let bg = self.config.background_color;
        let luminance = 0.299 * bg[0] + 0.587 * bg[1] + 0.114 * bg[2];
        let overlay_color = if luminance > 0.5 {
            gpui::rgba(0x000000ff) // dark text on light background
        } else {
            gpui::rgba(0xffffffff) // white text on dark background
        };

        {
            let camera = &self.state.borrow().camera;
            self.paint_projected_grid(bounds, width, height, camera, overlay_color, window);
            self.paint_projected_isolines(bounds, width, height, camera, window);
        }

        // Draw axis labels (AFTER rendering surface to be ON TOP)
        let camera = &self.state.borrow().camera;
        // Re-use width/height f32 from above

        let upright_rotation = |mut angle: f32| -> f32 {
            while angle > std::f32::consts::FRAC_PI_2 {
                angle -= std::f32::consts::PI;
            }
            while angle < -std::f32::consts::FRAC_PI_2 {
                angle += std::f32::consts::PI;
            }
            angle
        };

        let billboard_rotation = |pos: glam::Vec3| -> f32 {
            let (right, _) = camera.billboard_axes();
            if let (Some(a), Some(b)) = (
                camera.project_to_screen(pos, width, height),
                camera.project_to_screen(pos + right * 0.08, width, height),
            ) {
                upright_rotation((b.y - a.y).atan2(b.x - a.x))
            } else {
                0.0
            }
        };

        // Helper to draw text at 3D position
        let draw_label = |window: &mut Window,
                          text: String,
                          pos: glam::Vec3,
                          horizontal_anchor: HorizontalTextAnchor,
                          vertical_anchor: VerticalTextAnchor| {
            if let Some(screen_pos) = camera.project_to_screen(pos, width, height) {
                // Check if point is within reasonable bounds (not clipped)
                if screen_pos.z >= 0.0 && screen_pos.z <= 1.0 {
                    let font_size = 10.0;
                    let text_config =
                        GlyphTextConfig::rotated(font_size, overlay_color, billboard_rotation(pos));
                    paint_chart_text_at(
                        window,
                        cx,
                        &text,
                        screen_pos.x + f32::from(bounds.origin.x),
                        screen_pos.y + f32::from(bounds.origin.y),
                        &text_config,
                        horizontal_anchor,
                        vertical_anchor,
                    );
                }
            }
        };

        // Helper to draw tick lines
        // (Functionality moved inline for dynamic axes)

        // Only render grid and labels if in Cartesian mode
        if self.config.plot_type == SurfacePlotType::Cartesian {
            // Helper to get screen position of a 3D point
            let to_screen = |pos: glam::Vec3| -> Option<glam::Vec3> {
                let p = camera.project_to_screen(pos, width, height)?;
                if p.z >= 0.0 && p.z <= 1.0 {
                    Some(p)
                } else {
                    None
                }
            };

            // Shared helper to draw a single tick and its label
            let draw_tick_and_label =
                |window: &mut Window, pos: glam::Vec3, tick_vec: glam::Vec3, label: String| {
                    // Draw tick
                    let tick_end = pos + tick_vec;
                    if let (Some(s_start), Some(s_end)) = (to_screen(pos), to_screen(tick_end)) {
                        let p1 = gpui::Point {
                            x: px(s_start.x) + bounds.origin.x,
                            y: px(s_start.y) + bounds.origin.y,
                        };
                        let p2 = gpui::Point {
                            x: px(s_end.x) + bounds.origin.x,
                            y: px(s_end.y) + bounds.origin.y,
                        };
                        let mut builder = gpui::PathBuilder::stroke(px(1.0));
                        builder.move_to(p1);
                        builder.line_to(p2);
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, overlay_color);
                        }
                    }

                    // Draw label with offset to avoid overlapping tick line
                    // Position label past the tick end, then offset perpendicular in screen space
                    let label_pos_3d = pos + tick_vec * 1.5;
                    if let (Some(tick_start_screen), Some(tick_end_screen), Some(label_screen)) =
                        (to_screen(pos), to_screen(tick_end), to_screen(label_pos_3d))
                    {
                        let font_size = 8.0;

                        // Compute tick direction in screen space
                        let tick_dx = tick_end_screen.x - tick_start_screen.x;
                        let tick_dy = tick_end_screen.y - tick_start_screen.y;
                        let tick_len = (tick_dx * tick_dx + tick_dy * tick_dy).sqrt();

                        // Compute perpendicular offset in screen space
                        // This ensures label doesn't overlap tick even when viewed along tick axis
                        let (offset_x, offset_y) = if tick_len > 0.1 {
                            // Perpendicular to tick direction, biased downward (positive y in screen)
                            let perp_x = -tick_dy / tick_len;
                            let perp_y = tick_dx / tick_len;
                            // Choose direction that moves label down/right (more readable)
                            let offset_amount = font_size * 0.8;
                            if perp_y >= 0.0 {
                                (perp_x * offset_amount, perp_y * offset_amount)
                            } else {
                                (-perp_x * offset_amount, -perp_y * offset_amount)
                            }
                        } else {
                            // Tick is very short in screen space, just offset down
                            (0.0, font_size * 0.8)
                        };

                        let text_config = GlyphTextConfig::rotated(
                            font_size,
                            overlay_color,
                            billboard_rotation(label_pos_3d),
                        );
                        paint_chart_text_at(
                            window,
                            cx,
                            &label,
                            label_screen.x + f32::from(bounds.origin.x) + offset_x,
                            label_screen.y + f32::from(bounds.origin.y) + offset_y,
                            &text_config,
                            HorizontalTextAnchor::Middle,
                            VerticalTextAnchor::Middle,
                        );
                    }
                };

            // Dynamic X Axis (Freq)
            // candidates: (y=-0.5, z=1) "Front", (y=-0.5, z=-1) "Back"
            let x_candidates = [
                (glam::Vec3::new(0.0, -0.5, 1.0), 1.0),   // Front edge center
                (glam::Vec3::new(0.0, -0.5, -1.0), -1.0), // Back edge center
            ];

            let mut best_x_z_val = x_candidates[0].1;
            let mut max_screen_y = -f32::INFINITY;

            for (pos, z_val) in x_candidates {
                if let Some(screen_pos) = to_screen(pos)
                    && screen_pos.y > max_screen_y
                {
                    max_screen_y = screen_pos.y;
                    best_x_z_val = z_val;
                }
            }

            // Freq Labels (X axis)
            let freq_ticks = self
                .data
                .x_ticks
                .clone()
                .unwrap_or_else(default_frequency_ticks);
            for freq in freq_ticks {
                let x = self.data.normalize_x(freq);
                let pos = glam::Vec3::new(x, -0.5, best_x_z_val);
                let tick_dir_z = if best_x_z_val > 0.0 { 1.0 } else { -1.0 };
                let tick_vec = glam::Vec3::new(0.0, 0.0, 0.1 * tick_dir_z);

                let label = if freq >= 1000.0 {
                    format!("{}k", freq / 1000.0)
                } else {
                    format!("{}", freq)
                };

                draw_tick_and_label(window, pos, tick_vec, label);
            }

            // X Axis Title
            draw_label(
                window,
                self.data
                    .x_label
                    .clone()
                    .unwrap_or("Freq. (Hz)".to_string()),
                glam::Vec3::new(
                    0.0,
                    -0.5,
                    best_x_z_val + 0.4 * (if best_x_z_val > 0.0 { 1.0 } else { -1.0 }),
                ),
                HorizontalTextAnchor::Middle,
                VerticalTextAnchor::Middle,
            );

            // Dynamic Z Axis (Angle)
            let z_candidates = [
                (glam::Vec3::new(1.0, -0.5, 0.0), 1.0),   // Right edge center
                (glam::Vec3::new(-1.0, -0.5, 0.0), -1.0), // Left edge center
            ];

            let mut best_z_x_val = z_candidates[0].1;
            max_screen_y = -f32::INFINITY;

            for (pos, x_val) in z_candidates {
                if let Some(screen_pos) = to_screen(pos)
                    && screen_pos.y > max_screen_y
                {
                    max_screen_y = screen_pos.y;
                    best_z_x_val = x_val;
                }
            }

            // Angle Labels (Z axis) - 30° major ticks
            let angle_ticks = angle_major_ticks(&self.data);

            for angle in angle_ticks {
                let z = self.data.normalize_y(angle);
                let pos = glam::Vec3::new(best_z_x_val, -0.5, z);
                let tick_dir_x = if best_z_x_val > 0.0 { 1.0 } else { -1.0 };
                let tick_vec = glam::Vec3::new(0.1 * tick_dir_x, 0.0, 0.0);

                let label = format!("{}°", angle);

                draw_tick_and_label(window, pos, tick_vec, label);
            }
            // Angle Axis Title
            draw_label(
                window,
                self.data.y_label.clone().unwrap_or("Angle".to_string()),
                glam::Vec3::new(
                    best_z_x_val + 0.3 * (if best_z_x_val > 0.0 { 1.0 } else { -1.0 }),
                    -0.5,
                    0.0,
                ),
                HorizontalTextAnchor::Middle,
                VerticalTextAnchor::Middle,
            );

            // Dynamic Y Axis (SPL)
            let y_candidates = [
                (glam::Vec3::new(1.0, 0.0, 1.0), 1.0, 1.0),
                (glam::Vec3::new(1.0, 0.0, -1.0), 1.0, -1.0),
                (glam::Vec3::new(-1.0, 0.0, 1.0), -1.0, 1.0),
                (glam::Vec3::new(-1.0, 0.0, -1.0), -1.0, -1.0),
            ];

            let mut best_y_x = y_candidates[0].1;
            let mut best_y_z = y_candidates[0].2;
            let mut min_screen_x = f32::INFINITY;

            for (pos, x_val, z_val) in y_candidates {
                if let Some(screen_pos) = to_screen(pos)
                    && screen_pos.x < min_screen_x
                {
                    min_screen_x = screen_pos.x;
                    best_y_x = x_val;
                    best_y_z = z_val;
                }
            }

            // SPL Labels (Y axis)
            // Generate dynamic ticks based on actual data range
            let (spl_ticks, _) = spl_major_ticks(&self.data);
            for spl in spl_ticks {
                let y = self.data.normalize_z(spl) - 0.5;
                let pos = glam::Vec3::new(best_y_x, y, best_y_z);
                let tick_vec = glam::Vec3::new(best_y_x * 0.1, 0.0, best_y_z * 0.1);

                let label = format!("{}dB", spl);
                draw_tick_and_label(window, pos, tick_vec, label);
            }
            // SPL Axis Title
            draw_label(
                window,
                self.data.z_label.clone().unwrap_or("SPL".to_string()),
                glam::Vec3::new(best_y_x * 1.4, 0.0, best_y_z * 1.4),
                HorizontalTextAnchor::Middle,
                VerticalTextAnchor::Middle,
            );
        } else if self.config.plot_type == SurfacePlotType::Spherical {
            // Helper to get screen position of a 3D point
            let to_screen = |pos: glam::Vec3| -> Option<glam::Vec3> {
                let p = camera.project_to_screen(pos, width, height)?;
                // In Sphere mode, we might see back of sphere?
                // Just use z-buffer check [0,1]
                if p.z >= 0.0 && p.z <= 1.0 {
                    Some(p)
                } else {
                    None
                }
            };

            // Shared helper to draw a single tick and its label
            // Re-defining for simplicity as scope is separate
            let draw_tick_and_label =
                |window: &mut Window, pos: glam::Vec3, tick_vec: glam::Vec3, label: String| {
                    let tick_end = pos + tick_vec;
                    if let (Some(s_start), Some(s_end)) = (to_screen(pos), to_screen(tick_end)) {
                        let p1 = gpui::Point {
                            x: px(s_start.x) + bounds.origin.x,
                            y: px(s_start.y) + bounds.origin.y,
                        };
                        let p2 = gpui::Point {
                            x: px(s_end.x) + bounds.origin.x,
                            y: px(s_end.y) + bounds.origin.y,
                        };
                        let mut builder = gpui::PathBuilder::stroke(px(1.0));
                        builder.move_to(p1);
                        builder.line_to(p2);
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, overlay_color);
                        }
                    }

                    // Draw label with offset to avoid overlapping tick line
                    let label_pos_3d = pos + tick_vec * 1.5;
                    if let (Some(tick_start_screen), Some(tick_end_screen), Some(label_screen)) =
                        (to_screen(pos), to_screen(tick_end), to_screen(label_pos_3d))
                    {
                        let font_size = 8.0;

                        // Compute tick direction in screen space
                        let tick_dx = tick_end_screen.x - tick_start_screen.x;
                        let tick_dy = tick_end_screen.y - tick_start_screen.y;
                        let tick_len = (tick_dx * tick_dx + tick_dy * tick_dy).sqrt();

                        // Compute perpendicular offset in screen space
                        let (offset_x, offset_y) = if tick_len > 0.1 {
                            let perp_x = -tick_dy / tick_len;
                            let perp_y = tick_dx / tick_len;
                            let offset_amount = font_size * 0.8;
                            if perp_y >= 0.0 {
                                (perp_x * offset_amount, perp_y * offset_amount)
                            } else {
                                (-perp_x * offset_amount, -perp_y * offset_amount)
                            }
                        } else {
                            (0.0, font_size * 0.8)
                        };

                        let text_config = GlyphTextConfig::rotated(
                            font_size,
                            overlay_color,
                            billboard_rotation(label_pos_3d),
                        );
                        paint_chart_text_at(
                            window,
                            cx,
                            &label,
                            label_screen.x + f32::from(bounds.origin.x) + offset_x,
                            label_screen.y + f32::from(bounds.origin.y) + offset_y,
                            &text_config,
                            HorizontalTextAnchor::Middle,
                            VerticalTextAnchor::Middle,
                        );
                    }
                };

            // Draw Azimuth Labels (Equator)
            // Y data is Azimuth (-180..180).
            let az_ticks = self
                .data
                .y_ticks
                .clone()
                .unwrap_or_else(|| vec![-180.0, -90.0, 0.0, 90.0, 180.0]);

            for az in az_ticks {
                // Convert Azimuth to 3D pos on sphere equator (Phi=0)
                // normalize_y maps Azimuth to [-1, 1]
                // mesh.rs: theta = ny * PI => [-PI, PI]
                let ny = self.data.normalize_y(az);
                let theta = ny * std::f32::consts::PI;
                let radius = 1.0;

                // Phi = 0 => y_pos = 0, r_xz=radius
                let x = radius * theta.sin();
                let z = radius * theta.cos();
                let pos = glam::Vec3::new(x, 0.0, z);

                let tick_vec = pos.normalize() * 0.15; // Point out
                let label = format!("{}°", az);
                draw_tick_and_label(window, pos, tick_vec, label);
            }

            // Draw Elevation Labels (Meridian)
            // X data is Elevation (-90..90).
            let el_ticks = self
                .data
                .x_ticks
                .clone()
                .unwrap_or_else(|| vec![-90.0, -45.0, 0.0, 45.0, 90.0]);

            // Draw on Prime Meridian (Theta=0 -> Y=0 in data, Z positive)

            for el in el_ticks {
                if el.abs() > 89.0 {
                    continue;
                } // Skip poles to avoid clutter

                let nx = self.data.normalize_x(el);
                let phi = nx * std::f32::consts::FRAC_PI_2;
                let radius = 1.0;

                // Theta = 0
                let y = radius * phi.sin();
                let r_xz = radius * phi.cos();
                let x = 0.0;
                let z = r_xz * 1.0; // theta=0 -> sin=0, cos=1

                let pos = glam::Vec3::new(x, y, z);
                let tick_vec = pos.normalize() * 0.1;
                let label = format!("{}°", el);

                draw_tick_and_label(window, pos, tick_vec, label);
            }
        }

        // Draw colorbar legend if enabled
        if self.config.show_colorbar {
            let colorbar_width: f32 = 20.0;
            let colorbar_height: f32 = height * 0.6;
            let colorbar_x = f32::from(bounds.origin.x) + width - colorbar_width - 50.0;
            let colorbar_y = f32::from(bounds.origin.y) + (height - colorbar_height) / 2.0;
            let num_segments = 50;

            // Get Z range from data
            let (z_min, z_max) = (self.data.z_min, self.data.z_max);

            // Draw colorbar segments
            for i in 0..num_segments {
                let t = i as f32 / num_segments as f32;
                let segment_height = colorbar_height / num_segments as f32;
                let y = colorbar_y + colorbar_height
                    - (t + 1.0 / num_segments as f32) * colorbar_height;

                // Get color from colormap
                let color = self.config.colormap.color_at(1.0 - t);
                let rgba = gpui::rgba(
                    ((color.0 * 255.0) as u32) << 24
                        | ((color.1 * 255.0) as u32) << 16
                        | ((color.2 * 255.0) as u32) << 8
                        | 0xFF,
                );

                window.paint_quad(gpui::PaintQuad {
                    bounds: gpui::Bounds::new(
                        gpui::point(px(colorbar_x), px(y)),
                        gpui::size(px(colorbar_width), px(segment_height + 1.0)),
                    ),
                    corner_radii: gpui::Corners::default(),
                    background: rgba.into(),
                    border_widths: gpui::Edges::default(),
                    border_color: gpui::transparent_black(),
                    border_style: Default::default(),
                });
            }

            // Draw border around colorbar
            let mut builder = gpui::PathBuilder::stroke(px(1.0));
            builder.move_to(gpui::point(px(colorbar_x), px(colorbar_y)));
            builder.line_to(gpui::point(px(colorbar_x + colorbar_width), px(colorbar_y)));
            builder.line_to(gpui::point(
                px(colorbar_x + colorbar_width),
                px(colorbar_y + colorbar_height),
            ));
            builder.line_to(gpui::point(
                px(colorbar_x),
                px(colorbar_y + colorbar_height),
            ));
            builder.line_to(gpui::point(px(colorbar_x), px(colorbar_y)));
            if let Ok(path) = builder.build() {
                window.paint_path(path, overlay_color);
            }

            // Draw tick labels for colorbar
            let num_ticks = 5;
            let font_size = 9.0;
            for i in 0..=num_ticks {
                let t = i as f64 / num_ticks as f64;
                let value = z_min + t * (z_max - z_min);
                let y = colorbar_y + colorbar_height * (1.0 - t as f32);

                // Draw tick line
                let mut builder = gpui::PathBuilder::stroke(px(1.0));
                builder.move_to(gpui::point(px(colorbar_x + colorbar_width), px(y)));
                builder.line_to(gpui::point(px(colorbar_x + colorbar_width + 4.0), px(y)));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, overlay_color);
                }

                // Draw label
                let label = format!("{:.0}", value);
                let text_config = GlyphTextConfig::horizontal(font_size, overlay_color);
                paint_chart_text_at(
                    window,
                    cx,
                    &label,
                    colorbar_x + colorbar_width + 6.0,
                    y,
                    &text_config,
                    HorizontalTextAnchor::Start,
                    VerticalTextAnchor::Middle,
                );
            }

            // Draw colorbar title (Z label)
            if let Some(ref z_label) = self.data.z_label {
                let label_x = colorbar_x + colorbar_width / 2.0;
                let label_y = colorbar_y - 15.0;
                let text_config = GlyphTextConfig::horizontal(10.0, overlay_color);
                paint_chart_text_at(
                    window,
                    cx,
                    z_label,
                    label_x,
                    label_y,
                    &text_config,
                    HorizontalTextAnchor::Middle,
                    VerticalTextAnchor::Middle,
                );
            }
        }
    }
}
