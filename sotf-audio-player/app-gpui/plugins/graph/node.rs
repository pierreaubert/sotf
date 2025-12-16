//! Graph Node Component
//!
//! Renders a plugin as a node with input/output ports on a 2D canvas.

use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::PluginType;

use crate::theme::Theme;

/// Port type (input or output)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    Input,
    Output,
}

/// Port information for hit testing
#[derive(Debug, Clone)]
pub struct PortInfo {
    pub port_type: PortType,
    pub channel_index: usize,
    pub center: Point<Pixels>,
    pub radius: Pixels,
}

/// Node dimensions
pub const NODE_WIDTH: f32 = 120.0;
pub const NODE_HEIGHT: f32 = 80.0;
pub const PORT_RADIUS: f32 = 6.0;
pub const PORT_SPACING: f32 = 16.0;

/// A graph node representing a plugin
pub struct GraphNode {
    pub plugin_type: PluginType,
    pub plugin_name: String,
    pub enabled: bool,
    pub selected: bool,
    pub position: Point<Pixels>,
    pub input_channels: usize,
    pub output_channels: usize,
    pub color: Rgba,
    pub icon: &'static str,
}

impl GraphNode {
    pub fn new(
        plugin_type: PluginType,
        position: Point<Pixels>,
        input_channels: usize,
        output_channels: usize,
        theme: &Theme,
    ) -> Self {
        let color = plugin_color(&plugin_type, theme);
        let icon = plugin_icon(&plugin_type);
        Self {
            plugin_name: plugin_type.name().to_string(),
            plugin_type,
            enabled: true,
            selected: false,
            position,
            input_channels,
            output_channels,
            color,
            icon,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Get the bounds of this node
    pub fn bounds(&self) -> Bounds<Pixels> {
        Bounds {
            origin: self.position,
            size: size(px(NODE_WIDTH), px(NODE_HEIGHT)),
        }
    }

    /// Get input port position for a given channel
    pub fn input_port_position(&self, channel: usize) -> Point<Pixels> {
        let port_y = self.calculate_port_y(channel, self.input_channels);
        point(self.position.x, port_y)
    }

    /// Get output port position for a given channel
    pub fn output_port_position(&self, channel: usize) -> Point<Pixels> {
        let port_y = self.calculate_port_y(channel, self.output_channels);
        point(self.position.x + px(NODE_WIDTH), port_y)
    }

    fn calculate_port_y(&self, channel: usize, total_channels: usize) -> Pixels {
        if total_channels == 0 {
            return self.position.y + px(NODE_HEIGHT / 2.0);
        }
        let total_height = (total_channels - 1) as f32 * PORT_SPACING;
        let start_y = self.position.y + px((NODE_HEIGHT - total_height) / 2.0);
        start_y + px(channel as f32 * PORT_SPACING)
    }

    /// Get all port infos for hit testing
    pub fn port_infos(&self) -> Vec<PortInfo> {
        let mut ports = Vec::new();

        // Input ports
        for i in 0..self.input_channels {
            let center = self.input_port_position(i);
            ports.push(PortInfo {
                port_type: PortType::Input,
                channel_index: i,
                center,
                radius: px(PORT_RADIUS),
            });
        }

        // Output ports
        for i in 0..self.output_channels {
            let center = self.output_port_position(i);
            ports.push(PortInfo {
                port_type: PortType::Output,
                channel_index: i,
                center,
                radius: px(PORT_RADIUS),
            });
        }

        ports
    }

    /// Check if a point hits this node (excluding ports)
    pub fn hit_test(&self, point: Point<Pixels>) -> bool {
        self.bounds().contains(&point)
    }

    /// Check if a point hits a port, returns the port info if so
    pub fn hit_test_port(&self, point: Point<Pixels>) -> Option<PortInfo> {
        for port in self.port_infos() {
            let dx: f32 = (point.x - port.center.x).into();
            let dy: f32 = (point.y - port.center.y).into();
            let dist = (dx * dx + dy * dy).sqrt();
            let radius: f32 = port.radius.into();
            if dist <= radius + 4.0 {
                // 4px extra tolerance
                return Some(port);
            }
        }
        None
    }
}

impl IntoElement for GraphNode {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for GraphNode {
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
        // Node position is handled by parent div wrapper - we just need to specify size
        let layout_id = window.request_layout(
            Style {
                size: size(px(NODE_WIDTH).into(), px(NODE_HEIGHT).into()),
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
        let opacity = if self.enabled { 1.0 } else { 0.5 };

        // Node background
        let bg_color = Rgba {
            r: 0.118,
            g: 0.118,
            b: 0.145,
            a: opacity,
        };

        window.paint_quad(PaintQuad {
            bounds,
            corner_radii: Corners::all(px(8.0)),
            background: bg_color.into(),
            border_widths: Edges::all(px(if self.selected { 2.0 } else { 1.0 })),
            border_color: if self.selected {
                self.color.into()
            } else {
                Rgba {
                    r: 0.3,
                    g: 0.3,
                    b: 0.35,
                    a: opacity,
                }
                .into()
            },
            border_style: Default::default(),
        });

        // Top color bar
        let bar_bounds = Bounds {
            origin: bounds.origin,
            size: size(bounds.size.width, px(4.0)),
        };
        window.paint_quad(PaintQuad {
            bounds: bar_bounds,
            corner_radii: Corners {
                top_left: px(8.0),
                top_right: px(8.0),
                bottom_left: px(0.0),
                bottom_right: px(0.0),
            },
            background: Rgba {
                r: self.color.r,
                g: self.color.g,
                b: self.color.b,
                a: opacity,
            }
            .into(),
            border_widths: Edges::default(),
            border_color: Hsla::transparent_black(),
            border_style: Default::default(),
        });

        // Input ports (left side)
        for i in 0..self.input_channels {
            let port_center = self.input_port_position(i);
            self.paint_port(window, port_center, PortType::Input, opacity);
        }

        // Output ports (right side)
        for i in 0..self.output_channels {
            let port_center = self.output_port_position(i);
            self.paint_port(window, port_center, PortType::Output, opacity);
        }
    }
}

impl GraphNode {
    fn paint_port(&self, window: &mut Window, center: Point<Pixels>, port_type: PortType, opacity: f32) {
        let (fill_color, stroke_color) = match port_type {
            PortType::Input => (
                Rgba {
                    r: 0.2,
                    g: 0.6,
                    b: 0.9,
                    a: opacity,
                },
                Rgba {
                    r: 0.3,
                    g: 0.7,
                    b: 1.0,
                    a: opacity,
                },
            ),
            PortType::Output => (
                Rgba {
                    r: 0.9,
                    g: 0.5,
                    b: 0.2,
                    a: opacity,
                },
                Rgba {
                    r: 1.0,
                    g: 0.6,
                    b: 0.3,
                    a: opacity,
                },
            ),
        };

        let port_bounds = Bounds {
            origin: point(center.x - px(PORT_RADIUS), center.y - px(PORT_RADIUS)),
            size: size(px(PORT_RADIUS * 2.0), px(PORT_RADIUS * 2.0)),
        };

        window.paint_quad(PaintQuad {
            bounds: port_bounds,
            corner_radii: Corners::all(px(PORT_RADIUS)),
            background: fill_color.into(),
            border_widths: Edges::all(px(1.5)),
            border_color: stroke_color.into(),
            border_style: Default::default(),
        });
    }
}

// Plugin color scheme for different types - uses theme colors
fn plugin_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    match plugin_type {
        PluginType::EQ => theme.plugin_colors.eq,
        PluginType::Gain => theme.plugin_colors.gain,
        PluginType::Upmixer => theme.plugin_colors.upmixer,
        PluginType::Compressor => theme.plugin_colors.compressor,
        PluginType::Limiter => theme.plugin_colors.limiter,
        PluginType::Gate => theme.plugin_colors.gate,
        PluginType::LoudnessCompensation => theme.plugin_colors.loudness,
        PluginType::BinauralDecoder => theme.plugin_colors.binaural,
        PluginType::Convolution => theme.plugin_colors.convolution,
        PluginType::LoudnessMonitor => theme.plugin_colors.monitor,
        PluginType::SpectrumAnalyzer => theme.plugin_colors.spectrum,
        PluginType::ChannelMuteSolo => theme.plugin_colors.mute_solo,
    }
}

fn plugin_icon(plugin_type: &PluginType) -> &'static str {
    match plugin_type {
        PluginType::EQ => "~",
        PluginType::Gain => "^",
        PluginType::Upmixer => "*",
        PluginType::Compressor => "O",
        PluginType::Limiter => "#",
        PluginType::Gate => "[]",
        PluginType::LoudnessCompensation => "L",
        PluginType::BinauralDecoder => "@",
        PluginType::Convolution => "~",
        PluginType::LoudnessMonitor => "M",
        PluginType::SpectrumAnalyzer => "S",
        PluginType::ChannelMuteSolo => "X",
    }
}
