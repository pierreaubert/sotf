//! Interactive chart support with brush, zoom, and mouse event handling.
//!
//! This module provides interactive capabilities for gpui-px charts,
//! integrating d3rs brush and zoom functionality with GPUI mouse events.
//!
//! # Features
//!
//! - **Brush Selection**: Click and drag to select rectangular regions
//! - **Zoom to Selection**: Zoom into brushed regions
//! - **Zoom History**: Navigate back through zoom levels
//! - **Double-click Reset**: Reset to original view
//! - **Hover Events**: Track mouse position for tooltips
//!
//! # Example
//!
//! ```rust,no_run
//! use gpui_px::interaction::{ChartInteraction, InteractionMode};
//!
//! // Create interaction state for a chart
//! let mut interaction = ChartInteraction::new(20.0, 20000.0, -40.0, 10.0)
//!     .with_log_x(true)
//!     .with_size(500.0, 300.0)
//!     .with_mode(InteractionMode::Brush);
//!
//! // Handle mouse events
//! interaction.start_brush(100.0, 50.0);
//! interaction.update_brush(400.0, 200.0);
//!
//! // End brush and apply zoom
//! if let Some(selection) = interaction.end_brush(true) {
//!     println!("Zoomed to: {:?}", selection);
//! }
//!
//! // Double-click to reset
//! interaction.reset_zoom();
//! ```

use d3rs::brush::{BrushConfig, BrushSelection, BrushState, DomainSelection};
use d3rs::scale::{LinearScale, LogScale, Scale};
use d3rs::zoom::{ZoomConfig, ZoomState};
use std::sync::Arc;

// Re-export d3rs types for convenience
pub use d3rs::brush::{
    BrushConfig as BrushConfigD3, BrushSelection as BrushSelectionD3,
    DomainSelection as DomainSelectionD3,
};
pub use d3rs::interpolate::zoom::{interpolate_zoom, zoom_duration, ZoomParams, ZoomView};
pub use d3rs::zoom::{ZoomConfig as ZoomConfigD3, ZoomState as ZoomStateD3};

/// Chart interaction mode
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum InteractionMode {
    /// No interaction enabled
    #[default]
    None,
    /// Brush selection mode (click and drag to select)
    Brush,
    /// Pan mode (click and drag to pan)
    Pan,
    /// Zoom mode (scroll wheel to zoom)
    Zoom,
}

/// Callback type for brush end events
pub type BrushEndCallback = Arc<dyn Fn(DomainSelection) + Send + Sync>;

/// Callback type for zoom change events
pub type ZoomChangeCallback = Arc<dyn Fn(&ZoomState) + Send + Sync>;

/// Callback type for hover events (position in domain coordinates)
pub type HoverCallback = Arc<dyn Fn(Option<(f64, f64)>) + Send + Sync>;

/// Callback type for click events (position in domain coordinates)
pub type ClickCallback = Arc<dyn Fn(f64, f64) + Send + Sync>;

/// Chart interaction state that can be shared between components.
///
/// This struct maintains the state of brush selection and zoom levels,
/// allowing multiple components to react to chart interactions.
#[derive(Clone)]
pub struct ChartInteraction {
    /// Current brush state
    pub brush: BrushState,
    /// Current zoom state
    pub zoom: ZoomState,
    /// Brush configuration
    pub brush_config: BrushConfig,
    /// Zoom configuration
    pub zoom_config: ZoomConfig,
    /// Current interaction mode
    pub mode: InteractionMode,
    /// Whether X-axis uses log scale
    pub x_is_log: bool,
    /// Whether Y-axis uses log scale
    pub y_is_log: bool,
    /// Plot dimensions (width, height)
    pub plot_size: (f32, f32),
}

impl Default for ChartInteraction {
    fn default() -> Self {
        Self {
            brush: BrushState::new(),
            zoom: ZoomState::default(),
            brush_config: BrushConfig::default(),
            zoom_config: ZoomConfig::default(),
            mode: InteractionMode::None,
            x_is_log: false,
            y_is_log: false,
            plot_size: (600.0, 400.0),
        }
    }
}

