//! Plugin Graph Screen
//!
//! Full-screen workflow canvas for node-based plugin editing.
//! Uses the WorkflowCanvas from gpui-ui-kit for pan/zoom, connections, and hit testing.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::MenuItem;
use gpui_ui_kit::workflow::{
    NodeId, Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData, WorkflowTheme,
};
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{PluginGraph, PluginSettings, PluginType, SpecialNodeType};

use crate::app::types::Screen;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::theme::Theme;
use crate::ui::PlayerView;

// ============================================================================
// Node Type Constants
// ============================================================================

/// Node types for user_data
const NODE_TYPE_PLAYER: &str = "player";
const NODE_TYPE_INPUT_DEVICE: &str = "input_device";
const NODE_TYPE_OUTPUT_DEVICE: &str = "output_device";
const NODE_TYPE_PLUGIN: &str = "plugin";

// ============================================================================
// Drag and Drop Types
// ============================================================================

/// Drag data for palette items
#[derive(Clone)]
pub struct PaletteDragData {
    pub item_type: PaletteItemType,
    pub label: String,
    pub color: Rgba,
    pub text_on_accent: Rgba,
}

/// Type of item being dragged from palette
#[derive(Clone)]
pub enum PaletteItemType {
    Player,
    Plugin(PluginType),
}

impl Render for PaletteDragData {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        div()
            .px(d.pad_x)
            .py(d.pad_y)
            .bg(self.color)
            .rounded(d.r_md)
            .text_size(d.text_sm)
            .text_color(self.text_on_accent)
            .shadow_lg()
            .child(self.label.clone())
    }
}

