//! Spectrum analyzer primitives for audio UIs.

use gpui::prelude::*;
use gpui::*;
use std::panic;
use std::sync::Arc;

/// Colors used by the spectrum analyzer.
#[derive(Clone)]
pub struct SpectrumColors {
    pub background: Rgba,
    pub low: Rgba,
    pub mid: Rgba,
    pub high: Rgba,
}

impl Default for SpectrumColors {
    fn default() -> Self {
        Self {
            background: rgba(0x151515ff),
            low: rgba(0x4caf50ff),
            mid: rgba(0xffc107ff),
            high: rgba(0xf44336ff),
        }
    }
}

/// GPU-accelerated spectrum analyzer element.
pub struct SpectrumElement {
    magnitudes: Arc<[f32]>,
    min_freq: f32,
    max_freq: f32,
    smoothing: f32,
    previous_magnitudes: Option<Arc<[f32]>>,
    colors: SpectrumColors,
    height: Pixels,
    bar_gap: Pixels,
}

impl SpectrumElement {
    pub fn new(magnitudes: impl Into<Arc<[f32]>>) -> Self {
        Self {
            magnitudes: magnitudes.into(),
            min_freq: 20.0,
            max_freq: 20000.0,
            smoothing: 0.3,
            previous_magnitudes: None,
            colors: SpectrumColors::default(),
            height: px(120.0),
            bar_gap: px(1.0),
        }
    }

    pub fn frequency_range(mut self, min: f32, max: f32) -> Self {
        self.min_freq = min;
        self.max_freq = max;
        self
    }

    pub fn smoothing(mut self, smoothing: f32) -> Self {
        self.smoothing = smoothing.clamp(0.0, 0.99);
        self
    }

    pub fn previous(mut self, previous: impl Into<Arc<[f32]>>) -> Self {
        self.previous_magnitudes = Some(previous.into());
        self
    }