impl ChartInteraction {
    /// Create a new chart interaction state with specified domain bounds.
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self {
            brush: BrushState::new(),
            zoom: ZoomState::new(x_min, x_max, y_min, y_max),
            brush_config: BrushConfig::default(),
            zoom_config: ZoomConfig::default(),
            mode: InteractionMode::Brush,
            x_is_log: false,
            y_is_log: false,
            plot_size: (600.0, 400.0),
        }
    }

    /// Set X-axis to logarithmic scale.
    pub fn with_log_x(mut self, is_log: bool) -> Self {
        self.x_is_log = is_log;
        self.zoom = self.zoom.with_log_x(is_log);
        self
    }

    /// Set Y-axis to logarithmic scale.
    pub fn with_log_y(mut self, is_log: bool) -> Self {
        self.y_is_log = is_log;
        self.zoom = self.zoom.with_log_y(is_log);
        self
    }

    /// Set the plot dimensions.
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.plot_size = (width, height);
        self
    }

    /// Set the interaction mode.
    pub fn with_mode(mut self, mode: InteractionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set brush configuration.
    pub fn with_brush_config(mut self, config: BrushConfig) -> Self {
        self.brush_config = config;
        self
    }

    /// Set zoom configuration.
    pub fn with_zoom_config(mut self, config: ZoomConfig) -> Self {
        self.zoom_config = config;
        self
    }

    /// Start a brush selection at the given pixel coordinates.
    pub fn start_brush(&mut self, x: f32, y: f32) {
        self.brush.start(x as f64, y as f64);
    }

    /// Update the brush selection while dragging.
    pub fn update_brush(&mut self, x: f32, y: f32) {
        self.brush.update(x as f64, y as f64);
    }

    /// End the brush selection and optionally apply zoom.
    ///
    /// Returns the domain selection if the brush was valid.
    pub fn end_brush(&mut self, apply_zoom: bool) -> Option<DomainSelection> {
        let pixel_selection = self.brush.end()?;

        // Check if selection is too small
        if pixel_selection.is_trivial(self.brush_config.min_size) {
            return None;
        }

        // Convert to domain coordinates
        let domain = self.pixel_to_domain(&pixel_selection);

        // Apply zoom if requested
        if apply_zoom {
            self.zoom.zoom_to(domain.x0, domain.x1, domain.y0, domain.y1);
        }

        Some(domain)
    }

    /// Cancel the current brush selection.
    pub fn cancel_brush(&mut self) {
        self.brush.reset();
    }

    /// Get the current brush selection rectangle (if active).
    pub fn current_brush_selection(&self) -> Option<BrushSelection> {
        self.brush.current_selection()
    }

    /// Check if a brush selection is currently active.
    pub fn is_brushing(&self) -> bool {
        self.brush.is_active()
    }

    /// Zoom to a specific domain region.
    pub fn zoom_to(&mut self, x_min: f64, x_max: f64, y_min: f64, y_max: f64) {
        self.zoom.zoom_to(x_min, x_max, y_min, y_max);
    }

    /// Reset zoom to original view.
    pub fn reset_zoom(&mut self) {
        self.zoom.reset();
    }

    /// Go back one zoom level.
    pub fn zoom_back(&mut self) -> bool {
        self.zoom.zoom_back()
    }

    /// Check if currently zoomed.
    pub fn is_zoomed(&self) -> bool {
        self.zoom.is_zoomed()
    }

    /// Get current X domain.
    pub fn x_domain(&self) -> (f64, f64) {
        self.zoom.x_domain()
    }

    /// Get current Y domain.
    pub fn y_domain(&self) -> (f64, f64) {
        self.zoom.y_domain()
    }

    /// Get the current zoom level (number of zoom operations).
    pub fn zoom_level(&self) -> usize {
        self.zoom.zoom_level()
    }

    /// Convert pixel coordinates to domain coordinates.
    pub fn pixel_to_domain(&self, selection: &BrushSelection) -> DomainSelection {
        let (width, height) = self.plot_size;
        let (x_min, x_max) = self.zoom.x_domain();
        let (y_min, y_max) = self.zoom.y_domain();

        if self.x_is_log {
            let x_scale = LogScale::new()
                .domain(x_min.max(1e-10), x_max)
                .range(0.0, width as f64);
            if self.y_is_log {
                let y_scale = LogScale::new()
                    .domain(y_min.max(1e-10), y_max)
                    .range(height as f64, 0.0);
                selection.to_domain(&x_scale, &y_scale)
            } else {
                let y_scale = LinearScale::new()
                    .domain(y_min, y_max)
                    .range(height as f64, 0.0);
                selection.to_domain(&x_scale, &y_scale)
            }
        } else {
            let x_scale = LinearScale::new()
                .domain(x_min, x_max)
                .range(0.0, width as f64);
            if self.y_is_log {
                let y_scale = LogScale::new()
                    .domain(y_min.max(1e-10), y_max)
                    .range(height as f64, 0.0);
                selection.to_domain(&x_scale, &y_scale)
            } else {
                let y_scale = LinearScale::new()
                    .domain(y_min, y_max)
                    .range(height as f64, 0.0);
                selection.to_domain(&x_scale, &y_scale)
            }
        }
    }

    /// Convert a single pixel point to domain coordinates.
    pub fn point_to_domain(&self, x: f32, y: f32) -> (f64, f64) {
        let (width, height) = self.plot_size;
        let (x_min, x_max) = self.zoom.x_domain();
        let (y_min, y_max) = self.zoom.y_domain();

        let domain_x = if self.x_is_log {
            let x_scale = LogScale::new()
                .domain(x_min.max(1e-10), x_max)
                .range(0.0, width as f64);
            x_scale.invert(x as f64).unwrap_or(x_min)
        } else {
            let x_scale = LinearScale::new()
                .domain(x_min, x_max)
                .range(0.0, width as f64);
            x_scale.invert(x as f64).unwrap_or(x_min)
        };

        let domain_y = if self.y_is_log {
            let y_scale = LogScale::new()
                .domain(y_min.max(1e-10), y_max)
                .range(height as f64, 0.0);
            y_scale.invert(y as f64).unwrap_or(y_min)
        } else {
            let y_scale = LinearScale::new()
                .domain(y_min, y_max)
                .range(height as f64, 0.0);
            y_scale.invert(y as f64).unwrap_or(y_min)
        };

        (domain_x, domain_y)
    }
}

