//! GPUI Element implementation for 3D surface rendering

use super::camera::{Camera3D, OrbitControls};
use super::config::Surface3DConfig;
use super::data::SurfaceData;
use super::mesh::SurfaceMesh;
use super::renderer::Surface3DRenderer;
use crate::surface3d::config::SurfacePlotType;
use crate::text::{measure_text_width, paint_vector_text_at};
use gpui::*;
use image::{Frame, RgbaImage};
use std::cell::RefCell;
use std::panic;
use std::rc::Rc;
use std::sync::Arc;

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
        _cx: &mut App,
    ) {
        // Register mouse event handlers (must be done during paint)
        let _state = self.state.clone();
        let _bounds_for_handler = bounds;

        // Mouse event handlers are now handled by the parent view

        // Draw axis labels
        let camera = &self.state.borrow().camera;
        let bounds = bounds;
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();

        // Helper to draw text at 3D position
        let draw_label = |window: &mut Window,
                          text: String,
                          pos: glam::Vec3,
                          align_right: bool,
                          align_bottom: bool| {
            if let Some(screen_pos) = camera.project_to_screen(pos, width, height) {
                // Check if point is within reasonable bounds (not clipped)
                if screen_pos.z >= 0.0 && screen_pos.z <= 1.0 {
                    let mut x = screen_pos.x + f32::from(bounds.origin.x);
                    let mut y = screen_pos.y + f32::from(bounds.origin.y);

                    let font_size = 10.0;
                    let color = gpui::rgba(0x000000ff); // Black text

                    // Simple alignment adjustment
                    if align_right {
                        let text_width = measure_text_width(&text, font_size);
                        x -= text_width;
                    }

                    // Center vertically on point (approximate)
                    y -= font_size / 2.0;

                    if align_bottom {
                        // If we want the text to be ABOVE the point, we subtract height
                        // If we want it BELOW, we add?
                        // Default is centered.
                    }

                    paint_vector_text_at(
                        window, &text, x, y, font_size, 1.0, // stroke width
                        color, 0.0, // rotation
                    );
                }
            }
        };

        // Helper to draw tick lines
        let draw_tick = |window: &mut Window, pos: glam::Vec3, axis: usize| {
            // axis: 0=X (Freq), 1=Y (SPL), 2=Z (Angle)
            // We draw a small line perpendicular to the axis
            let tick_len = 0.05;
            let start = pos;
            let end = match axis {
                0 => glam::Vec3::new(pos.x, pos.y, pos.z + tick_len), // Freq tick along Z
                1 => glam::Vec3::new(pos.x - tick_len, pos.y, pos.z), // SPL tick along X
                2 => glam::Vec3::new(pos.x + tick_len, pos.y, pos.z), // Angle tick along X
                _ => pos,
            };

            if let (Some(s_start), Some(s_end)) = (
                camera.project_to_screen(start, width, height),
                camera.project_to_screen(end, width, height),
            ) {
                if s_start.z >= 0.0 && s_start.z <= 1.0 && s_end.z >= 0.0 && s_end.z <= 1.0 {
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
                        window.paint_path(path, gpui::rgba(0x000000ff));
                    }
                }
            }
        };

        // Only render grid and labels if in Cartesian mode
        if self.config.plot_type == SurfacePlotType::Cartesian {
            // Freq Labels (X axis)
            // Log scale 20, 100, 1k, 10k, 20k
            let freq_ticks = [20.0, 100.0, 1000.0, 10000.0, 20000.0];
            for &freq in &freq_ticks {
                let x = self.data.normalize_x(freq);
                // Position on the "front" edge of the floor
                let pos = glam::Vec3::new(x, -0.5, 1.0);
                draw_tick(window, pos, 0);

                // Label slightly further out
                let label_pos = glam::Vec3::new(x, -0.5, 1.15);
                let label = if freq >= 1000.0 {
                    format!("{}k", freq / 1000.0)
                } else {
                    format!("{}", freq)
                };
                draw_label(window, label, label_pos, false, false);
            }
            // Freq Axis Title
            draw_label(
                window,
                "Freq. (Hz)".to_string(),
                glam::Vec3::new(0.0, -0.5, 1.3),
                false,
                false,
            );

            // Angle Labels (Z axis)
            // -180, -90, 0, 90, 180
            let angle_ticks = [-180.0, -90.0, 0.0, 90.0, 180.0];
            for &angle in &angle_ticks {
                let z = self.data.normalize_y(angle);
                let pos = glam::Vec3::new(1.0, -0.5, z);
                draw_tick(window, pos, 2);

                let label_pos = glam::Vec3::new(1.15, -0.5, z);
                draw_label(window, format!("{}°", angle), label_pos, false, false);
            }
            // Angle Axis Title
            draw_label(
                window,
                "Angle".to_string(),
                glam::Vec3::new(1.3, -0.5, 0.0),
                false,
                false,
            );

            // SPL Labels (Y axis)
            // -40 to 10 step 10
            let spl_ticks = [-40.0, -30.0, -20.0, -10.0, 0.0, 10.0];
            for &spl in &spl_ticks {
                let y = self.data.normalize_z(spl) - 0.5;
                let pos = glam::Vec3::new(-1.0, y, 1.0);
                draw_tick(window, pos, 1);

                let label_pos = glam::Vec3::new(-1.15, y, 1.05);
                draw_label(window, format!("{}dB", spl), label_pos, true, false);
            }
            // SPL Axis Title
            draw_label(
                window,
                "SPL".to_string(),
                glam::Vec3::new(-1.3, 0.0, 1.2),
                true,
                false,
            );
        }

        // Now render the surface
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        let width = width as u32;
        let height = height as u32;

        if width == 0 || height == 0 {
            return;
        }

        // Ensure renderer and mesh are initialized
        self.ensure_renderer();
        self.ensure_mesh();

        // Resize renderer if needed
        if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
            renderer.resize(width, height);

            // Update camera and render
            let state = self.state.borrow();
            if let Some(pixels) = renderer.render(&state.camera) {
                // Create RgbaImage from RGBA pixel data
                if let Some(rgba_image) = RgbaImage::from_raw(width, height, pixels) {
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
}