    pub fn colors(mut self, colors: SpectrumColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    pub fn bar_gap(mut self, gap: Pixels) -> Self {
        self.bar_gap = gap;
        self
    }

    fn db_to_height(&self, db: f32) -> f32 {
        ((db + 100.0) / 103.0).clamp(0.0, 1.0)
    }
}

impl IntoElement for SpectrumElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SpectrumElement {
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
        let layout_id = window.request_layout(
            Style {
                size: size(relative(1.0).into(), self.height.into()),
                min_size: size(px(100.0).into(), px(60.0).into()),
                ..Default::default()
            },
            [],
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
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
        let bar_count = self.magnitudes.len();
        if bar_count == 0 {
            return;
        }

        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(px(4.0)),
            background: self.colors.background.into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

        let yellow_threshold = self.db_to_height(-6.0);
        let red_threshold = self.db_to_height(-1.0);
        let step_width = bounds.size.width / bar_count as f32;
        let meter_height = bounds.size.height;

        let smoothed_heights: Vec<f32> = self
            .magnitudes
            .iter()
            .enumerate()
            .map(|(i, &mag)| {
                let smoothed_mag = if let Some(ref prev) = self.previous_magnitudes {
                    if i < prev.len() {
                        prev[i] * self.smoothing + mag * (1.0 - self.smoothing)
                    } else {
                        mag
                    }
                } else {
                    mag
                };
                self.db_to_height(smoothed_mag)
            })
            .collect();

        let mut green_path = PathBuilder::fill();
        green_path.move_to(point(bounds.origin.x, bounds.origin.y + meter_height));
        let mut yellow_path = PathBuilder::fill();
        let mut has_yellow = false;
        let mut red_path = PathBuilder::fill();
        let mut has_red = false;

        for (i, &height_ratio) in smoothed_heights.iter().enumerate() {
            let x = bounds.origin.x + step_width * i as f32;
            let green_height = height_ratio.min(yellow_threshold);
            let green_y = bounds.origin.y + meter_height - (meter_height * green_height);
            green_path.line_to(point(x, green_y));
            green_path.line_to(point(x + step_width, green_y));

            if height_ratio > yellow_threshold {
                if !has_yellow {
                    has_yellow = true;
                    yellow_path.move_to(point(
                        bounds.origin.x,
                        bounds.origin.y + meter_height - (meter_height * yellow_threshold),
                    ));
                }
                let yellow_height =
                    (height_ratio - yellow_threshold).min(red_threshold - yellow_threshold);
                let yellow_top = yellow_threshold + yellow_height;
                let yellow_y = bounds.origin.y + meter_height - (meter_height * yellow_top);
                yellow_path.line_to(point(x, yellow_y));
                yellow_path.line_to(point(x + step_width, yellow_y));
            } else if has_yellow {
                let yellow_bottom_y =
                    bounds.origin.y + meter_height - (meter_height * yellow_threshold);
                yellow_path.line_to(point(x, yellow_bottom_y));
                yellow_path.line_to(point(x + step_width, yellow_bottom_y));
            }

            if height_ratio > red_threshold {
                if !has_red {
                    has_red = true;
                    red_path.move_to(point(
                        bounds.origin.x,
                        bounds.origin.y + meter_height - (meter_height * red_threshold),
                    ));
                }
                let red_height = height_ratio - red_threshold;
                let red_top = red_threshold + red_height;
                let red_y = bounds.origin.y + meter_height - (meter_height * red_top);
                red_path.line_to(point(x, red_y));
                red_path.line_to(point(x + step_width, red_y));
            } else if has_red {
                let red_bottom_y = bounds.origin.y + meter_height - (meter_height * red_threshold);
                red_path.line_to(point(x, red_bottom_y));
                red_path.line_to(point(x + step_width, red_bottom_y));
            }
        }

        green_path.line_to(point(
            bounds.origin.x + bounds.size.width,
            bounds.origin.y + meter_height,
        ));
        green_path.line_to(point(bounds.origin.x, bounds.origin.y + meter_height));
        if let Ok(path) = green_path.build() {
            window.paint_path(path, self.colors.low);
        }

        if has_yellow {
            let yellow_bottom_y =
                bounds.origin.y + meter_height - (meter_height * yellow_threshold);
            yellow_path.line_to(point(bounds.origin.x + bounds.size.width, yellow_bottom_y));
            yellow_path.line_to(point(bounds.origin.x, yellow_bottom_y));
            if let Ok(path) = yellow_path.build() {
                window.paint_path(path, self.colors.mid);
            }
        }

        if has_red {
            let red_bottom_y = bounds.origin.y + meter_height - (meter_height * red_threshold);
            red_path.line_to(point(bounds.origin.x + bounds.size.width, red_bottom_y));
            red_path.line_to(point(bounds.origin.x, red_bottom_y));
            if let Ok(path) = red_path.build() {
                window.paint_path(path, self.colors.high);
            }
        }
    }
}

/// A group of level meters with smoothed animation.
#[derive(Clone)]
pub struct MeterData {
    pub levels: Vec<f32>,
    pub peaks: Vec<f32>,
    pub names: Vec<String>,
}

impl MeterData {
    pub fn new(channels: usize) -> Self {
        Self {
            levels: vec![0.0; channels],
            peaks: vec![0.0; channels],
            names: (0..channels).map(|i| format!("CH{}", i + 1)).collect(),
        }
    }

    pub fn update(&mut self, new_levels: &[f32], smoothing: f32) {
        for (i, &new_level) in new_levels.iter().enumerate() {
            if i < self.levels.len() {
                self.levels[i] = self.levels[i] * smoothing + new_level * (1.0 - smoothing);
                if new_level > self.peaks[i] {
                    self.peaks[i] = new_level;
                } else {
                    self.peaks[i] *= 0.995;
                }
            }
        }
    }
}

fn rgba(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 24) & 0xff) as f32 / 255.0,
        g: ((hex >> 16) & 0xff) as f32 / 255.0,
        b: ((hex >> 8) & 0xff) as f32 / 255.0,
        a: (hex & 0xff) as f32 / 255.0,
    }
}
