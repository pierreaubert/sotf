//! Dense XTC controls with one owner for every schema parameter.
//!
//! Geometry and cancellation stay visible. Tracking, filter modeling, room,
//! auto-gain tuning and diagnostic bypasses use the same bottom section grammar.

// Rust guideline compliant 2026-02-21
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::{AppState, ToastMessage, i18n::PluginCommonTranslations};
use crate::components::design::Ds;
use crate::components::plugins::actions::OpenIrFile;
use crate::components::plugins::custom_view_registry::CustomViewRenderContext;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::ui_layout_renderer::{
    render_compact_choice_param, render_compact_selector_param, render_horizontal_param,
};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::{AnyElement, App, Empty, Entity, SharedString, div};
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Button, ButtonSize, ButtonVariant, Text,
};
use sotf_audio_player::PluginSettings;
use sotf_plugins::param_specs::xtc::PARAMS;

#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
pub(crate) fn render_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    text: PluginCommonTranslations,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let mut model = PARAMS[27];
    model.name = text.label(model.name);
    let mut row = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .child(render_compact_selector_param(
            d,
            entity.clone(),
            plugin_idx,
            27,
            &model,
            settings.param_value(27).unwrap_or(model.default_f64()),
            theme,
            cx,
        ));
    for index in [13, 24] {
        let mut spec = PARAMS[index];
        spec.name = text.label(spec.name);
        row = row.child(render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            index,
            &spec,
            settings.param_value(index).unwrap_or(spec.default_f64()),
            theme,
        ));
    }
    row.into_any_element()
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
pub(crate) fn render_xtc(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let PluginSettings::XTC { room_ir_file, .. } = ctx.settings else {
        return Empty.into_any_element();
    };
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(ctx.entity.read(cx).app.ui_state.language);
    let value = |index: usize| {
        ctx.settings
            .param_value(index)
            .unwrap_or(PARAMS[index].default_f64())
    };
    let numeric = |index: usize, enabled: bool| {
        let mut spec = PARAMS[index];
        spec.name = text.label(spec.name);
        render_horizontal_param(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            index,
            &spec,
            value(index),
            ctx.is_editing,
            ctx.selected_param,
            enabled,
            ctx.theme,
        )
    };
    let choice = |index: usize| {
        let mut spec = PARAMS[index];
        spec.name = text.label(spec.name);
        render_compact_choice_param(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            index,
            &spec,
            value(index),
            ctx.theme,
        )
    };
    let geometry_label = format!(
        "{} {:.2} m · {} {:.1}° · {} {:.2} cm",
        text.label("Distance"),
        value(0),
        text.label("Speaker Angle"),
        value(1),
        text.label("Head Radius"),
        value(2) * PARAMS[2].display_scale,
    );
    let geometry_status = Text::caption(geometry_label.clone());
    #[cfg(feature = "dev-api")]
    let geometry_status = geometry_status.dev_track_with_state(
        format!("plugin.layout.xtc.geometry_status.{}", ctx.plugin_idx),
        crate::app::dev_api::DevElementState::default().text(geometry_label),
    );
    let wide = ctx.available_width / ctx.layout_scale.max(0.01) >= 800.0;
    let geometry = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(Text::label(text.label("GEOMETRY")))
        .child(numeric(0, true))
        .child(numeric(1, true))
        .child(numeric(2, true));
    let cancellation = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(Text::label(text.label("CANCELLATION")))
        .child(numeric(7, true))
        .child(numeric(12, true));
    let primary = div()
        .w_full()
        .min_w_0()
        .flex()
        .gap(d.grid)
        .when(!wide, gpui::Styled::flex_col)
        .child(geometry)
        .child(cancellation);
    let group = |indices: &[usize], enabled: bool| {
        let mut left = div().flex_1().min_w_0().flex().flex_col().gap(d.grid);
        let mut right = div().flex_1().min_w_0().flex().flex_col().gap(d.grid);
        for (position, &index) in indices.iter().enumerate() {
            if wide && position % 2 == 1 {
                right = right.child(numeric(index, enabled));
            } else {
                left = left.child(numeric(index, enabled));
            }
        }
        div()
            .w_full()
            .min_w_0()
            .flex()
            .gap(d.grid)
            .child(left)
            .when(wide, |row| row.child(right))
    };
    let tracking = group(&[3, 4, 5, 6], true);
    let filters = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(choice(14))
        .child(group(&[8, 9, 10, 11], true));
    let reflections = value(15) > 0.5;
    let room = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(choice(15))
        .child(render_room_ir(
            ctx,
            room_ir_file.as_deref(),
            reflections,
            text,
            &d,
        ))
        .child(group(&[17, 18, 19, 20], reflections));
    let output = group(&[25, 26], value(24) > 0.5);
    let diagnostics = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .gap(d.grid)
        .child(choice(21))
        .child(choice(22))
        .child(choice(23));
    let items = vec![
        AccordionItem::new("xtc-tracking", text.label("Head Tracking")).content(tracking),
        AccordionItem::new("xtc-filters", text.label("Filter Model")).content(filters),
        AccordionItem::new("xtc-room", text.label("Room")).content(room),
        AccordionItem::new("xtc-output", text.label("Auto Gain")).content(output),
        AccordionItem::new("xtc-diagnostics", text.label("Diagnostics")).content(diagnostics),
    ];
    let instance = ctx.plugin_instance_id.unwrap_or(ctx.plugin_idx);
    let expanded = ctx
        .entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .plugin_sections
        .get(&instance)
        .into_iter()
        .flatten()
        .filter(|id| id.starts_with("xtc-"))
        .map(|id| SharedString::from(id.clone()))
        .collect();
    let sections = Accordion::new()
        .items(items)
        .mode(AccordionMode::Multiple)
        .expanded(expanded)
        .bordered(false)
        .rounded(false)
        .theme(ctx.theme.to_accordion_theme());
    #[cfg(feature = "dev-api")]
    let sections = sections.header_wrapper(|id, header| {
        header
            .dev_track(format!("xtc.section.{id}"))
            .into_any_element()
    });
    let entity = ctx.entity.clone();
    let sections = sections.on_change(move |id, open, _, cx| {
        entity.update(cx, |state, cx| {
            let stored = state
                .app
                .plugin_state
                .plugin_ui_state
                .plugin_sections
                .entry(instance)
                .or_default();
            stored.retain(|section| section != id.as_ref());
            if open {
                stored.push(id.to_string());
            }
            cx.notify();
        });
    });
    let feedback = ctx
        .plugin_data
        .as_ref()
        .and_then(|data| data.downcast_ref::<sotf_plugins::plugin_xtc::XtcData>())
        .filter(|data| data.auto_gain.gain_db.is_finite() && data.limiter_envelope.is_finite())
        .map_or_else(
            || text.label("No live data").to_string(),
            |data| {
                format!(
                    "Auto gain {:+.1} dB · Limiter {:.1} dB",
                    data.auto_gain.gain_db,
                    -20.0 * data.limiter_envelope.clamp(1e-9, 1.0).log10()
                )
            },
        );
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(geometry_status)
        .child(primary)
        .child(Text::caption(feedback))
        .child(Text::label(text.label("Advanced")))
        .child(sections)
        .into_any_element()
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
fn render_room_ir(
    ctx: &CustomViewRenderContext,
    path: Option<&str>,
    enabled: bool,
    text: PluginCommonTranslations,
    d: &Ds,
) -> AnyElement {
    let plugin_idx = ctx.plugin_idx;
    let has_file = path.is_some_and(|path| !path.is_empty());
    let filename = path
        .filter(|path| !path.is_empty())
        .and_then(|path| path.rsplit(['/', '\\']).next())
        .unwrap_or(text.label("None"))
        .to_string();
    let browse = Button::new(("xtc-ir-browse", plugin_idx), text.label("Browse"))
        .size(ButtonSize::Xs)
        .variant(ButtonVariant::Secondary)
        .theme(ctx.theme.to_button_theme())
        .disabled(!enabled)
        .on_click(move |window, cx| {
            window.dispatch_action(
                Box::new(OpenIrFile {
                    plugin_idx,
                    param_idx: 16,
                }),
                cx,
            );
        });
    #[cfg(feature = "dev-api")]
    let browse = browse.dev_track(format!("plugin.file_picker.{plugin_idx}.room_ir_file"));
    let entity = ctx.entity.clone();
    let clear = Button::new(("xtc-ir-clear", plugin_idx), text.label("Clear"))
        .size(ButtonSize::Xs)
        .variant(ButtonVariant::Ghost)
        .theme(ctx.theme.to_button_theme())
        .disabled(!enabled || !has_file)
        .on_click(move |_, cx| {
            entity.update(cx, |state, cx| {
                if let Err(error) = state
                    .app
                    .set_plugin_param_string(plugin_idx, 16, String::new())
                {
                    state.app.ui_state.toast_message = Some(ToastMessage::error(error));
                }
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let clear = clear.dev_track(format!("plugin.file_clear.{plugin_idx}.room_ir_file"));
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .gap(d.grid)
        .child(Text::label(text.label("Room IR")))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .text_size(d.text_sm)
                .text_color(ctx.theme.text_secondary)
                .child(filename),
        )
        .child(browse)
        .child(clear)
        .into_any_element()
}
