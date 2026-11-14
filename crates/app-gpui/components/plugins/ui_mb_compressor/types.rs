//! Dense multiband compressor controls with stable desktop parameter addresses.

// Rust guideline compliant 2026-02-21
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::{AppState, i18n::PluginCommonTranslations};
use crate::components::design::Ds;
use crate::components::plugins::common::render_transfer_curve_with_level;
use crate::components::plugins::ui_layout_renderer::{
    render_compact_choice_param, render_horizontal_param, render_narrow_horizontal_param,
};
use crate::components::plugins::ui_multiband_common::{band_tab_label, primary_uses_columns};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, SharedString, div};
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Button, ButtonSize, ButtonVariant, Text,
};
use sotf_audio_player::PluginSettings;
use sotf_plugins::param_specs::{
    ParamSpec,
    multiband_compressor::{BAND_TEMPLATE as MCB, GLOBAL_PARAMS as MC},
};

/// Snapshot of global and selected-band settings for the editor.
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent UI options and status flags retain their existing state schema"
)]
pub struct MbCompressorRenderState {
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
    pub knee_db: f64,
    pub makeup_gain_db: f64,
    pub auto_makeup: bool,
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
    pub mix: f64,
    pub link_channels: bool,
    pub per_band_lookahead_ms: f64,
    pub ms_mode: bool,
    pub sidechain_tilt_db: f64,
    pub link_amount: f64,
    pub range_db: f64,
    pub hold_ms: f64,
    pub band_range_db: f64,
    pub band_hold_ms: f64,
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
pub fn render_mb_compressor_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbCompressorRenderState,
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
        let render =
            if primary_columns && !wide && (matches!(index % 100, 6..=9 | 13) || index == 11) {
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
        .id(("mb-compressor-bands", plugin_idx))
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
        let button = Button::new(("mb-compressor-band", index), label)
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
            (17, 9, state.active),
            (15, 0, state.solo),
            (14, 1, state.bypass),
            (16, 8, state.auto_makeup),
        ] {
            toolbar = toolbar.child(choice(address(offset), &MCB[template], value));
        }
    }
    let dynamics = [
        (6, 2, state.threshold_db),
        (7, 3, state.ratio),
        (8, 4, state.attack_ms),
        (9, 5, state.release_ms),
    ];
    let mut left = div().flex_1().min_w_0().flex().flex_col().gap(d.grid);
    let mut right = div().flex_1().min_w_0().flex().flex_col().gap(d.grid);
    for (position, (offset, template, value)) in dynamics.into_iter().enumerate() {
        let control = numeric(
            address(offset),
            if band { &MCB[template] } else { &MC[offset] },
            value,
        );
        if primary_columns && position >= 2 {
            right = right.child(control);
        } else {
            left = left.child(control);
        }
    }
    if band {
        let makeup = numeric(address(13), &MCB[7], state.makeup_gain_db);
        // Balance three controls per narrow lane. Makeup's two-line label
        // otherwise pushes the fourth right-lane control below a short rack.
        if primary_columns && !wide {
            left = left.child(makeup);
        } else {
            right = right.child(makeup);
        }
    }
    right = right.child(numeric(11, &MC[11], state.mix));
    let response = || {
        render_transfer_curve_with_level(
            d,
            state.threshold_db,
            state.ratio,
            state.knee_db,
            false,
            140.0 * layout_scale,
            None,
            theme,
        )
    };
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
    secondary = secondary.child(numeric(
        address(10),
        if band { &MCB[6] } else { &MC[10] },
        state.knee_db,
    ));
    for (offset, value) in [
        (2, state.crossover_freq_1),
        (3, state.crossover_freq_2),
        (4, state.crossover_freq_3),
        (5, state.crossover_freq_4),
    ] {
        if offset < state.num_bands + 1 {
            global_controls = global_controls.child(numeric(offset, &MC[offset], value));
        }
    }
    global_controls = global_controls
        .child(numeric(13, &MC[13], state.per_band_lookahead_ms))
        .child(numeric(16, &MC[16], state.link_amount));
    // Band overrides inherit global Range/Hold until edited. Keep the
    // global defaults separately visible so changing a band never edits them.
    secondary = secondary
        .child(numeric(
            address(if band { 18 } else { 17 }),
            if band { &MCB[10] } else { &MC[17] },
            if band {
                state.band_range_db
            } else {
                state.range_db
            },
        ))
        .child(numeric(
            address(if band { 19 } else { 18 }),
            if band { &MCB[11] } else { &MC[18] },
            if band {
                state.band_hold_ms
            } else {
                state.hold_ms
            },
        ));
    secondary = secondary
        .child(Text::label(text.label("Sidechain")))
        .child(numeric(15, &MC[15], state.sidechain_tilt_db));
    let mut detector_choices = div().flex().flex_wrap().items_center().gap(d.grid);
    for index in [22, 21, 20] {
        let mut spec = MC[index];
        spec.name = text.label(spec.name);
        detector_choices = detector_choices.child(render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            index,
            &spec,
            settings.param_value(index).unwrap_or(spec.default_f64()),
            theme,
        ));
    }
    secondary = secondary.child(detector_choices);
    if settings.param_value(22).unwrap_or(0.0) > 0.5 {
        secondary = secondary.child(numeric(
            19,
            &MC[19],
            settings.param_value(19).unwrap_or(MC[19].default_f64()),
        ));
    }
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
    let section = "mb-compressor-advanced";
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
        .header_wrapper(move |_, header| header.dev_track("mbcomp.advanced").into_any_element());
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
