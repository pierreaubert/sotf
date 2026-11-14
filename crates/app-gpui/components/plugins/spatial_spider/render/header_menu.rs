use super::super::{SpatialSpiderSnapshot, SpiderViewMode};
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, deferred, div, rgba};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Menu, MenuItem, Popover, PopoverPlacement, PopoverTheme,
};
use sotf_plugins::speaker_config::SpeakerConfig;

#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub fn render_spatial_header_menu(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    snapshot: &SpatialSpiderSnapshot,
    config: Option<&'static SpeakerConfig>,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    let ui = &snapshot.ui;
    let _ = config;
    let mode = match ui.view_mode {
        SpiderViewMode::Off => "Off",
        SpiderViewMode::Disc2D => "2D",
        SpiderViewMode::View3D => "3D",
    };
    #[allow(
        unused_mut,
        reason = "Other platform and feature configurations require mutation"
    )]
    let mut items = vec![
        MenuItem::checkbox("off", "Off", ui.view_mode == SpiderViewMode::Off),
        MenuItem::checkbox("2d", "2D", ui.view_mode == SpiderViewMode::Disc2D),
    ];
    #[cfg(feature = "gpu-3d")]
    items.push(MenuItem::checkbox(
        "3d",
        "3D",
        ui.view_mode == SpiderViewMode::View3D,
    ));
    let toggle_entity = entity.clone();
    let select_entity = entity.clone();
    let close_entity = entity.clone();
    let menu = Menu::new(("spatial-header-menu", plugin_idx), items)
        .theme(theme.to_menu_theme())
        .on_select(move |id, _, cx| {
            select_entity.update(cx, |state, cx| {
                let ui = &mut state.app.plugin_ui.spatial_spider;
                match id.as_ref() {
                    "off" => ui.view_mode = SpiderViewMode::Off,
                    "2d" => ui.view_mode = SpiderViewMode::Disc2D,
                    "3d" => ui.view_mode = SpiderViewMode::View3D,
                    _ => return,
                }
                ui.menu_open = false;
                ui.correlation_menu_open = false;
                cx.notify();
            });
        });
    let content = div().flex().items_start().child(menu);
    let button = Button::new(
        ("spatial-header-trigger", plugin_idx),
        format!("{}: {mode} ▾", text.spatial.view_label),
    )
    .size(ButtonSize::Sm)
    .variant(ButtonVariant::Secondary)
    .theme(theme.to_button_theme())
    .on_click_event(move |_, _, cx| {
        toggle_entity.update(cx, |state, cx| {
            let ui = &mut state.app.plugin_ui.spatial_spider;
            ui.menu_open = !ui.menu_open;
            ui.correlation_menu_open = false;
            cx.notify();
        });
    });
    #[cfg(feature = "dev-api")]
    let button = {
        use crate::app::dev_api::{DevElementState, DevTrackExt};
        button.dev_track_with_state(
            format!("plugin.shell.{plugin_idx}.spatial"),
            DevElementState::default().text(mode),
        )
    };
    div()
        .relative()
        .flex_shrink_0()
        .mr(d.gap)
        .child(button)
        .when(ui.menu_open, |el| {
            el.child(
                deferred(
                    Popover::new(("spatial-header-popover", plugin_idx))
                        .placement(PopoverPlacement::BottomEnd)
                        .content(content)
                        .on_close(move |_, cx| {
                            close_entity.update(cx, |state, cx| {
                                state.app.plugin_ui.spatial_spider.menu_open = false;
                                state.app.plugin_ui.spatial_spider.correlation_menu_open = false;
                                cx.notify();
                            });
                        })
                        .build_with_theme(&PopoverTheme {
                            background: theme.surface,
                            border: theme.border,
                            backdrop: rgba(0x0000_0001), // intentional: invisible click-outside hit area matches toolkit Popover default
                        }),
                )
                .with_priority(1),
            )
        })
        .into_any_element()
}
