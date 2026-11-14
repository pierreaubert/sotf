//! Dense multiband expander controls with stable desktop parameter addresses.

// Rust guideline compliant 2026-02-21
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::{AppState, i18n::PluginCommonTranslations};
use crate::components::design::Ds;
use crate::components::graphs::common::rgba_to_u32;
use crate::components::graphs::response_graphs::{ChartConfig, Series, render_line_chart};
use crate::components::plugins::ui_layout_renderer::{
    render_compact_choice_param, render_horizontal_param, render_narrow_horizontal_param,
};
use crate::components::plugins::ui_multiband_common::{band_tab_label, primary_uses_columns};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, SharedString, div};
use gpui_px::ScaleType;
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Button, ButtonSize, ButtonVariant, Text,
};
use sotf_audio_player::PluginSettings;
use sotf_plugins::param_specs::{
    ParamSpec,
    multiband_expander::{BAND_TEMPLATE as MEB, GLOBAL_PARAMS as ME},
};
use sotf_plugins::plugin_multiband_expander::target_attenuation_db;

/// Snapshot of global and selected-band settings for the editor.
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent UI options and status flags retain their existing state schema"
)]
pub struct MbExpanderRenderState {
    pub num_bands: usize,
    pub crossover_preset: i32,
    pub crossover_freq_1: f64,
    pub crossover_freq_2: f64,
    pub crossover_freq_3: f64,
    pub crossover_freq_4: f64,
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub range_db: f64,
    pub knee_db: f64,
    pub hysteresis_db: f64,
    pub hold_ms: f64,
    pub auto_makeup: bool,
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
    pub mix: f64,
    pub link_channels: bool,
    pub detection_mode: i32,
    pub lookahead_ms: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
}