impl PlayerView {
    /// Ensure the WorkflowCanvas entity exists, creating it if needed
    pub(crate) fn ensure_workflow_canvas(&self, cx: &mut Context<Self>) {
        let has_canvas = self
            .state
            .read(cx)
            .app
            .plugin_state
            .workflow_canvas
            .is_some();

        if !has_canvas {
            // Build workflow graph from plugin graph, or create a default graph
            let (
                plugin_graph,
                output_device_name,
                output_channels,
                theme,
                input_devices,
                output_devices,
            ) = {
                let state = self.state.read(cx);
                let output_device_name = state
                    .app
                    .audio_device_state
                    .output_devices
                    .get(state.app.audio_device_state.selected_output_device_index)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "Default Output".to_string());
                let output_channels = state
                    .app
                    .audio_device_state
                    .output_devices
                    .get(state.app.audio_device_state.selected_output_device_index)
                    .and_then(|d| d.default_config.as_ref())
                    .map(|c| c.channels as usize)
                    .unwrap_or(2);
                (
                    Some(state.app.plugin_state.graph.clone()),
                    output_device_name,
                    output_channels,
                    state.app.ui_state.theme.clone(),
                    state.app.audio_device_state.input_devices.clone(),
                    state.app.audio_device_state.output_devices.clone(),
                )
            };

            let workflow_graph =
                build_workflow_graph(&plugin_graph, &output_device_name, output_channels);

            // Create the WorkflowCanvas entity
            let canvas = cx.new(|cx| WorkflowCanvas::with_graph(workflow_graph, cx));

            // Set theme and menu items
            let workflow_theme = create_workflow_theme(&theme);
            let menu_items = build_menu_items(&input_devices, &output_devices);

            // Clone state for the callback
            let state_for_dblclick = self.state.clone();
            let canvas_for_dblclick = canvas.clone();
            let state_for_change = self.state.clone();
            let canvas_for_change = canvas.clone();
            let state_for_menu = self.state.clone();
            let canvas_for_menu = canvas.clone();

            canvas.update(cx, |canvas, _cx| {
                canvas.set_theme(workflow_theme);
                canvas.set_menu_items(menu_items);

                // Set double-click callback to open node editor modal.
                // We defer the body because this callback runs while the
                // WorkflowCanvas entity is mutably borrowed by
                // handle_double_click.  Reading the canvas inside the
                // callback would trigger a "cannot read while being
                // updated" panic.  cx.defer() schedules the work after
                // the current entity update completes.
                canvas.set_on_node_double_click(move |node_id, _window, cx| {
                    let canvas = canvas_for_dblclick.clone();
                    let state = state_for_dblclick.clone();
                    cx.defer(move |cx| {
                        let graph_node_uuid = canvas
                            .read(cx)
                            .graph()
                            .nodes
                            .get(&node_id)
                            .and_then(|n| n.user_data.get("plugin_node_id"))
                            .and_then(|v| v.as_str())
                            .and_then(|s| sotf_audio_player::GraphNodeId::parse_str(s).ok());

                        state.update(cx, |state, _cx| {
                            state.app.plugin_state.editing_plugin_node = Some(node_id);
                            state.app.plugin_state.editing_graph_node_uuid = graph_node_uuid;
                            state.app.ui_state.input_mode =
                                crate::app::InputMode::EditingPluginNode;
                        });
                    });
                });

                // Reconcile PluginGraph (data model + engine source of truth)
                // with the WorkflowCanvas after every structural mutation.
                // Same defer pattern as the double-click callback: the
                // canvas is mutably borrowed when the observer fires.
                canvas.set_on_graph_change(move |cx| {
                    let canvas = canvas_for_change.clone();
                    let state = state_for_change.clone();
                    cx.defer(move |cx| {
                        // Snapshot the canvas graph (Clone) so we can
                        // mutate state without holding a canvas read.
                        let workflow_graph = canvas.read(cx).graph().clone();
                        state.update(cx, |state, _cx| {
                            reconcile_plugin_graph_with_canvas(state, &workflow_graph);
                            state.app.plugin_state.pending_plugin_update =
                                Some(crate::app::types::PluginUpdateType::Structural);
                            state.app.plugin_state.plugin_graph_modified = true;
                        });
                    });
                });

                // Per-node right-click context menu: mirror the rack's
                // per-plugin actions (Edit, Solo, Bypass, Remove). The
                // canvas only fires this for plugin nodes that registered
                // a `plugin_node_id` in user_data; non-plugin nodes
                // (Player / Input / Output) fall through to the default
                // canvas menu so the user can still add/replace devices.
                canvas.set_node_menu_items(|node_data| {
                    let is_plugin = node_data
                        .user_data
                        .get("node_type")
                        .and_then(|v| v.as_str())
                        == Some(NODE_TYPE_PLUGIN);
                    if is_plugin {
                        Some(plugin_node_menu_items())
                    } else {
                        None
                    }
                });
                canvas.set_on_node_menu_select(move |menu_id, node_id, _window, cx| {
                    let canvas = canvas_for_menu.clone();
                    let state = state_for_menu.clone();
                    let menu_id = menu_id.clone();
                    cx.defer(move |cx| {
                        dispatch_plugin_node_action(&canvas, &state, &menu_id, node_id, cx);
                    });
                });
            });

            // Store the canvas entity
            self.state.update(cx, |state, _cx| {
                state.app.plugin_state.workflow_canvas = Some(canvas);
            });
        }
    }

    /// Render the plugin graph screen with workflow canvas
    pub(crate) fn render_plugin_graph_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure the canvas entity exists
        self.ensure_workflow_canvas(cx);

        let (theme, workflow_canvas, node_count, connection_count, plugin_count) = {
            let state = self.state.read(cx);
            let (nc, cc) = state
                .app
                .plugin_state
                .workflow_canvas
                .as_ref()
                .map(|canvas| {
                    let stats = canvas.read(cx).stats();
                    (stats.0, stats.1)
                })
                .unwrap_or((0, 0));
            let pc = state.app.plugin_state.graph.len();
            (
                state.app.ui_state.theme.clone(),
                state.app.plugin_state.workflow_canvas.clone(),
                nc,
                cc,
                pc,
            )
        };

        div()
            .id("plugin-graph-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            // Header
            .child(self.render_graph_header(node_count, connection_count, plugin_count, cx))
            // Main content: sidebar + canvas
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Sidebar palette
                    .child(self.render_graph_palette(cx))
                    // Canvas area with drop support
                    .child({
                        let drag_highlight = Theme::opacity_8pct(theme.drag_over_border);
                        div()
                            .id("graph-canvas-area")
                            .flex_1()
                            .size_full()
                            .relative()
                            .drag_over::<PaletteDragData>(move |style, _, _, _| {
                                style.bg(drag_highlight)
                            })
                            .on_drop(cx.listener(|view, data: &PaletteDragData, _window, cx| {
                                view.handle_palette_drop(data, cx);
                            }))
                            .when_some(workflow_canvas, |el, canvas| el.child(canvas))
                    }),
            )
    }

    /// Handle dropping a palette item onto the canvas.
    ///
    /// Adds the node to both the `WorkflowCanvas` (visual) and the
    /// `PluginGraph` (persistent data model) so the node survives canvas
    /// rebuilds.
    fn handle_palette_drop(&mut self, data: &PaletteDragData, cx: &mut Context<Self>) {
        let canvas = self.state.read(cx).app.plugin_state.workflow_canvas.clone();
        if let Some(canvas) = canvas {
            // Offset drop position by existing node count so nodes don't stack
            let node_count = self.state.read(cx).app.plugin_state.graph.nodes.len()
                + self
                    .state
                    .read(cx)
                    .app
                    .plugin_state
                    .graph
                    .special_nodes
                    .len();
            let drop_x = 300.0 + (node_count as f32 % 4.0) * 180.0;
            let drop_y = 150.0 + (node_count as f32 / 4.0).floor() * 120.0;

            // Also persist plugin drops into the PluginGraph
            let graph_node_id = match &data.item_type {
                PaletteItemType::Plugin(plugin_type) => {
                    let id = self.state.update(cx, |state, _| {
                        state.app.plugin_state.graph.add_plugin_node(
                            plugin_type,
                            sotf_audio_player::NodePosition::new(drop_x, drop_y),
                        )
                    });
                    Some(id)
                }
                PaletteItemType::Player => None, // Player nodes are special, not plugin nodes
            };

            // Create node based on item type
            let node = match &data.item_type {
                PaletteItemType::Player => {
                    WorkflowNodeData::new("Player", Position::new(100.0, 200.0))
                        .with_ports(0, 2) // Output only: stereo
                        .with_size(160.0, 80.0)
                        .with_user_data(serde_json::json!({
                            "node_type": NODE_TYPE_PLAYER,
                            "channels": 2,
                        }))
                }
                PaletteItemType::Plugin(plugin_type) => {
                    let (inputs, outputs) = plugin_channel_counts(plugin_type);
                    let (max_in, max_out) = plugin_max_ports(plugin_type);
                    let mut user_data = serde_json::json!({
                        "node_type": NODE_TYPE_PLUGIN,
                        "plugin_type": format!("{:?}", plugin_type),
                        "enabled": true,
                    });
                    // Store the PluginGraph node ID so the modal can look up settings
                    if let Some(id) = graph_node_id {
                        user_data["plugin_node_id"] = serde_json::Value::String(id.to_string());
                    }
                    WorkflowNodeData::new(plugin_type.name(), Position::new(drop_x, drop_y))
                        .with_ports(inputs, outputs)
                        .with_max_ports(max_in, max_out)
                        .with_size(160.0, 90.0)
                        .with_user_data(user_data)
                }
            };

            canvas.update(cx, |canvas, cx| {
                canvas.add_node_notify(node, cx);
            });

            // Update stats
            cx.notify();
        }
    }

    /// Render the graph header with stats
    fn render_graph_header(
        &self,
        node_count: usize,
        connection_count: usize,
        plugin_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        div()
            .flex()
            .justify_between()
            .items_center()
            .px(d.card)
            .py(d.pad_y)
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            // Home button on the left
            .child(
                div()
                    .id("graph-home-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(rems(2.5))
                    .h(rems(2.0))
                    .cursor_pointer()
                    .rounded(d.r_md)
                    .hover(move |s| s.bg(surface_hover))
                    .child(Icon::new(IconName::Home).color(text_muted))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        state_for_home.update(cx, |state, _cx| {
                            state.app.ui_state.current_screen = Screen::Library;
                        });
                    }),
            )
            // Title and stats
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child("SIGNAL"),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!("#{} plugins", plugin_count)),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!("#{} links", connection_count)),
                    ),
            )
            // Reset view button on the right
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!("{} nodes", node_count)),
                    )
                    // Reset view button
                    .child(
                        div()
                            .id("reset-view")
                            .px(d.pad_y)
                            .py(d.pad_y_half)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.surface_hover))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    if let Some(canvas) =
                                        view.state.read(cx).app.plugin_state.workflow_canvas.clone()
                                    {
                                        canvas.update(cx, |canvas, cx| {
                                            canvas.reset_viewport(cx);
                                        });
                                    }
                                    cx.notify();
                                }),
                            )
                            .child("Reset View"),
                    ),
            )
    }

    /// Render the sidebar palette with plugin types (draggable)
    fn render_graph_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        // Input sources
        let input_items = vec![("Player", PaletteItemType::Player, theme.success)];

        // Plugin categories
        let plugin_categories = vec![
            (
                "EQ",
                vec![
                    (PluginType::EQ, "Parametric"),
                    (PluginType::LinearPhaseEq, "Linear Phase"),
                    (PluginType::DynamicEq, "Dynamic EQ"),
                ],
            ),
            (
                "Dynamics",
                vec![
                    (PluginType::Gain, "Gain"),
                    (PluginType::Compressor, "Comp"),
                    (PluginType::Limiter, "Limiter"),
                    (PluginType::Gate, "Gate"),
                    (PluginType::Expander, "Expander"),
                    (PluginType::DeEsser, "De-Esser"),
                    (PluginType::TransientShaper, "Transient"),
                    (PluginType::SpectralCompressor, "Spectral C."),
                ],
            ),
            (
                "Color",
                vec![
                    (PluginType::Saturation, "Saturation"),
                    (PluginType::Crossfeed, "Crossfeed"),
                ],
            ),
            (
                "Spatial",
                vec![
                    (PluginType::Upmixer, "Upmix"),
                    (PluginType::AAE, "AAE Reverb"),
                    (PluginType::Downmix, "Downmix"),
                    (PluginType::MonoToStereo, "Mono->2.0"),
                    (PluginType::BinauralDecoder, "Binaural"),
                    (PluginType::Convolution, "Convo"),
                    (PluginType::XTC, "XTC"),
                    (PluginType::StereoImager, "Stereo Img"),
                    (PluginType::Delay, "Delay"),
                ],
            ),
            (
                "Monitor",
                vec![
                    (PluginType::LoudnessCompensation, "Loud Comp"),
                    (PluginType::FletcherMunson, "F-Munson"),
                    (PluginType::LoudnessMonitor, "LUFS"),
                    (PluginType::SpectrumAnalyzer, "Spectrum"),
                    (PluginType::ChannelMuteSolo, "M/S"),
                ],
            ),
            (
                "Denoising",
                vec![
                    (PluginType::Denoiser, "Denoise"),
                    (PluginType::Declick, "Declick"),
                    (PluginType::HissReducer, "Hiss"),
                    (PluginType::SpeechDenoiser, "Speech"),
                    (PluginType::Aec, "AEC"),
                    (PluginType::Pnd, "PND"),
                ],
            ),
            (
                "Utility",
                vec![
                    (PluginType::Matrix, "Matrix"),
                    (PluginType::ABCompare, "A/B"),
                ],
            ),
        ];

        div()
            .id("graph-palette")
            .w(rems(8.75))
            .flex_shrink_0()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .py(d.pad_y)
            .overflow_y_scroll()
            // Input sources section
            .child(
                div()
                    .px(d.pad_y)
                    .pb(d.pad_y)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child("INPUT"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .px(d.pad_y)
                    .mb(d.gap_md)
                    .children(input_items.into_iter().map(|(label, item_type, color)| {
                        let drag_data = PaletteDragData {
                            item_type,
                            label: label.to_string(),
                            color,
                            text_on_accent: theme.text_on_accent,
                        };
                        div()
                            .id(SharedString::from(format!("palette-{}", label)))
                            .px(d.pad_y)
                            .py(d.pad_y_half)
                            .rounded(d.r_md)
                            .bg(theme.background)
                            .border_l_2()
                            .border_color(color)
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .cursor_grab()
                            .hover(|s| s.bg(theme.background_secondary))
                            .on_drag(drag_data, |info, _pos, _window, cx| {
                                cx.new(|_| info.clone())
                            })
                            .child(label)
                    })),
            )
            // Plugins section header
            .child(
                div()
                    .px(d.pad_y)
                    .pb(d.pad_y)
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child("PLUGINS"),
            )
            // Plugin categories
            .children(plugin_categories.into_iter().map(|(category, plugins)| {
                let theme = theme.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .px(d.pad_y)
                    .mb(d.gap)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(category),
                    )
                    .children(plugins.into_iter().map(|(plugin_type, label)| {
                        let color = plugin_color(&plugin_type, &theme);
                        let drag_data = PaletteDragData {
                            item_type: PaletteItemType::Plugin(plugin_type.clone()),
                            label: label.to_string(),
                            color,
                            text_on_accent: theme.text_on_accent,
                        };
                        div()
                            .id(SharedString::from(format!("palette-{:?}", plugin_type)))
                            .px(d.pad_y)
                            .py(d.pad_y_half)
                            .rounded(d.r_md)
                            .bg(theme.background)
                            .border_l_2()
                            .border_color(color)
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .cursor_grab()
                            .hover(|s| s.bg(theme.background_secondary))
                            .on_drag(drag_data, |info, _pos, _window, cx| {
                                cx.new(|_| info.clone())
                            })
                            .child(label)
                    }))
            }))
    }
}

