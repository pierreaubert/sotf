//! macOS AU Window implementation — embeds GPUI rendering inside an NSView.
//!
//! Unlike a standalone macOS window, the AU window does NOT own an NSWindow.
//! It takes a host-provided NSView, adds a CAMetalLayer-backed subview,
//! and renders GPUI content into it via wgpu (Metal backend).
//!
//! Frame rendering and event dispatch are driven externally by the Swift
//! AUViewController via FFI calls (request_frame, mouse events, resize).

use super::AuDisplay;
use gpui::{
    point, px, size, AnyWindowHandle, AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile,
    Bounds, Capslock, DevicePixels, DispatchEventResult, GpuSpecs, Modifiers, Pixels,
    PlatformAtlas, PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow, Point,
    PromptButton, PromptLevel, RequestFrameOptions, Scene, Size, TileId, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowParams,
};
use gpui_wgpu::{WgpuContext, WgpuRenderer, WgpuSurfaceConfig};
use objc::{class, msg_send, runtime::Object, sel, sel_impl};
use parking_lot::Mutex;
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HasDisplayHandle, HasWindowHandle,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};

/// Thread-local storage for the NSView pointer + dimensions,
/// set by `gpui_au_create` before calling `app.run()` so that
/// `AuWindow::new()` can read it during `open_window()`.
pub(crate) struct PendingViewInfo {
    pub ns_view: *mut Object,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

thread_local! {
    pub(crate) static PENDING_VIEW: RefCell<Option<PendingViewInfo>> = const { RefCell::new(None) };
}

/// Wrapper for a raw pointer to make it Send+Sync for OnceLock.
/// SAFETY: The pointer is only accessed from the main thread.
pub(crate) struct AuWindowPtr(pub *const AuWindow);
unsafe impl Send for AuWindowPtr {}
unsafe impl Sync for AuWindowPtr {}

/// Global window pointer, used by `gpui_au_request_frame` to find the window.
/// Single-instance: only one AU GPUI window per process (each AU appex is its own process).
pub(crate) static AU_WINDOW: std::sync::OnceLock<AuWindowPtr> = std::sync::OnceLock::new();

pub(crate) struct AuWindow {
    /// The NSView we render into (owned by the Swift AUViewController)
    view: *mut Object,
    bounds: Cell<Bounds<Pixels>>,
    scale_factor: Cell<f32>,
    input_handler: RefCell<Option<PlatformInputHandler>>,
    pub(crate) request_frame_callback: RefCell<Option<Box<dyn FnMut(RequestFrameOptions)>>>,
    input_callback: RefCell<Option<Box<dyn FnMut(PlatformInput) -> DispatchEventResult>>>,
    active_status_callback: RefCell<Option<Box<dyn FnMut(bool)>>>,
    hover_status_callback: RefCell<Option<Box<dyn FnMut(bool)>>>,
    resize_callback: RefCell<Option<Box<dyn FnMut(Size<Pixels>, f32)>>>,
    moved_callback: RefCell<Option<Box<dyn FnMut()>>>,
    should_close_callback: RefCell<Option<Box<dyn FnMut() -> bool>>>,
    hit_test_callback: RefCell<Option<Box<dyn FnMut() -> Option<WindowControlArea>>>>,
    close_callback: RefCell<Option<Box<dyn FnOnce()>>>,
    appearance_changed_callback: RefCell<Option<Box<dyn FnMut()>>>,
    mouse_position: Cell<Point<Pixels>>,
    modifiers: Cell<Modifiers>,
    renderer: Mutex<Option<WgpuRenderer>>,
}

impl AuWindow {
    /// Create a new AU window that renders into the NSView from PENDING_VIEW.
    ///
    /// The NSView must have been set in the PENDING_VIEW thread-local by
    /// `gpui_au_create` before calling `app.run()` / `open_window()`.
    pub fn new(_handle: AnyWindowHandle, _params: WindowParams) -> anyhow::Result<Self> {
        let view_info = PENDING_VIEW.with(|pv| pv.borrow_mut().take());
        let (ns_view, width, height, scale) = match view_info {
            Some(info) => (info.ns_view, info.width, info.height, info.scale),
            None => {
                log::warn!("GPUI AU: No pending view info — creating window without renderer");
                return Ok(Self {
                    view: std::ptr::null_mut(),
                    bounds: Cell::new(Bounds {
                        origin: Default::default(),
                        size: size(px(600.0), px(400.0)),
                    }),
                    scale_factor: Cell::new(2.0),
                    input_handler: RefCell::new(None),
                    request_frame_callback: RefCell::new(None),
                    input_callback: RefCell::new(None),
                    active_status_callback: RefCell::new(None),
                    hover_status_callback: RefCell::new(None),
                    resize_callback: RefCell::new(None),
                    moved_callback: RefCell::new(None),
                    should_close_callback: RefCell::new(None),
                    hit_test_callback: RefCell::new(None),
                    close_callback: RefCell::new(None),
                    appearance_changed_callback: RefCell::new(None),
                    mouse_position: Cell::new(Point::default()),
                    modifiers: Cell::new(Modifiers::default()),
                    renderer: Mutex::new(None),
                });
            }
        };

        // Configure the NSView with a CAMetalLayer for wgpu rendering.
        // wantsLayer=true alone creates a regular CALayer; wgpu needs CAMetalLayer.
        unsafe {
            let _: () = msg_send![ns_view, setWantsLayer: true];

            // Create a CAMetalLayer and set it as the view's layer
            let metal_layer: *mut Object = msg_send![class!(CAMetalLayer), layer];
            let _: () =
                msg_send![metal_layer, setContentsScale: scale as core_graphics::base::CGFloat];
            // Match the view's bounds
            let view_bounds: core_graphics::geometry::CGRect = msg_send![ns_view, bounds];
            let _: () = msg_send![metal_layer, setFrame: view_bounds];
            let _: () = msg_send![ns_view, setLayer: metal_layer];
        }

        let au_window = Self {
            view: ns_view,
            bounds: Cell::new(Bounds {
                origin: Default::default(),
                size: size(px(width), px(height)),
            }),
            scale_factor: Cell::new(scale),
            input_handler: RefCell::new(None),
            request_frame_callback: RefCell::new(None),
            input_callback: RefCell::new(None),
            active_status_callback: RefCell::new(None),
            hover_status_callback: RefCell::new(None),
            resize_callback: RefCell::new(None),
            moved_callback: RefCell::new(None),
            should_close_callback: RefCell::new(None),
            hit_test_callback: RefCell::new(None),
            close_callback: RefCell::new(None),
            appearance_changed_callback: RefCell::new(None),
            mouse_position: Cell::new(Point::default()),
            modifiers: Cell::new(Modifiers::default()),
            renderer: Mutex::new(None),
        };

        // Initialize wgpu renderer (Metal backend)
        let pixel_w = (width * scale) as i32;
        let pixel_h = (height * scale) as i32;

        let config = WgpuSurfaceConfig {
            size: size(DevicePixels(pixel_w), DevicePixels(pixel_h)),
            transparent: false,
        };

        let metal_instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            flags: wgpu::InstanceFlags::default(),
            ..Default::default()
        });

