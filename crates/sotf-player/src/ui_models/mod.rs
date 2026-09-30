//! UI-agnostic view models shared between GPUI and TUI shells.
//!
//! The modules here hold cross-platform screen state and user-intent events
//! so that `app-gpui` and `app-tui` remain thin rendering layers.

pub mod audio_preferences;
#[cfg(not(target_os = "ios"))]
pub mod capture;
pub mod correction_delivery;
pub mod headphone_eq;
pub mod recording {
    //! Recording wizard model (owned by sotf-capture).
    pub use sotf_capture::wizard::*;
}
pub mod room_eq;
pub mod spinorama_eq;
pub mod take_review;