/// Get a color for a plugin type
fn plugin_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    match plugin_type {
        PluginType::EQ => theme.info,
        PluginType::Gain => theme.success,
        PluginType::Compressor
        | PluginType::Limiter
        | PluginType::Gate
        | PluginType::Expander
        | PluginType::MultibandCompressor
        | PluginType::MultibandExpander => theme.warning,
        PluginType::AAE
        | PluginType::Upmixer
        | PluginType::BinauralDecoder
        | PluginType::Convolution
        | PluginType::XTC => theme.accent,
        PluginType::LoudnessCompensation
        | PluginType::FletcherMunson
        | PluginType::LoudnessMonitor
        | PluginType::SpectrumAnalyzer
        | PluginType::ChannelMuteSolo => theme.text_muted,
        PluginType::Matrix => theme.accent,
        PluginType::Denoiser => theme.info,
        PluginType::Declick => theme.info,
        PluginType::HissReducer => theme.info,
        PluginType::SpeechDenoiser => theme.info,
        PluginType::Pnd => theme.info,
        PluginType::ABCompare => theme.warning, // A/B Compare - use warning color
        PluginType::BandSplit | PluginType::BandMerge => theme.accent, // Band processing - use accent
        PluginType::Downmix => theme.accent,
        PluginType::MonoToStereo => theme.accent,
        PluginType::Crossfeed => theme.accent,
        PluginType::Delay => theme.info,
        PluginType::Aec => theme.info,
        PluginType::Beamformer => theme.accent,
        PluginType::AmbisonicsDecoder => theme.accent,
        PluginType::StereoImager => theme.accent,
        PluginType::DeEsser => theme.warning,
        PluginType::TransientShaper => theme.warning,
        PluginType::Saturation => theme.warning,
        PluginType::DynamicEq => theme.warning,
        PluginType::LinearPhaseEq => theme.success,
        PluginType::SpectralCompressor => theme.warning,
    }
}

/// Get input and output channel counts for a plugin type
/// Maximum port counts for a plugin type (None = fixed, matches default counts)
fn plugin_max_ports(plugin_type: &PluginType) -> (Option<usize>, Option<usize>) {
    match plugin_type {
        // Matrix is growable up to 8x8
        PluginType::Matrix => (Some(8), Some(8)),
        // Downmix: can accept up to 8 input channels
        PluginType::Downmix => (Some(8), Some(2)),
        // Ambisonics decoder: can output up to 8 channels
        PluginType::AmbisonicsDecoder => (Some(4), Some(8)),
        // All other plugins: fixed port counts (no growth allowed)
        _ => {
            let (i, o) = plugin_channel_counts(plugin_type);
            (Some(i), Some(o))
        }
    }
}

