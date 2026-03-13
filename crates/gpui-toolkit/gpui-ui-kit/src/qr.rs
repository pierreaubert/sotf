//! QR Code display component
//!
//! Renders an encoded QR code as a matrix of filled squares using GPUI's
//! low-level paint API. Suitable for sharing URLs, wallet addresses, or any
//! string data.
//!
//! Two variants are provided:
//!
//! - [`QrCode`] — stateless `RenderOnce` for sizes large enough to display
//!   all modules legibly.
//! - [`AnimatedQrCode`] — stateful `Entity` that automatically pans a zoomed
//!   viewport across the QR when the display size is too small for modules to
//!   be individually distinguishable, then settles to show the full code.

use crate::theme::ThemeExt;
use gpui::prelude::*;
use gpui::*;
use qrcode::QrCode as QrMatrix;
use qrcode::types::Color as QrColor;
use std::time::{Duration, Instant};

/// Minimum module size in pixels below which animation is triggered.
const MIN_MODULE_PX: f32 = 2.0;

/// Quiet-zone width in modules on each side of the QR matrix.
const QUIET_ZONE: usize = 4;

// ---------------------------------------------------------------------------
// QrCode (stateless, RenderOnce)
// ---------------------------------------------------------------------------

/// A QR code display component.
///
/// Encodes a string at Medium error-correction level and renders each dark
/// module as a filled rectangle scaled to the requested pixel size.
///
/// # Example
///
/// ```ignore
/// QrCode::new("https://example.com")
///     .size(px(200.0))
/// ```
pub struct QrCode {
    /// Raw string content to encode.
    data: String,
    /// Rendered size in pixels (width and height; the code is always square).
    size: Pixels,
    /// Foreground (dark module) color. Defaults to theme's `text_primary`.
    fg: Option<Rgba>,
    /// Background color. Defaults to transparent.
    bg: Option<Rgba>,
}

impl QrCode {
    /// Create a new QR code component that encodes `data`.
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            size: px(200.0),
            fg: None,
            bg: None,
        }
    }

    /// Set the rendered size (both width and height) in pixels.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Override the foreground (dark module) color.
    pub fn fg(mut self, color: Rgba) -> Self {
        self.fg = Some(color);
        self
    }

    /// Override the background color.
    pub fn bg(mut self, color: Rgba) -> Self {
        self.bg = Some(color);
        self
    }

    /// Build the canvas element with explicit colors.
    fn build(self, fg_color: Rgba, bg_color: Rgba) -> impl IntoElement {
        let requested_size = self.size;
        let size_f32: f32 = requested_size.into();
        let matrix = QrMatrix::new(self.data.as_bytes()).ok();

        canvas(
            move |_bounds, _window, _cx| matrix,
            move |bounds, matrix, window, _cx| {
                paint_qr_full(bounds, &matrix, size_f32, fg_color, bg_color, window);
            },
        )
        .w(requested_size)
        .h(requested_size)
    }
}

impl RenderOnce for QrCode {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let fg_color = self.fg.unwrap_or(theme.text_primary);
        let bg_color = self.bg.unwrap_or(theme.transparent);
        self.build(fg_color, bg_color)
    }
}

