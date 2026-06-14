// The `imp` module is compiled on the host during unit tests so the lock-free
// pending-queue tests can run without an iOS simulator. Most items here are
// only used on iOS/tvOS, so silence the resulting dead-code warnings on macOS.
#![cfg_attr(
    not(any(target_os = "ios", target_os = "tvos")),
    allow(dead_code, unused_imports)
)]

use gpui::*;
use rust_embed::RustEmbed;
use sotf_audio_player::{Player, SotfRemoteAuthToken, SotfRemoteServer};
use sotf_audio_player_gpui::app::state::ui::LayoutState;
use sotf_audio_player_gpui::app::{App, AppState};
use sotf_audio_player_gpui::ui;
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::panic::{AssertUnwindSafe, UnwindSafe};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

mod assets;
mod consts;
mod cstring;
mod misc;
mod pending;
mod remote_command;
mod sotf;
mod types;
mod update;

pub use cstring::*;
pub use misc::*;
pub use remote_command::*;
pub use sotf::*;
pub use types::*;
pub use update::*;
