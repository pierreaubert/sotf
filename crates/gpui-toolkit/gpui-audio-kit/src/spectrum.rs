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

/// Label and normalized position for a spectrum axis tick.
#[derive(Clone, Debug, PartialEq)]
pub struct SpectrumAxisLabel {
    pub label: String,
    pub position: f32,
}

/// Label and normalized position for a fixed dB axis tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumDbAxisLabel {
    pub label: &'static str,
    pub position: f32,
}

/// Visual settings used by the reusable spectrum axis renderers.
#[derive(Clone)]
pub struct SpectrumAxisTheme {
    pub text_color: Rgba,
    pub text_size: Rems,
    pub db_axis_width: f32,
    pub db_axis_padding_right: Rems,
    pub db_label_offset_y: f32,
    pub frequency_axis_height: f32,
    pub frequency_label_offset_x: f32,
}

impl Default for SpectrumAxisTheme {
    fn default() -> Self {
        Self {
            text_color: rgba(0xa1a1aaff),
            text_size: rems(0.75),
            db_axis_width: 32.0,
            db_axis_padding_right: rems(0.25),
            db_label_offset_y: -6.0,
            frequency_axis_height: 20.0,
            frequency_label_offset_x: -12.0,
        }
    }
}

const SPECTRUM_STANDARD_FREQUENCIES: [f32; 15] = [
    20.0, 30.0, 50.0, 100.0, 200.0, 300.0, 500.0, 1000.0, 2000.0, 3000.0, 5000.0, 10000.0, 15000.0,
    20000.0, 24000.0,
];

const SPECTRUM_DB_AXIS_LABELS: [SpectrumDbAxisLabel; 5] = [
    SpectrumDbAxisLabel {
        label: "+3",
        position: 0.0,
    },
    SpectrumDbAxisLabel {
        label: "0",
        position: 0.029,
    },
    SpectrumDbAxisLabel {
        label: "-20",
        position: 0.223,
    },
    SpectrumDbAxisLabel {
        label: "-40",
        position: 0.417,
    },
    SpectrumDbAxisLabel {
        label: "-60",
        position: 0.612,
    },
];

/// Format a frequency value for compact spectrum axis labels.
pub fn format_spectrum_frequency_label(freq: f32) -> String {
    if freq >= 1000.0 {
        let khz = freq / 1000.0;
        if khz == khz.floor() {
            format!("{}k", khz as i32)
        } else {
            format!("{khz:.1}k")
        }
    } else if freq == freq.floor() {
        format!("{}", freq as i32)
    } else {
        format!("{freq:.0}")
    }
}

/// Calculate logarithmic position of a frequency within a range.
pub fn logarithmic_frequency_position(freq: f32, min_freq: f32, max_freq: f32) -> f32 {
    let (min_freq, max_freq) = valid_frequency_range(min_freq, max_freq);
    let freq = if freq.is_finite() {
        freq.clamp(min_freq, max_freq)
    } else {
        min_freq
    };
    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let denominator = log_max - log_min;
    if denominator.abs() <= f32::EPSILON {
        0.0
    } else {
        ((freq.log10() - log_min) / denominator).clamp(0.0, 1.0)
    }
}

/// Generate non-overlapping frequency labels for a logarithmic spectrum axis.
pub fn spectrum_frequency_axis_labels(min_freq: f32, max_freq: f32) -> Vec<SpectrumAxisLabel> {
    let (min_freq, max_freq) = valid_frequency_range(min_freq, max_freq);
    let mut labels = Vec::new();

    labels.push(SpectrumAxisLabel {
        label: format_spectrum_frequency_label(min_freq),
        position: 0.0,
    });

    for freq in SPECTRUM_STANDARD_FREQUENCIES {
        if freq > min_freq * 1.1 && freq < max_freq * 0.9 {
            labels.push(SpectrumAxisLabel {
                label: format_spectrum_frequency_label(freq),
                position: logarithmic_frequency_position(freq, min_freq, max_freq),
            });
        }
    }

    labels.push(SpectrumAxisLabel {
        label: format_spectrum_frequency_label(max_freq),
        position: 1.0,
    });

    let mut filtered = Vec::new();
    for label in labels {
        if filtered.is_empty()
            || filtered
                .last()
                .map(|last: &SpectrumAxisLabel| label.position - last.position > 0.08)
                .unwrap_or(true)
        {
            filtered.push(label);
        }
    }
    filtered
}

