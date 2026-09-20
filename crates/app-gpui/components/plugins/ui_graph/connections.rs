use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, Select, SelectOption};

impl PlayerView {
    pub(super) fn render_routing_connection_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;
        let labels =
            crate::app::i18n::DesktopTranslations::routing_form(state.app.ui_state.language);
        let graph = &state.app.plugin_state.routing_controller().graph;
        let form = &state.app.plugin_state.graph_state.connection_form;
        let d = Ds::from_cx(cx);
        let mut root = div().flex().flex_wrap().items_end().gap(d.gap);
        let mut valid = true;
        for (side, label) in labels[..2].iter().enumerate() {
            let mut endpoints = Vec::new();
            for id in graph.nodes.keys().chain(graph.special_nodes.keys()) {
                let name = graph
                    .nodes
                    .get(id)
                    .map(|node| node.plugin.plugin_type().name().to_string())
                    .or_else(|| {
                        graph
                            .special_nodes
                            .get(id)
                            .map(|node| node.display_name().to_string())
                    })
                    .unwrap_or_default();
                let channels = if side == 0 {
                    graph.node_output_channels(*id)
                } else {
                    graph.node_input_channels(*id)
                };
                for port in 0..channels {
                    endpoints.push((
                        format!("{id}:{port}"),
                        format!("{name} · {} · {}", &id.to_string()[..8], port + 1),
                    ));
                }
            }
            endpoints.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
            let selected = form.endpoints[side].map(|(id, port)| format!("{id}:{port}"));
            valid &= selected
                .as_ref()
                .is_some_and(|value| endpoints.iter().any(|(key, _)| key == value));
            let toggle = self.state.downgrade();
            let change = self.state.downgrade();
            let highlight = self.state.downgrade();
            let mut select = Select::new(SharedString::from(format!("routing-endpoint-{side}")))
                .label(*label)
                .aria_label(*label)
                .options(
                    endpoints
                        .into_iter()
                        .map(|(key, label)| SelectOption::new(key, label))
                        .collect(),
                )
                .is_open(form.open == Some(side))
                .highlighted_index(form.highlighted[side])
                .on_highlight(move |index, _, cx| {
                    if let Some(state) = highlight.upgrade() {
                        state.update(cx, |state, cx| {
                            state
                                .app
                                .plugin_state
                                .graph_state
                                .connection_form
                                .highlighted[side] = index;
                            cx.notify();
                        });
                    }
                })
                .theme(theme.to_select_theme())
                .on_toggle(move |open, _, cx| {
                    if let Some(state) = toggle.upgrade() {
                        state.update(cx, |state, cx| {
                            state.app.plugin_state.graph_state.connection_form.open =
                                open.then_some(side);
                            cx.notify();
                        });
                    }
                })
                .on_change(move |value: &SharedString, _, cx| {
                    let Some((id, port)) = value.rsplit_once(':') else {
                        return;
                    };
                    let (Ok(id), Ok(port)) = (
                        sotf_audio_player::GraphNodeId::parse_str(id),
                        port.parse::<usize>(),
                    ) else {
                        return;
                    };
                    if let Some(state) = change.upgrade() {
                        state.update(cx, |state, cx| {
                            let form = &mut state.app.plugin_state.graph_state.connection_form;
                            form.endpoints[side] = Some((id, port));
                            form.open = None;
                            cx.notify();
                        });
                    }
                });
            if let Some(selected) = selected {
                select = select.selected(selected);
            }
            #[cfg(feature = "dev-api")]
            let select = {
                use crate::app::dev_api::DevTrackExt;
                select.dev_track(format!("routing.endpoint.{side}"))
            };
            root = root.child(div().min_w(rems(12.0)).flex_1().child(select));
        }
        let state = self.state.downgrade();
        root.child(track_routing_button(
            Button::new("routing-connect", labels[2])
                .size(ButtonSize::Sm)
                .disabled(!valid)
                .theme(theme.to_button_theme())
                .on_click_event(move |_, _, cx| {
                    if let Some(state) = state.upgrade() {
                        state.update(cx, |state, cx| {
                            let [Some((from, from_port)), Some((to, to_port))] =
                                state.app.plugin_state.graph_state.connection_form.endpoints
                            else {
                                return;
                            };
                            let graph = &mut state.app.plugin_state.routing_controller_mut().graph;
                            if from_port >= graph.node_output_channels(from)
                                || to_port >= graph.node_input_channels(to)
                            {
                                return;
                            }
                            match graph.add_connection(from, from_port, to, to_port) {
                                Ok(_) => state.app.plugin_state.graph_state.workflow_canvas = None,
                                Err(error) => {
                                    state.app.ui_state.toast_message =
                                        Some(crate::app::ToastMessage::error(error))
                                }
                            }
                            cx.notify();
                        });
                    }
                }),
            "routing-connect",
        ))
        .into_any_element()
    }
}

pub(super) fn track_routing_button(button: Button, id: &'static str) -> AnyElement {
    #[cfg(feature = "dev-api")]
    {
        use crate::app::dev_api::DevTrackExt;
        button.dev_track(id).into_any_element()
    }
    #[cfg(not(feature = "dev-api"))]
    {
        let _ = id;
        button.into_any_element()
    }
}
