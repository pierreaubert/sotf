//! Raster glyph text rendering for chart labels.

use gpui::{Corners, IntoElement, RenderImage, Rgba, Styled, canvas, px};
use image::{Frame, RgbaImage};
use std::sync::{Arc, LazyLock};

static DEFAULT_FONT: &[u8] = include_bytes!("../../assets/DejaVuSansMono.ttf");
static FONT: LazyLock<fontdue::Font> = LazyLock::new(|| {
    fontdue::Font::from_bytes(DEFAULT_FONT, fontdue::FontSettings::default())
        .expect("failed to parse embedded d3rs label font")
});

#[derive(Debug, Clone)]
pub struct GlyphTextConfig {
    pub font_size: f32,
    pub color: Rgba,
    pub rotation: f32,
    pub letter_spacing: f32,
}

impl GlyphTextConfig {
    pub fn horizontal(font_size: f32, color: impl Into<Rgba>) -> Self {
        Self {
            font_size,
            color: color.into(),
            rotation: 0.0,
            letter_spacing: 0.0,
        }
    }

    pub fn rotated(font_size: f32, color: impl Into<Rgba>, rotation: f32) -> Self {
        Self {
            font_size,
            color: color.into(),
            rotation,
            letter_spacing: 0.0,
        }
    }

    pub fn vertical_bottom_to_top(font_size: f32, color: impl Into<Rgba>) -> Self {
        Self::rotated(font_size, color, -std::f32::consts::FRAC_PI_2)
    }

    pub fn with_letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }
}

