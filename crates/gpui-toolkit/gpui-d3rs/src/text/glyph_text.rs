//! Raster glyph text rendering for chart labels.

use gpui::{canvas, div, px, Corners, IntoElement, ParentElement, RenderImage, Rgba, Styled};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalTextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalTextAnchor {
    Top,
    Middle,
    Alphabetic,
    Bottom,
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
    layout_width: f32,
    layout_height: f32,
    paint_offset: [f32; 2],
    anchor: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
struct RawMetrics {
    width: f32,
    ink_min_x: f32,
    ink_max_x: f32,
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
    render_glyph_text_anchored(
        text,
        config,
        HorizontalTextAnchor::Start,
        VerticalTextAnchor::Top,
    )
}

pub fn render_glyph_text_anchored(
    text: &str,
    config: &GlyphTextConfig,
    horizontal_anchor: HorizontalTextAnchor,
    vertical_anchor: VerticalTextAnchor,
) -> impl IntoElement {
    let text = text.to_string();
    let config = config.clone();
    let raster = rasterize_rotated_text(&text, &config, horizontal_anchor, vertical_anchor);
    let width = raster.layout_width.max(1.0);
    let height = raster.layout_height.max(1.0);
    let raster_width = raster.width.max(1) as f32;
    let raster_height = raster.height.max(1) as f32;
    let paint_offset = raster.paint_offset;
    let anchor = raster.anchor;

    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .ml(px(-anchor[0]))
        .mt(px(-anchor[1]))
        .child(
            canvas(
                move |_bounds, _, _cx| {},
                move |bounds, _, window, _cx| {
                    let raster =
                        rasterize_rotated_text(&text, &config, horizontal_anchor, vertical_anchor);
                    paint_raster(window, bounds, &raster);
                },
            )
            .absolute()
            .left(px(paint_offset[0]))
            .top(px(paint_offset[1]))
            .w(px(raster_width))
            .h(px(raster_height)),
        )
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
    let raster = rasterize_rotated_text(
        text,
        &config,
        HorizontalTextAnchor::Start,
        VerticalTextAnchor::Top,
    );
    let bounds = gpui::Bounds {
        origin: gpui::point(
            px(x + raster.paint_offset[0]),
            px(y + raster.paint_offset[1]),
        ),
        size: gpui::size(
            px(raster.width.max(1) as f32),
            px(raster.height.max(1) as f32),
        ),
    };
    paint_raster(window, bounds, &raster);
}

fn measure_raw(text: &str, font_size: f32, letter_spacing: f32) -> RawMetrics {
    if text.is_empty() {
        return RawMetrics {
            width: 0.0,
            ink_min_x: 0.0,
            ink_max_x: 0.0,
            min_y: -font_size * 0.8,
            max_y: font_size * 0.2,
        };
    }

    let mut width = 0.0;
    let mut ink_min_x = f32::INFINITY;
    let mut ink_max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let char_count = text.chars().count();

    for (idx, c) in text.chars().enumerate() {
        let metrics = FONT.metrics(c, font_size);
        let glyph_x = glyph_left(width, &metrics);
        let glyph_y = glyph_top(&metrics);

        if metrics.width > 0 && metrics.height > 0 {
            ink_min_x = ink_min_x.min(glyph_x);
            ink_max_x = ink_max_x.max(glyph_x + metrics.width as f32);
            min_y = min_y.min(glyph_y);
            max_y = max_y.max(glyph_y + metrics.height as f32);
        }

        width += glyph_advance(&metrics);
        if idx + 1 < char_count {
            width += letter_spacing;
        }
    }

    if !ink_min_x.is_finite() || !ink_max_x.is_finite() || ink_min_x >= ink_max_x {
        ink_min_x = 0.0;
        ink_max_x = width.max(1.0);
    }

    if !min_y.is_finite() || !max_y.is_finite() || min_y >= max_y {
        min_y = -font_size * 0.8;
        max_y = font_size * 0.2;
    }

    RawMetrics {
        width,
        ink_min_x,
        ink_max_x,
        min_y,
        max_y,
    }
}

fn glyph_advance(metrics: &fontdue::Metrics) -> f32 {
    metrics.advance_width.ceil()
}

fn glyph_left(cursor_x: f32, metrics: &fontdue::Metrics) -> f32 {
    (cursor_x + metrics.bounds.xmin).floor()
}

fn glyph_top(metrics: &fontdue::Metrics) -> f32 {
    (-metrics.bounds.height - metrics.bounds.ymin).floor()
}

fn rasterize_rotated_text(
    text: &str,
    config: &GlyphTextConfig,
    horizontal_anchor: HorizontalTextAnchor,
    vertical_anchor: VerticalTextAnchor,
) -> RasterText {
    let raster = rasterize_text(text, config, horizontal_anchor, vertical_anchor);
    if config.rotation.abs() < 0.0001 {
        raster
    } else {
        rotate_raster(&raster, config.rotation)
    }
}

fn rasterize_text(
    text: &str,
    config: &GlyphTextConfig,
    horizontal_anchor: HorizontalTextAnchor,
    vertical_anchor: VerticalTextAnchor,
) -> RasterText {
    let raw = measure_raw(text, config.font_size, config.letter_spacing);
    let padding = (config.font_size * 0.25).ceil().max(2.0);
    let layout_width = raw.width.max(0.0);
    let layout_height = (raw.max_y - raw.min_y).max(1.0);
    let width = (raw.ink_max_x - raw.ink_min_x + padding * 2.0)
        .ceil()
        .max(1.0) as u32;
    let height = (layout_height + padding * 2.0).ceil().max(1.0) as u32;
    let baseline_y = padding - raw.min_y;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let mut cursor_x = 0.0;

    for c in text.chars() {
        let (metrics, bitmap) = FONT.rasterize(c, config.font_size);
        let glyph_x = glyph_left(cursor_x, &metrics) - raw.ink_min_x + padding;
        let glyph_y = baseline_y + glyph_top(&metrics);

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

        cursor_x += glyph_advance(&metrics) + config.letter_spacing;
    }

    RasterText {
        width,
        height,
        pixels,
        layout_width,
        layout_height,
        paint_offset: [raw.ink_min_x - padding, -padding],
        anchor: [
            horizontal_anchor_offset(layout_width, horizontal_anchor),
            vertical_anchor_offset(&raw, vertical_anchor),
        ],
    }
}

fn horizontal_anchor_offset(width: f32, anchor: HorizontalTextAnchor) -> f32 {
    match anchor {
        HorizontalTextAnchor::Start => 0.0,
        HorizontalTextAnchor::Middle => width / 2.0,
        HorizontalTextAnchor::End => width,
    }
}

fn vertical_anchor_offset(raw: &RawMetrics, anchor: VerticalTextAnchor) -> f32 {
    match anchor {
        VerticalTextAnchor::Top => 0.0,
        VerticalTextAnchor::Middle => (raw.max_y - raw.min_y) / 2.0,
        VerticalTextAnchor::Alphabetic => -raw.min_y,
        VerticalTextAnchor::Bottom => raw.max_y - raw.min_y,
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

    let anchor_lx = src.anchor[0] - src_cx;
    let anchor_ly = src.anchor[1] - src_cy;
    let anchor_rx = anchor_lx * cos_r - anchor_ly * sin_r;
    let anchor_ry = anchor_lx * sin_r + anchor_ly * cos_r;

    RasterText {
        width: dst_w,
        height: dst_h,
        pixels: dst,
        layout_width: dst_w as f32,
        layout_height: dst_h as f32,
        paint_offset: [0.0, 0.0],
        anchor: [dst_cx + anchor_rx, dst_cy + anchor_ry],
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

fn paint_raster(
    window: &mut gpui::Window,
    bounds: gpui::Bounds<gpui::Pixels>,
    raster: &RasterText,
) {
    if let Some(rgba_image) =
        RgbaImage::from_raw(raster.width, raster.height, raster.pixels.clone())
    {
        let frame = Frame::new(rgba_image);
        let render_image = RenderImage::new(vec![frame]);
        let _ = window.paint_image(bounds, Corners::default(), Arc::new(render_image), 0, false);
    }
}
