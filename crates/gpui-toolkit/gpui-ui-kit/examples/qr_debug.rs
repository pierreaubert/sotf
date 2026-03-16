//! QR Code Debug Example
//!
//! Demonstrates QR code generation and camera-based QR reading:
//! - **Generate** tab: static and animated QR codes at various sizes
//! - **Scan** tab: live camera preview with QR detection overlay
//!
//! Camera permission is requested on first scan. If denied or no camera
//! is available, a clear error message is shown with instructions.

use gpui::*;
use gpui_ui_kit::qr::AnimatedQrCode;
use gpui_ui_kit::theme::ThemeExt;
use gpui_ui_kit::*;
use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Camera / QR scanning types (adapted from stkopt qr_reader.rs)
// ---------------------------------------------------------------------------

const PREVIEW_WIDTH: usize = 320;
const PREVIEW_HEIGHT: usize = 240;

#[derive(Debug, Clone)]
struct CameraPreview {
    rgb_pixels: Vec<u8>,
    width: usize,
    height: usize,
    qr_bounds: Option<[(f32, f32); 4]>,
}

#[derive(Debug, Clone)]
enum QrScanResult {
    Success(String, CameraPreview),
    Scanning(CameraPreview),
    Detected(CameraPreview),
    Error(String),
}

/// Camera permission / availability state
#[derive(Debug, Clone, PartialEq)]
enum CameraState {
    /// Haven't tried yet
    Idle,
    /// Camera is active and streaming
    Active,
    /// Camera access denied or unavailable
    Denied(String),
}

// ---------------------------------------------------------------------------
// QrDebug application
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Generate,
    Scan,
}

pub struct QrDebug {
    entity: Entity<Self>,
    active_tab: Tab,
    // Generate tab
    animated_qr_tiny: Entity<AnimatedQrCode>,
    animated_qr_small: Entity<AnimatedQrCode>,
    // Scan tab
    camera_state: CameraState,
    scan_result_rx: Option<mpsc::Receiver<QrScanResult>>,
    stop_tx: Option<mpsc::Sender<()>>,
    last_preview: Option<CameraPreview>,
    decoded_text: Option<String>,
}

impl QrDebug {
    fn new(cx: &mut Context<Self>) -> Self {
        let animated_qr_tiny =
            cx.new(|cx| AnimatedQrCode::new("https://example.com/qr-debug-demo", px(50.0), cx));
        let animated_qr_small =
            cx.new(|cx| AnimatedQrCode::new("https://example.com/qr-debug-demo", px(80.0), cx));

        Self {
            entity: cx.entity().clone(),
            active_tab: Tab::Generate,
            animated_qr_tiny,
            animated_qr_small,
            camera_state: CameraState::Idle,
            scan_result_rx: None,
            stop_tx: None,
            last_preview: None,
            decoded_text: None,
        }
    }