fn plugin_channel_counts(plugin_type: &PluginType) -> (usize, usize) {
    match plugin_type {
        // Most plugins are 2-in, 2-out (stereo passthrough)
        PluginType::EQ
        | PluginType::Gain
        | PluginType::Compressor
        | PluginType::Limiter
        | PluginType::Gate
        | PluginType::Expander
        | PluginType::MultibandCompressor
        | PluginType::MultibandExpander
        | PluginType::LoudnessCompensation
        | PluginType::FletcherMunson
        | PluginType::LoudnessMonitor
        | PluginType::SpectrumAnalyzer
        | PluginType::ChannelMuteSolo
        | PluginType::XTC
        | PluginType::Denoiser
        | PluginType::Declick
        | PluginType::HissReducer
        | PluginType::SpeechDenoiser
        | PluginType::Pnd => (2, 2),
        // Upmixer/AAE: stereo in, multi-channel out
        PluginType::Upmixer | PluginType::AAE => (2, 5),
        // Binaural decoder: multi-channel in, stereo out
        PluginType::BinauralDecoder => (5, 2),
        // Convolution: stereo in/out
        PluginType::Convolution => (2, 2),
        // Matrix: variable in/out (default to 2x2)
        PluginType::Matrix => (2, 2),
        // A/B Compare: stereo in/out
        PluginType::ABCompare => (2, 2),
        // Band Split: 2 in, 4 out (2 bands x 2 channels)
        PluginType::BandSplit => (2, 4),
        // Band Merge: 4 in, 2 out (2 bands x 2 channels merged back)
        PluginType::BandMerge => (4, 2),
        // Downmix: multi-channel in, stereo out
        PluginType::Downmix => (6, 2),
        // Mono to Stereo: mono in, stereo out
        PluginType::MonoToStereo => (1, 2),
        // Crossfeed: stereo in/out
        PluginType::Crossfeed => (2, 2),
        PluginType::Delay => (2, 2),
        // AEC: 2 in (mic + ref), 1 out
        PluginType::Aec => (2, 1),
        // Beamformer: M in, 1 out (default 2 mics)
        PluginType::Beamformer => (2, 1),
        // Ambisonics decoder: 4ch (FOA) in, multi-channel out
        PluginType::AmbisonicsDecoder => (4, 6),
        // Stereo imager: stereo only
        PluginType::StereoImager => (2, 2),
        // De-esser: stereo in/out
        PluginType::DeEsser => (2, 2),
        // Transient shaper: in-place processing
        PluginType::TransientShaper => (2, 2),
        // Saturation: in-place processing
        PluginType::Saturation => (2, 2),
        // Dynamic EQ: in-place processing
        PluginType::DynamicEq => (2, 2),
        // Linear-Phase EQ: in-place processing
        PluginType::LinearPhaseEq => (2, 2),
        // Spectral compressor: in-place processing
        PluginType::SpectralCompressor => (2, 2),
    }
}

// ============================================================================
// Workflow Canvas Integration
// ============================================================================

/// Build a WorkflowGraph from the PluginGraph, or create a default graph
fn build_workflow_graph(
    plugin_graph: &Option<PluginGraph>,
    default_output_name: &str,
    default_output_channels: usize,
) -> WorkflowGraph {
    // If we have an existing plugin graph, convert it
    if let Some(graph) = plugin_graph {
        return convert_plugin_graph(graph);
    }

    // Otherwise, create a default graph: Player → EQ → Output
    create_default_graph(default_output_name, default_output_channels)
}

/// Create the default graph: Player → EQ → Output
fn create_default_graph(output_name: &str, output_channels: usize) -> WorkflowGraph {
    let mut graph = WorkflowGraph::new();

    // Player node (input source) - only output ports (green)
    let player_node = WorkflowNodeData::new("Player", Position::new(50.0, 150.0))
        .with_ports(0, 2) // No inputs, 2 outputs (stereo)
        .with_size(140.0, 80.0)
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_PLAYER,
            "channels": 2,
        }));
    let player_id = player_node.id;

    // EQ node (plugin) - both input and output ports
    let eq_node = WorkflowNodeData::new("EQ", Position::new(250.0, 150.0))
        .with_ports(2, 2) // 2 inputs, 2 outputs
        .with_size(140.0, 90.0)
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_PLUGIN,
            "plugin_type": "EQ",
            "enabled": true,
        }));
    let eq_id = eq_node.id;

    // Output node - only input ports (blue)
    let output_channels_clamped = output_channels.min(8);
    let output_node = WorkflowNodeData::new(output_name, Position::new(450.0, 150.0))
        .with_ports(output_channels_clamped, 0) // N inputs, no outputs
        .with_size(160.0, 80.0 + (output_channels_clamped as f32 * 8.0))
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_OUTPUT_DEVICE,
            "channels": output_channels,
        }));
    let output_id = output_node.id;

    // Add nodes
    graph.add_node(player_node);
    graph.add_node(eq_node);
    graph.add_node(output_node);

    // Add connections: Player → EQ → Output
    // Connect stereo (2 channels) with "fat" links (all channels bundled)
    let _ = graph.add_connection(player_id, 0, eq_id, 0); // L channel
    let _ = graph.add_connection(player_id, 1, eq_id, 1); // R channel
    let _ = graph.add_connection(eq_id, 0, output_id, 0); // L channel
    let _ = graph.add_connection(eq_id, 1, output_id, 1); // R channel

    graph
}