/// Mouse event state for tracking interactions.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    /// Current mouse position in pixels (relative to plot area)
    pub position: Option<(f32, f32)>,
    /// Whether the left mouse button is pressed
    pub left_down: bool,
    /// Whether the right mouse button is pressed
    pub right_down: bool,
    /// Last click timestamp for double-click detection
    pub last_click_time: Option<std::time::Instant>,
    /// Last click position for double-click detection
    pub last_click_pos: Option<(f32, f32)>,
}

impl MouseState {
    /// Check if this is a double-click event.
    ///
    /// Returns true if the click occurred within 300ms and 5 pixels of the last click.
    pub fn is_double_click(&self, x: f32, y: f32) -> bool {
        if let (Some(last_time), Some((last_x, last_y))) =
            (self.last_click_time, self.last_click_pos)
        {
            let elapsed = last_time.elapsed();
            let distance = ((x - last_x).powi(2) + (y - last_y).powi(2)).sqrt();
            elapsed.as_millis() < 300 && distance < 5.0
        } else {
            false
        }
    }

    /// Record a click for double-click detection.
    pub fn record_click(&mut self, x: f32, y: f32) {
        self.last_click_time = Some(std::time::Instant::now());
        self.last_click_pos = Some((x, y));
    }
}

/// Configuration for chart mouse wheel behavior.
#[derive(Debug, Clone, Copy)]
pub struct WheelConfig {
    /// Zoom factor per scroll step (default: 1.1)
    pub zoom_factor: f64,
    /// Enable horizontal scroll for X-axis panning
    pub horizontal_pan: bool,
    /// Invert scroll direction
    pub invert: bool,
}

impl Default for WheelConfig {
    fn default() -> Self {
        Self {
            zoom_factor: 1.1,
            horizontal_pan: true,
            invert: false,
        }
    }
}

/// Apply mouse wheel zoom to chart interaction state.
///
/// # Arguments
/// * `interaction` - The chart interaction state to modify
/// * `delta_y` - Vertical scroll delta (positive = zoom out, negative = zoom in)
/// * `mouse_x` - Mouse X position in pixels (for zoom center)
/// * `mouse_y` - Mouse Y position in pixels (for zoom center)
/// * `config` - Wheel configuration
pub fn apply_wheel_zoom(
    interaction: &mut ChartInteraction,
    delta_y: f32,
    mouse_x: f32,
    mouse_y: f32,
    config: &WheelConfig,
) {
    let (x_min, x_max) = interaction.x_domain();
    let (y_min, y_max) = interaction.y_domain();

    // Get mouse position in domain coordinates
    let (focus_x, focus_y) = interaction.point_to_domain(mouse_x, mouse_y);

    // Calculate zoom factor
    let delta = if config.invert { -delta_y } else { delta_y };
    let factor = if delta > 0.0 {
        config.zoom_factor
    } else {
        1.0 / config.zoom_factor
    };

    // Apply zoom centered on mouse position
    let new_x_min = focus_x - (focus_x - x_min) * factor;
    let new_x_max = focus_x + (x_max - focus_x) * factor;
    let new_y_min = focus_y - (focus_y - y_min) * factor;
    let new_y_max = focus_y + (y_max - focus_y) * factor;

    interaction.zoom_to(new_x_min, new_x_max, new_y_min, new_y_max);
}

