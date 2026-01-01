//! VolumeKnob - A circular volume knob with path-painted fill indicator
//!
//! A visual volume control with:
//! - Path-painted circular fill that rises from bottom
//! - Scroll wheel adjustment
//! - Double-click to toggle mute
//! - Keyboard support (requires focus - click to focus):
//!   - Arrow Up/Right: increase volume
//!   - Arrow Down/Left: decrease volume
//!   - M key: toggle mute
//! - Mute state support
//! - Customizable colors and theme support

use crate::ComponentTheme;
use crate::theme::ThemeExt;
use gpui::*;
use std::f32::consts::PI;

/// Theme colors for volume knob styling
#[derive(Debug, Clone, ComponentTheme)]
pub struct VolumeKnobTheme {
    /// Accent color (ring and fill when active)
    #[theme(default = 0x808080ff, from = accent)]
    pub accent: Rgba,
    /// Color when muted
    #[theme(default = 0x4d4d4dff, from = text_muted)]
    pub muted: Rgba,
    /// Background color
    #[theme(default = 0x1a1a1aff, from = surface)]
    pub background: Rgba,
    /// Text color for label
    #[theme(default = 0xe6e6e6ff, from = text_primary)]
    pub text: Rgba,
}

/// Custom element that paints the volume knob fill using paths
struct VolumeKnobFillElement {
    size: Pixels,
    value: f32,
    bg_color: Rgba,
    fill_color: Rgba,
    ring_color: Rgba,
}

impl VolumeKnobFillElement {
    fn new(size: Pixels, value: f32, bg_color: Rgba, fill_color: Rgba, ring_color: Rgba) -> Self {
        Self {
            size,
            value,
            bg_color,
            fill_color,
            ring_color,
        }
    }
}