/// Convert an existing PluginGraph to a WorkflowGraph
fn convert_plugin_graph(graph: &PluginGraph) -> WorkflowGraph {
    let mut workflow_graph = WorkflowGraph::new();

    // Map from plugin graph node IDs to workflow node IDs
    let mut id_map: std::collections::HashMap<NodeId, NodeId> = std::collections::HashMap::new();

    // Convert special nodes (Input/Output devices) to workflow nodes
    for (special_node_id, special_node) in &graph.special_nodes {
        let (input_ports, output_ports, node_type) = match special_node.node_type {
            SpecialNodeType::Input => (0, special_node.channels.min(8), NODE_TYPE_INPUT_DEVICE),
            SpecialNodeType::Output => (special_node.channels.min(8), 0, NODE_TYPE_OUTPUT_DEVICE),
            SpecialNodeType::Split => (1, special_node.channels.min(8), "split"),
            SpecialNodeType::Merge => (special_node.channels.min(8), 1, "merge"),
        };

        let height = 80.0 + (input_ports.max(output_ports) as f32 * 8.0);
        // Special nodes: allow growth up to 8 ports on the variable side
        let (max_in, max_out) = match special_node.node_type {
            SpecialNodeType::Input => (Some(0), Some(8)),
            SpecialNodeType::Output => (Some(8), Some(0)),
            SpecialNodeType::Split => (Some(1), Some(8)),
            SpecialNodeType::Merge => (Some(8), Some(1)),
        };
        let workflow_node = WorkflowNodeData::new(
            special_node.display_name(),
            Position::new(special_node.position.x, special_node.position.y),
        )
        .with_ports(input_ports, output_ports)
        .with_max_ports(max_in, max_out)
        .with_size(160.0, height)
        .with_user_data(serde_json::json!({
            "node_type": node_type,
            "channels": special_node.channels,
        }));

        let workflow_id = workflow_node.id;
        id_map.insert(*special_node_id, workflow_id);
        workflow_graph.add_node(workflow_node);
    }

    // Convert plugin nodes to workflow nodes
    for (graph_node_id, node) in &graph.nodes {
        let plugin_type = node.plugin.plugin_type();
        let (input_ports, output_ports) = (node.input_channels.min(8), node.output_channels.min(8));

        let height = 90.0 + ((input_ports.max(output_ports)).saturating_sub(2) as f32 * 8.0);
        let (max_in, max_out) = plugin_max_ports(&plugin_type);
        let workflow_node = WorkflowNodeData::new(
            plugin_type.name(),
            Position::new(node.position.x, node.position.y),
        )
        .with_ports(input_ports, output_ports)
        .with_max_ports(max_in, max_out)
        .with_size(160.0, height)
        .with_user_data(serde_json::json!({
            "node_type": NODE_TYPE_PLUGIN,
            "plugin_type": format!("{:?}", plugin_type),
            "plugin_node_id": graph_node_id.to_string(),
            "enabled": node.plugin.enabled,
        }));

        let workflow_id = workflow_node.id;
        id_map.insert(*graph_node_id, workflow_id);
        workflow_graph.add_node(workflow_node);
    }

    // Convert connections
    for conn in &graph.connections {
        if let (Some(&from_id), Some(&to_id)) =
            (id_map.get(&conn.from_node), id_map.get(&conn.to_node))
        {
            let _ = workflow_graph.add_connection(from_id, conn.from_port, to_id, conn.to_port);
        }
    }

    workflow_graph
}

/// Create a WorkflowTheme from the app Theme
fn create_workflow_theme(theme: &Theme) -> WorkflowTheme {
    WorkflowTheme {
        canvas_background: theme.background,
        grid_color: Rgba {
            r: theme.border.r,
            g: theme.border.g,
            b: theme.border.b,
            a: 0.3,
        },
        grid_spacing: 20.0,
        node_background: theme.surface,
        node_border: theme.border,
        node_border_selected: theme.accent,
        node_header: Rgba {
            r: theme.surface.r * 0.8,
            g: theme.surface.g * 0.8,
            b: theme.surface.b * 0.8,
            a: theme.surface.a,
        },
        node_text: theme.text_primary,
        node_border_radius: 8.0,
        node_header_height: 28.0,
        node_content_padding: 8.0,
        port_input: theme.info,     // Blue for input ports
        port_output: theme.success, // Green for output ports
        port_hover: theme.accent_hover,
        port_valid: theme.success,
        port_invalid: theme.error,
        port_radius: 6.0,
        connection_color: theme.text_secondary,
        connection_selected: theme.accent,
        connection_width: 4.0,      // Fat links (all channels)
        connection_width_thin: 1.5, // Thin links (single channel)
        connection_preview: Rgba {
            r: theme.accent.r,
            g: theme.accent.g,
            b: theme.accent.b,
            a: 0.6,
        },
        selection_fill: Rgba {
            r: theme.accent.r,
            g: theme.accent.g,
            b: theme.accent.b,
            a: 0.1,
        },
        selection_border: theme.accent,
    }
}

// ============================================================================
// Per-Node Context Menu (Edit / Solo / Bypass / Remove)
// ============================================================================
//
// Mirrors the rack's per-plugin actions. Edit is also available via
// double-click; the menu is the discoverable variant. Solo / Bypass / Remove
// have no other entry point in the graph view, so the menu is the only way
// to reach them.

const NODE_MENU_EDIT: &str = "node-edit";
const NODE_MENU_SOLO: &str = "node-solo";
const NODE_MENU_BYPASS: &str = "node-bypass";
const NODE_MENU_REMOVE: &str = "node-remove";

fn plugin_node_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new(NODE_MENU_EDIT, "Edit Parameters"),
        MenuItem::separator(),
        MenuItem::new(NODE_MENU_BYPASS, "Bypass / Activate"),
        MenuItem::new(NODE_MENU_SOLO, "Solo"),
        MenuItem::separator(),
        MenuItem::new(NODE_MENU_REMOVE, "Remove"),
    ]
}