/// Fixed dB-axis labels used by spectrum analyzer views.
pub fn spectrum_db_axis_labels() -> &'static [SpectrumDbAxisLabel] {
    &SPECTRUM_DB_AXIS_LABELS
}

/// Render a horizontal logarithmic frequency axis.
pub fn render_spectrum_frequency_axis(
    min_freq: f32,
    max_freq: f32,
    theme: SpectrumAxisTheme,
) -> impl IntoElement {
    let freq_labels = spectrum_frequency_axis_labels(min_freq, max_freq);
    let text_color = theme.text_color;
    let text_size = theme.text_size;
    let axis_height = theme.frequency_axis_height;
    let label_offset_x = theme.frequency_label_offset_x;

    div()
        .w_full()
        .h(px(axis_height))
        .relative()
        .children(freq_labels.into_iter().map(move |label| {
            div()
                .absolute()
                .left(relative(label.position))
                .top_0()
                .text_size(text_size)
                .text_color(text_color)
                .child(div().ml(px(label_offset_x)).child(label.label))
        }))
}

/// Render a vertical dB axis for spectrum displays.
pub fn render_spectrum_db_axis(theme: SpectrumAxisTheme) -> impl IntoElement {
    let text_color = theme.text_color;
    let text_size = theme.text_size;
    let axis_width = theme.db_axis_width;
    let padding_right = theme.db_axis_padding_right;
    let label_offset_y = theme.db_label_offset_y;

    div()
        .w(px(axis_width))
        .h_full()
        .flex()
        .flex_col()
        .relative()
        .children(spectrum_db_axis_labels().iter().map(move |label| {
            div()
                .absolute()
                .top(relative(label.position))
                .right_0()
                .text_size(text_size)
                .text_color(text_color)
                .pr(padding_right)
                .child(div().mt(px(label_offset_y)).child(label.label))
        }))
}

fn valid_frequency_range(min_freq: f32, max_freq: f32) -> (f32, f32) {
    let min_freq = if min_freq.is_finite() && min_freq > 0.0 {
        min_freq
    } else {
        20.0
    };
    let max_freq = if max_freq.is_finite() && max_freq > min_freq {
        max_freq
    } else {
        (min_freq * 2.0).max(20_000.0)
    };
    (min_freq, max_freq)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn frequency_labels_use_compact_audio_format() {
        assert_eq!(format_spectrum_frequency_label(20.0), "20");
        assert_eq!(format_spectrum_frequency_label(1000.0), "1k");
        assert_eq!(format_spectrum_frequency_label(1500.0), "1.5k");
        assert_eq!(format_spectrum_frequency_label(20_000.0), "20k");
    }

    #[::core::prelude::v1::test]
    fn logarithmic_frequency_position_is_bounded() {
        assert_eq!(logarithmic_frequency_position(20.0, 20.0, 20_000.0), 0.0);
        assert_eq!(
            logarithmic_frequency_position(20_000.0, 20.0, 20_000.0),
            1.0
        );
        let mid = logarithmic_frequency_position(1000.0, 20.0, 20_000.0);
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(logarithmic_frequency_position(-1.0, 20.0, 20_000.0), 0.0);
        assert_eq!(
            logarithmic_frequency_position(f32::NAN, 20.0, 20_000.0),
            0.0
        );
    }

    #[::core::prelude::v1::test]
    fn frequency_axis_labels_include_bounds_and_avoid_overlap() {
        let labels = spectrum_frequency_axis_labels(20.0, 20_000.0);
        assert_eq!(labels.first().unwrap().label, "20");
        assert_eq!(labels.first().unwrap().position, 0.0);
        assert_eq!(labels.last().unwrap().label, "20k");
        assert_eq!(labels.last().unwrap().position, 1.0);

        for pair in labels.windows(2) {
            assert!(pair[1].position - pair[0].position > 0.08);
        }
    }

    #[::core::prelude::v1::test]
    fn db_axis_labels_are_stable() {
        let labels = spectrum_db_axis_labels();
        assert_eq!(labels[0].label, "+3");
        assert_eq!(labels[1].label, "0");
        assert_eq!(labels[4].label, "-60");
        assert!(
            labels
                .windows(2)
                .all(|pair| pair[0].position < pair[1].position)
        );
    }

    #[::core::prelude::v1::test]
    fn axis_render_helpers_are_constructible() {
        let theme = SpectrumAxisTheme::default();
        let _freq_axis = render_spectrum_frequency_axis(20.0, 20_000.0, theme.clone());
        let _db_axis = render_spectrum_db_axis(theme);
    }
}