impl IntoElement for QrCode {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

// ---------------------------------------------------------------------------
// AnimatedQrCode (stateful Entity)
// ---------------------------------------------------------------------------

/// An animated QR code that pans a zoomed viewport when the display size is
/// too small for modules to be individually legible.
///
/// When the QR fits comfortably, it renders identically to [`QrCode`].
///
/// # Example
///
/// ```ignore
/// // In a Context<Parent>:
/// let qr = cx.new(|cx| AnimatedQrCode::new("https://example.com", px(60.0), cx));
/// // In render:
/// parent.child(qr)
/// ```
pub struct AnimatedQrCode {
    /// Encoded QR matrix (None on encode failure).
    matrix: Option<QrMatrix>,
    /// Number of modules on one side of the QR.
    modules: usize,
    /// Display size in pixels.
    size: Pixels,
    /// Foreground color.
    fg: Option<Rgba>,
    /// Background color.
    bg: Option<Rgba>,
    /// Whether animation is needed.
    needs_animation: bool,
    /// Animation start time.
    start: Instant,
    /// Total cycle duration for one full pan traversal.
    cycle_duration: Duration,
    /// Zoom factor: how many times larger we render modules vs the tiny size.
    zoom: f32,
}

impl AnimatedQrCode {
    /// Create a new animated QR code.
    ///
    /// If the given `size` is too small for modules to be legible, a panning
    /// animation starts automatically. Otherwise it renders statically.
    pub fn new(data: impl AsRef<[u8]>, size: Pixels, cx: &mut Context<Self>) -> Self {
        let matrix = QrMatrix::new(data.as_ref()).ok();
        let modules = matrix.as_ref().map_or(0, |m| m.width());
        let size_f32: f32 = size.into();
        let total_modules = modules + QUIET_ZONE * 2;
        let module_px = if total_modules > 0 {
            size_f32 / total_modules as f32
        } else {
            0.0
        };

        let needs_animation = modules > 0 && module_px < MIN_MODULE_PX;

        // Compute zoom so that each module is at least MIN_MODULE_PX * 2 for
        // comfortable readability in the zoomed viewport.
        let zoom = if needs_animation {
            (MIN_MODULE_PX * 2.0 / module_px).max(1.0)
        } else {
            1.0
        };

        // Cycle duration scales with QR complexity: more modules → longer pan.
        // A full raster scan at comfortable speed.
        let rows_in_view = (size_f32 / (module_px * zoom)).ceil() as usize;
        let pan_rows = if modules > rows_in_view {
            modules - rows_in_view
        } else {
            1
        };
        let cycle_duration = Duration::from_millis((pan_rows as u64 * 400).max(2000));

        if needs_animation {
            cx.spawn(async move |this: WeakEntity<Self>, cx| {
                loop {
                    smol::Timer::after(Duration::from_millis(33)).await;
                    let alive = this.update(cx, |_this, cx| {
                        cx.notify();
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();
        }

        Self {
            matrix,
            modules,
            size,
            fg: None,
            bg: None,
            needs_animation,
            start: Instant::now(),
            cycle_duration,
            zoom,
        }
    }

    /// Override the foreground (dark module) color.
    pub fn fg(mut self, color: Rgba) -> Self {
        self.fg = Some(color);
        self
    }

    /// Override the background color.
    pub fn bg(mut self, color: Rgba) -> Self {
        self.bg = Some(color);
        self
    }
}

impl Render for AnimatedQrCode {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let fg_color = self.fg.unwrap_or(theme.text_primary);
        let bg_color = self.bg.unwrap_or(theme.transparent);
        let requested_size = self.size;
        let size_f32: f32 = requested_size.into();

        if !self.needs_animation || self.matrix.is_none() {
            // Static render — same as QrCode
            let colors: Vec<QrColor> = self
                .matrix
                .as_ref()
                .map(|m| m.to_colors())
                .unwrap_or_default();
            let modules = self.modules;

            return canvas(
                move |_bounds, _window, _cx| (colors, modules),
                move |bounds, (colors, modules), window, _cx| {
                    paint_qr_static(bounds, &colors, modules, size_f32, fg_color, bg_color, window);
                },
            )
            .w(requested_size)
            .h(requested_size)
            .into_any_element();
        }

        // Animated render: compute pan offset from elapsed time
        let elapsed = self.start.elapsed();
        let modules = self.modules;
        let zoom = self.zoom;
        let total_modules = modules + QUIET_ZONE * 2;
        let base_module_px = size_f32 / total_modules as f32;
        let zoomed_module_px = base_module_px * zoom;

        // How many modules fit in the viewport at the zoomed scale
        let viewport_modules = (size_f32 / zoomed_module_px).floor();
        // Total scrollable range in modules (including quiet zones)
        let scroll_range = total_modules as f32 - viewport_modules;

        // Ping-pong progress: 0→1→0 over cycle_duration
        let cycle_secs = self.cycle_duration.as_secs_f32();
        let raw_t = (elapsed.as_secs_f32() % (cycle_secs * 2.0)) / cycle_secs;
        let t = if raw_t <= 1.0 {
            raw_t
        } else {
            2.0 - raw_t
        };

        // Ease the progress for smooth motion
        let eased = ease_in_out_cubic(t);

        // Scroll both axes together (diagonal pan)
        let offset_modules = eased * scroll_range.max(0.0);

        let colors: Vec<QrColor> = self
            .matrix
            .as_ref()
            .map(|m| m.to_colors())
            .unwrap_or_default();

        canvas(
            move |_bounds, _window, _cx| colors,
            move |bounds, colors, window, _cx| {
                // Background
                if bg_color.a > 0.0 {
                    window.paint_quad(PaintQuad {
                        bounds,
                        corner_radii: Corners::default(),
                        background: bg_color.into(),
                        border_widths: Edges::default(),
                        border_color: bg_color.into(),
                        border_style: BorderStyle::default(),
                    });
                }

                if modules == 0 {
                    return;
                }

                let origin_x: f32 = bounds.origin.x.into();
                let origin_y: f32 = bounds.origin.y.into();

                // Pixel offset of the viewport into the full zoomed QR
                let pixel_offset = offset_modules * zoomed_module_px;

                for row in 0..modules {
                    for col in 0..modules {
                        if colors[row * modules + col] == QrColor::Dark {
                            let x = (col + QUIET_ZONE) as f32 * zoomed_module_px - pixel_offset;
                            let y = (row + QUIET_ZONE) as f32 * zoomed_module_px - pixel_offset;

                            // Clip to viewport
                            if x + zoomed_module_px < 0.0
                                || x > size_f32
                                || y + zoomed_module_px < 0.0
                                || y > size_f32
                            {
                                continue;
                            }

                            // Clamp to viewport edges
                            let draw_x = x.max(0.0);
                            let draw_y = y.max(0.0);
                            let draw_w =
                                (x + zoomed_module_px).min(size_f32) - draw_x;
                            let draw_h =
                                (y + zoomed_module_px).min(size_f32) - draw_y;

                            if draw_w > 0.0 && draw_h > 0.0 {
                                window.paint_quad(PaintQuad {
                                    bounds: Bounds {
                                        origin: point(
                                            px(origin_x + draw_x),
                                            px(origin_y + draw_y),
                                        ),
                                        size: size(px(draw_w), px(draw_h)),
                                    },
                                    corner_radii: Corners::default(),
                                    background: fg_color.into(),
                                    border_widths: Edges::default(),
                                    border_color: fg_color.into(),
                                    border_style: BorderStyle::default(),
                                });
                            }
                        }
                    }
                }
            },
        )
        .w(requested_size)
        .h(requested_size)
        .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// Shared paint helpers
// ---------------------------------------------------------------------------

/// Paint the full QR matrix (used by static QrCode).
fn paint_qr_full(
    bounds: Bounds<Pixels>,
    matrix: &Option<QrMatrix>,
    size_f32: f32,
    fg_color: Rgba,
    bg_color: Rgba,
    window: &mut Window,
) {
    if bg_color.a > 0.0 {
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::default(),
            background: bg_color.into(),
            border_widths: Edges::default(),
            border_color: bg_color.into(),
            border_style: BorderStyle::default(),
        });
    }

    let Some(matrix) = matrix else { return };
    let modules = matrix.width();
    if modules == 0 {
        return;
    }

    let colors = matrix.to_colors();
    let total_modules = modules + QUIET_ZONE * 2;
    let module_px = size_f32 / total_modules as f32;
    let origin_x: f32 = bounds.origin.x.into();
    let origin_y: f32 = bounds.origin.y.into();

    for row in 0..modules {
        for col in 0..modules {
            if colors[row * modules + col] == QrColor::Dark {
                let x = origin_x + (col + QUIET_ZONE) as f32 * module_px;
                let y = origin_y + (row + QUIET_ZONE) as f32 * module_px;
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(px(x), px(y)),
                        size: size(px(module_px), px(module_px)),
                    },
                    corner_radii: Corners::default(),
                    background: fg_color.into(),
                    border_widths: Edges::default(),
                    border_color: fg_color.into(),
                    border_style: BorderStyle::default(),
                });
            }
        }
    }
}

/// Paint a pre-extracted color slice (used by AnimatedQrCode static path).
fn paint_qr_static(
    bounds: Bounds<Pixels>,
    colors: &[QrColor],
    modules: usize,
    size_f32: f32,
    fg_color: Rgba,
    bg_color: Rgba,
    window: &mut Window,
) {
    if bg_color.a > 0.0 {
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::default(),
            background: bg_color.into(),
            border_widths: Edges::default(),
            border_color: bg_color.into(),
            border_style: BorderStyle::default(),
        });
    }

    if modules == 0 {
        return;
    }

    let total_modules = modules + QUIET_ZONE * 2;
    let module_px = size_f32 / total_modules as f32;
    let origin_x: f32 = bounds.origin.x.into();
    let origin_y: f32 = bounds.origin.y.into();

    for row in 0..modules {
        for col in 0..modules {
            if colors[row * modules + col] == QrColor::Dark {
                let x = origin_x + (col + QUIET_ZONE) as f32 * module_px;
                let y = origin_y + (row + QUIET_ZONE) as f32 * module_px;
                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(px(x), px(y)),
                        size: size(px(module_px), px(module_px)),
                    },
                    corner_radii: Corners::default(),
                    background: fg_color.into(),
                    border_widths: Edges::default(),
                    border_color: fg_color.into(),
                    border_style: BorderStyle::default(),
                });
            }
        }
    }
}

/// Cubic ease-in-out for smooth animation.
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}