    fn start_camera(&mut self, cx: &mut Context<Self>) {
        // Stop any existing camera
        self.stop_camera();

        self.camera_state = CameraState::Active;
        self.decoded_text = None;
        self.last_preview = None;

        let (result_tx, result_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();

        self.scan_result_rx = Some(result_rx);
        self.stop_tx = Some(stop_tx);

        // Spawn background thread: request permission (macOS), then start capture
        thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                if !request_camera_permission_macos() {
                    let _ = result_tx.send(QrScanResult::Error(
                        "Camera access denied. Grant permission in System Settings > \
                         Privacy & Security > Camera."
                            .to_string(),
                    ));
                    return;
                }
            }

            if let Err(e) = camera_capture_loop(result_tx.clone(), stop_rx) {
                let _ = result_tx.send(QrScanResult::Error(e));
            }
        });

        // Spawn polling loop to pull frames into the UI
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                smol::Timer::after(Duration::from_millis(50)).await;
                let Ok(()) = this.update(cx, |this, cx| {
                    this.poll_camera(cx);
                }) else {
                    break;
                };
            }
        })
        .detach();
    }

    fn stop_camera(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        self.scan_result_rx = None;
        if self.camera_state == CameraState::Active {
            self.camera_state = CameraState::Idle;
        }
    }

    fn poll_camera(&mut self, cx: &mut Context<Self>) {
        let Some(rx) = self.scan_result_rx.take() else {
            return;
        };

        // Drain all pending results, keep the latest
        let mut changed = false;
        while let Ok(result) = rx.try_recv() {
            match result {
                QrScanResult::Success(text, preview) => {
                    self.decoded_text = Some(text);
                    self.last_preview = Some(preview);
                    changed = true;
                }
                QrScanResult::Scanning(preview) | QrScanResult::Detected(preview) => {
                    self.last_preview = Some(preview);
                    changed = true;
                }
                QrScanResult::Error(e) => {
                    self.camera_state = CameraState::Denied(e);
                    // Don't call stop_camera here — we already took rx
                    if let Some(tx) = self.stop_tx.take() {
                        let _ = tx.send(());
                    }
                    changed = true;
                }
            }
        }

        // Put rx back unless we got an error
        if !matches!(self.camera_state, CameraState::Denied(_)) {
            self.scan_result_rx = Some(rx);
        }

        if changed {
            cx.notify();
        }
    }

    // ----- Render helpers -----

    fn render_generate_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .flex_col()
            .gap_6()
            // Static QR
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Static QR Codes").weight(TextWeight::Bold))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .items_end()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Text::new("200px").muted(true))
                                    .child(QrCode::new("https://github.com/anthropics/claude-code")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Text::new("120px").muted(true))
                                    .child(
                                        QrCode::new("https://example.com")
                                            .size(px(120.0)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Text::new("Custom colors").muted(true))
                                    .child(
                                        QrCode::new("SOTF Audio Engine")
                                            .size(px(150.0))
                                            .fg(rgba(0x2da44eff))
                                            .bg(rgba(0x1a1a2eff)),
                                    ),
                            ),
                    ),
            )
            // Animated QR
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Animated QR Codes (too small — auto-pans)").weight(TextWeight::Bold))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .items_end()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Text::new("50px").muted(true))
                                    .child(self.animated_qr_tiny.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Text::new("80px").muted(true))
                                    .child(self.animated_qr_small.clone()),
                            ),
                    ),
            )
            // Border around section
            .p_4()
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
    }

    fn render_scan_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let entity = self.entity.clone();

        let mut content = div().flex().flex_col().gap_4();

        match &self.camera_state {
            CameraState::Idle => {
                // Show start button
                content = content
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_4()
                            .py_8()
                            .child(Text::new("Point your camera at a QR code to scan it."))
                            .child(
                                Text::new(
                                    "Camera permission will be requested when you start scanning.",
                                )
                                .muted(true),
                            )
                            .child(
                                div()
                                    .id("start-scan-btn")
                                    .px_6()
                                    .py_3()
                                    .bg(theme.accent)
                                    .text_color(rgba(0xffffffff))
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .hover(|s| s.opacity(0.9))
                                    .child("Start Camera")
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        move |_event, _window, cx| {
                                            entity.update(cx, |this, cx| {
                                                this.start_camera(cx);
                                            });
                                        },
                                    ),
                            ),
                    );
            }

            CameraState::Denied(err) => {
                // Show error with instructions
                let surface_hover = theme.surface_hover;
                content = content.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_6()
                        .bg(rgba(0xff000015))
                        .border_1()
                        .border_color(theme.error)
                        .rounded_lg()
                        .child(
                            Text::new("Camera Unavailable")
                                .weight(TextWeight::Bold)
                                .color(theme.error),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_secondary)
                                .child(err.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .text_sm()
                                .text_color(theme.text_muted)
                                .child("To grant camera access:")
                                .child("1. Open System Settings > Privacy & Security > Camera")
                                .child("2. Enable access for this application")
                                .child("3. Restart the app and try again"),
                        )
                        .child(
                            div()
                                .id("retry-scan-btn")
                                .mt_2()
                                .px_4()
                                .py_2()
                                .bg(theme.surface)
                                .border_1()
                                .border_color(theme.border)
                                .rounded_md()
                                .cursor_pointer()
                                .text_sm()
                                .hover(move |s| s.bg(surface_hover))
                                .child("Retry")
                                .on_mouse_up(
                                    MouseButton::Left,
                                    {
                                        let entity = self.entity.clone();
                                        move |_event, _window, cx| {
                                            entity.update(cx, |this, cx| {
                                                this.camera_state = CameraState::Idle;
                                                this.start_camera(cx);
                                            });
                                        }
                                    },
                                ),
                        ),
                );
            }

            CameraState::Active => {
                // Camera preview
                let stop_entity = self.entity.clone();
                let surface_hover = theme.surface_hover;

                // Status line
                let status = if self.decoded_text.is_some() {
                    "QR Code Detected!"
                } else {
                    "Scanning..."
                };

                let status_color = if self.decoded_text.is_some() {
                    theme.success
                } else {
                    theme.text_muted
                };

                content = content
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                Text::new(status)
                                    .weight(TextWeight::Semibold)
                                    .color(status_color),
                            )
                            .child(
                                div()
                                    .id("stop-scan-btn")
                                    .px_4()
                                    .py_2()
                                    .bg(theme.surface)
                                    .border_1()
                                    .border_color(theme.border)
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_sm()
                                    .hover(move |s| s.bg(surface_hover))
                                    .child("Stop Camera")
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        move |_event, _window, cx| {
                                            stop_entity.update(cx, |this, cx| {
                                                this.stop_camera();
                                                cx.notify();
                                            });
                                        },
                                    ),
                            ),
                    );

                // Camera preview canvas
                if let Some(preview) = &self.last_preview {
                    let rgb = preview.rgb_pixels.clone();
                    let pw = preview.width;
                    let ph = preview.height;
                    let qr_bounds = preview.qr_bounds;
                    let accent = theme.accent;
                    let success = theme.success;
                    let has_decode = self.decoded_text.is_some();

                    content = content.child(
                        div()
                            .border_1()
                            .border_color(if has_decode {
                                theme.success
                            } else {
                                theme.border
                            })
                            .rounded_lg()
                            .overflow_hidden()
                            .child(
                                canvas(
                                    move |_bounds, _window, _cx| (rgb, pw, ph, qr_bounds),
                                    move |bounds, (rgb, pw, ph, qr_bounds), window, _cx| {
                                        paint_camera_preview(
                                            bounds, &rgb, pw, ph, qr_bounds, accent,
                                            success, has_decode, window,
                                        );
                                    },
                                )
                                .w(px(PREVIEW_WIDTH as f32 * 2.0))
                                .h(px(PREVIEW_HEIGHT as f32 * 2.0)),
                            ),
                    );
                } else {
                    // No frame yet — show placeholder
                    content = content.child(
                        div()
                            .w(px(PREVIEW_WIDTH as f32 * 2.0))
                            .h(px(PREVIEW_HEIGHT as f32 * 2.0))
                            .bg(rgba(0x1a1a1aff))
                            .border_1()
                            .border_color(theme.border)
                            .rounded_lg()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Text::new("Initializing camera...").muted(true)),
                    );
                }

                // Decoded result
                if let Some(text) = &self.decoded_text {
                    content = content.child(
                        div()
                            .p_4()
                            .bg(rgba(0x00ff0010))
                            .border_1()
                            .border_color(theme.success)
                            .rounded_lg()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                Text::new("Decoded Content:")
                                    .weight(TextWeight::Bold)
                                    .color(theme.success),
                            )
                            .child(
                                div()
                                    .p_3()
                                    .bg(theme.surface)
                                    .rounded_md()
                                    .text_sm()
                                    .text_color(theme.text_primary)
                                    .child(text.clone()),
                            ),
                    );
                }
            }
        }

        content
            .p_4()
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
    }
}

