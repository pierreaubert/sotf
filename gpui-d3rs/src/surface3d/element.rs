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

        // Now render the surface
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        let width_u32 = width as u32;
        let height_u32 = height as u32;

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

        // Draw axis labels (AFTER rendering surface to be ON TOP)
        let camera = &self.state.borrow().camera;
        // Re-use width/height f32 from above

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
        // (Functionality moved inline for dynamic axes)


        // Only render grid and labels if in Cartesian mode
        if self.config.plot_type == SurfacePlotType::Cartesian {
            // Helper to get screen position of a 3D point
            let to_screen = |pos: glam::Vec3| -> Option<glam::Vec3> {
                let p = camera.project_to_screen(pos, width, height)?;
                if p.z >= 0.0 && p.z <= 1.0 { Some(p) } else { None }
            };

            // Shared helper to draw a single tick and its label
            let draw_tick_and_label = |window: &mut Window,
                                     pos: glam::Vec3,
                                     tick_vec: glam::Vec3,
                                     label: String| {
                // Draw tick
                let tick_end = pos + tick_vec;
                if let (Some(s_start), Some(s_end)) = (to_screen(pos), to_screen(tick_end)) {
                     let p1 = gpui::Point { x: px(s_start.x) + bounds.origin.x, y: px(s_start.y) + bounds.origin.y };
                     let p2 = gpui::Point { x: px(s_end.x) + bounds.origin.x, y: px(s_end.y) + bounds.origin.y };
                     let mut builder = gpui::PathBuilder::stroke(px(1.0));
                     builder.move_to(p1);
                     builder.line_to(p2);
                     if let Ok(path) = builder.build() { window.paint_path(path, gpui::rgba(0x000000ff)); }
                }

                // Draw label
                // Position label slightly past the tick
                let label_pos = pos + tick_vec * 1.5;
                if let Some(screen_pos) = to_screen(label_pos) {
                    let screen_x = screen_pos.x + f32::from(bounds.origin.x);
                    let screen_y = screen_pos.y + f32::from(bounds.origin.y);
                    let font_size = 8.0;
                    let text_width = measure_text_width(&label, font_size);

                    paint_vector_text_at(
                        window, &label,
                        screen_x - text_width / 2.0,
                        screen_y - font_size / 2.0,
                        font_size, 1.0, gpui::rgba(0x000000ff),
                        0.0 // Always upright (face camera)
                    );
                }
            };

            // Dynamic X Axis (Freq)
            // candidates: (y=-0.5, z=1) "Front", (y=-0.5, z=-1) "Back"
            let x_candidates = [
                (glam::Vec3::new(0.0, -0.5, 1.0), 1.0), // Front edge center
                (glam::Vec3::new(0.0, -0.5, -1.0), -1.0) // Back edge center
            ];

            let mut best_x_z_val = x_candidates[0].1;
            let mut max_screen_y = -f32::INFINITY;

            for (pos, z_val) in x_candidates {
                if let Some(screen_pos) = to_screen(pos) {
                    if screen_pos.y > max_screen_y {
                        max_screen_y = screen_pos.y;
                        best_x_z_val = z_val;
                    }
                }
            }

            // Freq Labels (X axis)
            let freq_ticks = self.data.x_ticks.clone().unwrap_or_else(|| vec![100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0]);
            for freq in freq_ticks {
                let x = self.data.normalize_x(freq);
                let pos = glam::Vec3::new(x, -0.5, best_x_z_val);
                let tick_dir_z = if best_x_z_val > 0.0 { 1.0 } else { -1.0 };
                let tick_vec = glam::Vec3::new(0.0, 0.0, 0.1 * tick_dir_z);

                let label = if freq >= 1000.0 { format!("{}k", freq / 1000.0) } else { format!("{}", freq) };

                draw_tick_and_label(window, pos, tick_vec, label);
            }

            // X Axis Title
             draw_label(
                window,
                self.data.x_label.clone().unwrap_or("Freq. (Hz)".to_string()),
                glam::Vec3::new(0.0, -0.5, best_x_z_val + 0.4 * (if best_x_z_val > 0.0 { 1.0 } else { -1.0 })),
                false,
                false,
            );


            // Dynamic Z Axis (Angle)
            let z_candidates = [
                (glam::Vec3::new(1.0, -0.5, 0.0), 1.0), // Right edge center
                (glam::Vec3::new(-1.0, -0.5, 0.0), -1.0) // Left edge center
            ];
            
            let mut best_z_x_val = z_candidates[0].1;
            max_screen_y = -f32::INFINITY;

            for (pos, x_val) in z_candidates {
                if let Some(screen_pos) = to_screen(pos) {
                    if screen_pos.y > max_screen_y {
                        max_screen_y = screen_pos.y;
                        best_z_x_val = x_val;
                    }
                }
            }

            // Angle Labels (Z axis)
            let angle_ticks = self.data.y_ticks.clone().unwrap_or_else(|| {
                let min_angle = self.data.y_min;
                let max_angle = self.data.y_max;
                let step = 60.0;
                let start = (min_angle / step).ceil() * step;
                let mut ticks = Vec::new();
                let mut angle = start;
                while angle <= max_angle + 0.1 {
                    ticks.push(angle);
                    angle += step;
                }
                ticks
            });

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
                glam::Vec3::new(best_z_x_val + 0.3 * (if best_z_x_val > 0.0 { 1.0 } else { -1.0 }), -0.5, 0.0),
                false,
                false,
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
                if let Some(screen_pos) = to_screen(pos) {
                    if screen_pos.x < min_screen_x {
                        min_screen_x = screen_pos.x;
                        best_y_x = x_val;
                        best_y_z = z_val;
                    }
                }
            }

            // SPL Labels (Y axis)
            let spl_ticks = self.data.z_ticks.clone().unwrap_or_else(|| vec![-40.0, -30.0, -20.0, -10.0, 0.0, 10.0]);
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
                false,
                false,
            );
        } else if self.config.plot_type == SurfacePlotType::Spherical {
            // Helper to get screen position of a 3D point
            let to_screen = |pos: glam::Vec3| -> Option<glam::Vec3> {
                let p = camera.project_to_screen(pos, width, height)?;
                // In Sphere mode, we might see back of sphere?
                // Just use z-buffer check [0,1]
                if p.z >= 0.0 && p.z <= 1.0 { Some(p) } else { None }
            };

            // Shared helper to draw a single tick and its label (Same as above, maybe verify if scope allows sharing or redefine)
            // Re-defining for simplicity as scope is separate
             let draw_tick_and_label = |window: &mut Window,
                                     pos: glam::Vec3,
                                     tick_vec: glam::Vec3,
                                     label: String| {
                let tick_end = pos + tick_vec;
                if let (Some(s_start), Some(s_end)) = (to_screen(pos), to_screen(tick_end)) {
                     let p1 = gpui::Point { x: px(s_start.x) + bounds.origin.x, y: px(s_start.y) + bounds.origin.y };
                     let p2 = gpui::Point { x: px(s_end.x) + bounds.origin.x, y: px(s_end.y) + bounds.origin.y };
                     let mut builder = gpui::PathBuilder::stroke(px(1.0));
                     builder.move_to(p1);
                     builder.line_to(p2);
                     if let Ok(path) = builder.build() { window.paint_path(path, gpui::rgba(0x000000ff)); }
                }

                let label_pos = pos + tick_vec * 1.5;
                if let Some(screen_pos) = to_screen(label_pos) {
                    let screen_x = screen_pos.x + f32::from(bounds.origin.x);
                    let screen_y = screen_pos.y + f32::from(bounds.origin.y);
                    let font_size = 8.0;
                    let text_width = measure_text_width(&label, font_size);
                    
                    paint_vector_text_at(
                        window, &label, 
                        screen_x - text_width / 2.0, 
                        screen_y - font_size / 2.0, 
                        font_size, 1.0, gpui::rgba(0x000000ff), 
                        0.0
                    );
                }
            };

            // Draw Azimuth Labels (Equator)
            // Y data is Azimuth (-180..180).
            let az_ticks = self.data.y_ticks.clone().unwrap_or_else(|| vec![-180.0, -90.0, 0.0, 90.0, 180.0]);
            
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
            let el_ticks = self.data.x_ticks.clone().unwrap_or_else(|| vec![-90.0, -45.0, 0.0, 45.0, 90.0]);
            
            // Draw on Prime Meridian (Theta=0 -> Y=0 in data, Z positive)
            
            for el in el_ticks {
                if el.abs() > 89.0 { continue; } // Skip poles to avoid clutter

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

    }
}