// ============================================================================
// GPUI-specific rendering functions (only available with gpui feature)
// ============================================================================

#[cfg(feature = "gpui")]
mod gpui_render {
    use super::*;
    use d3rs::zoom::ZoomState;
    use gpui::prelude::*;
    use gpui::*;

    /// Render a brush selection overlay.
    ///
    /// This renders a semi-transparent rectangle showing the current brush selection.
    pub fn render_brush_overlay(selection: &BrushSelection, config: &BrushConfig) -> impl IntoElement {
        let x = selection.x0 as f32;
        let y = selection.y0 as f32;
        let width = selection.width() as f32;
        let height = selection.height() as f32;

        let (_r, _g, _b, a) = config.fill_color;

        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(width))
            .h(px(height))
            .bg(hsla(210.0 / 360.0, 0.5, 0.6, a as f32 / 255.0))
            .border_1()
            .border_color(hsla(210.0 / 360.0, 0.5, 0.4, 1.0))
    }

    /// Render a zoom indicator showing the current zoom level.
    pub fn render_zoom_indicator(zoom: &ZoomState, x: f32, y: f32) -> impl IntoElement {
        let level = zoom.zoom_level();
        if level == 0 {
            return div().into_any_element();
        }

        let text = format!("Zoom: {}x", level);

        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .px_2()
            .py_1()
            .bg(hsla(0.0, 0.0, 0.2, 0.8))
            .rounded_md()
            .child(text)
            .text_color(hsla(0.0, 0.0, 1.0, 1.0))
            .text_xs()
            .into_any_element()
    }

    /// Render a reset button for zoom.
    pub fn render_reset_button<F>(x: f32, y: f32, on_click: F) -> impl IntoElement
    where
        F: Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
        div()
            .id("reset-button")
            .absolute()
            .left(px(x))
            .top(px(y))
            .px_2()
            .py_1()
            .bg(hsla(0.0, 0.0, 0.3, 0.9))
            .rounded_md()
            .child("Reset")
            .text_color(hsla(0.0, 0.0, 1.0, 1.0))
            .text_xs()
            .cursor_pointer()
            .hover(|s| s.bg(hsla(0.0, 0.0, 0.4, 0.9)))
            .on_click(on_click)
    }

    /// Render crosshairs at the mouse position.
    pub fn render_crosshairs(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .size_full()
            .child(
                // Vertical line
                div()
                    .absolute()
                    .left(px(x))
                    .top_0()
                    .w_px()
                    .h(px(height))
                    .bg(hsla(0.0, 0.0, 0.5, 0.5)),
            )
            .child(
                // Horizontal line
                div()
                    .absolute()
                    .left_0()
                    .top(px(y))
                    .w(px(width))
                    .h_px()
                    .bg(hsla(0.0, 0.0, 0.5, 0.5)),
            )
    }
}