impl Drop for QrDebug {
    fn drop(&mut self) {
        self.stop_camera();
    }
}

impl Render for QrDebug {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let entity = self.entity.clone();
        let active_tab = self.active_tab;

        // Tab bar
        let tab_bar = div().flex().gap_1().mb_4().child(
            {
                let entity = entity.clone();
                tab_button("tab-generate", "Generate QR", active_tab == Tab::Generate, &theme)
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.active_tab = Tab::Generate;
                            cx.notify();
                        });
                    })
            },
        )
        .child(
            {
                let entity = entity.clone();
                tab_button("tab-scan", "Scan QR", active_tab == Tab::Scan, &theme)
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        entity.update(cx, |this, cx| {
                            this.active_tab = Tab::Scan;
                            cx.notify();
                        });
                    })
            },
        );

        let content = match active_tab {
            Tab::Generate => self.render_generate_tab(cx).into_any_element(),
            Tab::Scan => self.render_scan_tab(cx).into_any_element(),
        };

        div()
            .id("qr-debug-root")
            .size_full()
            .bg(theme.background)
            .text_color(theme.text_primary)
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .child(Heading::h1("QR Code Debug"))
            .child(tab_bar)
            .child(content)
    }
}

// ---------------------------------------------------------------------------
// UI helpers
// ---------------------------------------------------------------------------

