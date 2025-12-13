//! Upmixer Plugin UI Component
//!
//! Controls for the Upmixer plugin with:
//! - Speaker configuration selector
//! - Rotary knobs for gains and frequency controls
//! - Toggles for processing modes

use super::common::{render_knob, render_param_row, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Divider, HStack, Select, SelectOption, SelectSize, StackAlign, StackSpacing, Text, TextSize,
    VStack,
};

/// State for rendering the Upmixer plugin
pub struct UpmixerRenderState<'a> {
    pub speaker_config: &'a str,
    pub gain_front_direct: f64,
    pub stereo_width: f64,
    pub gain_front_ambient: f64,
    pub gain_rear_ambient: f64,
    pub lfe_cutoff_hz: f64,
    pub bandpass_hz: f64,
    pub height_gain: f64,
    pub lfe_gain: f64,
    pub enable_subharmonic_synth: bool,
    pub subharmonic_gain: f64,
    pub enable_hr_direct: bool,
    pub hr_sharpen: f64,
    pub safety_cap_db: f64,
    pub decorrelation_mode: usize,
    pub is_editing: bool,
    pub selected_param: usize,
    pub config_open: bool,
}

/// Render the upmixer plugin controls
/// Uses Entity<AppState> for direct state updates
pub fn render_upmixer_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: UpmixerRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // We need to own the string for the closure
    let speaker_config_owned = state.speaker_config.to_string();
    let config_open = state.config_open;
    let decorrelation_mode = state.decorrelation_mode;

    VStack::new()
        .spacing(StackSpacing::None)
        .child(
            // Top toolbar row
            HStack::new()
                .spacing(StackSpacing::Md)
                .build()
                .px_3()
                .py_2()
                .bg(theme.background_secondary)
                .border_b_1()
                .border_color(theme.border)
                .justify_between()
                // Speaker Config
                .child(
                    div().w(px(140.0)).child(
                        Select::new("config-select")
                            .options(
                                [
                                    "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                                    "9.1.4", "9.1.6",
                                ]
                                .iter()
                                .map(|c| SelectOption::new(c.to_string(), c.to_string()))
                                .collect(),
                            )
                            .selected(speaker_config_owned.clone())
                            .is_open(config_open)
                            .label("Config")
                            .size(SelectSize::Sm)
                            .on_toggle({
                                let entity = entity.clone();
                                move |is_open, _window, cx| {
                                    entity.update(cx, |state, _| {
                                        state.app.upmixer_config_open = is_open;
                                    });
                                }
                            })
                            .on_change({
                                let entity = entity.clone();
                                move |value, _, cx| {
                                    let configs = [
                                        "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2",
                                        "7.1.4", "9.1.4", "9.1.6",
                                    ];
                                    let idx = configs
                                        .iter()
                                        .position(|&c| c == value.as_ref())
                                        .unwrap_or(0);
                                    entity.update(cx, |state, _| {
                                        state.app.set_plugin_param(plugin_idx, 0, idx as f64);
                                        state.app.upmixer_config_open = false;
                                        state.app.needs_plugin_update = true;
                                        // Update level meters when channel config changes
                                        state.app.update_level_meter_groups();
                                    });
                                }
                            }),
                    ),
                )
                // Separator
                .child(
                    Divider::vertical()
                        .color(theme.border)
                        .build_simple()
                        .h(px(16.0)),
                )
                // Subharmonic Synth Toggle
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    "SubHarm",
                    state.enable_subharmonic_synth,
                    9,
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                // HR Direct Toggle
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    "HR Direct",
                    state.enable_hr_direct,
                    11,
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                // Decorrelation Mode
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(
                            Text::new("Decorrelation:")
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                        )
                        .child(render_param_row(
                            "",
                            if state.decorrelation_mode == 0 {
                                "Velvet"
                            } else {
                                "LFO"
                            },
                            14,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .build()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, {
                            let entity = entity.clone();
                            move |_, _, cx| {
                                entity.update(cx, |state, _| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        14,
                                        if decorrelation_mode == 0 { 1.0 } else { 0.0 },
                                    );
                                });
                            }
                        }),
                ),
        )
        // Main Row Container
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                .align(StackAlign::Start)
                .wrap(true)
                // 2. Gains (Center, LFE, Surround, Top)
                // Indices must match plugin_editing.rs
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Center",
                    state.gain_front_ambient,
                    -12.0,
                    12.0,
                    "dB",
                    2, // gain_front_ambient
                    state.selected_param,
                    state.is_editing,
                    Some('c'),
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "LFE",
                    state.lfe_gain,
                    -12.0,
                    12.0,
                    "dB",
                    8, // lfe_gain
                    state.selected_param,
                    state.is_editing,
                    Some('l'),
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Surr",
                    state.gain_rear_ambient,
                    -12.0,
                    12.0,
                    "dB",
                    3, // gain_rear_ambient
                    state.selected_param,
                    state.is_editing,
                    Some('s'),
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Top",
                    state.height_gain,
                    -12.0,
                    12.0,
                    "dB",
                    7, // height_gain
                    state.selected_param,
                    state.is_editing,
                    Some('t'),
                    theme,
                ))
                // 3. Frequency Knobs
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "LFE Cut",
                    state.lfe_cutoff_hz,
                    20.0,
                    180.0,
                    "Hz",
                    4, // lfe_cutoff_hz
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Bandpass",
                    state.bandpass_hz,
                    150.0,
                    350.0,
                    "Hz",
                    6, // bandpass_hz
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                // Safety Cap
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Safety",
                    state.safety_cap_db,
                    0.0,
                    3.0,
                    "dB",
                    13, // safety_cap_db
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .build()
                .p_4()
                .justify_center(),
        )
        // Edit Hint Bar removed
        .build()
        .w_full()
        .h_full()
}
