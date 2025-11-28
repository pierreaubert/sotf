//! GPU-accelerated Spectrum Analyzer Element
//!
//! A custom Element implementation for rendering audio spectrum analyzers
//! with direct GPU drawing for high performance and smooth animation.

use gpui::*;
use std::panic;
use std::sync::Arc;

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
            low: rgb(0x22c55e),   // Green for bass
            mid: rgb(0xeab308),   // Yellow for mids
            high: rgb(0xef4444),  // Red for highs
        }
    }
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