/// Render primary controls before the response and secondary configuration.
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub fn render_mb_expander_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbExpanderRenderState,
    available_width: f32,
    available_height: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
    settings: &PluginSettings,
    cx: &App,
) -> AnyElement {
    let wide = available_width / layout_scale.max(0.01) >= 800.0;
    let primary_columns = primary_uses_columns(available_width, available_height, layout_scale);
    let selected = state.selected_band_idx.min(state.num_bands);
    let band = selected > 0;
    // The desktop protocol uses one-based bands at multiples of 100 and
    // retains the old standalone dynamics offsets within each band.
    let address = |offset| {
        if band {
            selected * 100 + offset
        } else {
            offset
        }
    };
    let numeric = |index: usize, spec: &ParamSpec, value: f64| {
        let mut spec = *spec;
        spec.name = text.label(spec.name);
        let render = if primary_columns && !wide && (matches!(index % 100, 6..=10) || index == 14) {
            render_narrow_horizontal_param
        } else {
            render_horizontal_param
        };
        render(
            d,
            entity.clone(),
            plugin_idx,
            index,
            &spec,
            value,
            state.is_editing,
            state.selected_param,
            true,
            theme,
        )
    };
    let choice = |index: usize, spec: &ParamSpec, value: bool| {
        let mut spec = *spec;
        spec.name = text.label(spec.name);
        render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            index,
            &spec,
            if value { 1.0 } else { 0.0 },
            theme,
        )
    };
    let mut tabs = div()
        .id(("mb-expander-bands", plugin_idx))
        .flex()
        .w_full()
        .min_w_0()
        .items_center()
        .overflow_x_scroll()
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .gap(d.grid);
    for index in 0..=state.num_bands {
        let change_entity = entity.clone();
        let label = if index == 0 {
            text.global.to_string()
        } else {
            band_tab_label(
                index,
                state.num_bands,
                [
                    state.crossover_freq_1,
                    state.crossover_freq_2,
                    state.crossover_freq_3,
                    state.crossover_freq_4,
                ],
            )
        };
        let button = Button::new(("mb-expander-band", index), label)
            .size(ButtonSize::Xs)
            .variant(if index == selected {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Ghost
            })
            .theme(theme.to_button_theme())
            .on_click(move |_, cx| {
                change_entity.update(cx, |state, cx| {
                    state.app.plugin_state.selected_eq_band = index;
                    cx.notify();
                });
            });
        #[cfg(feature = "dev-api")]
        let button = button.dev_track(format!("plugin.mb.band.{plugin_idx}.{index}"));
        tabs = tabs.child(button);
    }
    let mut toolbar = div().flex().flex_wrap().items_center().gap(d.grid);
    if band {
        for (offset, template, value) in [
            (17, 11, state.active),
            (15, 0, state.solo),
            (14, 1, state.bypass),
            (16, 10, state.auto_makeup),
        ] {
            toolbar = toolbar.child(choice(address(offset), &MEB[template], value));
        }
    }
    let dynamics = [
        (6, 2, state.threshold_db),
        (7, 3, state.ratio),
        (10, 6, state.range_db),
        (8, 4, state.attack_ms),
        (9, 5, state.release_ms),
    ];
    let mut left = div().flex_1().min_w_0().flex().flex_col().gap(d.grid);
    let mut right = div().flex_1().min_w_0().flex().flex_col().gap(d.grid);
    for (position, (offset, template, value)) in dynamics.into_iter().enumerate() {
        let control = numeric(
            address(offset),
            if band { &MEB[template] } else { &ME[offset] },
            value,
        );
        if primary_columns && position >= 3 {
            right = right.child(control);
        } else {
            left = left.child(control);
        }
    }
    right = right.child(numeric(14, &ME[14], state.mix));
    let response_width = if wide {
        available_width * 0.5
    } else {
        available_width
    };
    let response = || render_response(d, &state, response_width, layout_scale, text, theme);
    if wide {
        left = left.child(response());
    }
    let primary = div()
        .flex()
        .w_full()
        .min_w_0()
        .gap(d.grid)
        .when(!primary_columns, gpui::Styled::flex_col)
        .child(left)
        .child(right);
    let mut secondary = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(Text::label(if band {
            text.label("Band")
        } else {
            text.label("DYNAMICS")
        }));
    let mut global_controls = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(Text::label(text.global));
    for (offset, template, value) in [
        (11, 7, state.knee_db),
        (12, 8, state.hysteresis_db),
        (13, 9, state.hold_ms),
    ] {
        secondary = secondary.child(numeric(
            address(offset),
            if band { &MEB[template] } else { &ME[offset] },
            value,
        ));
    }
    for (offset, value) in [
        (2, state.crossover_freq_1),
        (3, state.crossover_freq_2),
        (4, state.crossover_freq_3),
        (5, state.crossover_freq_4),
    ] {
        if offset < state.num_bands + 1 {
            global_controls = global_controls.child(numeric(offset, &ME[offset], value));
        }
    }
    global_controls = global_controls.child(numeric(17, &ME[17], state.lookahead_ms));
    let mut detector = ME[16];
    detector.name = text.label(detector.name);
    global_controls = global_controls.child(render_compact_choice_param(
        d,
        entity.clone(),
        plugin_idx,
        16,
        &detector,
        settings.param_value(16).unwrap_or(detector.default_f64()),
        theme,
    ));
    let secondary = div()
        .w_full()
        .min_w_0()
        .flex()
        .gap(d.grid)
        .when(!wide, gpui::Styled::flex_col)
        .child(global_controls)
        .child(secondary);
    let app = &entity.read(cx).app;
    let instance = app
        .plugin_state
        .graph_state
        .editing_graph_node_uuid
        .and_then(|id| app.plugin_state.routing_controller().graph.nodes.get(&id))
        .map(|node| node.plugin.id)
        .or_else(|| {
            app.plugin_state
                .graph
                .get_plugin(plugin_idx)
                .map(|plugin| plugin.id)
        })
        .unwrap_or(plugin_idx);
    let section = "mb-expander-advanced";
    let expanded = app
        .plugin_state
        .plugin_ui_state
        .plugin_sections
        .get(&instance)
        .is_some_and(|sections| sections.iter().any(|id| id == section));
    let accordion = Accordion::new()
        .items(vec![
            AccordionItem::new(section, text.label("Advanced")).content(secondary),
        ])
        .mode(AccordionMode::Multiple)
        .expanded(if expanded {
            vec![SharedString::from(section)]
        } else {
            Vec::new()
        })
        .bordered(false)
        .rounded(false)
        .theme(theme.to_accordion_theme());
    #[cfg(feature = "dev-api")]
    let accordion = accordion
        .header_wrapper(move |_, header| header.dev_track("mbexp.advanced").into_any_element());
    let accordion = accordion.on_change(move |id, open, _, cx| {
        entity.update(cx, |state, cx| {
            let sections = state
                .app
                .plugin_state
                .plugin_ui_state
                .plugin_sections
                .entry(instance)
                .or_default();
            sections.retain(|section| section != id.as_ref());
            if open {
                sections.push(id.to_string());
            }
            cx.notify();
        });
    });
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(tabs)
        .when(band, |column| column.child(toolbar))
        .child(primary)
        .when(!wide, |column| column.child(response()))
        .child(accordion)
        .into_any_element()
}

/// Draw the DSP's static expansion target before envelope and makeup processing.
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
fn render_response(
    d: &Ds,
    state: &MbExpanderRenderState,
    available_width: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    // Include the threshold's full schema domain and maximum attenuation.
    let input: Vec<_> = (0..=160).map(|i| -80.0 + f64::from(i) * 0.5).collect();
    let output: Vec<_> = input
        .iter()
        .map(|&level| {
            let attenuation = target_attenuation_db(
                level as f32,
                state.threshold_db as f32,
                state.ratio as f32,
                state.knee_db as f32,
                state.range_db as f32,
            )
            .unwrap_or(0.0);
            level - f64::from(attenuation)
        })
        .collect();
    let chart = render_line_chart(
        vec![
            Series::new(
                "Unity",
                rgba_to_u32(theme.text_muted),
                input.clone(),
                input.clone(),
            )
            .with_opacity(0.35),
            Series::new("Expansion", rgba_to_u32(theme.accent), input, output),
        ],
        ChartConfig {
            title: None,
            x_label: None,
            y_label: None,
            x_range: (-80.0, 0.0),
            y_range: (-160.0, 0.0),
            x_scale: ScaleType::Linear,
            // Leave room for the primary column gap and the shell padding.
            width: (available_width - 32.0 * layout_scale).max(280.0 * layout_scale),
            height: 220.0 * layout_scale,
        },
        theme,
        None,
    );
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(chart)
        .child(Text::caption(text.label("Target response")))
        .into_any_element()
}