#[cfg(feature = "gpui")]
pub use gpui_render::{render_brush_overlay, render_crosshairs, render_reset_button, render_zoom_indicator};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_interaction_creation() {
        let interaction = ChartInteraction::new(0.0, 100.0, -10.0, 10.0);
        assert_eq!(interaction.x_domain(), (0.0, 100.0));
        assert_eq!(interaction.y_domain(), (-10.0, 10.0));
        assert!(!interaction.is_zoomed());
    }

    #[test]
    fn test_brush_lifecycle() {
        let mut interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0).with_size(500.0, 500.0);

        assert!(!interaction.is_brushing());

        interaction.start_brush(100.0, 100.0);
        assert!(interaction.is_brushing());

        interaction.update_brush(300.0, 300.0);
        let selection = interaction.current_brush_selection().unwrap();
        assert_eq!(selection.width(), 200.0);
        assert_eq!(selection.height(), 200.0);

        let domain = interaction.end_brush(false).unwrap();
        assert!(!interaction.is_brushing());
        // Domain values depend on scale conversion
        assert!(domain.x1 > domain.x0);
        assert!(domain.y1 > domain.y0);
    }

    #[test]
    fn test_brush_with_zoom() {
        let mut interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0).with_size(500.0, 500.0);

        // Brush the center 50% of the chart
        interaction.start_brush(125.0, 125.0);
        interaction.update_brush(375.0, 375.0);
        interaction.end_brush(true);

        // Should now be zoomed
        assert!(interaction.is_zoomed());
        let (x_min, x_max) = interaction.x_domain();
        assert!(x_min > 0.0 && x_max < 100.0);
    }

    #[test]
    fn test_zoom_reset() {
        let mut interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0);

        interaction.zoom_to(25.0, 75.0, 25.0, 75.0);
        assert!(interaction.is_zoomed());

        interaction.reset_zoom();
        assert!(!interaction.is_zoomed());
        assert_eq!(interaction.x_domain(), (0.0, 100.0));
    }

    #[test]
    fn test_zoom_back() {
        let mut interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0);

        interaction.zoom_to(25.0, 75.0, 25.0, 75.0);
        interaction.zoom_to(35.0, 65.0, 35.0, 65.0);
        assert_eq!(interaction.zoom_level(), 2);

        interaction.zoom_back();
        assert_eq!(interaction.zoom_level(), 1);
        assert_eq!(interaction.x_domain(), (25.0, 75.0));

        interaction.zoom_back();
        assert!(!interaction.is_zoomed());
    }

    #[test]
    fn test_log_scale_interaction() {
        let interaction = ChartInteraction::new(20.0, 20000.0, -40.0, 10.0)
            .with_log_x(true)
            .with_size(500.0, 200.0);

        // Get domain point at center of chart
        let (x, _y) = interaction.point_to_domain(250.0, 100.0);

        // For log scale, center should be geometric mean: sqrt(20 * 20000) ≈ 632
        assert!((x - 632.0).abs() < 50.0);
    }

    #[test]
    fn test_double_click_detection() {
        let mut state = MouseState::default();

        // First click - not a double click
        assert!(!state.is_double_click(100.0, 100.0));
        state.record_click(100.0, 100.0);

        // Immediate second click - should be double click
        assert!(state.is_double_click(101.0, 101.0));

        // Far away click - not a double click
        assert!(!state.is_double_click(200.0, 200.0));
    }

    #[test]
    fn test_wheel_zoom() {
        let mut interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0).with_size(500.0, 500.0);
        let config = WheelConfig::default();

        let original_x = interaction.x_domain();

        // Zoom in (negative delta)
        apply_wheel_zoom(&mut interaction, -1.0, 250.0, 250.0, &config);

        // Should be zoomed in (smaller domain range)
        let new_x = interaction.x_domain();
        assert!(new_x.1 - new_x.0 < original_x.1 - original_x.0);
    }

    #[test]
    fn test_interaction_mode() {
        let interaction = ChartInteraction::default();
        assert_eq!(interaction.mode, InteractionMode::None);

        let interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0);
        assert_eq!(interaction.mode, InteractionMode::Brush);

        let interaction = interaction.with_mode(InteractionMode::Zoom);
        assert_eq!(interaction.mode, InteractionMode::Zoom);
    }

    #[test]
    fn test_brush_config() {
        let config = BrushConfig {
            fill_color: (255, 0, 0, 128),
            stroke_color: (255, 0, 0),
            stroke_width: 2.0,
            min_size: 10.0,
        };

        let interaction = ChartInteraction::default().with_brush_config(config.clone());
        assert_eq!(interaction.brush_config.min_size, 10.0);
    }

    #[test]
    fn test_trivial_brush_rejected() {
        let mut interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0).with_size(500.0, 500.0);

        // Very small brush (less than min_size of 5.0)
        interaction.start_brush(100.0, 100.0);
        interaction.update_brush(102.0, 102.0);
        let result = interaction.end_brush(false);

        // Should return None because selection is too small
        assert!(result.is_none());
    }

    #[test]
    fn test_cancel_brush() {
        let mut interaction = ChartInteraction::new(0.0, 100.0, 0.0, 100.0);

        interaction.start_brush(100.0, 100.0);
        assert!(interaction.is_brushing());

        interaction.cancel_brush();
        assert!(!interaction.is_brushing());
        assert!(interaction.current_brush_selection().is_none());
    }
}