/// Resolve the workflow node id to a `(plugin_uuid, linear_index)` pair so
/// the same dispatch can address the plugin via either its stable UUID
/// (used by `editing_graph_node_uuid`) or its linear index (used by
/// `PluginEditingManager::toggle_plugin` etc.). Returns None if the
/// workflow node isn't a plugin node — the menu was registered only for
/// plugin nodes, but this defensively handles user_data drift.
fn resolve_plugin_node(
    canvas: &Entity<WorkflowCanvas>,
    state: &Entity<crate::app::AppState>,
    node_id: NodeId,
    cx: &mut App,
) -> Option<(sotf_audio_player::GraphNodeId, usize)> {
    let plugin_uuid = canvas
        .read(cx)
        .graph()
        .nodes
        .get(&node_id)
        .and_then(|n| n.user_data.get("plugin_node_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| sotf_audio_player::GraphNodeId::parse_str(s).ok())?;

    let plugin_index = state
        .read(cx)
        .app
        .plugin_state
        .graph
        .plugins_linear()?
        .iter()
        .position(|n| n.id == plugin_uuid)?;

    Some((plugin_uuid, plugin_index))
}

fn dispatch_plugin_node_action(
    canvas: &Entity<WorkflowCanvas>,
    state: &Entity<crate::app::AppState>,
    menu_id: &SharedString,
    node_id: NodeId,
    cx: &mut App,
) {
    use crate::components::plugins::editing::PluginEditingManager;

    let menu_id = menu_id.as_ref();

    // Edit doesn't need an index — it sets the editing target by both
    // workflow node id and plugin UUID, just like the double-click path.
    if menu_id == NODE_MENU_EDIT {
        let plugin_uuid = canvas
            .read(cx)
            .graph()
            .nodes
            .get(&node_id)
            .and_then(|n| n.user_data.get("plugin_node_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| sotf_audio_player::GraphNodeId::parse_str(s).ok());
        state.update(cx, |state, _cx| {
            state.app.plugin_state.editing_plugin_node = Some(node_id);
            state.app.plugin_state.editing_graph_node_uuid = plugin_uuid;
            state.app.ui_state.input_mode = crate::app::InputMode::EditingPluginNode;
        });
        return;
    }

    let Some((_uuid, index)) = resolve_plugin_node(canvas, state, node_id, cx) else {
        return;
    };

    state.update(cx, |state, _cx| match menu_id {
        NODE_MENU_BYPASS => state.app.toggle_plugin(index),
        NODE_MENU_SOLO => state.app.toggle_plugin_solo(index),
        NODE_MENU_REMOVE => state.app.remove_plugin(index),
        _ => {}
    });
}

/// Build menu items for the workflow canvas context menu
fn build_menu_items(
    input_devices: &[AudioDevice],
    output_devices: &[AudioDevice],
) -> Vec<MenuItem> {
    let mut items = Vec::new();

    // Input sources section
    items.push(MenuItem::new("input-header", "Input Sources").disabled(true));
    items.push(MenuItem::new("input-player", "Player (Audio Files)"));

    // Add hardware input devices
    for (idx, device) in input_devices.iter().enumerate() {
        let channels = device
            .default_config
            .as_ref()
            .map(|c| c.channels)
            .unwrap_or(2);
        let name = format!("{} ({} ch)", device.name, channels);
        items.push(MenuItem::new(format!("input-device-{}", idx), name));
    }

    items.push(MenuItem::separator());

    // Output devices section
    items.push(MenuItem::new("output-header", "Output Devices").disabled(true));
    for (idx, device) in output_devices.iter().enumerate() {
        let channels = device
            .default_config
            .as_ref()
            .map(|c| c.channels)
            .unwrap_or(2);
        let name = format!("{} ({} ch)", device.name, channels);
        items.push(MenuItem::new(format!("output-{}", idx), name));
    }
    if output_devices.is_empty() {
        items.push(MenuItem::new("no-outputs", "(no output devices)").disabled(true));
    }

    items.push(MenuItem::separator());

    // EQ section
    items.push(MenuItem::new("eq-header", "EQ").disabled(true));
    items.push(MenuItem::new("plugin-eq", "Parametric EQ"));
    items.push(MenuItem::new("plugin-linear-phase-eq", "Linear-Phase EQ"));
    items.push(MenuItem::new("plugin-dynamic-eq", "Dynamic EQ"));

    items.push(MenuItem::separator());

    // Dynamics section
    items.push(MenuItem::new("dynamics-header", "Dynamics").disabled(true));
    items.push(MenuItem::new("plugin-gain", "Gain"));
    items.push(MenuItem::new("plugin-compressor", "Compressor"));
    items.push(MenuItem::new("plugin-limiter", "Limiter"));
    items.push(MenuItem::new("plugin-gate", "Gate"));
    items.push(MenuItem::new("plugin-expander", "Expander"));
    items.push(MenuItem::new("plugin-de-esser", "De-Esser"));
    items.push(MenuItem::new("plugin-transient-shaper", "Transient Shaper"));
    items.push(MenuItem::new("plugin-spectral-comp", "Spectral Compressor"));
    items.push(MenuItem::new("plugin-saturation", "Saturation"));

    items.push(MenuItem::separator());

    // Spatial section
    items.push(MenuItem::new("spatial-header", "Spatial").disabled(true));
    items.push(MenuItem::new("plugin-upmixer", "Upmixer"));
    items.push(MenuItem::new("plugin-aae", "AAE Reverb"));
    items.push(MenuItem::new("plugin-downmix", "Downmix"));
    items.push(MenuItem::new("plugin-mono-to-stereo", "Mono to Stereo"));
    items.push(MenuItem::new("plugin-binaural", "Binaural Decoder"));
    items.push(MenuItem::new("plugin-convolution", "Convolution"));
    items.push(MenuItem::new("plugin-xtc", "Crosstalk Cancellation"));
    items.push(MenuItem::new("plugin-stereo-imager", "Stereo Imager"));
    items.push(MenuItem::new("plugin-crossfeed", "Crossfeed"));
    items.push(MenuItem::new("plugin-delay", "Delay"));

    items.push(MenuItem::separator());

    // Monitor section
    items.push(MenuItem::new("monitor-header", "Monitor").disabled(true));
    items.push(MenuItem::new(
        "plugin-loudness-comp",
        "Loudness Compensation",
    ));
    items.push(MenuItem::new("plugin-fletcher-munson", "Fletcher-Munson"));
    items.push(MenuItem::new("plugin-loudness-mon", "Loudness Monitor"));
    items.push(MenuItem::new("plugin-spectrum", "Spectrum Analyzer"));
    items.push(MenuItem::new("plugin-mute-solo", "Channel Mute/Solo"));

    items.push(MenuItem::separator());

    // Denoising section
    items.push(MenuItem::new("denoising-header", "Denoising").disabled(true));
    items.push(MenuItem::new("plugin-denoiser", "Denoiser"));
    items.push(MenuItem::new("plugin-declick", "Declick"));
    items.push(MenuItem::new("plugin-hiss-reducer", "Hiss Reducer"));
    items.push(MenuItem::new("plugin-speech-denoiser", "Speech Denoiser"));
    items.push(MenuItem::new("plugin-aec", "AEC"));
    items.push(MenuItem::new("plugin-pnd", "PND Varispeed"));

    items.push(MenuItem::separator());

    // Utility section
    items.push(MenuItem::new("utility-header", "Utility").disabled(true));
    items.push(MenuItem::new("plugin-matrix", "Matrix Mixer"));
    items.push(MenuItem::new("plugin-ab-compare", "A/B Compare"));

    items
}

// ============================================================================
// Plugin Node Modal
// ============================================================================

impl PlayerView {
    /// Render the plugin node editor modal
    pub(crate) fn render_plugin_node_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        // Get the node being edited
        let node_id = state.app.plugin_state.editing_plugin_node;
        let node_info = node_id.and_then(|id| {
            state
                .app
                .plugin_state
                .workflow_canvas
                .as_ref()
                .and_then(|canvas| {
                    let canvas_read = canvas.read(cx);
                    let graph = canvas_read.graph();
                    graph.nodes.get(&id).map(|node| {
                        let name = node.title.clone();
                        let node_type = node
                            .user_data
                            .get("node_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let plugin_type = node
                            .user_data
                            .get("plugin_type")
                            .and_then(|v| v.as_str())
                            .map(|s: &str| s.to_string());
                        let plugin_node_id = node
                            .user_data
                            .get("plugin_node_id")
                            .and_then(|v| v.as_str())
                            .map(|s: &str| s.to_string());
                        (name, node_type, plugin_type, plugin_node_id)
                    })
                })
        });

        let (node_name, node_type, plugin_type, plugin_node_id) =
            node_info.unwrap_or_else(|| ("Unknown".to_string(), "unknown".to_string(), None, None));

        // Look up the actual plugin settings and linear index from the plugin graph.
        // Also resolve the GraphNodeId UUID for the node-ID-based editing path.
        let graph_node_uuid = plugin_node_id
            .as_ref()
            .and_then(|id_str| sotf_audio_player::GraphNodeId::parse_str(id_str).ok());

        let (plugin_settings, plugin_linear_idx) = graph_node_uuid
            .map(|uuid| {
                let graph = &state.app.plugin_state.graph;
                let settings = graph.nodes.get(&uuid).map(|n| n.plugin.settings.clone());
                let linear_idx = graph.linear_index_of_node(uuid);
                (settings, linear_idx)
            })
            .unwrap_or((None, None));

        // For non-linear graphs, linear_idx is None but the node still exists.
        // Enable editing whenever the plugin exists in the graph — the
        // `editing_graph_node_uuid` (set on double-click) ensures parameter
        // changes are dispatched via GraphNodeId, bypassing linear indices.
        let node_exists_in_graph = graph_node_uuid
            .is_some_and(|uuid| state.app.plugin_state.graph.nodes.contains_key(&uuid));

        let state_for_close = self.state.clone();

        // Determine the title based on node type
        let title = if node_type == NODE_TYPE_PLUGIN {
            node_name.clone()
        } else if node_type == NODE_TYPE_PLAYER {
            "Player".to_string()
        } else if node_type == NODE_TYPE_OUTPUT_DEVICE {
            format!("Output: {}", node_name)
        } else if node_type == NODE_TYPE_INPUT_DEVICE {
            format!("Input: {}", node_name)
        } else {
            node_name.clone()
        };

        // Compute modal dimensions: 85% of window, clamped to reasonable bounds
        let window_w = state.app.ui_state.window_width.max(400.0);
        let window_h = state.app.ui_state.window_height.max(300.0);
        let modal_w = (window_w * 0.85).clamp(400.0, 1600.0);
        let modal_h = (window_h * 0.85).clamp(300.0, 1200.0);

        // Create the modal
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.overlay_bg)
            .on_mouse_down(MouseButton::Left, {
                let state = state_for_close.clone();
                move |_, _, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                        state.app.plugin_state.editing_plugin_node = None;
                        state.app.plugin_state.editing_graph_node_uuid = None;
                    });
                }
            })
            .child(
                div()
                    .id("plugin-node-modal")
                    .w(px(modal_w))
                    .max_h(px(modal_h))
                    .bg(theme.surface)
                    .rounded(d.r_lg)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    // Stop propagation so clicking inside doesn't close
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    // Header
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .px(d.card)
                            .py(d.pad_x)
                            .bg(theme.background_secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_size(d.text_base)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(d.gap)
                                    // Load button
                                    .child(
                                        div()
                                            .id("modal-load")
                                            .px(d.pad_x)
                                            .py(d.pad_y_half)
                                            .rounded(d.r_md)
                                            .bg(theme.surface)
                                            .text_size(d.text_sm)
                                            .text_color(theme.text_secondary)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.surface_hover))
                                            .child("Load"),
                                    )
                                    // Save button
                                    .child(
                                        div()
                                            .id("modal-save")
                                            .px(d.pad_x)
                                            .py(d.pad_y_half)
                                            .rounded(d.r_md)
                                            .bg(theme.surface)
                                            .text_size(d.text_sm)
                                            .text_color(theme.text_secondary)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme.surface_hover))
                                            .child("Save"),
                                    )
                                    // Close button
                                    .child({
                                        let state = state_for_close.clone();
                                        div()
                                            .id("modal-close")
                                            .px(d.pad_x)
                                            .py(d.pad_y_half)
                                            .rounded(d.r_md)
                                            .bg(theme.error)
                                            .text_size(d.text_sm)
                                            .text_color(theme.text_on_accent)
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.8))
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                cx.stop_propagation();
                                                state.update(cx, |state, _cx| {
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::Normal;
                                                    state.app.plugin_state.editing_plugin_node =
                                                        None;
                                                    state
                                                        .app
                                                        .plugin_state
                                                        .editing_graph_node_uuid = None;
                                                });
                                            })
                                            .child("Close")
                                    }),
                            ),
                    )
                    // Body - plugin content
                    .child(
                        div()
                            .id("plugin-modal-body")
                            .flex_1()
                            .overflow_y_scroll()
                            .p(d.card)
                            .child(self.render_plugin_node_content(
                                &node_type,
                                plugin_type.as_deref(),
                                plugin_settings.as_ref(),
                                plugin_linear_idx,
                                node_exists_in_graph,
                                &theme,
                                cx,
                            )),
                    ),
            )
    }

    /// Render the content for a plugin node based on its type.
    ///
    /// `plugin_linear_idx` is the linear index in the plugin graph when available
    /// (for linear graphs). `node_exists` indicates the node exists in the
    /// `PluginGraph` even if a linear index isn't available (non-linear graph).
    /// Editing is enabled when either is true — the `editing_graph_node_uuid`
    /// context handles dispatching parameter changes via node ID.
    fn render_plugin_node_content(
        &self,
        node_type: &str,
        plugin_type: Option<&str>,
        plugin_settings: Option<&PluginSettings>,
        plugin_linear_idx: Option<usize>,
        node_exists: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        match node_type {
            NODE_TYPE_PLUGIN => {
                // If we have actual plugin settings, render the real plugin UI.
                // Enable editing when the node is in the graph (linear or non-linear).
                if let Some(settings) = plugin_settings {
                    let idx = plugin_linear_idx.unwrap_or(0);
                    let editing = plugin_linear_idx.is_some() || node_exists;
                    return self.render_plugin_settings_ui(settings, idx, editing, theme, cx);
                }

                // Fallback: show placeholder based on plugin type
                match plugin_type {
                    Some("EQ") => div()
                        .flex()
                        .flex_col()
                        .gap(d.section)
                        .child(
                            div()
                                .text_size(d.text_lg)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("Parametric EQ"),
                        )
                        .child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(theme.text_muted)
                                .child("No plugin data available"),
                        )
                        .into_any_element(),
                    Some("Gain") => div()
                        .flex()
                        .flex_col()
                        .gap(d.section)
                        .child(
                            div()
                                .text_size(d.text_lg)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("Gain"),
                        )
                        .child(
                            div()
                                .text_size(d.text_sm)
                                .text_color(theme.text_muted)
                                .child("No plugin data available"),
                        )
                        .into_any_element(),
                    _ => div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(format!("Plugin type: {}", plugin_type.unwrap_or("Unknown")))
                        .into_any_element(),
                }
            }
            NODE_TYPE_PLAYER => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_lg)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Audio Player"),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("This node represents the audio file playback source."),
                )
                .into_any_element(),
            NODE_TYPE_OUTPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_lg)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Output Device"),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("This node represents the audio output destination."),
                )
                .into_any_element(),
            NODE_TYPE_INPUT_DEVICE => div()
                .flex()
                .flex_col()
                .gap(d.section)
                .child(
                    div()
                        .text_size(d.text_lg)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child("Input Device"),
                )
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("This node represents an audio input source."),
                )
                .into_any_element(),
            _ => div()
                .text_size(d.text_sm)
                .text_color(theme.text_muted)
                .child("Unknown node type")
                .into_any_element(),
        }
    }

    /// Render the actual plugin UI based on settings.
    ///
    /// Delegates to `render_plugin_content` which already handles all plugin
    /// types via the custom view registry and layout renderer fallback.
    fn render_plugin_settings_ui(
        &self,
        settings: &PluginSettings,
        plugin_idx: usize,
        is_editing: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (
            plugin_graph,
            loudness,
            selected_eq_band,
            spectrum_tilt_open,
            spectrum_ref_open,
            param_selection,
            plugin_data,
        ) = {
            let state = self.state.read(cx);
            let pd: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> = match settings {
                PluginSettings::SpectrumAnalyzer { .. } => {
                    state.app.playback.spectrum_info.clone().map(|s| {
                        std::sync::Arc::new(s) as std::sync::Arc<dyn std::any::Any + Send + Sync>
                    })
                }
                PluginSettings::Compressor { .. } => {
                    state.app.playback.compressor_info.clone().map(|c| {
                        std::sync::Arc::new(c) as std::sync::Arc<dyn std::any::Any + Send + Sync>
                    })
                }
                _ => None,
            };
            (
                state.app.plugin_state.graph.clone(),
                state.app.playback.loudness_info.clone(),
                state.app.plugin_state.selected_eq_band,
                state.app.spectrum_tilt_select_open,
                state.app.spectrum_reference_select_open,
                if is_editing {
                    state.app.plugin_state.plugin_param_selection
                } else {
                    0
                },
                pd,
            )
        };

        super::render_plugin_content(
            self.state.clone(),
            plugin_idx,
            settings,
            is_editing,
            param_selection,
            theme,
            false,
            selected_eq_band,
            loudness,
            plugin_data,
            spectrum_tilt_open,
            spectrum_ref_open,
            &plugin_graph,
            None,
            cx,
        )
    }
}

