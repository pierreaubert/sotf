//! Spectrum Analyzer UI Components
//!
//! This module consolidates all spectrum analyzer functionality:
//! - GPU-accelerated spectrum element (`SpectrumElement`)
//! - Plugin parameter editing UI
//! - Full-screen spectrum display
//! - Meter data for level meters with smoothed animation

use std::panic;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;

use super::common::{render_edit_hints, render_knob, render_section_header};
use crate::theme::Theme;
use crate::ui::PlayerView;

// ============================================================================
// GPU-Accelerated Spectrum Element
// ============================================================================

/// Colors used by the spectrum analyzer
#[derive(Clone)]
pub struct SpectrumColors {
    /// Background color
    pub background: Rgba,
    /// Low frequency (bass) color
    pub low: Rgba,
    /// Mid frequency color
    pub mid: Rgba,
    /// High frequency (treble) color
    pub high: Rgba,
}

impl Default for SpectrumColors {
    fn default() -> Self {
        Self {
            background: rgb(0x000000),
            low: rgb(0x22c55e),  // Green for bass
            mid: rgb(0xeab308),  // Yellow for mids
            high: rgb(0xef4444), // Red for highs
        }
    }
}

/// GPU-accelerated spectrum analyzer element
///
/// Renders a frequency spectrum with bars colored by frequency band
/// using direct GPU quad rendering for maximum performance.
pub struct SpectrumElement {
    /// Magnitude values for each frequency bin (in dB, typically -100 to 0)
    magnitudes: Arc<[f32]>,
    /// Min frequency (Hz) for display
    min_freq: f32,
    /// Max frequency (Hz) for display
    max_freq: f32,
    /// Smoothing factor for animation (0.0 = no smoothing, 1.0 = max smoothing)
    smoothing: f32,
    /// Previous magnitudes for smoothing animation
    previous_magnitudes: Option<Arc<[f32]>>,
    /// Colors for frequency bands
    colors: SpectrumColors,
    /// Height of the element
    height: Pixels,
    /// Gap between bars
    bar_gap: Pixels,
}

impl SpectrumElement {
    /// Create a new spectrum element with the given magnitude data
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

    /// Set the frequency range for display labels
    pub fn frequency_range(mut self, min: f32, max: f32) -> Self {
        self.min_freq = min;
        self.max_freq = max;
        self
    }

    /// Set the smoothing factor for animation
    pub fn smoothing(mut self, smoothing: f32) -> Self {
        self.smoothing = smoothing.clamp(0.0, 0.99);
        self
    }

    /// Set previous magnitudes for smooth animation
    pub fn previous(mut self, previous: impl Into<Arc<[f32]>>) -> Self {
        self.previous_magnitudes = Some(previous.into());
        self
    }

    /// Set custom colors
    pub fn colors(mut self, colors: SpectrumColors) -> Self {
        self.colors = colors;
        self
    }

    /// Set the height
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// Set the gap between bars
    pub fn bar_gap(mut self, gap: Pixels) -> Self {
        self.bar_gap = gap;
        self
    }

    /// Convert dB value to normalized height (0.0 to 1.0)
    fn db_to_height(&self, db: f32) -> f32 {
        // Assume range is -100 dB to 0 dB
        ((db + 100.0) / 100.0).clamp(0.0, 1.0)
    }

    /// Get color for a frequency bin based on its position
    fn bin_color(&self, bin_index: usize, total_bins: usize) -> Rgba {
        let t = bin_index as f32 / total_bins as f32;
        if t < 0.3 {
            self.colors.low
        } else if t < 0.7 {
            self.colors.mid
        } else {
            self.colors.high
        }
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

        // Paint background
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(px(4.0)),
            background: self.colors.background.into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

        // Calculate bar width
        let total_gap = self.bar_gap * (bar_count as f32 - 1.0);
        let available_width = bounds.size.width - total_gap - px(4.0); // 2px padding each side
        let bar_width = available_width / bar_count as f32;

        // Paint each bar
        for (i, &mag) in self.magnitudes.iter().enumerate() {
            // Apply smoothing if we have previous values
            let smoothed_mag = if let Some(ref prev) = self.previous_magnitudes {
                if i < prev.len() {
                    prev[i] * self.smoothing + mag * (1.0 - self.smoothing)
                } else {
                    mag
                }
            } else {
                mag
            };

            let height_ratio = self.db_to_height(smoothed_mag);
            let bar_height = bounds.size.height * height_ratio - px(4.0); // 2px padding top and bottom

            if bar_height > px(0.0) {
                let x = bounds.origin.x + px(2.0) + (bar_width + self.bar_gap) * i as f32;
                let y = bounds.origin.y + bounds.size.height - bar_height - px(2.0);

                let color = self.bin_color(i, bar_count);

                window.paint_quad(PaintQuad {
                    bounds: Bounds {
                        origin: point(x, y),
                        size: size(bar_width, bar_height),
                    },
                    corner_radii: Corners {
                        top_left: px(2.0),
                        top_right: px(2.0),
                        bottom_left: px(0.0),
                        bottom_right: px(0.0),
                    },
                    background: color.into(),
                    border_widths: Edges::default(),
                    border_color: Hsla::transparent_black(),
                    border_style: Default::default(),
                });
            }
        }
    }
}