fn tab_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    theme: &gpui_ui_kit::theme::Theme,
) -> Stateful<Div> {
    let mut btn = div()
        .id(id)
        .px_4()
        .py_2()
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .font_weight(FontWeight::MEDIUM);

    if active {
        btn = btn
            .bg(theme.accent)
            .text_color(rgba(0xffffffff));
    } else {
        let hover_bg = theme.surface_hover;
        btn = btn
            .text_color(theme.text_secondary)
            .hover(move |s| s.bg(hover_bg));
    }

    btn.child(label)
}

// ---------------------------------------------------------------------------
// Camera preview painting
// ---------------------------------------------------------------------------

fn paint_camera_preview(
    bounds: Bounds<Pixels>,
    rgb: &[u8],
    pw: usize,
    ph: usize,
    qr_bounds: Option<[(f32, f32); 4]>,
    accent: Rgba,
    success: Rgba,
    has_decode: bool,
    window: &mut Window,
) {
    let bw: f32 = bounds.size.width.into();
    let bh: f32 = bounds.size.height.into();
    let ox: f32 = bounds.origin.x.into();
    let oy: f32 = bounds.origin.y.into();

    let scale_x = bw / pw as f32;
    let scale_y = bh / ph as f32;

    // Paint each pixel as a scaled quad
    // For performance at 320x240, each source pixel maps to a ~2x2 block
    for y in 0..ph {
        for x in 0..pw {
            let idx = (y * pw + x) * 3;
            if idx + 2 >= rgb.len() {
                break;
            }

            let r = rgb[idx] as f32 / 255.0;
            let g = rgb[idx + 1] as f32 / 255.0;
            let b = rgb[idx + 2] as f32 / 255.0;
            let color = Rgba { r, g, b, a: 1.0 };

            let px_x = ox + x as f32 * scale_x;
            let px_y = oy + y as f32 * scale_y;

            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(px(px_x), px(px_y)),
                    size: size(px(scale_x.ceil()), px(scale_y.ceil())),
                },
                corner_radii: Corners::default(),
                background: color.into(),
                border_widths: Edges::default(),
                border_color: color.into(),
                border_style: BorderStyle::default(),
            });
        }
    }

    // Draw QR bounding box overlay
    if let Some(corners) = qr_bounds {
        let highlight = if has_decode { success } else { accent };
        let line_w = 3.0_f32;

        // Draw lines between consecutive corners
        for i in 0..4 {
            let (x1, y1) = corners[i];
            let (x2, y2) = corners[(i + 1) % 4];

            let px1 = ox + x1 * bw;
            let py1 = oy + y1 * bh;
            let px2 = ox + x2 * bw;
            let py2 = oy + y2 * bh;

            // Approximate line as a thin quad
            let dx = px2 - px1;
            let dy = py2 - py1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1.0 {
                continue;
            }

            // Draw as a series of small dots along the line for simplicity
            let steps = (len / 2.0).ceil() as usize;
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let lx = px1 + dx * t;
                let ly = py1 + dy * t;
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(px(lx - line_w / 2.0), px(ly - line_w / 2.0)),
                        size: size(px(line_w), px(line_w)),
                    },
                    corner_radii: Corners::default(),
                    background: highlight.into(),
                    border_widths: Edges::default(),
                    border_color: highlight.into(),
                    border_style: BorderStyle::default(),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// macOS camera permission (in-process AVFoundation call)
// ---------------------------------------------------------------------------

// Request camera permission via AVFoundation's TCC integration.
// Must run in the same process so macOS associates the grant with our .app bundle identity.
#[cfg(target_os = "macos")]
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn request_camera_permission_macos() -> bool {
    use block::ConcreteBlock;
    use objc::runtime::{Class, BOOL, NO};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let cls = Class::get("AVCaptureDevice").expect("AVCaptureDevice class not found");

        // AVMediaTypeVideo = "vide"
        let media_type: *mut objc::runtime::Object =
            msg_send![Class::get("NSString").unwrap(), stringWithUTF8String: c"vide".as_ptr()];

        // Check current status first (0=notDetermined, 1=restricted, 2=denied, 3=authorized)
        let status: i64 = msg_send![cls, authorizationStatusForMediaType: media_type];
        if status == 3 {
            return true; // Already authorized
        }
        if status == 1 || status == 2 {
            return false; // Restricted or denied
        }

        // Status is notDetermined — trigger the system permission dialog
        let (tx, rx) = std::sync::mpsc::channel();
        let callback = ConcreteBlock::new(move |granted: BOOL| {
            let _ = tx.send(granted != NO);
        });
        let callback = callback.copy();

        let _: () = msg_send![cls, requestAccessForMediaType: media_type
                                        completionHandler: &*callback];

        rx.recv().unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Camera capture background thread
// ---------------------------------------------------------------------------

fn camera_error_message(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("lock") || lower.contains("busy") || lower.contains("in use") {
        format!(
            "Camera is in use by another application. \
             Close the other app and try again.\n\nDetails: {}",
            raw
        )
    } else if lower.contains("permission") || lower.contains("denied") || lower.contains("authorized") {
        format!(
            "Camera access denied. Grant permission in \
             System Settings > Privacy & Security > Camera.\n\nDetails: {}",
            raw
        )
    } else if lower.contains("not found") || lower.contains("no camera") || lower.contains("no device") {
        "No camera found. Connect a camera and try again.".to_string()
    } else {
        format!(
            "Could not open camera: {}",
            raw
        )
    }
}

fn camera_capture_loop(
    result_tx: mpsc::Sender<QrScanResult>,
    stop_rx: mpsc::Receiver<()>,
) -> Result<(), String> {
    let formats_to_try = [
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(1280, 720),
            FrameFormat::MJPEG,
            30,
        ))),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(1280, 720),
            FrameFormat::YUYV,
            30,
        ))),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(CameraFormat::new(
            Resolution::new(640, 480),
            FrameFormat::MJPEG,
            30,
        ))),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::None),
    ];

    let mut camera = None;
    let mut last_error = String::new();

    for requested in &formats_to_try {
        match Camera::new(CameraIndex::Index(0), *requested) {
            Ok(cam) => {
                camera = Some(cam);
                break;
            }
            Err(e) => {
                last_error = format!("{}", e);
            }
        }
    }

    let mut camera = camera.ok_or_else(|| camera_error_message(&last_error))?;

    camera
        .open_stream()
        .map_err(|e| camera_error_message(&e.to_string()))?;

    loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }

        let frame = match camera.frame() {
            Ok(f) => f,
            Err(_) => {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        let decoded = match frame.decode_image::<RgbFormat>() {
            Ok(img) => img,
            Err(_) => {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        let width = decoded.width() as usize;
        let height = decoded.height() as usize;
        let rgb_data = decoded.into_raw();

        // Grayscale for QR detection
        let mut gray_data = Vec::with_capacity(width * height);
        for chunk in rgb_data.chunks(3) {
            if chunk.len() == 3 {
                let gray =
                    (chunk[0] as u32 * 299 + chunk[1] as u32 * 587 + chunk[2] as u32 * 114) / 1000;
                gray_data.push(gray as u8);
            }
        }

        // Downsample for preview
        let preview_rgb = downsample_rgb(&rgb_data, width, height, PREVIEW_WIDTH, PREVIEW_HEIGHT);

        // Detect QR codes
        let mut decoder = rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| {
            gray_data.get(y * width + x).copied().unwrap_or(0)
        });
        let grids = decoder.detect_grids();

        if grids.is_empty() {
            let preview = CameraPreview {
                rgb_pixels: preview_rgb,
                width: PREVIEW_WIDTH,
                height: PREVIEW_HEIGHT,
                qr_bounds: None,
            };
            let _ = result_tx.send(QrScanResult::Scanning(preview));
        } else {
            let mut decoded_any = false;
            let mut qr_bounds = None;

            for grid in &grids {
                if qr_bounds.is_none() {
                    qr_bounds = Some(extract_qr_bounds(grid, width, height));
                }

                if let Ok((_meta, content)) = grid.decode() {
                    let preview = CameraPreview {
                        rgb_pixels: preview_rgb.clone(),
                        width: PREVIEW_WIDTH,
                        height: PREVIEW_HEIGHT,
                        qr_bounds,
                    };
                    let _ = result_tx.send(QrScanResult::Success(content, preview));
                    decoded_any = true;
                }
            }

            if !decoded_any {
                let preview = CameraPreview {
                    rgb_pixels: preview_rgb,
                    width: PREVIEW_WIDTH,
                    height: PREVIEW_HEIGHT,
                    qr_bounds,
                };
                let _ = result_tx.send(QrScanResult::Detected(preview));
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

fn downsample_rgb(
    src: &[u8],
    src_width: usize,
    src_height: usize,
    dst_width: usize,
    dst_height: usize,
) -> Vec<u8> {
    let mut dst = Vec::with_capacity(dst_width * dst_height * 3);
    let x_ratio = src_width as f32 / dst_width as f32;
    let y_ratio = src_height as f32 / dst_height as f32;

    for dst_y in 0..dst_height {
        for dst_x in 0..dst_width {
            let src_x = (dst_x as f32 * x_ratio) as usize;
            let src_y = (dst_y as f32 * y_ratio) as usize;
            let idx = (src_y * src_width + src_x) * 3;

            if idx + 2 < src.len() {
                dst.push(src[idx]);
                dst.push(src[idx + 1]);
                dst.push(src[idx + 2]);
            } else {
                dst.extend_from_slice(&[128, 128, 128]);
            }
        }
    }

    dst
}

fn extract_qr_bounds<G>(
    grid: &rqrr::Grid<G>,
    img_width: usize,
    img_height: usize,
) -> [(f32, f32); 4] {
    let bounds = &grid.bounds;
    let normalize = |p: rqrr::Point| {
        (
            p.x as f32 / img_width as f32,
            p.y as f32 / img_height as f32,
        )
    };
    [
        normalize(bounds[0]),
        normalize(bounds[1]),
        normalize(bounds[2]),
        normalize(bounds[3]),
    ]
}

fn main() {
    MiniApp::run(
        MiniAppConfig::new("QR Code Debug")
            .size(750.0, 800.0)
            .scrollable(true)
            .with_theme(true),
        |cx| cx.new(QrDebug::new),
    );
}