impl IntoElement for VolumeKnobFillElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for VolumeKnobFillElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
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
                size: Size {
                    width: self.size.into(),
                    height: self.size.into(),
                },
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
        // Extract bounds into f32 for calculations
        let size_f32 = self.size.to_f64() as f32;
        let origin_x = bounds.origin.x;
        let origin_y = bounds.origin.y;
        let radius = size_f32 / 2.0;

        // Transparent color for borders we don't want to render
        let transparent = Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };

        // Draw background circle
        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(px(radius)),
            background: self.bg_color.into(),
            border_widths: Edges::default(),
            border_color: transparent.into(),
            border_style: BorderStyle::default(),
        });

        // Draw fill - a circular segment from bottom
        if self.value > 0.001 {
            // Calculate geometry in f32
            let center_x = radius;
            let center_y = radius;

            // Calculate the y-coordinate of the "water line" (relative to element origin)
            // At 0%: water_line_y = center_y + radius (bottom of circle)
            // At 100%: water_line_y = center_y - radius (top of circle)
            let water_line_y = center_y + radius - (self.value * 2.0 * radius);

            // Only draw if the water line is within the circle
            if water_line_y < center_y + radius {
                // Calculate intersection points of horizontal line with circle
                // Circle equation: (x - cx)^2 + (y - cy)^2 = r^2
                // At y = water_line_y: (x - cx)^2 = r^2 - (water_line_y - cy)^2
                let dy = water_line_y - center_y;
                let dx_squared = radius * radius - dy * dy;

                if dx_squared > 0.0 {
                    let dx = dx_squared.sqrt();
                    let left_x = center_x - dx;

                    // Build a path for the filled portion using PathBuilder
                    let mut builder = PathBuilder::fill();

                    // Start at left intersection point
                    builder.move_to(point(origin_x + px(left_x), origin_y + px(water_line_y)));

                    // Draw arc from left to right along the bottom of the circle
                    // We'll approximate with line segments for a smooth curve
                    let start_angle = (dy / radius).asin();
                    let end_angle = PI - start_angle;

                    // Number of segments for smooth arc
                    let segments = 32;
                    for i in 1..=segments {
                        let t = i as f32 / segments as f32;
                        let angle = start_angle + t * (end_angle - start_angle);
                        // Angle measured from right (0) going counter-clockwise
                        // We want bottom arc, so we go from left intersection to right
                        let arc_angle = PI - angle; // Convert to standard angle
                        let x = center_x + radius * arc_angle.cos();
                        let y = center_y + radius * arc_angle.sin();
                        builder.line_to(point(origin_x + px(x), origin_y + px(y)));
                    }

                    // Close the path back to start
                    builder.line_to(point(origin_x + px(left_x), origin_y + px(water_line_y)));

                    if let Ok(path) = builder.build() {
                        window.paint_path(path, self.fill_color);
                    }
                } else if self.value > 0.99 {
                    // Nearly full - draw full circle
                    let inset = px(1.0);
                    window.paint_quad(PaintQuad {
                        bounds: Bounds {
                            origin: point(bounds.origin.x + inset, bounds.origin.y + inset),
                            size: size(
                                bounds.size.width - inset * 2.0,
                                bounds.size.height - inset * 2.0,
                            ),
                        },
                        corner_radii: Corners::all(px(radius - 1.0)),
                        background: self.fill_color.into(),
                        border_widths: Edges::default(),
                        border_color: transparent.into(),
                        border_style: BorderStyle::default(),
                    });
                }
            }
        }

        // Draw border ring
        let ring_inset = px(3.0);
        let ring_bounds = Bounds {
            origin: point(bounds.origin.x + ring_inset, bounds.origin.y + ring_inset),
            size: size(
                bounds.size.width - ring_inset * 2.0,
                bounds.size.height - ring_inset * 2.0,
            ),
        };
        // Create ring color with 30% opacity
        let ring_with_opacity = Rgba {
            r: self.ring_color.r,
            g: self.ring_color.g,
            b: self.ring_color.b,
            a: self.ring_color.a * 0.3,
        };
        window.paint_quad(PaintQuad {
            bounds: ring_bounds,
            corner_radii: Corners::all(px(radius - 3.0)),
            background: transparent.into(),
            border_widths: Edges::all(px(2.0)),
            border_color: ring_with_opacity.into(),
            border_style: BorderStyle::default(),
        });
    }
}

/// A circular volume knob with fill indicator.
#[derive(IntoElement)]
pub struct VolumeKnob {
    id: ElementId,
    value: f32,
    label: SharedString,
    size: Pixels,
    muted: bool,
    /// Optional theme (uses global theme if not set)
    theme: Option<VolumeKnobTheme>,
    /// Override: accent color
    accent_color: Option<Rgba>,
    /// Override: muted color
    muted_color: Option<Rgba>,
    /// Override: background color
    bg_color: Option<Rgba>,
    /// Override: text color
    text_color: Option<Rgba>,
    on_change: Option<Box<dyn Fn(f32, &mut Window, &mut App) + 'static>>,
    on_mute_toggle: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

static VOLUME_KNOB_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl VolumeKnob {
    pub fn new() -> Self {
        let counter = VOLUME_KNOB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            id: ElementId::Name(format!("volume-knob-{}", counter).into()),
            value: 0.0,
            label: "".into(),
            size: px(40.0),
            muted: false,
            theme: None,
            accent_color: None,
            muted_color: None,
            bg_color: None,
            text_color: None,
            on_change: None,
            on_mute_toggle: None,
        }
    }

    /// Set the theme
    pub fn theme(mut self, theme: VolumeKnobTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Override accent color (ring and fill when active)
    pub fn accent_color(mut self, color: impl Into<Rgba>) -> Self {
        self.accent_color = Some(color.into());
        self
    }

    /// Override muted color
    pub fn muted_color(mut self, color: impl Into<Rgba>) -> Self {
        self.muted_color = Some(color.into());
        self
    }

    /// Override background color
    pub fn bg_color(mut self, color: impl Into<Rgba>) -> Self {
        self.bg_color = Some(color.into());
        self
    }

    /// Override text color
    pub fn text_color(mut self, color: impl Into<Rgba>) -> Self {
        self.text_color = Some(color.into());
        self
    }

    /// Set value change handler (called on scroll wheel)
    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set mute toggle handler (called on double-click)
    pub fn on_mute_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mute_toggle = Some(Box::new(handler));
        self
    }
}

