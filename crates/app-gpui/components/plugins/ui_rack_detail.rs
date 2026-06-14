//! Plugin rack detail panel — meter rendering, plugin detail view, and add-plugin menu.
//!
//! Extracted from ui_rack.rs for maintainability.

use super::level_meters::render_gradient_meter;
use gpui_audio_kit::db_to_position;
use super::render_plugin_content;
use super::ui_plugin_shell::{plugin_accent_color as plugin_color, plugin_icon};
use crate::app::constants::spacing;
use crate::app::state::plugin::{PluginUiView, available_controllers};
use crate::app::state::{DividerDragState, DividerType};
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{CollapseDirection, PaneDivider, PaneDividerTheme};
use sotf_audio_player::PluginType;
use sotf_plugins::param_specs::{find_by_key as pk, upmixer::PARAMS as UP, aae::PARAMS as AAE_P};
use crate::components::themed_tooltip as make_tooltip;
use super::ui_rack::{plugin_description, short_name, speaker_config_to_channels};

mod misc;

