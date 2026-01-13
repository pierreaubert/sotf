//! Spectrum Analyzer UI Components

use std::panic;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Select, SelectOption, SelectSize};
use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};

use super::common::{ParamSectionStyle, render_knob, render_section_header};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
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
        // These defaults match the dark theme spectrum colors.
        // When rendering, use Theme::spectrum_colors for theme-aware colors.
        Self {
            background: rgb(0x1a1a1a), // Dark background from theme.surface
            low: rgb(0x22c55e),        // Green for bass (theme.success)
            mid: rgb(0xeab308),        // Yellow for mids (theme.warning)
            high: rgb(0xef4444),       // Red for highs (theme.error)
        }
    }
}

impl From<&crate::theme::SpectrumColors> for SpectrumColors {
    fn from(theme_colors: &crate::theme::SpectrumColors) -> Self {
        Self {
            background: theme_colors.background,
            low: theme_colors.bass,
            mid: theme_colors.mids,
            high: theme_colors.treble,
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

        // Optimization: For high bin counts, drawing thousands of quads is slow.
        // Use a single Path to draw the spectrum shape.
        // If bin count is low (< 100), bars look nice. If high, a filled curve/histogram is better.
        // We switch to Path rendering always for consistent performance.

        // Build the path points
        let mut path = PathBuilder::fill();
        // Start at bottom left
        path.move_to(point(bounds.origin.x, bounds.origin.y + bounds.size.height));

        let total_width = bounds.size.width;
        let step_width = total_width / bar_count as f32;

        for (i, &mag) in self.magnitudes.iter().enumerate() {
            // Apply smoothing
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
            // Height ratio 0.0 means bar_height should be small? 
            // bounds.size.height is max height. 
            // y goes down (0 at top).
            // So if height_ratio is 1.0 (max), bar_height is bounds.height.
            // y = bounds.y + bounds.height - bar_height = bounds.y.
            // Correct.
            let bar_height = (bounds.size.height * height_ratio).max(px(0.0));
            
            let x = bounds.origin.x + step_width * i as f32;
            let y = bounds.origin.y + bounds.size.height - bar_height;

            // Add points for "stepped" look (histogram style)
            path.line_to(point(x, y)); // Move up/down to new height
            path.line_to(point(x + step_width, y)); // Move right across bar width
        }

        // Finish at bottom right
        path.line_to(point(bounds.origin.x + bounds.size.width, bounds.origin.y + bounds.size.height));
        // Close shape
        path.line_to(point(bounds.origin.x, bounds.origin.y + bounds.size.height));

        // Use a solid color for the filled path (e.g., Green/Low color)
        // Note: GPUI paint_path currently supports a single color.
        // For gradient support, we would need to use a mask or shader, which is more complex.
        // For now, this resolves the performance bottleneck.
        window.paint_path(
            path.build().unwrap(),
            self.colors.low,
        );
        
        // Note: 'paint_path' with filled shape works best.
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
// Axis Components
// ============================================================================

/// Format frequency value for axis label
fn format_freq_label(freq: f32) -> String {
    if freq >= 1000.0 {
        let khz = freq / 1000.0;
        if khz == khz.floor() {
            format!("{}k", khz as i32)
        } else {
            format!("{:.1}k", khz)
        }
    } else if freq == freq.floor() {
        format!("{}", freq as i32)
    } else {
        format!("{:.0}", freq)
    }
}

/// Calculate logarithmic position of a frequency within a range
fn freq_to_log_position(freq: f32, min_freq: f32, max_freq: f32) -> f32 {
    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let log_freq = freq.log10();
    (log_freq - log_min) / (log_max - log_min)
}

/// Generate frequency labels for the given range
fn generate_freq_labels(min_freq: f32, max_freq: f32) -> Vec<(String, f32)> {
    // Standard frequency points to consider
    let all_freqs: [f32; 15] = [
        20.0, 30.0, 50.0, 100.0, 200.0, 300.0, 500.0, 1000.0, 2000.0, 3000.0, 5000.0, 10000.0,
        15000.0, 20000.0, 24000.0,
    ];

    let mut labels = Vec::new();

    // Always include min and max
    labels.push((format_freq_label(min_freq), 0.0));

    // Add intermediate labels that fall within range
    for &freq in &all_freqs {
        if freq > min_freq * 1.1 && freq < max_freq * 0.9 {
            let pos = freq_to_log_position(freq, min_freq, max_freq);
            labels.push((format_freq_label(freq), pos));
        }
    }

    labels.push((format_freq_label(max_freq), 1.0));

    // Filter to avoid overlapping labels (keep at least 0.08 apart)
    let mut filtered = Vec::new();
    for (label, pos) in labels {
        if filtered.is_empty()
            || filtered
                .last()
                .map(|(_, last_pos): &(String, f32)| pos - last_pos > 0.08)
                .unwrap_or(true)
        {
            filtered.push((label, pos));
        }
    }

    filtered
}

/// Render horizontal frequency axis (logarithmic scale)
fn render_frequency_axis(min_freq: f32, max_freq: f32, theme: &Theme) -> impl IntoElement {
    let freq_labels = generate_freq_labels(min_freq, max_freq);

    div()
        .w_full()
        .h(px(20.0))
        .relative()
        .children(freq_labels.into_iter().map(|(label, pos)| {
            div()
                .absolute()
                .left(relative(pos))
                .top_0()
                .text_xs()
                .text_color(theme.text_muted)
                .child(
                    div()
                        .ml(px(-12.0)) // Center the label
                        .child(label),
                )
        }))
}

/// Render vertical dB axis (-60dB to 0dB)
fn render_db_axis(theme: &Theme) -> impl IntoElement {
    let db_labels = [("0", 0.0), ("-20", 0.333), ("-40", 0.666), ("-60", 1.0)];

    div()
        .w(px(32.0))
        .h_full()
        .flex()
        .flex_col()
        .relative()
        .children(db_labels.iter().map(|(label, pos)| {
            div()
                .absolute()
                .top(relative(*pos as f32))
                .right_0()
                .text_xs()
                .text_color(theme.text_muted)
                .pr_1()
                .child(
                    div()
                        .mt(px(-6.0)) // Center vertically
                        .child(*label),
                )
        }))
}

// ============================================================================
// Plugin UI
// ============================================================================

use sotf_plugins::SpectrumData;

/// State for rendering the Spectrum Analyzer plugin
pub struct SpectrumRenderState<'a> {
    pub num_bins: usize,
    pub min_freq: f32,
    pub max_freq: f32,
    pub smoothing: f32,
    pub tilt_correction: SpectralTiltCorrection,
    pub tilt_reference: TiltReferenceFreq,
    pub tilt_select_open: bool,
    pub reference_select_open: bool,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a SpectrumData>,
}

/// Render the Spectrum Analyzer plugin
pub fn render_spectrum_analyzer_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: SpectrumRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Spectrum display section with axes
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .param_section_style_lg(theme)
                .child(render_section_header("SPECTRUM ANALYZER", theme))
                // Main spectrum area with dB axis
                .child(
                    div()
                        .flex()
                        .gap_1()
                        // dB axis (vertical, left side)
                        .child(render_db_axis(theme))
                        // Spectrum bars
                        .child(
                            div()
                                .flex_1()
                                .h(px(200.0))
                                .bg(theme.surface)
                                .rounded_lg()
                                .border_1()
                                .border_color(theme.border)
                                .flex()
                                .items_end()
                                .gap_px()
                                .p_2()
                                .child(if let Some(data) = state.data {
                                    // Use real spectrum data
                                    let magnitudes: Arc<[f32]> = data.magnitudes.clone().into();
                                    SpectrumElement::new(magnitudes)
                                        .height(px(200.0))
                                        .frequency_range(state.min_freq, state.max_freq)
                                        .smoothing(state.smoothing)
                                        .colors(SpectrumColors::from(&theme.spectrum_colors))
                                        .into_any_element()
                                } else {
                                    // Fallback if no data available
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size_full()
                                        .text_color(theme.text_muted)
                                        .child("No signal")
                                        .into_any_element()
                                }),
                        ),
                )
                // Frequency axis (horizontal, below spectrum)
                .child(
                    div()
                        .flex()
                        .child(div().w(px(32.0))) // Spacer to align with dB axis
                        .child(render_frequency_axis(state.min_freq, state.max_freq, theme)),
                ),
        )
        // Controls
        .child(
            div()
                .flex()
                .gap_4()
                .justify_center()
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Bins",
                    state.num_bins as f64,
                    10.0,
                    100.0,
                    "",
                    0,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Min Hz",
                    state.min_freq as f64,
                    10.0,
                    1000.0,
                    "Hz",
                    1,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Max Hz",
                    state.max_freq as f64,
                    1000.0,
                    24000.0,
                    "Hz",
                    2,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Smooth",
                    state.smoothing as f64,
                    0.0,
                    1.0,
                    "",
                    3,
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                )),
        )
        // Tilt correction controls
        .child(
            div()
                .flex()
                .gap_4()
                .justify_center()
                .items_center()
                // Tilt correction selector
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child("Tilt"),
                        )
                        .child(
                            div().w(px(80.0)).child(
                                Select::new("tilt-correction-select")
                                    .options(vec![
                                        SelectOption::new("none".to_string(), "None"),
                                        SelectOption::new("pink".to_string(), "Pink (+3dB/oct)"),
                                    ])
                                    .selected(match state.tilt_correction {
                                        SpectralTiltCorrection::None => "none".to_string(),
                                        SpectralTiltCorrection::Pink => "pink".to_string(),
                                        SpectralTiltCorrection::Custom(_) => "none".to_string(),
                                    })
                                    .is_open(state.tilt_select_open)
                                    .size(SelectSize::Sm)
                                    .theme(theme.to_select_theme())
                                    .on_toggle({
                                        let entity = entity.clone();
                                        move |is_open, _window, cx| {
                                            entity.update(cx, |state, _| {
                                                state.app.spectrum_tilt_select_open = is_open;
                                            });
                                        }
                                    })
                                    .on_change({
                                        let entity = entity.clone();
                                        move |value, _, cx| {
                                            entity.update(cx, |state, _cx| {
                                                let tilt = match value.as_ref() {
                                                    "pink" => SpectralTiltCorrection::Pink,
                                                    _ => SpectralTiltCorrection::None,
                                                };
                                                state
                                                    .app
                                                    .set_spectrum_tilt_correction(plugin_idx, tilt);
                                            });
                                        }
                                    }),
                            ),
                        ),
                )
                // Reference frequency selector
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child("Reference"),
                        )
                        .child(
                            div().w(px(90.0)).child(
                                Select::new("tilt-reference-select")
                                    .options(vec![
                                        SelectOption::new("standard".to_string(), "1kHz"),
                                        SelectOption::new("minfreq".to_string(), "Min Freq"),
                                    ])
                                    .selected(match state.tilt_reference {
                                        TiltReferenceFreq::Standard => "standard".to_string(),
                                        TiltReferenceFreq::MinFreq => "minfreq".to_string(),
                                    })
                                    .is_open(state.reference_select_open)
                                    .size(SelectSize::Sm)
                                    .theme(theme.to_select_theme())
                                    .on_toggle({
                                        let entity = entity.clone();
                                        move |is_open, _window, cx| {
                                            entity.update(cx, |state, _| {
                                                state.app.spectrum_reference_select_open = is_open;
                                            });
                                        }
                                    })
                                    .on_change({
                                        let entity = entity.clone();
                                        move |value, _, cx| {
                                            entity.update(cx, |state, _cx| {
                                                let reference = match value.as_ref() {
                                                    "minfreq" => TiltReferenceFreq::MinFreq,
                                                    _ => TiltReferenceFreq::Standard,
                                                };
                                                state.app.set_spectrum_tilt_reference(
                                                    plugin_idx, reference,
                                                );
                                            });
                                        }
                                    }),
                            ),
                        ),
                ),
        )
        // .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}

impl PlayerView {
    /// Render the full-screen spectrum analyzer display
    /// Uses GPU-accelerated SpectrumElement for high-performance rendering
    pub(crate) fn render_spectrum_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        let content = if let Some(info) = &state.app.playback.spectrum_info {
            // Convert magnitudes to Arc for the GPU element
            let magnitudes: Arc<[f32]> = info.magnitudes.clone().into();

            div()
                .flex()
                .flex_col()
                .size_full()
                // Main spectrum area with axes
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .gap_1()
                        // dB axis (vertical, left side)
                        .child(render_db_axis(&theme))
                        // GPU-accelerated spectrum visualization
                        .child(
                            div().flex_1().child(
                                SpectrumElement::new(magnitudes)
                                    .height(px(256.0))
                                    .frequency_range(20.0, 20000.0)
                                    .smoothing(0.3)
                                    .colors(SpectrumColors::from(&theme.spectrum_colors)),
                            ),
                        ),
                )
                // Frequency axis (horizontal, below spectrum)
                .child(
                    div()
                        .flex()
                        .child(div().w(px(32.0))) // Spacer to align with dB axis
                        .child(render_frequency_axis(20.0, 20000.0, &theme)),
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