impl Default for VolumeKnob {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for VolumeKnob {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Get theme: use explicit theme, or derive from global theme
        let global_theme = cx.theme();
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(|| VolumeKnobTheme::from(&global_theme));

        // Apply color overrides or use theme defaults
        let accent_color = self.accent_color.unwrap_or(theme.accent);
        let muted_color = self.muted_color.unwrap_or(theme.muted);
        let bg_color = self.bg_color.unwrap_or(theme.background);
        let text_color = self.text_color.unwrap_or(theme.text);

        let display_value = if self.muted {
            0.0
        } else {
            self.value.clamp(0.0, 1.0)
        };
        let ring_color = if self.muted {
            muted_color
        } else {
            accent_color
        };
        let text_color_final = if self.muted { muted_color } else { text_color };

        // Make fill color slightly lighter than the background
        let fill_color = if self.muted {
            muted_color
        } else {
            // Lighten the background color by converting to Hsla, increasing lightness,
            // then converting back to Rgba
            let mut lighter: Hsla = bg_color.into();
            lighter.l = (lighter.l + 0.15).min(1.0);
            lighter.into()
        };

        // Capture values for closures
        let current_value = self.value;
        let current_muted = self.muted;

        let mut container = div()
            .id(self.id)
            .relative()
            .w(self.size)
            .h(self.size)
            .cursor_pointer()
            .focusable();

        // Convert handlers to Rc for sharing between closures
        let on_change_rc = self.on_change.map(std::rc::Rc::new);
        let on_mute_rc = self.on_mute_toggle.map(std::rc::Rc::new);

        // Scroll wheel - adjust value
        if let Some(ref change_handler) = on_change_rc {
            let scroll_handler = change_handler.clone();
            container = container.on_scroll_wheel(move |event, window, cx| {
                let delta = event.delta.pixel_delta(px(20.0)).y;
                let scroll_up = delta < px(0.0);
                let step = 0.05;
                let change = if scroll_up { step } else { -step };
                let new_value = (current_value + change).clamp(0.0, 1.0);
                scroll_handler(new_value, window, cx);
            });
        }

        // Double-click - toggle mute
        if let Some(ref mute_handler) = on_mute_rc {
            let click_mute = mute_handler.clone();
            container = container.on_click(move |event, window, cx| {
                if event.click_count() == 2 {
                    click_mute(!current_muted, window, cx);
                }
            });
        }

        // Keyboard support
        if on_change_rc.is_some() || on_mute_rc.is_some() {
            let key_change = on_change_rc.clone();
            let key_mute = on_mute_rc.clone();

            container = container.on_key_down(move |event, window, cx| {
                match event.keystroke.key.as_str() {
                    "up" | "right" => {
                        if let Some(ref handler) = key_change {
                            let step = 0.05;
                            let new_value = (current_value + step).clamp(0.0, 1.0);
                            handler(new_value, window, cx);
                        }
                    }
                    "down" | "left" => {
                        if let Some(ref handler) = key_change {
                            let step = 0.05;
                            let new_value = (current_value - step).clamp(0.0, 1.0);
                            handler(new_value, window, cx);
                        }
                    }
                    "m" => {
                        if let Some(ref handler) = key_mute {
                            handler(!current_muted, window, cx);
                        }
                    }
                    _ => {}
                }
            });
        }

        container
            // Custom painted fill element
            .child(div().absolute().inset_0().child(VolumeKnobFillElement::new(
                self.size,
                display_value,
                bg_color,
                fill_color,
                ring_color,
            )))
            // Label text in center
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_color_final)
                    .child(self.label),
            )
    }
}
