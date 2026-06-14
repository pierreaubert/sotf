//! Footer right section — device selection, volume control, and device popup menus.
//!
//! Extracted from footer.rs for maintainability.

#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::components::themed_tooltip as footer_tooltip;
use crate::ui::{FOOTER_HEIGHT_REMS, PlayerView};
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::VolumeKnob;
use gpui_ui_kit::{Menu, MenuItem};

mod misc;