        let window_handle = au_window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Window handle unavailable: {e}"))?;
        let display_handle = au_window
            .display_handle()
            .map_err(|e| anyhow::anyhow!("Display handle unavailable: {e}"))?;

        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: display_handle.as_raw(),
            raw_window_handle: window_handle.as_raw(),
        };

        match (|| -> anyhow::Result<WgpuRenderer> {
            let surface = unsafe { metal_instance.create_surface_unsafe(target)? };
            let context = WgpuContext::new(metal_instance, &surface)?;
            let mut gpu_context: Option<WgpuContext> = Some(context);
            drop(surface);
            WgpuRenderer::new(&mut gpu_context, &au_window, config)
        })() {
            Ok(renderer) => {
                log::info!("GPUI AU: wgpu renderer created (Metal) for {}x{} @{:.1}x", width, height, scale);
                *au_window.renderer.lock() = Some(renderer);
            }
            Err(e) => {
                log::error!("GPUI AU: Failed to create wgpu renderer: {e:#}");
            }
        }

        Ok(au_window)
    }

    /// Register this window in the global AU_WINDOW slot.
    /// Called from AuPlatform::open_window after Boxing.
    pub(crate) fn register_global(boxed: &AuWindow) {
        let ptr: *const AuWindow = boxed;
        let _ = AU_WINDOW.set(AuWindowPtr(ptr));
        log::info!("GPUI AU: Window registered at {:p}", ptr);
    }

    /// Request a frame render (called from Swift via FFI)
    #[allow(dead_code)]
    pub fn request_frame(&self) {
        let cb = self.request_frame_callback.borrow_mut().take();
        if let Some(mut cb) = cb {
            cb(RequestFrameOptions::default());
            let mut slot = self.request_frame_callback.borrow_mut();
            if slot.is_none() {
                *slot = Some(cb);
            }
        }
    }

    /// Handle resize from the host (called from Swift via FFI)
    pub fn handle_resize(&self, width: f32, height: f32, scale: f32) {
        let new_size = size(px(width), px(height));
        self.bounds.set(Bounds {
            origin: Default::default(),
            size: new_size,
        });
        self.scale_factor.set(scale);

        // Update Metal layer scale and frame
        if !self.view.is_null() {
            unsafe {
                let layer: *mut Object = msg_send![self.view, layer];
                let _: () =
                    msg_send![layer, setContentsScale: scale as core_graphics::base::CGFloat];
                let new_frame = core_graphics::geometry::CGRect {
                    origin: core_graphics::geometry::CGPoint { x: 0.0, y: 0.0 },
                    size: core_graphics::geometry::CGSize {
                        width: width as f64,
                        height: height as f64,
                    },
                };
                let _: () = msg_send![layer, setFrame: new_frame];
            }
        }

        // Update wgpu surface
        let pixel_w = (width * scale) as i32;
        let pixel_h = (height * scale) as i32;
        {
            let mut guard = self.renderer.lock();
            if let Some(renderer) = guard.as_mut() {
                renderer.update_drawable_size(size(DevicePixels(pixel_w), DevicePixels(pixel_h)));
            }
        }

        // Fire resize callback
        let cb = self.resize_callback.borrow_mut().take();
        if let Some(mut cb) = cb {
            cb(new_size, scale);
            let mut slot = self.resize_callback.borrow_mut();
            if slot.is_none() {
                *slot = Some(cb);
            }
        }
    }

    /// Dispatch a mouse event (called from Swift via FFI)
    pub fn dispatch_input(&self, event: PlatformInput) {
        // Update tracked mouse position for MouseMove/Down/Up
        match &event {
            PlatformInput::MouseDown(e) => self.mouse_position.set(e.position),
            PlatformInput::MouseUp(e) => self.mouse_position.set(e.position),
            PlatformInput::MouseMove(e) => self.mouse_position.set(e.position),
            _ => {}
        }

        if let Some(callback) = self.input_callback.borrow_mut().as_mut() {
            callback(event);
        }
    }
}

