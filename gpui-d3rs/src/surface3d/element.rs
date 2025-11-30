//! GPUI Element implementation for 3D surface rendering

use super::camera::{Camera3D, OrbitControls};
use super::config::Surface3DConfig;
use super::data::SurfaceData;
use super::mesh::SurfaceMesh;
use super::renderer::Surface3DRenderer;
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
            let mesh = SurfaceMesh::from_data(&self.data);
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
        let state = self.state.clone();
        let bounds_for_handler = bounds;

        // Mouse down - start drag
        window.on_mouse_event({
            let state = state.clone();
            move |event: &MouseDownEvent, phase, _window, _cx| {
                // Only handle Capture phase to get events first
                if phase != DispatchPhase::Capture {
                    return;
                }
                if !bounds_for_handler.contains(&event.position) {
                    return;
                }

                let mut state = state.borrow_mut();
                match event.button {
                    MouseButton::Left => {
                        state.dragging = true;
                        state.last_mouse = Some(event.position);
                    }
                    MouseButton::Middle => {
                        state.panning = true;
                        state.last_mouse = Some(event.position);
                    }
                    _ => {}
                }
            }
        });

        // Mouse up - end drag
        window.on_mouse_event({
            let state = state.clone();
            move |event: &MouseUpEvent, phase, _window, _cx| {
                // Only handle Capture phase
                if phase != DispatchPhase::Capture {
                    return;
                }

                let mut state = state.borrow_mut();
                match event.button {
                    MouseButton::Left => {
                        state.dragging = false;
                    }
                    MouseButton::Middle => {
                        state.panning = false;
                    }
                    _ => {}
                }
            }
        });

        // Mouse move - rotate or pan
        window.on_mouse_event({
            let state = state.clone();
            move |event: &MouseMoveEvent, phase, window, _cx| {
                // Only handle Capture phase
                if phase != DispatchPhase::Capture {
                    return;
                }

                let mut state = state.borrow_mut();
                if let Some(last) = state.last_mouse {
                    let delta_x: f32 = event.position.x.into();
                    let delta_y: f32 = event.position.y.into();
                    let last_x: f32 = last.x.into();
                    let last_y: f32 = last.y.into();
                    let dx = delta_x - last_x;
                    let dy = delta_y - last_y;

                    if state.dragging {
                        state.controls.rotate(dx, dy);
                        state.update_camera();
                        window.refresh();
                    } else if state.panning {
                        // Clone camera to avoid borrow conflict
                        let camera_clone = state.camera.clone();
                        state.controls.pan(dx, dy, &camera_clone);
                        state.update_camera();
                        window.refresh();
                    }
                }

                if state.dragging || state.panning {
                    state.last_mouse = Some(event.position);
                }
            }
        });

        // Scroll - zoom
        window.on_mouse_event({
            let state = state.clone();
            let bounds_for_scroll = bounds;
            move |event: &ScrollWheelEvent, phase, window, _cx| {
                // Only handle Capture phase
                if phase != DispatchPhase::Capture {
                    return;
                }
                if !bounds_for_scroll.contains(&event.position) {
                    return;
                }

                let mut state = state.borrow_mut();
                let delta = match event.delta {
                    ScrollDelta::Lines(lines) => lines.y as f32 * 0.5,
                    ScrollDelta::Pixels(pixels) => {
                        let py: f32 = pixels.y.into();
                        py * 0.01
                    }
                };
                state.controls.zoom(delta);
                state.update_camera();
                window.refresh();
            }
        });

        // Double click - reset view
        window.on_mouse_event({
            let state = state.clone();
            let bounds_for_click = bounds;
            move |event: &MouseDownEvent, phase, window, _cx| {
                // Only handle Capture phase
                if phase != DispatchPhase::Capture {
                    return;
                }
                if !bounds_for_click.contains(&event.position) {
                    return;
                }
                if event.button != MouseButton::Left || event.click_count != 2 {
                    return;
                }

                let mut state = state.borrow_mut();
                state.controls.reset();
                state.update_camera();
                window.refresh();
            }
        });

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