impl Default for GlyphTextConfig {
    fn default() -> Self {
        Self::horizontal(
            12.0,
            Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphTextMetrics {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
}

#[derive(Debug, Clone)]
struct RasterText {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct RawMetrics {
    width: f32,
    min_y: f32,
    max_y: f32,
}

pub fn measure_glyph_text_width(text: &str, font_size: f32) -> f32 {
    measure_glyph_text(text, font_size).width
}

pub fn measure_glyph_text(text: &str, font_size: f32) -> GlyphTextMetrics {
    let raw = measure_raw(text, font_size, 0.0);
    let height = (raw.max_y - raw.min_y).max(font_size);
    GlyphTextMetrics {
        width: raw.width,
        height,
        ascent: (-raw.min_y).max(0.0),
        descent: raw.max_y.max(0.0),
    }
}

pub fn render_glyph_text(text: &str, config: &GlyphTextConfig) -> impl IntoElement {
    let text = text.to_string();
    let config = config.clone();
    let raster = rasterize_rotated_text(&text, &config);
    let width = raster.width.max(1) as f32;
    let height = raster.height.max(1) as f32;

    canvas(
        move |_bounds, _, _cx| {},
        move |bounds, _, window, _cx| {
            let raster = rasterize_rotated_text(&text, &config);
            paint_raster(window, bounds, raster);
        },
    )
    .w(px(width))
    .h(px(height))
}

pub fn paint_glyph_text_at(
    window: &mut gpui::Window,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: impl Into<Rgba>,
    rotation: f32,
) {
    let config = GlyphTextConfig::rotated(font_size, color, rotation);
    let raster = rasterize_rotated_text(text, &config);
    let width = raster.width.max(1) as f32;
    let height = raster.height.max(1) as f32;
    let bounds = gpui::Bounds {
        origin: gpui::point(px(x), px(y)),
        size: gpui::size(px(width), px(height)),
    };
    paint_raster(window, bounds, raster);
}

fn measure_raw(text: &str, font_size: f32, letter_spacing: f32) -> RawMetrics {
    if text.is_empty() {
        return RawMetrics {
            width: 0.0,
            min_y: -font_size * 0.8,
            max_y: font_size * 0.2,
        };
    }

    let mut width = 0.0;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (idx, c) in text.chars().enumerate() {
        let metrics = FONT.metrics(c, font_size);
        min_y = min_y.min(metrics.ymin as f32);
        max_y = max_y.max(metrics.ymin as f32 + metrics.height as f32);
        width += metrics.advance_width;
        if idx + 1 < text.chars().count() {
            width += letter_spacing;
        }
    }

    if !min_y.is_finite() || !max_y.is_finite() || min_y >= max_y {
        min_y = -font_size * 0.8;
        max_y = font_size * 0.2;
    }

    RawMetrics {
        width,
        min_y,
        max_y,
    }
}

fn rasterize_rotated_text(text: &str, config: &GlyphTextConfig) -> RasterText {
    let raster = rasterize_text(text, config);
    if config.rotation.abs() < 0.0001 {
        raster
    } else {
        rotate_raster(&raster, config.rotation)
    }
}

fn rasterize_text(text: &str, config: &GlyphTextConfig) -> RasterText {
    let raw = measure_raw(text, config.font_size, config.letter_spacing);
    let padding = (config.font_size * 0.25).ceil().max(2.0);
    let width = (raw.width + padding * 2.0).ceil().max(1.0) as u32;
    let height = (raw.max_y - raw.min_y + padding * 2.0).ceil().max(1.0) as u32;
    let baseline_y = padding - raw.min_y;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let mut cursor_x = padding;

    for c in text.chars() {
        let (metrics, bitmap) = FONT.rasterize(c, config.font_size);
        let glyph_x = cursor_x + metrics.xmin as f32;
        let glyph_y = baseline_y + metrics.ymin as f32;

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col];
                if alpha == 0 {
                    continue;
                }
                let x = (glyph_x + col as f32).round() as i32;
                let y = (glyph_y + row as f32).round() as i32;
                blend_pixel(&mut pixels, width, height, x, y, config.color, alpha);
            }
        }

        cursor_x += metrics.advance_width + config.letter_spacing;
    }

    RasterText {
        width,
        height,
        pixels,
    }
}

fn rotate_raster(src: &RasterText, rotation: f32) -> RasterText {
    let cos_r = rotation.cos();
    let sin_r = rotation.sin();
    let src_w = src.width as f32;
    let src_h = src.height as f32;
    let corners = [
        (-src_w / 2.0, -src_h / 2.0),
        (src_w / 2.0, -src_h / 2.0),
        (-src_w / 2.0, src_h / 2.0),
        (src_w / 2.0, src_h / 2.0),
    ];

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (x, y) in corners {
        let rx = x * cos_r - y * sin_r;
        let ry = x * sin_r + y * cos_r;
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }

    let dst_w = (max_x - min_x).ceil().max(1.0) as u32 + 2;
    let dst_h = (max_y - min_y).ceil().max(1.0) as u32 + 2;
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    let dst_cx = dst_w as f32 / 2.0;
    let dst_cy = dst_h as f32 / 2.0;
    let src_cx = src_w / 2.0;
    let src_cy = src_h / 2.0;

    for y in 0..src.height {
        for x in 0..src.width {
            let idx = ((y * src.width + x) * 4) as usize;
            let alpha = src.pixels[idx + 3];
            if alpha == 0 {
                continue;
            }

            let lx = x as f32 + 0.5 - src_cx;
            let ly = y as f32 + 0.5 - src_cy;
            let rx = lx * cos_r - ly * sin_r;
            let ry = lx * sin_r + ly * cos_r;
            let dx = (dst_cx + rx).round() as i32;
            let dy = (dst_cy + ry).round() as i32;
            blend_raw_pixel(&mut dst, dst_w, dst_h, dx, dy, &src.pixels[idx..idx + 4]);
        }
    }

    RasterText {
        width: dst_w,
        height: dst_h,
        pixels: dst,
    }
}

fn blend_pixel(pixels: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: Rgba, alpha: u8) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }

    let idx = ((y as u32 * width + x as u32) * 4) as usize;
    let src_a = (alpha as f32 / 255.0) * color.a.clamp(0.0, 1.0);
    let dst_a = pixels[idx + 3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }

    let src = [color.r, color.g, color.b];
    for channel in 0..3 {
        let dst = pixels[idx + channel] as f32 / 255.0;
        let out = (src[channel] * src_a + dst * dst_a * (1.0 - src_a)) / out_a;
        pixels[idx + channel] = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    pixels[idx + 3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
}

fn blend_raw_pixel(pixels: &mut [u8], width: u32, height: u32, x: i32, y: i32, src: &[u8]) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }

    let idx = ((y as u32 * width + x as u32) * 4) as usize;
    let src_a = src[3] as f32 / 255.0;
    let dst_a = pixels[idx + 3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }

    for channel in 0..3 {
        let src_c = src[channel] as f32 / 255.0;
        let dst_c = pixels[idx + channel] as f32 / 255.0;
        let out = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
        pixels[idx + channel] = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    pixels[idx + 3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
}

fn paint_raster(window: &mut gpui::Window, bounds: gpui::Bounds<gpui::Pixels>, raster: RasterText) {
    if let Some(rgba_image) = RgbaImage::from_raw(raster.width, raster.height, raster.pixels) {
        let frame = Frame::new(rgba_image);
        let render_image = RenderImage::new(vec![frame]);
        let _ = window.paint_image(bounds, Corners::default(), Arc::new(render_image), 0, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_metrics_are_nonzero() {
        let metrics = measure_glyph_text("1000 Hz", 12.0);
        assert!(metrics.width > 0.0);
        assert!(metrics.height > 0.0);
    }

    #[test]
    fn rotated_text_expands_bounds() {
        let config =
            GlyphTextConfig::rotated(14.0, gpui::rgb(0xffffff), std::f32::consts::FRAC_PI_2);
        let raster = rasterize_rotated_text("1000", &config);
        assert!(raster.width > 0);
        assert!(raster.height > raster.width);
    }
}