impl HasWindowHandle for AuWindow {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        let view = NonNull::new(self.view as *mut c_void)
            .ok_or(raw_window_handle::HandleError::Unavailable)?;
        let handle = AppKitWindowHandle::new(view);
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl HasDisplayHandle for AuWindow {
    fn display_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError>
    {
        let handle = AppKitDisplayHandle::new();
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(handle.into()) })
    }
}

impl PlatformWindow for AuWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds.get()
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds.get())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds.get().size
    }

    fn resize(&mut self, _size: Size<Pixels>) {
        // Resize is driven externally by the host via handle_resize
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor.get()
    }

    fn appearance(&self) -> WindowAppearance {
        if self.view.is_null() {
            return WindowAppearance::Light;
        }
        unsafe {
            let effective: *mut Object = msg_send![self.view, effectiveAppearance];
            if effective.is_null() {
                return WindowAppearance::Light;
            }
            let name: *mut Object = msg_send![effective, name];
            if name.is_null() {
                return WindowAppearance::Light;
            }
            let dark_aqua: *mut Object =
                msg_send![class!(NSString), stringWithUTF8String: c"NSAppearanceNameDarkAqua".as_ptr()];
            let is_dark: bool = msg_send![name, isEqualToString: dark_aqua];
            if is_dark {
                WindowAppearance::Dark
            } else {
                WindowAppearance::Light
            }
        }
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(AuDisplay::main()))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.mouse_position.get()
    }

    fn modifiers(&self) -> Modifiers {
        self.modifiers.get()
    }

    fn capslock(&self) -> Capslock {
        Capslock { on: false }
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        *self.input_handler.borrow_mut() = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.borrow_mut().take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        // No prompt support in AU extensions
        None
    }

    fn activate(&self) {}

    fn is_active(&self) -> bool {
        true // AU view is always considered active when visible
    }

    fn is_hovered(&self) -> bool {
        false
    }

    fn set_title(&mut self, _title: &str) {}

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {}

    fn minimize(&self) {}
    fn zoom(&self) {}
    fn toggle_fullscreen(&self) {}

    fn is_fullscreen(&self) -> bool {
        false
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        *self.request_frame_callback.borrow_mut() = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        *self.input_callback.borrow_mut() = Some(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        *self.active_status_callback.borrow_mut() = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        *self.hover_status_callback.borrow_mut() = Some(callback);
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        *self.resize_callback.borrow_mut() = Some(callback);
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        *self.moved_callback.borrow_mut() = Some(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        *self.should_close_callback.borrow_mut() = Some(callback);
    }

    fn on_hit_test_window_control(
        &self,
        callback: Box<dyn FnMut() -> Option<WindowControlArea>>,
    ) {
        *self.hit_test_callback.borrow_mut() = Some(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        *self.close_callback.borrow_mut() = Some(callback);
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        *self.appearance_changed_callback.borrow_mut() = Some(callback);
    }

    fn draw(&self, scene: &Scene) {
        let mut guard = self.renderer.lock();
        if let Some(renderer) = guard.as_mut() {
            renderer.draw(scene);
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        let guard = self.renderer.lock();
        if let Some(renderer) = guard.as_ref() {
            renderer.sprite_atlas().clone()
        } else {
            Arc::new(FallbackAtlas::new())
        }
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        let guard = self.renderer.lock();
        guard.as_ref().map(|r| r.gpu_specs())
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {}
}

// ── Fallback atlas ────────────────────────────────────────────────────────────

struct FallbackAtlas {
    state: Mutex<FallbackAtlasState>,
}

struct FallbackAtlasState {
    next_id: u32,
    tiles: HashMap<AtlasKey, AtlasTile>,
}

impl FallbackAtlas {
    fn new() -> Self {
        Self {
            state: Mutex::new(FallbackAtlasState {
                next_id: 1,
                tiles: HashMap::new(),
            }),
        }
    }
}

impl PlatformAtlas for FallbackAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<
            Option<(Size<DevicePixels>, std::borrow::Cow<'a, [u8]>)>,
        >,
    ) -> anyhow::Result<Option<AtlasTile>> {
        let mut state = self.state.lock();

        if let Some(tile) = state.tiles.get(key) {
            return Ok(Some(tile.clone()));
        }

        let data = build()?;
        if let Some((tile_size, _pixels)) = data {
            let id = state.next_id;
            state.next_id += 1;

            let tile = AtlasTile {
                texture_id: AtlasTextureId {
                    index: 0,
                    kind: AtlasTextureKind::Monochrome,
                },
                tile_id: TileId(id),
                padding: 0,
                bounds: Bounds {
                    origin: point(DevicePixels(0), DevicePixels(0)),
                    size: tile_size,
                },
            };

            state.tiles.insert(key.clone(), tile.clone());
            Ok(Some(tile))
        } else {
            Ok(None)
        }
    }

    fn remove(&self, key: &AtlasKey) {
        self.state.lock().tiles.remove(key);
    }
}