// ============================================================================
// PluginGraph ↔ WorkflowCanvas reconciliation
// ============================================================================

/// Bring `PluginGraph` (engine source-of-truth) back into sync with the
/// `WorkflowGraph` (canvas) after the user mutated the canvas — deleting a
/// node, drawing a new connection, pasting, undo/redo.
///
/// Reconciliation rules:
/// - Plugin nodes still in the canvas (matched by `user_data["plugin_node_id"]`)
///   keep their `Plugin` settings — we don't touch them.
/// - Plugin nodes missing from the canvas are removed from `PluginGraph.nodes`.
/// - Special I/O nodes are matched by `user_data["node_type"]` (Input/Output).
///   Special nodes missing from the canvas are removed from
///   `PluginGraph.special_nodes` so the engine config stays consistent.
/// - `PluginGraph.connections` is rebuilt from scratch from the canvas
///   connections (the canvas is now the source of truth for topology).
/// - If the currently-edited graph node was deleted, clear
///   `editing_graph_node_uuid`.
pub(crate) fn reconcile_plugin_graph_with_canvas(
    state: &mut crate::app::AppState,
    workflow: &WorkflowGraph,
) {
    use sotf_audio_player::GraphNodeId;
    use std::collections::{HashMap, HashSet};

    // 1) Map workflow NodeId → plugin GraphNodeId, and collect surviving
    //    plugin-node ids.
    let mut canvas_to_plugin: HashMap<NodeId, GraphNodeId> = HashMap::new();
    let mut surviving_plugin_ids: HashSet<GraphNodeId> = HashSet::new();
    let mut canvas_special_nodes: Vec<(NodeId, &str)> = Vec::new();

    for (workflow_id, node) in &workflow.nodes {
        if let Some(plugin_id_str) = node
            .user_data
            .get("plugin_node_id")
            .and_then(|v| v.as_str())
            && let Ok(graph_id) = GraphNodeId::parse_str(plugin_id_str)
        {
            canvas_to_plugin.insert(*workflow_id, graph_id);
            surviving_plugin_ids.insert(graph_id);
            continue;
        }
        // Not a plugin node — track it as a special I/O candidate.
        if let Some(node_type) = node.user_data.get("node_type").and_then(|v| v.as_str())
            && (node_type == NODE_TYPE_INPUT_DEVICE || node_type == NODE_TYPE_OUTPUT_DEVICE)
        {
            canvas_special_nodes.push((*workflow_id, node_type));
        }
    }

    let plugin_graph = &mut state.app.plugin_state.graph;

    // 2) Drop plugin nodes that vanished from the canvas.
    plugin_graph
        .nodes
        .retain(|id, _| surviving_plugin_ids.contains(id));

    // 3) Best-effort match each canvas special I/O node to a single
    //    PluginGraph special node by SpecialNodeType. The post-roomeq
    //    canvases produced by `apply_room_eq_as_graph` have one Input
    //    and one Output, so this is unambiguous in practice.
    let mut canvas_special_to_graph: HashMap<NodeId, sotf_audio_player::GraphNodeId> =
        HashMap::new();
    let mut used_special: HashSet<sotf_audio_player::GraphNodeId> = HashSet::new();
    for (workflow_id, node_type) in &canvas_special_nodes {
        let target = match *node_type {
            NODE_TYPE_INPUT_DEVICE => Some(SpecialNodeType::Input),
            NODE_TYPE_OUTPUT_DEVICE => Some(SpecialNodeType::Output),
            _ => None,
        };
        let Some(target_kind) = target else { continue };
        if let Some((&id, _)) = plugin_graph.special_nodes.iter().find(|(id, sn)| {
            !used_special.contains(id)
                && std::mem::discriminant(&sn.node_type) == std::mem::discriminant(&target_kind)
        }) {
            used_special.insert(id);
            canvas_special_to_graph.insert(*workflow_id, id);
        }
    }

    // 4) Drop special nodes the canvas no longer references.
    plugin_graph
        .special_nodes
        .retain(|id, _| used_special.contains(id));

    // 5) Rebuild connections from the canvas. Per-port wires are kept
    //    one-to-one (PluginGraph.connections carries port info, unlike
    //    the engine's PluginGraphConfig).
    plugin_graph.connections.clear();
    for conn in &workflow.connections {
        let from = canvas_to_plugin
            .get(&conn.from_node)
            .copied()
            .or_else(|| canvas_special_to_graph.get(&conn.from_node).copied());
        let to = canvas_to_plugin
            .get(&conn.to_node)
            .copied()
            .or_else(|| canvas_special_to_graph.get(&conn.to_node).copied());
        if let (Some(from), Some(to)) = (from, to) {
            let _ = plugin_graph.add_connection(from, conn.from_port, to, conn.to_port);
        }
    }

    // 6) Forget the edited node if it was deleted.
    if let Some(uuid) = state.app.plugin_state.editing_graph_node_uuid
        && !surviving_plugin_ids.contains(&uuid)
    {
        state.app.plugin_state.editing_graph_node_uuid = None;
        state.app.plugin_state.editing_plugin_node = None;
    }
}
