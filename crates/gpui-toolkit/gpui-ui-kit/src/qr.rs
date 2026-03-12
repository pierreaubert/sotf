//! QR Code display component
//!
//! Renders an encoded QR code as a matrix of filled squares using GPUI's
//! low-level paint API. Suitable for sharing URLs, wallet addresses, or any
//! string data.
//!
//! Complements camera-based QR *reading* (e.g. via `nokhwa` + `rqrr`) by
//! providing the display/generation side.

use crate::theme::ThemeExt;
use gpui::prelude::*;
use gpui::*;
// Alias the `qrcode` crate types to avoid shadowing by our component struct.
use qrcode::QrCode as QrMatrix;
use qrcode::types::Color as QrColor;

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

    /// Build the canvas element with explicit colors (both owned — no borrowed theme).
    fn build(self, fg_color: Rgba, bg_color: Rgba) -> impl IntoElement {
        let requested_size = self.size;
        let size_f32: f32 = requested_size.into();

        // Encode the data at Medium ECC. On failure the canvas paints nothing.
        let matrix = QrMatrix::new(self.data.as_bytes()).ok();

        canvas(
            // prepaint: forward the encoded matrix to the paint callback.
            move |_bounds, _window, _cx| matrix,
            // paint: draw background then one quad per dark module.
            move |bounds, matrix, window, _cx| {
                // Paint background only when it carries visible opacity.
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

                let Some(matrix) = matrix else {
                    return;
                };

                let modules = matrix.width();
                if modules == 0 {
                    return;
                }

                let colors = matrix.to_colors();

                // Inset the matrix by a 4-module quiet zone on each side
                // so scanners can locate the finder patterns.
                let quiet = 4_usize;
                let total_modules = modules + quiet * 2;
                let module_px = size_f32 / total_modules as f32;

                let origin_x: f32 = bounds.origin.x.into();
                let origin_y: f32 = bounds.origin.y.into();

                for row in 0..modules {
                    for col in 0..modules {
                        if colors[row * modules + col] == QrColor::Dark {
                            let x = origin_x + (col + quiet) as f32 * module_px;
                            let y = origin_y + (row + quiet) as f32 * module_px;

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