// ============================================================================
// Meter Data
// ============================================================================

/// A group of level meters with smoothed animation
#[derive(Clone)]
pub struct MeterData {
    /// Current level values (in linear 0.0-1.0)
    pub levels: Vec<f32>,
    /// Peak hold values
    pub peaks: Vec<f32>,
    /// Channel names
    pub names: Vec<String>,
}

impl MeterData {
    /// Create empty meter data
    pub fn new(channels: usize) -> Self {
        Self {
            levels: vec![0.0; channels],
            peaks: vec![0.0; channels],
            names: (0..channels).map(|i| format!("CH{}", i + 1)).collect(),
        }
    }

    /// Update levels with smoothing
    pub fn update(&mut self, new_levels: &[f32], smoothing: f32) {
        for (i, &new_level) in new_levels.iter().enumerate() {
            if i < self.levels.len() {
                self.levels[i] = self.levels[i] * smoothing + new_level * (1.0 - smoothing);
                // Update peak with slow decay
                if new_level > self.peaks[i] {
                    self.peaks[i] = new_level;
                } else {
                    self.peaks[i] *= 0.995; // Slow peak decay
                }
            }
        }
    }
}

// ============================================================================
// Plugin UI
// ============================================================================

/// Render the Spectrum Analyzer plugin
pub fn render_spectrum_analyzer_plugin(
    plugin_idx: usize,
    num_bins: usize,
    min_freq: f32,
    max_freq: f32,
    smoothing: f32,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // Generate simulated spectrum bars for the plugin card view
    let bar_count = 32;
    let bars: Vec<f32> = (0..bar_count)
        .map(|i| {
            // Simulated frequency response curve
            let t = i as f32 / bar_count as f32;
            let peak = 0.5;
            let spread = 0.3;
            let value = (-(t - peak).powi(2) / (2.0 * spread * spread)).exp();
            value * 0.8 + 0.1
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Spectrum display section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_4()
                .child(render_section_header("SPECTRUM ANALYZER", theme))
                .child(
                    div()
                        .h(px(120.0))
                        .w_full()
                        .bg(theme.surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_end()
                        .gap_px()
                        .p_2()
                        .children(bars.into_iter().enumerate().map(|(i, height)| {
                            let t = i as f32 / bar_count as f32;
                            let color = if t < 0.3 {
                                theme.meter_normal
                            } else if t < 0.7 {
                                theme.meter_warning
                            } else {
                                theme.meter_clip
                            };
                            div().flex_1().h(relative(height)).bg(color).rounded_t_sm()
                        })),
                ),
        )
        // Controls
        .child(
            div()
                .flex()
                .gap_4()
                .child(render_knob(
                    plugin_idx,
                    "Bins",
                    num_bins as f64,
                    10.0,
                    100.0,
                    "",
                    0,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    plugin_idx,
                    "Min Hz",
                    min_freq as f64,
                    10.0,
                    1000.0,
                    "Hz",
                    1,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    plugin_idx,
                    "Max Hz",
                    max_freq as f64,
                    1000.0,
                    24000.0,
                    "Hz",
                    2,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    plugin_idx,
                    "Smooth",
                    smoothing as f64,
                    0.0,
                    1.0,
                    "",
                    3,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                )),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}

impl PlayerView {
    /// Render the full-screen spectrum analyzer display
    /// Uses GPU-accelerated SpectrumElement for high-performance rendering
    pub(crate) fn render_spectrum_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        let content = if let Some(info) = &state.app.spectrum_info {
            // Convert magnitudes to Arc for the GPU element
            let magnitudes: Arc<[f32]> = info.magnitudes.clone().into();

            div()
                .flex()
                .flex_col()
                .size_full()
                // GPU-accelerated spectrum visualization
                .child(
                    SpectrumElement::new(magnitudes)
                        .height(px(256.0))
                        .frequency_range(20.0, 20000.0)
                        .smoothing(0.3),
                )
                // Frequency labels
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child("20 Hz")
                        .child("1 kHz")
                        .child("20 kHz"),
                )
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(theme.text_muted)
                .child("No spectrum data available. Play audio to see visualization.")
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Spectrum Analyzer"),
            )
            .child(content)
    }
}
